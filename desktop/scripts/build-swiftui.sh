#!/bin/bash
# Compile the native UI with the macOS SDK; no package downloads required.
set -euo pipefail
DESKTOP_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$DESKTOP_DIR"
mkdir -p target/swift-module-cache
FLAGS=()
if [ "${WHOOSH_SWIFT_DEBUG:-0}" = 1 ]; then FLAGS=(-D DEBUG -Onone); else FLAGS=(-O); fi
xcrun swiftc -parse-as-library -swift-version 5 "${FLAGS[@]}" \
  -target "$(uname -m)-apple-macosx14.0" \
  -module-cache-path target/swift-module-cache \
  native/swift/*.swift -o target/WhooshBridge-SwiftUI
