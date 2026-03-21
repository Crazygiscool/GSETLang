#!/bin/bash
# APT Repository Upload Script
# Usage: ./scripts/upload-apt.sh [version]

set -e

VERSION=${1:-"2.0.2"}
REPO_DIR="$(cd "$(dirname "$0")/.." && pwd)"
DEBIAN_DIR="$REPO_DIR/packages/debian"

echo "=== GSET APT Upload Script ==="
echo "Version: $VERSION"
echo

# Check prerequisites
for cmd in dpkg-deb; do
    if ! command -v $cmd &> /dev/null; then
        echo "WARNING: $cmd not found"
    fi
done

# Build binary
echo "[1/6] Building binary..."
cd "$REPO_DIR"
CGO_ENABLED=0 go build -ldflags="-s -w -X main.Version=$VERSION" -o gset .

# Create package directory structure
echo "[2/6] Creating package structure..."
PKG_DIR="/tmp/gset-${VERSION}-amd64"
mkdir -p "$PKG_DIR/usr/bin"
mkdir -p "$PKG_DIR/etc"
mkdir -p "$PKG_DIR/usr/share/doc/gset"
mkdir -p "$PKG_DIR/usr/share/lintian/overrides"
mkdir -p "$PKG_DIR/DEBIAN"

# Copy files
cp gset "$PKG_DIR/usr/bin/gset"
cp gset.conf "$PKG_DIR/etc/gset.conf"
cp LICENSE "$PKG_DIR/usr/share/doc/gset/copyright"
chmod 755 "$PKG_DIR/usr/bin/gset"

# Create control file
cat > "$PKG_DIR/DEBIAN/control" <<EOF
Package: gset
Version: ${VERSION}
Architecture: amd64
Maintainer: GSET Team <gset@example.com>
Description: GSET - Generic Syntax Extension Tool
 GSET allows you to write code in any language syntax and compile it 
 to any other language using configurable keyword mappings.
 .
 Features:
  - Write in Python, Java, JavaScript, Go, or custom syntax
  - Configurable via gset.conf
  - Supports multiple target compilers
Homepage: https://github.com/gset-lang/gset
EOF

# Create changelog
cat > "$PKG_DIR/usr/share/doc/gset/changelog" <<EOF
gset (${VERSION}) stable; urgency=medium

  * Initial release

 -- GSET Team <gset@example.com>  $(date -R)
EOF
gzip -9 "$PKG_DIR/usr/share/doc/gset/changelog"

# Create md5sums
cd "$PKG_DIR"
find . -type f ! -path './DEBIAN/*' -exec md5sum {} \; > DEBIAN/md5sums
cd "$REPO_DIR"

# Create postinst (optional)
cat > "$PKG_DIR/DEBIAN/postinst" <<EOF
#!/bin/sh
set -e
case "$1" in
    configure)
        echo "GSET installed! Config at /etc/gset.conf"
        ;;
esac
exit 0
EOF
chmod 755 "$PKG_DIR/DEBIAN/postinst"

# Build .deb package
echo "[3/6] Building .deb package..."
dpkg-deb --build "$PKG_DIR" /tmp/gset_${VERSION}_amd64.deb

echo "[4/6] Package created: /tmp/gset_${VERSION}_amd64.deb"
echo "[5/6] Signing package (requires GPG key)..."
echo "   gpg --armor --detach-sign /tmp/gset_${VERSION}_amd64.deb"

echo "[6/6] Next steps to upload to Launchpad/Debian:"
echo ""
echo "Option 1: Upload to personal PPA on Launchpad"
echo "   sudo apt-get install dput"
echo "   dput ppa:yourusername/ppa /tmp/gset_${VERSION}_amd64.deb"
echo ""
echo "Option 2: Create your own apt repository"
echo "   sudo apt-get install reprepro"
echo "   mkdir -p /var/www/debian"
echo "   cp gset_${VERSION}_amd64.deb /var/www/debian/"
echo "   cd /var/www/debian"
echo "   reprepro -b . includedeb stable gset_${VERSION}_amd64.deb"
echo ""
echo "Option 3: Upload directly to Debian (requires maintainer)"
echo "   Visit: https://packages.debian.org/upload"
echo ""
echo "Package ready at: /tmp/gset_${VERSION}_amd64.deb"