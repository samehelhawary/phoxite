//! A plain text buffer edited in line / UTF-16 column coordinates: what a
//! front end without its own buffers (the LSP server, tests) keeps open
//! documents in.
//!
//! Line endings are normalized to `\n`, as the engine expects. LSP counts
//! `\r\n` as one line break, so positions are unaffected.

use super::{IdeError, IdeResult, Pos, Range, TextEdit};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Document {
    text: String,
    /// Byte offset of the start of each line.
    lines: Vec<usize>,
}

impl Document {
    pub fn new(text: &str) -> Self {
        let mut document = Self { text: normalize(text), lines: Vec::new() };
        document.reindex();
        document
    }

    pub fn text(&self) -> String {
        self.text.clone()
    }

    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// Replaces the whole text.
    pub fn set_text(&mut self, text: &str) {
        self.text = normalize(text);
        self.reindex();
    }

    fn reindex(&mut self) {
        self.lines.clear();
        self.lines.push(0);
        self.lines.extend(self.text.match_indices('\n').map(|(i, _)| i + 1));
    }

    /// The byte offset of `pos`, clamped to its line and to the text.
    pub fn offset(&self, pos: Pos) -> usize {
        let Some(&start) = self.lines.get(pos.line as usize) else { return self.text.len() };
        let end = self.lines.get(pos.line as usize + 1).map_or(self.text.len(), |&next| next - 1);
        let line = &self.text[start..end];
        let mut units = 0u32;
        for (i, c) in line.char_indices() {
            if units >= pos.col {
                return start + i;
            }
            units += c.len_utf16() as u32;
        }
        start + line.len()
    }

    /// The position of byte offset `offset`.
    pub fn pos(&self, offset: usize) -> Pos {
        let offset = offset.min(self.text.len());
        let line = self.lines.partition_point(|&start| start <= offset) - 1;
        let start = self.lines[line];
        let col = self.text[start..offset].chars().map(|c| c.len_utf16() as u32).sum();
        Pos::new(line as u32, col)
    }

    /// Replaces `range` with `text` (an LSP incremental change).
    pub fn replace(&mut self, range: Range, text: &str) {
        let (from, to) = (self.offset(range.start), self.offset(range.end));
        self.text.replace_range(from..to.max(from), &normalize(text));
        self.reindex();
    }

    /// Applies edits expressed in the coordinates of the current text. They
    /// must not overlap; edits at one position apply in order.
    pub fn apply_edits(&mut self, edits: &[TextEdit]) -> IdeResult<()> {
        let mut items = Vec::with_capacity(edits.len());
        for (i, e) in edits.iter().enumerate() {
            if e.range.start > e.range.end {
                return Err(IdeError::invalid(format!("edit {i} has an inverted range")));
            }
            items.push((self.offset(e.range.start), self.offset(e.range.end), i));
        }
        items.sort_unstable();
        let mut prev_to = 0;
        for &(from, to, i) in &items {
            if from < prev_to {
                return Err(IdeError::invalid(format!("edit {i} overlaps another edit")));
            }
            prev_to = to;
        }
        let mut out = String::with_capacity(self.text.len());
        let mut at = 0;
        for &(from, to, i) in &items {
            out.push_str(&self.text[at..from]);
            out.push_str(&normalize(&edits[i].new_text));
            at = to;
        }
        out.push_str(&self.text[at..]);
        self.text = out;
        self.reindex();
        Ok(())
    }
}

fn normalize(text: &str) -> String {
    if text.contains('\r') { text.replace("\r\n", "\n") } else { text.to_owned() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edits_apply_in_utf16_columns() {
        let mut d = Document::new("a😀b\r\nline two\n");
        assert_eq!(d.offset(Pos::new(0, 3)), 5);
        assert_eq!(d.pos(5), Pos::new(0, 3));
        assert_eq!(d.offset(Pos::new(0, 99)), 6, "clamped to the line");
        let at = |l, c| Pos::new(l, c);
        d.apply_edits(&[
            TextEdit { range: Range::new(at(1, 0), at(1, 4)), new_text: "LINE".into() },
            TextEdit { range: Range::point(at(0, 3)), new_text: "_".into() },
        ])
        .unwrap();
        assert_eq!(d.text(), "a😀_b\nLINE two\n");
        d.replace(Range::new(at(2, 0), at(2, 0)), "end");
        assert_eq!(d.as_str(), "a😀_b\nLINE two\nend");
        assert!(
            d.apply_edits(&[
                TextEdit { range: Range::new(at(0, 0), at(0, 2)), new_text: String::new() },
                TextEdit { range: Range::new(at(0, 1), at(0, 3)), new_text: String::new() },
            ])
            .is_err()
        );
    }
}
