//! Semantic tokens of a Twig template, for editors that colour by them
//! (one without a Twig grammar gets its Twig coloured this way).
//!
//! The names inside `{{ }}` and `{% %}` are told apart by what is around
//! them: a filter after `|`, a test after `is`, a function before `(`, a
//! property or a method after `.`, the key of a hash before `:`, else a
//! variable. Tags, operators and literals are the grammar's, and so is
//! everything outside the tags; comments and the body of a `verbatim` block
//! have no token.

use crate::semantic::{SemanticToken, TokenType};
use crate::syntax::LineIndex;

/// Words that are operators or literals, not names.
const WORDS: &[&str] = &[
    "and",
    "or",
    "xor",
    "not",
    "in",
    "is",
    "matches",
    "starts",
    "ends",
    "with",
    "has",
    "some",
    "every",
    "same",
    "as",
    "divisible",
    "by",
    "true",
    "false",
    "null",
    "none",
    "if",
    "else",
    "import",
    "from",
    "only",
    "ignore",
    "missing",
];

/// The tokens of `text`, in document order.
pub fn semantic_tokens(text: &str) -> Vec<SemanticToken> {
    let index = LineIndex::new(text);
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut at = 0;
    while at + 1 < bytes.len() {
        let opener = &bytes[at..at + 2];
        if opener == b"{#" {
            at = text[at..].find("#}").map_or(bytes.len(), |end| at + end + 2);
        } else if opener == b"{{" || opener == b"{%" {
            let statement = opener == b"{%";
            let end = tag(text, at + 2, statement, &index, &mut out);
            // The body of `{% verbatim %}` isn't Twig.
            let name = first_word(&text[at + 2..end]);
            at = end;
            if statement && matches!(name, "verbatim" | "raw") {
                let closing = format!("end{name}");
                while let Some(found) = text[at..].find("{%") {
                    let start = at + found;
                    if first_word(&text[start + 2..]) == closing {
                        at = start;
                        break;
                    }
                    at = start + 2;
                }
            }
        } else {
            at += 1;
        }
    }
    out
}

fn first_word(inner: &str) -> &str {
    let inner = inner.trim_start_matches(['-', '~']).trim_start();
    let end = inner.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).unwrap_or(inner.len());
    &inner[..end]
}

fn is_word(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// Tokenizes the tag whose inside starts at `from`; the byte after its
/// closing delimiter (the end of the text when it isn't closed).
fn tag(text: &str, from: usize, statement: bool, index: &LineIndex, out: &mut Vec<SemanticToken>) -> usize {
    let bytes = text.as_bytes();
    let close: &[u8] = if statement { b"%}" } else { b"}}" };
    let mut at = from;
    // The first word of a statement is its tag.
    let mut tag_name = statement;
    // After `is` (and `is not`): a test.
    let mut test = false;
    let mut braces = 0usize;
    while at < bytes.len() {
        let byte = bytes[at];
        if bytes[at..].starts_with(close) && !(close == b"}}" && braces > 0) {
            return at + 2;
        }
        if byte == b'\'' || byte == b'"' {
            at += 1;
            while at < bytes.len() && bytes[at] != byte {
                if bytes[at] == b'\\' {
                    at += 1;
                }
                at += 1;
            }
            at += 1;
            test = false;
            continue;
        }
        if byte == b'{' {
            braces += 1;
        } else if byte == b'}' {
            braces = braces.saturating_sub(1);
        }
        if !(byte.is_ascii_alphabetic() || byte == b'_') {
            // A number's letters (`1e3`) aren't a name.
            if byte.is_ascii_digit() {
                while at < bytes.len() && (is_word(bytes[at]) || bytes[at] == b'.') {
                    at += 1;
                }
                continue;
            }
            at += 1;
            continue;
        }
        let start = at;
        while at < bytes.len() && is_word(bytes[at]) {
            at += 1;
        }
        let word = &text[start..at];
        if std::mem::take(&mut tag_name) {
            continue;
        }
        let before = text[..start].trim_end().as_bytes().last().copied();
        let after = text[at..].trim_start().as_bytes().first().copied();
        let ty = if before == Some(b'|') {
            Some(TokenType::Function)
        } else if before == Some(b'.') {
            Some(if after == Some(b'(') { TokenType::Method } else { TokenType::Property })
        } else if test && word != "not" {
            Some(TokenType::Function)
        } else if WORDS.contains(&word) {
            None
        } else if after == Some(b'(') {
            Some(TokenType::Function)
        } else if braces > 0 && after == Some(b':') {
            Some(TokenType::Property)
        } else {
            Some(TokenType::Variable)
        };
        test = word == "is" || (test && word == "not");
        if let Some(ty) = ty {
            out.push(SemanticToken { start: index.pos(start as u32), len: (at - start) as u32, ty, modifiers: 0 });
        }
    }
    bytes.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The tokens of `text` as `(the text, its type)`.
    fn tokens(text: &str) -> Vec<(String, &'static str)> {
        let index = LineIndex::new(text);
        semantic_tokens(text)
            .into_iter()
            .map(|token| {
                let start = index.byte(token.start) as usize;
                (text[start..start + token.len as usize].to_owned(), token.ty.as_str())
            })
            .collect()
    }

    fn token(text: &str, ty: &'static str) -> (String, &'static str) {
        (text.to_owned(), ty)
    }

    #[test]
    fn names_are_told_apart_by_what_is_around_them() {
        assert_eq!(
            tokens("<p>{{ post.title|upper }}</p>"),
            [token("post", "variable"), token("title", "property"), token("upper", "function"),]
        );
        assert_eq!(
            tokens("{{ path('home', { page: n }) ~ user.name() }}"),
            [
                token("path", "function"),
                token("page", "property"),
                token("n", "variable"),
                token("user", "variable"),
                token("name", "method"),
            ]
        );
    }

    #[test]
    fn a_statements_tag_and_the_operators_are_not_names() {
        assert_eq!(
            tokens("{% for item in items if item.on is not empty and not done %}"),
            [
                token("item", "variable"),
                token("items", "variable"),
                token("item", "variable"),
                token("on", "property"),
                token("empty", "function"),
                token("done", "variable"),
            ]
        );
        assert_eq!(
            tokens("{%- set total = 1e3 + count -%}{% endif %}"),
            [token("total", "variable"), token("count", "variable"),]
        );
    }

    #[test]
    fn strings_comments_text_and_verbatim_bodies_have_no_token() {
        assert_eq!(tokens("text {# {{ a }} #} {{ 'b }} c' }} more"), []);
        assert_eq!(tokens("{% verbatim %}{{ a }}{% endverbatim %}{{ b }}"), [token("b", "variable")]);
        // A tag that isn't closed ends with the text.
        assert_eq!(tokens("{{ a"), [token("a", "variable")]);
    }

    #[test]
    fn positions_are_lines_and_columns() {
        let found = semantic_tokens("x\n  {{ name }}");
        assert_eq!(found.len(), 1);
        assert_eq!((found[0].start.line, found[0].start.col, found[0].len), (1, 5, 4));
    }
}
