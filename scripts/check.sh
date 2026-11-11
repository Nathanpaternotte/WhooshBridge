#!/bin/bash
# Run the same offline checks as macOS CI, from any working directory.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
export PATH="$HOME/.cargo/bin:$PATH"
CHECK_PYTHON="${WHOOSH_PYTHON:-$ROOT/venv/bin/python}"
if [ ! -x "$CHECK_PYTHON" ]; then CHECK_PYTHON=python3; fi
cargo fmt --manifest-path desktop/Cargo.toml --check
cargo test --manifest-path desktop/Cargo.toml --locked
cargo clippy --manifest-path desktop/Cargo.toml --locked --all-targets -- -D warnings
"$CHECK_PYTHON" -m unittest discover -s desktop/tests -v
"$CHECK_PYTHON" desktop/tests/check_fit_identity.py
node --test desktop/tests/*.test.cjs

bash desktop/scripts/build-swiftui.sh
bash desktop/scripts/test-swiftui.sh
