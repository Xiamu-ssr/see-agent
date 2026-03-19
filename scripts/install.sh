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

Arguments:
  VERSION   Version tag to install (e.g. v0.3.0). Default: latest release.

Options:
  --help    Show this help message.

Examples:
  bash install.sh              # install latest
  bash install.sh v0.3.0       # install specific version

Environment:
  SAC_HOME  Override workspace directory (default: ~/.see-agent-corp)
EOF
    exit 0
}

# ── Parse args ────────────────────────────────────────────────
VERSION=""
for arg in "$@"; do
    case "$arg" in
        --help|-h) usage ;;
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

# ── Download & install ───────────────────────────────────────
install() {
    local target version url tmp_dir archive_name

    target="$(detect_platform)"
    version="$(resolve_version)"
    archive_name="${BINARY_NAME}-${target}.tar.gz"
    url="https://github.com/${REPO}/releases/download/${version}/${archive_name}"

    info "platform:  $target"
    info "version:   $version"
    info "download:  $url"

    # Create temp dir
    tmp_dir="$(mktemp -d)"
    trap 'rm -rf "$tmp_dir"' EXIT

    # Download
    info "downloading..."
    if ! curl -fSL --progress-bar --connect-timeout 10 --max-time 120 -o "${tmp_dir}/${archive_name}" "$url"; then
        error "download failed. Check that ${version} exists at https://github.com/${REPO}/releases"
    fi

    # Extract
    info "extracting..."
    tar xzf "${tmp_dir}/${archive_name}" -C "$tmp_dir"

    # Find binary
    local binary_path="${tmp_dir}/${BINARY_NAME}-${target}"
    if [ ! -f "$binary_path" ]; then
        binary_path="${tmp_dir}/${BINARY_NAME}"
    fi
    if [ ! -f "$binary_path" ]; then
        error "binary not found in archive"
    fi

    # Install
    mkdir -p "$INSTALL_DIR"
    cp "$binary_path" "${INSTALL_DIR}/${BINARY_NAME}"
    chmod +x "${INSTALL_DIR}/${BINARY_NAME}"

    ok "installed ${BINARY_NAME} ${version} to ${INSTALL_DIR}/${BINARY_NAME}"

    # ── Install Safehouse (macOS only) ────────────────────────
    install_safehouse

    # ── PATH check ────────────────────────────────────────────
    if ! echo "$PATH" | tr ':' '\n' | grep -q "^${INSTALL_DIR}$"; then
        warn "${INSTALL_DIR} is not in your PATH"
        echo ""
        echo "Add it to your shell profile:"
        echo ""

        local shell_name
        shell_name="$(basename "${SHELL:-/bin/zsh}")"

        case "$shell_name" in
            zsh)
                echo "  echo 'export PATH=\"${INSTALL_DIR}:\$PATH\"' >> ~/.zshrc"
                echo "  source ~/.zshrc"
                ;;
            bash)
                echo "  echo 'export PATH=\"${INSTALL_DIR}:\$PATH\"' >> ~/.bashrc"
                echo "  source ~/.bashrc"
                ;;
            fish)
                echo "  set -Ux fish_user_paths ${INSTALL_DIR} \$fish_user_paths"
                ;;
            *)
                echo "  export PATH=\"${INSTALL_DIR}:\$PATH\""
                ;;
        esac
        echo ""
    fi

    # ── Verify ────────────────────────────────────────────────
    echo ""
    ok "🦞 Claw Race is ready!"
    echo ""
    echo "  Start:    ${BINARY_NAME} start --port 28789"
    echo "  Status:   ${BINARY_NAME} status"
    echo "  Open:     http://localhost:28789"
    echo ""
}

install
