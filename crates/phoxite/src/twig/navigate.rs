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

use mago_twig_syntax::cst::{Block, Embed, Expression, Extends, From, Import, Macro, Template};
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
        Context::Template => return None,
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
    // The last segment is in every file naming the template.
    let needle = name.rsplit('/').next().unwrap_or(name);
    let mut out = Vec::new();
    for template in paths.names() {
        let Some(path) = paths.resolve(&template) else { continue };
        let Some(text) = read(&path).filter(|text| text.contains(needle)) else { continue };
        for found in template_refs(&text) {
            if found.name == name {
                out.push((path.clone(), found.range));
            }
        }
    }
    out
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
