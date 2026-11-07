#!/usr/bin/env python3
"""Write version and update settings to the packaged app, never private keys."""
import base64
import json
from pathlib import Path
import plistlib
import sys
import tomllib

ROOT = Path(__file__).resolve().parents[1]


def configure(app, arch):
    if arch not in {'arm64', 'x86_64'}:
        raise ValueError('An architecture-specific update feed is required')
    config = json.loads((ROOT / 'update-config.json').read_text())
    if len(base64.b64decode(config['public_ed_key'], validate=True)) != 32:
        raise ValueError('Invalid Sparkle public key')
    if type(config['build_number']) is not int or config['build_number'] < 1:
        raise ValueError('A positive, increasing build number is required')
    version = tomllib.loads((ROOT / 'Cargo.toml').read_text())['package']['version']
    path = app / 'Contents' / 'Info.plist'
    info = plistlib.loads(path.read_bytes())
    info.update(CFBundleVersion=str(config['build_number']), CFBundleShortVersionString=version,
                SUFeedURL=f"{config['feed_base_url']}/appcast-{arch}.xml",
                SUPublicEDKey=config['public_ed_key'], SUVerifyUpdateBeforeExtraction=True,
                SURequireSignedFeed=True, SUSignedFeedFailureExpirationInterval=0,
                SUEnableAutomaticChecks=True, SUAutomaticallyUpdate=False,
                SUAllowsAutomaticUpdates=False, SUEnableSystemProfiling=False,
                SUScheduledCheckInterval=86400)
    path.write_bytes(plistlib.dumps(info))


if __name__ == '__main__':
    configure(Path(sys.argv[1]), sys.argv[2])
