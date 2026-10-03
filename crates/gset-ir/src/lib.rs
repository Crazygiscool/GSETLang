//! The language-neutral typed intermediate representation.
//!
//! Every frontend lowers its own concrete syntax tree into this IR, and every
//! backend emits target source from it. Nothing in this crate may assume a
//! particular source or target language. It sits at the bottom of the workspace
//! dependency graph and depends on no other crate here.
//!
//! The Go implementation this replaces had no intermediate representation at
//! all: `transpiler/emit.go` was a single emitter struct with 42
//! `switch e.target` dispatch points walking the source AST directly. That made
//! each new language a 43-edit manual change, and let construct-specific bugs
//! reach user-facing output silently — a Python list comprehension emitted into
//! Go, `&&` emitted into Ruby (a syntax error), `??` emitted into Go.
//!
//! Three invariants are load-bearing:
//!
//! 1. **Every node carries a `Span`.** Without source positions there is no way
//!    to report a diagnostic that points at the code that caused it. The Go
//!    emitter assembled output from `strings.Join` with no error channel at all.
//! 2. **Imports are items.** In Go, `ast.Program.Imports` was written by the
//!    parser and read by no emitter, so no language ever emitted an import from
//!    a top-level `import`. Here, `Import` is an `Item`, and backends are
//!    required to consume it.
//! 3. **Types are explicit, and `Unknown` is contagious.** A frontend that
//!    cannot determine a type records `Type::Unknown` rather than guessing.
//!    Guessing produces confidently-wrong code, which is strictly worse than
//!    emitting a diagnostic.
//!
//! Construct support is declared rather than assumed: backends answer
//! `gset_backend::Backend::capability` per construct before emission, so an
//! unsupported construct becomes a typed error instead of a silent `return nil`.
//! Six statement types (`enum`, `struct`, `trait`, `interface`, type alias and
//! `export`) disappeared from Go output with no warning at all.

#![forbid(unsafe_code)]
