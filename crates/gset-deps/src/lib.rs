//! Dependency resolution.
//!
//! "Dependencies put in other places" needs this to be a subsystem rather than
//! a config lookup. It owns manifest and lockfile parsing per ecosystem, the
//! module graph they imply, and the classification of each module by what can
//! actually be done with it.
//!
//! # Ecosystems
//!
//! pip (`requirements.txt`, `pyproject.toml`), npm (`package.json`,
//! `package-lock.json`), Cargo (`Cargo.toml`, `Cargo.lock`), Maven (`pom.xml`)
//! and Go modules (`go.mod`, `go.sum`).
//!
//! # Lockfile first
//!
//! A lockfile is an already-solved graph. Parsing one is tractable and gives an
//! exact answer. A manifest without a lockfile is an unsolved constraint set,
//! and implementing a full version solver is explicitly out of scope. Manifest
//! resolution is therefore best-effort and reports an "unpinned" diagnostic
//! rather than pretending to have solved it.
//!
//! # What "resolved" honestly means here
//!
//! Not every dependency can be translated, and the model says so instead of
//! guessing. Each module resolves to one of:
//!
//! - `LocalPath` — source on disk inside the project. Transpilable.
//! - `LockedRemote` — pinned by a lockfile to a version. Transpilable if its
//!   source is available locally, otherwise opaque.
//! - `Sdk` — a standard library or toolchain-provided module. Not transpilable;
//!   it belongs to the target's own runtime.
//! - `OpaqueForeign` — a compiled artefact such as a wheel, a jar or a native
//!   addon. There is no source to translate.
//!
//! For `OpaqueForeign`, the IR keeps the import and the backend emits a
//! passthrough import plus a diagnostic naming the target-side package manager
//! that must supply it. That is the honest ceiling for "dependencies in other
//! places": graph-aware resolution and correct import emission, **not**
//! source-level inlining of a binary. Pretending otherwise would produce code
//! that looks complete and is not.

#![forbid(unsafe_code)]
