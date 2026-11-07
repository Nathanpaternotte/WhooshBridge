#!/usr/bin/env python3
"""Create an isolated two-version updater fixture. Never touches the real app."""
import json
from pathlib import Path
import plistlib
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]
CONFIG = json.loads((ROOT / 'update-config.json').read_text())
STAGE = ROOT / 'target/updater-smoke'
FRAMEWORK = ROOT / 'target/sparkle' / CONFIG['sparkle_version']


def run(*args):
    subprocess.run([str(arg) for arg in args], check=True)


def main():
    STAGE.mkdir(parents=True, exist_ok=True)
    for version, folder in [('1', 'installed'), ('2', 'candidate')]:
        app = STAGE / folder / 'Whoosh Update Test.app'
        contents = app / 'Contents'
        (contents / 'MacOS').mkdir(parents=True, exist_ok=True)
        (contents / 'Frameworks').mkdir(exist_ok=True)
        run('ditto', FRAMEWORK / 'Sparkle.framework', contents / 'Frameworks/Sparkle.framework')
        shutil.copy2(ROOT / 'target/macos/Whoosh Bridge.app/Contents/Frameworks/libWhooshUpdater.dylib', contents / 'Frameworks')
        info = dict(CFBundleIdentifier='com.mywhooshbridge.updatetest', CFBundleName='Whoosh Update Test',
                    CFBundleExecutable='UpdateTest', CFBundlePackageType='APPL',
                    CFBundleVersion=version, CFBundleShortVersionString=f'1.0.{version}',
                    LSMinimumSystemVersion='14.0', NSPrincipalClass='NSApplication',
                    SUFeedURL='http://127.0.0.1:18765/appcast.xml',
                    SUPublicEDKey=CONFIG['public_ed_key'], SUVerifyUpdateBeforeExtraction=True,
                    SURequireSignedFeed=True, SUEnableAutomaticChecks=False,
                    SUAllowsAutomaticUpdates=False, SUEnableSystemProfiling=False,
                    NSAppTransportSecurity={'NSAllowsLocalNetworking': True})
        (contents / 'Info.plist').write_bytes(plistlib.dumps(info))
        run('xcrun', 'clang', '-fobjc-arc', '-fblocks', '-mmacosx-version-min=14.0',
            '-framework', 'Cocoa', '-L', contents / 'Frameworks', '-lWhooshUpdater',
            '-Wl,-rpath,@executable_path/../Frameworks', ROOT / 'tests/updater_smoke.m',
            '-o', contents / 'MacOS/UpdateTest')
        run('codesign', '--force', '--deep', '--sign', '-', app)
    server = STAGE / 'server'
    server.mkdir(exist_ok=True)
    run('ditto', '-c', '-k', '--sequesterRsrc', '--keepParent',
        STAGE / 'candidate/Whoosh Update Test.app', server / 'update.zip')
    run(FRAMEWORK / 'bin/generate_appcast', '--account', CONFIG['keychain_account'],
        '--maximum-deltas', '0', '--download-url-prefix', 'http://127.0.0.1:18765/', server)
    print('Serve target/updater-smoke/server on 127.0.0.1:18765 and launch the installed test app.')
    print('After Install and Relaunch, installed/launched-version.txt must contain 2.')


if __name__ == '__main__':
    main()
