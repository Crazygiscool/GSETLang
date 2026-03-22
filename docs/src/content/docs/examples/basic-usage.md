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

```gset
function main() {
    print("Hello, World!")
}

main()
```

Run it:

```bash
$ gset run hello.gset
Hello, World!
```

## Example: Variables and Arithmetic

```gset
function main() {
    x = 10
    y = 20
    result = x + y
    print(result)
}

main()
```

## Example: Arrays and For Loops

```gset
function main() {
    nums = [1, 2, 3, 4, 5]
    
    for i in nums {
        print(i)
    }
    
    sum = 0
    for n in nums {
        sum = sum + n
    }
    print(sum)
}

main()
```

## Example: While Loops

```gset
function main() {
    count = 5
    while count > 0 {
        print(count)
        count = count - 1
    }
    print("Blastoff!")
}

main()
```

## Example: Conditionals

```gset
function main() {
    score = 85
    
    if score >= 90 {
        print("Grade: A")
    } else if score >= 80 {
        print("Grade: B")
    } else if score >= 70 {
        print("Grade: C")
    } else {
        print("Grade: F")
    }
}

main()
```

## Example: Functions

```gset
function greet(name) {
    print("Hello, ")
    print(name)
}

function add(a, b) {
    return a + b
}

function factorial(n) {
    if n <= 1 {
        return 1
    }
    return n
}

function main() {
    greet("GSET")
    print(add(5, 3))
    print(factorial(5))
}

main()
```

## Example: List Comprehensions

```gset
function main() {
    nums = [1, 2, 3, 4, 5]
    squared = [x * x for x in nums]
    print(squared)
    
    evens = [x for x in nums if x % 2 == 0]
    print(evens)
}

main()
```

## Output Formats

GSET supports multiple output formats:

```bash
# Run directly (default)
gset run file.gset

# Show transpiled output only
gset transpile file.gset

# Show version
gset version

# Show help
gset help
```
