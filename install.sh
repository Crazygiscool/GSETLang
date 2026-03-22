#!/bin/bash

# GSET Install Script
# One-liner: curl -fsSL https://raw.githubusercontent.com/Crazygiscool/GSETLang/main/install.sh | bash

set -e

VERSION="2.1.2"
REPO="Crazygiscool/GSETLang"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Detect OS
detect_os() {
    case "$(uname -s)" in
        Linux*)     echo "linux" ;;
        Darwin*)    echo "darwin" ;;
        CYGWIN*)   echo "windows" ;;
        MINGW*)    echo "windows" ;;
        *)         echo "unknown" ;;
    esac
}

# Detect architecture
detect_arch() {
    case "$(uname -m)" in
        x86_64)    echo "amd64" ;;
        aarch64)   echo "arm64" ;;
        armv7l)    echo "arm" ;;
        i386)      echo "386" ;;
        *)         echo "amd64" ;;
    esac
}

# Detect download URL based on OS and arch
get_download_url() {
    OS=$1
    ARCH=$2
    BASE="https://github.com/${REPO}/releases/download/v${VERSION}"
    
    if [ "$OS" = "windows" ]; then
        echo "${BASE}/gset-${OS}-${ARCH}.zip"
    else
        echo "${BASE}/gset-${OS}-${ARCH}.tar.gz"
    fi
}

# Install to custom directory
install_to() {
    local INSTALL_DIR="$1"
    local OS=$(detect_os)
    local ARCH=$(detect_arch)
    local URL=$(get_download_url $OS $ARCH)
    local ARCHIVE_NAME="gset-install-temp"
    
    info "Downloading GSET v${VERSION} for ${OS}/${ARCH}..."
    
    # Create temp directory
    TEMP_DIR=$(mktemp -d)
    cd "$TEMP_DIR"
    
    # Download
    if command -v curl &> /dev/null; then
        curl -fsSL "$URL" -o "${ARCHIVE_NAME}" || { error "Download failed"; cd /; rm -rf "$TEMP_DIR"; exit 1; }
    elif command -v wget &> /dev/null; then
        wget -q "$URL" -O "${ARCHIVE_NAME}" || { error "Download failed"; cd /; rm -rf "$TEMP_DIR"; exit 1; }
    else
        error "curl or wget is required"
        cd /; rm -rf "$TEMP_DIR"
        exit 1
    fi
    
    # Extract
    info "Installing to ${INSTALL_DIR}..."
    
    if [ "$OS" = "windows" ]; then
        unzip -o "${ARCHIVE_NAME}" -d "$INSTALL_DIR" 2>/dev/null || tar -xzf "${ARCHIVE_NAME}" -C "$INSTALL_DIR" 2>/dev/null
    else
        tar -xzf "${ARCHIVE_NAME}" -C "$INSTALL_DIR"
    fi
    
    # Make executable
    chmod +x "${INSTALL_DIR}/gset" 2>/dev/null || chmod +x "${INSTALL_DIR}/gset.exe" 2>/dev/null
    
    # Cleanup
    cd /
    rm -rf "$TEMP_DIR"
    
    info "Installed successfully!"
}

# Add to PATH permanently
add_to_path() {
    local INSTALL_DIR="$1"
    local SHELL_RC=""
    
    # Detect shell
    if [ -n "$BASH_VERSION" ]; then
        SHELL_RC="$HOME/.bashrc"
    elif [ -n "$ZSH_VERSION" ]; then
        SHELL_RC="$HOME/.zshrc"
    fi
    
    # Check if already in PATH
    if [ -d "${INSTALL_DIR}" ] && [[ ":$PATH:" == *":${INSTALL_DIR}:"* ]]; then
        info "GSET is already in PATH"
        return 0
    fi
    
    # Add to shell config
    if [ -n "$SHELL_RC" ]; then
        if ! grep -q "GSET" "$SHELL_RC" 2>/dev/null; then
            echo "" >> "$SHELL_RC"
            echo "# GSET" >> "$SHELL_RC"
            echo "export PATH=\"\${HOME}/.local/bin:\${PATH}\"" >> "$SHELL_RC"
            info "Added ${INSTALL_DIR} to PATH in ${SHELL_RC}"
            info "Restart your shell or run: source ${SHELL_RC}"
        fi
    fi
}

# Main installation
main() {
    echo ""
    echo "============================================"
    echo "  GSET v${VERSION} Installer"
    echo "  Generic Syntax Extension Tool"
    echo "============================================"
    echo ""
    
    local INSTALL_DIR="${HOME}/.local/bin"
    
    # Create install directory
    mkdir -p "$INSTALL_DIR"
    
    # Install
    install_to "$INSTALL_DIR"
    
    # Check if in PATH
    if [[ ":$PATH:" != *":${INSTALL_DIR}:"* ]]; then
        warn "${INSTALL_DIR} is not in your PATH"
        echo ""
        echo "Add this to your shell config (~/.bashrc, ~/.zshrc, etc):"
        echo "  export PATH=\"\${HOME}/.local/bin:\${PATH}\""
        echo ""
        echo "Then restart your shell or run: source ~/.bashrc"
    fi
    
    echo ""
    info "Run 'gset --help' to get started!"
    echo ""
}

# Show help
show_help() {
    echo "GSET Installer v${VERSION}"
    echo ""
    echo "Usage: ./install.sh [OPTIONS]"
    echo ""
    echo "Options:"
    echo "  --dir DIR      Install to custom directory (default: ~/.local/bin)"
    echo "  --help        Show this help"
    echo ""
    echo "One-liner install:"
    echo "  curl -fsSL https://raw.githubusercontent.com/${REPO}/main/install.sh | bash"
}

# Parse arguments
INSTALL_DIR="${HOME}/.local/bin"

while [[ $# -gt 0 ]]; do
    case $1 in
        --dir)
            INSTALL_DIR="$2"
            shift 2
            ;;
        --help)
            show_help
            exit 0
            ;;
        *)
            error "Unknown option: $1"
            show_help
            exit 1
            ;;
    esac
done

main
