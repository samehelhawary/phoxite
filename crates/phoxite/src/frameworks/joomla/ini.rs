//! Joomla's language files (`language/en-GB/com_content.ini`): one
//! `KEY="value"` a line, `;` starting a comment. [`read`] gives each key
//! with its value and where the key is written.
//!
//! A value is between double quotes, a quote inside it written `\"`; a file
//! of an older extension may leave the quotes out. A line that isn't a
//! `KEY=` is skipped, so a file being edited reads as far as it makes sense.

use crate::base::Range;
use crate::syntax::LineIndex;

/// A language constant of a file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub key: String,
    pub value: String,
    /// Where the key is written.
    pub range: Range,
}

/// Whether `name` can be a language constant: capitals, digits and
/// underscores, starting with a letter.
pub fn is_constant(name: &str) -> bool {
    name.len() > 1
        && name.starts_with(|c: char| c.is_ascii_uppercase())
        && name.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
}

/// The constants of the language file `text`.
pub fn read(text: &str) -> Vec<Entry> {
    let lines = LineIndex::new(text);
    let mut out = Vec::new();
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        let start = offset;
        offset += line.len();
        let written = line.trim_end_matches(['\n', '\r']);
        let indent = written.len() - written.trim_start().len();
        let rest = &written[indent..];
        if rest.starts_with([';', '#', '[']) {
            continue;
        }
        let Some(equals) = rest.find('=') else { continue };
        let key = rest[..equals].trim_end();
        if key.is_empty() || !key.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-')) {
            continue;
        }
        let raw = rest[equals + 1..].trim();
        let value = match raw.strip_prefix('"') {
            Some(quoted) => {
                // To the last quote that isn't escaped.
                let mut end = quoted.len();
                let mut escaped = false;
                for (at, c) in quoted.char_indices() {
                    match c {
                        '\\' => escaped = !escaped,
                        '"' if !escaped => {
                            end = at;
                            break;
                        }
                        _ => escaped = false,
                    }
                }
                quoted[..end].replace("\\\"", "\"")
            }
            None => raw.to_owned(),
        };
        let from = start + indent;
        out.push(Entry {
            key: key.to_ascii_uppercase(),
            value,
            range: lines.range(from as u32, (from + key.len()) as u32),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_language_file_has_its_constants() {
        let text = "; A comment\n\nCOM_SHOP=\"Shop\"\n  COM_SHOP_HITS = \"Hits: %d\"\nCOM_SHOP_QUOTE=\"Say \\\"hi\\\" now\"\nPLAIN=No quotes\nnot a line\n[section]\nCOM_SHOP_OPEN=\"never closed\n";
        let found = read(text);
        let pairs: Vec<(&str, &str)> = found.iter().map(|entry| (entry.key.as_str(), entry.value.as_str())).collect();
        assert_eq!(
            pairs,
            [
                ("COM_SHOP", "Shop"),
                ("COM_SHOP_HITS", "Hits: %d"),
                ("COM_SHOP_QUOTE", "Say \"hi\" now"),
                ("PLAIN", "No quotes"),
                ("COM_SHOP_OPEN", "never closed"),
            ]
        );
        assert_eq!((found[1].range.start.line, found[1].range.start.col, found[1].range.end.col), (3, 2, 15));
        assert!(is_constant("COM_SHOP_HITS") && is_constant("JYES") && !is_constant("Shop") && !is_constant("J"));
    }
}
