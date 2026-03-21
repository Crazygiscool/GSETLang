#!/bin/bash
# AUR Upload Script for GSET
# Version: 2.0.2
# License: CC BY-NC 4.0
# Usage: ./scripts/upload-aur.sh [version]

set -e

REPO_DIR="$(cd "$(dirname "$0")/.." && pwd)"
AUR_DIR="$REPO_DIR/packages/aur"
GITHUB_REPO="Crazygiscool/GSETLang"
VERSION=${1:-"2.0.2"}

echo "=== GSET AUR Upload Script ==="
echo "Version: $VERSION"
echo "GitHub: https://github.com/$GITHUB_REPO"
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
sha256sum gset-${VERSION}.tar.gz
cd "$REPO_DIR"

# Update PKGBUILD version
echo "[4/5] Updating PKGBUILD..."
sed -i "s/pkgver=.*/pkgver=$VERSION/" "$AUR_DIR/PKGBUILD"
sed -i "s/pkgrel=.*/pkgrel=1/" "$AUR_DIR/PKGBUILD"

echo "[5/5] Next steps:"
echo ""
echo "1. Create GitHub Release:"
echo "   - Go to: https://github.com/$GITHUB_REPO/releases/new?tag=v$VERSION"
echo "   - Upload: /tmp/gset-${VERSION}.tar.gz"
echo "   - Title: GSET v$VERSION"
echo ""
echo "2. Push to AUR (requires AUR account):"
echo "   cd $AUR_DIR"
echo "   git init  (if not already)"
echo "   git add ."
echo "   git commit -m 'Update to v$VERSION'"
echo "   git remote add aur ssh://aur@aur.archlinux.org/gset-git.git"
echo "   git push aur master"
echo ""
echo "   Or use aurutils:"
echo "   aurutils sync gset-git"
echo ""
echo "Tarball ready at: /tmp/gset-${VERSION}.tar.gz"