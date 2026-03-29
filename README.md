# GSET - Generic Syntax Extension Tool

<div align="center">

**Write in any language syntax, compile to any language.**

[![License: CC BY-NC 4.0](https://img.shields.io/badge/License-CC%20BY--NC%204.0-yellow)](LICENSE)
[![Go Version](https://img.shields.io/badge/Go-1.21+-00ADD8?style=flat&logo=go)](https://go.dev/)
[![Release](https://img.shields.io/github/v/release/Crazygiscool/GSETLang)](https://github.com/Crazygiscool/GSETLang/releases)
[![Tests](https://img.shields.io/github/actions/workflow/status/Crazygiscool/GSETLang/test.yml)](https://github.com/Crazygiscool/GSETLang/actions)
[![Go Report Card](https://goreportcard.com/badge/github.com/Crazygiscool/GSETLang)](https://goreportcard.com/report/github.com/Crazygiscool/GSETLang)

</div>

---

## What is GSET?

GSET (Generic Syntax Extension Tool) v2.1.3 is a transpiler that allows you to write code using any language's syntax (Python, JavaScript, Java, Go, etc.) and compile it to run on any target language's runtime.

**Write this:**
```gset
print("Hello from Python syntax!")
nums = [1, 2, 3]
for i in nums {
    print(i)
}
```

**Compile and run with:**
- Python → `print("Hello from Python syntax!")`
- JavaScript → `console.log("Hello from Python syntax!")`
- Java → `System.out.println("Hello from Python syntax!")`
- Go → `fmt.Println("Hello from Python syntax!")`

---

## Quick Install (One-Liner)

```bash
# Linux / macOS / Windows (Git Bash/WSL)
curl -fsSL https://raw.githubusercontent.com/Crazygiscool/GSETLang/main/install.sh | bash
```

That's it! The script auto-detects your OS and architecture.

---

## Installation Methods

### 1. Quick Install (Recommended)

**Linux / macOS / Windows (Git Bash, WSL, MSYS2)**
```bash
curl -fsSL https://raw.githubusercontent.com/Crazygiscool/GSETLang/main/install.sh | bash
```

**Windows (PowerShell)**
```powershell
irm https://raw.githubusercontent.com/Crazygiscool/GSETLang/main/install.ps1 | iex
```

### 2. Winget (Windows)

```powershell
winget install GSETLang.GSET
```

### 3. Scoop (Windows)

```powershell
scoop bucket add extras
scoop install gset
```

### 4. Manual Download

**Linux**
```bash
wget https://github.com/Crazygiscool/GSETLang/releases/latest/download/gset-linux-amd64.tar.gz
tar -xzf gset-linux-amd64.tar.gz
chmod +x gset && mv gset ~/.local/bin/
```

**macOS**
```bash
# Intel
wget https://github.com/Crazygiscool/GSETLang/releases/latest/download/gset-darwin-amd64.tar.gz
# Apple Silicon
wget https://github.com/Crazygiscool/GSETLang/releases/latest/download/gset-darwin-arm64.tar.gz
tar -xzf gset-darwin-*.tar.gz
chmod +x gset && mv gset ~/.local/bin/
```

**Windows (PowerShell)**
```powershell
irm https://github.com/Crazygiscool/GSETLang/releases/latest/download/gset-windows-amd64.zip -OutFile gset.zip
Expand-Archive gset.zip -DestinationPath C:\Tools\gset
# Add C:\Tools\gset to your PATH
```

### 5. Build from Source

```bash
git clone https://github.com/Crazygiscool/GSETLang
cd GSETLang
go build -o gset .
```

### 6. Arch Linux (AUR)

```bash
yay -S gset-git
# or
paru -S gset-git
```

---

## Quick Start

### 1. Create a GSET file
```bash
cat > hello.gset << 'EOF'
function main() {
    print("Hello, World!")
}

main()
EOF
```

### 2. Run it
```bash
gset run hello.gset
```

**Output:**
```
Hello, World!
```

### 3. Or just transpile to see the output
```bash
gset transpile hello.gset
```

**Output:**
```go
func main() {
    fmt.Println("Hello, World!")
}
main()
```

---

## Features

### Variables & Types
```gset
x = 42
name = "GSET"
isActive = true
price = 3.14
nums = [1, 2, 3, 4, 5]
```

### Control Flow
```gset
if x > 10 {
    print("big")
} else if x > 5 {
    print("medium")
} else {
    print("small")
}
```

### Loops
```gset
# For loop
for i in nums {
    print(i)
}

# For-each with index
for idx, val in nums {
    print(idx)
    print(val)
}

# While loop
while count > 0 {
    print(count)
    count = count - 1
}
```

### Functions
```gset
function greet(name) {
    print("Hello, ")
    print(name)
}

function factorial(n) {
    if n <= 1 {
        return 1
    }
    return n * factorial(n - 1)
}
```

### List Comprehensions
```gset
nums = [1, 2, 3, 4, 5]
squared = [x * x for x in nums]
evens = [x for x in nums if x % 2 == 0]
```

### Classes
```gset
class Person {
    name = "John"
    age = 30
    
    function greet() {
        print("Hello, my name is " + name)
    }
}
```

### Error Handling
```gset
try {
    result = riskyOperation()
} catch e {
    print("Error: " + e)
} finally {
    cleanup()
}
```

### Pattern Matching (Match)
```gset
match value {
    case 1:
        print("one")
    case 2:
        print("two")
    default:
        print("other")
}
```

---

## Language Targets

GSET auto-detects target language from file extension:

| Extension | Target Compiler | Example Output |
|-----------|-----------------|----------------|
| `.gset` | Go (default) | `fmt.Println("Hello")` |
| `.py` | Python | `print("Hello")` |
| `.js` | Node.js | `console.log("Hello")` |
| `.go` | Go | `fmt.Println("Hello")` |
| `.java` | Java | `System.out.println("Hello")` |
| `.rb` | Ruby | `puts "Hello"` |
| `.php` | PHP | `echo "Hello";` |
| `.ts` | TypeScript | `console.log("Hello")` |
| `.cs` | C# | `Console.WriteLine("Hello")` |
| `.rs` | Rust | `println!("Hello")` |
| `.swift` | Swift | `print("Hello")` |
| `.kt` | Kotlin | `println("Hello")` |
| `.cpp` | C++ | `std::cout << "Hello" << std::endl` |
| `.c` | C | `printf("Hello\n")` |

---

## Custom Keywords

### Via File Header
```gset
say=fmt.Println
---
say("Hello with custom keyword!")
```

### Via Configuration File
Create `gset.conf` in your project directory:

```conf
# Global keywords
say=fmt.Println
print=fmt.Print

# Language-specific
ext.py.say=print
ext.js.say=console.log
ext.java.say=System.out.println
```

---

## Security Features

GSET includes comprehensive security hardening for safe operation:

### Input Validation
- **Path Traversal Prevention**: Blocks `../` and similar path traversal attempts
- **Input Size Limits**: Maximum 10MB input file size
- **Identifier Validation**: Max 256 character identifiers
- **Safe File Extensions**: Whitelist-based extension validation

### Command Security
- **Command Injection Prevention**: Sanitizes shell command arguments
- **Dangerous Pattern Detection**: Blocks commands containing `;`, `&&`, `||`, `|`, `` ` ``, `$(`
- **Path Traversal in Commands**: Prevents `..` in command arguments

### Parser Security
- **Nesting Depth Limits**: Maximum 100 levels of nesting
- **Statement Limits**: Maximum 1000 statements per block, 10000 per program
- **Depth Tracking**: Prevents stack overflow from deeply nested code

### Config Validation
- **Config File Size**: Maximum 1MB configuration file size
- **Keyword Limits**: Maximum 1000 keywords
- **Dangerous Command Detection**: Validates compiler commands for safety

### Runtime Security
- **Safe Temporary Files**: Uses `/tmp` with unique naming
- **Graceful Shutdown**: Handles SIGINT/SIGTERM for clean exits

---

## Development

```bash
# Clone and build
git clone https://github.com/Crazygiscool/GSETLang
cd GSETLang
make build

# Run tests
make test

# Run tests with race detector
make test-race

# Run benchmark tests
make benchmark

# Cross-compile for all platforms
make crossbuild

# Install locally
make install
```

---

## Project Structure

```
GSETLang/
├── main.go              # Main entry point
├── parser/              # Parser package
├── lexer/               # Lexer package
├── transpiler/          # Transpiler package
├── config/              # Configuration package
├── security/           # Security utilities
├── logger/             # Logging package
├── test/                # Test files
├── docs/               # Documentation
└── packages/          # Distribution packages
```

---

## Testing

### Unit Tests
GSET includes comprehensive unit tests for all core packages:

```bash
# Run all unit tests
go test ./...

# Run with race detector
go test -race ./...

# Run benchmarks
go test -bench=. -benchmem ./...
```

### Test Coverage

| Package | Tests |
|---------|-------|
| lexer | 45+ tests |
| parser | 20+ tests |
| transpiler | 20+ tests |
| security | 8 test suites |
| config | 2 test suites |

---

## Logging

GSET includes structured logging with multiple levels:

```bash
# Enable debug logging
GSET_DEBUG=1 gset run hello.gset
```

Log levels: DEBUG, INFO, WARN, ERROR, FATAL

---

## Available Platforms

| OS | Arch | Download |
|----|-------|----------|
| Linux | amd64 | [gset-linux-amd64.tar.gz](https://github.com/Crazygiscool/GSETLang/releases/latest/download/gset-linux-amd64.tar.gz) |
| Linux | arm64 | [gset-linux-arm64.tar.gz](https://github.com/Crazygiscool/GSETLang/releases/latest/download/gset-linux-arm64.tar.gz) |
| Linux | 386 | [gset-linux-386.tar.gz](https://github.com/Crazygiscool/GSETLang/releases/latest/download/gset-linux-386.tar.gz) |
| macOS | amd64 | [gset-darwin-amd64.tar.gz](https://github.com/Crazygiscool/GSETLang/releases/latest/download/gset-darwin-amd64.tar.gz) |
| macOS | arm64 | [gset-darwin-arm64.tar.gz](https://github.com/Crazygiscool/GSETLang/releases/latest/download/gset-darwin-arm64.tar.gz) |
| Windows | amd64 | [gset-windows-amd64.zip](https://github.com/Crazygiscool/GSETLang/releases/latest/download/gset-windows-amd64.zip) |
| Windows | 386 | [gset-windows-386.zip](https://github.com/Crazygiscool/GSETLang/releases/latest/download/gset-windows-386.zip) |

---

## License

**CC BY-NC 4.0** - Creative Commons Attribution-NonCommercial 4.0

- Use and remix freely
- Attribution required
- No commercial use

See [LICENSE](LICENSE) for full text.

---

## Links

- [GitHub Repository](https://github.com/Crazygiscool/GSETLang)
- [Releases](https://github.com/Crazygiscool/GSETLang/releases)
- [Documentation](https://gsetlang.vercel.app)
- [Report Issues](https://github.com/Crazygiscool/GSETLang/issues)

---

<div align="center">

**Version:** 2.1.3 | **License:** CC BY-NC 4.0 | **Copyright:** 2024-2026 GSET Team

</div>
