---
title: Custom Keywords
description: Define your own keyword mappings for personalized syntax.
---

## Why Custom Keywords?

Custom keywords let you:
- Create domain-specific syntax
- Mix syntaxes from different languages
- Define your own DSL (Domain Specific Language)

## Basic Mapping

In `gset.conf`:

```ini
[keywords.py]
greet = print
calc = fmt.Println
```

Then in your code:

```
main() {
    greet("Hello!")
    calc("Result:", 42)
}
```

## Creating a DSL

### Example: Simple Test Language

```ini
[keywords.py]
test = 
assert = if !(%s) { panic("test failed") }
should_equal = assert
before = func init()
after = func cleanup()
```

Now write tests in readable syntax:

```
main() {
    before()
    
    x = 10
    x should_equal 10
    
    after()
}
```

### Example: Configuration DSL

```ini
[keywords.py]
server = const
port = = 8080
host = = "localhost"
```

Creates config-like code:

```
main() {
    server.port = 8080
    server.host = "localhost"
}
```

## Combining Languages

Mix Python, JavaScript, and Go syntax:

```ini
[keywords.py.js.go]
fn = func
let = var
const = const
```

## Best Practices

1. **Keep mappings consistent** - Define once, use many times
2. **Document your DSL** - Make keywords self-documenting
3. **Start simple** - Begin with essential keywords
4. **Test thoroughly** - Verify transpilation produces expected output
