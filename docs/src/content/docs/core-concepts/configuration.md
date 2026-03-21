---
title: Configuration
description: Complete guide to gset.conf settings.
---

GSET uses `gset.conf` for all configuration. It should be in your project directory or home folder.

## File Format

GSET uses INI-style configuration:

```ini
[section]
key = value
```

## Full Example

```ini
[compiler]
default = python
verbose = false

[python]
command = python3
extension = .py
timeout = 30

[node]
command = node
extension = .js
timeout = 30

[java]
command = java
extension = .java
timeout = 60

[go]
command = go run
extension = .go
timeout = 60

[keywords.py.go]
def = func
print = fmt.Println
True = true
False = false
None = nil

[keywords.py.java]
print = System.out.println
```

## Compiler Section

| Setting | Default | Description |
|---------|---------|-------------|
| `default` | `python` | Default compiler to use |
| `verbose` | `false` | Enable verbose output |

## Language Sections

Each language section (`[python]`, `[node]`, `[java]`, etc.) supports:

| Setting | Default | Description |
|---------|---------|-------------|
| `command` | - | Command to execute |
| `extension` | - | File extension for output |
| `timeout` | `30` | Execution timeout in seconds |

## Keywords Section

Format: `[keywords.<source>.<target>]`

Define keyword translations from source to target language.

## Environment Variables

GSET also reads from environment variables:

- `GSET_CONFIG` - Path to custom config file
- `GSET_DEFAULT_COMPILER` - Override default compiler
- `GSET_VERBOSE` - Enable verbose mode
