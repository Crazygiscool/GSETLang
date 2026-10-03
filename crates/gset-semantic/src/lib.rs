//! Semantic analysis: symbols, types, and import graphs.
//!
//! This crate owns the passes that turn a set of lowered modules into something
//! a backend can emit correctly. Three responsibilities:
//!
//! - **Symbol tables and scope resolution.** Bindings to modules, and local
//!   bindings to scopes. Go emitted every identifier verbatim, so a shadowed
//!   local and a package-level function of the same name were indistinguishable
//!   at emit time.
//! - **Type inference.** Python and JavaScript are dynamically typed, so a
//!   frontend cannot record types directly and the backend cannot choose an
//!   output type without knowing what it is holding. Inference reconstructs
//!   types from how values are used.
//! - **Import graphs.** Which module imports which, whether that graph has
//!   cycles, and which modules are reachable. This is the foundation for the
//!   project-level and dependency work in M4 and M5.
//!
//! # Inference must not guess
//!
//! When inference cannot determine a type it records `Type::Unknown` and the
//! uncertainty propagates. Backends respond to `Unknown` with a documented
//! conservative strategy and may attach a diagnostic. This is deliberate:
//! Python-to-Go is largely a guessing game, and a wrong guess produces code
//! that compiles and then misbehaves at runtime, which is far worse than a
//! diagnostic. The Go implementation had no inference at all and simply emitted
//! `Object` for every Java parameter and return type, including a `void`
//! return for any function whose only `return` sat inside an `if`.
//!
//! # Parallelism
//!
//! Independent modules are inferred in parallel via `rayon`. This exists so that
//! transpiling a project is bounded by the largest module rather than the sum
//! of all of them.

#![forbid(unsafe_code)]
