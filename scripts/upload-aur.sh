#!/bin/bash
# AUR Upload Script for GSET
# Usage: ./scripts/upload-aur.sh

set -e

REPO_DIR="$(cd "$(dirname "$0")/.." && pwd)"
AUR_DIR="$REPO_DIR/packages/aur"
VERSION=${1:-"2.0.2"}

echo "=== GSET AUR Upload Script ==="
echo "Version: $VERSION"
echo

# Check prerequisites
if ! command -v git &> /dev/null; then
    echo "ERROR: git is required"
    exit 1
fi

# Check if we're in a git repository
cd "$REPO_DIR"
if ! git rev-parse --git-dir > /dev/null 2>&1; then
    echo "ERROR: Not in a git repository"
    exit 1
fi

# Build binary
echo "[1/5] Building binary..."
CGO_ENABLED=0 go build -ldflags="-s -w -X main.Version=$VERSION" -o gset .

# Create distribution directory
echo "[2/5] Preparing distribution..."
mkdir -p /tmp/gset-release
cp gset /tmp/gset-release/
cp gset.conf /tmp/gset-release/
cp LICENSE /tmp/gset-release/

# Create tarball
echo "[3/5] Creating tarball..."
cd /tmp
tar -czf gset-${VERSION}.tar.gz gset-release/
cd "$REPO_DIR"

# Update PKGBUILD version
echo "[4/5] Updating PKGBUILD..."
sed -i "s/pkgver=.*/pkgver=$VERSION/" "$AUR_DIR/PKGBUILD"
sed -i "s/pkgrel=.*/pkgrel=1/" "$AUR_DIR/PKGBUILD"

# Calculate checksums
cd /tmp
sha256sum gset-${VERSION}.tar.gz
cd "$REPO_DIR"

echo "[5/5] Next steps:"
echo ""
echo "1. Create a GitHub release with the tarball:"
echo "   - Upload /tmp/gset-${VERSION}.tar.gz"
echo "   - Tag: v${VERSION}"
echo ""
echo "2. Upload to AUR:"
echo "   cd $AUR_DIR"
echo "   git add ."
echo "   git commit -m 'Update to v${VERSION}'"
echo "   git push origin main"
echo ""
echo "3. Or use aurutils:"
echo "   cd $AUR_DIR"
echo "   aurutils sync gset-git"
echo ""
echo "Tarball ready at: /tmp/gset-${VERSION}.tar.gz"