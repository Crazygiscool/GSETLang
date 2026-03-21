#!/bin/bash
# Build script for GSET

set -e

VERSION=${1:-"dev"}
BUILD_DIR=${2:-"./dist"}

echo "Building GSET v$VERSION..."

mkdir -p "$BUILD_DIR"

# Build for current platform
echo "Building for current platform..."
go build -ldflags="-s -w -X main.Version=$VERSION" -o "$BUILD_DIR/gset" .

echo "Build complete: $BUILD_DIR/gset"

# Cross-compile for other platforms
echo "Cross-compiling..."

# Linux
GOOS=linux GOARCH=amd64 go build -ldflags="-s -w -X main.Version=$VERSION" -o "$BUILD_DIR/gset-linux-amd64" .
GOOS=linux GOARCH=arm64 go build -ldflags="-s -w -X main.Version=$VERSION" -o "$BUILD_DIR/gset-linux-arm64" .

# macOS
GOOS=darwin GOARCH=amd64 go build -ldflags="-s -w -X main.Version=$VERSION" -o "$BUILD_DIR/gset-darwin-amd64" .
GOOS=darwin GOARCH=arm64 go build -ldflags="-s -w -X main.Version=$VERSION" -o "$BUILD_DIR/gset-darwin-arm64" .

# Windows
GOOS=windows GOARCH=amd64 go build -ldflags="-s -w -X main.Version=$VERSION" -o "$BUILD_DIR/gset-windows-amd64.exe" .

echo "All builds complete!"
ls -la "$BUILD_DIR"