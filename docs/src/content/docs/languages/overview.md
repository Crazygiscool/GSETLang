---
title: Language Support
description: Languages supported by GSET.
---

GSET supports multiple programming languages. Each language has specific compilers and configurations.

## Supported Languages

### Python

```ini
[python]
command = python3
extension = .py
```

Python is the default target for GSET. Requires `python3` installed.

### JavaScript (Node.js)

```ini
[node]
command = node
extension = .js
```

Requires Node.js runtime.

### Go

```ini
[go]
command = go run
extension = .go
```

Requires Go SDK installed.

### Java

```ini
[java]
command = java
extension = .java
```

Requires JDK installed. GSET will compile `.java` files automatically.

### Ruby

```ini
[ruby]
command = ruby
extension = .rb
```

Requires Ruby interpreter.

### PHP

```ini
[php]
command = php
extension = .php
```

Requires PHP CLI.

## Adding Custom Languages

GSET can be extended to support additional languages by:

1. Adding compiler configuration in `gset.conf`
2. Defining keyword mappings
3. Implementing custom transpilers in the source code

## Runtime Requirements

| Language | Required Software |
|----------|------------------|
| Python | Python 3.x |
| JavaScript | Node.js |
| Go | Go SDK |
| Java | JDK |
| Ruby | Ruby |
| PHP | PHP CLI |
