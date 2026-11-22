//! What a CakePHP template is given. `templates/Articles/view.php` is the
//! template of `ArticlesController::view()`: its variables are what the
//! action sets (`$this->set('article', $article)`, `$this->set(compact('article'))`,
//! `$this->set(['article' => $article])`, `viewBuilder()->setVar()`), with
//! what `beforeFilter()`, `beforeRender()` and `initialize()` of the
//! controller and of the classes it extends set for every action. A method
//! that renders the template by name (`$this->render('edit')`) gives it its
//! variables too. A cell's template has what its method sets, a mail
//! template what a mailer's method of its name passes (`setViewVars()`),
//! and an element what `$this->element('name', [...])` passes.
//!
//! `$this` in a template is the view: the application's `AppView`, else
//! CakePHP's `View`.
//!
//! The types come to the template's own inference through
//! [`Declarations::view_variable`](crate::types::Declarations::view_variable).

use std::path::Path;
use std::sync::Arc;

use mago_syntax::cst::{Argument, Call, ClassLikeMemberSelector, Expression, Method, MethodCall, Variable};
use mago_syntax::walker::Walker;

use super::classes::{self, Kind};
use super::inflect::camelize;
use super::refs::ELEMENT;
use crate::engine::PhpEngine;
use crate::frameworks::Framework;
use crate::frameworks::laravel::views::{data, passed_variables, type_of};
use crate::frameworks::php_array::string_literal;
use crate::frameworks::refs::{ReferenceKind, sites};
use crate::names::FileNames;
use crate::summary::FileSummary;
use crate::syntax::ParsedFile;
use crate::types::{ExprTypes, Type};

/// The methods whose variables every action's template has.
const EVERY_ACTION: [&str; 3] = ["beforefilter", "beforerender", "initialize"];

/// The calls setting view variables in the methods asked about.
struct Sets<'a, 'w> {
    /// The class's short name, and whether the walker is in it.
    class: &'w str,
    inside: bool,
    /// The methods wanted, lower case.
    methods: &'w [String],
    /// A template's name: a method rendering it is wanted too.
    template: &'w str,
    /// The method the walker is in, and its calls.
    current: Option<(String, Vec<&'a MethodCall<'a>>, bool)>,
    found: Vec<&'a MethodCall<'a>>,
}

struct SetWalker;

fn method_name(call: &MethodCall<'_>) -> Option<String> {
    match &call.method {
        ClassLikeMemberSelector::Identifier(name) => Some(String::from_utf8_lossy(name.value).to_ascii_lowercase()),
        _ => None,
    }
}

impl<'a> Walker<'a, 'a, Sets<'a, '_>> for SetWalker {
    fn walk_in_class(&self, class: &'a mago_syntax::cst::Class<'a>, cx: &mut Sets<'a, '_>) {
        cx.inside = class.name.value.eq_ignore_ascii_case(cx.class.as_bytes());
    }

    fn walk_out_class(&self, _: &'a mago_syntax::cst::Class<'a>, cx: &mut Sets<'a, '_>) {
        cx.inside = false;
    }

    fn walk_in_method(&self, method: &'a Method<'a>, cx: &mut Sets<'a, '_>) {
        if cx.inside {
            let name = String::from_utf8_lossy(method.name.value).to_ascii_lowercase();
            let wanted = cx.methods.contains(&name);
            cx.current = Some((name, Vec::new(), wanted));
        }
    }

    fn walk_out_method(&self, _: &'a Method<'a>, cx: &mut Sets<'a, '_>) {
        if let Some((_, calls, true)) = cx.current.take() {
            cx.found.extend(calls);
        }
    }

    fn walk_in_method_call(&self, call: &'a MethodCall<'a>, cx: &mut Sets<'a, '_>) {
        let template = cx.template;
        let Some((_, calls, wanted)) = cx.current.as_mut() else { return };
        let Some(name) = method_name(call) else { return };
        match name.as_str() {
            "set" | "setvar" | "setvars" | "setviewvars" => calls.push(call),
            // `$this->render('edit')`, `viewBuilder()->setTemplate('edit')`: this method's
            // variables are that template's.
            "render" | "settemplate" => {
                let first = call.argument_list.arguments.iter().next().and_then(|argument| match argument {
                    Argument::Positional(argument) => string_literal(argument.value),
                    Argument::Named(_) => None,
                });
                if first.is_some_and(|(named, _, _)| named.rsplit('/').next() == Some(template)) {
                    *wanted = true;
                }
            }
            _ => {}
        }
    }
}

/// Whether `call` is on `$this`, or on what a method of `$this` returns
/// (`$this->viewBuilder()->setVar()`).
fn on_this(call: &MethodCall<'_>) -> bool {
    match call.object {
        Expression::Variable(Variable::Direct(this)) => this.name == b"$this",
        Expression::Call(Call::Method(inner)) => on_this(inner),
        _ => false,
    }
}

impl PhpEngine {
    /// Whether the file at `path` is a template of a CakePHP project: a PHP
    /// file of the project below a `templates` folder.
    pub fn is_cake_template(&self, path: &Path) -> bool {
        self.frameworks().contains(Framework::CakePhp)
            && path.extension().is_some_and(|extension| extension == "php")
            && path.parent().is_some_and(|folder| folder.components().any(|part| part.as_os_str() == "templates"))
            && !self.is_library(path)
    }

    /// The class of `$this` in the template at `path`: the application's
    /// `AppView`, else CakePHP's `View`.
    pub fn cake_view_class(&self, _path: &Path) -> Option<String> {
        let project = self.cake();
        let resolver = self.resolver();
        let own = format!("{}\\View\\AppView", project.namespace);
        [own.as_str(), classes::VIEW]
            .into_iter()
            .find_map(|class| resolver.class_model(class).map(|model| model.fqn().to_owned()))
    }

    /// The classes whose methods give the template at `path` its variables,
    /// each with the methods that do (lower case): a controller and its
    /// action, a cell and its method, the mailers and their method of the
    /// template's name.
    fn cake_template_sources(&self, path: &Path) -> Vec<(String, Vec<String>)> {
        let project = self.cake();
        let Some(context) = project.template_context(path) else { return Vec::new() };
        let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else { return Vec::new() };
        let namespace = project.namespace_of(context.plugin.as_deref());
        // `view_all.php` is of `viewAll()`.
        let action = camelize(stem).to_ascii_lowercase();
        let resolver = self.resolver();
        let mut out = Vec::new();
        if let Some((prefix, name)) = context.controller() {
            let prefix = if prefix.is_empty() { String::new() } else { format!("{}\\", prefix.replace('/', "\\")) };
            let class =
                format!("{namespace}\\{}\\{prefix}{name}{}", Kind::Controller.folder(), Kind::Controller.suffix());
            let mut methods = vec![action];
            methods.extend(EVERY_ACTION.map(str::to_owned));
            out.push((class, methods));
        } else if let Some(cell) = context.folder.strip_prefix("cell/") {
            let class = format!("{namespace}\\{}\\{}Cell", Kind::Cell.folder(), cell.replace('/', "\\"));
            out.push((class, vec![action]));
        } else if context.folder.starts_with("email/") {
            for class in resolver.namespace_classes(&format!("{namespace}\\Mailer")) {
                out.push((class, vec![action.clone()]));
            }
        }
        out
    }

    /// The variables the project's code gives the template at `path`:
    /// `(name, type)`, sorted by name, the types of one name joined.
    pub fn cake_view_variables(&self, path: &Path) -> Arc<Vec<(String, Type)>> {
        if !self.is_cake_template(path) {
            return Arc::default();
        }
        let count = self.edit_count();
        if let Some((at, known)) = self.cake_views_cache().lock().get(path)
            && *at == count
        {
            return known.clone();
        }
        // While this template is worked out, a template asking again has none.
        let Some(_working) = self.view_in_progress(path) else { return Arc::default() };
        let resolver = self.resolver();
        let stem = path.file_stem().and_then(|stem| stem.to_str()).unwrap_or_default().to_owned();
        let mut passed: Vec<(String, Type)> = Vec::new();

        // What the methods of the template's class set.
        for (class, methods) in self.cake_template_sources(path) {
            let Some(model) = resolver.class_model(&class) else { continue };
            // The class and the classes of the project it extends (an `AppController`).
            let mut chain = vec![model.fqn().to_owned()];
            chain.extend(model.parents.iter().map(|parent| parent.trim_start_matches('\\').to_owned()));
            chain.retain(|class| !class.to_ascii_lowercase().starts_with("cake\\"));
            for (depth, class) in chain.iter().enumerate() {
                let Some(file) = self.class_targets(class).into_iter().next().map(|target| target.path) else {
                    continue;
                };
                let text = match self.open_document(&file) {
                    Some(document) => Some(document.text.to_string()),
                    None => std::fs::read_to_string(&file).ok(),
                };
                let Some(text) = text.filter(|text| text.contains("set")) else { continue };
                let parsed = ParsedFile::parse(&file.to_string_lossy(), text);
                let short = class.rsplit('\\').next().unwrap_or(class);
                // An ancestor's own actions aren't this controller's template's.
                let wanted: Vec<String> = if depth == 0 {
                    methods.clone()
                } else {
                    methods.iter().filter(|method| EVERY_ACTION.contains(&method.as_str())).cloned().collect()
                };
                let mut sets = Sets {
                    class: short,
                    inside: false,
                    methods: &wanted,
                    template: &stem,
                    current: None,
                    found: Vec::new(),
                };
                SetWalker.walk_program(parsed.program(), &mut sets);
                if sets.found.is_empty() {
                    continue;
                }
                let names = FileNames::resolve(&parsed);
                let summary = FileSummary::extract(&parsed);
                let types = ExprTypes::new(&parsed, &names, &summary, &resolver).at_path(Some(&file));
                for call in sets.found {
                    if !on_this(call) {
                        continue;
                    }
                    let arguments: Vec<&Expression<'_>> = call
                        .argument_list
                        .arguments
                        .iter()
                        .filter_map(|argument| match argument {
                            Argument::Positional(argument) => Some(argument.value),
                            Argument::Named(_) => None,
                        })
                        .collect();
                    match arguments.as_slice() {
                        // `set('article', $article)`.
                        [name, value, ..] if string_literal(name).is_some() => {
                            if let Some((name, _, _)) = string_literal(name) {
                                passed.push((name, type_of(&types, value)));
                            }
                        }
                        // `set(compact('article'))`, `set(['article' => $article])`.
                        [values, ..] => data(&types, values, &mut passed),
                        [] => {}
                    }
                }
            }
        }

        // An element: what `element('name', [...])` passes it.
        let root = self.root();
        let key = path.strip_prefix(&root).unwrap_or(path).to_string_lossy().replace('\\', "/");
        if key.contains("/element/") {
            let open: Vec<_> = self.open_documents().into_iter().filter(|document| !document.embedded).collect();
            let kind: &'static dyn ReferenceKind = &ELEMENT;
            let text = kind.usage_text(&key);
            for candidate in self.usage_candidates(kind, &key).unwrap_or_default() {
                let Some(file) = self.usage_file(&candidate, &open, text) else { continue };
                let starts: Vec<u32> = sites(&file.parsed, &resolver, &[kind], None)
                    .into_iter()
                    .filter(|site| {
                        kind.patterns().iter().any(|pattern| pattern.matches(site, &resolver))
                            && self
                                .site_context(&candidate, site)
                                .is_some_and(|cx| kind.canonical(&cx, &site.value) == key)
                    })
                    .map(|site| site.start)
                    .collect();
                if starts.is_empty() {
                    continue;
                }
                let names = FileNames::resolve(&file.parsed);
                let summary = FileSummary::extract(&file.parsed);
                let types = ExprTypes::new(&file.parsed, &names, &summary, &resolver).at_path(Some(&candidate));
                passed.extend(passed_variables(&types, &file.parsed, &starts));
            }
        }

        let mut names: Vec<String> = passed.iter().map(|(name, _)| name.clone()).collect();
        names.sort();
        names.dedup();
        let variables: Vec<(String, Type)> = names
            .into_iter()
            .map(|name| {
                let types: Vec<Type> =
                    passed.iter().filter(|(known, _)| *known == name).map(|(_, ty)| ty.clone()).collect();
                (name, Type::union(types))
            })
            .collect();
        let variables = Arc::new(variables);
        self.cake_views_cache().lock().insert(path.to_path_buf(), (count, variables.clone()));
        variables
    }
}
