#!/usr/bin/env bash
# see-agent-corp.sh — local development wrapper
# Usage: ./see-agent-corp.sh <command> [args...]
# Same interface as the release binary, but builds from source.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
BIN="$SCRIPT_DIR/target/release/see-agent-corp"

# Build if binary missing or source is newer
needs_build() {
    [ ! -f "$BIN" ] && return 0
    # Check if any Rust source is newer than the binary
    local newest_src
    newest_src=$(find "$SCRIPT_DIR/see-agent-corp" "$SCRIPT_DIR/see-agent-corp-app" -name '*.rs' -newer "$BIN" 2>/dev/null | head -1)
    [ -n "$newest_src" ]
}

if needs_build; then
    echo "Building see-agent-corp..." >&2
    cargo build -p see-agent-corp-app --release --manifest-path "$SCRIPT_DIR/Cargo.toml" --quiet
    echo "Build complete." >&2
fi

exec "$BIN" "$@"
