# GSET - Generic Syntax Extension Tool

<div align="center">

**Write in any language syntax, compile to any language.**

[![License: CC BY-NC 4.0](https://img.shields.io/badge/License-CC%20BY--NC%204.0-yellow)](LICENSE)
[![Go Version](https://img.shields.io/badge/Go-1.21+-00ADD8?style=flat&logo=go)](https://go.dev/)
[![AUR Package](https://img.shields.io/badge/AUR-gset--git-orange)](https://aur.archlinux.org/packages/gset-git/)

</div>

---

## What is GSET?

GSET is a transpiler that allows you to write code using any language's syntax (Python, JavaScript, Java, Go, etc.) and compile it to run on any target language's runtime.

**Write this:**
```
print("Hello from Python syntax!")
```

**Compile and run with:**
- Python → `print("Hello from Python syntax!")`
- JavaScript → `console.log("Hello from Python syntax!")`  
- Java → `System.out.println("Hello from Python syntax!")`
- Go → `fmt.Println("Hello from Python syntax!")`

---

## Installation

### Linux (AUR - Recommended)
```bash
yay -S gset-git
# or
paru -S gset-git
```

### Linux (Direct)
```bash
wget https://github.com/Crazygiscool/GSETLang/releases/latest/download/gset-linux-amd64
chmod +x gset-linux-amd64
sudo mv gset-linux-amd64 /usr/local/bin/gset
```

### macOS
```bash
# Using Homebrew (if available)
brew install gset

# Or download the app bundle
wget https://github.com/Crazygiscool/GSETLang/releases/latest/download/GSET-2.0.2-macOS.zip
unzip GSET-2.0.2-macOS.zip
# Drag GSET.app to /Applications
```

### Windows
```bash
# Using Chocolatey
choco install gset

# Or download installer
wget https://github.com/Crazygiscool/GSETLang/releases/latest/download/gset-setup.exe
# Run the installer
```

### Debian/Ubuntu
```bash
wget https://github.com/Crazygiscool/GSETLang/releases/latest/download/gset_2.0.2_amd64.deb
sudo dpkg -i gset_2.0.2_amd64.deb
```

### Build from Source
```bash
git clone https://github.com/Crazygiscool/GSETLang
cd GSETLang
go build -o gset .
```

---

## Quick Start

### 1. Create a GSET file
```bash
echo 'print("Hello, World!")' > hello.gset
```

### 2. Run it
```bash
gset hello.gset
```

**Output:**
```
Hello, World!
```

---

## Language Syntax Support

GSET auto-detects the target language from file extension:

| Extension | Target Compiler | Example Output |
|-----------|-----------------|-----------------|
| `.py` | Python | `print("Hello")` |
| `.js` | Node.js | `console.log("Hello")` |
| `.go` | Go | `fmt.Println("Hello")` |
| `.java` | Java | `System.out.println("Hello")` |
| `.rb` | Ruby | `puts "Hello"` |
| `.php` | PHP | `echo "Hello"` |
| `.gset` | Go (default) | `fmt.Println("Hello")` |

**Example - JavaScript syntax in a .js file:**
```javascript
console.log("Hello from JavaScript syntax!")
```
```bash
$ gset hello.js
Hello from JavaScript syntax!
```

---

## Custom Keywords

### Method 1: File Header
Define keywords at the top of your file using `---` as separator:
```
shout=fmt.Println
---
shout("Hello with custom keyword!")
```

### Method 2: Global Config
Edit `/etc/gset.conf` or create `gset.conf` in your project directory:

```conf
# Global keywords
say=fmt.Println
print=fmt.Print

# Language-specific keywords
ext.py.say=print
ext.js.say=console.log
ext.java.say=System.out.println

# Compiler settings
compiler.py.command=python3
compiler.go.wrapper=package main\nfunc main() {\n##CODE##\n}
```

---

## Configuration Options

### Keyword Mapping
```conf
# Map any keyword to any target
myprint=fmt.Println
custom=System.out.println
echo=print
```

### Compiler Configuration
```conf
# Custom compiler for each extension
compiler.py.command=python3
compiler.py.args=
compiler.py.wrapper=

compiler.go.command=go
compiler.go.args=run
compiler.go.wrapper=package main\nfunc main() {\n##CODE##\n}
```

### Extension-Specific Keywords
```conf
ext.py.say=print
ext.py.println=print
ext.js.say=console.log
ext.js.log=console.log
ext.java.say=System.out.println
ext.go.say=fmt.Println
```

---

## Project Structure

```
GSETLang/
├── ast/           # Abstract Syntax Tree definitions
├── lexer/         # Tokenizer
├── parser/        # Recursive descent parser
├── transpiler/    # Code translation & execution
├── config/        # Configuration loader
├── packages/      # OS-specific packages
│   ├── arch/      # Arch Linux PKGBUILD
│   ├── aur/       # AUR package
│   ├── debian/    # APT package files
│   ├── windows/  # Inno Setup script
│   └── macos/     # PKG builder
├── scripts/       # Build & upload scripts
├── gset.conf      # Default configuration
├── LICENSE        # CC BY-NC 4.0
└── RELEASE.md     # Release notes
```

---

## Usage Examples

### Basic Function Call
```bash
$ echo 'print("Hello GSET!")' > test.gset
$ gset test.gset
Hello GSET!
```

### Using File Extension for Language Target
```bash
$ echo 'console.log("Running with Node!")' > test.js
$ gset test.js
Running with Node!
```

### Custom Keywords
```bash
$ cat > mycode.gset << 'EOF'
shout=fmt.Println
---
shout("This uses shout instead of print!")
EOF
$ gset mycode.gset
This uses shout instead of print!
```

### Multiple Statements
```bash
$ echo 'print("Line 1")
print("Line 2")' > multi.gset
$ gset multi.gset
Line 1
Line 2
```

---

## License

**CC BY-NC 4.0** - Creative Commons Attribution-NonCommercial 4.0

- ✅ Use and remix freely
- ✅ Attribution required
- ❌ No commercial use

See [LICENSE](LICENSE) for full text.

---

## Links

- [GitHub Repository](https://github.com/Crazygiscool/GSETLang)
- [AUR Package](https://aur.archlinux.org/packages/gset-git/)
- [Report Issues](https://github.com/Crazygiscool/GSETLang/issues)

---

<div align="center">

**Version:** 2.0.2 | **License:** CC BY-NC 4.0 | **Copyright:** 2024 GSET Team

</div>