#!/usr/bin/env bash
set -euo pipefail

# agentcorp installer
# Usage: curl -fsSL https://raw.githubusercontent.com/OWNER/agentcorp/main/scripts/install.sh | sh

REPO="${AGENTCORP_REPO:-OWNER/agentcorp}"
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
    URL="https://github.com/${REPO}/releases/latest/download/agentcorp-${TARGET}.tar.gz"
else
    URL="https://github.com/${REPO}/releases/download/${VERSION}/agentcorp-${TARGET}.tar.gz"
fi

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

echo "Downloading agentcorp for ${TARGET}..."
curl -fsSL "$URL" | tar xz -C "$TMP"
chmod +x "$TMP/agentcorp-${TARGET}"

echo "Installing to ${INSTALL_DIR}/agentcorp (may require sudo)..."
if [ -w "$INSTALL_DIR" ]; then
    mv "$TMP/agentcorp-${TARGET}" "${INSTALL_DIR}/agentcorp"
else
    sudo mv "$TMP/agentcorp-${TARGET}" "${INSTALL_DIR}/agentcorp"
fi

echo "Installed agentcorp to ${INSTALL_DIR}/agentcorp"
agentcorp --version
