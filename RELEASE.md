# GSET v2.0.2

**Generic Syntax Extension Tool** - Write in any language syntax, compile to any language.

[![License: CC BY-NC 4.0](https://img.shields.io/badge/License-CC%20BY--NC%204.0-yellow)](LICENSE)
[![Go Version](https://img.shields.io/badge/Go-1.21+-00ADD8?style=flat&logo=go)](https://go.dev/)
[![AUR Package](https://img.shields.io/badge/AUR-gset--git-orange)](https://aur.archlinux.org/packages/gset-git/)

## What is GSET?

GSET is a transpiler that allows you to write code in any language syntax and compile it to any other language using configurable keyword mappings.

**Example:** Write in Python syntax, compile to Go:
```
# GSET file (Python-like syntax)
print("Hello from Python!")

# Compiles to:
fmt.Println("Hello from Python!")
```

## Features

- Write in Python, JavaScript, Java, Go, or custom syntax
- Map keywords to any target language functions via `gset.conf`
- Cross-platform compiler support (Python, Node, Go, Java, etc.)
- Fully configurable - no hardcoded values
- Open source, non-commercial license

## Installation

### Linux (AUR)
```bash
yay -S gset-git
# or
paru -S gset-git
```

### Linux (Direct Download)
```bash
wget https://github.com/Crazygiscool/GSETLang/releases/download/v2.0.2/gset-linux-amd64
chmod +x gset-linux-amd64
sudo mv gset-linux-amd64 /usr/local/bin/gset
```

### macOS
1. Download `GSET-2.0.2-macOS.zip`
2. Extract `GSET.app`
3. Drag to `/Applications`
4. Run: `gset --version`

### Windows
1. Download `gset-setup-2.0.2.exe`
2. Run installer
3. Choose install location
4. Add to PATH (optional)

### Debian/Ubuntu
```bash
wget https://github.com/Crazygiscool/GSETLang/releases/download/v2.0.2/gset_2.0.2_amd64.deb
sudo dpkg -i gset_2.0.2_amd64.deb
```

## Quick Start

1. Create a GSET file (e.g., `hello.gset`):
```
print("Hello, World!")
```

2. Run it:
```bash
gset hello.gset
```

3. Output:
```
Hello, World!
```

### Custom Keywords

Add to file header or `gset.conf`:
```
shout=fmt.Println
---
shout("Hello from GSET!")
```

### Multiple Languages

GSET auto-detects file extension:

| Extension | Target Compiler |
|-----------|----------------|
| `.py` | Python (`python3`) |
| `.js` | Node.js (`node`) |
| `.go` | Go (`go run`) |
| `.java` | Java (`javac` + `java`) |

Example - Write in JavaScript syntax:
```javascript
console.log("Hello from JavaScript syntax!")
```

## Configuration

Edit `gset.conf` to customize keywords and compilers:

```conf
# Global keywords
say=fmt.Println
print=fmt.Print

# Language-specific
ext.py.say=print
ext.js.say=console.log
ext.java.say=System.out.println

# Compilers
compiler.py.command=python3
compiler.go.wrapper=package main\nfunc main() {\n##CODE##\n}
```

## License

This software is licensed under **CC BY-NC 4.0** (Creative Commons Attribution-NonCommercial 4.0).

- ✅ Use and remix allowed
- ✅ Attribution required
- ❌ No commercial/economic use

See [LICENSE](LICENSE) for full text.

## Links

- [Source Code](https://github.com/Crazygiscool/GSETLang)
- [AUR Package](https://aur.archlinux.org/packages/gset-git/)
- [Report Issues](https://github.com/Crazygiscool/GSETLang/issues)

---

**Version:** 2.0.2  
**License:** CC BY-NC 4.0  
**Copyright:** 2024 GSET Team