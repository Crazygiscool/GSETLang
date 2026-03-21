---
title: Multiple Languages
description: Using GSET with different language targets.
---

GSET can transpile code to multiple target languages. Each has its own strengths.

## Cross-Language Development

You can write core logic once and target multiple languages:

```
main() {
    data = [1, 2, 3, 4, 5]
    sum = 0
    i = 0
    while i < length(data) {
        sum = sum + data[i]
        i = i + 1
    }
    print(sum)
}
```

This same code can run on:
- Python runtime
- Node.js runtime
- Any other supported runtime

## Language-Specific Transformations

Use conditional keywords:

```ini
[keywords.py.go]
print = fmt.Println
list = make([]int, 0)

[keywords.py.node]
print = console.log
list = []
```

## Multi-Compiler Workflow

Create platform-specific builds:

```bash
# Build for Python ecosystem
gset transpile core.gset -t python -o core_py.py

# Build for JavaScript ecosystem
gset transpile core.gset -t node -o core_js.js

# Build for Go ecosystem
gset transpile core.gset -t go -o core_go.go
```

## Shared Libraries

GSET enables sharing logic across teams using different languages:

```
math_utils.gset:
    add(a, b) { return a + b }
    multiply(a, b) { return a * b }
```

Each team can transpile to their language of choice.

## Platform-Specific Code

Use comments to mark platform-specific sections:

```
main() {
    // @platform:python
    print("Python platform")
    
    // @platform:node
    console.log("Node platform")
}
```

GSET processes these directives during transpilation.
