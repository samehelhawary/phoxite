//! A WordPress project's scripts and `block.json` files as documents: what
//! the string at the caret names, for goto, hover, Find References, rename
//! and completion there.
//!
//! The engine doesn't parse JavaScript. The string around the caret is
//! taken from its line, and it names:
//!
//! - a block, when the project declares one of that name (in a
//!   `block.json`: any string that is one, `parent` and `allowedBlocks`
//!   included);
//! - a REST route, when the project registers one its path fits;
//! - the action of an AJAX request, on a line saying `action` before it:
//!   the hooks `wp_ajax_{action}` and `wp_ajax_nopriv_{action}`;
//! - in a `block.json`, the file of `"file:./render.php"`.
//!
//! Completion goes by the text before the string: `action` offers the
//! actions PHP listens to with a named callback, `path`, `fetch` or a
//! leading slash the REST routes, and `Block` or `name` (and anywhere in a
//! `block.json`) the blocks.

use std::path::{Path, PathBuf};

use crate::base::{CancellationToken, IdeError, IdeResult, Pos, Range, WorkspaceEdit};
use crate::engine::PhpEngine;
use crate::frameworks::Framework;
use crate::frameworks::refs::{RefContext, ReferenceKind};
use crate::frameworks::wordpress::assets::is_block_json;
use crate::frameworks::wordpress::refs::{BLOCK, HOOK, RestRoute};
use crate::navigate::{HoverInfo, Target, Usage};
use crate::project::is_php_path;
use crate::syntax::LineIndex;

/// What a string of a script names.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Named {
    Block,
    RestRoute,
    AjaxAction,
    /// A file a `block.json` names.
    File(PathBuf),
}

struct Subject<'e> {
    cx: RefContext<'e>,
    named: Named,
    value: String,
    /// The bytes of the string's content.
    start: usize,
    end: usize,
    lines: LineIndex,
}

/// A completion item of a script's string.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScriptCompletion {
    pub label: String,
    /// What it is (`block`).
    pub detail: &'static str,
    /// The string's content, which the label replaces.
    pub range: Range,
}

const AJAX: [&str; 2] = ["wp_ajax_", "wp_ajax_nopriv_"];

/// The content of the string around byte `offset` of `text`, on its line:
/// the bytes it starts and ends at.
fn string_at(text: &str, offset: usize) -> Option<(usize, usize)> {
    let offset = offset.min(text.len());
    let line = text[..offset].rfind('\n').map_or(0, |at| at + 1);
    let end = text[offset..].find('\n').map_or(text.len(), |at| offset + at);
    let bytes = text.as_bytes();
    let mut at = line;
    while at < end {
        let quote = bytes[at];
        if !matches!(quote, b'\'' | b'"' | b'`') {
            at += 1;
            continue;
        }
        let start = at + 1;
        let mut close = start;
        while close < end && bytes[close] != quote {
            close += if bytes[close] == b'\\' { 2 } else { 1 };
        }
        let close = close.min(end);
        if start <= offset && offset <= close {
            return Some((start, close));
        }
        at = close + 1;
    }
    None
}

fn identifier(value: &str) -> bool {
    !value.is_empty() && value.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

impl PhpEngine {
    fn wordpress_context(&self) -> Option<RefContext<'_>> {
        self.frameworks().contains(Framework::WordPress).then(|| self.ref_context()).flatten()
    }

    /// What the string at `pos` of the script or `block.json` at `path`
    /// names.
    fn script_subject(&self, path: &Path, text: &str, pos: Pos) -> Option<Subject<'_>> {
        let cx = self.wordpress_context()?;
        let lines = LineIndex::new(text);
        let (start, end) = string_at(text, lines.byte(pos) as usize)?;
        let value = text[start..end].to_owned();
        let line = text[..start].rfind('\n').map_or(0, |at| at + 1);
        let before = &text[line..start];
        let named = if let Some(file) = value.strip_prefix("file:").filter(|_| is_block_json(path)) {
            let file = path.parent()?.join(file.trim_start_matches("./"));
            Named::File(file.is_file().then_some(file)?)
        } else if !BLOCK.resolve(&cx, &value, 0).is_empty() {
            Named::Block
        } else if value.contains('/') && !RestRoute.resolve(&cx, &value, 0).is_empty() {
            Named::RestRoute
        } else if before.contains("action") && identifier(&value) && !is_block_json(path) {
            Named::AjaxAction
        } else {
            return None;
        };
        Some(Subject { cx, named, value, start, end, lines })
    }

    /// The calls listening to the hooks of the AJAX request `action`.
    fn ajax_listeners(&self, cx: &RefContext<'_>, action: &str, cancel: &CancellationToken) -> IdeResult<Vec<Usage>> {
        let mut found = Vec::new();
        for prefix in AJAX {
            let hook = format!("{prefix}{action}");
            found.extend(self.kind_usages(cx, &HOOK, &hook, &hook, false, cancel)?);
        }
        found.sort_by(|a, b| (&a.path, a.range.start).cmp(&(&b.path, b.range.start)));
        found.dedup_by(|b, a| a.path == b.path && a.range == b.range);
        Ok(found)
    }

    /// Where what the string at `pos` names is declared: a block's
    /// registration, a route's, the calls listening to an AJAX action. `None`
    /// outside a WordPress project and where the string names nothing.
    pub fn wordpress_script_definition(&self, path: &Path, text: &str, pos: Pos) -> Option<Vec<Target>> {
        let subject = self.script_subject(path, text, pos)?;
        Some(match &subject.named {
            Named::File(file) => vec![Target { path: file.clone(), range: Range::default() }],
            Named::Block => BLOCK.resolve(&subject.cx, &subject.value, 0),
            Named::RestRoute => RestRoute.resolve(&subject.cx, &subject.value, 0),
            Named::AjaxAction => {
                let listeners = self.ajax_listeners(&subject.cx, &subject.value, &CancellationToken::new()).ok()?;
                let php = listeners.into_iter().filter(|usage| is_php_path(&usage.path));
                php.map(|usage| Target { path: usage.path, range: usage.range }).collect()
            }
        })
    }

    pub fn wordpress_script_hover(&self, path: &Path, text: &str, pos: Pos) -> Option<HoverInfo> {
        let subject = self.script_subject(path, text, pos)?;
        let root = self.root();
        let relative = |path: &Path| path.strip_prefix(&root).unwrap_or(path).to_string_lossy().replace('\\', "/");
        let markdown = match &subject.named {
            Named::File(file) => format!("**file** `{}`", relative(file)),
            Named::Block => BLOCK.hover(&subject.cx, &subject.value)?,
            Named::RestRoute => RestRoute.hover(&subject.cx, &subject.value)?,
            Named::AjaxAction => {
                let listeners = self.ajax_listeners(&subject.cx, &subject.value, &CancellationToken::new()).ok()?;
                let mut places: Vec<String> = listeners
                    .iter()
                    .filter(|usage| is_php_path(&usage.path))
                    .map(|usage| format!("{}:{}", relative(&usage.path), usage.range.start.line + 1))
                    .collect();
                places.dedup();
                if places.is_empty() {
                    return None;
                }
                format!("**AJAX action** `{}`\n\nHandled in {}", subject.value, places.join(", "))
            }
        };
        Some(HoverInfo { markdown, range: subject.lines.range(subject.start as u32, subject.end as u32) })
    }

    /// The usages of what the string at `pos` names, in PHP and in the
    /// scripts.
    pub fn wordpress_script_references(
        &self,
        path: &Path,
        text: &str,
        pos: Pos,
        include_declaration: bool,
        cancel: &CancellationToken,
    ) -> Option<IdeResult<Vec<Usage>>> {
        let subject = self.script_subject(path, text, pos)?;
        let (cx, value) = (&subject.cx, subject.value.as_str());
        Some(match &subject.named {
            Named::File(_) => return None,
            Named::Block => self.kind_usages(cx, &BLOCK, value, value, include_declaration, cancel),
            Named::RestRoute => {
                let key = RestRoute.canonical(cx, value);
                self.kind_usages(cx, &RestRoute, value, &key, include_declaration, cancel)
            }
            Named::AjaxAction => self.ajax_listeners(cx, value, cancel),
        })
    }

    /// What a rename of the string at `pos` changes, as a kind and its
    /// target, with the bytes of the string the new name replaces.
    fn script_rename(
        &self,
        subject: &Subject<'_>,
    ) -> Option<(&'static dyn ReferenceKind, crate::frameworks::refs::RenameTarget, (usize, usize))> {
        let (cx, value) = (&subject.cx, subject.value.as_str());
        let whole = (0, value.len());
        match &subject.named {
            Named::File(_) => None,
            Named::Block => Some((&BLOCK, BLOCK.rename_target(cx, value, 0)?, whole)),
            Named::RestRoute => {
                let target = RestRoute.rename_target(cx, value, 0)?;
                let range = RestRoute.site_rename_range(value, &target);
                Some((&RestRoute, target, range))
            }
            Named::AjaxAction => {
                let hook = format!("{}{value}", AJAX[0]);
                Some((&HOOK, HOOK.rename_target(cx, &hook, 0)?, whole))
            }
        }
    }

    /// The range a rename at `pos` changes; `Some(None)` when what is there
    /// can't be renamed.
    pub fn wordpress_script_prepare_rename(&self, path: &Path, text: &str, pos: Pos) -> Option<Option<Range>> {
        let subject = self.script_subject(path, text, pos)?;
        Some(
            self.script_rename(&subject).map(|(_, _, (from, to))| {
                subject.lines.range((subject.start + from) as u32, (subject.start + to) as u32)
            }),
        )
    }

    /// Renames what the string at `pos` names: in PHP and in the scripts as
    /// they are saved.
    pub fn wordpress_script_rename(
        &self,
        path: &Path,
        text: &str,
        pos: Pos,
        new_name: &str,
        cancel: &CancellationToken,
    ) -> Option<IdeResult<WorkspaceEdit>> {
        let subject = self.script_subject(path, text, pos)?;
        let Some((kind, target, _)) = self.script_rename(&subject) else {
            return Some(Err(IdeError::invalid("this string can't be renamed")));
        };
        Some(self.kind_rename(&subject.cx, kind, target, new_name, cancel))
    }

    /// What the string at `pos` can be, by the text before it; see the
    /// module documentation.
    pub fn wordpress_script_completion(&self, path: &Path, text: &str, pos: Pos) -> Option<Vec<ScriptCompletion>> {
        let cx = self.wordpress_context()?;
        let lines = LineIndex::new(text);
        let (start, end) = string_at(text, lines.byte(pos) as usize)?;
        let value = &text[start..end];
        let line = text[..start].rfind('\n').map_or(0, |at| at + 1);
        let before = &text[line..start];
        let range = lines.range(start as u32, end as u32);
        let names = self.names();
        let item = |label: String, detail: &'static str| ScriptCompletion { label, detail, range };
        let blocks = || BLOCK.complete(&cx, value, 0).into_iter().map(|found| item(found.label, "block")).collect();
        if is_block_json(path) {
            return Some(blocks());
        }
        let slash = value.starts_with('/');
        let mut found: Vec<ScriptCompletion> = if before.contains("action") {
            names.ajax_actions.iter().map(|action| item(action.clone(), "AJAX action")).collect()
        } else if slash || ["path", "fetch", "url", "Url", "URL"].iter().any(|word| before.contains(word)) {
            let routes = RestRoute.complete(&cx, value, 0);
            routes.into_iter().map(|found| item(format!("/{}", found.label), "REST route")).collect()
        } else if before.contains("Block") || before.contains("name") {
            blocks()
        } else {
            return None;
        };
        found.dedup_by(|b, a| a.label == b.label);
        Some(found)
    }
}

#[cfg(test)]
mod tests {
    use super::string_at;

    #[test]
    fn the_string_around_the_caret_is_found_on_its_line() {
        let text = "post(url, { action: 'shop_add', id: \"a\\\"b\" });\nconst t = `x`;\n";
        let at = |needle: &str| text.find(needle).unwrap() + 1;
        let found = |offset: usize| string_at(text, offset).map(|(start, end)| &text[start..end]);
        assert_eq!(found(at("shop_add")), Some("shop_add"));
        assert_eq!(found(at("a\\\"b")), Some("a\\\"b"));
        assert_eq!(found(at("x`")), Some("x"));
        assert_eq!(found(at("url")), None);
        assert_eq!(found(at("id:")), None);
    }
}
