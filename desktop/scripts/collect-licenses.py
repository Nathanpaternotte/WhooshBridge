"""Collect dependency notices from local build inputs; never read app secrets."""
import argparse
import importlib.metadata
import json
import os
from pathlib import Path
import shutil
import sys
import tomllib

parser = argparse.ArgumentParser()
parser.add_argument('destination', type=Path)
args = parser.parse_args()
root = Path(__file__).resolve().parents[1]
args.destination.mkdir(parents=True, exist_ok=True)
index = []

def legal(name):
    return name.upper().startswith(('LICENSE', 'LICENCE', 'COPYING', 'NOTICE', 'AUTHORS', 'COPYRIGHT'))

cargo = Path(os.environ.get('CARGO_HOME', str(Path.home()/'.cargo')))
lock = tomllib.loads((root/'Cargo.lock').read_text())
for package in lock['package']:
    if not package.get('source', '').startswith('registry+'):
        continue
    folder = next((cargo/'registry/src').glob(f"*/{package['name']}-{package['version']}"), None)
    if folder is None:
        # Target-specific dependencies not built on this host need not be bundled.
        continue
    files = [p for p in folder.iterdir() if p.is_file() and legal(p.name)]
    for sub in ['licenses', 'LICENSES']:
        if (folder/sub).is_dir():
            files.extend(p for p in (folder/sub).rglob('*') if p.is_file())
    entry = {'ecosystem':'rust', 'name':package['name'], 'version':package['version']}
    manifest = tomllib.loads((folder/'Cargo.toml').read_text())['package']
    entry['license'] = manifest.get('license', 'See upstream license')
    destination = args.destination/f"rust-{package['name']}-{package['version']}"
    destination.mkdir(exist_ok=True)
    for path in files:
        relative = path.relative_to(folder)
        target = destination/relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(path, target)
    entry['notice_files'] = len(files)
    index.append(entry)
for dist in importlib.metadata.distributions():
    name, version = dist.metadata['Name'], dist.version
    files = [f for f in dist.files or [] if legal(Path(str(f)).name) and Path(dist.locate_file(f)).is_file()]
    destination = args.destination/f'python-{name}-{version}'
    destination.mkdir(exist_ok=True)
    for number, filename in enumerate(files):
        shutil.copy2(dist.locate_file(filename), destination/f'{number}-{Path(str(filename)).name}')
    index.append({'ecosystem':'python', 'name':name, 'version':version,
                  'license':dist.metadata.get('License-Expression') or dist.metadata.get('License') or 'See upstream license', 'notice_files':len(files)})
python_license = Path(sys.base_prefix)/'LICENSE.txt'
if python_license.is_file():
    shutil.copy2(python_license, args.destination/'Python-LICENSE.txt')
(args.destination/'index.json').write_text(json.dumps(index, indent=2)+'\n')
print(f'Collected notices for {len(index)} local build dependencies.')
