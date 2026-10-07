#!/bin/bash

# GSET Build Script (Rust)
# Cross-compiles GSET for multiple platforms

set -e

VERSION=$(grep 'version' Cargo.toml | head -1 | sed -n 's/.*version = "\(.*\)".*/\1/p')
DIR="dist"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "Building GSET v${VERSION}..."

rm -rf $DIR
mkdir -p $DIR

cargo build --release -p gset-cli
cp target/release/gset $DIR/gset-${VERSION}-linux-amd64 2>/dev/null || cp target/release/gset $DIR/gset-${VERSION}-$(uname -s | tr '[:upper:]' '[:lower:]')-$(uname -m) 2>/dev/null || true

echo "Built Rust binary"
