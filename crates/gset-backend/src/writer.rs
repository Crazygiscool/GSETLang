//! The output buffer every backend writes through.
//!
//! # Why indentation is centralised
//!
//! The Go emitter hardcoded a single `indentUnit = "    "` for all five
//! targets, including Go itself. Four spaces is not Go's convention — `gofmt`
//! uses tabs — so every generated `.go` file failed `gofmt`, and the M2
//! syntax-check gate would have failed on output the emitter considered
//! correct.
//!
//! A writer that owns indentation means a backend cannot emit a construct at
//! the wrong depth by forgetting to indent, and cannot pick the wrong indent
//! character because it never writes one. The backend supplies its
//! indentation once, from [`crate::Backend::indentation`].
//!
//! # Deferred indentation
//!
//! Indentation is applied lazily, when the first non-empty text of a line is
//! written. A blank line therefore carries no trailing whitespace, which
//! `gofmt` and every other formatter would otherwise strip, and which would
//! make generated output differ from itself after a formatter pass.

/// A text buffer with indentation, used by backends to emit source.
#[derive(Clone, Debug)]
pub struct CodeWriter {
    buffer: String,
    indent: &'static str,
    level: usize,
    /// Whether the current line already has its indentation applied.
    line_started: bool,
    /// Whether anything has been written to the current line.
    line_has_text: bool,
}

impl CodeWriter {
    /// Creates a writer whose indentation unit is `indent`.
    pub fn new(indent: &'static str) -> Self {
        CodeWriter {
            buffer: String::new(),
            indent,
            level: 0,
            line_started: false,
            line_has_text: false,
        }
    }

    /// Increases the indentation level by one.
    pub fn indent(&mut self) {
        self.level += 1;
    }

    /// Decreases the indentation level by one.
    ///
    /// Saturates at zero rather than underflowing: a backend that over-dedents
    /// has a bug, but emitting at column zero is a better failure than a panic
    /// in the middle of a translation.
    pub fn dedent(&mut self) {
        self.level = self.level.saturating_sub(1);
    }

    /// Writes text without a trailing newline.
    ///
    /// The indentation for the line is emitted before `text` if this is the
    /// first write on the line.
    pub fn write(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        self.apply_indent();
        self.buffer.push_str(text);
        self.line_has_text = true;
    }

    /// Writes one line of text.
    pub fn writeln(&mut self, text: &str) {
        if !text.is_empty() {
            self.apply_indent();
            self.buffer.push_str(text);
            self.line_has_text = true;
        }
        self.newline();
    }

    /// Ends the current line.
    pub fn newline(&mut self) {
        self.buffer.push('\n');
        self.line_started = false;
        self.line_has_text = false;
    }

    /// Emits a blank line, unless the previous line was already blank.
    ///
    /// Collapsing repeats keeps output stable: a backend that emits a blank
    /// line after every item, including the last, would otherwise leave a
    /// trailing blank that a formatter removes, making the output idempotency
    /// check fail.
    pub fn blank(&mut self) {
        if !self.buffer.ends_with("\n\n") && !self.buffer.is_empty() {
            self.buffer.push('\n');
            self.line_started = false;
            self.line_has_text = false;
        }
    }

    /// Whether the current line already contains text.
    pub fn line_has_text(&self) -> bool {
        self.line_has_text
    }

    /// Whether nothing has been written yet.
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    /// The current indentation level.
    pub fn level(&self) -> usize {
        self.level
    }

    /// Consumes the writer, returning the accumulated text.
    pub fn into_string(self) -> String {
        self.buffer
    }

    /// Borrows the accumulated text.
    pub fn as_str(&self) -> &str {
        &self.buffer
    }

    fn apply_indent(&mut self) {
        if self.line_started {
            return;
        }
        for _ in 0..self.level {
            self.buffer.push_str(self.indent);
        }
        self.line_started = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_are_indented_on_first_write() {
        let mut writer = CodeWriter::new("    ");
        writer.writeln("fn main() {");
        writer.indent();
        writer.writeln("body");
        writer.dedent();
        writer.writeln("}");
        assert_eq!(writer.as_str(), "fn main() {\n    body\n}\n");
    }

    #[test]
    fn partial_writes_share_one_indent() {
        let mut writer = CodeWriter::new("\t");
        writer.indent();
        writer.write("let x = ");
        writer.write("1;");
        writer.newline();
        assert_eq!(writer.as_str(), "\tlet x = 1;\n");
    }

    #[test]
    fn blank_lines_carry_no_trailing_whitespace() {
        let mut writer = CodeWriter::new("    ");
        writer.indent();
        writer.writeln("a");
        writer.blank();
        writer.writeln("b");
        assert_eq!(writer.as_str(), "    a\n\n    b\n");
    }

    #[test]
    fn repeated_blanks_collapse() {
        let mut writer = CodeWriter::new("    ");
        writer.writeln("a");
        writer.blank();
        writer.blank();
        writer.blank();
        writer.writeln("b");
        assert_eq!(writer.as_str(), "a\n\nb\n");
    }

    #[test]
    fn a_leading_blank_is_suppressed() {
        // Output must not start with a blank line; a formatter would remove it
        // and the output would no longer be idempotent.
        let mut writer = CodeWriter::new("    ");
        writer.blank();
        writer.writeln("a");
        assert_eq!(writer.as_str(), "a\n");
    }

    #[test]
    fn dedent_saturates() {
        let mut writer = CodeWriter::new("    ");
        writer.dedent();
        writer.writeln("x");
        assert_eq!(writer.as_str(), "x\n");
    }

    #[test]
    fn empty_writes_do_not_start_a_line() {
        let mut writer = CodeWriter::new("    ");
        writer.indent();
        writer.write("");
        writer.writeln("x");
        assert_eq!(writer.as_str(), "    x\n");
    }
}
