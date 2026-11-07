#!/usr/bin/env python3
"""Prepare a signed architecture-specific feed; never publish automatically."""
import argparse
import base64
import hashlib
import json
from pathlib import Path
import plistlib
import re
import shutil
import subprocess
import tempfile
from urllib.parse import quote
import xml.etree.ElementTree as ET
import zipfile

ROOT = Path(__file__).resolve().parents[1]
SPARKLE = '{http://www.andymatuschak.org/xml-namespaces/sparkle}'


def validate_archive(archive, config):
    with zipfile.ZipFile(archive) as source:
        info = plistlib.loads(source.read('Whoosh Bridge.app/Contents/Info.plist'))
    expected_feeds = {f"{config['feed_base_url']}/appcast-{arch}.xml": arch
                      for arch in ('arm64', 'x86_64')}
    arch = expected_feeds.get(info.get('SUFeedURL'))
    if not arch or info.get('SUPublicEDKey') != config['public_ed_key']:
        raise ValueError('Archive feed or signing key does not match update-config.json')
    if info.get('CFBundleVersion') != str(config['build_number']):
        raise ValueError('Archive build number is stale; rebuild the app')
    if not info.get('SUVerifyUpdateBeforeExtraction') or not info.get('SURequireSignedFeed'):
        raise ValueError('Archive must require signed archives and feeds')
    return info, arch


def validate_feed(path, archive, config, tag, info):
    root = ET.fromstring(path.read_bytes())
    items = root.findall('./channel/item')
    if len(items) != 1 or items[0].findtext(SPARKLE + 'version') != info['CFBundleVersion']:
        raise ValueError('Unexpected appcast version')
    enclosure = items[0].find('enclosure')
    expected = f"https://github.com/{config['repository']}/releases/download/{quote(tag, safe='')}/{quote(archive.name)}"
    if enclosure is None or enclosure.get('url') != expected:
        raise ValueError('Unexpected download URL')
    if int(enclosure.get('length', '0')) != archive.stat().st_size:
        raise ValueError('Incorrect archive size')
    if len(base64.b64decode(enclosure.get(SPARKLE + 'edSignature', ''), validate=True)) != 64:
        raise ValueError('Missing archive signature')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--tag', required=True, help='Future GitHub Release tag (immutable archive URL)')
    parser.add_argument('--archive', type=Path, required=True)
    args = parser.parse_args()
    if not re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9._+-]*', args.tag):
        parser.error('Use a simple release tag without slashes or whitespace')
    config = json.loads((ROOT / 'desktop/update-config.json').read_text())
    archive = args.archive.resolve()
    info, arch = validate_archive(archive, config)
    tools = ROOT / 'desktop/target/sparkle' / config['sparkle_version'] / 'bin'
    public = subprocess.check_output([str(tools / 'generate_keys'), '--account',
                                      config['keychain_account'], '-p'], text=True).strip()
    if public != config['public_ed_key']:
        raise ValueError('Keychain signing key does not match the bundled public key')
    destination = ROOT / 'updates' / f'appcast-{arch}.xml'
    if destination.exists():
        previous = ET.fromstring(destination.read_bytes()).findall('./channel/item')
        for item in previous:
            if int(item.findtext(SPARKLE + 'version', '0')) >= int(info['CFBundleVersion']):
                raise ValueError('Increase build_number before replacing an existing feed entry')
    # Only the requested archive is scanned: never mix source ZIPs or architectures.
    with tempfile.TemporaryDirectory(prefix='whoosh-update-') as temporary:
        stage = Path(temporary)
        shutil.copy2(archive, stage / archive.name)
        feed = stage / destination.name
        subprocess.run([str(tools / 'generate_appcast'), '--account', config['keychain_account'],
                        '--maximum-deltas', '0', '--download-url-prefix',
                        f"https://github.com/{config['repository']}/releases/download/{quote(args.tag, safe='')}/",
                        '--link', f"https://github.com/{config['repository']}",
                        '-o', str(feed), str(stage)], check=True)
        validate_feed(feed, archive, config, args.tag, info)
        enclosure = ET.fromstring(feed.read_bytes()).find('./channel/item/enclosure')
        for signed_file, signature in [(archive, enclosure.get(SPARKLE + 'edSignature')), (feed, None)]:
            command = [str(tools / 'sign_update'), '--account', config['keychain_account'],
                       '--verify', str(signed_file)]
            if signature:
                command.append(signature)
            subprocess.run(command, check=True)
        destination.parent.mkdir(exist_ok=True)
        shutil.copy2(feed, destination)
    # Save a publication manifest so the exact file can be checked before upload.
    manifest = {'tag': args.tag, 'archive': archive.name, 'build': info['CFBundleVersion'],
                'sha256': hashlib.sha256(archive.read_bytes()).hexdigest(),
                'feed': str(destination.relative_to(ROOT))}
    (archive.parent / f'update-{arch}.json').write_text(json.dumps(manifest, indent=2) + '\n')
    print(f'Signed feed ready: {destination.relative_to(ROOT)}')
    print('Not published. Upload this exact ZIP to the specified Release before publishing the feed.')


if __name__ == '__main__':
    main()
