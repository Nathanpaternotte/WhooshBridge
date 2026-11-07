#!/usr/bin/env python3
"""Export the maintained source tree without requiring a Git repository."""
from pathlib import Path
import zipfile

ROOT = Path(__file__).resolve().parents[1]
ROOT_FILES = {'AGENTS.md', 'LICENSE', 'README.md', 'README.fr.md',
              'THIRD_PARTY_NOTICES.md', '.gitignore'}
TREES = {'.github', 'desktop/src', 'desktop/bridge', 'desktop/assets',
         'desktop/locales', 'desktop/scripts', 'desktop/tests', 'docs', 'scripts'}
APP_FILES = {'desktop/Cargo.toml', 'desktop/Cargo.lock', 'desktop/README.md',
             'desktop/requirements-runtime.txt', 'desktop/requirements-build.txt',
             'desktop/requirements-test.txt'}
BLOCKED_PARTS = {'.git', '__pycache__', '.pytest_cache', 'target', 'venv', 'dist'}
BLOCKED_SUFFIXES = {'.fit', '.tcx', '.dms', '.log', '.pem', '.key', '.p12', '.pyc', '.zip'}


def source_files():
    candidates = [ROOT / name for name in ROOT_FILES | APP_FILES]
    for name in TREES:
        candidates.extend((ROOT / name).rglob('*'))
    result = []
    for path in sorted(set(candidates)):
        relative = path.relative_to(ROOT)
        if any(part in BLOCKED_PARTS for part in relative.parts):
            continue
        if path.is_symlink() or any(parent.is_symlink() for parent in path.parents if parent != ROOT):
            raise ValueError(f'Symlinks are not allowed in source exports: {relative}')
        if not path.is_file() or path.name == '.DS_Store':
            continue
        if path.name.startswith('.env') or path.suffix.lower() in BLOCKED_SUFFIXES:
            raise ValueError(f'Private/generated file in a source directory: {relative}')
        result.append(path)
    for name in ROOT_FILES | APP_FILES:
        if not (ROOT / name).is_file():
            raise ValueError(f'Required source file missing: {name}')
    return result


def main():
    files = source_files()
    archive = ROOT / 'dist/Whoosh-Bridge-source.zip'
    archive.parent.mkdir(exist_ok=True)
    with zipfile.ZipFile(archive, 'w', zipfile.ZIP_DEFLATED) as output:
        for path in files:
            output.write(path, 'Whoosh-Bridge/' + path.relative_to(ROOT).as_posix())
    print(f'Exported {len(files)} source files to {archive.relative_to(ROOT)} (no Git history).')


if __name__ == '__main__':
    main()
