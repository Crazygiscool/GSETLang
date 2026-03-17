# GSETLang: Go Set Your Own Language

**GSETLang** is a meta-programmable language engine built in Go. It is designed to solve the "Syntax War" by giving the developer total control over the structural and lexical rules of their code. Instead of being forced into a specific style, you define the environment, and the compiler adapts to you.

---

## 💡 The Philosophy

Traditional programming languages are rigid; if you don't like curly braces or specific keywords, you have to switch languages entirely. **GSETLang** operates on the principle of **"Syntax Agnosticism."** It treats the visual representation of code as a configurable layer, allowing for high-level customization without sacrificing performance.

---

## 🛠 Features

* **Structural Flexibility:** Native support for both **Indentation-based** (Python-style) and **Bracket-based** (C-style) block scoping within the same project.
* **Keyword Mapping:** Full authority to rename core actions. Map "print" to "shout," "var" to "let," or "func" to "task" via simple configuration.
* **Layered Configuration:** A three-tier hierarchy for rule definitions:
    1. **Global Defaults:** The language's baseline standards.
    2. **Project Environment (`.gsetenv`):** Standardizes syntax across a specific repository.
    3. **File Headers:** Inline overrides at the top of a `.gset` file for specialized logic.
* **Go-Powered Engine:** Built on the Go toolchain for rapid compilation, efficient memory management, and a seamless path toward self-hosting.

---

## 🏗 How It Works

GSETLang uses a "Chameleon Lexer" that acts as a pre-processor. It reads the configuration header of a file to set its internal state. 

If a file is set to `indent` mode, the engine tracks leading whitespace and injects virtual tokens into the stream. If set to `bracket` mode, it scans for traditional delimiters. This allows the core Abstract Syntax Tree (AST) to remain consistent regardless of how the code looks on the screen.



---

## 🎯 Use Cases

* **DSL Creation:** Quickly spin up a Domain Specific Language with custom terminology.
* **Educational Tools:** Create a simplified coding environment for beginners using native-language keywords.
* **Team Standardization:** Define a specific "Team Style" that enforces one look across all contributors.
* **Legacy Porting:** Mimic the syntax of older languages while utilizing a modern, high-speed backend.
