---
title: Introduction
description: Learn what GSET is and how it can help you write code in any language syntax.
---

GSET (Generic Syntax Extension Tool) is a transpiler that bridges the gap between programming languages. It allows you to write code using the syntax and patterns of one programming language, then compile and run it using the runtime of another language.

## Why GSET?

Programming languages each have their own strengths:

- **Python**: Clean, readable syntax with powerful libraries
- **JavaScript**: Ubiquitous for web development
- **Go**: Excellent concurrency support and fast compilation
- **Java**: Mature ecosystem and cross-platform support

GSET lets you pick the syntax you prefer while targeting the runtime that best fits your needs.

## How It Works

GSET uses a two-step process:

1. **Parse**: Read your source code and build an Abstract Syntax Tree (AST)
2. **Transpile**: Convert the AST to your target language and execute

The configuration file (`gset.conf`) controls keyword mappings between languages.

## Features

- **Multi-language support**: Python, JavaScript, Go, Java, Ruby, PHP, and more
- **Keyword mapping**: Define custom translations between language keywords
- **External configuration**: All settings in `gset.conf`, no hardcoding
- **Cross-platform**: Works on Linux, macOS, and Windows
- **Open source**: Licensed under CC BY-NC 4.0
