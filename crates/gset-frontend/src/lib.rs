//! Source-language frontends.
//!
//! A frontend turns source text for one language into a [`gset_ir`] module.
//! Frontends are the reason this rewrite is not a port: the Go implementation
//! had exactly one hand-written frontend and could not gain another without
//! rewriting its emitter.
//!
//! Real languages are parsed with [tree-sitter](https://tree-sitter.github.io).
//! Each frontend is a grammar dependency plus a lowering pass that walks the
//! concrete syntax tree and emits IR. Grammars are isolated behind one Cargo
//! feature per language because each one is generated C and costs seconds to
//! compile; a build that does not enable a language never compiles its grammar.
//!
//! Lowering uses tree-sitter *queries* to pull out semantic landmarks (function
//! and class declarations, imports, the statement forms) and hand-written
//! walkers for the bodies. This keeps the per-language code small and keeps it
//! resilient to grammar node renames, which happen on every grammar upgrade.
//!
//! Adding a language must not require touching any backend. That property is
//! the whole point of routing every frontend through the shared IR.

// `deny` rather than `forbid` so the tree-sitter grammar modules can carry a
// narrow `#[allow(unsafe_code)]`. The grammars are generated C and are the only
// unavoidable unsafe in the workspace; confining the exception to those modules
// keeps it from leaking into the IR or the backends.
#![deny(unsafe_code)]

use gset_ir::{LangId, Lowered};

pub mod limits;

#[cfg(feature = "python")]
pub mod python;

/// Lowers one source language into the IR.
///
/// Implemented per language. The trait exists so `gset-cli` can select a
/// frontend from a file extension without matching on concrete types, which is
/// what keeps adding a language from touching the CLI.
pub trait Frontend {
    /// The language this frontend parses.
    fn lang(&self) -> LangId;

    /// The language's name, as it appears on the command line.
    fn name(&self) -> &'static str;

    /// Whether the frontend can handle `path`'s extension.
    fn handles_extension(&self, path: &std::path::Path) -> bool;

    /// Parses `text` and lowers it into a module.
    ///
    /// `source` names the input for diagnostics. Returning a [`Lowered`] rather
    /// than a `Result` keeps the recovery path explicit: a frontend that hits
    /// one unsupported construct should report it and keep going, and
    /// [`Lowered::module`] is `None` only when nothing safe can be emitted.
    fn lower(&self, source: &str, text: &str) -> Lowered;
}

/// Every frontend compiled into this build.
///
/// Defined twice rather than built from a `cfg`-gated `vec!` because attributes
/// on expressions are still unstable, and a single `vec!` cannot be assembled
/// from conditionally-compiled elements.
#[cfg(feature = "python")]
pub fn frontends() -> Vec<Box<dyn Frontend>> {
    vec![Box::new(python::Python) as Box<dyn Frontend>]
}

/// Every frontend compiled into this build, which with no language feature
/// enabled is none at all.
#[cfg(not(feature = "python"))]
pub fn frontends() -> Vec<Box<dyn Frontend>> {
    Vec::new()
}

/// Finds the frontend for a path by extension.
///
/// Returns `None` rather than falling back to a default. The Go implementation
/// fell back to Go for every unrecognised extension, so transpiling a `.rs` or
/// `.php` file silently emitted Go with no warning.
pub fn frontend_for_path(path: &std::path::Path) -> Option<Box<dyn Frontend>> {
    frontends()
        .into_iter()
        .find(|frontend| frontend.handles_extension(path))
}

/// Finds a frontend by its command-line name.
pub fn frontend_by_name(name: &str) -> Option<Box<dyn Frontend>> {
    let wanted = name.to_ascii_lowercase();
    frontends()
        .into_iter()
        .find(|frontend| frontend.name() == wanted)
}
