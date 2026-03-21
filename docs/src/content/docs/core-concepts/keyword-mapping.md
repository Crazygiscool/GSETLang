---
title: Keyword Mapping
description: Configure how keywords are translated between languages.
---

Keyword mapping is the core of GSET's flexibility. It lets you write code using one language's syntax while targeting another language's runtime.

## Configuration Format

In your `gset.conf`:

```ini
[keywords.<source>.<target>]
keyword = replacement
```

## Examples

### Python to Go

```ini
[keywords.py.go]
def = func
print = fmt.Println
True = true
False = false
None = nil
elif = else if
```

### JavaScript to Python

```ini
[keywords.js.py]
function = def
const = 
let = 
var = 
console.log = print
const = 
true = True
false = False
```

### Any to Java

```ini
[keywords.any.java]
print = System.out.println
main = public static void main
```

## Common Mappings

### Control Flow

| Python | JavaScript | Go | Java |
|--------|------------|-----|------|
| `if` | `if` | `if` | `if` |
| `elif` | `else if` | `else if` | `else if` |
| `else` | `else` | `else` | `else` |
| `for` | `for` | `for` | `for` |
| `while` | `while` | `for` | `while` |

### Data Types

| Python | JavaScript | Go | Java |
|--------|------------|-----|------|
| `int` | `let` | `int` | `int` |
| `str` | `const` | `string` | `String` |
| `list` | `[]` | `[]` | `[]` |
| `dict` | `{}` | `map` | `HashMap` |

## Multiple Targets

GSET supports mapping one source to multiple targets:

```ini
[keywords.py.go]
def = func

[keywords.py.java]
def = public static void
```

## Inheritance

Use `any` as a wildcard source to apply mappings to all targets:

```ini
[keywords.any]
print = console.log
```
