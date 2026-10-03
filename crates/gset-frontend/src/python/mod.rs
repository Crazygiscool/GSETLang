//! Python frontend.
//!
//! The first source language, chosen because it is dynamic: it forces type
//! inference, which exercises the hardest part of the IR early instead of
//! hiding behind annotations.

use gset_ir::{Diagnostic, LangId, Lowered, SourceId, Span};

use crate::Frontend;

/// The tree-sitter Python grammar.
///
/// Wrapped in a function rather than a constant because building a `Language`
/// is not free, and a caller that never parses should not pay for it.
pub fn grammar() -> tree_sitter::Language {
    tree_sitter_python::LANGUAGE.into()
}

/// A parsed file, plus whatever could not be parsed.
pub struct Parsed {
    /// The syntax tree.
    ///
    /// Present even when the source has syntax errors: tree-sitter recovers and
    /// replaces the damaged region with `ERROR` nodes, and discarding the tree
    /// would throw away the rest of a file that is mostly fine.
    pub tree: tree_sitter::Tree,
    /// One diagnostic per `ERROR` or missing node.
    pub diagnostics: Vec<Diagnostic>,
}

/// Parses Python source.
///
/// `source` identifies the text so every span it produces points at real
/// source. Diagnostics from this function describe syntax only; semantic
/// problems are the lowering's business.
///
/// The error case is a failure to produce any tree at all, which needs a
/// cancelled parse to reach. It is kept as a `Result` rather than an empty
/// `Parsed` because there is no honest empty tree to return, and inventing one
/// would let the caller walk a tree that was never parsed.
pub fn parse(source: SourceId, text: &str) -> Result<Parsed, Diagnostic> {
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&grammar()).map_err(|error| {
        Diagnostic::error(
            Span::synthetic(),
            format!("cannot load python grammar: {error}"),
        )
    })?;

    let tree = parser.parse(text, None).ok_or_else(|| {
        Diagnostic::error(
            Span::point(source, 0),
            "python parser produced no syntax tree",
        )
    })?;

    let mut diagnostics = Vec::new();
    collect_syntax_diagnostics(tree.root_node(), text, source, &mut diagnostics);
    Ok(Parsed { tree, diagnostics })
}

/// Converts a tree-sitter node's byte range into an IR span.
fn span_of(node: tree_sitter::Node<'_>, source: SourceId) -> Span {
    Span::new(source, node.start_byte() as u32, node.end_byte() as u32)
}

/// Records a diagnostic for every `ERROR` and missing node under `node`.
///
/// `text` is the source the tree was parsed from, needed to quote the offending
/// fragment in the message.
fn collect_syntax_diagnostics(
    node: tree_sitter::Node<'_>,
    text: &str,
    source: SourceId,
    out: &mut Vec<Diagnostic>,
) {
    if node.is_missing() {
        // A missing node is zero-width: it marks the point where a token should
        // have been. Reporting it as a span over nothing would produce a
        // diagnostic with no context, so the message names the token instead.
        out.push(
            Diagnostic::error(span_of(node, source), format!("missing `{}`", node.kind()))
                .with_code("gset-syntax"),
        );
        return;
    }

    if node.is_error() {
        let fragment = text.get(node.byte_range()).unwrap_or("").trim();
        let detail = if fragment.is_empty() {
            String::new()
        } else {
            format!(" near `{fragment}`")
        };
        out.push(
            Diagnostic::error(span_of(node, source), format!("could not parse{detail}"))
                .with_code("gset-syntax"),
        );
    }

    // Drive the cursor directly: holding a child iterator borrows the cursor
    // mutably, which rules out reading the field name inside a closure.
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            collect_syntax_diagnostics(cursor.node(), text, source, out);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
}

/// The Python frontend.
#[derive(Clone, Copy, Debug, Default)]
pub struct Python;

impl Frontend for Python {
    fn lang(&self) -> LangId {
        LangId::PYTHON
    }

    fn name(&self) -> &'static str {
        "python"
    }

    fn handles_extension(&self, path: &std::path::Path) -> bool {
        matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("py" | "pyi")
        )
    }

    fn lower(&self, _source: &str, _text: &str) -> Lowered {
        // Lowering lands next. The parse half is wired up and tested against
        // real input first, so the grammar is proven before anything is built
        // on top of it.
        let mut diagnostics = gset_ir::DiagnosticBag::new();
        diagnostics.push(Diagnostic::error(
            Span::synthetic(),
            "python lowering is not implemented yet",
        ));
        Lowered {
            module: None,
            diagnostics,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{frontend_by_name, frontend_for_path};

    fn source_id() -> SourceId {
        SourceId::from_raw(0)
    }

    fn parse_ok(text: &str) -> Parsed {
        parse(source_id(), text).expect("parse")
    }

    #[test]
    fn the_grammar_loads() {
        // If this fails the grammar and the tree-sitter runtime disagree, and
        // every other test in this module would be vacuous.
        assert!(grammar().node_kind_count() > 0);
    }

    #[test]
    fn a_trivial_module_parses_without_complaint() {
        let parsed = parse_ok("x = 1\n");
        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        assert_eq!(parsed.tree.root_node().kind(), "module");
        assert!(!parsed.tree.root_node().has_error());
    }

    #[test]
    fn the_constructs_the_ir_needs_are_recognised() {
        // The frontend is only worth building if the grammar actually exposes
        // the shapes the IR models. This pins those node names.
        let source = "\
import os
from os import path

def f(a: int) -> str:
    return str(a)

class C:
    pass
";
        let parsed = parse_ok(source);
        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        let sexp = parsed.tree.root_node().to_sexp();
        for kind in [
            "import_statement",
            "import_from_statement",
            "function_definition",
            "class_definition",
            "typed_parameter",
            "block",
        ] {
            assert!(sexp.contains(kind), "missing {kind} in:\n{sexp}");
        }
    }

    #[test]
    fn control_flow_shapes_match_the_ir() {
        let source = "\
def f():
    for i in range(3):
        while i:
            if i:
                break
            else:
                continue
    try:
        pass
    except ValueError:
        pass
    with open('f') as fh:
        pass
";
        let parsed = parse_ok(source);
        let sexp = parsed.tree.root_node().to_sexp();
        for kind in [
            "for_statement",
            "while_statement",
            "if_statement",
            "else_clause",
            "try_statement",
            "except_clause",
            "with_statement",
            "pass_statement",
            "break_statement",
            "continue_statement",
        ] {
            assert!(sexp.contains(kind), "missing {kind} in:\n{sexp}");
        }
    }

    #[test]
    fn a_syntax_error_yields_a_tree_and_a_diagnostic() {
        // Discarding the tree would lose the rest of the file; the diagnostic
        // exists so the user learns which part was damaged.
        let parsed = parse_ok("def f(:\n    pass\n");
        assert!(parsed.tree.root_node().has_error());
        assert!(
            !parsed.diagnostics.is_empty(),
            "a broken file must still be reported"
        );
        assert_eq!(parsed.diagnostics[0].code, Some("gset-syntax"));
    }

    #[test]
    fn spans_point_at_the_offending_text() {
        let text = "x = 1\ndef (:\n";
        let parsed = parse_ok(text);
        let diagnostic = &parsed.diagnostics[0];
        assert_eq!(diagnostic.span.source(), source_id());
        let start = diagnostic.span.start() as usize;
        let end = diagnostic.span.end() as usize;
        assert!(
            end <= text.len(),
            "span {start}..{end} is outside the {}-byte input",
            text.len()
        );
    }

    #[test]
    fn python_handles_py_and_pyi_only() {
        assert!(Python.handles_extension(std::path::Path::new("a.py")));
        assert!(Python.handles_extension(std::path::Path::new("a.pyi")));
        assert!(!Python.handles_extension(std::path::Path::new("a.go")));
        assert!(!Python.handles_extension(std::path::Path::new("a.rs")));
    }

    #[test]
    fn the_registry_finds_python_by_name_and_extension() {
        assert!(frontend_by_name("python").is_some());
        assert!(
            frontend_by_name("Python").is_some(),
            "names are case-insensitive"
        );
        assert!(frontend_by_name("go").is_none(), "no Go frontend yet");
        assert!(frontend_for_path(std::path::Path::new("x.py")).is_some());
        assert!(
            frontend_for_path(std::path::Path::new("x.rs")).is_none(),
            "an unknown extension must not silently fall back to a default"
        );
    }
}
