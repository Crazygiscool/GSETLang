---
title: How It Works
description: Understanding GSET's architecture and how it transpiles code.
---

GSET uses a classic compiler architecture with three main stages:

## 1. Lexing (Tokenization)

The lexer reads your source code and breaks it into tokens - the smallest meaningful units:

- Keywords (`if`, `else`, `while`)
- Identifiers (`variableName`, `functionName`)
- Operators (`+`, `-`, `=`, `==`)
- Literals (`42`, `"hello"`, `true`)
- Punctuation (`{`, `}`, `(`, `)`, `;`)

## 2. Parsing (AST Generation)

The parser takes the token stream and builds an Abstract Syntax Tree (AST). The AST represents the hierarchical structure of your program:

```
Program
└── CallExpression (main)
    └── BlockStatement
        └── ExpressionStatement
            └── CallExpression (print)
                └── StringLiteral ("Hello")
```

## 3. Transpilation & Execution

The transpiler walks the AST and:
1. Translates syntax keywords based on your configuration
2. Converts to the target language's structure
3. Executes using the appropriate runtime

## Keyword Mapping

The key feature is keyword translation. In your `gset.conf`:

```ini
[keywords]
# Map source keywords to target keywords
def = func
print = fmt.Println
```

This allows you to write in one syntax while targeting another.

## File Extension Detection

GSET uses file extensions to determine the source and target languages:

| Extension | Language |
|-----------|----------|
| `.py` | Python |
| `.js` | JavaScript |
| `.go` | Go |
| `.java` | Java |
| `.rb` | Ruby |
| `.gset` | Auto-detect from config |
