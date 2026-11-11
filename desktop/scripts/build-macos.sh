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
SPARKLE_DIR="$("$BUILD_PYTHON" scripts/fetch-sparkle.py)"
cargo build --release --locked --bin whoosh-engine --bin whoosh-web-login
bash scripts/build-swiftui.sh
.build-venv312/bin/python -m PyInstaller --noconfirm --clean --onedir \
  --name garmin-bridge --distpath target/sidecar --workpath target/pyinstaller \
  bridge/garmin_bridge.py
APP="target/macos/Whoosh Bridge.app"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp assets/WhooshBridge.icns "$APP/Contents/Resources/WhooshBridge.icns"
cp ../LICENSE ../THIRD_PARTY_NOTICES.md "$APP/Contents/Resources/"
cp target/WhooshBridge-SwiftUI "$APP/Contents/MacOS/WhooshBridge.new"
cp target/release/whoosh-engine "$APP/Contents/MacOS/whoosh-engine.new"
mv -f "$APP/Contents/MacOS/whoosh-engine.new" "$APP/Contents/MacOS/whoosh-engine"
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
"$BUILD_PYTHON" scripts/configure-updates.py "$APP" "$(uname -m)"
mkdir -p "$APP/Contents/Frameworks"
ditto "$SPARKLE_DIR/Sparkle.framework" "$APP/Contents/Frameworks/Sparkle.framework"
xcrun clang -dynamiclib -fobjc-arc -fblocks -mmacosx-version-min=14.0 \
  -Wall -Wextra -Werror -Wno-unused-parameter \
  -F "$SPARKLE_DIR" -framework Cocoa -framework Sparkle \
  -Wl,-rpath,@loader_path -install_name @rpath/libWhooshUpdater.dylib \
  native/updater.m -o "$APP/Contents/Frameworks/libWhooshUpdater.dylib"
cp "$SPARKLE_DIR/LICENSE" "$APP/Contents/Resources/Sparkle-LICENSE.txt"
.build-venv312/bin/python scripts/collect-licenses.py "$APP/Contents/Resources/licenses"
codesign --force --deep --sign - "$APP"
echo "Application created: $DESKTOP_DIR/$APP"
