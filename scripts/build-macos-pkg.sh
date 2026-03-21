#!/bin/bash
# Build macOS PKG Installer for GSET
# Version: 2.0.2
# License: CC BY-NC 4.0
#
# This script creates a macOS PKG installer using the `packages` tool
# or falls back to a simple DMG if packages is not available.
#
# Requirements:
# - macOS (to build PKG)
# - OR use Packages app: https://github.com/stephane-lieumont-company/Packages
#
# Usage:
#   ./build-macos-pkg.sh
#
# Output:
#   dist/GSET-2.0.2.pkg

set -e
VERSION="2.0.2"
BUILD_DIR="./dist"
PKG_NAME="GSET-${VERSION}"

echo "=== GSET macOS PKG Builder ==="
echo "Version: ${VERSION}"
echo

# Create build directory
mkdir -p "$BUILD_DIR"

# Check if we're on macOS
if [[ "$(uname)" != "Darwin" ]]; then
    echo "WARNING: Not on macOS - cannot build PKG natively"
    echo "Options:"
    echo "1. Run this script on a macOS machine"
    echo "2. Use the Packages app (https://github.com/stephane-lieumont-company/Packages)"
    echo "   Open packages/macos/GSET.pkgproj in Packages and build"
    echo "3. Create a DMG instead (see below)"
    
    # Create a simple DMG as fallback
    echo ""
    echo "Creating DMG instead..."
    
    # Build the binary for darwin
    echo "[1/3] Building binary for macOS..."
    GOOS=darwin GOARCH=amd64 CGO_ENABLED=0 go build -ldflags="-s -w -X main.Version=$VERSION" -o "$BUILD_DIR/gset" .
    
    # Create app bundle
    echo "[2/3] Creating app bundle..."
    mkdir -p "$BUILD_DIR/GSET.app/Contents/MacOS"
    cp "$BUILD_DIR/gset" "$BUILD_DIR/GSET.app/Contents/MacOS/GSET"
    
    # Create Info.plist
    cat > "$BUILD_DIR/GSET.app/Contents/Info.plist" << 'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key><string>GSET</string>
    <key>CFBundleIdentifier</key><string>com.gset.lang</string>
    <key>CFBundleVersion</key><string>2.0.2</string>
    <key>CFBundleExecutable</key><string>GSET</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleSignature</key><string>????</string>
    <key>CFBundleIconFile</key><string></string>
    <key>CFBundleInfoDictionaryVersion</key><string>6.0</string    <key>LSMinimumSystemVersion</key><string>10.13</string>
    <key>NSHighResolutionCapable</key><true/>
    <key>LSApplicationCategoryType</key><string>public.app-category.developer-tools</string>
</dict>
</plist>
EOF

    # Create DMG
    if command -v hdiutil &> /dev/null; then
        echo "[3/3] Creating DMG..."
        hdiutil create -volname "GSET-${VERSION}" -srcfolder "$BUILD_DIR/GSET.app" -ov "$BUILD_DIR/GSET-${VERSION}.dmg"
    else
        echo "[3/3] Creating ZIP archive (hdiutil not available on Linux)..."
        cd "$BUILD_DIR"
        zip -r "GSET-${VERSION}-macOS.zip" GSET.app
        cd "$REPO_DIR"
    fi
    
    echo ""
    echo "=== Build Complete ==="
    echo "Output: $BUILD_DIR/GSET-${VERSION}.dmg"
    echo ""
    echo "To install: Mount the DMG and drag GSET.app to Applications"
    exit 0
fi

# We're on macOS - try to build PKG
echo "[1/3] Building binary for macOS..."
GOOS=darwin GOARCH=amd64 CGO_ENABLED=0 go build -ldflags="-s -w -X main.Version=$VERSION" -o "$BUILD_DIR/gset" .

echo "[2/3] Checking for Packages tool..."
if command -v packages &> /dev/null; then
    echo "Found Packages - building PKG..."
    packages build -o "$BUILD_DIR/$PKG_NAME.pkg" packages/macos/GSET.pkgproj
else
    echo "Packages tool not found - checking for buildpkg..."
    
    # Try using pkgbuild as fallback
    echo "Creating PKG using pkgbuild..."
    
    # Create temp dir structure
    PKG_TMP="/tmp/gset-pkg"
    rm -rf "$PKG_TMP"
    mkdir -p "$PKG_TMP/usr/local/bin"
    mkdir -p "$PKG_TMP/etc"
    mkdir -p "$PKG_TMP/Library/Application Support/GSET"
    
    # Copy files
    cp "$BUILD_DIR/gset" "$PKG_TMP/usr/local/bin/gset"
    cp "gset.conf" "$PKG_TMP/etc/gset.conf"
    cp "LICENSE" "$PKG_TMP/Library/Application Support/GSET/LICENSE"
    
    # Set permissions
    chmod 755 "$PKG_TMP/usr/local/bin/gset"
    
    # Build PKG
    pkgbuild --root "$PKG_TMP" \
             --identifier "com.gset.lang" \
             --version "$VERSION" \
             --ownership recommended \
             "$BUILD_DIR/$PKG_NAME.pkg"
fi

echo "[3/3] Signing package (optional)..."
if command -v codesign &> /dev/null; then
    echo "To sign the package, run:"
    echo "  codesign -s 'Your Developer ID' $BUILD_DIR/$PKG_NAME.pkg"
fi

echo ""
echo "=== Build Complete ==="
echo "Output: $BUILD_DIR/$PKG_NAME.pkg"
echo ""
echo "To install: Double-click the PKG file"