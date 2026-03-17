#!/usr/bin/env bash
set -euo pipefail

echo "=== see-agent quality gate ==="
echo ""

# 1. Clippy (native targets)
echo "--- Step 1/4: clippy (native) ---"
cargo clippy --all-targets -- -D warnings
echo "    clippy: PASS"
echo ""

# 2. Clippy (WASM target for see-web)
echo "--- Step 2/4: clippy (wasm) ---"
cargo clippy -p see-web --target wasm32-unknown-unknown -- -D warnings
echo "    clippy (wasm): PASS"
echo ""

# 3. Tests
echo "--- Step 3/4: cargo test ---"
cargo test
echo "    tests: PASS"
echo ""

# 4. Build check (ensures binary compiles)
echo "--- Step 4/4: build ---"
cargo build -p see-app --quiet
echo "    build: PASS"
echo ""

echo "=== ALL CHECKS PASSED ==="
echo ""
echo "To run E2E smoke tests (requires running outside sandboxed environments):"
echo "  target/debug/see init"
echo "  target/debug/see status"
echo "  target/debug/see agent create --id test"
echo "  target/debug/see agent list"
echo "  target/debug/see agent delete test"
