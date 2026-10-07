//! Resource limits applied while parsing and lowering.
//!
//! These are not arbitrary. They are the limits the Go implementation enforced
//! in `security/security.go` and `parser/parser.go`, carried over unchanged so
//! the rewrite does not quietly become more permissive on hostile input. The Go
//! code kept two independent depth limits, one in each package; there is one
//! here, and the duplication is the bug that made it possible for them to drift.
//!
//! Every limit reports a [`Diagnostic`] rather than aborting. A file that is too
//! large or too deeply nested is a user error worth pointing at, not a reason to
//! exit without saying why.
//!
//! ```
//! use gset_frontend::limits::DEFAULT;
//!
//! // Nesting is bounded by depth, and size by bytes. They are separate limits
//! // and a small input can still be too deeply nested.
//! let deep = "(".repeat(DEFAULT.max_depth as usize + 1);
//! assert!(DEFAULT.check_input_size(&deep).is_ok());
//! let error = DEFAULT.check_depth(DEFAULT.max_depth + 1).unwrap_err();
//! assert!(error.contains("nesting depth"), "{error}");
//! ```

use gset_ir::Diagnostic;

/// Limits applied to one source file.
///
/// A struct rather than constants so a caller can tighten a limit for a
/// sandboxed or embedded use without editing the crate.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Limits {
    /// Largest accepted source file, in bytes.
    pub max_input_size: usize,
    /// Deepest accepted nesting.
    pub max_depth: u32,
    /// Largest accepted statement count per module.
    pub max_statements: usize,
    /// Longest accepted string literal, in bytes.
    pub max_string_length: usize,
    /// Longest accepted identifier, in bytes.
    pub max_identifier_len: usize,
}

/// The limits used for ordinary command-line use.
///
/// The numbers match the Go implementation's `security` and `parser` packages.
pub const DEFAULT: Limits = Limits {
    max_input_size: 10 * 1024 * 1024,
    max_depth: 100,
    max_statements: 10_000,
    max_string_length: 1024 * 1024,
    max_identifier_len: 256,
};

impl Default for Limits {
    fn default() -> Self {
        DEFAULT
    }
}

impl Limits {
    /// Rejects a source file that is too large to process.
    ///
    /// Checked before parsing, because a tree-sitter parse of a multi-gigabyte
    /// file would exhaust memory before any limit got a chance to fire.
    pub fn check_input_size(&self, text: &str) -> Result<(), String> {
        if text.len() > self.max_input_size {
            return Err(format!(
                "input is {} bytes, which exceeds the {} byte limit",
                text.len(),
                self.max_input_size
            ));
        }
        Ok(())
    }

    /// Rejects nesting deeper than [`Limits::max_depth`].
    ///
    /// Deep nesting is how a recursive descent parser is made to blow its stack.
    /// tree-sitter handles deep input iteratively, but the *lowering* is
    /// recursive, so this bounds work the grammar cannot.
    pub fn check_depth(&self, depth: u32) -> Result<(), String> {
        if depth > self.max_depth {
            return Err(format!(
                "maximum nesting depth exceeded ({depth} > {})",
                self.max_depth
            ));
        }
        Ok(())
    }

    /// Rejects a module with more statements than [`Limits::max_statements`].
    pub fn check_statement_count(&self, count: usize) -> Result<(), String> {
        if count > self.max_statements {
            return Err(format!(
                "module has {count} statements, which exceeds the {} statement limit",
                self.max_statements
            ));
        }
        Ok(())
    }

    /// Rejects a string literal that is too long.
    pub fn check_string_length(&self, length: usize) -> Result<(), String> {
        if length > self.max_string_length {
            return Err(format!(
                "string literal is {length} bytes, which exceeds the {} byte limit",
                self.max_string_length
            ));
        }
        Ok(())
    }

    /// Rejects an identifier that is too long.
    pub fn check_identifier_len(&self, length: usize) -> Result<(), String> {
        if length > self.max_identifier_len {
            return Err(format!(
                "identifier is {length} bytes, which exceeds the {} byte limit",
                self.max_identifier_len
            ));
        }
        Ok(())
    }

    /// The deepest node in `node`'s subtree, counting the node itself as depth 1.
    ///
    /// Iterative rather than recursive on purpose: measuring the depth by
    /// recursing would itself overflow the stack on exactly the input the limit
    /// exists to catch.
    pub fn depth_of(node: &tree_sitter::Node<'_>) -> u32 {
        let mut deepest = 0;
        let mut current: Vec<(tree_sitter::Node<'_>, u32)> = vec![(*node, 1u32)];
        while let Some((current_node, depth)) = current.pop() {
            if depth > deepest {
                deepest = depth;
            }
            if depth > u32::MAX / 2 {
                // Pathological tree; stop before the counter wraps.
                return deepest;
            }
            let mut cursor = current_node.walk();
            if cursor.goto_first_child() {
                loop {
                    current.push((cursor.node(), depth + 1));
                    if !cursor.goto_next_sibling() {
                        break;
                    }
                }
            }
        }
        deepest
    }

    /// Checks `text` against every limit that can be judged without lowering it.
    ///
    /// Returns the first failure. The order is input size, then depth, because
    /// size is the cheapest to detect and the most likely to be the real
    /// problem.
    pub fn check_source(&self, text: &str, root: &tree_sitter::Node<'_>) -> Result<(), Diagnostic> {
        self.check_input_size(text).map_err(|message| {
            Diagnostic::error(gset_ir::Span::synthetic(), message).with_code("gset-limit")
        })?;
        let depth = Self::depth_of(root);
        self.check_depth(depth).map_err(|message| {
            Diagnostic::error(gset_ir::Span::synthetic(), message).with_code("gset-limit")
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::python;
    use gset_ir::SourceId;

    // The Go security package's own cases, carried over so the rewrite is not
    // more permissive on hostile input than what it replaces.

    #[test]
    fn input_size_boundary_is_exact() {
        // Go: short, exactly at the limit, and one byte over.
        assert!(DEFAULT.check_input_size("short input").is_ok());
        assert!(
            DEFAULT
                .check_input_size(&"a".repeat(DEFAULT.max_input_size))
                .is_ok()
        );
        let over = DEFAULT
            .check_input_size(&"a".repeat(DEFAULT.max_input_size + 1))
            .unwrap_err();
        assert!(over.contains("exceeds"), "{over}");
    }

    #[test]
    fn depth_boundary_is_exact() {
        // Go: 0, exactly at the limit, one over, and a middling depth.
        assert!(DEFAULT.check_depth(0).is_ok());
        assert!(DEFAULT.check_depth(DEFAULT.max_depth).is_ok());
        assert!(DEFAULT.check_depth(DEFAULT.max_depth + 1).is_err());
        assert!(DEFAULT.check_depth(50).is_ok());
    }

    #[test]
    fn statement_count_boundary_is_exact() {
        assert!(DEFAULT.check_statement_count(0).is_ok());
        assert!(
            DEFAULT
                .check_statement_count(DEFAULT.max_statements)
                .is_ok()
        );
        assert!(
            DEFAULT
                .check_statement_count(DEFAULT.max_statements + 1)
                .is_err()
        );
    }

    #[test]
    fn string_and_identifier_boundaries_are_exact() {
        assert!(
            DEFAULT
                .check_string_length(DEFAULT.max_string_length)
                .is_ok()
        );
        assert!(
            DEFAULT
                .check_string_length(DEFAULT.max_string_length + 1)
                .is_err()
        );
        assert!(
            DEFAULT
                .check_identifier_len(DEFAULT.max_identifier_len)
                .is_ok()
        );
        assert!(
            DEFAULT
                .check_identifier_len(DEFAULT.max_identifier_len + 1)
                .is_err()
        );
    }

    #[test]
    fn the_limits_match_the_go_implementation() {
        // Asserted explicitly: if these drift, the rewrite has silently become
        // more permissive than the thing it replaces.
        assert_eq!(DEFAULT.max_input_size, 10 * 1024 * 1024);
        assert_eq!(DEFAULT.max_depth, 100);
        assert_eq!(DEFAULT.max_statements, 10_000);
        assert_eq!(DEFAULT.max_string_length, 1024 * 1024);
        assert_eq!(DEFAULT.max_identifier_len, 256);
    }

    #[test]
    fn a_deeply_nested_input_is_rejected_rather_than_overflowing() {
        // The Go parser hung on `export add(a, b) {` and needed a 5s watchdog.
        // Depth is now bounded up front, so no watchdog is needed.
        let source = format!("x = {}1{}", "(".repeat(500), ")".repeat(500));
        let parsed = python::parse(SourceId::from_raw(0), &source).expect("parse");
        let error = DEFAULT
            .check_source(&source, &parsed.tree.root_node())
            .expect_err("deeply nested input must be rejected");
        assert_eq!(error.code, Some("gset-limit"));
        assert!(error.message.contains("nesting depth"), "{}", error.message);
    }

    #[test]
    fn measuring_depth_does_not_overflow_on_a_deep_tree() {
        // depth_of must be iterative: a recursive measurement would blow the
        // stack on the very input the limit exists to catch.
        let source = format!("x = {}1{}", "(".repeat(2_000), ")".repeat(2_000));
        let parsed = python::parse(SourceId::from_raw(0), &source).expect("parse");
        let depth = Limits::depth_of(&parsed.tree.root_node());
        assert!(
            depth > DEFAULT.max_depth,
            "expected a deep tree, got {depth}"
        );
    }

    #[test]
    fn ordinary_source_passes_every_limit() {
        let source = "def f(x):\n    return x + 1\n";
        let parsed = python::parse(SourceId::from_raw(0), source).expect("parse");
        assert!(
            DEFAULT
                .check_source(source, &parsed.tree.root_node())
                .is_ok()
        );
    }

    #[test]
    fn a_tightened_limit_is_honoured() {
        // The point of a struct rather than constants.
        let strict = Limits {
            max_input_size: 16,
            ..DEFAULT
        };
        assert!(strict.check_input_size("x = 1").is_ok());
        assert!(strict.check_input_size(&"x = 1".repeat(10)).is_err());
    }
}
