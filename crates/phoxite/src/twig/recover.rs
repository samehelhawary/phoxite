//! Error recovery on top of `mago-twig-syntax`, whose parser keeps no
//! statements at all when a tag fails to parse (only errors it can defer
//! leave the rest of the template). Editors work on broken templates all
//! the time, so before parsing:
//!
//! 1. the blocks left open (`{% if %}` without `{% endif %}`, …) are closed
//!    by end tags appended after the text, and
//! 2. while the parse fails, the tag at the first error is blanked out
//!    (turned into spaces, newlines kept) and the text parsed again.
//!
//! Offsets and lines of the original text don't move: blanking keeps the
//! length, and the end tags go after the last byte.

use std::borrow::Cow;

use mago_allocator::LocalArena;
use mago_database::file::FileId;
use mago_span::HasSpan;
use mago_twig_syntax::parser::parse_file_content;

/// The most tags blanked before giving up (the template then has no statements).
const MAX_REPAIRS: usize = 16;

/// Tags with a body closed by `end<tag>`.
const BLOCK_TAGS: &[&str] =
    &["if", "for", "block", "macro", "embed", "with", "apply", "autoescape", "sandbox", "guard", "cache", "set"];

/// `text`, repaired so that it parses (see the module docs).
pub(super) fn recovered(text: &str) -> Cow<'_, str> {
    let mut current = Cow::Borrowed(text);
    let closing = missing_end_tags(text);
    if !closing.is_empty() {
        current = Cow::Owned(format!("{text}{closing}"));
    }
    for _ in 0..MAX_REPAIRS {
        let Some(at) = fatal_error(&current) else { break };
        let Some((start, end)) = tag_around(&current, at) else { break };
        if current[start..end].trim().is_empty() {
            break;
        }
        current = Cow::Owned(blank(&current, start, end));
    }
    current
}

/// Where the first error is, when the parse kept no statement.
fn fatal_error(text: &str) -> Option<usize> {
    crate::syntax::stack::parser_stack(|| {
        let arena = LocalArena::new();
        let template = parse_file_content(&arena, FileId::zero(), text.as_bytes());
        if !template.statements.is_empty() {
            return None;
        }
        let first = template.errors.iter().map(|e| e.span().start.offset).min()?;
        Some(first as usize)
    })
}

const OPENERS: [&str; 3] = ["{{", "{%", "{#"];

fn closer(opener: &str) -> &'static str {
    match opener {
        "{{" => "}}",
        "{%" => "%}",
        _ => "#}",
    }
}

/// The tag an error at byte `at` belongs to: from the last delimiter opening
/// at or before it to its closing delimiter (or up to the next tag, or the
/// end, when it isn't closed).
fn tag_around(text: &str, at: usize) -> Option<(usize, usize)> {
    let mut limit = (at + 2).min(text.len());
    while !text.is_char_boundary(limit) {
        limit -= 1;
    }
    let head = &text[..limit];
    let (start, opener) = OPENERS.iter().filter_map(|o| head.rfind(o).map(|at| (at, *o))).max_by_key(|(at, _)| *at)?;
    let rest = &text[start + 2..];
    let next_open = OPENERS.iter().filter_map(|o| rest.find(o)).min();
    let end = match rest.find(closer(opener)) {
        Some(close) if next_open.is_none_or(|next| close < next) => start + 2 + close + 2,
        _ => next_open.map_or(text.len(), |next| start + 2 + next),
    };
    Some((start, end))
}

/// End tags closing the blocks `text` leaves open, innermost first.
fn missing_end_tags(text: &str) -> String {
    open_blocks(text).iter().rev().map(|(name, _)| format!("{{% end{name} %}}")).collect()
}

/// The blocks `text` leaves open, outermost first: the tag's name and the
/// byte its `{%` is at.
pub(super) fn open_blocks(text: &str) -> Vec<(String, usize)> {
    scan_blocks(text).open
}

/// What [`scan_blocks`] finds.
#[derive(Default)]
struct Blocks {
    /// The blocks still open at the end, outermost first: the tag's name
    /// and the byte its `{%` is at.
    open: Vec<(String, usize)>,
    /// The blocks an end tag of a block around them closed over
    /// (`{% block b %}{% if a %}{% endblock %}`: the `if`).
    abandoned: Vec<(String, usize)>,
    /// Where those end tags are.
    closers: Vec<usize>,
}

fn scan_blocks(text: &str) -> Blocks {
    let mut found = Blocks::default();
    let mut open: Vec<(String, usize)> = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find("{%") {
        let tag_at = text.len() - rest.len() + at;
        let after = &rest[at + 2..];
        let Some(close) = after.find("%}") else { break };
        let inner = after[..close].trim_matches(|c: char| c == '-' || c == '~' || c.is_whitespace());
        let name: String = inner.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
        let args = inner[name.len()..].trim();
        rest = &after[close + 2..];
        if matches!(name.as_str(), "verbatim" | "raw") {
            // The body isn't Twig: skip to its end tag.
            let end = format!("end{name}");
            match rest.find(&end) {
                Some(at) => rest = &rest[at..],
                None => open.push((name, tag_at)),
            }
            continue;
        }
        if let Some(ended) = name.strip_prefix("end") {
            if let Some(at) = open.iter().rposition(|(o, _)| o == ended) {
                if at + 1 < open.len() {
                    found.abandoned.extend(open.drain(at + 1..));
                    found.closers.push(tag_at);
                }
                open.truncate(at);
            }
            continue;
        }
        let has_body = BLOCK_TAGS.contains(&name.as_str())
            && match name.as_str() {
                // `{% set x = 1 %}` and `{% block title 'x' %}` have no body.
                "set" => !args.contains('='),
                "block" => !args.contains(char::is_whitespace),
                _ => true,
            };
        if has_body {
            open.push((name, tag_at));
        }
    }
    found.open = open;
    found
}

/// A syntax error of a template: the byte range it is at and what the
/// parser says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SyntaxProblem {
    pub start: usize,
    pub end: usize,
    pub message: String,
}

/// The syntax errors of `text`: the blocks it leaves open, then what the
/// parser reports, each broken tag blanked in turn so that the errors after
/// it are found too (at most [`MAX_REPAIRS`]).
///
/// A tag the parser doesn't know isn't an error: extensions add tags
/// (`{% trans %}`, `{% form_theme %}`), and which ones a project has can't
/// be told from the template.
pub(super) fn syntax_problems(text: &str) -> Vec<SyntaxProblem> {
    let mut out: Vec<SyntaxProblem> = Vec::new();
    let Blocks { open, abandoned, closers } = scan_blocks(text);
    for (name, at) in open.iter().chain(&abandoned) {
        let end = text[*at..].find("%}").map_or(text.len(), |close| at + close + 2);
        out.push(SyntaxProblem { start: *at, end, message: format!("Unclosed '{name}' block") });
    }
    let closing: String = open.iter().rev().map(|(name, _)| format!("{{% end{name} %}}")).collect();
    let mut current = format!("{text}{closing}");
    // Tags extensions add, whose end tags aren't errors either.
    let mut unknown: Vec<String> = Vec::new();
    for _ in 0..MAX_REPAIRS {
        let Some((errors, fatal)) = parse_errors(&current) else { break };
        let first = errors.iter().map(|error| error.0).min();
        for (start, end, message, unknown_tag) in errors {
            let Some((tag_start, tag_end)) = tag_around(&current, start) else { continue };
            let name = tag_name(&current[tag_start..tag_end]);
            if unknown_tag {
                if !unknown.contains(&name) {
                    unknown.push(name);
                }
                continue;
            }
            if name.strip_prefix("end").is_some_and(|ended| unknown.iter().any(|known| known == ended)) {
                continue;
            }
            // Errors in the end tags appended above are the open blocks',
            // and so is the one at an end tag that closed over a block.
            if start >= text.len() || closers.contains(&tag_start) {
                continue;
            }
            let end = end.clamp(start, text.len());
            // An error without a width marks its tag.
            let (start, end) = if end > start { (start, end) } else { (tag_start, tag_end.min(text.len())) };
            if !out.iter().any(|known| known.start == start && known.message == message) {
                out.push(SyntaxProblem { start, end, message });
            }
        }
        if !fatal {
            break;
        }
        let Some((start, end)) = first.and_then(|at| tag_around(&current, at)) else { break };
        if current[start..end].trim().is_empty() {
            break;
        }
        current = blank(&current, start, end);
    }
    out.sort_by_key(|problem| problem.start);
    out
}

/// The first word of a tag (`{%- endif %}` → `endif`).
fn tag_name(tag: &str) -> String {
    tag.trim_start_matches(['{', '%', '-', '~'])
        .trim_start()
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect()
}

/// One error of a parse: `(start, end, message, an unknown tag)`.
type ParsedError = (usize, usize, String, bool);

/// The errors of one parse as `(start, end, message, an unknown tag)`, and
/// whether the parse kept no statement; `None` when there is no error.
fn parse_errors(text: &str) -> Option<(Vec<ParsedError>, bool)> {
    use mago_twig_syntax::error::ParseError;
    crate::syntax::stack::parser_stack(|| {
        let arena = LocalArena::new();
        let template = parse_file_content(&arena, FileId::zero(), text.as_bytes());
        if template.errors.is_empty() {
            return None;
        }
        let errors = template
            .errors
            .iter()
            .map(|error| {
                let span = error.span();
                let unknown = matches!(error, ParseError::UnknownTag(..));
                (span.start.offset as usize, span.end.offset as usize, error.to_string(), unknown)
            })
            .collect();
        Some((errors, template.statements.is_empty()))
    })
}

/// `text` with the bytes `start..end` turned into spaces (newlines kept, so
/// offsets and lines don't move).
pub(super) fn blank(text: &str, start: usize, end: usize) -> String {
    let end = end.min(text.len());
    let mut out = String::with_capacity(text.len());
    for (at, c) in text.char_indices() {
        if at >= start && at < end && c != '\n' && c != '\r' {
            out.extend(std::iter::repeat_n(' ', c.len_utf8()));
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closes_open_blocks_innermost_first() {
        assert_eq!(missing_end_tags("{% if a %}{% for x in y %}{% set z %}"), "{% endset %}{% endfor %}{% endif %}");
        assert_eq!(missing_end_tags("{% if a %}{% endif %}{% set z = 1 %}{% block t 'x' %}"), "");
        assert_eq!(missing_end_tags("{% block b %}{% verbatim %}{% if %}{% endverbatim %}"), "{% endblock %}");
        assert_eq!(missing_end_tags("{%- macro m() -%}{{ x }}"), "{% endmacro %}");
    }

    #[test]
    fn blanks_the_broken_tag_and_keeps_offsets() {
        let text = "{% set a = 1 %}\n{{ a| }}\n{% if %}é";
        let fixed = recovered(text);
        assert_eq!(fixed.len(), text.len() + "{% endif %}".len());
        assert!(fixed.starts_with("{% set a = 1 %}\n"));
        assert!(!fixed.contains("{{"));
        assert_eq!(fixed.lines().count(), text.lines().count());
        assert!(fatal_error(&fixed).is_none());
    }

    #[test]
    fn valid_templates_stay_borrowed() {
        assert!(matches!(recovered("{% if a %}{{ b }}{% endif %}"), Cow::Borrowed(_)));
    }

    #[test]
    fn tag_around_handles_unclosed_tags() {
        assert_eq!(tag_around("ab {{ x {% y %}", 6), Some((3, 8)));
        assert_eq!(tag_around("ab {% x ", 8), Some((3, 8)));
        assert_eq!(tag_around("no tags", 3), None);
    }
}
