---
title: CLI Reference
description: Complete reference for GSET command line interface.
---

## Commands

### `gset run`

Transpile and execute a GSET file.

```bash
gset run <file> [options]
```

**Options:**

| Option | Description |
|--------|-------------|
| `-c, --compiler` | Specify target compiler |
| `-t, --timeout` | Set execution timeout (seconds) |
| `-v, --verbose` | Enable verbose output |

**Examples:**

```bash
gset run hello.gset
gset run script.gset -c python
gset run app.gset -t 60 -v
```

### `gset version`

Display version information.

```bash
gset version
```

Output: `GSET v2.0.2`

### `gset help`

Show help message.

```bash
gset help
```

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error |
| 2 | File not found |
| 3 | Compilation error |
| 4 | Runtime error |
| 5 | Timeout |

## Environment Variables

| Variable | Description |
|----------|-------------|
| `GSET_CONFIG` | Path to config file |
| `GSET_DEFAULT_COMPILER` | Default compiler |
| `GSET_VERBOSE` | Enable verbose mode |
| `GSET_TIMEOUT` | Default timeout |

## Configuration File

GSET looks for `gset.conf` in:
1. Current directory
2. `./config/`
3. `$HOME/.gset/`
4. `/etc/gset/`
