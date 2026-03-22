# GSET Makefile

.PHONY: all build test clean install crossbuild release

VERSION := 2.1.2
REPO := github.com/Crazygiscool/GSETLang

all: build

# Build for current platform
build:
	@echo "Building GSET v$(VERSION)..."
	go build -ldflags="-s -w -X main.version=$(VERSION)" -o gset .
	@echo "Built: ./gset"
	cp gset test/gset
	@echo "Copied to test/gset"

# Build with debug info
debug:
	go build -o gset .
	@echo "Built: ./gset (debug)"
	cp gset test/gset
	@echo "Copied to test/gset"

# Run tests
test:
	go test ./...

# Run test files
test-files:
	@./gset transpile test/comprehensive.gset

# Clean build artifacts
clean:
	rm -rf dist/
	rm -f gset
	rm -f gset.exe
	rm -f test/gset

# Cross-compile for all platforms
crossbuild:
	@chmod +x build.sh
	@./build.sh
	cp gset test/gset
	@echo "Copied to test/gset"

# Create release (requires GitHub CLI)
release: crossbuild
	@echo "Creating GitHub release..."
	gh release create v$(VERSION) \
		--title "GSET v$(VERSION)" \
		--notes "See CHANGELOG.md for details" \
		dist/*

# Install locally (for development)
install: build
	install -Dm755 gset $(HOME)/.local/bin/gset
	@echo "Installed to ~/.local/bin/gset"

# Uninstall
uninstall:
	rm -f $(HOME)/.local/bin/gset
	@echo "Uninstalled GSET"

# Format code
fmt:
	go fmt ./...

# Lint code
lint:
	golangci-lint run || echo "Install golangci-lint for linting"

# Generate documentation
docs:
	cd docs && npm run build

# Watch mode (requires air)
dev:
	air

# Version info
version:
	@echo "v$(VERSION)"

# Help
help:
	@echo "GSET Makefile"
	@echo ""
	@echo "Targets:"
	@echo "  build       - Build for current platform"
	@echo "  debug       - Build with debug info"
	@echo "  test        - Run Go tests"
	@echo "  test-files  - Run test .gset files"
	@echo "  clean       - Remove build artifacts"
	@echo "  crossbuild  - Build for all platforms"
	@echo "  release     - Create GitHub release"
	@echo "  install     - Install locally"
	@echo "  uninstall   - Remove local installation"
	@echo "  fmt         - Format code"
	@echo "  lint        - Lint code"
	@echo "  help        - Show this help"
