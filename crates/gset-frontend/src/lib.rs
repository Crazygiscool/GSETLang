//! Source-language frontends.
//!
//! A frontend turns source text for one language into a [`gset_ir`] module.
//! Frontends are the reason this rewrite is not a port: the Go implementation
//! had exactly one hand-written frontend and could not gain another without
//! rewriting its emitter.
//!
//! Real languages are parsed with [tree-sitter](https://tree-sitter.github.io).
//! Each frontend is a grammar dependency plus a lowering pass that walks the
//! concrete syntax tree and emits IR. Grammars are isolated per language
//! because each one is heavy C, and the goal is that `cargo build -p gset-cli`
//! never recompiles any of them.
//!
//! Lowering should use tree-sitter *queries* to pull out semantic landmarks
//! (function and class declarations, imports, the statement forms) and
//! hand-written walkers for the bodies. This keeps the per-language code small
//! and keeps it resilient to grammar node renames, which happen on every
//! grammar upgrade.
//!
//! Adding a language must not require touching any backend. That property is
//! the whole point of routing every frontend through the shared IR.

// `deny` rather than `forbid` so the tree-sitter grammar modules can carry a
// narrow `#[allow(unsafe_code)]`. The grammars are generated C and are the only
// unavoidable unsafe in the workspace; confining the exception to those modules
// keeps it from leaking into the IR or the backends.
#![deny(unsafe_code)]
