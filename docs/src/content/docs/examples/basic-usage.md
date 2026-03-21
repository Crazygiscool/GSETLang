---
title: Basic Usage
description: Common GSET usage patterns and examples.
---

## Running GSET Files

```bash
gset run hello.gset
```

GSET will:
1. Read the file
2. Parse the syntax
3. Transpile to target language
4. Execute with appropriate runtime

## Example: Hello World

Create `hello.gset`:

```
main() {
    print("Hello, World!")
}
```

Run it:

```bash
$ gset run hello.gset
Hello, World!
```

## Example: Variables and Arithmetic

```
main() {
    x = 10
    y = 20
    result = x + y
    print(result)
}
```

## Example: Conditionals

```
main() {
    age = 25
    if age >= 18 {
        print("Adult")
    } else {
        print("Minor")
    }
}
```

## Example: Loops

```
main() {
    i = 0
    while i < 5 {
        print(i)
        i = i + 1
    }
}
```

## Example: Functions

```
main() {
    result = add(5, 3)
    print(result)
}

add(a, b) {
    return a + b
}
```

## Output Formats

GSET supports multiple output formats:

```bash
# Run directly (default)
gset run file.gset

# Show transpiled output only
gset transpile file.gset

# Save output to file
gset transpile file.gset -o output.py
```
