#!/usr/bin/env bash
# see-agent-corp.sh — local development wrapper
#
# Usage:
#   ./see-agent-corp.sh start [--port PORT]   Auto-build + start daemon
#   ./see-agent-corp.sh stop                  Kill daemon + free port
#   ./see-agent-corp.sh restart [--port PORT] stop + start
#   ./see-agent-corp.sh <any other command>   Auto-build + forward to binary
#
# Same CLI as the release binary, but auto-recompiles when source changes.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
BIN="$SCRIPT_DIR/target/release/see-agent-corp"
DEFAULT_PORT=28789

# --- helpers ---------------------------------------------------------------

needs_build() {
    [ ! -f "$BIN" ] && return 0
    # Any Rust/TOML/HTML source newer than the binary?
    local newest
    newest=$(find "$SCRIPT_DIR/see-agent-corp" "$SCRIPT_DIR/see-agent-corp-app" \
                  "$SCRIPT_DIR/see-agent-corp-web" \
                  -name '*.rs' -o -name '*.toml' -o -name '*.html' -o -name '*.css' \
            2>/dev/null | xargs stat -f '%m %N' 2>/dev/null | sort -rn | head -1 | awk '{print $2}')
    [ -z "$newest" ] && return 1
    [ "$newest" -nt "$BIN" ]
}

do_build() {
    echo "▸ Building frontend (trunk)..." >&2
    (cd "$SCRIPT_DIR/see-agent-corp-web" && trunk build --release) >&2
    echo "▸ Building binary (cargo)..." >&2
    cargo build -p see-agent-corp-app --release --manifest-path "$SCRIPT_DIR/Cargo.toml" >&2
    echo "✓ Build complete." >&2
}

# Resolve the port the daemon is actually using.
# Priority: --port arg > DEFAULT_PORT
resolve_port() {
    local port="$DEFAULT_PORT"
    local args=("$@")
    for ((i=0; i<${#args[@]}; i++)); do
        if [[ "${args[$i]}" == "--port" ]] && (( i+1 < ${#args[@]} )); then
            port="${args[$((i+1))]}"
            break
        fi
    done
    echo "$port"
}

# Find PID file path (same logic as the binary)
pid_file() {
    local ws="${HOME}/.see-agent-corp"
    echo "${ws}/server.pid"
}

kill_by_pid_file() {
    local pf
    pf="$(pid_file)"
    [ -f "$pf" ] || return 1
    local pid
    pid=$(cat "$pf" 2>/dev/null | tr -d '[:space:]')
    [ -z "$pid" ] && return 1
    if kill -0 "$pid" 2>/dev/null; then
        echo "▸ Stopping server (PID $pid)..." >&2
        kill "$pid" 2>/dev/null || true
        # Wait up to 5s for graceful shutdown
        for _ in $(seq 1 50); do
            kill -0 "$pid" 2>/dev/null || break
            sleep 0.1
        done
        # Force kill if still alive
        if kill -0 "$pid" 2>/dev/null; then
            echo "▸ Force killing (PID $pid)..." >&2
            kill -9 "$pid" 2>/dev/null || true
        fi
        echo "✓ Server stopped." >&2
    fi
    rm -f "$pf"
}

kill_by_port() {
    local port="$1"
    local pids
    pids=$(lsof -ti "tcp:$port" 2>/dev/null || true)
    if [ -n "$pids" ]; then
        echo "▸ Killing processes on port $port: $pids" >&2
        echo "$pids" | xargs kill -9 2>/dev/null || true
        sleep 0.3
        echo "✓ Port $port freed." >&2
    fi
}

# --- commands --------------------------------------------------------------

cmd_stop() {
    local port
    port=$(resolve_port "$@")
    # Try PID file first (clean shutdown)
    kill_by_pid_file || true
    # Then ensure the port is actually free
    kill_by_port "$port"
}

cmd_start() {
    # Stop any existing instance first
    local port
    port=$(resolve_port "$@")
    kill_by_pid_file 2>/dev/null || true
    kill_by_port "$port" 2>/dev/null || true

    # Build if needed
    if needs_build; then
        do_build
    fi

    # Start daemon
    echo "▸ Starting server on port $port..." >&2
    "$BIN" start "$@"
}

cmd_restart() {
    cmd_stop "$@"
    sleep 0.3
    cmd_start "$@"
}

# --- main ------------------------------------------------------------------

CMD="${1:-}"

case "$CMD" in
    start)
        shift
        cmd_start "$@"
        ;;
    stop)
        shift
        cmd_stop "$@"
        ;;
    restart)
        shift
        cmd_restart "$@"
        ;;
    *)
        # Any other command: auto-build + forward
        if needs_build; then
            do_build
        fi
        exec "$BIN" "$@"
        ;;
esac
