//! Diagnostics.
//!
//! The Go emitter had no error channel at all: it assembled output with
//! `strings.Join` and could not report that it had dropped a construct. Six
//! statement types vanished from output silently, and `&&` emitted into Ruby was
//! simply invalid source with nothing to say so.
//!
//! A [`Diagnostic`] carries a severity, a [`Span`], a message, and optional
//! labelled secondary spans. [`DiagnosticBag`] accumulates them, supports the
//! warning-as-error promotion that lets `gset transpile` keep going on
//! recoverable problems while `gset build` in a CI pipeline treats the same
//! input as fatal.
//!
//! # Degradation, not failure
//!
//! A diagnostic is a value, never a panic. Lowering that cannot understand a
//! construct emits a diagnostic and a `Type::Unknown` rather than unwinding, so
//! one unsupported construct cannot take down a whole translation.

use crate::span::{SourceMap, Span};

/// How serious a [`Diagnostic`] is.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Severity {
    /// Information, such as which constructs were approximated.
    Info,
    /// Something questionable that still has a defensible rendering.
    Warning,
    /// The construct cannot be represented correctly.
    Error,
}

impl Severity {
    /// Reports whether this severity stops a non-strict transpilation.
    pub fn is_error(self) -> bool {
        matches!(self, Severity::Error)
    }
}

/// A second location that helps explain a diagnostic.
///
/// Used for the secondary location that every real compiler wants and that
/// `e.message = "..."` could never express: the alternative being suggested, or
/// the span that made the error likely.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Label {
    /// Where this location is.
    pub span: Span,
    /// What to say about it.
    pub message: String,
}

impl Label {
    /// Creates a labelled span.
    pub fn new(span: Span, message: impl Into<String>) -> Self {
        Label {
            span,
            message: message.into(),
        }
    }

    /// Creates a label at the primary span itself.
    pub fn here(span: Span, message: impl Into<String>) -> Self {
        Label::new(span, message)
    }
}

/// A single problem found while lowering, analysing or emitting.
///
/// Diagnostics are owned values rather than formatted strings so a caller can
/// route them: the CLI renders them to a terminal, a `--format json` mode
/// serialises them, and a future language-server mode can keep them live.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Diagnostic {
    /// How serious this is.
    pub severity: Severity,
    /// The primary location.
    pub span: Span,
    /// What went wrong.
    pub message: String,
    /// Where a code identifier is defined, when that is the useful context.
    pub code: Option<&'static str>,
    /// Additional related locations.
    pub labels: Vec<Label>,
    /// Actionable advice.
    pub help: Option<String>,
}

impl Diagnostic {
    /// Creates a diagnostic at `span`.
    pub fn new(severity: Severity, span: Span, message: impl Into<String>) -> Self {
        Diagnostic {
            severity,
            span,
            message: message.into(),
            code: None,
            labels: Vec::new(),
            help: None,
        }
    }

    /// Creates an error.
    pub fn error(span: Span, message: impl Into<String>) -> Self {
        Diagnostic::new(Severity::Error, span, message)
    }

    /// Creates a warning.
    pub fn warning(span: Span, message: impl Into<String>) -> Self {
        Diagnostic::new(Severity::Warning, span, message)
    }

    /// Creates an informational note.
    pub fn info(span: Span, message: impl Into<String>) -> Self {
        Diagnostic::new(Severity::Info, span, message)
    }

    /// Attaches a stable machine-readable code.
    ///
    /// Frontends should set these. They are how a user can suppress a specific
    /// known-unavoidable diagnostic without suppressing a whole class of them,
    /// which is the difference between a usable escape hatch and a blunt one.
    pub fn with_code(mut self, code: &'static str) -> Self {
        self.code = Some(code);
        self
    }

    /// Adds a secondary location.
    pub fn with_label(mut self, label: Label) -> Self {
        self.labels.push(label);
        self
    }

    /// Attaches actionable advice.
    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }

    /// Renders this diagnostic as a single line of text.
    ///
    /// Deliberately one line. A diagnostic that spans several lines cannot be
    /// grepped, and `gset transpile ... 2>&1 | grep` is how people look for a
    /// specific problem. Multi-line rendering belongs to a separate, explicitly
    /// requested mode.
    pub fn render_one_line(&self, sources: &SourceMap) -> String {
        let level = match self.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Info => "info",
        };
        let mut out = format!("{level}: {}", self.message);
        if let Some(code) = self.code {
            out.push_str(&format!(" [{code}]"));
        }
        out.push_str(&format!(" at {}", sources.location(self.span)));
        if let Some(help) = &self.help {
            out.push_str(&format!(" (help: {help})"));
        }
        out
    }
}

/// An accumulating collection of [`Diagnostic`]s.
///
/// Two counting modes, because the CLI genuinely needs both:
///
/// - `transpile` reports everything and keeps going, then exits non-zero only if
///   any of it was an error.
/// - A strict mode promotes warnings to errors, for pipelines that should treat
///   an approximation as a failure.
#[derive(Clone, Default, Debug)]
pub struct DiagnosticBag {
    diagnostics: Vec<Diagnostic>,
    warnings_as_errors: bool,
}

impl DiagnosticBag {
    /// Creates an empty bag.
    pub fn new() -> Self {
        DiagnosticBag::default()
    }

    /// Creates an empty bag that treats warnings as errors.
    pub fn strict() -> Self {
        DiagnosticBag {
            diagnostics: Vec::new(),
            warnings_as_errors: true,
        }
    }

    /// Turns warning promotion on or off.
    pub fn set_warnings_as_errors(&mut self, enabled: bool) {
        self.warnings_as_errors = enabled;
    }

    /// Adds a diagnostic.
    pub fn push(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.push(diagnostic);
    }

    /// Adds every diagnostic from `other`, leaving `other` unchanged.
    pub fn extend(&mut self, other: DiagnosticBag) {
        self.diagnostics.extend(other.diagnostics);
    }

    /// Returns the effective severity of a diagnostic, applying promotion.
    ///
    /// Promotion happens on read rather than on insert, so a diagnostic is never
    /// silently rewritten. That keeps the collected value faithful to what
    /// actually happened, which matters when the same bag is reported by more
    /// than one consumer.
    pub fn effective_severity(&self, diagnostic: &Diagnostic) -> Severity {
        if self.warnings_as_errors && diagnostic.severity == Severity::Warning {
            Severity::Error
        } else {
            diagnostic.severity
        }
    }

    /// Returns the collected diagnostics in the order they were found.
    pub fn iter(&self) -> impl Iterator<Item = &Diagnostic> {
        self.diagnostics.iter()
    }

    /// Returns the diagnostics as a slice.
    pub fn as_slice(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Returns the number of diagnostics, before promotion.
    pub fn len(&self) -> usize {
        self.diagnostics.len()
    }

    /// Reports whether nothing was collected.
    pub fn is_empty(&self) -> bool {
        self.diagnostics.is_empty()
    }

    /// Returns the diagnostics that are errors after promotion.
    pub fn errors(&self) -> impl Iterator<Item = &Diagnostic> {
        self.diagnostics
            .iter()
            .filter(|d| self.effective_severity(d).is_error())
    }

    /// Returns the number of errors after promotion.
    pub fn error_count(&self) -> usize {
        self.errors().count()
    }

    /// Reports whether anything is fatal, after promotion.
    ///
    /// This is the single question the CLI needs before deciding an exit code.
    pub fn has_errors(&self) -> bool {
        self.errors().next().is_some()
    }

    /// Sorts diagnostics by file, then line, then column.
    ///
    /// Lowering runs in parallel across modules, so without this the same input
    /// can produce diagnostics in a different order on every run. Stable within
    /// a position because `sort_by` is stable, so several diagnostics at one
    /// span keep the order they were found in.
    pub fn sort(&mut self, sources: &SourceMap) {
        self.diagnostics.sort_by_key(|d| {
            let file = d.span.source().raw();
            let (line, column) = sources.get(d.span.source()).map_or(
                // Synthetic and unknown files sort last, by raw id so the order
                // is still deterministic.
                (usize::MAX, usize::MAX),
                |f| f.line_index().location(d.span.start()),
            );
            (file, line, column)
        });
    }

    /// Renders every diagnostic as one line, in collected order.
    pub fn render_one_line(&self, sources: &SourceMap) -> String {
        self.diagnostics
            .iter()
            .map(|d| d.render_one_line(sources))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::span::SourceId;

    fn span_of(offset: u32) -> Span {
        Span::new(SourceId::from_raw(0), offset, offset + 1)
    }

    #[test]
    fn a_diagnostic_renders_with_severity_message_and_location() {
        let mut sources = SourceMap::new();
        let id = sources.add("main.py", "x = 1\ny = 2\n");

        let diagnostic = Diagnostic::error(Span::new(id, 6, 7), "cannot emit comprehension")
            .with_code("gset_unsupported")
            .with_help("use a for loop");

        assert_eq!(
            diagnostic.render_one_line(&sources),
            "error: cannot emit comprehension [gset_unsupported] at main.py:2:1 (help: use a for loop)"
        );
    }

    #[test]
    fn labels_carry_the_definition_site() {
        let diagnostic = Diagnostic::warning(span_of(0), "unknown name")
            .with_label(Label::new(span_of(10), "first referenced here"))
            .with_label(Label::new(span_of(20), "declared here"));
        assert_eq!(diagnostic.labels.len(), 2);
        assert_eq!(diagnostic.labels[1].message, "declared here");
    }

    #[test]
    fn warnings_do_not_fail_a_transpile_but_errors_do() {
        let mut bag = DiagnosticBag::new();
        bag.push(Diagnostic::warning(span_of(0), "approximated"));
        assert!(!bag.has_errors());
        assert_eq!(bag.len(), 1);

        bag.push(Diagnostic::error(span_of(1), "cannot represent"));
        assert!(bag.has_errors());
        assert_eq!(bag.error_count(), 1);
    }

    #[test]
    fn strict_mode_promotes_warnings_to_errors() {
        let mut bag = DiagnosticBag::strict();
        bag.push(Diagnostic::warning(span_of(0), "approximated"));

        assert!(bag.has_errors());
        assert_eq!(bag.error_count(), 1);
        // Promotion happens on read, so the stored diagnostic keeps its real
        // severity. Rewriting it on insert would lose the distinction between
        // "this was a warning" and "this was promoted".
        assert_eq!(bag.as_slice()[0].severity, Severity::Warning);
        assert_eq!(bag.effective_severity(&bag.as_slice()[0]), Severity::Error);
    }

    #[test]
    fn promotion_can_be_toggled() {
        let mut bag = DiagnosticBag::new();
        bag.push(Diagnostic::warning(span_of(0), "approximated"));
        bag.set_warnings_as_errors(true);
        assert!(bag.has_errors());
        bag.set_warnings_as_errors(false);
        assert!(!bag.has_errors());
    }

    #[test]
    fn an_empty_bag_has_no_errors() {
        let bag = DiagnosticBag::new();
        assert!(bag.is_empty());
        assert!(!bag.has_errors());
        assert_eq!(bag.render_one_line(&SourceMap::new()), "");
    }

    #[test]
    fn diagnostics_sort_by_position_not_by_discovery_order() {
        let mut sources = SourceMap::new();
        let id = sources.add("main.py", "a\nb\nc\n");

        let mut bag = DiagnosticBag::new();
        // Collected in reverse order, as parallel lowering would produce them.
        bag.push(Diagnostic::error(Span::new(id, 4, 5), "third"));
        bag.push(Diagnostic::error(Span::new(id, 0, 1), "first"));
        bag.push(Diagnostic::error(Span::new(id, 2, 3), "second"));

        bag.sort(&sources);
        let messages: Vec<_> = bag.iter().map(|d| d.message.as_str()).collect();
        assert_eq!(messages, ["first", "second", "third"]);
    }

    #[test]
    fn sorting_puts_synthetic_diagnostics_last() {
        let mut sources = SourceMap::new();
        let id = sources.add("main.py", "a\n");

        let mut bag = DiagnosticBag::new();
        bag.push(Diagnostic::warning(Span::synthetic(), "no source"));
        bag.push(Diagnostic::error(Span::new(id, 0, 1), "real problem"));

        bag.sort(&sources);
        assert_eq!(bag.as_slice()[0].message, "real problem");
        assert_eq!(bag.as_slice()[1].message, "no source");
    }

    #[test]
    fn sorting_keeps_discovery_order_within_one_position() {
        let mut sources = SourceMap::new();
        let id = sources.add("main.py", "a\n");

        let mut bag = DiagnosticBag::new();
        bag.push(Diagnostic::error(Span::new(id, 0, 1), "first reported"));
        bag.push(Diagnostic::error(Span::new(id, 0, 1), "second reported"));

        bag.sort(&sources);
        let messages: Vec<_> = bag.iter().map(|d| d.message.as_str()).collect();
        assert_eq!(messages, ["first reported", "second reported"]);
    }

    #[test]
    fn a_synthetic_diagnostic_still_renders() {
        // A diagnostic must never be dropped just because it has no file.
        let diagnostic = Diagnostic::info(Span::synthetic(), "lowered without source");
        assert_eq!(
            diagnostic.render_one_line(&SourceMap::new()),
            "info: lowered without source at <synthetic>"
        );
    }

    #[test]
    fn rendering_joins_multiple_diagnostics_with_newlines() {
        let mut sources = SourceMap::new();
        let id = sources.add("main.py", "a\nb\n");

        let mut bag = DiagnosticBag::new();
        bag.push(Diagnostic::error(Span::new(id, 0, 1), "first"));
        bag.push(Diagnostic::warning(Span::new(id, 2, 3), "second"));

        assert_eq!(
            bag.render_one_line(&sources),
            "error: first at main.py:1:1\nwarning: second at main.py:2:1"
        );
    }
}
