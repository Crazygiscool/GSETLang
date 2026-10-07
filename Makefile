# GSET Makefile

.PHONY: all build test clippy fmt lint release clean install

VERSION := 3.2.1
REPO := github.com/Crazygiscool/GSETLang

all: build

# Build for current platform
build:
	cargo build --release -p gset-cli
	cp target/release/gset gset-$(VERSION) 2>/dev/null || true

debug:
	cargo build -p gset-cli
	cp target/debug/gset gset-$(VERSION)-debug 2>/dev/null || true

# Run tests
test:
	cargo test --workspace

# Run clippy
clippy:
	cargo clippy --workspace --all-targets -- -D warnings

# Format check
fmt:
	cargo fmt --all --check

# Run lint suite
lint: fmt clippy

# Cross build
crossbuild:
	cargo build --release -p gset-cli --target x86_64-unknown-linux-gnu 2>/dev/null || true
	cargo build --release -p gset-cli --target aarch64-unknown-linux-gnu 2>/dev/null || true
	cargo build --release -p gset-cli --target x86_64-apple-darwin 2>/dev/null || true
	cargo build --release -p gset-cli --target aarch64-apple-darwin 2>/dev/null || true
	cargo build --release -p gset-cli --target x86_64-pc-windows-msvc 2>/dev/null || true

clean:
	cargo clean
	rm -f gset* target/gset*
