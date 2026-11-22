//! What a Laminas template is given.
//!
//! - **MVC:** `view/application/index/index.phtml` is the template of
//!   `indexAction()` of `Application\Controller\IndexController`, or the one
//!   the action names (`setTemplate('application/index/list')`). Its
//!   variables are what the action gives its view model:
//!   `new ViewModel(['posts' => $posts])`, a returned array,
//!   `setVariable('posts', $posts)`, `setVariables([...])`. Each view model
//!   of a method is followed by the variable it is assigned to, so a child
//!   (`$sidebar = new ViewModel([...]); $sidebar->setTemplate('…');
//!   $view->addChild($sidebar, 'sidebar')`) has its own.
//! - **The layout:** what a controller gives it, in any of its methods:
//!   `$this->layout()->setVariable('title', …)`, `$this->layout()->title = …`,
//!   for the layout it names (`$this->layout('layout/admin')`), else the
//!   configuration's, else `layout/layout`.
//! - **Mezzio:** what `$this->template->render('app::home-page', [...])`
//!   passes.
//! - **Both:** what `$this->partial('name', [...])` passes a partial.
//!
//! `$this` in a template is the renderer: laminas-view 3's `Template`, else
//! `PhpRenderer`; Plates's `Template` in a Mezzio application on Plates.
//!
//! The types come to the template's own inference through
//! [`Declarations::view_variable`](crate::types::Declarations::view_variable).
//! A Twig template of a Mezzio application gets the same variables through
//! [`PhpEngine::twig_template_variables`].

use std::path::Path;
use std::sync::Arc;

use mago_span::HasSpan;
use mago_syntax::cst::{
    Access, Argument, Assignment, Call, ClassLikeMemberSelector, Expression, Instantiation, Method, MethodCall, Return,
    Variable,
};
use mago_syntax::walker::Walker;

use super::project::LaminasProject;
use super::refs::TEMPLATE;
use crate::engine::PhpEngine;
use crate::frameworks::Framework;
use crate::frameworks::laravel::views::{data, passed_variables, type_of};
use crate::frameworks::php_array::{is_array, string_literal};
use crate::frameworks::refs::{ReferenceKind, sites};
use crate::names::FileNames;
use crate::summary::FileSummary;
use crate::syntax::ParsedFile;
use crate::types::{ExprTypes, Type};

/// A view model of a method, or the layout as the method sets it up.
#[derive(Default)]
struct Model<'a> {
    /// The variable it is assigned to; `None` for the one made where it is
    /// returned (and for a returned array).
    variable: Option<Vec<u8>>,
    /// Arrays of variables: a view model's, a returned one, `setVariables()`'s.
    arrays: Vec<&'a Expression<'a>>,
    /// `setVariable('name', $value)`, `$this->layout()->name = $value`.
    pairs: Vec<(String, &'a Expression<'a>)>,
    /// `setTemplate('name')`, `$this->layout('name')`.
    template: Option<String>,
}

impl Model<'_> {
    fn is_empty(&self) -> bool {
        self.arrays.is_empty() && self.pairs.is_empty()
    }
}

/// What a method of a controller does for its views.
struct Views<'a> {
    /// The method's name.
    name: String,
    models: Vec<Model<'a>>,
    /// The variable the method returns.
    returned: Option<Vec<u8>>,
    layout: Model<'a>,
}

impl<'a> Views<'a> {
    /// The view model held by the variable `variable`; the unnamed one for
    /// `None`.
    fn model(&mut self, variable: Option<&[u8]>) -> &mut Model<'a> {
        let at = self.models.iter().position(|model| model.variable.as_deref() == variable);
        let at = at.unwrap_or_else(|| {
            self.models.push(Model { variable: variable.map(<[u8]>::to_vec), ..Model::default() });
            self.models.len() - 1
        });
        &mut self.models[at]
    }
}

struct Collector<'a, 'w> {
    /// The class's short name, and whether the walker is in it.
    class: &'w str,
    inside: bool,
    current: Option<Views<'a>>,
    found: Vec<Views<'a>>,
    /// Where the view models assigned to a variable are made.
    assigned: Vec<u32>,
}

struct ViewWalker;

fn positional<'a>(list: &'a mago_syntax::cst::ArgumentList<'a>) -> Vec<&'a Expression<'a>> {
    list.arguments
        .iter()
        .filter_map(|argument| match argument {
            Argument::Positional(argument) => Some(argument.value),
            Argument::Named(_) => None,
        })
        .collect()
}

fn direct<'a>(expr: &Expression<'a>) -> Option<&'a [u8]> {
    match expr {
        Expression::Variable(Variable::Direct(variable)) => Some(variable.name),
        _ => None,
    }
}

/// `new ViewModel(…)`: its first argument, when the class is a view model.
fn view_model<'a>(expr: &'a Expression<'a>) -> Option<Option<&'a Expression<'a>>> {
    let Expression::Instantiation(new) = expr else { return None };
    instantiated(new)
}

fn instantiated<'a>(new: &'a Instantiation<'a>) -> Option<Option<&'a Expression<'a>>> {
    let Expression::Identifier(class) = new.class else { return None };
    let name = String::from_utf8_lossy(class.value());
    let short = name.rsplit('\\').next().unwrap_or(&name);
    short.ends_with("ViewModel").then(|| new.argument_list.as_ref().and_then(|list| positional(list).first().copied()))
}

/// Whether `expr` is `$this->layout(…)`: the layout's view model.
fn is_layout(expr: &Expression<'_>) -> bool {
    let Expression::Call(Call::Method(call)) = expr else { return false };
    direct(call.object) == Some(b"$this".as_slice())
        && matches!(&call.method, ClassLikeMemberSelector::Identifier(name) if name.value.eq_ignore_ascii_case(b"layout"))
}

impl<'a> Walker<'a, 'a, Collector<'a, '_>> for ViewWalker {
    fn walk_in_class(&self, class: &'a mago_syntax::cst::Class<'a>, cx: &mut Collector<'a, '_>) {
        cx.inside = class.name.value.eq_ignore_ascii_case(cx.class.as_bytes());
    }

    fn walk_out_class(&self, _: &'a mago_syntax::cst::Class<'a>, cx: &mut Collector<'a, '_>) {
        cx.inside = false;
    }

    fn walk_in_method(&self, method: &'a Method<'a>, cx: &mut Collector<'a, '_>) {
        if cx.inside {
            let name = String::from_utf8_lossy(method.name.value).into_owned();
            cx.current = Some(Views { name, models: Vec::new(), returned: None, layout: Model::default() });
        }
    }

    fn walk_out_method(&self, _: &'a Method<'a>, cx: &mut Collector<'a, '_>) {
        cx.found.extend(cx.current.take());
    }

    fn walk_in_assignment(&self, assignment: &'a Assignment<'a>, cx: &mut Collector<'a, '_>) {
        let Some(views) = cx.current.as_mut() else { return };
        // `$view = new ViewModel([...]);`
        if let (Some(variable), Some(first)) = (direct(assignment.lhs), view_model(assignment.rhs)) {
            cx.assigned.push(assignment.rhs.span().start.offset);
            views.model(Some(variable)).arrays.extend(first);
        }
        // `$this->layout()->title = 'Books';`
        if let Expression::Access(Access::Property(property)) = assignment.lhs
            && is_layout(property.object)
            && let ClassLikeMemberSelector::Identifier(name) = &property.property
        {
            views.layout.pairs.push((String::from_utf8_lossy(name.value).into_owned(), assignment.rhs));
        }
    }

    /// `return new ViewModel(['posts' => $posts]);`: the method's own.
    fn walk_in_instantiation(&self, new: &'a Instantiation<'a>, cx: &mut Collector<'a, '_>) {
        let Some(views) = cx.current.as_mut() else { return };
        if cx.assigned.contains(&new.span().start.offset) {
            return;
        }
        if let Some(first) = instantiated(new) {
            views.model(None).arrays.extend(first);
        }
    }

    fn walk_in_method_call(&self, call: &'a MethodCall<'a>, cx: &mut Collector<'a, '_>) {
        let Some(views) = cx.current.as_mut() else { return };
        let ClassLikeMemberSelector::Identifier(method) = &call.method else { return };
        let method = String::from_utf8_lossy(method.value).to_ascii_lowercase();
        let arguments = positional(&call.argument_list);
        let named = || arguments.first().and_then(|name| string_literal(name)).map(|(name, _, _)| name);
        // `$this->layout('layout/admin')`.
        if method == "layout" && direct(call.object) == Some(b"$this".as_slice()) {
            views.layout.template = named().or(views.layout.template.take());
            return;
        }
        if !["setvariable", "setvariables", "settemplate"].contains(&method.as_str()) {
            return;
        }
        // On the layout, on the view model a variable holds, else on the method's own.
        let model = if is_layout(call.object) {
            &mut views.layout
        } else {
            let variable = direct(call.object).filter(|name| *name != b"$this");
            let known =
                variable.filter(|name| views.models.iter().any(|model| model.variable.as_deref() == Some(*name)));
            views.model(known)
        };
        match method.as_str() {
            "setvariable" => {
                if let [name, value, ..] = arguments.as_slice()
                    && let Some((name, _, _)) = string_literal(name)
                {
                    model.pairs.push((name, value));
                }
            }
            "setvariables" => model.arrays.extend(arguments.first().copied()),
            _ => model.template = named().or(model.template.take()),
        }
    }

    /// `return ['posts' => $posts];`, `return $view;`.
    fn walk_in_return(&self, r#return: &'a Return<'a>, cx: &mut Collector<'a, '_>) {
        let Some(views) = cx.current.as_mut() else { return };
        let Some(value) = r#return.value else { return };
        if is_array(value) {
            views.model(None).arrays.push(value);
        } else if let Some(variable) = direct(value) {
            views.returned = Some(variable.to_vec());
        }
    }
}

impl PhpEngine {
    /// Whether the file at `path` is a template of a Laminas project: one a
    /// `template_map` names, or a file of a folder of templates.
    pub fn is_laminas_template(&self, path: &Path) -> bool {
        self.frameworks().contains(Framework::Laminas) && self.laminas().is_template(path) && !self.is_library(path)
    }

    /// The class of `$this` in a template.
    pub fn laminas_view_class(&self) -> Option<String> {
        let resolver = self.resolver();
        [
            "Laminas\\View\\Renderer\\Template",
            "Laminas\\View\\Renderer\\PhpRenderer",
            "Zend\\View\\Renderer\\PhpRenderer",
            "League\\Plates\\Template\\Template",
        ]
        .into_iter()
        .find_map(|class| resolver.class_model(class).map(|model| model.fqn().to_owned()))
    }

    /// The controllers of the project: the classes its controller manager
    /// makes and its routes lead to.
    fn laminas_controllers(&self, project: &LaminasProject) -> Vec<String> {
        let resolver = self.resolver();
        let mut out: Vec<String> = Vec::new();
        let named = project.controllers.iter().flat_map(|service| [Some(service.name.clone()), service.target.clone()]);
        let routed = project.routes.iter().map(|route| route.controller.clone());
        for class in named.chain(routed).flatten() {
            let Some(model) = resolver.class_model(&class) else { continue };
            let controller = model.methods.iter().any(|method| method.name.ends_with("Action"));
            if controller && !out.iter().any(|known| known == model.fqn()) {
                out.push(model.fqn().to_owned());
            }
        }
        out
    }

    /// The variables the project's code gives the template at `path`:
    /// `(name, type)`, sorted by name, the types of one name joined.
    pub fn laminas_view_variables(&self, path: &Path) -> Arc<Vec<(String, Type)>> {
        if !self.frameworks().contains(Framework::Laminas) || self.is_library(path) || !self.laminas().is_template(path)
        {
            return Arc::default();
        }
        let count = self.edit_count();
        if let Some((at, known)) = self.laminas_views_cache().lock().get(path)
            && *at == count
        {
            return known.clone();
        }
        // While this template is worked out, a template asking again has none.
        let Some(_working) = self.view_in_progress(path) else { return Arc::default() };
        let project = self.laminas();
        let resolver = self.resolver();
        let names = project.template_names(path);
        let layout = project.layout.clone().unwrap_or_else(|| "layout/layout".to_owned());
        let mut passed: Vec<(String, Type)> = Vec::new();

        // MVC: what a controller's methods give their view models and the layout.
        for class in self.laminas_controllers(&project) {
            let Some(file) = self.class_targets(&class).into_iter().next().map(|target| target.path) else { continue };
            if self.is_library(&file) {
                continue;
            }
            let text = match self.open_document(&file) {
                Some(document) => Some(document.text.to_string()),
                None => std::fs::read_to_string(&file).ok(),
            };
            let Some(text) = text else { continue };
            let parsed = ParsedFile::parse(&file.to_string_lossy(), text);
            let short = class.rsplit('\\').next().unwrap_or(&class);
            let mut collector =
                Collector { class: short, inside: false, current: None, found: Vec::new(), assigned: Vec::new() };
            ViewWalker.walk_program(parsed.program(), &mut collector);
            // The view models rendered with this template.
            let mut rendering: Vec<Model<'_>> = Vec::new();
            for views in collector.found {
                let own = views
                    .name
                    .ends_with("Action")
                    .then(|| LaminasProject::action_template(&class, &views.name))
                    .flatten();
                let returned = views.returned;
                for model in views.models {
                    // The one the action returns has the action's template, unless it names one.
                    let is_own = model.variable.is_none() || model.variable == returned;
                    let template = model.template.clone().or_else(|| own.clone().filter(|_| is_own));
                    if template.is_some_and(|template| names.contains(&template)) {
                        rendering.push(model);
                    }
                }
                let template = views.layout.template.clone().unwrap_or_else(|| layout.clone());
                if !views.layout.is_empty() && names.contains(&template) {
                    rendering.push(views.layout);
                }
            }
            if rendering.is_empty() {
                continue;
            }
            let file_names = FileNames::resolve(&parsed);
            let summary = FileSummary::extract(&parsed);
            let types = ExprTypes::new(&parsed, &file_names, &summary, &resolver).at_path(Some(&file));
            for model in rendering {
                for array in model.arrays {
                    data(&types, array, &mut passed);
                }
                for (name, value) in model.pairs {
                    passed.push((name, type_of(&types, value)));
                }
            }
        }

        // What a call naming the template passes: Mezzio's `render()`, a partial.
        let root = self.root();
        let key = path.strip_prefix(&root).unwrap_or(path).to_string_lossy().replace('\\', "/");
        let open: Vec<_> = self.open_documents().into_iter().filter(|document| !document.embedded).collect();
        let kind: &'static dyn ReferenceKind = &TEMPLATE;
        let text = kind.usage_text(&key);
        for candidate in self.usage_candidates(kind, &key).unwrap_or_default() {
            let Some(file) = self.usage_file(&candidate, &open, text) else { continue };
            let starts: Vec<u32> = sites(&file.parsed, &resolver, &[kind], None)
                .into_iter()
                .filter(|site| {
                    kind.patterns().iter().any(|pattern| pattern.matches(site, &resolver))
                        && self.site_context(&candidate, site).is_some_and(|cx| kind.canonical(&cx, &site.value) == key)
                })
                .map(|site| site.start)
                .collect();
            if starts.is_empty() {
                continue;
            }
            let file_names = FileNames::resolve(&file.parsed);
            let summary = FileSummary::extract(&file.parsed);
            let types = ExprTypes::new(&file.parsed, &file_names, &summary, &resolver).at_path(Some(&candidate));
            passed.extend(passed_variables(&types, &file.parsed, &starts));
        }

        let mut found: Vec<String> = passed.iter().map(|(name, _)| name.clone()).collect();
        found.sort();
        found.dedup();
        let variables: Vec<(String, Type)> = found
            .into_iter()
            .map(|name| {
                let types: Vec<Type> =
                    passed.iter().filter(|(known, _)| *known == name).map(|(_, ty)| ty.clone()).collect();
                (name, Type::union(types))
            })
            .collect();
        let variables = Arc::new(variables);
        self.laminas_views_cache().lock().insert(path.to_path_buf(), (count, variables.clone()));
        variables
    }
}
