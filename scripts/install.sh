#!/usr/bin/env bash
set -euo pipefail

# ─────────────────────────────────────────────────────────────
# Claw Race (see-agent-corp) installer
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/Xiamu-ssr/see-agent/main/scripts/install.sh | bash
#   curl -fsSL ... | bash -s -- v0.3.0          # install specific version
#   curl -fsSL ... | bash -s -- --help
# ─────────────────────────────────────────────────────────────

REPO="Xiamu-ssr/see-agent"
BINARY_NAME="see-agent-corp"
INSTALL_DIR="${SAC_HOME:-$HOME/.see-agent-corp}/bin"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
CYAN='\033[0;36m'
NC='\033[0m'

info()  { echo -e "${CYAN}[info]${NC}  $*" >&2; }
ok()    { echo -e "${GREEN}[ok]${NC}    $*" >&2; }
warn()  { echo -e "${YELLOW}[warn]${NC}  $*" >&2; }
error() { echo -e "${RED}[error]${NC} $*" >&2; exit 1; }

# ── Install Safehouse (macOS sandbox) ─────────────────────────
install_safehouse() {
    local os
    os="$(uname -s)"

    if [ "$os" != "Darwin" ]; then
        info "skipping Safehouse (macOS only, Linux sandbox TBD)"
        return
    fi

    if command -v safehouse &>/dev/null; then
        ok "Safehouse already installed: $(safehouse --version 2>/dev/null || echo 'unknown version')"
        return
    fi

    info "installing Agent Safehouse (macOS sandbox)..."

    if command -v brew &>/dev/null; then
        brew install eugene1g/safehouse/agent-safehouse
        ok "Safehouse installed via Homebrew"
    else
        warn "Homebrew not found. Install Safehouse manually:"
        echo "  brew install eugene1g/safehouse/agent-safehouse"
        echo "  or: https://github.com/eugene1g/agent-safehouse"
    fi
}

usage() {
    cat <<EOF
Claw Race Installer 🦞

Usage:
  install.sh [VERSION]
  install.sh --local [--no-path]

Arguments:
  VERSION   Version tag to install (e.g. v0.3.0). Default: latest release.

Options:
  --help    Show this help message.
  --local   Install from extracted local bundle instead of downloading.
  --no-path Skip writing PATH config to shell rc files.

Examples:
  bash install.sh              # install latest
  bash install.sh v0.3.0       # install specific version
  bash install.sh --local      # install from local extracted tar.gz

Environment:
  SAC_HOME  Override workspace directory (default: ~/.see-agent-corp)
EOF
    exit 0
}

# ── Parse args ────────────────────────────────────────────────
VERSION=""
MODE_LOCAL=false
AUTO_PATH=true
for arg in "$@"; do
    case "$arg" in
        --help|-h) usage ;;
        --local)   MODE_LOCAL=true ;;
        --no-path) AUTO_PATH=false ;;
        v*)        VERSION="$arg" ;;
        *)         error "unknown argument: $arg" ;;
    esac
done

# ── Detect platform ──────────────────────────────────────────
detect_platform() {
    local os arch target

    os="$(uname -s)"
    arch="$(uname -m)"

    case "$os" in
        Darwin) os="apple-darwin" ;;
        Linux)  os="unknown-linux-gnu" ;;
        *)      error "unsupported OS: $os (only macOS and Linux are supported)" ;;
    esac

    case "$arch" in
        x86_64|amd64)   arch="x86_64" ;;
        arm64|aarch64)   arch="aarch64" ;;
        *)               error "unsupported architecture: $arch" ;;
    esac

    target="${arch}-${os}"
    echo "$target"
}

# ── Resolve version ──────────────────────────────────────────
resolve_version() {
    if [ "$MODE_LOCAL" = true ]; then
        echo "local-bundle"
        return
    fi

    if [ -n "$VERSION" ]; then
        echo "$VERSION"
        return
    fi

    info "fetching latest release tag..."
    local tag
    tag=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
        | python3 -c "import sys,json; print(json.load(sys.stdin)['tag_name'])" 2>/dev/null)
    
    # fallback if python3 not available
    if [ -z "$tag" ]; then
        tag=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
            | grep -o '"tag_name"[[:space:]]*:[[:space:]]*"[^"]*"' \
            | head -1 \
            | grep -o 'v[^"]*')
    fi

    if [ -z "$tag" ]; then
        error "failed to fetch latest release. Check https://github.com/${REPO}/releases"
    fi

    echo "$tag"
}

find_local_binary() {
    local target script_dir candidate
    target="$1"
    script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

    candidate="${script_dir}/${BINARY_NAME}-${target}"
    if [ -f "$candidate" ]; then
        echo "$candidate"
        return 0
    fi

    candidate="${script_dir}/${BINARY_NAME}"
    if [ -f "$candidate" ]; then
        echo "$candidate"
        return 0
    fi

    return 1
}

install_binary() {
    local binary_path version
    binary_path="$1"
    version="$2"

    mkdir -p "$INSTALL_DIR"
    cp "$binary_path" "${INSTALL_DIR}/${BINARY_NAME}"
    chmod +x "${INSTALL_DIR}/${BINARY_NAME}"

    ok "installed ${BINARY_NAME} ${version} to ${INSTALL_DIR}/${BINARY_NAME}"
}

ensure_path() {
    if [ "$AUTO_PATH" = false ]; then
        return
    fi

    if echo "$PATH" | tr ':' '\n' | grep -q "^${INSTALL_DIR}$"; then
        return
    fi

    local shell_name rc_file path_line
    shell_name="$(basename "${SHELL:-/bin/zsh}")"

    case "$shell_name" in
        zsh)  rc_file="${HOME}/.zshrc" ;;
        bash) rc_file="${HOME}/.bashrc" ;;
        fish)
            warn "fish shell detected; run: set -Ux fish_user_paths ${INSTALL_DIR} \$fish_user_paths"
            return
            ;;
        *)
            warn "unsupported shell for auto PATH update: ${shell_name}"
            warn "add manually: export PATH=\"${INSTALL_DIR}:\$PATH\""
            return
            ;;
    esac

    path_line="export PATH=\"${INSTALL_DIR}:\$PATH\""
    touch "$rc_file"
    if grep -Fqx "$path_line" "$rc_file"; then
        info "PATH entry already exists in ${rc_file}"
    else
        echo "$path_line" >> "$rc_file"
        ok "added ${INSTALL_DIR} to ${rc_file}"
    fi
    info "reload shell config: source ${rc_file}"
}

# ── Download & install ───────────────────────────────────────
install() {
    local target version url tmp_dir archive_name binary_path

    target="$(detect_platform)"
    version="$(resolve_version)"

    info "platform:  $target"
    info "version:   $version"

    if [ "$MODE_LOCAL" = true ]; then
        if [ -n "$VERSION" ]; then
            warn "version argument is ignored in --local mode"
        fi
        binary_path="$(find_local_binary "$target" || true)"
        if [ -z "$binary_path" ]; then
            error "local binary not found. Extract release tar.gz and run: bash install.sh --local"
        fi
    else
        archive_name="${BINARY_NAME}-${target}.tar.gz"
        url="https://github.com/${REPO}/releases/download/${version}/${archive_name}"
        info "download:  $url"

        tmp_dir="$(mktemp -d)"
        trap 'rm -rf "$tmp_dir"' EXIT

        info "downloading..."
        local downloaded=false

        if ! $downloaded && command -v curl &>/dev/null; then
            info "trying curl..."
            if curl -fSL -o "${tmp_dir}/${archive_name}" "$url" 2>/dev/null; then
                downloaded=true
            fi
        fi

        if ! $downloaded && command -v wget &>/dev/null; then
            info "trying wget..."
            if wget -q -O "${tmp_dir}/${archive_name}" "$url" 2>/dev/null; then
                downloaded=true
            fi
        fi

        if ! $downloaded && command -v python3 &>/dev/null; then
            info "trying python3..."
            if python3 -c "
import urllib.request, sys
urllib.request.urlretrieve('$url', '${tmp_dir}/${archive_name}')
print('ok')
" 2>/dev/null | grep -q ok; then
                downloaded=true
            fi
        fi

        if ! $downloaded; then
            echo ""
            warn "automatic download failed (security software may be blocking it)"
            echo ""
            echo "  Manual install:"
            echo "  1. Download from: $url"
            echo "  2. Extract:       tar xzf ${archive_name}"
            echo "  3. Run local:     bash install.sh --local"
            echo ""
            exit 1
        fi

        info "extracting..."
        tar xzf "${tmp_dir}/${archive_name}" -C "$tmp_dir"

        binary_path="${tmp_dir}/${BINARY_NAME}-${target}"
        if [ ! -f "$binary_path" ]; then
            binary_path="${tmp_dir}/${BINARY_NAME}"
        fi
        if [ ! -f "$binary_path" ]; then
            error "binary not found in archive"
        fi
    fi

    install_binary "$binary_path" "$version"

    install_safehouse

    ensure_path

    echo ""
    ok "🦞 Claw Race is ready!"
    echo ""
    echo "  Start:    ${BINARY_NAME} start --port 28789"
    echo "  Status:   ${BINARY_NAME} status"
    echo "  Open:     http://localhost:28789"
    echo ""
}

install
