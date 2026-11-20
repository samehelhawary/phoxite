//! Formatting of Twig templates: the indentation of every line from the
//! nesting of HTML elements and Twig blocks (`{% if %}` … `{% else %}` …
//! `{% endif %}`, `{% for %}`, `{% block %}`, a tag an extension adds when
//! its `end…` tag is there), and the spacing inside the tags, as Twig's
//! coding standards put it. Nothing is rewrapped or moved to another line.
//!
//! Inside a tag written on one line:
//! - one space after the opening delimiter and before the closing one
//!   (`{{x}}` → `{{ x }}`, `{%-if a-%}` → `{%- if a -%}`, `{#x#}` → `{# x #}`);
//! - no space around the `|` of a filter;
//! - one space where several are written.
//!
//! Lines:
//! - The content of `<script>` and `<style>` is indented by its brackets
//!   (`{`, `(`, `[`), one level inside its tag; a line continuing a chain
//!   (`.then(…)`, `&& …`) goes one level further. Where that can't be done
//!   safely (a template literal, a string or a comment left open at the end
//!   of a line, brackets that don't balance) the content keeps its own
//!   indentation, shifted as a whole. The code itself isn't reformatted.
//! - `<pre>`, `<textarea>`, `{% verbatim %}`, comments (`{# #}`, `<!-- -->`)
//!   and Twig tags written over several lines keep every line they span as
//!   written.
//! - The attribute lines of an HTML tag written over several lines go one
//!   level in (a line starting with `>` or `/>` stays at the tag's level).
//! - Trailing whitespace goes.
//!
//! When every branch of an `{% if %}` that ends in `{% else %}` opens the same
//! elements (`<div class="a">` in one, `<div>` in the other), they are open
//! after `{% endif %}`, and what follows nests in them until their closing
//! tags. An element opened in one block and closed in another doesn't nest.
//!
//! Only whitespace changes: should the result differ from the template in
//! anything else, the template is returned as it was.

use super::builtins;
use crate::base::TextEdit;

/// The editor's indentation.
#[derive(Clone, Copy, Debug)]
pub struct TwigFormatOptions {
    pub indent: u32,
    pub use_tabs: bool,
}

impl Default for TwigFormatOptions {
    fn default() -> Self {
        Self { indent: 4, use_tabs: false }
    }
}

/// The edits formatting `text`, one per changed line; in the lines `within`
/// (first and last, 0-based) when given.
pub fn format(text: &str, options: &TwigFormatOptions, within: Option<(u32, u32)>) -> Vec<TextEdit> {
    crate::blade::format::edits(text, &format_text(text, options), within)
}

/// `text` formatted; see the module documentation.
pub fn format_text(text: &str, options: &TwigFormatOptions) -> String {
    let spaced = spaced_tags(text);
    let formatted = indented(&spaced, options);
    let visible = |text: &str| text.chars().filter(|c| !c.is_whitespace()).collect::<String>();
    // The spacing of tags only adds and removes spaces; so does indenting.
    if visible(&formatted) == visible(text) { formatted } else { text.to_owned() }
}

// ---------------------------------------------------------------------------
// The spacing inside tags
// ---------------------------------------------------------------------------

/// `text` with the spacing inside its one-line tags put right.
fn spaced_tags(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut copied = 0;
    let mut at = 0;
    while let Some((start, opener)) = next_opener(text, at) {
        let Some(end) = tag_end(text, start, opener) else { break };
        let tag = &text[start..end];
        if let Some(formatted) = format_tag(tag, opener) {
            out.push_str(&text[copied..start]);
            out.push_str(&formatted);
            copied = end;
        }
        at = end;
        // The body of `{% verbatim %}` isn't Twig.
        if opener == "{%"
            && let Some(name) = ["verbatim", "raw"].iter().find(|name| first_word(tag) == **name)
        {
            at = closing_tag(text, end, &format!("end{name}")).unwrap_or(text.len());
        }
    }
    out.push_str(&text[copied..]);
    out
}

const OPENERS: [&str; 3] = ["{{", "{%", "{#"];

fn closer(opener: &str) -> &'static str {
    match opener {
        "{{" => "}}",
        "{%" => "%}",
        _ => "#}",
    }
}

fn next_opener(text: &str, from: usize) -> Option<(usize, &'static str)> {
    OPENERS
        .iter()
        .filter_map(|opener| text[from..].find(opener).map(|at| (from + at, *opener)))
        .min_by_key(|(at, _)| *at)
}

/// The byte after the closing delimiter of the tag opening at `start`;
/// strings are skipped (a `}}` inside one closes nothing).
fn tag_end(text: &str, start: usize, opener: &str) -> Option<usize> {
    let close = closer(opener);
    let bytes = text.as_bytes();
    if opener == "{#" {
        return text[start + 2..].find(close).map(|at| start + 2 + at + 2);
    }
    let mut at = start + 2;
    let mut quote: Option<u8> = None;
    while at < bytes.len() {
        let byte = bytes[at];
        match quote {
            Some(_) if byte == b'\\' => at += 1,
            Some(open) if byte == open => quote = None,
            Some(_) => {}
            None if byte == b'\'' || byte == b'"' => quote = Some(byte),
            None if bytes[at..].starts_with(close.as_bytes()) => return Some(at + 2),
            None => {}
        }
        at += 1;
    }
    None
}

/// The first word of a `{% %}` tag.
fn first_word(tag: &str) -> &str {
    let inner = tag[2..].trim_start_matches(['-', '~']).trim_start();
    let end = inner.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).unwrap_or(inner.len());
    &inner[..end]
}

/// Where the tag `{% <name> %}` starts, from byte `from` on.
fn closing_tag(text: &str, from: usize, name: &str) -> Option<usize> {
    let mut at = from;
    while let Some(found) = text[at..].find("{%") {
        let start = at + found;
        if first_word(&text[start..]) == name {
            return Some(start);
        }
        at = start + 2;
    }
    None
}

/// `tag` with its spacing put right; `None` for a tag left alone (written
/// over several lines).
fn format_tag(tag: &str, opener: &str) -> Option<String> {
    if tag.contains('\n') {
        return None;
    }
    let close = closer(opener);
    let mut inner = &tag[2..tag.len() - 2];
    if opener == "{#" {
        let inner = inner.trim();
        return Some(if inner.is_empty() { "{# #}".to_owned() } else { format!("{{# {inner} #}}") });
    }
    // The whitespace control characters stay against their delimiters.
    let mut open = opener.to_owned();
    if let Some(control) = inner.chars().next().filter(|c| matches!(c, '-' | '~')) {
        open.push(control);
        inner = &inner[1..];
    }
    let mut end = close.to_owned();
    if let Some(control) = inner.chars().last().filter(|c| matches!(c, '-' | '~'))
        // `{{ a - }}` isn't Twig, but `{{ -1 -}}` is: the last character controls whitespace.
        && !inner.trim_end_matches(['-', '~']).is_empty()
    {
        end.insert(0, control);
        inner = &inner[..inner.len() - 1];
    }
    let inner = spaced(inner);
    Some(if inner.is_empty() { format!("{open} {end}") } else { format!("{open} {inner} {end}") })
}

/// `code` with single spaces between its parts and none around a filter's
/// `|`; strings are kept as written.
fn spaced(code: &str) -> String {
    let mut out = String::with_capacity(code.len());
    let mut quote: Option<char> = None;
    let mut escaped = false;
    // A space is owed before the next thing written.
    let mut space = false;
    for c in code.trim().chars() {
        if let Some(open) = quote {
            out.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == open {
                quote = None;
            }
            continue;
        }
        if c.is_whitespace() {
            space = true;
            continue;
        }
        if c == '|' {
            // No space before it, none after it.
            out.push(c);
            space = false;
            continue;
        }
        if space && !out.ends_with('|') && !out.is_empty() {
            out.push(' ');
        }
        space = false;
        if c == '\'' || c == '"' {
            quote = Some(c);
        }
        out.push(c);
    }
    out
}

// ---------------------------------------------------------------------------
// Indentation
// ---------------------------------------------------------------------------

/// Twig tags with a body closed by `end<tag>`.
const BLOCK_TAGS: &[&str] =
    &["if", "for", "block", "macro", "embed", "with", "apply", "autoescape", "sandbox", "guard", "cache", "set"];

/// Branches inside a block: the line goes back a level.
const MIDDLES: &[&str] = &["else", "elseif"];

/// Elements without content (no closing tag).
const VOID: &[&str] =
    &["area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source", "track", "wbr"];

/// Elements whose next sibling of the same name closes them (`<li>` …).
const SELF_CLOSING_SIBLINGS: &[&str] = &["li", "option", "tr", "td", "th", "dt", "dd"];

/// Elements whose content is kept as written.
const PRESERVED: &[&str] = &["pre", "textarea"];

/// Elements whose content isn't HTML.
const EMBEDDED: &[&str] = &["script", "style"];

#[derive(Clone, Debug, PartialEq, Eq)]
enum Kind {
    /// What opens: its name, and whether it is a Twig block.
    Open(String, bool),
    Close(String, bool),
    /// `{% else %}` (plain) or `{% elseif %}`.
    Middle(bool),
}

#[derive(Clone, Debug)]
struct Event {
    at: usize,
    end: usize,
    kind: Kind,
}

/// How a run of lines is laid out (first and last line, inclusive).
#[derive(Clone, Copy, Debug)]
enum Region {
    /// Left as written.
    Keep(usize, usize),
    /// The content of a `<script>` or a `<style>` (CSS when the flag is
    /// set), one level inside the line of its opener.
    Shift(usize, usize, usize, bool),
    /// The attribute lines of a tag written over several lines; the line of
    /// the tag's start.
    Tag(usize, usize, usize),
}

/// The start of every line of `text`.
fn line_starts(text: &str) -> Vec<usize> {
    std::iter::once(0).chain(text.match_indices('\n').map(|(at, _)| at + 1)).collect()
}

fn line_of(starts: &[usize], offset: usize) -> usize {
    starts.partition_point(|start| *start <= offset).saturating_sub(1)
}

/// The byte after the `>` closing the HTML tag at `start`; quoted attribute
/// values and the Twig tags inside are skipped.
fn html_tag_end(text: &str, start: usize) -> usize {
    let bytes = text.as_bytes();
    let mut at = start + 1;
    let mut quote: Option<u8> = None;
    while at < bytes.len() {
        if let Some(opener) = OPENERS.iter().find(|opener| bytes[at..].starts_with(opener.as_bytes())) {
            at = tag_end(text, at, opener).unwrap_or(bytes.len());
            continue;
        }
        let byte = bytes[at];
        match quote {
            Some(open) if byte == open => quote = None,
            Some(_) => {}
            None if byte == b'"' || byte == b'\'' => quote = Some(byte),
            None if byte == b'>' => return at + 1,
            None => {}
        }
        at += 1;
    }
    bytes.len()
}

/// Where `</name` starts (any case), from byte `from` on.
fn html_closing(text: &str, from: usize, name: &str) -> Option<usize> {
    let lower = text[from..].to_ascii_lowercase();
    lower.find(&format!("</{name}")).map(|at| from + at)
}

fn html_name(text: &str) -> String {
    text.chars()
        .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | ':' | '_' | '.'))
        .collect::<String>()
        .to_ascii_lowercase()
}

/// What opens and closes in `text`, and the runs of lines with a layout of
/// their own.
fn analyse(text: &str, starts: &[usize]) -> (Vec<Event>, Vec<Region>) {
    let bytes = text.as_bytes();
    let line = |offset: usize| line_of(starts, offset.min(text.len().saturating_sub(1)));
    let mut events: Vec<Event> = Vec::new();
    let mut regions: Vec<Region> = Vec::new();
    // The lines after the first of something written over several lines.
    let keep_rest = |regions: &mut Vec<Region>, start: usize, end: usize| {
        let (first, last) = (line(start), line(end.saturating_sub(1)));
        if last > first {
            regions.push(Region::Keep(first + 1, last));
        }
    };
    let mut at = 0;
    while at < bytes.len() {
        let rest = &text[at..];
        if rest.starts_with("{#") {
            let end = tag_end(text, at, "{#").unwrap_or(text.len());
            keep_rest(&mut regions, at, end);
            at = end;
        } else if rest.starts_with("{{") {
            let end = tag_end(text, at, "{{").unwrap_or(text.len());
            keep_rest(&mut regions, at, end);
            at = end;
        } else if rest.starts_with("{%") {
            let end = tag_end(text, at, "{%").unwrap_or(text.len());
            keep_rest(&mut regions, at, end);
            let tag = &text[at..end];
            let name = first_word(tag).to_owned();
            let arguments = tag[2..].trim_start_matches(['-', '~']).trim_start()[name.len()..]
                .trim_end_matches(['%', '}', '-', '~'])
                .trim();
            at = end;
            if matches!(name.as_str(), "verbatim" | "raw") {
                // Its body isn't Twig, and stays as written.
                let closing = closing_tag(text, end, &format!("end{name}"));
                let stop = closing.unwrap_or(text.len());
                let (first, last) = (line(end.saturating_sub(1)), line(stop));
                // The line of the end tag is laid out when the tag starts it.
                let own_line = closing.is_some_and(|stop| text[starts[line(stop)]..stop].trim().is_empty());
                let last = if own_line { last.saturating_sub(1) } else { last };
                if last > first {
                    regions.push(Region::Keep(first + 1, last));
                }
                if closing.is_some() {
                    events.push(Event { at: end - tag.len(), end, kind: Kind::Open(name, true) });
                }
                at = stop;
            } else if let Some(ended) = name.strip_prefix("end").filter(|ended| !ended.is_empty()) {
                events.push(Event { at: end - tag.len(), end, kind: Kind::Close(ended.to_owned(), true) });
            } else if MIDDLES.contains(&name.as_str()) {
                events.push(Event { at: end - tag.len(), end, kind: Kind::Middle(name == "else") });
            } else {
                let known = builtins::TAGS.iter().any(|tag| tag.name == name);
                let has_body = if BLOCK_TAGS.contains(&name.as_str()) {
                    match name.as_str() {
                        // `{% set x = 1 %}` and `{% block title 'x' %}` have no body.
                        "set" => !arguments.contains('='),
                        "block" => !arguments.contains(char::is_whitespace),
                        _ => true,
                    }
                } else {
                    // A tag an extension adds has a body when its end tag is there.
                    !known && !name.is_empty() && closing_tag(text, end, &format!("end{name}")).is_some()
                };
                if has_body {
                    events.push(Event { at: end - tag.len(), end, kind: Kind::Open(name, true) });
                }
            }
        } else if rest.starts_with("<!--") {
            let end = rest.find("-->").map_or(text.len(), |found| at + found + 3);
            keep_rest(&mut regions, at, end);
            at = end;
        } else if rest.starts_with("</") && rest[2..].starts_with(|c: char| c.is_ascii_alphabetic()) {
            let name = html_name(&rest[2..]);
            let end = rest.find('>').map_or(text.len(), |found| at + found + 1);
            events.push(Event { at, end, kind: Kind::Close(name, false) });
            at = end;
        } else if rest.starts_with('<') && rest[1..].starts_with(|c: char| c.is_ascii_alphabetic()) {
            let name = html_name(&rest[1..]);
            let end = html_tag_end(text, at);
            let (first, last) = (line(at), line(end.saturating_sub(1)));
            if last > first {
                regions.push(Region::Tag(first + 1, last, first));
            }
            let self_closed = text[at..end].trim_end_matches('>').ends_with('/');
            let start = at;
            at = end;
            if PRESERVED.contains(&name.as_str()) {
                let stop = html_closing(text, end, &name).map_or(text.len(), |stop| html_tag_end(text, stop));
                let stop_line = line(stop.saturating_sub(1));
                if stop_line > last {
                    regions.push(Region::Keep(last + 1, stop_line));
                }
                at = stop;
            } else if EMBEDDED.contains(&name.as_str()) && !self_closed {
                let closing = html_closing(text, end, &name);
                let stop = closing.unwrap_or(text.len());
                let own_line = closing.is_some_and(|stop| text[starts[line(stop)]..stop].trim().is_empty());
                let stop_line = line(stop);
                let content_last = if own_line { stop_line.saturating_sub(1) } else { stop_line };
                if content_last > last {
                    regions.push(if own_line {
                        Region::Shift(last + 1, content_last, first, name == "style")
                    } else {
                        Region::Keep(last + 1, content_last)
                    });
                }
                if closing.is_some() {
                    events.push(Event { at: start, end, kind: Kind::Open(name, false) });
                }
                at = stop;
            } else if !self_closed && !VOID.contains(&name.as_str()) {
                events.push(Event { at: start, end, kind: Kind::Open(name, false) });
            }
        } else {
            at += rest.chars().next().map_or(1, char::len_utf8);
        }
    }
    (events, regions)
}

/// An open `{% if %}`: where it is among what is open, the elements each
/// branch finished so far left open (`None` for a branch that left a Twig
/// block open), and whether the branch being read is a plain `{% else %}`.
struct IfBlock {
    at: usize,
    branches: Vec<Option<Vec<String>>>,
    plain_else: bool,
}

/// What is open: each with its name and whether it is a Twig block.
#[derive(Default)]
struct Nest {
    open: Vec<(String, bool)>,
    ifs: Vec<IfBlock>,
}

impl Nest {
    fn len(&self) -> usize {
        self.open.len()
    }

    /// Whether `name` opening now closes the element before it (`<li>` after `<li>`).
    fn closes_sibling(&self, name: &str) -> bool {
        SELF_CLOSING_SIBLINGS.contains(&name) && self.open.last().is_some_and(|(open, twig)| !twig && open == name)
    }

    /// The elements open above index `at`; `None` when a Twig block is among them.
    fn above(&self, at: usize) -> Option<Vec<String>> {
        self.open[at + 1..].iter().map(|(name, twig)| (!twig).then(|| name.clone())).collect()
    }

    fn truncate(&mut self, at: usize) {
        self.open.truncate(at);
        self.ifs.retain(|block| block.at < at);
    }

    /// Opens, branches or closes; the level of the tag doing it (a closing
    /// tag and a branch are written at the level of what they close).
    fn apply(&mut self, kind: &Kind) -> usize {
        match kind {
            Kind::Open(name, twig) => {
                if !twig && self.closes_sibling(name) {
                    self.open.pop();
                }
                let level = self.open.len();
                if *twig && name == "if" {
                    self.ifs.push(IfBlock { at: self.open.len(), branches: Vec::new(), plain_else: false });
                }
                self.open.push((name.clone(), *twig));
                level
            }
            Kind::Middle(plain) => {
                // The `if` or `for` the branch belongs to: what the branch before left open goes.
                let Some(at) = self.open.iter().rposition(|(name, twig)| *twig && (name == "if" || name == "for"))
                else {
                    return self.open.len();
                };
                let left = self.above(at);
                self.truncate(at + 1);
                if let Some(block) = self.ifs.last_mut().filter(|block| block.at == at) {
                    block.branches.push(left);
                    block.plain_else = *plain;
                }
                at
            }
            Kind::Close(name, twig) => {
                let Some(at) = self.open.iter().rposition(|(open, open_twig)| open == name && open_twig == twig) else {
                    return self.open.len();
                };
                // The elements every branch of an `if … else` opened are open after it.
                let mut kept: Vec<String> = Vec::new();
                if *twig
                    && name == "if"
                    && let Some(block) = self.ifs.last().filter(|block| block.at == at && block.plain_else)
                {
                    let last = self.above(at);
                    let same = block.branches.iter().all(|branch| branch.is_some() && *branch == last);
                    if same {
                        kept = last.unwrap_or_default();
                    }
                }
                self.truncate(at);
                self.open.extend(kept.into_iter().map(|name| (name, false)));
                at
            }
        }
    }
}

/// The level of each line of the content of a `<script>` (or, with `css`, a
/// `<style>`), from its brackets; `None` for a line kept as written (inside
/// a comment over several lines). `None` as a whole when the code can't be
/// read safely: a template literal, a string left open at the end of a
/// line, a Twig tag over several lines, brackets that don't balance.
fn code_levels(lines: &[&str], css: bool) -> Option<Vec<Option<usize>>> {
    let mut depth: usize = 0;
    let mut in_comment = false;
    let mut out = Vec::with_capacity(lines.len());
    for line in lines {
        let bytes = line.as_bytes();
        let trimmed = line.trim_start();
        let closers = trimmed.bytes().take_while(|b| matches!(b, b'}' | b']' | b')')).count();
        // A line continuing the statement before it.
        let chained = ["?.", "&&", "||"].iter().any(|start| trimmed.starts_with(start))
            || (trimmed.starts_with('.') && !trimmed.starts_with("..."));
        let hangs = !css && chained;
        out.push((!in_comment).then(|| depth.saturating_sub(closers) + usize::from(hangs)));
        let mut at = 0;
        while at < bytes.len() {
            if in_comment {
                match line[at..].find("*/") {
                    Some(end) => {
                        in_comment = false;
                        at += end + 2;
                    }
                    None => break,
                }
                continue;
            }
            // A Twig tag is neither code nor a bracket.
            if let Some(opener) = OPENERS.iter().find(|opener| bytes[at..].starts_with(opener.as_bytes())) {
                at = tag_end(line, at, opener)?;
                continue;
            }
            match bytes[at] {
                b'`' => return None,
                quote @ (b'\'' | b'"') => {
                    at += 1;
                    while at < bytes.len() && bytes[at] != quote {
                        if bytes[at] == b'\\' {
                            at += 1;
                        }
                        at += 1;
                    }
                    // Left open at the end of the line: the next line would be part of it.
                    if at >= bytes.len() {
                        return None;
                    }
                }
                b'/' if bytes.get(at + 1) == Some(&b'*') => {
                    in_comment = true;
                    at += 1;
                }
                b'/' if !css && bytes.get(at + 1) == Some(&b'/') => break,
                b'{' | b'(' | b'[' => depth += 1,
                b'}' | b')' | b']' => depth = depth.checked_sub(1)?,
                _ => {}
            }
            at += 1;
        }
    }
    (depth == 0 && !in_comment).then_some(out)
}

/// How wide `lead` (spaces and tabs) is, a tab being `indent` columns.
fn columns(lead: &str, indent: u32) -> usize {
    lead.chars().map(|c| if c == '\t' { indent.max(1) as usize } else { 1 }).sum()
}

/// `text` with every line indented by what is open around it.
fn indented(text: &str, options: &TwigFormatOptions) -> String {
    let starts = line_starts(text);
    let (events, regions) = analyse(text, &starts);
    let unit = if options.use_tabs { "\t".to_owned() } else { " ".repeat(options.indent.max(1) as usize) };
    let lines: Vec<&str> = text.split('\n').collect();
    let mut levels: Vec<usize> = vec![0; lines.len()];
    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    let mut stack = Nest::default();
    // The levels of the script or style being laid out, by its first line.
    let mut code: Option<(usize, Option<Vec<Option<usize>>>)> = None;
    let mut next = 0;
    for (number, raw) in lines.iter().enumerate() {
        let (line, ending) = match raw.strip_suffix('\r') {
            Some(line) => (line, "\r"),
            None => (*raw, ""),
        };
        let start = starts[number];
        let end = start + raw.len();
        let first = next;
        while next < events.len() && events[next].at < end {
            next += 1;
        }
        let on_line = &events[first..next];
        let region = regions.iter().find(|region| match region {
            Region::Keep(a, b) | Region::Shift(a, b, ..) | Region::Tag(a, b, _) => (*a..=*b).contains(&number),
        });
        let content = line.trim();
        // The closing tags and branches the line starts with put it back a level.
        let mut cursor = start + (line.len() - line.trim_start().len());
        let mut leading = 0;
        let mut own: Option<usize> = None;
        for event in on_line {
            let starts_line = text[cursor..event.at].trim().is_empty();
            // A sibling that closes the one before it (`<li>` after `<li>`).
            if let Kind::Open(name, false) = &event.kind
                && starts_line
                && stack.closes_sibling(name)
            {
                stack.open.pop();
            }
            let closes = matches!(event.kind, Kind::Close(..) | Kind::Middle(_));
            if !closes || !starts_line {
                break;
            }
            own = Some(stack.apply(&event.kind));
            cursor = event.end;
            leading += 1;
        }
        // At the level of the last thing the line starts by closing, else of what is open.
        let level = own.unwrap_or(stack.len());
        for event in &on_line[leading..] {
            stack.apply(&event.kind);
        }
        levels[number] = level;
        let laid_out = match region {
            Some(Region::Keep(..)) => format!("{line}{ending}"),
            _ if content.is_empty() => ending.to_owned(),
            Some(Region::Shift(a, b, opener, css)) => {
                if code.as_ref().is_none_or(|(first, _)| first != a) {
                    code = Some((*a, code_levels(&lines[*a..=*b], *css)));
                }
                match code.as_ref().and_then(|(_, levels)| levels.as_ref()) {
                    // By its brackets; a line inside a comment as written.
                    Some(found) => match found[number - a] {
                        Some(depth) => format!("{}{content}{ending}", unit.repeat(levels[*opener] + 1 + depth)),
                        None => format!("{line}{ending}"),
                    },
                    // Its own indentation, from the least indented line of the run.
                    None => {
                        let lead = |line: &str| columns(&line[..line.len() - line.trim_start().len()], options.indent);
                        let least =
                            lines[*a..=*b].iter().filter(|line| !line.trim().is_empty()).map(|line| lead(line)).min();
                        let extra = lead(line) - least.unwrap_or(0).min(lead(line));
                        format!("{}{}{content}{ending}", unit.repeat(levels[*opener] + 1), " ".repeat(extra))
                    }
                }
            }
            Some(Region::Tag(_, _, opener)) => {
                let closes = content.starts_with('>') || content.starts_with("/>");
                format!("{}{content}{ending}", unit.repeat(levels[*opener] + usize::from(!closes)))
            }
            None => format!("{}{content}{ending}", unit.repeat(level)),
        };
        out.push(laid_out);
    }
    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn formatted(text: &str) -> String {
        let out = format_text(text, &TwigFormatOptions::default());
        // Formatted text has nothing left to format.
        assert_eq!(format_text(&out, &TwigFormatOptions::default()), out, "not stable:\n{out}");
        out
    }

    #[test]
    fn delimiters_get_one_space_inside() {
        assert_eq!(formatted("{{x}}"), "{{ x }}");
        assert_eq!(formatted("{{   x   }}"), "{{ x }}");
        assert_eq!(formatted("{%if a%}b{%endif%}"), "{% if a %}b{% endif %}");
        assert_eq!(formatted("{#note#} {#  #}"), "{# note #} {# #}");
        // Whitespace control stays against the delimiter.
        assert_eq!(formatted("{%-if a-%}{{~ x ~}}{% endif %}"), "{%- if a -%}{{~ x ~}}{% endif %}");
        assert_eq!(formatted("{{-x-}}"), "{{- x -}}");
    }

    #[test]
    fn filters_lose_their_spaces_and_runs_of_spaces_become_one() {
        assert_eq!(formatted("{{ name | upper | default( 'x' ) }}"), "{{ name|upper|default( 'x' ) }}");
        assert_eq!(formatted("{% set  a   =  1 %}"), "{% set a = 1 %}");
        assert_eq!(
            formatted("{% for  x  in  items|slice(0, 2) %}{% endfor %}"),
            "{% for x in items|slice(0, 2) %}{% endfor %}"
        );
        // The inside of a string, with what looks like a delimiter or a filter.
        assert_eq!(formatted("{{ 'a  |  b }}'|upper }}"), "{{ 'a  |  b }}'|upper }}");
        assert_eq!(formatted(r#"{{ "it\"s  x" }}"#), r#"{{ "it\"s  x" }}"#);
        // An unclosed tag: nothing after it is touched.
        assert_eq!(formatted("{{x}} {{ y"), "{{ x }} {{ y");
    }

    #[test]
    fn lines_are_indented_by_the_elements_and_blocks_around_them() {
        let text = "{% extends 'base.html.twig' %}\n{% block body %}\n<ul class=\"list\">\n{% for item in items %}\n<li>\n{{ item.name }}\n</li>\n{% else %}\n<li>none</li>\n{% endfor %}\n</ul>\n   <br>\n<img src=\"a.png\"/>\n{% endblock %}\n";
        let expected = "{% extends 'base.html.twig' %}\n{% block body %}\n    <ul class=\"list\">\n        {% for item in items %}\n            <li>\n                {{ item.name }}\n            </li>\n        {% else %}\n            <li>none</li>\n        {% endfor %}\n    </ul>\n    <br>\n    <img src=\"a.png\"/>\n{% endblock %}\n";
        assert_eq!(formatted(text), expected);
        // Two spaces, and tabs.
        let two = format_text("<div>\n<p>x</p>\n</div>", &TwigFormatOptions { indent: 2, use_tabs: false });
        assert_eq!(two, "<div>\n  <p>x</p>\n</div>");
        let tabs = format_text("<div>\n<p>x</p>\n</div>", &TwigFormatOptions { indent: 4, use_tabs: true });
        assert_eq!(tabs, "<div>\n\t<p>x</p>\n</div>");
    }

    #[test]
    fn what_has_no_body_opens_nothing() {
        // A `set` with a value, a short `block`, an `include`; a `set` that captures does.
        let text =
            "{% set a = 1 %}\n{% block title 'Home' %}\n{% include 'a.twig' %}\n{% set b %}\nx\n{% endset %}\ny\n";
        assert_eq!(
            formatted(text),
            "{% set a = 1 %}\n{% block title 'Home' %}\n{% include 'a.twig' %}\n{% set b %}\n    x\n{% endset %}\ny\n"
        );
        // A tag an extension adds: a block when its end tag is there.
        assert_eq!(
            formatted("{% trans %}\nHello\n{% endtrans %}\n{% form_theme f 'x' %}\ny"),
            "{% trans %}\n    Hello\n{% endtrans %}\n{% form_theme f 'x' %}\ny"
        );
        // Siblings that close each other, and a closing tag that closes them.
        assert_eq!(formatted("<ul>\n<li>a\n<li>b\n</ul>\nx"), "<ul>\n    <li>a\n    <li>b\n</ul>\nx");
    }

    #[test]
    fn preserved_content_stays_as_written() {
        let text = "<div>\n<script>\nif (a) {\n  b();\n}\n</script>\n<pre>\n  kept\n    as is\n</pre>\n{% verbatim %}\n  {{ raw }}\n      deep\n{% endverbatim %}\n{#\n   a comment\n over lines #}\n<!--\n  html comment\n-->\n</div>\n";
        let expected = "<div>\n    <script>\n        if (a) {\n            b();\n        }\n    </script>\n    <pre>\n  kept\n    as is\n</pre>\n    {% verbatim %}\n  {{ raw }}\n      deep\n    {% endverbatim %}\n    {#\n   a comment\n over lines #}\n    <!--\n  html comment\n-->\n</div>\n";
        assert_eq!(formatted(text), expected);
    }

    #[test]
    fn a_tag_over_several_lines() {
        let text = "<div>\n<a href=\"x\"\nclass=\"y\"\n>\ntext\n</a>\n{{ call(\n   a,\n b) }}\n</div>";
        let expected = "<div>\n    <a href=\"x\"\n        class=\"y\"\n    >\n        text\n    </a>\n    {{ call(\n   a,\n b) }}\n</div>";
        assert_eq!(formatted(text), expected);
    }

    #[test]
    fn an_element_every_branch_opens_is_open_after_the_block() {
        let text =
            "{% if a %}\n<div class=\"x\">\n{% else %}\n<div>\n{% endif %}\ntext\n<p>\ninner\n</p>\n</div>\nafter\n";
        let expected = "{% if a %}\n    <div class=\"x\">\n{% else %}\n    <div>\n{% endif %}\n    text\n    <p>\n        inner\n    </p>\n</div>\nafter\n";
        assert_eq!(formatted(text), expected);
        // With `elseif` branches too, and inside something else.
        let text = "<main>\n{% if a %}\n<a href=\"x\">\n{% elseif b %}\n<a href=\"y\">\n{% else %}\n<a>\n{% endif %}\ntext\n</a>\n</main>";
        let expected = "<main>\n    {% if a %}\n        <a href=\"x\">\n    {% elseif b %}\n        <a href=\"y\">\n    {% else %}\n        <a>\n    {% endif %}\n        text\n    </a>\n</main>";
        assert_eq!(formatted(text), expected);
    }

    #[test]
    fn branches_that_differ_and_elements_crossing_blocks_do_not_nest() {
        // No `else`: the element may not be open at all.
        let text = "{% if a %}\n<div>\n{% endif %}\ntext\n{% if a %}\n</div>\n{% endif %}\nafter";
        assert_eq!(
            formatted(text),
            "{% if a %}\n    <div>\n{% endif %}\ntext\n{% if a %}\n    </div>\n{% endif %}\nafter"
        );
        // Branches opening different elements.
        let text = "{% if a %}\n<div>\n{% else %}\n<span>\n{% endif %}\ntext\nafter";
        assert_eq!(formatted(text), "{% if a %}\n    <div>\n{% else %}\n    <span>\n{% endif %}\ntext\nafter");
        // A branch of a `for … else` is at the level of its `for`.
        let text = "{% for x in y %}\n<li>\n{{ x }}\n{% else %}\nnone\n{% endfor %}";
        assert_eq!(formatted(text), "{% for x in y %}\n    <li>\n        {{ x }}\n{% else %}\n    none\n{% endfor %}");
    }

    #[test]
    fn scripts_and_styles_are_indented_by_their_brackets() {
        let text = "<script>\nfunction f(a) {\nif (a) {\nreturn [\n1,\n2,\n];\n}\nreturn fetch(a)\n.then((r) => r.json())\n&& true;\n}\n</script>\n<style>\n.a {\ncolor: red;\n}\n@media (min-width: 1px) {\n.b { color: blue; }\n}\n</style>";
        let expected = "<script>\n    function f(a) {\n        if (a) {\n            return [\n                1,\n                2,\n            ];\n        }\n        return fetch(a)\n            .then((r) => r.json())\n            && true;\n    }\n</script>\n<style>\n    .a {\n        color: red;\n    }\n    @media (min-width: 1px) {\n        .b { color: blue; }\n    }\n</style>";
        assert_eq!(formatted(text), expected);
        // Twig inside the code is no bracket; strings and comments aren't either.
        let text = "<script>\nconst a = {{ data|json_encode }};\nif (a) { // {\nlog(\"}\", '(');\n/* {\n   kept as written\n*/\n}\n</script>";
        let expected = "<script>\n    const a = {{ data|json_encode }};\n    if (a) { // {\n        log(\"}\", '(');\n        /* {\n   kept as written\n*/\n    }\n</script>";
        assert_eq!(formatted(text), expected);
    }

    #[test]
    fn code_that_cannot_be_read_safely_keeps_its_own_indentation() {
        // A template literal: its lines are part of a string.
        let text = "<div>\n<script>\nconst t = `\n  a\n    b`;\nif (t) {\n  go();\n}\n</script>\n</div>";
        let expected = "<div>\n    <script>\n        const t = `\n          a\n            b`;\n        if (t) {\n          go();\n        }\n    </script>\n</div>";
        assert_eq!(formatted(text), expected);
        // Brackets that don't balance (a regular expression with one), a string left open.
        for code in ["const r = /[{]/;\n  x();", "const s = 'a \\\n  b';", "f(\n  a"] {
            let text = format!("<script>\n{code}\n</script>");
            let out = formatted(&text);
            let shifted: String = code.lines().map(|line| format!("    {line}\n")).collect();
            assert_eq!(out, format!("<script>\n{shifted}</script>"), "{code}");
        }
    }

    #[test]
    fn only_whitespace_changes_and_line_endings_stay() {
        let text = "<p>\r\n{{x}}   \r\n</p>\r\n";
        assert_eq!(formatted(text), "<p>\r\n    {{ x }}\r\n</p>\r\n");
        let visible = |text: &str| text.chars().filter(|c| !c.is_whitespace()).collect::<String>();
        let messy = "<div>{% if a %}<b>\n{{  x|upper  }}</b>{% endif %}\n  </div> {# c #}\n<input type=\"text\" value=\"{{ v }}\">\n";
        assert_eq!(visible(&formatted(messy)), visible(messy));
    }

    #[test]
    fn edits_are_one_per_changed_line_and_only_the_lines_asked_for() {
        let text = "<div>\n{{a}}\n{{ b }}\n</div>\n";
        let options = TwigFormatOptions::default();
        let all = format(text, &options, None);
        assert_eq!(all.iter().map(|edit| edit.new_text.as_str()).collect::<Vec<_>>(), ["    {{ a }}", "    {{ b }}"]);
        let one = format(text, &options, Some((2, 2)));
        assert_eq!(one.len(), 1);
        assert_eq!(one[0].range.start.line, 2);
        assert_eq!(format("<div>\n    {{ a }}\n</div>\n", &options, None), []);
    }
}
