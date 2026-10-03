//! Target-language backends.
//!
//! A backend turns a [`gset_ir`] module into runnable source for one target
//! language. This crate is the direct replacement for `transpiler/emit.go`, and
//! its shape is the main architectural change of the rewrite.
//!
//! # Why a trait instead of a `switch`
//!
//! The Go emitter was one `emitter` struct with a `target` field and **42**
//! `switch e.target` dispatch points spread across the file. Consequences that
//! were observed directly in the output:
//!
//! - `printBuiltin` per-target mappings existed, but `&&`/`||` had no Ruby
//!   mapping, so `a && b` emitted Ruby source containing `&&`: a syntax error.
//! - `??` had no Go case and fell through to a default that emitted `??` into
//!   Go, which has no such operator.
//! - Python list-comprehension syntax was emitted into Go verbatim.
//! - `[]interface{}{...}` was emitted into Java, which has no such type.
//! - Java got `for (var i = 0;; i ++;)`, with doubled semicolons, because the
//!   init and update expressions already appended their own terminators.
//! - JavaScript block-lambdas emitted a literal backslash-`n` from a Go string
//!   escape.
//! - Go's `%q` and `%g` formatted strings and floats for all five targets.
//!
//! Each of those is one forgotten `case`. Expressing targets as a trait means
//! the compiler enumerates them: add a variant and every backend fails to
//! compile until it handles that construct.
//!
//! # Capability negotiation
//!
//! A backend declares what it can do with [`Backend::capability`] *before*
//! emitting. The driver consults it per construct, so an unsupported construct
//! is a typed error or an explicit `// gset(unsupported):` marker rather than a
//! silent omission. In Go, six statement types (`enum`, `struct`, `trait`,
//! `interface`, type alias, `export`) vanished from output with no warning,
//! because `statement()` had no case and returned `nil`.
//!
//! # Indentation
//!
//! [`CodeWriter`] owns indentation. Go hardcoded `indentUnit = "    "` for every
//! target including Go itself, which guaranteed generated Go failed `gofmt`.
//! Per-target indentation belongs to the backend's configuration.

#![forbid(unsafe_code)]
