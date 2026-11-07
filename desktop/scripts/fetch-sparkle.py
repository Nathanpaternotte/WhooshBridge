#!/usr/bin/env python3
"""Fetch the pinned official distribution and verify it before extraction."""
import hashlib
import json
from pathlib import Path
import subprocess
import tarfile

ROOT = Path(__file__).resolve().parents[1]


def fetch():
    config = json.loads((ROOT / 'update-config.json').read_text())
    version = config['sparkle_version']
    cache = ROOT / 'target' / 'sparkle'
    cache.mkdir(parents=True, exist_ok=True)
    archive = cache / f'{version}.tar.xz'
    expected = config['sparkle_sha256']
    if not archive.exists() or hashlib.sha256(archive.read_bytes()).hexdigest() != expected:
        subprocess.run(['curl', '-fLsS', '--retry', '3', '--proto', '=https',
                        f'https://github.com/sparkle-project/Sparkle/releases/download/{version}/Sparkle-{version}.tar.xz',
                        '-o', str(archive)], check=True)
    if hashlib.sha256(archive.read_bytes()).hexdigest() != expected:
        raise SystemExit('Sparkle checksum mismatch; refusing to extract.')
    destination = cache / version
    destination.mkdir(exist_ok=True)
    with tarfile.open(archive) as source:
        source.extractall(destination, filter='data')
    return destination


if __name__ == '__main__':
    print(fetch())
