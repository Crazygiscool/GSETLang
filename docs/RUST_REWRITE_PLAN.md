# Rust Rewrite Plan

Status: **approved, in progress** — branch `feat/rust-rewrite`
Decision date: 2026-10-02

GSET is being rewritten from Go to Rust, and simultaneously redesigned from a
"GSET-toplevelanguage transpiler" into a general **any-language → any-language**
compiler.

This document is the source of truth for the rewrite. It supersedes the Go-era
`AGENTS.md` architecture notes, which are kept only until M6 deletes the Go tree.

---

## 1. Why this is a rewrite, not a port

The Go implementation was 7,137 lines across 16 files. An audit found that
roughly 1,500 lines were dead code and that the surviving code was structurally
incapable of supporting N languages in × M languages out:

- `transpiler/emit.go` was a single emitter struct with **42 `switch e.target`
  dispatch points and no per-target abstraction**. Adding a sixth language
  meant 43 hand edits in scattered functions.
- **Dead code:** the parallel `Translate`/`translate*` path (~400 lines) was
  reachable only from benchmarks; `BuildFile` had zero callers repo-wide;
  15 of 67 AST node types were never constructed; 39 of 92 keyword types were
  never referenced by the parser.
- **`parser.go:651` discarded the entire class body** (`_ = p.parseBlock()`), so
  every class always emitted as an empty shell.
- **Invalid output was routine:** Python list-comprehension syntax emitted into
  Go, `??` emitted into Go, `[]interface{}{}` emitted into Java, `&&`/`||`
  emitted into Ruby (a Ruby syntax error), `var x = 0;;` into Java, a literal
  backslash-`n` in JavaScript lambdas, and Go's `%q`/`%g` string and float
  formatting applied to all five targets.
- **Near-zero tests:** the lexer had three benchmarks and zero assertions;
  `transpiler_test.go` contained only benchmarks; all 14 integration tests
  exercised only the Go backend.
- **Reachable panics:** an unterminated `"` reversed a slice bounds
  (`lexer.go:373`); comments were skipped by recursion (`lexer.go:125`), giving a
  stack-overflow path on large inputs.
- **Configuration was ~95% inert.** The keyword map was consulted in exactly two
  places. Four of eight `security` functions were dead, and two had their results
  discarded.

Porting this line-by-line would inherit every defect while still encoding the
design that cannot scale.

---

## 2. Decisions

| # | Decision | Rationale |
|---|---|---|
| 1 | **tree-sitter frontends + shared typed IR** | Industry-standard convergence point for polyglot source-to-source work. Each new language is a grammar dependency plus a lowering pass, not a new compiler. |
| 2 | **Local + ecosystem dependency resolution** | "Dependencies in other places" requires a first-class resolution subsystem, not a config lookup. |
| 3 | **Hard cutover, preserve CLI + release surface** | The Go tree is deleted in one commit at M6. Binary name `gset` and release asset names are preserved so the existing one-liner install keeps working. |
| 4 | **Optimise for CLI startup + single-file latency** | Per-invocation cost dominates a transpiler CLI. |
| 5 | **Retire the GSET language** | It becomes vestigial once real-language frontends exist. All effort goes to Python/JS/TS/Go/Java. |
| 6 | **All 5 backends working at M2** | Go, Python, JavaScript, Java, Ruby — a true like-for-like replacement. |
| 7 | **No backwards compatibility for the transpiler** | `gset.conf` and its keyword system are deleted outright, replaced by `gset.toml`. No output parity with Go is required. |
| 8 | **Many small commits** | ~250 commits, each independently buildable and revertible. The history is the audit trail. |

### What "no backwards compatibility" concretely removes

- No need for **output parity** with Go — only bug avoidance. The Go output is
  recorded as *negative* fixtures, not golden files.
- `gset.conf` INI parsing is not ported. Config becomes `gset.toml` and keeps
  only what has real use: default target, interpreter paths, project root,
  dependency search roots.
- The keyword-alias machinery (`callExpr` → `kw` lookup, `exprUsesFmt`) is not
  ported at all. Builtins are first-class in each backend.

### What remains as compatibility surface

Only the thing decision 3 preserved: the `gset` binary name, and release asset
names of the form `gset-$OS-$ARCH.tar.gz` — the exact string `install.sh`
fetches. Changing it breaks every existing user's installer.

### Known trade-off from retiring GSET

We lose the *lossless round-trip anchor*. `IR → GSET → IR` proved the IR was
complete. `IR → Python → IR` cannot be structural. Verification is replaced by
three mechanisms:

1. Golden files per frontend × backend pair.
2. The target-syntax-check harness (§5, M2 gate).
3. A **semantic** round-trip property test: emit Python, re-parse it, assert the
   re-parsed IR matches the original under normalisation.

Weaker than structural round-trip, still strong, and it costs one property-test
file.

---

## 3. Architecture

```
GSETLang/
├── Cargo.toml                  # workspace; lto="fat", panic="abort"
├── rust-toolchain.toml         # pinned
├── crates/
│   ├── gset-ir/                # typed IR, spans, diagnostics, lowering traits
│   ├── gset-frontend/          # Frontend trait + registry
│   │   ├── python/  javascript/  typescript/  go/  java/  ruby/  rust/  c/
│   ├── gset-backend/           # Backend trait + CodeWriter + capability matrix
│   │   └── go/  python/  javascript/  java/  ruby/
│   ├── gset-semantic/          # symbols, type inference, import graph
│   ├── gset-deps/              # manifests, lockfiles, resolution
│   └── gset-cli/               # bin "gset": transpile/run/version/help/resolve
├── grammars/                   # tree-sitter grammar sources, if any are vendored
├── tests/
│   ├── corpus/<lang>/          # real-language input sources
│   └── golden/<case>.<target>  # expected output
├── docs/                       # engineering plans (this file). Not the docs site.
└── scripts/ packages/ manifests/ .github/workflows/
```

Crate granularity is load-bearing, not cosmetic: each tree-sitter grammar is
heavy C, so isolating them means `cargo build -p gset-cli` never recompiles
grammars.

### The IR

Typed, span-annotated, module-aware. Every node carries a source span.

```rust
struct Module { items: Vec<Item>, imports: Vec<Import>, lang: LangId, span: Span }
enum Item { Fn(FnDecl), Class(ClassDecl), Record(..), Enum(..), Global(VarDecl), .. }

struct Expr { kind: ExprKind, ty: Type, span: Span }
enum Type { Prim, Named(Symbol), Array(Box<Type>), Map(..), Set(..),
            Optional(Box<Type>), Fn(..), Union(..), Unknown }
```

Three design decisions specifically kill bug classes found in the audit:

**1. Imports are IR items and are mandatory to consume.** In Go,
`ast.Program.Imports` was written by the parser and read by nothing, so no
language ever emitted an import from a top-level `import`. In Rust, `Import` is
an `Item`, and `emit_module` is required to consume it.

**2. A capability matrix replaces silent `return nil`.** Backends declare
support *before* emission:

```rust
enum Support { Native, Desugared, Unsupported }
trait Backend {
    fn id(&self) -> TargetId;
    fn capability(&self, c: Capability) -> Support;
    fn emit_module(&self, m: &Module, w: &mut CodeWriter) -> Result<(), Diagnostic>;
}
```

The driver queries `capability()` per construct before emitting. Unsupported
becomes a typed error or an explicit `// gset(unsupported):` marker — never a
silent drop. In Go, six statement types (`enum`, `struct`, `trait`, `interface`,
type alias, `export`) vanished from output with no warning.

**3. `CodeWriter` owns indentation.** Go hardcoded `indentUnit = "    "` for all
five languages, which guaranteed every generated `.go` file failed `gofmt`.

---

## 4. Milestones

Go is **frozen and in-tree** as a read-only oracle until M6. Every Rust commit
is purely additive until the deletion commit.

### M0 — Baseline & scaffolding

- `rust-toolchain.toml` pinned to 1.98.1; workspace skeleton; dependency set.
- Go startup baseline: **captured**. See below.
- Go output fixtures: **captured** as negative fixtures in `tests/baseline/`,
  one entry per defect class, with the M2 syntax matrix run against them.
- CI: `cargo fmt`, `cargo clippy`, `cargo test`, `cargo-deny`, cross-platform
  build matrix. `cargo-audit` is **not** included: `cargo deny check advisories`
  reads the same RustSec advisory database, so a second tool would duplicate the
  job and add a second install to every run. Adopt `cargo-audit` only if
  `cargo-deny`'s advisory coverage is ever found to be insufficient.

**Status: complete.**

#### Startup baseline

Measured with Go 1.21.13 (matching CI, via `GOTOOLCHAIN`) against Rust 1.98.1,
release builds, 200 interleaved invocations of `gset version`, so machine drift
hits both equally:

| | median | p90 | min |
|---|---|---|---|
| Go 1.21.13 | 3.76 ms | 5.74 ms | 2.25 ms |
| Rust 1.98.1 (scaffold, no grammar loaded) | 2.07 ms | 3.35 ms | 1.45 ms |

Rust is **45 % faster** already, on a CLI that does not yet parse anything. The
gap should widen: Go links and initialises the whole emitter, and Rust will not
load a grammar until a language is known.

The Go figure also depends on the working directory, because `LoadConfig("")`
resolves against `os.Getwd()` and runs before argument parsing:

| `gset.conf` in cwd | Go median | delta |
|---|---|---|
| none | 4.15 ms | — |
| 256 KB | 12.97 ms | +8.8 ms |
| 1 MB | 42.14 ms | +38.0 ms |

So a large config is a per-invocation tax paid even by `gset version`, scaling
linearly with file size. The repo's own 9 KB `gset.conf` is within noise, which
is why an earlier measurement understated this.

#### Defect fixtures

`tests/baseline/` holds ten minimal `.gset` inputs, the Go implementation's real
output for all five targets, and a checklist of 15 defect classes. Running the M2
syntax matrix against that output is the "before" column:

| Target | Check | Pass | Fail |
|---|---|---|---|
| go | `gofmt -e` | 6 | 4 |
| python | `python3 -m py_compile` | 9 | 1 |
| javascript | `node --check` | 8 | 2 |
| ruby | `ruby -c` | 9 | 1 |
| java | `javac` | 0 | 10 |
| **total** | | **32** | **18** |

These figures are parse-and-compile only. Most of the checklist produces *valid*
syntax that means the wrong thing — a dropped `if` that still compiles, an `enum`
that vanishes without a warning, `??` quietly becoming `||` in Ruby — so the
checklist is what the rewrite owes against, and the matrix is only the part that
happens to be automatable.

### M1 — Pipeline proof: Python → IR → Go

The first end-to-end. IR first (types, diagnostics, expressions, statements,
items, lowering builder), then a Python frontend (grammar + query-driven
lowering), then the Go backend, then the CLI.

A fresh `tests/corpus/py/` set of 15–20 varied Python files is written alongside
M1, covering the constructs the audit says the Go emitter handled. Without it the
syntax-check gate has nothing to run.

**Gate:** `gset transpile fib.py --to go` emits Go that passes `gofmt` and
`go vet`. Startup within budget of the Go baseline.

Python first specifically because it is dynamic — it forces type inference and
therefore exercises the hardest part of the IR early, instead of hiding behind
Go's explicit types.

### M2 — All 5 backends + second frontend

- Second frontend: JavaScript/TypeScript (shares an IR lowering path).
- All 5 backends over the common subset.

**The M2 acceptance criterion mechanises the elimination of the entire bug class
the audit found:**

> For every corpus case, generated output must pass the target language's own
> syntax checker — `gofmt`/`go vet`, `python -m py_compile`, `node --check`,
> `javac`, `ruby -c` — gated on toolchain presence in CI.

`&&` into Ruby, `??` into Go, `[]interface{}{}` into Java, `var x = 0;;` into
Java: none of it can survive this gate.

- `gset-semantic`: symbol tables, scope resolution, type inference with
  `Unknown` propagation.
- Property test: `IR → python → IR` semantic equivalence.

### M3 — Frontend breadth

Go, Java, Ruby, then C/C++ and Rust. Each is a grammar dependency plus a lowering
pass.

### M4 — Local project graph

Multi-file and multi-directory projects, `--dep-path` search roots,
package-qualified imports, workspace roots, cycle detection, parallel lowering
via `rayon`, content-hash caching.

### M5 — Ecosystem dependency resolution

pip, npm, Cargo, Maven, Go modules.

- **Lockfile-first.** Lockfiles are already solved, so parsing them is tractable.
  Manifest-only resolution is best-effort with an explicit "unpinned" diagnostic.
  A full version solver is out of scope and not recommended.
- **Honest boundary.** Not every dependency is translatable: a compiled wheel has
  no source. `ResolvedModule` distinguishes
  `LocalPath | LockedRemote | Sdk | OpaqueForeign`. Opaque dependencies keep their
  import and gain a diagnostic naming the target-side package manager that must
  supply them. This is what "dependencies in other places" can honestly mean —
  graph-aware resolution and correct import emission, **not** source-level
  inlining of binaries.

### M6 — Cutover

1. Rewrite CI (`test.yml`, `release.yml`) for Rust.
2. Preserve release asset names exactly as `gset-$OS-$ARCH.tar.gz` / `.zip`.
3. Rewrite packaging: `install.sh`, `install.ps1`, `installer.nsi`,
   `packages/windows/*`, `packages/{aur,arch,choco,debian}`, `manifests/*`,
   `scripts/*`.
4. `Makefile`: keep `VERSION :=` — `build.sh` greps for it. Version appears in
   ~10 files; a bump must touch all.
5. Rewrite README, `AGENTS.md`, and the `GSETLang-Docs` submodule content.
6. **Delete the Go tree last, as a standalone commit.**

Two pre-existing release bugs get fixed here, both already recorded in
`AGENTS.md`: root `build.sh` produces `gset-$VERSION-$OS-$ARCH.tar.gz` while CI
uploads the unversioned name, and the winget manifest points at a
`gset-setup.exe` that CI never publishes.

---

## 5. Verification strategy

The Go version had almost no tests. The Rust version's:

- **Golden files** per frontend × backend pair.
- **Target-syntax-check harness** (§4, M2 gate) — the single highest-value
  mechanism, because it converts "did we emit valid output" from a judgement call
  into an automated check.
- **`cargo-fuzz`** for lowering and lexing. The Go lexer had a reachable panic
  (`lexer.go:373`) and a comment-recursion stack overflow (`lexer.go:125`); both
  become fuzz targets.
- **Property tests** for `IR → python → IR` semantic equivalence.
- Model the table-driven style of `security_test.go` — the only well-written test
  file in the Go repo, and it ports near-mechanically.
- **Benchmarks as gates** (`criterion`): any change regressing startup or
  throughput fails the build.

---

## 6. Performance

Target: match the recorded Go startup baseline.

- `OnceLock` for registries; arguments parsed before any grammar is touched.
- `lto = "fat"`, `panic = "abort"`, strip.
- **Startup vs. grammar-linking is the central tension.** Bundling 25 grammars
  statically inflates the binary and relocation cost. Mitigation is feature-gated
  grammar sets (`--no-default-features --features grammars-python,grammars-go`),
  with the default set chosen by measurement rather than intuition.

---

## 7. Risks

| Risk | Mitigation |
|---|---|
| Type inference is the real cost; wrong inference yields confidently-wrong code | `Unknown` propagates explicitly; backends pick a conservative strategy; golden tests gate every construct |
| tree-sitter CLI is a new build dependency | Needed from M1 onward; CI provisions it once |
| Startup vs. grammar-linking conflict | Feature-gated grammar sets; benchmark-gated decision |
| Loss of the lossless round-trip anchor | Semantic round-trip property test + syntax-check harness |
| M3–M5 are each multi-month | M1 and M2 are independently shippable |
| Eclipse the existing install base at cutover | Asset names and package manifests preserved (decision 3) |

---

## 8. Non-goals

- A full semantic version solver.
- Source-level inlining of compiled/binary dependencies.
- Semantic preservation across all language pairs.
- LSP or IDE integration.
- Keeping the GSET language.

---

## 9. Commit conventions

Branch: `feat/rust-rewrite`. Conventional commits, lowercase imperative subject.

Rules:

- One commit = one reviewable change. Every commit builds and passes
  `cargo test` + `cargo clippy`.
- Vertical slices: `feat(frontend/python): lower while loops`, not
  `feat(frontend/python): part 1 of 7`.
- Each bug fixed from the Go audit lands with a `test:` commit that would have
  caught it, written first.
- Go stays frozen and in-tree until M6, so all Rust commits are additive until
  the deletion commit.

Roughly 250 commits: M0 ~10, M1 ~45, M2 ~60, M3 ~20 per language, M4 ~30,
M5 ~30, M6 ~30.