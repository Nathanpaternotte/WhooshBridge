#!/bin/bash
set -euo pipefail
DESKTOP_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$DESKTOP_DIR"
mkdir -p target/swift-module-cache
xcrun swiftc -parse-as-library -swift-version 5 \
  -target "$(uname -m)-apple-macosx14.0" -module-cache-path target/swift-module-cache \
  native/swift/BridgeModel.swift native/swift/NativeUpdater.swift tests/swiftui_model.swift \
  -o target/swiftui-model-tests
target/swiftui-model-tests
