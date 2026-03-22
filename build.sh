#!/bin/bash

# GSET Build Script
# Cross-compiles GSET for multiple platforms

set -e

VERSION=$(grep 'GSET v' main.go | head -1 | sed 's/.*GSET v//' | sed 's/".*//')
REPO="github.com/Crazygiscool/GSETLang"
DIR="dist"

echo "Building GSET v${VERSION}..."

# Clean and create dist directory
rm -rf $DIR
mkdir -p $DIR

# Build for each platform
PLATFORMS=(
    "linux/amd64"
    "linux/arm64"
    "linux/386"
    "darwin/amd64"
    "darwin/arm64"
    "windows/amd64"
    "windows/386"
)

for PLATFORM in "${PLATFORMS[@]}"; do
    GOOS=${PLATFORM%/*}
    GOARCH=${PLATFORM#*/}
    OUTPUT="$DIR/gset-${GOOS}-${GOARCH}"
    
    if [ "$GOOS" = "windows" ]; then
        OUTPUT="${OUTPUT}.exe"
    fi
    
    echo "Building $GOOS/$GOARCH..."
    GOOS=$GOOS GOARCH=$GOARCH go build -ldflags="-s -w" -o "$OUTPUT" .
    
    # Get file size
    SIZE=$(du -h "$OUTPUT" | cut -f1)
    echo "  -> $OUTPUT ($SIZE)"
done

echo ""
echo "Creating archives..."

# Create archives
cd $DIR

# Linux archives
tar -czf "gset-linux-amd64.tar.gz" "gset-linux-amd64"
tar -czf "gset-linux-arm64.tar.gz" "gset-linux-arm64"
tar -czf "gset-linux-386.tar.gz" "gset-linux-386"

# macOS archives
tar -czf "gset-darwin-amd64.tar.gz" "gset-darwin-amd64"
tar -czf "gset-darwin-arm64.tar.gz" "gset-darwin-arm64"

# Windows archives (zip)
zip "gset-windows-amd64.zip" "gset-windows-amd64.exe"
zip "gset-windows-386.zip" "gset-windows-386.exe"

# Create checksums
sha256sum * > SHA256SUMS

echo ""
echo "Build complete! Files in ./dist/:"
ls -lh

echo ""
echo "Checksums:"
cat SHA256SUMS
