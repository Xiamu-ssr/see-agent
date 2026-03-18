#!/usr/bin/env bash
set -euo pipefail

# see-agent-corp installer
# Usage: curl -fsSL https://raw.githubusercontent.com/OWNER/see-agent-corp/main/scripts/install.sh | sh

REPO="${SAC_REPO:-OWNER/see-agent-corp}"
VERSION="${1:-latest}"
INSTALL_DIR="${INSTALL_DIR:-/usr/local/bin}"

# Detect OS and architecture
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

case "${OS}_${ARCH}" in
    linux_x86_64)   TARGET="x86_64-unknown-linux-gnu" ;;
    linux_aarch64)  TARGET="aarch64-unknown-linux-gnu" ;;
    darwin_arm64)   TARGET="aarch64-apple-darwin" ;;
    darwin_x86_64)  TARGET="x86_64-apple-darwin" ;;
    *) echo "Unsupported platform: ${OS} ${ARCH}"; exit 1 ;;
esac

if [ "$VERSION" = "latest" ]; then
    URL="https://github.com/${REPO}/releases/latest/download/see-agent-corp-${TARGET}.tar.gz"
else
    URL="https://github.com/${REPO}/releases/download/${VERSION}/see-agent-corp-${TARGET}.tar.gz"
fi

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

echo "Downloading see-agent-corp for ${TARGET}..."
curl -fsSL "$URL" | tar xz -C "$TMP"
chmod +x "$TMP/see-agent-corp-${TARGET}"

echo "Installing to ${INSTALL_DIR}/see-agent-corp (may require sudo)..."
if [ -w "$INSTALL_DIR" ]; then
    mv "$TMP/see-agent-corp-${TARGET}" "${INSTALL_DIR}/see-agent-corp"
else
    sudo mv "$TMP/see-agent-corp-${TARGET}" "${INSTALL_DIR}/see-agent-corp"
fi

echo "Installed see-agent-corp to ${INSTALL_DIR}/see-agent-corp"
see-agent-corp --version
