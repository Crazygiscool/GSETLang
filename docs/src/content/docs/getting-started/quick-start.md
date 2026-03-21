---
title: Quick Start
description: Get up and running with GSET in minutes.
---

## Basic Usage

Once installed, you can run GSET with a source file:

```bash
gset run yourfile.gset
```

GSET automatically detects the file extension and uses the appropriate compiler.

## Create Your First GSET File

Create a file called `hello.gset`:

```
main() {
    print("Hello, GSET!")
}
```

## Run It

```bash
gset run hello.gset
```

GSET will transpile and execute the code.

## Using Configuration

GSET reads settings from `gset.conf` in the current directory or your home folder.

Example `gset.conf`:

```ini
[compiler]
default = python

[python]
command = python3
extension = .py

[java]
command = java
extension = .java
```

## Command Line Options

| Option | Description |
|--------|-------------|
| `gset run <file>` | Transpile and run a file |
| `gset version` | Show version information |
| `gset help` | Display help message |

## Next Steps

- Learn about [keyword mapping](/core-concepts/keyword-mapping/)
- Explore [configuration options](/core-concepts/configuration/)
- See [examples](/examples/basic-usage/)
