#!/usr/bin/env bash
set -euo pipefail

# see-agent installer
# Usage: curl -fsSL https://raw.githubusercontent.com/OWNER/see-agent/main/scripts/install.sh | sh

REPO="${SEE_REPO:-OWNER/see-agent}"
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
    URL="https://github.com/${REPO}/releases/latest/download/see-${TARGET}.tar.gz"
else
    URL="https://github.com/${REPO}/releases/download/${VERSION}/see-${TARGET}.tar.gz"
fi

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

echo "Downloading see-agent for ${TARGET}..."
curl -fsSL "$URL" | tar xz -C "$TMP"
chmod +x "$TMP/see-${TARGET}"

echo "Installing to ${INSTALL_DIR}/see (may require sudo)..."
if [ -w "$INSTALL_DIR" ]; then
    mv "$TMP/see-${TARGET}" "${INSTALL_DIR}/see"
else
    sudo mv "$TMP/see-${TARGET}" "${INSTALL_DIR}/see"
fi

echo "Installed see-agent to ${INSTALL_DIR}/see"
see --version
