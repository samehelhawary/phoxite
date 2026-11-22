//! A Drupal project's YAML files and templates as documents: what the text
//! at the caret names, for goto, hover, Find References, rename and
//! completion there.
//!
//! A YAML file is read line by line, by what it is ([`FileKind`]):
//!
//! | where | what |
//! | --- | --- |
//! | `@id` anywhere; `alias:` and `parent:` of a services file | a service |
//! | `route_name:` and `base_route:` of a links file | a route |
//! | `parent:` and `parent_id:` of a links file | a plugin |
//! | `_permission:` of a routing file | each permission it lists |
//! | an item of a list in an info or a libraries file | a library; `drupal:node` in an info file is the module `node` |
//! | a class's name, with `::method` or not | the class or its method |
//! | a key that declares a name (a service, a route, a permission, a library, a link) | that name: its usages and its rename |
//!
//! In a template, the string given to `attach_library()` is a library.
//!
//! Each is answered by the name's reference kind
//! ([`crate::frameworks::drupal::refs`]), as for a string of PHP.

use std::path::Path;

use crate::base::{CancellationToken, IdeError, IdeResult, Pos, Range, WorkspaceEdit};
use crate::engine::PhpEngine;
use crate::frameworks::Framework;
use crate::frameworks::drupal::FileKind;
use crate::frameworks::drupal::refs::{LIBRARY, MODULE, PERMISSION, PLUGIN, ROUTE, SERVICE};
use crate::frameworks::refs::{RefContext, ReferenceKind};
use crate::navigate::{HoverInfo, Target, Usage};
use crate::syntax::LineIndex;

/// What the text at the caret names.
enum Named {
    Name(&'static dyn ReferenceKind),
    /// A class (FQN), and a method of it.
    Class(String, Option<String>),
    /// A place: a file an `.info` names, the anchor an alias stands for.
    Place(std::path::PathBuf, Range),
}

struct Subject<'e> {
    cx: RefContext<'e>,
    named: Named,
    value: String,
    /// The bytes of the name in the document.
    start: usize,
    end: usize,
    lines: LineIndex,
}

/// A completion item of a YAML file or a template.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NameCompletion {
    pub label: String,
    pub detail: Option<String>,
    /// What the label replaces.
    pub range: Range,
}

/// A line of a YAML file around the caret.
struct Line<'t> {
    /// The byte the line starts at.
    start: usize,
    text: &'t str,
    /// How far the line is indented.
    indent: usize,
    /// The key before its `:`, when it has one, and the byte (in the line)
    /// after the colon.
    key: Option<(&'t str, usize)>,
    /// A list item: the byte (in the line) after its `- `.
    item: Option<usize>,
}

fn line_at(text: &str, offset: usize) -> Line<'_> {
    let offset = offset.min(text.len());
    let start = text[..offset].rfind('\n').map_or(0, |at| at + 1);
    let end = text[offset..].find('\n').map_or(text.len(), |at| offset + at);
    let line = text[start..end].trim_end_matches('\r');
    let indent = line.len() - line.trim_start().len();
    let body = &line[indent..];
    let item = body.strip_prefix("- ").map(|rest| indent + 2 + (rest.len() - rest.trim_start().len()));
    let from = item.unwrap_or(indent);
    // A key ends at the first `:` followed by a space or the line's end, outside quotes.
    let mut key = None;
    let mut quote: Option<char> = None;
    for (at, c) in line[from..].char_indices() {
        match quote {
            Some(open) if c == open => quote = None,
            Some(_) => {}
            None if c == '\'' || c == '"' => quote = Some(c),
            None if c == ':' => {
                let after = from + at + 1;
                if line[after..].is_empty() || line[after..].starts_with(' ') {
                    key = Some((line[from..from + at].trim_matches(['\'', '"']), after));
                    break;
                }
            }
            None => {}
        }
    }
    Line { start, text: line, indent, key, item }
}

/// The token around byte `at` of `line` (from byte `from` on): what is
/// between quotes, brackets, commas and, unless `spaced`, spaces.
fn token(line: &str, from: usize, at: usize, spaced: bool, split: &[char]) -> Option<(usize, usize)> {
    let stop =
        |c: char| matches!(c, '\'' | '"' | '[' | ']' | '{' | '}' | ',') || (!spaced && c == ' ') || split.contains(&c);
    let at = at.clamp(from, line.len());
    let start = line[from..at].rfind(stop).map_or(from, |found| from + found + 1);
    let end = line[at..].find(stop).map_or(line.len(), |found| at + found);
    let raw = &line[start..end];
    let trimmed = raw.trim();
    let start = start + (raw.len() - raw.trim_start().len());
    (!trimmed.is_empty()).then_some((start, start + trimmed.len()))
}

/// Whether `text` is a class's name written out: `Drupal\shop\Catalog`.
fn class_name(text: &str) -> bool {
    let text = text.trim_start_matches('\\');
    text.contains('\\')
        && text.split('\\').all(|part| {
            part.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
                && part.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        })
}

impl PhpEngine {
    fn drupal_context(&self) -> Option<RefContext<'_>> {
        self.frameworks().contains(Framework::Drupal).then(|| self.ref_context()).flatten()
    }

    /// What the text at `pos` of the YAML file at `path` names.
    fn yaml_subject(&self, path: &Path, text: &str, pos: Pos) -> Option<Subject<'_>> {
        let kind = FileKind::of(path)?;
        let cx = self.drupal_context()?;
        let lines = LineIndex::new(text);
        let offset = lines.byte(pos) as usize;
        let line = line_at(text, offset);
        let at = offset - line.start;
        // A line without a key of its own is a value of the key above it: an item of its
        // list, or what follows it on the next line.
        let mut key = line.key.map(|(key, _)| key).or_else(|| parent_key(text, line.start, line.indent));
        let mut value_from = line.key.map(|(_, after)| after).or(line.item).or(Some(line.indent));
        // In a mapping written in braces, the key before the caret: `{ route_name: shop.books }`.
        let braced = inline_key(line.text, at);
        if let Some((inline, after)) = braced {
            key = Some(inline);
            value_from = Some(after);
        }
        let subject = |named: Named, value: &str, start: usize, end: usize| Subject {
            cx: cx.clone(),
            named,
            value: value.to_owned(),
            start: line.start + start,
            end: line.start + end,
            lines: LineIndex::new(text),
        };
        // Drupal 7's `.info`: `dependencies[] = node`, `files[] = shop.test`.
        if kind == FileKind::InfoLegacy {
            let (written, value) = line.text.split_once('=')?;
            let from = written.len() + 1 + (value.len() - value.trim_start().len());
            let value = value.trim().trim_matches(['\'', '"']);
            if at < from || value.is_empty() {
                return None;
            }
            let from = line.text[from..].find(value).map_or(from, |found| from + found);
            return match written.trim().split('[').next().unwrap_or_default() {
                "dependencies" | "base theme" => {
                    // `views (>=3.0)`, `drupal:node`: the module's name.
                    let name = value.split_whitespace().next().unwrap_or(value);
                    let name = name.rsplit(':').next().unwrap_or(name);
                    let start = from + value.find(name).unwrap_or(0);
                    Some(subject(Named::Name(&MODULE), name, start, start + name.len()))
                }
                "files" | "scripts" | "stylesheets" => {
                    let file = path.parent()?.join(value);
                    file.is_file()
                        .then(|| subject(Named::Place(file, Range::default()), value, from, from + value.len()))
                }
                _ => None,
            };
        }
        // An alias: the anchor of that name in the file.
        if let Some((start, end)) = token(line.text, value_from.unwrap_or(0).min(at), at, false, &[])
            && let Some(name) = line.text[start..end].strip_prefix('*').filter(|name| !name.is_empty())
        {
            let anchor = format!("&{name}");
            let found = text.match_indices(&anchor).map(|(found, _)| found).find(|found| {
                !text[found + anchor.len()..].starts_with(|c: char| c.is_alphanumeric() || c == '_' || c == '-')
            })?;
            let range = lines.range((found + 1) as u32, (found + anchor.len()) as u32);
            return Some(subject(Named::Place(path.to_path_buf(), range), name, start + 1, end));
        }
        // The key itself, where it declares a name.
        if let Some((written, after)) = line.key
            && at < after
            && line.item.is_none()
        {
            let start = line.text[..after].find(written)?;
            let declared: Option<(&'static dyn ReferenceKind, String)> = match (kind, line.indent) {
                (FileKind::Services, indent) if indent > 0 && under(text, line.start, "services") => {
                    Some((&SERVICE, written.to_owned()))
                }
                (FileKind::Routing, 0) => Some((&ROUTE, written.to_owned())),
                (FileKind::Permissions, 0) => Some((&PERMISSION, written.to_owned())),
                (FileKind::Libraries, 0) => {
                    let name = path.file_name()?.to_str()?.strip_suffix(".libraries.yml")?;
                    Some((&LIBRARY, format!("{name}/{written}")))
                }
                (FileKind::Links | FileKind::Layouts, 0) => Some((&PLUGIN, written.to_owned())),
                _ => None,
            };
            let (named, value) = declared?;
            // A library's key is the end of its name.
            return Some(subject(Named::Name(named), &value, start, start + written.len()));
        }
        let from = value_from?;
        // A permission may have spaces, and several are joined by `+` or `,`.
        let permission = kind == FileKind::Routing && key == Some("_permission");
        let (start, end) = token(line.text, from, at, permission, if permission { &['+'] } else { &[] })?;
        let written = &line.text[start..end];
        if permission {
            return Some(subject(Named::Name(&PERMISSION), written, start, end));
        }
        if let Some(id) = written.strip_prefix('@') {
            let id = id.trim_start_matches('?');
            return Some(subject(Named::Name(&SERVICE), id, end - id.len(), end));
        }
        let named: Option<&'static dyn ReferenceKind> = match (kind, key) {
            (FileKind::Services, Some("alias" | "parent")) => Some(&SERVICE),
            (FileKind::Links, Some("route_name" | "base_route")) => Some(&ROUTE),
            (FileKind::Links, Some("parent" | "parent_id")) => Some(&PLUGIN),
            _ => None,
        };
        if let Some(named) = named {
            return Some(subject(Named::Name(named), written, start, end));
        }
        if line.item.is_some() && line.key.is_none() && matches!(kind, FileKind::Info | FileKind::Libraries) {
            // `drupal:node`, `node:node`: a module.
            if let Some((_, module)) = written.split_once(':').filter(|_| kind == FileKind::Info) {
                let module = module.split_whitespace().next().unwrap_or(module);
                let from = end - (written.len() - written.find(':')? - 1);
                return Some(subject(Named::Name(&MODULE), module, from, from + module.len()));
            }
            if written.contains('/') {
                return Some(subject(Named::Name(&LIBRARY), written, start, end));
            }
        }
        // A class, with a method or not: `\Drupal\shop\Controller\BookController::list`.
        let (class, method) = match written.split_once("::") {
            Some((class, method)) => (class, Some(method.to_owned())),
            None => (written, None),
        };
        if !class_name(class) {
            // `shop.controller:list`: a service and a method of its class.
            let (service, method) = written.split_once(':').filter(|(_, method)| !method.contains(':'))?;
            let project = self.drupal();
            let class = project.services.service_class(service)?;
            return Some(if at <= start + service.len() {
                subject(Named::Name(&SERVICE), service, start, start + service.len())
            } else {
                subject(Named::Class(class, Some(method.to_owned())), method, end - method.len(), end)
            });
        }
        Some(subject(Named::Class(class.trim_start_matches('\\').to_owned(), method), written, start, end))
    }

    /// What the string at `pos` of a template names: the library given to
    /// `attach_library()`.
    fn twig_subject(&self, text: &str, pos: Pos) -> Option<Subject<'_>> {
        let cx = self.drupal_context()?;
        let lines = LineIndex::new(text);
        let offset = lines.byte(pos) as usize;
        let line = line_at(text, offset);
        let at = offset - line.start;
        let before = &line.text[..at.min(line.text.len())];
        let open = before.rfind(['\'', '"'])?;
        if !before[..open].trim_end().ends_with("attach_library(") {
            return None;
        }
        let quote = line.text[open..].chars().next()?;
        let close = line.text[open + 1..].find(quote).map_or(line.text.len(), |found| open + 1 + found);
        if at > close {
            return None;
        }
        Some(Subject {
            cx,
            named: Named::Name(&LIBRARY),
            value: line.text[open + 1..close].to_owned(),
            start: line.start + open + 1,
            end: line.start + close,
            lines,
        })
    }

    fn document_subject(&self, path: &Path, text: &str, pos: Pos) -> Option<Subject<'_>> {
        if path.extension().is_some_and(|ext| ext == "twig") {
            self.twig_subject(text, pos)
        } else {
            self.yaml_subject(path, text, pos)
        }
    }

    /// Where what the text at `pos` of the YAML file or template at `path`
    /// names is declared. `None` outside a Drupal project and where the
    /// text names nothing.
    pub fn drupal_document_definition(&self, path: &Path, text: &str, pos: Pos) -> Option<Vec<Target>> {
        let subject = self.document_subject(path, text, pos)?;
        Some(match &subject.named {
            Named::Name(kind) => kind.resolve(&subject.cx, &subject.value, 0),
            Named::Class(class, method) => {
                let method = method.as_deref().and_then(|method| {
                    crate::frameworks::laravel::refs::controller_action::method_target(&subject.cx, class, method)
                });
                method.map_or_else(|| self.class_targets(class), |target| vec![target])
            }
            Named::Place(file, range) => vec![Target { path: file.clone(), range: *range }],
        })
    }

    pub fn drupal_document_hover(&self, path: &Path, text: &str, pos: Pos) -> Option<HoverInfo> {
        let subject = self.document_subject(path, text, pos)?;
        let markdown = match &subject.named {
            Named::Name(kind) => kind.hover(&subject.cx, &subject.value)?,
            Named::Class(class, method) => {
                self.class_targets(class).first()?;
                match method {
                    Some(method) => format!("```php\n{class}::{method}()\n```"),
                    None => format!("```php\n{class}\n```"),
                }
            }
            Named::Place(..) => return None,
        };
        Some(HoverInfo { markdown, range: subject.lines.range(subject.start as u32, subject.end as u32) })
    }

    /// The usages of what the text at `pos` names, in PHP, in the YAML
    /// files and in the templates.
    pub fn drupal_document_references(
        &self,
        path: &Path,
        text: &str,
        pos: Pos,
        include_declaration: bool,
        cancel: &CancellationToken,
    ) -> Option<IdeResult<Vec<Usage>>> {
        let subject = self.document_subject(path, text, pos)?;
        let Named::Name(kind) = subject.named else { return None };
        Some(self.kind_usages(&subject.cx, kind, &subject.value, &subject.value, include_declaration, cancel))
    }

    /// The range a rename at `pos` changes; `Some(None)` when what is there
    /// can't be renamed.
    pub fn drupal_document_prepare_rename(&self, path: &Path, text: &str, pos: Pos) -> Option<Option<Range>> {
        let subject = self.document_subject(path, text, pos)?;
        let Named::Name(kind) = subject.named else { return Some(None) };
        Some(kind.rename_target(&subject.cx, &subject.value, 0).map(|target| {
            // The part renamed is the end of the name: what the document writes of it.
            let renamed = target.range.1 - target.range.0;
            subject.lines.range((subject.end - renamed.min(subject.end - subject.start)) as u32, subject.end as u32)
        }))
    }

    /// Renames what the text at `pos` names: its key, the PHP strings and
    /// the other files naming it, as they are saved.
    pub fn drupal_document_rename(
        &self,
        path: &Path,
        text: &str,
        pos: Pos,
        new_name: &str,
        cancel: &CancellationToken,
    ) -> Option<IdeResult<WorkspaceEdit>> {
        let subject = self.document_subject(path, text, pos)?;
        let Named::Name(kind) = subject.named else {
            return Some(Err(IdeError::invalid("this can't be renamed here")));
        };
        let Some(target) = kind.rename_target(&subject.cx, &subject.value, 0) else {
            return Some(Err(IdeError::invalid("this name can't be renamed")));
        };
        Some(self.kind_rename(&subject.cx, kind, target, new_name, cancel))
    }

    /// The names that can be written at `pos`: by the key of the line in a
    /// YAML file, and in a template's `attach_library()`.
    pub fn drupal_document_completion(&self, path: &Path, text: &str, pos: Pos) -> Option<Vec<NameCompletion>> {
        let cx = self.drupal_context()?;
        let lines = LineIndex::new(text);
        let offset = lines.byte(pos) as usize;
        let line = line_at(text, offset);
        let at = (offset - line.start).min(line.text.len());
        let (kind, start, end): (&'static dyn ReferenceKind, usize, usize) = if path
            .extension()
            .is_some_and(|ext| ext == "twig")
        {
            let before = &line.text[..at];
            let open = before.rfind(['\'', '"'])?;
            if !before[..open].trim_end().ends_with("attach_library(") {
                return None;
            }
            let quote = line.text[open..].chars().next()?;
            let close = line.text[open + 1..].find(quote).map_or(line.text.len(), |found| open + 1 + found);
            (&LIBRARY, open + 1, close.max(at))
        } else {
            let file = FileKind::of(path)?;
            let key = line.key.map(|(key, _)| key).or_else(|| parent_key(text, line.start, line.indent));
            let from = line.key.map(|(_, after)| after).or(line.item).unwrap_or(line.indent);
            if at < from {
                return None;
            }
            let permission = file == FileKind::Routing && key == Some("_permission");
            let split: &[char] = if permission { &['+'] } else { &[] };
            let (start, end) = token(line.text, from, at, permission, split).unwrap_or((at, at));
            let written = &line.text[start..end];
            let kind: &'static dyn ReferenceKind = if permission {
                &PERMISSION
            } else if written.starts_with('@') {
                &SERVICE
            } else {
                match (file, key) {
                    (FileKind::Services, Some("alias" | "parent")) => &SERVICE,
                    (FileKind::Links, Some("route_name" | "base_route")) => &ROUTE,
                    (FileKind::Links, Some("parent" | "parent_id")) => &PLUGIN,
                    (FileKind::Info | FileKind::Libraries, _) if line.item.is_some() && line.key.is_none() => &LIBRARY,
                    // A class: the namespaces and classes below what is written so far.
                    (_, Some("class" | "_controller" | "_form" | "_title_callback")) => {
                        let range = lines.range((line.start + start) as u32, (line.start + end) as u32);
                        return Some(self.class_names(written.split("::").next().unwrap_or(written), range));
                    }
                    _ => return None,
                }
            };
            // After the `@` (and the `?`) of a service.
            let prefix = written.len() - written.trim_start_matches(['@', '?']).len();
            (kind, start + prefix, end)
        };
        let range = lines.range((line.start + start) as u32, (line.start + end) as u32);
        let items = kind.complete(&cx, "", 0);
        Some(items.into_iter().map(|item| NameCompletion { label: item.label, detail: item.detail, range }).collect())
    }
}

/// The key before byte `at` of `line` in a mapping written in braces
/// (`{ title: 'Books', route_name: shop.books }`), and the byte after its
/// colon.
fn inline_key(line: &str, at: usize) -> Option<(&str, usize)> {
    let before = &line[..at.min(line.len())];
    let open = before.rfind('{')?;
    if before[open..].contains('}') {
        return None;
    }
    let from = before.rfind(['{', ',']).unwrap_or(open) + 1;
    let colon = before[from..].find(':')?;
    let key = before[from..from + colon].trim().trim_matches(['\'', '"']);
    (!key.is_empty()).then_some((key, from + colon + 1))
}

/// The key the line at byte `line` of `text`, indented by `indent`, is
/// below: that of the nearest line above it that is indented less, or of
/// the line opening the bracketed list it is in.
fn parent_key(text: &str, line: usize, indent: usize) -> Option<&str> {
    let mut end = line;
    while end > 0 {
        let start = text[..end - 1].rfind('\n').map_or(0, |at| at + 1);
        let above = text[start..end - 1].trim_end_matches('\r');
        let written = above.trim_start();
        if !written.is_empty() && !written.starts_with('#') {
            let depth = above.len() - written.len();
            if depth < indent || (depth == indent && written.trim_end().ends_with('[')) {
                return line_at(text, start).key.map(|(key, _)| key);
            }
        }
        end = start;
    }
    None
}

impl PhpEngine {
    /// The namespaces and classes directly below the namespace `written`
    /// ends in (those of `Drupal\shop` for `Drupal\shop\Cat`), each written
    /// out.
    pub(crate) fn class_names(&self, written: &str, range: Range) -> Vec<NameCompletion> {
        let written = written.trim_start_matches('\\');
        let namespace = written.rsplit_once('\\').map_or("", |(namespace, _)| namespace);
        let Some(index) = self.index() else { return Vec::new() };
        let mut found: Vec<NameCompletion> = index
            .namespace_children(namespace)
            .unwrap_or_default()
            .into_iter()
            .filter(|decl| !matches!(decl.kind, crate::index::DeclKind::Function | crate::index::DeclKind::Constant))
            .map(|decl| NameCompletion {
                detail: Some(
                    if decl.kind == crate::index::DeclKind::Namespace { "namespace" } else { "class" }.to_owned(),
                ),
                label: decl.fqn,
                range,
            })
            .collect();
        found.sort_by(|a, b| a.label.cmp(&b.label));
        found.dedup_by(|b, a| a.label == b.label);
        found
    }
}

/// Whether the line at byte `line` of `text` is below the top-level key
/// `section` (`services:`).
fn under(text: &str, line: usize, section: &str) -> bool {
    let top = text[..line].lines().rev().find(|above| !above.is_empty() && !above.starts_with([' ', '#', '\t']));
    top.is_some_and(|top| top.trim_end().trim_end_matches(':') == section)
}

#[cfg(test)]
mod tests {
    use super::{class_name, line_at, token, under};

    #[test]
    fn a_line_is_a_key_and_what_follows_it() {
        let text = "services:\n  shop.catalog:\n    arguments: ['@shop.prices', '@?logger']\n    - shop/cart\n  _permission: 'administer shop+access shop'\n";
        let at = |needle: &str| text.find(needle).unwrap();
        let line = line_at(text, at("shop.catalog"));
        assert_eq!((line.indent, line.key.map(|(key, _)| key)), (2, Some("shop.catalog")));
        assert!(under(text, line.start, "services"));
        let line = line_at(text, at("@?logger"));
        let (_, from) = line.key.unwrap();
        let found = token(line.text, from, at("@?logger") + 3 - line.start, false, &[]).unwrap();
        assert_eq!(&line.text[found.0..found.1], "@?logger");
        let line = line_at(text, at("shop/cart"));
        assert!(line.key.is_none() && line.item.is_some());
        // A permission keeps its spaces and stops at `+`.
        let line = line_at(text, at("access shop"));
        let (_, from) = line.key.unwrap();
        let found = token(line.text, from, at("access shop") + 2 - line.start, true, &['+']).unwrap();
        assert_eq!(&line.text[found.0..found.1], "access shop");
        assert!(class_name("\\Drupal\\shop\\Catalog") && !class_name("shop.catalog") && !class_name("a\\b c"));
    }
}
