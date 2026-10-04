//! The `gset` transpilation pipeline.
//!
//! Kept out of `main.rs` so it can be exercised directly by tests without
//! spawning the binary. The pipeline is deliberately small: choose a frontend
//! from the source, lower to the IR, choose a backend from the target, emit.
//! `gset-semantic` slots in between once it exists, and nothing before or after
//! it needs to change.

use std::path::Path;

use gset_backend::{backend_by_name, emit};
use gset_frontend::{frontend_by_name, frontend_for_path};
use gset_ir::{DiagnosticBag, SourceMap};

/// The result of one transpilation.
#[derive(Debug)]
pub struct Output {
    /// The generated source. Empty when lowering failed before a module existed.
    pub text: String,
    /// Everything reported by the frontend and the backend, combined.
    pub diagnostics: DiagnosticBag,
    /// The map every diagnostic's span indexes into.
    pub source_map: SourceMap,
    /// Whether anything was an error, so the CLI can choose an exit code.
    pub failed: bool,
}

/// Why transpilation could not start.
#[derive(Debug, thiserror::Error)]
pub enum PipelineError {
    /// No frontend handles the source language.
    #[error("no frontend for source `{0}`; supported: python")]
    UnknownFrontend(String),
    /// No backend emits the target language.
    #[error("no backend for target `{0}`; supported: go")]
    UnknownBackend(String),
}

/// Transpiles `text` to `target`.
///
/// `from` names the source language; when absent it is inferred from `name`'s
/// extension.
pub fn transpile(
    name: &str,
    text: &str,
    from: Option<&str>,
    target: &str,
) -> Result<Output, PipelineError> {
    let frontend = match from {
        Some(language) => frontend_by_name(language)
            .ok_or_else(|| PipelineError::UnknownFrontend(language.to_string()))?,
        None => frontend_for_path(Path::new(name))
            .ok_or_else(|| PipelineError::UnknownFrontend(name.to_string()))?,
    };

    let lowered = frontend.lower(name, text);
    let source_map = lowered.source_map;
    let mut diagnostics = lowered.diagnostics;

    let Some(module) = lowered.module else {
        return Ok(Output {
            text: String::new(),
            diagnostics,
            source_map,
            failed: true,
        });
    };

    let backend =
        backend_by_name(target).ok_or_else(|| PipelineError::UnknownBackend(target.to_string()))?;
    let emitted = emit(backend.as_ref(), &module, &source_map);
    let failed = emitted.has_errors();
    diagnostics.extend(emitted.diagnostics);

    Ok(Output {
        text: emitted.text,
        diagnostics,
        source_map,
        failed,
    })
}
