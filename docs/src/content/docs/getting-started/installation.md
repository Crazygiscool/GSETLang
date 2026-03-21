---
title: Installation
description: How to install GSET on your system.
---

## Pre-built Binaries

Download the latest release from the [GitHub Releases](https://github.com/Crazygiscool/GSETLang/releases) page.

### Linux

```bash
# Download the binary
wget https://github.com/Crazygiscool/GSETLang/releases/download/v2.0.2/gset-linux-amd64

# Make it executable
chmod +x gset-linux-amd64

# Move to your PATH
sudo mv gset-linux-amd64 /usr/local/bin/gset
```

### macOS

```bash
# Download and unzip
unzip GSET-2.0.2-macOS.zip

# Make it executable
chmod +x gset-darwin-amd64

# Move to your PATH
sudo mv gset-darwin-amd64 /usr/local/bin/gset
```

### Windows

Download `gset-windows-amd64.exe` from the releases page and add it to your PATH.

## Package Managers

### Arch Linux (AUR)

```bash
# Using yay or paru
yay -S gset
```

### APT (Debian/Ubuntu)

Download the `.deb` package from releases and install:

```bash
sudo dpkg -i gset_2.0.2_amd64.deb
```

## Build from Source

Requirements: Go 1.21+

```bash
git clone https://github.com/Crazygiscool/GSETLang.git
cd GSETLang
go build -o gset
```

## Verify Installation

```bash
gset version
```

You should see: `GSET v2.0.2`
