#!/bin/bash
# Build a separate synthetic app. This app never starts the engine or updater.
set -euo pipefail
DESKTOP_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$DESKTOP_DIR"
WHOOSH_SWIFT_DEBUG=1 bash scripts/build-swiftui.sh
APP="target/preview/Whoosh Bridge Preview.app"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp target/WhooshBridge-SwiftUI "$APP/Contents/MacOS/WhooshBridge"
cp assets/WhooshBridge.icns ../THIRD_PARTY_NOTICES.md "$APP/Contents/Resources/"
cat > "$APP/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleName</key><string>Whoosh Bridge Preview</string>
<key>CFBundleIdentifier</key><string>com.mywhooshbridge.preview</string>
<key>CFBundleExecutable</key><string>WhooshBridge</string>
<key>CFBundleIconFile</key><string>WhooshBridge</string>
<key>LSMinimumSystemVersion</key><string>14.0</string>
<key>NSHighResolutionCapable</key><true/>
<key>NSPrincipalClass</key><string>NSApplication</string>
<key>LSEnvironment</key><dict><key>WHOOSH_UI_PREVIEW</key><string>rides</string></dict>
</dict></plist>
PLIST
codesign --force --sign - "$APP"
echo "$DESKTOP_DIR/$APP"
