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
    use crate::limits::{DEFAULT, Limits};
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
    /// Every file in the corpus must parse cleanly.
    ///
    /// The corpus exists to drive the lowering, and a corpus file that does not
    /// parse would make every later failure ambiguous. This is the check that
    /// keeps the fixtures honest.
    #[test]
    fn the_whole_corpus_parses_without_errors() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/py");
        let mut files: Vec<_> = std::fs::read_dir(&root)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", root.display()))
            .map(|entry| entry.expect("dir entry").path())
            .filter(|path| path.extension().is_some_and(|extension| extension == "py"))
            .collect();
        files.sort();

        assert!(
            files.len() >= 15,
            "the plan asks for 15-20 varied files, found {}",
            files.len()
        );

        let mut failures = Vec::new();
        for file in &files {
            let text = std::fs::read_to_string(file).expect("read corpus file");
            let parsed = parse(source_id(), &text).expect("parse");
            if !parsed.diagnostics.is_empty() {
                failures.push(format!(
                    "{}: {:?}",
                    file.file_name().unwrap().to_string_lossy(),
                    messages(&parsed.diagnostics)
                ));
            }
        }
        assert!(
            failures.is_empty(),
            "corpus files must parse cleanly:\n{}",
            failures.join("\n")
        );
    }

    /// The corpus must actually cover the audit's defect classes.
    ///
    /// Otherwise it is 20 files that happen to compile, and the whole point is
    /// lost. One representative file per class is required.
    #[test]
    fn the_corpus_covers_every_audit_defect_class() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/py");
        // The manifest lives one level up so the corpus directory holds nothing
        // but language sources.
        let manifest = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/MANIFEST.md"),
        )
        .expect("corpus manifest");

        // Each mapping line is "<label>: <file>". Split on the last colon so a
        // label may itself contain colons.
        let recorded: Vec<(String, String)> = manifest
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .filter_map(|line| {
                let (label, file) = line.rsplit_once(": ")?;
                Some((label.trim().to_string(), file.trim().to_string()))
            })
            .collect();
        assert!(
            !recorded.is_empty(),
            "the manifest recorded no defect-class mappings at all"
        );

        let required = [
            (
                "class 1: only the first top-level if emitted",
                "module_shape.py",
            ),
            ("class 2: struct and enum vanished", "declarations.py"),
            ("class 3: ?? has no Go mapping", "null_handling.py"),
            ("class 5: comprehension passed through", "comprehensions.py"),
            ("class 6: literal newline in a block lambda", "lambdas.py"),
            ("class 7: malformed loop header", "loops.py"),
            ("class 8: interface{} and an undeclared name", "literals.py"),
            ("class 9: missing import", "imports.py"),
            ("class 10: integer literal saturates", "numeric_literals.py"),
            ("class 11: name replaced by nil", "declaration_after_if.py"),
            ("class 12: untyped parameter", "returns.py"),
            ("class 13: void method returning a value", "returns.py"),
            ("class 14: function swallows the module", "returns.py"),
            ("decorators survive as written", "decorators.py"),
            ("f-strings keep their shape", "fstrings.py"),
        ];
        for (class, file) in required {
            assert!(
                recorded
                    .iter()
                    .any(|(label, recorded_file)| { label == class && recorded_file == file }),
                "the manifest does not record {class} -> {file}"
            );
            assert!(
                root.join(file).exists(),
                "{class} names {file}, which does not exist"
            );
        }
    }

    /// Counts nodes of one kind under `node`.
    ///
    /// Walking beats counting occurrences in `to_sexp`, where a node's kind also
    /// appears as its field name.
    fn count_kind(node: &tree_sitter::Node<'_>, kind: &str) -> usize {
        let mut total = 0;
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                if cursor.node().kind() == kind {
                    total += 1;
                }
                total += count_kind(&cursor.node(), kind);
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        total
    }

    fn messages(diagnostics: &[Diagnostic]) -> Vec<String> {
        diagnostics
            .iter()
            .map(|diagnostic| format!("{}: {}", diagnostic.code.unwrap_or("?"), diagnostic.message))
            .collect()
    }
    /// Go's `TestParseIndexExpression` and `TestParseChainedIndexExpression`.
    ///
    /// Ported because a subscript is the shape most likely to be mis-lowered: it
    /// is an index into a list in one target and a hash lookup in another, and
    /// the two must not be confused. The Go test asserted on a re-rendered
    /// string; here the structure is asserted instead, because a string
    /// comparison would only prove the emitter round-trips.
    #[test]
    fn subscripts_and_chained_subscripts_parse() {
        let source = "nums = [1, 2, 3]
print(nums[0])
print(nums[1] + nums[2])
matrix = [[1, 2], [3, 4]]
print(matrix[1][0])
print(grid[0][1][2])
";
        let parsed = parse_ok(source);
        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        let sexp = parsed.tree.root_node().to_sexp();

        // Count real nodes: `to_sexp` also prints `subscript:` as a field
        // name, so counting occurrences would double every one.
        assert_eq!(
            count_kind(&parsed.tree.root_node(), "subscript"),
            8,
            "expected eight subscript nodes (1+1+1+2+3) in:\n{sexp}"
        );
        // The chained ones really are nested, not flattened.
        assert!(
            sexp.contains("subscript value: (subscript"),
            "matrix[1][0] must nest its subscripts:\n{sexp}"
        );
        assert!(
            sexp.contains("subscript value: (subscript value: (subscript"),
            "grid[0][1][2] must nest three deep:\n{sexp}"
        );
    }

    /// Go's `TestVariableDeclarationIsDeclared`.
    ///
    /// The intent was that a declaration is recognised as a declaration. Here
    /// that means each annotated assignment and each `def` is an item rather
    /// than a bare expression statement, which is the split `gset-semantic`
    /// depends on and the thing defect class 14 lost when a trailing statement
    /// was absorbed into the function above it.
    #[test]
    fn declarations_are_recognised_as_declarations() {
        let source = concat!(
            "x = 1\n",
            "y: int = 2\n",
            "def f():\n",
            "    return 1\n",
            "\n",
            "class C:\n",
            "    pass\n",
            "z = 3\n",
        );
        let parsed = parse_ok(source);
        let root = parsed.tree.root_node();

        // Top-level kinds, in source order. An `expression_statement` wrapping
        // an `assignment` that has a `type` field is an annotated declaration.
        fn classify(node: tree_sitter::Node<'_>) -> String {
            let annotated = node.named_child(0).is_some_and(|child| {
                child.kind() == "assignment" && child.child_by_field_name("type").is_some()
            });
            if annotated {
                "annotated_decl".to_string()
            } else {
                node.kind().to_string()
            }
        }

        let top_level: Vec<String> = {
            let mut cursor = root.walk();
            let mut kinds = Vec::new();
            if cursor.goto_first_child() {
                loop {
                    if cursor.node().is_named() {
                        kinds.push(classify(cursor.node()));
                    }
                    if !cursor.goto_next_sibling() {
                        break;
                    }
                }
            }
            kinds
        };

        assert_eq!(
            top_level,
            [
                "expression_statement",
                "annotated_decl",
                "function_definition",
                "class_definition",
                "expression_statement"
            ],
            "the annotated assignment must be distinguishable from a bare one"
        );
    }

    /// Go's `TestParseGarbageBlockTerminatesWithError`.
    ///
    /// The Go test needed a 5 second watchdog and could hang. This asserts the
    /// same thing without a watchdog: parsing malformed input must produce a
    /// diagnostic, promptly, and never panic.
    #[test]
    fn malformed_input_produces_a_diagnostic_and_never_panics() {
        for source in [
            "export add(a, b) {",
            "def f(:\n    pass\n",
            "class ???\n",
            "x = = 1",
            "if:\n",
            "@\n",
            "(((((((",
            "\"\"unterminated",
            "def f(\n",
            "lambda\n",
            "x = [1, 2\n",
        ] {
            let parsed = parse(source_id(), source).expect("a tree is always produced");
            assert!(
                parsed.tree.root_node().has_error() || !parsed.diagnostics.is_empty(),
                "expected a problem to be reported for:\n{source}"
            );
        }
    }

    /// Go's `TestParseMatchDoesNotHang`, `TestParseTryCatchDoesNotHang` and
    /// `TestParseListComprehensionDoesNotHang`.
    ///
    /// Those constructs had a parser that could loop forever. tree-sitter
    /// guarantees termination, so what is worth asserting is that the shapes are
    /// recognised and bounded by the depth limit.
    #[test]
    fn the_constructs_that_once_hung_are_bounded() {
        let source = concat!(
            "match x:\n",
            "    case 1:\n",
            "        print('one')\n",
            "    case _:\n",
            "        print('other')\n",
            "\n",
            "try:\n",
            "    risky()\n",
            "except ValueError:\n",
            "    pass\n",
            "finally:\n",
            "    done()\n",
            "\n",
            "squared = [n * n for n in nums]\n",
            "evens = [n for n in nums if n % 2 == 0]\n",
        );
        let parsed = parse_ok(source);
        let root = parsed.tree.root_node();
        let sexp = root.to_sexp();
        for kind in [
            "match_statement",
            "case_clause",
            "try_statement",
            "except_clause",
            "finally_clause",
            "list_comprehension",
        ] {
            assert!(sexp.contains(kind), "missing {kind} in:\n{sexp}");
        }
        assert!(
            Limits::depth_of(&root) <= DEFAULT.max_depth,
            "these constructs must stay within the depth limit"
        );
    }
}
