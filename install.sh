#!/bin/bash

# --- CONFIGURATION ---
# REPLACE THIS with your actual "username/repo"
GITHUB_REPO="zhangjinshui-nerveee/gantt-cli"
BINARY_NAME="gantt-cli"
INSTALL_DIR="$HOME/.local/bin"
# ---------------------

set -e # Exit immediately if a command exits with a non-zero status

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo -e "${YELLOW}Installing ${BINARY_NAME}...${NC}"

# 1. Ensure the install directory exists
mkdir -p "$INSTALL_DIR"

# 2. Determine download URL (Latest Release)
# This fetches the latest release tag from the GitHub API
LATEST_URL="https://github.com/${GITHUB_REPO}/releases/latest/download/${BINARY_NAME}-linux-amd64"

echo -e "Downloading binary to ${INSTALL_DIR}..."

# 3. Download the file using curl or wget
if command -v curl >/dev/null 2>&1; then
    curl -L --fail "$LATEST_URL" -o "$INSTALL_DIR/$BINARY_NAME"
elif command -v wget >/dev/null 2>&1; then
    wget -qO "$INSTALL_DIR/$BINARY_NAME" "$LATEST_URL"
else
    echo -e "${RED}Error: Neither curl nor wget was found. Please install one to continue.${NC}"
    exit 1
fi

# 4. Make it executable
chmod +x "$INSTALL_DIR/$BINARY_NAME"

echo -e "${GREEN}Success! ${BINARY_NAME} has been installed to ${INSTALL_DIR}.${NC}"

# 5. Check PATH
if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
    echo -e "${YELLOW}Warning: ${INSTALL_DIR} is not in your PATH.${NC}"
    echo "Add the following line to your shell configuration file (.bashrc, .zshrc, etc.):"
    echo -e "  export PATH=\"\$HOME/.local/bin:\$PATH\""
else
    echo "You can now run the app by typing: ${BINARY_NAME}"
fi
