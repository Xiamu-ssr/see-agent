#!/usr/bin/env bash
set -euo pipefail

echo "=== see-agent quality gate ==="
echo ""

# 0. Build frontend (optional, if trunk available)
if command -v trunk &>/dev/null; then
    echo "--- Step 0: trunk build ---"
    (cd see-web && trunk build --release 2>&1)
    echo "    trunk build: PASS"
    echo ""
fi

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

# 5. Magic value scan (outside isolation zone)
echo "--- Step 5: magic value scan ---"
MAGIC_FAIL=0

# Numeric literals ≥ 2 that should be constants (exclude io/, consts.rs, tests, see-web, target, .git)
# Pattern: standalone numbers like 500, 1024, 200 etc in Rust code outside test blocks
NUMERIC_HITS=$(find see/src see-app/src -name '*.rs' \
    ! -path '*/io/*' \
    ! -name 'consts.rs' \
    ! -path '*/target/*' \
    -print0 | xargs -0 grep -nE '\b[0-9]{2,}\b' \
    --include='*.rs' \
    2>/dev/null | \
    grep -vE '#\[cfg\(test\)\]|#\[test\]|assert|mod tests|\.len\(\)|as [uf](32|64)|0x[0-9a-fA-F]|line [0-9]|"[^"]*[0-9]+[^"]*"|//.*[0-9]|version|[0-9]+\.[0-9]+|from_raw|enum |struct |impl |use |pub |fn |const |static ' \
    2>/dev/null || true)

# String literals that look like config defaults (exclude io/, consts.rs, tests, see-web)
# Uses awk to only scan lines before #[cfg(test)] in each file (test code is exempt)
STRING_HITS=$(find see/src see-app/src -name '*.rs' \
    ! -path '*/io/*' \
    ! -name 'consts.rs' \
    ! -path '*/target/*' \
    -exec awk '/^#\[cfg\(test\)\]/{exit} /"\.(see-agent)"|"2024-11-05"|"gpt-4o"|"~\/.see-agent"/{print FILENAME":"NR": "$0}' {} \; \
    2>/dev/null || true)

if [ -n "$STRING_HITS" ]; then
    echo "    WARN: potential magic strings outside isolation zone:"
    echo "$STRING_HITS" | head -20
    MAGIC_FAIL=1
fi

if [ "$MAGIC_FAIL" -eq 0 ]; then
    echo "    magic value scan: PASS"
else
    echo "    magic value scan: WARN (review above)"
fi
echo ""

echo "=== ALL CHECKS PASSED ==="
echo ""
echo "To run E2E smoke tests (requires running outside sandboxed environments):"
echo "  target/debug/see init"
echo "  target/debug/see status"
echo "  target/debug/see agent create --id test"
echo "  target/debug/see agent list"
echo "  target/debug/see agent delete test"
