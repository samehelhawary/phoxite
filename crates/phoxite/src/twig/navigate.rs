//! Problems, hover, goto and usages in Twig templates.
//!
//! - **Problems** ([`problems`]): the template's syntax errors, as
//!   diagnostics of source [`DIAGNOSTIC_SOURCE`].
//! - **Hover** ([`hover`]): what a built-in tag, filter, function, test or
//!   global does, and the arguments of a macro in scope.
//! - **Goto** ([`definition`]): a template name goes to its file; a macro
//!   call goes to the macro, in this template (`_self.input()`, `input()`)
//!   or in the one it is imported from (`forms.input()` after `{% import
//!   'forms.html.twig' as forms %}`, a name of `{% from … import … %}`); the
//!   name of a `{% block %}` goes to the block it overrides in the parent
//!   template (`{% extends %}`, followed up to [`MAX_PARENTS`] parents), and
//!   one written inside `{% embed 'x' %}` to the block of `x` it overrides.
//! - **Usages** ([`template_usages`]): the places the project's templates
//!   name a template.
//!
//! Like the rest of the module these read the template's text and, for
//! what is in other templates, the files of the project's template
//! directories ([`TemplatePaths`]).

use std::path::{Path, PathBuf};

use mago_twig_syntax::cst::{Argument, Block, Call, Embed, Expression, Extends, Filter, From, Import, Macro, Template};
use mago_twig_syntax::walker::Walker;

use super::builtins::{self, Builtin};
use super::complete::{Context, context_at};
use super::recover::{blank, syntax_problems};
use super::templates::TemplatePaths;
use super::{ScopeKind, scope_names, string_content, template_at, template_refs, utf8, with_template};
use crate::base::{Diagnostic, Pos, Range, Severity};
use crate::syntax::LineIndex;

/// The `source` of a Twig template's diagnostics.
pub const DIAGNOSTIC_SOURCE: &str = "twig";

/// The code of a syntax error.
pub const SYNTAX_CODE: &str = "TwigSyntax";

/// How many parent templates goto follows for a block.
pub const MAX_PARENTS: usize = 8;

/// The template's syntax errors. A tag Twig itself doesn't have is not one:
/// extensions add tags.
pub fn problems(text: &str) -> Vec<Diagnostic> {
    let index = LineIndex::new(text);
    syntax_problems(text)
        .into_iter()
        .map(|problem| Diagnostic {
            range: index.range(problem.start as u32, problem.end as u32),
            severity: Severity::Error,
            message: problem.message,
            source: Some(DIAGNOSTIC_SOURCE.into()),
            code: Some(SYNTAX_CODE.into()),
            unnecessary: false,
            deprecated: false,
            server: None,
        })
        .collect()
}

/// Markdown about the name at `pos` and the range of that name: a built-in
/// (with what the Symfony bridge adds when `symfony`), or a macro in scope.
pub fn hover(text: &str, pos: Pos, symfony: bool) -> Option<(String, Range)> {
    let index = LineIndex::new(text);
    let offset = index.byte(pos) as usize;
    let found = context_at(text, offset)?;
    let word = text.get(found.start..found.end).filter(|word| !word.is_empty())?;
    let range = index.range(found.start as u32, found.end as u32);
    let named = |lists: &[&'static [Builtin]]| lists.iter().flat_map(|list| list.iter()).find(|b| b.name == word);
    let symfony_or = |list: &'static [Builtin]| if symfony { list } else { &[] };
    let (what, builtin) = match found.context {
        Context::Tag => ("tag", named(&[builtins::TAGS, symfony_or(builtins::SYMFONY_TAGS)])),
        Context::Filter => ("filter", named(&[builtins::FILTERS, symfony_or(builtins::SYMFONY_FILTERS)])),
        Context::Test => ("test", named(&[builtins::TESTS])),
        Context::Template | Context::Route => return None,
        Context::Attribute => {
            // `_self.button`: a macro of this template.
            if object_before(text, found.start) != "_self" {
                return None;
            }
            let blanked = blank(text, found.tag.0, found.tag.1);
            let names = with_template(&blanked, |text, template| scope_names(text, template, offset as u32));
            let name = names.iter().find(|name| name.name == word && name.kind == ScopeKind::Macro)?;
            let detail = name.detail.clone().unwrap_or_default();
            return Some((format!("```twig\nmacro {word}{detail}\n```"), range));
        }
        Context::Expression => {
            // A macro or variable of the template wins over a built-in of the same name.
            let blanked = blank(text, found.tag.0, found.tag.1);
            let names = with_template(&blanked, |text, template| scope_names(text, template, offset as u32));
            if let Some(name) = names.iter().find(|name| name.name == word) {
                return match name.kind {
                    ScopeKind::Macro => {
                        let detail = name.detail.clone().unwrap_or_default();
                        // A macro of this template has its arguments, an imported one its template.
                        let markdown = if detail.starts_with('(') {
                            format!("```twig\nmacro {word}{detail}\n```")
                        } else {
                            format!("```twig\nmacro {word}\n```\n\nFrom `{detail}`")
                        };
                        Some((markdown, range))
                    }
                    ScopeKind::MacroNamespace => {
                        let detail = name.detail.clone().unwrap_or_default();
                        Some((format!("The macros of `{detail}`"), range))
                    }
                    ScopeKind::Loop => Some((format!("```twig\nloop\n```\n\n{}", builtins::LOOP.doc), range)),
                    ScopeKind::Variable => None,
                };
            }
            let functions = named(&[builtins::FUNCTIONS, symfony_or(builtins::SYMFONY_FUNCTIONS)]);
            match functions {
                Some(function) => ("function", Some(function)),
                None => ("variable", named(&[builtins::GLOBALS, symfony_or(builtins::SYMFONY_GLOBALS)])),
            }
        }
    };
    let builtin = builtin?;
    let signature = builtin.signature.unwrap_or_default();
    Some((format!("```twig\n{}{signature}\n```\n\n{} ({what})", builtin.name, builtin.doc), range))
}

/// Where goto definition goes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TwigTarget {
    /// The file; `None` for the template itself.
    pub path: Option<PathBuf>,
    /// The name declared there; the start of the file for a template.
    pub range: Range,
}

/// Goto definition at `pos`; see the module documentation. Empty when the
/// caret is on nothing goto knows, or on something that isn't found.
pub fn definition(text: &str, pos: Pos, paths: &TemplatePaths) -> Vec<TwigTarget> {
    if let Some(found) = template_at(text, pos) {
        return paths
            .resolve(&found.name)
            .map(|path| TwigTarget { path: Some(path), range: Range::default() })
            .into_iter()
            .collect();
    }
    let index = LineIndex::new(text);
    let offset = index.byte(pos) as usize;
    let here = declarations(text);
    // The name of a `{% block %}`: the block it overrides.
    if let Some((block, embed)) = here.blocks.iter().find(|(block, _)| block.start <= offset && offset <= block.end) {
        let overridden = match embed {
            None => here.extends.clone(),
            // Inside an embed: a block of the embedded template.
            Some(embedded) => embedded.clone(),
        };
        return overridden.and_then(|from| block_from(from, &block.name, paths)).into_iter().collect();
    }
    let Some(found) = context_at(text, offset) else { return Vec::new() };
    if !matches!(found.context, Context::Expression | Context::Attribute) {
        return Vec::new();
    }
    let Some(word) = text.get(found.start..found.end).filter(|word| !word.is_empty()) else { return Vec::new() };
    let local = |name: &str| {
        here.macros
            .iter()
            .find(|declared| declared.name == name)
            .map(|declared| TwigTarget { path: None, range: index.range(declared.start as u32, declared.end as u32) })
    };
    // `forms.input`, `_self.input`: a macro of the template before the dot.
    if found.context == Context::Attribute {
        let object = object_before(text, found.start);
        if object == "_self" {
            return local(word).into_iter().collect();
        }
        let Some(import) = here.imports.iter().find(|import| import.alias == object) else { return Vec::new() };
        return match &import.template {
            // `{% import _self as forms %}`.
            None => local(word).into_iter().collect(),
            Some(template) => macro_in(template, word, paths).into_iter().collect(),
        };
    }
    // A name imported with `{% from %}`, else a macro of this template.
    if let Some(imported) = here.imported.iter().find(|imported| imported.name == word) {
        return match &imported.template {
            None => local(&imported.original).into_iter().collect(),
            Some(template) => macro_in(template, &imported.original, paths).into_iter().collect(),
        };
    }
    // Not from its own declaration.
    local(word).filter(|target| index.byte(target.range.start) as usize != found.start).into_iter().collect()
}

/// The name before the `.` that precedes byte `at` (`forms` for
/// `forms.input`); empty when what is there isn't a name.
fn object_before(text: &str, at: usize) -> String {
    let before = text[..at].trim_end();
    let Some(object) = before.strip_suffix('.') else { return String::new() };
    let start = object
        .char_indices()
        .rev()
        .take_while(|(_, c)| c.is_ascii_alphanumeric() || *c == '_')
        .last()
        .map_or(object.len(), |(at, _)| at);
    object[start..].to_owned()
}

/// The macro `name` of the template named `template`.
fn macro_in(template: &str, name: &str, paths: &TemplatePaths) -> Option<TwigTarget> {
    let path = paths.resolve(template)?;
    let text = read(&path)?;
    let declared = declarations(&text);
    let found = declared.macros.iter().find(|declared| declared.name == name)?;
    let range = LineIndex::new(&text).range(found.start as u32, found.end as u32);
    Some(TwigTarget { path: Some(path), range })
}

/// The block `name` of the template named `parent`, else of the nearest of
/// its parents that has it.
fn block_from(mut parent: String, name: &str, paths: &TemplatePaths) -> Option<TwigTarget> {
    let mut seen: Vec<PathBuf> = Vec::new();
    for _ in 0..MAX_PARENTS {
        let path = paths.resolve(&parent)?;
        if seen.contains(&path) {
            return None;
        }
        let text = read(&path)?;
        let declared = declarations(&text);
        // Not a block the template writes inside an embed of its own.
        if let Some((block, _)) = declared.blocks.iter().find(|(block, embed)| embed.is_none() && block.name == name) {
            let range = LineIndex::new(&text).range(block.start as u32, block.end as u32);
            return Some(TwigTarget { path: Some(path), range });
        }
        parent = declared.extends?;
        seen.push(path);
    }
    None
}

fn read(path: &Path) -> Option<String> {
    std::fs::read_to_string(path).ok().map(|text| text.replace("\r\n", "\n"))
}

/// The places the project's templates name the template `name`
/// (`{% extends %}`, `{% include %}`, `include()`, …), by file.
pub fn template_usages(name: &str, paths: &TemplatePaths) -> Vec<(PathBuf, Range)> {
    template_usages_with(name, paths, &read)
}

/// [`template_usages`], the text of each template given by `text_of` (an
/// editor's text for the ones it has open).
pub fn template_usages_with(
    name: &str,
    paths: &TemplatePaths,
    text_of: &dyn Fn(&Path) -> Option<String>,
) -> Vec<(PathBuf, Range)> {
    // The last segment is in every file naming the template.
    let needle = name.rsplit('/').next().unwrap_or(name);
    let mut out = Vec::new();
    for template in paths.names() {
        let Some(path) = paths.resolve(&template) else { continue };
        let Some(text) = text_of(&path).filter(|text| text.contains(needle)) else { continue };
        for found in template_refs(&text) {
            if found.name == name {
                out.push((path.clone(), found.range));
            }
        }
    }
    out
}

/// The chain before the `.` the caret is after (`post`, `author` for
/// `post.author.na|`), and the range of the name being written after it.
/// `None` when the caret isn't after a `.`, or what is before it isn't a
/// chain of names (a call, an index).
pub fn access_chain_at(text: &str, pos: Pos) -> Option<(Vec<String>, Range)> {
    let index = LineIndex::new(text);
    let found = context_at(text, index.byte(pos) as usize)?;
    if found.context != Context::Attribute {
        return None;
    }
    let is_name = |c: char| c.is_ascii_alphanumeric() || c == '_';
    let mut chain: Vec<String> = Vec::new();
    let mut rest = text[..found.start].trim_end();
    while let Some(before) = rest.strip_suffix('.') {
        let before = before.trim_end();
        let start = before.rfind(|c: char| !is_name(c)).map_or(0, |at| at + 1);
        let name = &before[start..];
        if name.is_empty() || name.starts_with(|c: char| c.is_ascii_digit()) {
            return None;
        }
        chain.insert(0, name.to_owned());
        rest = before[..start].trim_end();
    }
    // What is before the chain mustn't be part of the expression (`f().a`, `a[0].b`).
    if rest.ends_with([')', ']', '}']) || chain.is_empty() {
        return None;
    }
    Some((chain, index.range(found.start as u32, found.end as u32)))
}

/// The `{% for %}` loops around byte `pos`, outermost first: each target
/// with the chain of names it goes through (`{% for c in post.comments %}`
/// is `c` over `post`, `comments`). A loop over anything else than a chain
/// of names is left out.
pub fn loops_at(text: &str, pos: Pos) -> Vec<(String, Vec<String>)> {
    let offset = LineIndex::new(text).byte(pos) as usize;
    let is_name = |c: char| c.is_ascii_alphanumeric() || c == '_';
    // Every `for` still open at the caret; `None` for one whose source isn't a chain.
    let mut open: Vec<Option<(String, Vec<String>)>> = Vec::new();
    let mut at = 0;
    while let Some(found) = text[at..].find("{%") {
        let start = at + found;
        if start >= offset {
            break;
        }
        let Some(close) = text[start..].find("%}") else { break };
        let inner = text[start + 2..start + close].trim_matches(|c: char| c == '-' || c == '~' || c.is_whitespace());
        at = start + close + 2;
        if inner == "endfor" || inner.starts_with("endfor ") {
            open.pop();
            continue;
        }
        let Some(rest) = inner.strip_prefix("for ") else { continue };
        let Some((targets, source)) = rest.split_once(" in ") else {
            open.push(None);
            continue;
        };
        // `for key, value in …`: the value is the element.
        let target = targets.rsplit(',').next().unwrap_or(targets).trim();
        // The source up to a filter or a condition.
        let source = source.split(['|', ' ']).next().unwrap_or_default().trim();
        let chain: Vec<String> = source.split('.').map(str::to_owned).collect();
        let named = !target.is_empty()
            && target.chars().all(is_name)
            && chain.iter().all(|name| !name.is_empty() && name.chars().all(is_name));
        open.push(named.then(|| (target.to_owned(), chain)));
    }
    open.into_iter().flatten().collect()
}

/// What kind of name is written at `pos`, and its range: a filter (after
/// `|`), a test (after `is`), or a function (elsewhere in an expression).
/// For the names a project's own extensions add, which the project answers.
pub fn name_kind_at(text: &str, pos: Pos) -> Option<(super::TwigCompletionKind, Range)> {
    use super::TwigCompletionKind as Kind;
    let index = LineIndex::new(text);
    let found = context_at(text, index.byte(pos) as usize)?;
    let kind = match found.context {
        Context::Filter => Kind::Filter,
        Context::Test => Kind::Test,
        Context::Expression => Kind::Function,
        Context::Tag | Context::Template | Context::Attribute | Context::Route => return None,
    };
    Some((kind, index.range(found.start as u32, found.end as u32)))
}

/// The range of the route name being written at `pos`: the string that is
/// the first argument of `path(` or `url(`, closed or not. For completion,
/// which the project's routes answer.
pub fn route_string_at(text: &str, pos: Pos) -> Option<Range> {
    let index = LineIndex::new(text);
    let found = context_at(text, index.byte(pos) as usize)?;
    (found.context == Context::Route).then(|| index.range(found.start as u32, found.end as u32))
}

/// The route names a template writes as the first argument of `path()` and
/// `url()`, in source order.
pub fn route_refs(text: &str) -> Vec<super::TemplateRef> {
    let index = LineIndex::new(text);
    let mut refs = with_template(text, |_, template: &Template<'_>| {
        let mut refs: Vec<(u32, u32, String)> = Vec::new();
        RouteCollector.walk_template(template, &mut refs);
        refs
    });
    refs.sort_by_key(|(start, ..)| *start);
    refs.into_iter()
        .filter(|(_, end, _)| *end <= index.len())
        .map(|(start, end, name)| super::TemplateRef { name, range: index.range(start, end) })
        .collect()
}

/// The route name at `pos`, if the caret is on one.
pub fn route_at(text: &str, pos: Pos) -> Option<super::TemplateRef> {
    route_refs(text).into_iter().find(|found| found.range.start <= pos && pos <= found.range.end)
}

struct RouteCollector;

impl<'ast, 'arena> Walker<'ast, 'arena, Vec<(u32, u32, String)>> for RouteCollector {
    fn walk_in_call(&self, node: &'ast Call<'arena>, refs: &mut Vec<(u32, u32, String)>) {
        let Expression::Name(callee) = node.callee else { return };
        if !matches!(callee.name, b"path" | b"url") {
            return;
        }
        let first = node.argument_list.arguments.iter().find_map(|argument| match argument {
            Argument::Positional(positional) if positional.ellipsis.is_none() => Some(positional.value),
            Argument::Named(named) if matches!(named.name.value, b"name" | b"route") => Some(named.value),
            _ => None,
        });
        if let Some(Expression::String(string)) = first {
            let (name, start, end) = string_content(string.raw, string.span);
            refs.push((start, end, name));
        }
    }
}

/// The range of the string at `pos` when the `trans` filter follows it
/// (`'form.ti|'|trans`): a translation key being written. For completion,
/// which the project's catalogues answer.
pub fn translation_string_at(text: &str, pos: Pos) -> Option<Range> {
    let index = LineIndex::new(text);
    let offset = index.byte(pos) as usize;
    let bytes = text.as_bytes();
    let line_start = text[..offset].rfind('\n').map_or(0, |at| at + 1);
    // The quote the caret is after, on its line.
    let mut open: Option<usize> = None;
    let mut at = line_start;
    while at < offset {
        match open {
            Some(_) if bytes[at] == b'\\' => at += 1,
            Some(quote) if bytes[at] == bytes[quote] => open = None,
            Some(_) => {}
            None if bytes[at] == b'\'' || bytes[at] == b'"' => open = Some(at),
            None => {}
        }
        at += 1;
    }
    let quote = open?;
    let mut end = offset;
    while end < bytes.len() && bytes[end] != bytes[quote] && bytes[end] != b'\n' {
        end += 1;
    }
    let after = text.get(end + 1..)?.trim_start().strip_prefix('|')?.trim_start();
    let filter: String = after.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
    (filter == "trans").then(|| index.range(quote as u32 + 1, end as u32))
}

/// The translation keys a template writes before the `trans` filter
/// (`'form.title'|trans`), in source order.
pub fn translation_refs(text: &str) -> Vec<super::TemplateRef> {
    let index = LineIndex::new(text);
    let mut refs = with_template(text, |_, template: &Template<'_>| {
        let mut refs: Vec<(u32, u32, String)> = Vec::new();
        TranslationCollector.walk_template(template, &mut refs);
        refs
    });
    refs.sort_by_key(|(start, ..)| *start);
    refs.into_iter()
        .filter(|(_, end, _)| *end <= index.len())
        .map(|(start, end, name)| super::TemplateRef { name, range: index.range(start, end) })
        .collect()
}

/// The translation key at `pos`, if the caret is on one.
pub fn translation_at(text: &str, pos: Pos) -> Option<super::TemplateRef> {
    translation_refs(text).into_iter().find(|found| found.range.start <= pos && pos <= found.range.end)
}

struct TranslationCollector;

impl<'ast, 'arena> Walker<'ast, 'arena, Vec<(u32, u32, String)>> for TranslationCollector {
    fn walk_in_filter(&self, node: &'ast Filter<'arena>, refs: &mut Vec<(u32, u32, String)>) {
        if node.name.value != b"trans" {
            return;
        }
        if let Expression::String(string) = node.operand {
            let (name, start, end) = string_content(string.raw, string.span);
            refs.push((start, end, name));
        }
    }
}

/// A name declared in a template, with the byte range of the name.
struct Named {
    name: String,
    start: usize,
    end: usize,
}

/// `{% import 'x' as alias %}`; `template` is `None` for `_self`.
struct ImportAlias {
    alias: String,
    template: Option<String>,
}

/// A name of `{% from 'x' import original as name %}`.
struct ImportedMacro {
    name: String,
    original: String,
    template: Option<String>,
}

/// What a template declares and imports.
#[derive(Default)]
struct Declarations {
    macros: Vec<Named>,
    /// Each with the `{% embed %}` it is written in: `None` outside one, else
    /// the embedded template (`None` when its name isn't a string).
    blocks: Vec<(Named, Option<Option<String>>)>,
    /// The `{% embed %}`s around what is being walked, innermost last.
    embeds: Vec<Option<String>>,
    imports: Vec<ImportAlias>,
    imported: Vec<ImportedMacro>,
    /// The parent template, when its name is a string.
    extends: Option<String>,
}

fn declarations(text: &str) -> Declarations {
    let length = text.len();
    let mut found = with_template(text, |_, template: &Template<'_>| {
        let mut found = Declarations::default();
        DeclarationCollector.walk_template(template, &mut found);
        found
    });
    // Not what is in the end tags the recovery appended.
    found.macros.retain(|named| named.end <= length);
    found.blocks.retain(|(named, _)| named.end <= length);
    found
}

/// The name of a template expression that is a string; `None` for `_self`
/// and for anything computed.
fn template_name(expression: &Expression<'_>) -> Option<String> {
    match expression {
        Expression::String(string) => Some(string_content(string.raw, string.span).0),
        _ => None,
    }
}

struct DeclarationCollector;

impl<'ast, 'arena> Walker<'ast, 'arena, Declarations> for DeclarationCollector {
    fn walk_in_macro(&self, node: &'ast Macro<'arena>, found: &mut Declarations) {
        found.macros.push(Named {
            name: utf8(node.name.value),
            start: node.name.span.start.offset as usize,
            end: node.name.span.end.offset as usize,
        });
    }

    fn walk_in_block(&self, node: &'ast Block<'arena>, found: &mut Declarations) {
        let embed = found.embeds.last().cloned();
        found.blocks.push((
            Named {
                name: utf8(node.name.value),
                start: node.name.span.start.offset as usize,
                end: node.name.span.end.offset as usize,
            },
            embed,
        ));
    }

    fn walk_in_embed(&self, node: &'ast Embed<'arena>, found: &mut Declarations) {
        found.embeds.push(template_name(&node.template));
    }

    fn walk_out_embed(&self, _: &'ast Embed<'arena>, found: &mut Declarations) {
        found.embeds.pop();
    }

    fn walk_in_import(&self, node: &'ast Import<'arena>, found: &mut Declarations) {
        found.imports.push(ImportAlias { alias: utf8(node.alias.value), template: template_name(&node.template) });
    }

    fn walk_in_from(&self, node: &'ast From<'arena>, found: &mut Declarations) {
        let template = template_name(&node.template);
        for imported in node.names.iter() {
            found.imported.push(ImportedMacro {
                name: utf8(imported.to.unwrap_or(imported.from).value),
                original: utf8(imported.from.value),
                template: template.clone(),
            });
        }
    }

    fn walk_in_extends(&self, node: &'ast Extends<'arena>, found: &mut Declarations) {
        if found.extends.is_none() {
            found.extends = template_name(&node.template);
        }
    }
}
