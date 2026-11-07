#!/bin/bash
# Build an architecture-specific, self-contained .app on macOS.
set -euo pipefail
DESKTOP_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$DESKTOP_DIR"
export PATH="$HOME/.cargo/bin:$PATH"
if [ "$(uname -s)" != Darwin ]; then
  echo 'This package must be built on macOS.' >&2
  exit 1
fi
BUILD_PYTHON="${WHOOSH_PYTHON:-$DESKTOP_DIR/../venv/bin/python}"
if [ ! -x "$BUILD_PYTHON" ]; then BUILD_PYTHON=python3; fi
"$BUILD_PYTHON" -c 'import sys; assert sys.version_info >= (3, 11), "Python 3.11+ required. Set WHOOSH_PYTHON."'
"$BUILD_PYTHON" -m venv .build-venv312
.build-venv312/bin/python -m pip install -r requirements-build.txt
export PYINSTALLER_CONFIG_DIR="$DESKTOP_DIR/target/pyinstaller-cache"
cargo build --release --locked --bins
.build-venv312/bin/python -m PyInstaller --noconfirm --clean --onedir \
  --name garmin-bridge --distpath target/sidecar --workpath target/pyinstaller \
  bridge/garmin_bridge.py
APP="target/macos/Whoosh Bridge.app"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp assets/WhooshBridge.icns "$APP/Contents/Resources/WhooshBridge.icns"
cp ../LICENSE ../THIRD_PARTY_NOTICES.md "$APP/Contents/Resources/"
cp target/release/whoosh-bridge "$APP/Contents/MacOS/WhooshBridge.new"
mv -f "$APP/Contents/MacOS/WhooshBridge.new" "$APP/Contents/MacOS/WhooshBridge"
cp target/release/whoosh-web-login "$APP/Contents/MacOS/whoosh-web-login.new"
mv -f "$APP/Contents/MacOS/whoosh-web-login.new" "$APP/Contents/MacOS/whoosh-web-login"
# ditto replaces existing files while retaining executable modes.
ditto target/sidecar/garmin-bridge "$APP/Contents/Resources/garmin-bridge"
cat > "$APP/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleName</key><string>Whoosh Bridge</string>
<key>CFBundleDisplayName</key><string>Whoosh Bridge</string>
<key>CFBundleIdentifier</key><string>com.mywhooshbridge.desktop</string>
<key>CFBundleVersion</key><string>1</string>
<key>CFBundleShortVersionString</key><string>0.1.0</string>
<key>CFBundleExecutable</key><string>WhooshBridge</string>
<key>CFBundleIconFile</key><string>WhooshBridge</string>
<key>CFBundleLocalizations</key><array><string>en</string><string>fr</string></array>
<key>CFBundleDevelopmentRegion</key><string>en</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>LSMinimumSystemVersion</key><string>14.0</string>
<key>NSHighResolutionCapable</key><true/>
<key>NSPrincipalClass</key><string>NSApplication</string>
</dict></plist>
PLIST
.build-venv312/bin/python scripts/collect-licenses.py "$APP/Contents/Resources/licenses"
codesign --force --deep --sign - "$APP"
echo "Application created: $DESKTOP_DIR/$APP"
