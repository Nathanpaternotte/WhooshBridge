#!/bin/bash
# Package an already-built macOS app for GitHub Releases.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
APP="$ROOT/desktop/target/macos/Whoosh Bridge.app"
if [ ! -d "$APP" ]; then
  echo 'Build the app first: bash desktop/scripts/build-macos.sh' >&2
  exit 1
fi
codesign --verify --deep --strict "$APP"
VERSION=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$APP/Contents/Info.plist")
ARCH=$(lipo -archs "$APP/Contents/MacOS/WhooshBridge")
case "$ARCH" in
  arm64|x86_64) ;;
  *) echo "Unsupported/mixed architecture: $ARCH" >&2; exit 1 ;;
esac
mkdir -p "$ROOT/dist"
ARCHIVE="$ROOT/dist/Whoosh-Bridge-${VERSION}-macos-${ARCH}.zip"
ditto -c -k --sequesterRsrc --keepParent "$APP" "$ARCHIVE"
(cd "$ROOT/dist" && shasum -a 256 "$(basename "$ARCHIVE")" > "$(basename "$ARCHIVE").sha256")
echo "Ready-to-use app: $ARCHIVE"
