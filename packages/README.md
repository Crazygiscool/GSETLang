# GSET Packages

This directory contains package files for various Linux distributions and operating systems.

## Installation Methods

### Linux (Manual)

```bash
# Download the latest release
wget https://github.com/gset-lang/gset/releases/latest/download/gset-linux-amd64

# Make executable
chmod +x gset-linux-amd64

# Install to PATH
sudo mv gset-linux-amd64 /usr/local/bin/gset
```

### Arch Linux (AUR)

```bash
# Using yay (AUR helper)
yay -S gset-git

# Or manually
git clone https://aur.archlinux.org/gset-git.git
cd gset-git
makepkg -si
```

### Debian/Ubuntu (APT)

```bash
# Download the .deb package
wget https://github.com/gset-lang/gset/releases/latest/download/gset_1.0.0_amd64.deb

# Install
sudo dpkg -i gset_1.0.0_amd64

# Fix dependencies if needed
sudo apt-get -f install
```

### macOS

```bash
# Using Homebrew (if available in tap)
brew install gset

# Or manual
wget https://github.com/gset-lang/gset/releases/latest/download/gset-darwin-amd64
chmod +x gset-darwin-amd64
sudo mv gset-darwin-amd64 /usr/local/bin/gset
```

### Windows

#### Option 1: Chocolatey
```powershell
choco install gset
```

#### Option 2: Scoop
```powershell
scoop install gset
```

#### Option 3: Manual
1. Download `gset-windows-amd64.exe` from releases
2. Add to PATH or use from any directory

### Building from Source

```bash
# Clone the repository
git clone https://github.com/gset-lang/gset.git
cd gset

# Build
go build -o gset .

# Or use the build script
./scripts/build.sh
```

## Configuration

After installation, a default configuration file is located at:
- Linux: `/etc/gset.conf` or `~/.gset.conf`
- macOS: `/usr/local/etc/gset.conf` or `~/.gset.conf`
- Windows: Installed directory or user's config

You can also create a `gset.conf` in the same directory as your source files.

## Creating Custom Packages

See the individual package directories for package-specific build instructions:
- `arch/` - Arch Linux PKGBUILD
- `aur/` - AUR package
- `debian/` - Debian control file
- `windows/` - NSIS installer script
- `choco/` - Chocolatey nuspec