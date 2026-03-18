#!/usr/bin/env bash
# agentcorp.sh — local development wrapper
# Usage: ./agentcorp.sh <command> [args...]
# Same interface as the release binary, but builds from source.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
BIN="$SCRIPT_DIR/target/release/agentcorp"

# Build if binary missing or source is newer
needs_build() {
    [ ! -f "$BIN" ] && return 0
    # Check if any Rust source is newer than the binary
    local newest_src
    newest_src=$(find "$SCRIPT_DIR/agentcorp" "$SCRIPT_DIR/agentcorp-app" -name '*.rs' -newer "$BIN" 2>/dev/null | head -1)
    [ -n "$newest_src" ]
}

if needs_build; then
    echo "Building agentcorp..." >&2
    cargo build -p agentcorp-app --release --manifest-path "$SCRIPT_DIR/Cargo.toml" --quiet
    echo "Build complete." >&2
fi

exec "$BIN" "$@"
