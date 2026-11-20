//! What a Drupal template is given: the variables of its theme hook, with
//! the types the functions preparing them assign.
//!
//! `shop-book.html.twig` is the template of the theme hook `shop_book`
//! (`node--article.html.twig` of `node__article`, whose base hook is
//! `node`). `hook_theme()` names its variables; a preprocess function
//! (`<extension>_preprocess_<hook>()`, `template_preprocess_<hook>()`) sets
//! them with `$variables['name'] = …`, which says their types and adds
//! more. Every template also has the ones Drupal's theme documentation
//! lists ([`EVERYWHERE`]).

use std::path::Path;

use mago_span::HasSpan;
use mago_syntax::cst::{Expression, Literal, Variable};
use mago_syntax::walker::Walker;

use crate::engine::PhpEngine;
use crate::names::{FileNames, NameKind};
use crate::summary::{DeclaredKind, FileSummary};
use crate::types::{ExprTypes, Type};

const BACKSLASH: char = 92 as char;

/// What Drupal gives every template.
const EVERYWHERE: [&str; 9] = [
    "attributes",
    "title_attributes",
    "content_attributes",
    "title_prefix",
    "title_suffix",
    "directory",
    "logged_in",
    "is_admin",
    "is_front",
];

struct Assigned<'a> {
    /// The assignments to an element of an array, in source order.
    found: Vec<(&'a Expression<'a>, &'a Expression<'a>)>,
}

struct AssignedWalker;

impl<'a> Walker<'a, 'a, Assigned<'a>> for AssignedWalker {
    fn walk_in_expression(&self, expr: &'a Expression<'a>, cx: &mut Assigned<'a>) {
        if let Expression::Assignment(assignment) = expr
            && matches!(assignment.lhs, Expression::ArrayAccess(_))
        {
            cx.found.push((assignment.lhs, assignment.rhs));
        }
    }
}

struct Rendered<'a> {
    hook: &'a str,
    /// The other entries of the arrays whose `#theme` is the hook: the key
    /// without its `#`, and the value.
    found: Vec<(String, &'a Expression<'a>)>,
    /// The variables given the hook as their `#theme` (by an array assigned
    /// to them, or `$build['#theme'] = …`), and where.
    themed: Vec<(&'a [u8], u32)>,
    /// `$build['#book'] = $book`: the variable, the key without its `#`,
    /// the value, and where.
    assigned: Vec<(&'a [u8], String, &'a Expression<'a>, u32)>,
    /// Where an array theming the hook is written.
    literal: Vec<u32>,
    /// `$build = shop_build();`: the variable, the function called, and
    /// where.
    called: Vec<(&'a [u8], String, u32)>,
}

struct RenderedWalker;

fn text(expr: &Expression<'_>) -> Option<String> {
    match expr {
        Expression::Literal(Literal::String(string)) => {
            string.value.map(|value| String::from_utf8_lossy(value).into_owned())
        }
        _ => None,
    }
}

impl<'a> Walker<'a, 'a, Rendered<'a>> for RenderedWalker {
    fn walk_in_expression(&self, expr: &'a Expression<'a>, cx: &mut Rendered<'a>) {
        // A render array built up: `$build['#theme'] = 'shop_book'; $build['#book'] = $book;`.
        if let Expression::Assignment(assignment) = expr {
            let at = assignment.lhs.span().start.offset;
            match assignment.lhs {
                Expression::ArrayAccess(access) => {
                    if let (Expression::Variable(Variable::Direct(variable)), Some(key)) =
                        (access.array, text(access.index))
                        && let Some(name) = key.strip_prefix('#')
                    {
                        if name == "theme" {
                            if text(assignment.rhs).as_deref() == Some(cx.hook) {
                                cx.themed.push((variable.name, at));
                            }
                        } else {
                            cx.assigned.push((variable.name, name.to_owned(), assignment.rhs, at));
                        }
                    }
                }
                Expression::Variable(Variable::Direct(variable)) if themes(assignment.rhs, cx.hook) => {
                    cx.themed.push((variable.name, at));
                }
                Expression::Variable(Variable::Direct(variable)) => {
                    if let Expression::Call(mago_syntax::cst::Call::Function(call)) = assignment.rhs
                        && let Expression::Identifier(name) = call.function
                    {
                        cx.called.push((variable.name, String::from_utf8_lossy(name.value()).into_owned(), at));
                    }
                }
                _ => {}
            }
        }
        if !themes(expr, cx.hook) {
            return;
        }
        cx.literal.push(expr.span().start.offset);
        for (key, value) in entries(expr) {
            if let Some(name) = key.strip_prefix('#').filter(|name| *name != "theme") {
                cx.found.push((name.to_owned(), value));
            }
        }
    }
}

/// The entries of the array `expr` is that have a string key.
fn entries<'a>(expr: &'a Expression<'a>) -> Vec<(String, &'a Expression<'a>)> {
    let elements = match expr {
        Expression::Array(array) => &array.elements,
        Expression::LegacyArray(array) => &array.elements,
        _ => return Vec::new(),
    };
    elements
        .iter()
        .filter_map(|element| match element {
            mago_syntax::cst::ArrayElement::KeyValue(element) => Some((text(element.key)?, element.value)),
            _ => None,
        })
        .collect()
}

/// Whether `expr` is an array whose `#theme` is `hook`.
fn themes(expr: &Expression<'_>, hook: &str) -> bool {
    entries(expr).iter().any(|(key, value)| key == "#theme" && text(value).as_deref() == Some(hook))
}

impl PhpEngine {
    /// What the render arrays of the theme hook `hook` pass it: for
    /// `['#theme' => 'shop_book', '#book' => $book]`, `book` and the type of
    /// `$book`. Only the variables in `declared`: a render array has other
    /// properties too.
    fn rendered(&self, hook: &str, declared: &[String]) -> Vec<(String, Type)> {
        let kind: &'static dyn crate::frameworks::refs::ReferenceKind = &crate::frameworks::drupal::refs::THEME_HOOK;
        let Ok(candidates) = self.usage_candidates(kind, hook) else { return Vec::new() };
        let open: Vec<_> = self.open_documents().into_iter().filter(|document| !document.embedded).collect();
        let mut out = Vec::new();
        // The files naming the hook, and the functions of theirs that build an array for it.
        let mut builders: Vec<String> = Vec::new();
        for candidate in &candidates {
            builders.extend(self.rendered_in(candidate, &open, hook, hook, &[], declared, &mut out));
        }
        builders.sort();
        builders.dedup();
        // The other files calling one of those functions: `$build = shop_build(); $build['#book'] = $book;`.
        let mut seen = candidates;
        for builder in &builders {
            let Ok(callers) = self.usage_candidates(kind, builder) else { continue };
            for caller in callers {
                if !seen.contains(&caller) {
                    self.rendered_in(&caller, &open, "", builder, &builders, declared, &mut out);
                    seen.push(caller);
                }
            }
        }
        out
    }

    /// What the file at `path` passes the theme hook `hook` (those of
    /// `declared`), added to `out`: by the arrays theming it, and by what is
    /// assigned to a variable given one, in place or by one of `builders`
    /// (functions of other files returning such an array). `word` is a text
    /// the file has. Returns the functions of the file that build such an
    /// array.
    #[allow(clippy::too_many_arguments)]
    fn rendered_in(
        &self,
        path: &Path,
        open: &[crate::engine::OpenDocument],
        hook: &str,
        word: &str,
        builders: &[String],
        declared: &[String],
        out: &mut Vec<(String, Type)>,
    ) -> Vec<String> {
        let Some(file) = self.usage_file(path, open, word) else { return Vec::new() };
        let mut rendered = Rendered {
            hook,
            found: Vec::new(),
            themed: Vec::new(),
            assigned: Vec::new(),
            literal: Vec::new(),
            called: Vec::new(),
        };
        RenderedWalker.walk_program(file.parsed.program(), &mut rendered);
        let summary = FileSummary::extract(&file.parsed);
        // What is assigned to a themed variable in the function or method theming it.
        let bodies: Vec<(u32, u32)> = summary
            .functions
            .iter()
            .map(|function| (function.loc.start, function.loc.end))
            .chain(summary.class_likes.iter().flat_map(|class| class.methods.iter().map(|m| (m.loc.start, m.loc.end))))
            .collect();
        // The functions of the file writing an array that themes the hook.
        let own: Vec<String> = summary
            .functions
            .iter()
            .filter(|function| rendered.literal.iter().any(|at| function.loc.start <= *at && *at <= function.loc.end))
            .map(|function| function.name.clone())
            .collect();
        for (variable, called, at) in &rendered.called {
            let short = called.rsplit(BACKSLASH).next().unwrap_or(called);
            if own.iter().chain(builders).any(|builder| builder.eq_ignore_ascii_case(short)) {
                rendered.themed.push((variable, *at));
            }
        }
        for (variable, at) in &rendered.themed {
            let inside =
                bodies.iter().filter(|(start, end)| start <= at && at <= end).min_by_key(|(start, end)| end - start);
            let (start, end) = inside.copied().unwrap_or((0, u32::MAX));
            for (assigned, name, value, at) in &rendered.assigned {
                if assigned == variable && start <= *at && *at <= end {
                    rendered.found.push((name.clone(), value));
                }
            }
        }
        if rendered.found.is_empty() {
            return own;
        }
        let resolver = self.resolver();
        let names = FileNames::resolve(&file.parsed);
        let types = ExprTypes::new(&file.parsed, &names, &summary, &resolver);
        for (name, value) in rendered.found {
            if !declared.contains(&name) {
                continue;
            }
            let frames = types.find(value.span().start.offset).map(|(_, frames)| frames).unwrap_or_default();
            out.push((name, types.type_of(value, &frames).widen()));
        }
        own
    }

    /// What the function `fqn` assigns to the elements of its first
    /// parameter by name: `$variables['price'] = $book->price();`.
    fn preprocessed(&self, fqn: &str) -> Vec<(String, Type)> {
        let mut out = Vec::new();
        let open: Vec<_> = self.open_documents().into_iter().filter(|document| !document.embedded).collect();
        let resolver = self.resolver();
        for target in self.declarations_of(NameKind::Function, fqn, Path::new(""), &FileSummary::default()) {
            let Some(file) = self.usage_file(&target.path, &open, fqn) else { continue };
            let summary = FileSummary::extract(&file.parsed);
            let Some(function) = summary.functions.iter().find(|function| function.fqn.eq_ignore_ascii_case(fqn))
            else {
                continue;
            };
            let Some(parameter) = function.params.first().map(|param| format!("${}", param.name)) else { continue };
            let names = FileNames::resolve(&file.parsed);
            let types = ExprTypes::new(&file.parsed, &names, &summary, &resolver);
            let mut assigned = Assigned { found: Vec::new() };
            AssignedWalker.walk_program(file.parsed.program(), &mut assigned);
            for (target, value) in assigned.found {
                let at = target.span().start.offset;
                if at < function.loc.start || at > function.loc.end {
                    continue;
                }
                let Expression::ArrayAccess(access) = target else { continue };
                let Expression::Variable(Variable::Direct(variable)) = access.array else { continue };
                let Expression::Literal(Literal::String(key)) = access.index else { continue };
                let Some(key) = key.value.map(|key| String::from_utf8_lossy(key).into_owned()) else { continue };
                if variable.name != parameter.as_bytes() {
                    continue;
                }
                let frames = types.find(value.span().start.offset).map(|(_, frames)| frames).unwrap_or_default();
                out.push((key, types.type_of(value, &frames).widen()));
            }
        }
        out
    }

    /// The variables of the template named `name`
    /// (`@shop/shop-book.html.twig`), sorted; see the module documentation.
    /// Empty for a template that is no theme hook's.
    pub(crate) fn drupal_template_variables(&self, name: &str) -> Vec<(String, Type)> {
        let file = name.rsplit('/').next().unwrap_or(name);
        let Some(hook) = file.strip_suffix(".html.twig").map(|hook| hook.replace('-', "_")) else { return Vec::new() };
        let names = self.names();
        let declared = |hook: &str| names.named(&[DeclaredKind::DrupalThemeHook], hook).into_iter().next();
        // A suggestion's variables are its base hook's, and both are prepared for it.
        let mut hooks = vec![hook.clone()];
        let mut base = hook.as_str();
        let mut found = declared(base);
        while found.is_none() {
            let Some((shorter, _)) = base.rsplit_once("__") else { break };
            base = shorter;
            hooks.push(base.to_owned());
            found = declared(base);
        }
        let Some(found) = found else { return Vec::new() };
        let mut variables: Vec<(String, Vec<Type>)> =
            found.classes.iter().map(|name| (name.clone(), Vec::new())).collect();
        variables.extend(EVERYWHERE.iter().map(|name| ((*name).to_owned(), Vec::new())));
        let project = self.drupal();
        let prefixes =
            project.extensions.iter().map(|extension| extension.name.as_str()).chain(std::iter::once("template"));
        for prefix in prefixes {
            for hook in &hooks {
                for (name, ty) in self.preprocessed(&format!("{prefix}_preprocess_{hook}")) {
                    let types = match variables.iter_mut().find(|(known, _)| *known == name) {
                        Some((_, types)) => types,
                        None => {
                            variables.push((name, Vec::new()));
                            &mut variables.last_mut().expect("just pushed").1
                        }
                    };
                    if ty != Type::Mixed && !types.contains(&ty) {
                        types.push(ty);
                    }
                }
            }
        }
        // What the code rendering the hook gives it.
        for hook in &hooks {
            for (name, ty) in self.rendered(hook, &found.classes) {
                if let Some((_, types)) = variables.iter_mut().find(|(known, _)| *known == name)
                    && ty != Type::Mixed
                    && ty != Type::Null
                    && !types.contains(&ty)
                {
                    types.push(ty);
                }
            }
        }
        variables.sort_by(|a, b| a.0.cmp(&b.0));
        variables.dedup_by(|b, a| a.0 == b.0);
        variables
            .into_iter()
            .map(|(name, types)| (name, if types.is_empty() { Type::Mixed } else { Type::union(types) }))
            .collect()
    }
}
