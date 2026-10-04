//! The [`Backend`] trait, the emit driver, and the registry.

use gset_ir::{DiagnosticBag, Module, SourceMap, Span};

use crate::go::Go;
use crate::target::{Capability, Support, TargetId};
use crate::writer::CodeWriter;

/// The result of running a backend over a module.
#[derive(Clone, Debug)]
pub struct Emitted {
    /// The generated source text.
    pub text: String,
    /// Everything the backend could not express, or chose to warn about.
    pub diagnostics: DiagnosticBag,
}

impl Emitted {
    /// Whether any error-severity diagnostic was produced.
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity.is_error())
    }
}

/// A target-language backend.
///
/// Implementors emit a whole [`Module`]. They must consume [`gset_ir::Item`]
/// values exhaustively: a match with a catch-all that does nothing reintroduces
/// the silent-drop defect the trait exists to prevent.
pub trait Backend: Sync {
    /// Which target this backend emits.
    fn id(&self) -> TargetId;

    /// How well this backend can express `capability`.
    ///
    /// Consulted by the driver before emission. A backend that returns
    /// [`Support::Unsupported`] must still answer for the construct during
    /// emission rather than ignoring it.
    fn capability(&self, capability: Capability) -> Support;

    /// The indentation unit, written once per nesting level.
    fn indentation(&self) -> &'static str {
        "    "
    }

    /// Emits `module` into `writer`, reporting anything it cannot express.
    fn emit_module(
        &self,
        module: &Module,
        source_map: &SourceMap,
        writer: &mut CodeWriter,
        diagnostics: &mut DiagnosticBag,
    );
}

/// Runs `backend` over `module`, returning the generated text and diagnostics.
pub fn emit(backend: &dyn Backend, module: &Module, source_map: &SourceMap) -> Emitted {
    let mut writer = CodeWriter::new(backend.indentation());
    let mut diagnostics = DiagnosticBag::new();
    backend.emit_module(module, source_map, &mut writer, &mut diagnostics);
    Emitted {
        text: writer.into_string(),
        diagnostics,
    }
}

/// The backend for a target, if one exists.
pub fn backend_for(id: TargetId) -> Option<Box<dyn Backend>> {
    match id {
        TargetId::GO => Some(Box::new(Go)),
        _ => None,
    }
}

/// The backend named `name`, if one exists.
pub fn backend_by_name(name: &str) -> Option<Box<dyn Backend>> {
    TargetId::from_name(name).and_then(backend_for)
}

/// Reports a construct this backend cannot express.
///
/// The single constructor every backend uses, so the code identifier is
/// consistent and greppable.
pub fn unsupported(diagnostics: &mut DiagnosticBag, span: Span, what: impl std::fmt::Display) {
    diagnostics.push(
        gset_ir::Diagnostic::error(
            span,
            format!(
                "this backend cannot express {what}; it is reported rather than emitted \
                 incorrectly"
            ),
        )
        .with_code("gset-backend-unsupported"),
    );
}
