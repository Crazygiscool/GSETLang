# Go implementation baseline (negative fixtures)

**Nothing in `outputs/` is a target. It is a record of what the Go implementation
actually did, kept so the rewrite does not reproduce it.**

The Go implementation in this repo is about to be deleted (milestone 6). A defect
that is only described in prose is a defect that quietly comes back, so every
defect found during the audit has a small input under `inputs/` and the Go
implementation's real output for all five targets under `outputs/`.

Regenerate with:

```bash
go build -o gset .
./scripts/capture-go-baseline.sh
```

Review the diff before committing. After the fixtures are first recorded the Go
implementation should never change again.

## How these are used

These files are **not** asserted against. A test that required `outputs/` to stay
byte-identical would be asserting that the bugs persist.

Instead each defect below is a line in a checklist. The M2 acceptance criterion
is mechanical: for every case in `tests/corpus/`, generated output must pass the
target language's own syntax checker. Running that matrix against `outputs/`
today is the "before" column, and it is the number the rewrite has to beat.

| Target | Check | Pass | Fail |
|---|---|---|---|
| go | `gofmt -e` (parse) | 6 | 4 |
| python | `python3 -m py_compile` | 9 | 1 |
| javascript | `node --check` | 8 | 2 |
| ruby | `ruby -c` | 9 | 1 |
| java | `javac` | 0 | 10 |
| **total** | | **32** | **18** |

Java fails on every input. Note these figures are *parse and compile* only. The
defects in the next section are mostly invisible to these checkers, because they
produce valid syntax that means the wrong thing — which is why the checklist
matters as much as the matrix.

## Defect classes

### Silently dropped constructs

**1. Only the first `if` in a module is emitted** — `inputs/two_ifs.gset`, all targets.

```gset
if 1 > 0 { print("one") }
if 2 > 0 { print("two") }
```

The second `if` loses its header and its condition survives as a bare expression,
while its body still prints. Go output:

```go
if (1 > 0) {
    fmt.Println("one")
}
2 > 0
fmt.Println("two")
```

Valid Go, wrong program. No diagnostic is produced. This is the most damaging
class in the corpus because the output looks correct.

**2. `struct` and `enum` produce nothing at all** — `inputs/declarations.gset`, all five targets.

A `struct Point`, an `enum Color` and a `class Widget` in one input produce
output containing only the `class`. The struct and the enum are gone in Go,
Python, JavaScript, Java and Ruby, with no warning and no error.

### Invalid syntax in the output

**3. `??` has no Go mapping** — `inputs/null_coalescing.gset` → go.

`x = 1 ?? 2` is emitted verbatim. Go has no `??` operator, so the file does not
parse: `illegal character U+003F '?'`.

**4. `??` silently becomes `||` in Ruby** — same input → ruby.

`x = 1 || 2`. Syntactically valid, semantically unrelated: `||` is boolean or,
`??` is "use the left unless it is null". A translation that compiles and
disagrees with the source.

**5. Python comprehension syntax emitted into Go** — `inputs/comprehension.gset` → go.

```go
squares = [n * n for n in nums]
```

Does not parse: `expected ']', found 'for'`. The IR has no comprehension node at
all, so the source text passed through.

**6. Literal `\n` in JavaScript block lambdas** — `inputs/lambda.gset` → javascript.

```javascript
f = (x) => {\n    return x * 2\n}
```

The escape is two characters, not a newline, so the whole body is one line: the
`return` is followed by the literal text `n}` on the same line. Reachable code
after it is swallowed.

**7. Java `for` headers have doubled semicolons** — `inputs/c_style_for.gset` → java.

```java
for (i = 0;; i < 3; i = i + 1;) {
```

Four separators for three header clauses, and the condition is duplicated into
the increment slot.

### Undeclared names and missing declarations

**8. `interface{}` emitted into Go** — `inputs/list_literal.gset` → go.

`xs = []interface{}{1, 2}`: `xs` is never declared, and `interface{}` is a Go
interface not a list type.

**9. Missing `import "fmt"`** — `inputs/c_style_for.gset` → go.

The output calls `fmt.Println` with no import. `import "fmt"` is decided by a
hardcoded `exprUsesFmt` string search over the AST, so it is missed whenever the
call shape differs from what that search expects. This file *parses* and fails to
compile.

**10. Integer literals saturate at int64 max** — `inputs/numeric_literals.gset`, all targets.

`big = 12345678901234567890` becomes `9223372036854775807`. The value is parsed
into an `int64` and the overflow is discarded with no diagnostic. The same
fixture shows every numeric literal routed through Go format verbs, which is how
large integers and floats both get mangled.

**11. A declaration after an `if` has its name replaced by a null literal** —
`inputs/declaration_after_if.gset`, all targets.

```gset
if 1 > 0 { print("one") }
x = 1 ?? 2
print(x)
```

Go gets `nil = 1 ?? 2`, Java and JavaScript get `null = ...`, and the following
`print(x)` then references a variable that was never declared at all.

**12. Untyped parameters in Go** — `inputs/return_in_branch.gset` → go.

`func pick(n) {` — a Go parameter requires a type.

**13. `void` method that returns a value** — same input → java.

`static void pick(Object n)` containing `return 1;`. A compile error: a void
method cannot return a value. Every Java parameter and return type is `Object`
regardless of the source, and the method is `void` whenever the only `return`
sits inside an `if`.

**14. A function swallows the rest of the module** — same input, all targets.

The trailing `print(pick(5))` is emitted *inside* `pick()`, and `main()` is left
empty. Statement ownership is not tracked, so the first top-level statement after
a function definition is absorbed into that function.

### Formatting

**15. No trailing newline in any output** — all 50 files.

Every generated file ends without a newline. `gofmt -l` flags every Go fixture
on indentation as well (4 spaces, where Go uses tabs), so generated Go has never
passed `gofmt` and cannot have been checked in CI.

## Corrections to the original audit

Two claims from the audit did **not** reproduce, and are recorded here so they are
not carried forward:

- **`&&` and `||` are valid Ruby.** Ruby supports both operators at the same
  precedence as the C family, so emitting `&&` into Ruby is not the syntax error
  the audit recorded. The real Ruby defects are classes 1 and 4 above.
- **`[]interface{}{...}` was recorded as a Java problem.** It reaches Go too, and
  is class 8 above.

## What the rewrite owes

For each class, the corresponding IR or pipeline decision that prevents it:

| Class | Prevented by |
|---|---|
| 1, 2 | Frontends lower every statement to an `Item`/`Stmt` node; the M2 matrix plus a "nothing dropped" assertion catches silent omission |
| 3, 4, 6, 7 | `Backend::capability` is checked per construct before emission, so a missing operator is a typed error rather than a forgotten `case` |
| 5 | `ExprKind::Comprehension` exists, so the source syntax has no path to the emitter |
| 8, 9 | Imports are `Item`s that backends must consume; `Expr` carries types, so list literals need no Go-specific spelling |
| 10 | Literals are kept as text in `Literal`, never parsed into a machine integer |
| 11, 13, 14 | `Type::Unknown` propagates instead of guessing; scope resolution owns statement ownership |
| 12 | Every `Expr` carries a type, so a parameter cannot be emitted untyped |
| 15 | The backend owns indentation per target and terminates its output |

Classes 3, 5, 7 and 13 are the ones the syntax matrix catches on its own.
Classes 1, 2, 4, 10, 11 and 14 produce valid code that is wrong, which is why
`outputs/` is kept as a checklist rather than only as a test corpus.
