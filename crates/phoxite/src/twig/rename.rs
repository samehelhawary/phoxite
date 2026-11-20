//! Rename in Twig templates: blocks and macros.
//!
//! - **A block** is renamed in every template of its family: the templates
//!   reached from this one by `{% extends %}` and `{% use %}`, in either
//!   direction. There the `{% block name %}` tags, the names after
//!   `endblock` and the `block('name')` calls change. A block written
//!   inside `{% embed 'x' %}` overrides a block of `x`, so it belongs to
//!   the family of `x`, not to the one of the template it is written in.
//! - **A macro** is renamed where it is declared (with the name after
//!   `endmacro`) and where it is called: `_self.name()`, `alias.name()` for
//!   an alias importing its template, and the names of `{% from … import
//!   name %}` with the calls of a name imported without `as`.
//!
//! A template's own name isn't renamed here: PHP names templates too
//! (`render('page.html.twig')`), and a rename that leaves those behind
//! would break them.
//!
//! The templates read are the ones of the project's template directories
//! ([`TemplatePaths`]), from disk; the document the rename starts in is read
//! as it is in the editor.

use std::path::{Path, PathBuf};

use mago_twig_syntax::cst::{
    Argument, Block, BlockBody, Call, Embed, Expression, Extends, From, Import, Macro, MethodCall, Template, Use,
};
use mago_twig_syntax::walker::Walker;

use super::templates::TemplatePaths;
use super::{string_content, utf8, with_template};
use crate::base::{Pos, Range};
use crate::project::normalize;
use crate::syntax::LineIndex;

/// A name in a template, with its byte range.
#[derive(Clone, Debug)]
struct Named {
    name: String,
    start: usize,
    end: usize,
}

impl Named {
    fn contains(&self, offset: usize) -> bool {
        self.start <= offset && offset <= self.end
    }
}

/// A block's name, and the `{% embed %}` it is written in: `None` outside
/// one, else the embedded template (`None` when its name isn't a string).
#[derive(Clone, Debug)]
struct BlockName {
    named: Named,
    embed: Option<Option<String>>,
}

/// A name of `{% from 'x' import original as alias %}`.
struct FromItem {
    original: Named,
    aliased: bool,
    /// `None` for `_self`.
    template: Option<String>,
}

/// The names of one template that a rename may change.
#[derive(Default)]
struct Names {
    /// `{% block name %}`, the names after `endblock` and the strings of
    /// `block('name')`.
    blocks: Vec<BlockName>,
    /// The `{% embed %}`s around what is being walked, innermost last.
    embeds: Vec<Option<String>>,
    /// The templates of `{% extends %}` and `{% use %}` that are strings.
    parents: Vec<String>,
    /// `{% macro name %}` and the names after `endmacro`.
    macros: Vec<Named>,
    /// `{% import 'x' as alias %}`: the alias and the template (`None` for `_self`).
    imports: Vec<(String, Option<String>)>,
    from_items: Vec<FromItem>,
    /// `object.name(…)`: the object when it is a plain name, and the method.
    method_calls: Vec<(String, Named)>,
    /// `name(…)`.
    calls: Vec<Named>,
}

fn names(text: &str) -> Names {
    let length = text.len();
    let mut found = with_template(text, |_, template: &Template<'_>| {
        let mut found = Names::default();
        NameCollector.walk_template(template, &mut found);
        found
    });
    // Not what is in the end tags the recovery appended.
    found.blocks.retain(|block| block.named.end <= length);
    found.macros.retain(|named| named.end <= length);
    found
}

impl Names {
    fn block(&mut self, named: Named) {
        self.blocks.push(BlockName { named, embed: self.embeds.last().cloned() });
    }
}

fn named(value: &[u8], span: mago_span::Span) -> Named {
    Named { name: utf8(value), start: span.start.offset as usize, end: span.end.offset as usize }
}

/// The name of a template expression that is a string.
fn template_name(expression: &Expression<'_>) -> Option<String> {
    match expression {
        Expression::String(string) => Some(string_content(string.raw, string.span).0),
        _ => None,
    }
}

struct NameCollector;

impl<'ast, 'arena> Walker<'ast, 'arena, Names> for NameCollector {
    fn walk_in_block(&self, node: &'ast Block<'arena>, found: &mut Names) {
        found.block(named(node.name.value, node.name.span));
        if let BlockBody::Long(long) = &node.body
            && let Some(end) = &long.end_name
        {
            found.block(named(end.value, end.span));
        }
    }

    fn walk_in_embed(&self, node: &'ast Embed<'arena>, found: &mut Names) {
        found.embeds.push(template_name(&node.template));
    }

    fn walk_out_embed(&self, _: &'ast Embed<'arena>, found: &mut Names) {
        found.embeds.pop();
    }

    fn walk_in_macro(&self, node: &'ast Macro<'arena>, found: &mut Names) {
        found.macros.push(named(node.name.value, node.name.span));
        if let Some(end) = &node.end_name {
            found.macros.push(named(end.value, end.span));
        }
    }

    fn walk_in_extends(&self, node: &'ast Extends<'arena>, found: &mut Names) {
        found.parents.extend(template_name(&node.template));
    }

    fn walk_in_use(&self, node: &'ast Use<'arena>, found: &mut Names) {
        found.parents.extend(template_name(&node.template));
    }

    fn walk_in_import(&self, node: &'ast Import<'arena>, found: &mut Names) {
        found.imports.push((utf8(node.alias.value), template_name(&node.template)));
    }

    fn walk_in_from(&self, node: &'ast From<'arena>, found: &mut Names) {
        let template = template_name(&node.template);
        for imported in node.names.iter() {
            found.from_items.push(FromItem {
                original: named(imported.from.value, imported.from.span),
                aliased: imported.to.is_some(),
                template: template.clone(),
            });
        }
    }

    fn walk_in_method_call(&self, node: &'ast MethodCall<'arena>, found: &mut Names) {
        if let Expression::Name(object) = node.object {
            found.method_calls.push((utf8(object.name), named(node.method.value, node.method.span)));
        }
    }

    fn walk_in_call(&self, node: &'ast Call<'arena>, found: &mut Names) {
        let Expression::Name(callee) = node.callee else { return };
        found.calls.push(named(callee.name, callee.span));
        if callee.name != b"block" {
            return;
        }
        let first = node.argument_list.arguments.iter().find_map(|argument| match argument {
            Argument::Positional(positional) if positional.ellipsis.is_none() => Some(positional.value),
            _ => None,
        });
        if let Some(Expression::String(string)) = first {
            let (name, start, end) = string_content(string.raw, string.span);
            found.block(Named { name, start: start as usize, end: end as usize });
        }
    }
}

/// What the caret is on.
enum Subject {
    /// A block of the family of the template at this path.
    Block(PathBuf, String),
    /// A macro of the template at this path.
    Macro(PathBuf, String),
}

/// Whether `template`, named in the template at `here`, is the one at `target`.
fn names_template(template: &Option<String>, here: &Path, target: &Path, paths: &TemplatePaths) -> bool {
    match template {
        None => here == target,
        Some(name) => paths.resolve(name).is_some_and(|path| normalize(&path) == target),
    }
}

/// What is renamed at byte `offset` of the template at `path`, and the
/// name's byte range there.
fn subject(found: &Names, offset: usize, path: &Path, paths: &TemplatePaths) -> Option<(Subject, Named)> {
    if let Some(block) = found.blocks.iter().find(|block| block.named.contains(offset)) {
        let root = match &block.embed {
            None => path.to_path_buf(),
            // Inside an embed: a block of the embedded template.
            Some(embedded) => normalize(&paths.resolve(embedded.as_deref()?)?),
        };
        return Some((Subject::Block(root, block.named.name.clone()), block.named.clone()));
    }
    if let Some(declared) = found.macros.iter().find(|named| named.contains(offset)) {
        return Some((Subject::Macro(path.to_path_buf(), declared.name.clone()), declared.clone()));
    }
    let resolved = |template: &Option<String>| match template {
        None => Some(path.to_path_buf()),
        Some(name) => paths.resolve(name).map(|path| normalize(&path)),
    };
    if let Some((object, method)) = found.method_calls.iter().find(|(_, method)| method.contains(offset)) {
        let template = if object == "_self" {
            Some(path.to_path_buf())
        } else {
            let (_, template) = found.imports.iter().find(|(alias, _)| alias == object)?;
            resolved(template)
        };
        return Some((Subject::Macro(template?, method.name.clone()), method.clone()));
    }
    if let Some(item) = found.from_items.iter().find(|item| item.original.contains(offset)) {
        return Some((Subject::Macro(resolved(&item.template)?, item.original.name.clone()), item.original.clone()));
    }
    // A call of a name imported without `as`.
    let call = found.calls.iter().find(|call| call.contains(offset))?;
    let item = found.from_items.iter().find(|item| !item.aliased && item.original.name == call.name)?;
    Some((Subject::Macro(resolved(&item.template)?, call.name.clone()), call.clone()))
}

/// The range a rename at `pos` changes: the name of a block or of a macro.
pub fn prepare_rename(text: &str, pos: Pos, path: &Path, paths: &TemplatePaths) -> Option<Range> {
    let index = LineIndex::new(text);
    let offset = index.byte(pos) as usize;
    let (_, at) = subject(&names(text), offset, &normalize(path), paths)?;
    Some(index.range(at.start as u32, at.end as u32))
}

/// Whether `name` can be the name of a block or a macro.
fn valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// What a rename replaces with the new name: the ranges, by file.
pub type Renamed = Vec<(PathBuf, Vec<Range>)>;

/// Renames the block or macro at `pos` of the template at `path` (whose
/// text in the editor is `text`) to `new_name`: the ranges to replace, by
/// file. `None` when the caret is on neither; an error when the name isn't
/// one a block or macro can have.
pub fn rename(
    text: &str,
    pos: Pos,
    path: &Path,
    new_name: &str,
    paths: &TemplatePaths,
) -> Option<Result<Renamed, String>> {
    let path = normalize(path);
    let offset = LineIndex::new(text).byte(pos) as usize;
    let here = names(text);
    let (subject, _) = subject(&here, offset, &path, paths)?;
    if !valid_name(new_name) {
        return Some(Err(format!("'{new_name}' is not a valid name")));
    }
    // Every template of the project, this one as it is in the editor.
    let mut templates: Vec<(PathBuf, String, Names)> = vec![(path.clone(), text.to_owned(), here)];
    for name in paths.names() {
        let Some(file) = paths.resolve(&name).map(|file| normalize(&file)) else { continue };
        if file == path {
            continue;
        }
        let Ok(source) = std::fs::read_to_string(&file) else { continue };
        let source = source.replace("\r\n", "\n");
        let found = names(&source);
        templates.push((file, source, found));
    }
    let mut out: Vec<(PathBuf, Vec<Range>)> = Vec::new();
    match &subject {
        Subject::Block(root, name) => {
            let family = family(&templates, root, paths);
            for (file, source, found) in &templates {
                // A block is of the family of the template it is written in,
                // or of the one its embed names.
                let of_family = |block: &&BlockName| match &block.embed {
                    None => family.contains(file),
                    Some(None) => false,
                    Some(Some(embedded)) => {
                        paths.resolve(embedded).is_some_and(|embedded| family.contains(&normalize(&embedded)))
                    }
                };
                let spans = found.blocks.iter().filter(|block| block.named.name == *name).filter(of_family);
                push(&mut out, file, source, spans.map(|block| block.named.clone()).collect());
            }
        }
        Subject::Macro(declared, name) => {
            for (file, source, found) in &templates {
                let mut spans: Vec<Named> = Vec::new();
                if file == declared {
                    spans.extend(found.macros.iter().filter(|named| named.name == *name).cloned());
                }
                let imports = |object: &str| {
                    (object == "_self" && file == declared)
                        || found
                            .imports
                            .iter()
                            .any(|(alias, template)| alias == object && names_template(template, file, declared, paths))
                };
                spans.extend(
                    found
                        .method_calls
                        .iter()
                        .filter(|(object, method)| method.name == *name && imports(object))
                        .map(|(_, method)| method.clone()),
                );
                let mut called = false;
                for item in &found.from_items {
                    if item.original.name == *name && names_template(&item.template, file, declared, paths) {
                        spans.push(item.original.clone());
                        called |= !item.aliased;
                    }
                }
                if called {
                    spans.extend(found.calls.iter().filter(|call| call.name == *name).cloned());
                }
                push(&mut out, file, source, spans);
            }
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Some(Ok(out))
}

fn push(out: &mut Vec<(PathBuf, Vec<Range>)>, file: &Path, source: &str, mut spans: Vec<Named>) {
    if spans.is_empty() {
        return;
    }
    spans.sort_by_key(|named| named.start);
    spans.dedup_by_key(|named| named.start);
    let index = LineIndex::new(source);
    let ranges = spans.iter().map(|named| index.range(named.start as u32, named.end as u32)).collect();
    out.push((file.to_path_buf(), ranges));
}

/// The templates reached from the one at `start` by `extends` and `use`, in
/// either direction (with `start`).
fn family(templates: &[(PathBuf, String, Names)], start: &Path, paths: &TemplatePaths) -> Vec<PathBuf> {
    let edges: Vec<(&Path, PathBuf)> = templates
        .iter()
        .flat_map(|(file, _, found)| {
            found
                .parents
                .iter()
                .filter_map(|parent| paths.resolve(parent))
                .map(move |parent| (file.as_path(), normalize(&parent)))
        })
        .collect();
    let mut family: Vec<PathBuf> = vec![start.to_path_buf()];
    let mut next = 0;
    while next < family.len() {
        let current = family[next].clone();
        next += 1;
        for (child, parent) in &edges {
            let other = if *child == current {
                parent.as_path()
            } else if *parent == current {
                child
            } else {
                continue;
            };
            if !family.iter().any(|known| known == other) {
                family.push(other.to_path_buf());
            }
        }
    }
    family
}
