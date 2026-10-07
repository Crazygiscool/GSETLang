//! Source positions.
//!
//! Every IR node carries a [`Span`]. This module defines that span, the
//! [`SourceMap`] that turns byte offsets back into human-readable positions, and
//! [`SourceId`] to distinguish files once the compiler handles more than one.
//!
//! Spans are `Copy` and 12 bytes. IR nodes are cloned and passed around
//! constantly, so the span must not allocate and must not borrow from the
//! [`SourceMap`] — a node has to stay valid after the map it came from is gone.
//! That is why a span stores a [`SourceId`] and a byte range rather than a
//! reference into source text.

use std::fmt;
use std::ops::Range;

/// Identifies a source file within a [`SourceMap`].
///
/// One ID per file rather than per module, because a module assembled from
/// several files is normal in the multi-file work of M4.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct SourceId(u32);

impl SourceId {
    /// The identifier used for spans that genuinely have no known origin, such
    /// as a node synthesised by a lowering pass.
    ///
    /// Kept distinct from a real file so diagnostics can be told apart from
    /// synthetic code instead of pointing at an arbitrary byte offset in
    /// whichever file happened to be first.
    pub const SYNTHETIC: SourceId = SourceId(u32::MAX);

    /// Creates an identifier from a raw index.
    ///
    /// Only useful when deserialising an IR, or in tests.
    pub const fn from_raw(raw: u32) -> Self {
        SourceId(raw)
    }

    /// Returns the raw index.
    pub const fn raw(self) -> u32 {
        self.0
    }
}

impl fmt::Debug for SourceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if *self == SourceId::SYNTHETIC {
            f.write_str("SourceId(synthetic)")
        } else {
            write!(f, "SourceId({})", self.0)
        }
    }
}

/// A half-open byte range within one source file.
///
/// `[start, end)`. Empty spans are legal and meaningful: an empty range marks a
/// point, which is what a zero-width construct such as a missing operand
/// deserves in a diagnostic.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    source: SourceId,
    start: u32,
    end: u32,
}

impl Span {
    /// Creates a span over `start..end` within `source`.
    ///
    /// # Panics
    ///
    /// Debug builds panic if `end < start`. A reversed span is always a bug in
    /// the caller, and catching it at construction beats emitting a diagnostic
    /// that points backwards.
    pub const fn new(source: SourceId, start: u32, end: u32) -> Self {
        debug_assert!(start <= end, "span start is after span end");
        Span { source, start, end }
    }

    /// Creates a span at a single byte offset.
    pub const fn point(source: SourceId, offset: u32) -> Self {
        Span::new(source, offset, offset)
    }

    /// Creates a span attributed to no real file.
    pub const fn synthetic() -> Self {
        Span::new(SourceId::SYNTHETIC, 0, 0)
    }

    /// Returns the file this span belongs to.
    pub const fn source(self) -> SourceId {
        self.source
    }

    /// Returns the start byte offset.
    pub const fn start(self) -> u32 {
        self.start
    }

    /// Returns the end byte offset.
    pub const fn end(self) -> u32 {
        self.end
    }

    /// Returns the span as a [`Range`].
    pub const fn range(self) -> Range<usize> {
        self.start as usize..self.end as usize
    }

    /// Returns the length in bytes.
    pub const fn len(self) -> u32 {
        self.end - self.start
    }

    /// Reports whether the span covers no bytes.
    pub const fn is_empty(self) -> bool {
        self.end == self.start
    }

    /// Reports whether this span points at no real file.
    pub fn is_synthetic(self) -> bool {
        self.source == SourceId::SYNTHETIC
    }

    /// Extends the span to also cover `other`.
    ///
    /// The result spans from the earlier start to the later end. Spans from
    /// different files cannot be merged meaningfully; combining them yields the
    /// span of `self`, which keeps the earliest position rather than inventing a
    /// range that exists in no file.
    pub fn to(self, other: Span) -> Span {
        if self.source != other.source {
            return self;
        }
        Span {
            source: self.source,
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }

    /// Reports whether `offset` falls inside the span.
    ///
    /// An empty span contains exactly the offset it points at, which is what
    /// makes `point` spans usable for underlining an insertion point.
    pub const fn contains(self, offset: u32) -> bool {
        offset >= self.start && offset <= self.end
    }

    /// Reports whether the two spans are in the same file and touch or overlap.
    pub fn touches(self, other: Span) -> bool {
        self.source == other.source && self.start <= other.end && other.start <= self.end
    }
}

/// The default span is synthetic.
///
/// A span that defaulted to file 0 offset 0 would silently point diagnostics at
/// the top of whichever file happened to be registered first, which is worse
/// than admitting there is no source.
impl Default for Span {
    fn default() -> Self {
        Span::synthetic()
    }
}

impl fmt::Debug for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_synthetic() {
            return f.write_str("Span(synthetic)");
        }
        write!(f, "{}:{}..{}", self.source.raw(), self.start, self.end)
    }
}

/// One-to-one line/column lookup, stored as byte offsets.
///
/// Offsets rather than a character count per line, because a file with non-ASCII
/// identifiers is normal and byte offsets are what a slice index needs.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Default, Debug)]
pub struct LineIndex {
    starts: Vec<u32>,
    len: u32,
}

impl LineIndex {
    /// Builds an index for `text` by recording where each line begins.
    pub fn new(text: &str) -> Self {
        let mut starts = vec![0u32];
        for (offset, byte) in text.bytes().enumerate() {
            if byte == b'\n' {
                starts.push(offset as u32 + 1);
            }
        }
        LineIndex {
            starts,
            len: text.len() as u32,
        }
    }

    /// Returns the number of lines.
    ///
    /// Text ending in a newline counts that trailing empty line, so `"a\n"` has
    /// two lines and the last one is empty.
    pub fn line_count(&self) -> usize {
        self.starts.len()
    }

    /// Returns the byte offset where the 1-based `line` begins.
    ///
    /// Lines past the end clamp to the final line rather than panicking, so a
    /// span from a stale or truncated parse degrades into a readable
    /// diagnostic instead of crashing the compiler.
    pub fn line_start(&self, line: usize) -> u32 {
        let line = line.clamp(1, self.starts.len());
        self.starts[line - 1]
    }

    /// Converts a byte offset into a 1-based line and column.
    ///
    /// Columns count bytes, consistent with [`LineIndex`] and with the
    /// terminals that will display them.
    ///
    /// An offset past the end of the file clamps to the end of the file rather
    /// than producing a column in the billions. An out-of-bounds offset means
    /// the caller's span is wrong, and pointing at end-of-file is the most
    /// useful thing to do with it.
    pub fn location(&self, offset: u32) -> (usize, usize) {
        let offset = offset.min(self.len);
        // partition_point gives the count of line starts at or before offset,
        // which is exactly the 0-based line index.
        let line = self
            .starts
            .partition_point(|&start| start <= offset)
            .clamp(1, self.starts.len());
        let start = self.starts[line - 1];
        (line, offset.saturating_sub(start) as usize + 1)
    }
}

/// A source file registered in a [`SourceMap`].
#[derive(Clone, Debug)]
pub struct SourceFile {
    name: String,
    text: String,
    line_index: LineIndex,
}

impl SourceFile {
    /// Returns the display name, typically a path.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the full text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Returns the line index.
    pub fn line_index(&self) -> &LineIndex {
        &self.line_index
    }

    /// Returns the text covered by `span`, or `None` if the span is synthetic
    /// or reaches past the end of the file.
    pub fn slice(&self, span: Span) -> Option<&str> {
        if span.is_synthetic() {
            return None;
        }
        self.text.get(span.range())
    }
}

/// The set of source files a compilation is working from.
///
/// Frontends register files here and use the returned [`SourceId`] on every
/// span. Backends and the diagnostic renderer resolve spans back through this
/// map. Owning the text centrally is what lets a [`Span`] stay 12 bytes and
/// `Copy` while still pointing at real code.
#[derive(Clone, Debug, Default)]
pub struct SourceMap {
    files: Vec<SourceFile>,
}

impl SourceMap {
    /// Creates an empty map.
    pub fn new() -> Self {
        SourceMap::default()
    }

    /// Registers `text` under `name` and returns its identifier.
    pub fn add(&mut self, name: impl Into<String>, text: impl Into<String>) -> SourceId {
        let text = text.into();
        let line_index = LineIndex::new(&text);
        self.files.push(SourceFile {
            name: name.into(),
            text,
            line_index,
        });
        SourceId((self.files.len() - 1) as u32)
    }

    /// Returns the file for `id`, or `None` for [`SourceId::SYNTHETIC`] and for
    /// identifiers that were never registered.
    pub fn get(&self, id: SourceId) -> Option<&SourceFile> {
        self.files.get(id.raw() as usize)
    }

    /// Returns the number of registered files.
    pub fn len(&self) -> usize {
        self.files.len()
    }

    /// Reports whether no files are registered.
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// Renders `span` as `file:line:column`, the form editors can jump to.
    ///
    /// Returns a placeholder rather than `None` when the file is unknown, so a
    /// diagnostic is never dropped just because its file was synthetic.
    pub fn location(&self, span: Span) -> String {
        match self.get(span.source()) {
            Some(file) => {
                let (line, column) = file.line_index().location(span.start());
                format!("{}:{}:{}", file.name(), line, column)
            }
            None => "<synthetic>".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn span_measures_its_own_range() {
        let span = Span::new(SourceId::from_raw(0), 3, 9);
        assert_eq!(span.len(), 6);
        assert_eq!(span.range(), 3..9);
        assert!(!span.is_empty());
    }

    #[test]
    fn point_span_is_empty_but_contains_its_offset() {
        let span = Span::point(SourceId::from_raw(0), 5);
        assert!(span.is_empty());
        assert_eq!(span.len(), 0);
        // An empty span still contains the offset it points at, so a missing
        // operand can be underlined rather than having nowhere to point.
        assert!(span.contains(5));
        assert!(!span.contains(4));
        assert!(!span.contains(6));
    }

    #[test]
    fn merging_spans_covers_both() {
        let a = Span::new(SourceId::from_raw(1), 2, 5);
        let b = Span::new(SourceId::from_raw(1), 8, 12);
        assert_eq!(a.to(b), Span::new(SourceId::from_raw(1), 2, 12));
        // Merging is order independent.
        assert_eq!(b.to(a), Span::new(SourceId::from_raw(1), 2, 12));
    }

    #[test]
    fn merging_across_files_keeps_the_first_span() {
        let a = Span::new(SourceId::from_raw(0), 2, 5);
        let b = Span::new(SourceId::from_raw(1), 8, 12);
        assert_eq!(a.to(b), a);
        assert_eq!(b.to(a), b);
    }

    #[test]
    fn touch_detection_respects_file_boundaries() {
        let a = Span::new(SourceId::from_raw(0), 0, 5);
        let adjacent = Span::new(SourceId::from_raw(0), 5, 9);
        let other_file = Span::new(SourceId::from_raw(1), 5, 9);
        assert!(a.touches(adjacent));
        assert!(a.to(adjacent) == Span::new(SourceId::from_raw(0), 0, 9));
        assert!(!a.touches(other_file));
    }

    #[test]
    fn line_index_finds_line_starts() {
        let index = LineIndex::new("alpha\nbeta\ngamma");
        assert_eq!(index.line_count(), 3);
        assert_eq!(index.line_start(1), 0);
        assert_eq!(index.line_start(2), 6);
        assert_eq!(index.line_start(3), 11);
    }

    #[test]
    fn line_index_counts_a_trailing_newline_as_an_empty_final_line() {
        let index = LineIndex::new("alpha\n");
        assert_eq!(index.line_count(), 2);
        assert_eq!(index.line_start(2), 6);
    }

    #[test]
    fn location_is_one_based() {
        let index = LineIndex::new("alpha\nbeta\ngamma");
        assert_eq!(index.location(0), (1, 1));
        assert_eq!(index.location(5), (1, 6));
        assert_eq!(index.location(6), (2, 1));
        assert_eq!(index.location(11), (3, 1));
    }

    #[test]
    fn out_of_range_offsets_and_lines_clamp_instead_of_panicking() {
        // A span from a stale or truncated parse must degrade into a readable
        // diagnostic, never a crash.
        let index = LineIndex::new("alpha\nbeta");
        assert_eq!(index.line_start(99), 6);

        // An offset past the end clamps to end-of-file, giving the last line and
        // a column just past its last byte, rather than a column in the
        // billions.
        let (line, column) = index.location(u32::MAX);
        assert_eq!(line, 2);
        assert_eq!(column, 5);

        // Within range, nothing is clamped.
        assert_eq!(index.location(0), (1, 1));
        assert_eq!(index.location(6), (2, 1));
    }

    #[test]
    fn empty_file_has_a_valid_location() {
        let index = LineIndex::new("");
        assert_eq!(index.line_count(), 1);
        assert_eq!(index.location(0), (1, 1));
        assert_eq!(index.location(u32::MAX), (1, 1));
    }

    #[test]
    fn source_map_resolves_locations_and_text() {
        let mut map = SourceMap::new();
        let id = map.add("main.py", "x = 1\ny = 2\n");
        assert_eq!(map.len(), 1);

        let span = Span::new(id, 6, 7);
        assert_eq!(map.location(span), "main.py:2:1");
        assert_eq!(map.get(id).unwrap().slice(span), Some("y"));

        let second = map.add("other.py", "print(1)");
        assert_eq!(map.location(Span::new(second, 0, 1)), "other.py:1:1");
    }

    #[test]
    fn synthetic_spans_resolve_to_a_placeholder_not_a_file() {
        let map = SourceMap::new();
        let span = Span::synthetic();
        assert!(span.is_synthetic());
        assert!(map.get(SourceId::SYNTHETIC).is_none());
        assert_eq!(map.location(span), "<synthetic>");
    }

    #[test]
    fn slice_rejects_out_of_bounds_spans_instead_of_panicking() {
        let mut map = SourceMap::new();
        let id = map.add("short.py", "ab");
        let beyond = Span::new(id, 0, 999);
        assert_eq!(map.get(id).unwrap().slice(beyond), None);
        // And a span that lands mid-codepoint is also refused rather than
        // producing invalid UTF-8.
        let utf8 = map.add("emoji.py", "🎉");
        assert_eq!(map.get(utf8).unwrap().slice(Span::new(utf8, 0, 1)), None);
    }
}
