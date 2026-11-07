#!/bin/bash
set -euo pipefail
DESKTOP="$(cd "$(dirname "$0")/.." && pwd)"
TEST_PYTHON="${WHOOSH_PYTHON:-python3}"
SPARKLE_DIR="$("$TEST_PYTHON" "$DESKTOP/scripts/fetch-sparkle.py")"
mkdir -p "$DESKTOP/target/updater-tests"
xcrun clang -fobjc-arc -fblocks -mmacosx-version-min=14.0 \
  -Wall -Wextra -Werror -Wno-unused-parameter -Wno-nonnull \
  -F "$SPARKLE_DIR" -framework Cocoa -framework Sparkle \
  -Wl,-rpath,"$SPARKLE_DIR" "$DESKTOP/tests/updater_native.m" \
  -o "$DESKTOP/target/updater-tests/lifecycle"
"$DESKTOP/target/updater-tests/lifecycle"
