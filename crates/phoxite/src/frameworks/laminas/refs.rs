//! Laminas's and Mezzio's reference kinds ([`kinds`]), on the machinery
//! every framework shares ([`crate::frameworks::refs`]). All are one type,
//! [`LaminasName`].
//!
//! | kind | names | where |
//! | --- | --- | --- |
//! | `laminasRoute` | a route, where the configuration (or Mezzio's `config/routes.php`) names it; `blog/post` is the child `post` of `blog` | `url('name')` in a template, a controller's `url()->fromRoute()` and `redirect()->toRoute()`, Mezzio's `UrlHelper::generate()` and `RouterInterface::generateUri()`, the name `$app->get()` gives, `'route' => 'name'` |
//! | `laminasTemplate` | a template's file, by a `template_map`, a template path or a Mezzio namespace (`app::home-page`) | `setTemplate()`, `partial()`, `partialLoop()`, `layout()`, a renderer's `render()`, the keys of `template_map`, `'layout' =>`, `'not_found_template' =>`, `'exception_template' =>` |
//! | `laminasService` | a service, where a configuration declares it; a class's name is the class | `get()`, `has()` and `build()` of a container |
//! | `laminasPlugin` | a controller plugin's class | `plugin('name')` in a controller |
//! | `laminasHelper` | a view helper's class | `plugin('name')` in a view |
//! | `laminasAction` | a controller's action: `'index'` is `indexAction()` of the controller beside it, of the route the URL is of, else of the controller asking | `'action' => 'index'` |
//! | `laminasValidator`, `laminasFilter` | a validator's and a filter's class, by the name their plugin manager has it under | `'validators' => [['name' => 'StringLength']]`, `'filters' => [['name' => 'StringTrim']]`, `attachByName()` |
//! | `laminasFormElement` | a form element's class | `'type' => 'text'` of what a form's `add()` is given, and of a `'spec'` |
//! | `laminasNavigation` | a navigation container of the configuration | `$this->navigation('navigation')` |
//! | `laminasRouteParam` | a parameter a route's path has (`:id`, `{id}`), its parents' too | the keys of the parameters of `url('name', ['id' => 1])`, `fromRoute()`, `toRoute()`, `generate()` |
//! | `laminasConfig` | a key of the configuration, below the keys read before it | `$container->get('config')['db']['host']` |
//!
//! Each has completion, goto, hover and Find References. Nothing is renamed.
//!
//! A route, a template and a service no configuration has, and an action its
//! controller doesn't have, are reported ([`ReferenceKind::undefined`]).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use mago_span::HasSpan;
use mago_syntax::cst::{Access, ArrayElement, ClassLikeConstantSelector, Expression};
use mago_syntax::walker::Walker;

use super::project::{LaminasProject, Manager, camel, dashed};
use crate::base::Range;
use crate::frameworks::laravel::refs::controller_action::method_target;
use crate::frameworks::php_array::{elements, string_literal};
use crate::frameworks::refs::{
    CallTarget, CalleePattern, PlacePattern, Position, RefContext, RefItem, ReferenceKind, TEMPLATE_THIS, Undefined,
};
use crate::index::path_key;
use crate::names::{FileNames, NameKind, Resolution};
use crate::navigate::Target;
use crate::summary::Visibility;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Source {
    Route,
    Template,
    Service,
    Plugin,
    Helper,
    Action,
    Config,
    Validator,
    Filter,
    FormElement,
    Navigation,
    RouteParam,
}

/// One of the kinds of the module documentation.
pub struct LaminasName {
    id: &'static str,
    label: &'static str,
    source: Source,
    patterns: &'static [PlacePattern],
}

const fn method(
    classes: &'static [&'static str],
    names: &'static [&'static str],
    index: usize,
    name: &'static str,
) -> PlacePattern {
    PlacePattern::argument(CalleePattern::Method { classes, names }, index, name)
}

const fn entry(key: &'static str) -> PlacePattern {
    PlacePattern::argument(CalleePattern::Entry(key), 0, "value")
}

pub const CONTAINERS: &[&str] = &[
    "Psr\\Container\\ContainerInterface",
    "Laminas\\ServiceManager\\ServiceLocatorInterface",
    "Zend\\ServiceManager\\ServiceLocatorInterface",
    "Interop\\Container\\ContainerInterface",
];
pub const CONTROLLERS: &[&str] =
    &["Laminas\\Mvc\\Controller\\AbstractController", "Zend\\Mvc\\Controller\\AbstractController"];
/// What `$this` is in a template: the renderer (laminas-view 2), the
/// template (laminas-view 3), or unknown outside any class.
pub const RENDERERS: &[&str] = &[
    "Laminas\\View\\Renderer\\PhpRenderer",
    "Zend\\View\\Renderer\\PhpRenderer",
    "Laminas\\View\\Renderer\\Template",
    "Zend\\View\\Renderer\\Template",
    "League\\Plates\\Template\\Template",
    TEMPLATE_THIS,
];
const URL_PLUGIN: &[&str] = &["Laminas\\Mvc\\Controller\\Plugin\\Url", "Zend\\Mvc\\Controller\\Plugin\\Url"];
const REDIRECT_PLUGIN: &[&str] =
    &["Laminas\\Mvc\\Controller\\Plugin\\Redirect", "Zend\\Mvc\\Controller\\Plugin\\Redirect"];
const LAYOUT_PLUGIN: &[&str] = &["Laminas\\Mvc\\Controller\\Plugin\\Layout", "Zend\\Mvc\\Controller\\Plugin\\Layout"];
const VIEW_MODELS: &[&str] = &[
    "Laminas\\View\\Model\\ModelInterface",
    "Zend\\View\\Model\\ModelInterface",
    "Laminas\\View\\Model\\ViewModel",
    "Zend\\View\\Model\\ViewModel",
];
const URL_HELPER: &[&str] = &[
    "Mezzio\\Helper\\UrlHelper",
    "Zend\\Expressive\\Helper\\UrlHelper",
    "Mezzio\\LaminasView\\UrlHelper",
    "Zend\\Expressive\\LaminasView\\UrlHelper",
    "Laminas\\View\\Helper\\Url",
    "Zend\\View\\Helper\\Url",
    "Zend\\Expressive\\ZendView\\UrlHelper",
];
const ROUTERS: &[&str] = &["Mezzio\\Router\\RouterInterface", "Zend\\Expressive\\Router\\RouterInterface"];
const APPLICATIONS: &[&str] = &[
    "Mezzio\\Application",
    "Zend\\Expressive\\Application",
    "Mezzio\\Router\\RouteCollectorInterface",
    "Zend\\Expressive\\Router\\RouteCollectorInterface",
    "Mezzio\\Router\\RouteCollector",
    "Zend\\Expressive\\Router\\RouteCollector",
];
const TEMPLATE_RENDERERS: &[&str] =
    &["Mezzio\\Template\\TemplateRendererInterface", "Zend\\Expressive\\Template\\TemplateRendererInterface"];
const VALIDATOR_CHAINS: &[&str] = &["Laminas\\Validator\\ValidatorChain", "Zend\\Validator\\ValidatorChain"];
const FILTER_CHAINS: &[&str] = &["Laminas\\Filter\\FilterChain", "Zend\\Filter\\FilterChain"];
const FORMS: &[&str] = &[
    "Laminas\\Form\\FieldsetInterface",
    "Laminas\\Form\\Fieldset",
    "Laminas\\Form\\Factory",
    "Zend\\Form\\FieldsetInterface",
    "Zend\\Form\\Fieldset",
    "Zend\\Form\\Factory",
];
const VERBS: &[&str] = &["get", "post", "put", "patch", "delete", "any"];

pub static ROUTE: LaminasName = LaminasName {
    id: "laminasRoute",
    label: "route",
    source: Source::Route,
    patterns: &[
        method(RENDERERS, &["url"], 0, "name"),
        method(URL_PLUGIN, &["fromRoute"], 0, "route"),
        method(REDIRECT_PLUGIN, &["toRoute"], 0, "route"),
        method(URL_HELPER, &["generate", "__invoke"], 0, "routeName"),
        method(ROUTERS, &["generateUri"], 0, "name"),
        // Where Mezzio names a route.
        method(APPLICATIONS, VERBS, 2, "name"),
        method(APPLICATIONS, &["route"], 3, "name"),
        entry("route"),
    ],
};

pub static TEMPLATE: LaminasName = LaminasName {
    id: "laminasTemplate",
    label: "template",
    source: Source::Template,
    patterns: &[
        method(VIEW_MODELS, &["setTemplate"], 0, "template"),
        method(LAYOUT_PLUGIN, &["setTemplate"], 0, "template"),
        method(RENDERERS, &["partial", "partialLoop", "render", "layout"], 0, "name"),
        method(CONTROLLERS, &["layout"], 0, "template"),
        method(TEMPLATE_RENDERERS, &["render"], 0, "name"),
        entry("template_map").at(Position::ArrayKey),
        entry("layout"),
        entry("not_found_template"),
        entry("exception_template"),
    ],
};

pub static SERVICE: LaminasName = LaminasName {
    id: "laminasService",
    label: "service",
    source: Source::Service,
    patterns: &[method(CONTAINERS, &["get", "has", "build"], 0, "id")],
};

pub static PLUGIN: LaminasName = LaminasName {
    id: "laminasPlugin",
    label: "controller plugin",
    source: Source::Plugin,
    patterns: &[method(CONTROLLERS, &["plugin"], 0, "name")],
};

pub static HELPER: LaminasName = LaminasName {
    id: "laminasHelper",
    label: "view helper",
    source: Source::Helper,
    patterns: &[method(RENDERERS, &["plugin"], 0, "name")],
};

pub static ACTION: LaminasName = LaminasName {
    id: "laminasAction",
    label: "action",
    source: Source::Action,
    patterns: &[
        // The parameters of a URL: an action its route's controller doesn't have is reported.
        method(RENDERERS, &["url"], 1, "params").at(Position::ValueOfKey("action")),
        method(URL_PLUGIN, &["fromRoute"], 1, "params").at(Position::ValueOfKey("action")),
        method(REDIRECT_PLUGIN, &["toRoute"], 1, "params").at(Position::ValueOfKey("action")),
        // A route's defaults, and any other array.
        entry("action"),
    ],
};

pub static CONFIG: LaminasName = LaminasName {
    id: "laminasConfig",
    label: "configuration",
    source: Source::Config,
    patterns: &[PlacePattern::argument(CalleePattern::ResultKey { classes: CONTAINERS, names: &["get"] }, 0, "key")],
};

pub static VALIDATOR: LaminasName = LaminasName {
    id: "laminasValidator",
    label: "validator",
    source: Source::Validator,
    patterns: &[
        // An input's specification: `'validators' => [['name' => 'StringLength']]`.
        entry("validators").at(Position::ValueOfKey("name")).nested(),
        method(VALIDATOR_CHAINS, &["attachByName", "prependByName"], 0, "name"),
    ],
};

pub static FILTER: LaminasName = LaminasName {
    id: "laminasFilter",
    label: "filter",
    source: Source::Filter,
    patterns: &[
        entry("filters").at(Position::ValueOfKey("name")).nested(),
        method(FILTER_CHAINS, &["attachByName", "prependByName"], 0, "name"),
    ],
};

pub static FORM_ELEMENT: LaminasName = LaminasName {
    id: "laminasFormElement",
    label: "form element",
    source: Source::FormElement,
    patterns: &[
        // `$this->add(['type' => 'text', 'name' => 'title'])`, and a specification.
        method(FORMS, &["add", "create", "createElement", "createFieldset", "createForm"], 0, "spec")
            .at(Position::ValueOfKey("type")),
        entry("spec").at(Position::ValueOfKey("type")),
    ],
};

pub static NAVIGATION: LaminasName = LaminasName {
    id: "laminasNavigation",
    label: "navigation",
    source: Source::Navigation,
    patterns: &[method(RENDERERS, &["navigation"], 0, "container")],
};

pub static ROUTE_PARAM: LaminasName = LaminasName {
    id: "laminasRouteParam",
    label: "route parameter",
    source: Source::RouteParam,
    patterns: &[
        method(RENDERERS, &["url"], 1, "params").at(Position::ArrayKey),
        method(URL_PLUGIN, &["fromRoute"], 1, "params").at(Position::ArrayKey),
        method(REDIRECT_PLUGIN, &["toRoute"], 1, "params").at(Position::ArrayKey),
        method(URL_HELPER, &["generate", "__invoke"], 1, "routeParams").at(Position::ArrayKey),
        method(ROUTERS, &["generateUri"], 1, "substitutions").at(Position::ArrayKey),
    ],
};

/// Laminas's kinds.
pub fn kinds() -> &'static [&'static dyn ReferenceKind] {
    static KINDS: [&dyn ReferenceKind; 12] = [
        &ROUTE,
        &TEMPLATE,
        &SERVICE,
        &PLUGIN,
        &HELPER,
        &ACTION,
        &CONFIG,
        &VALIDATOR,
        &FILTER,
        &FORM_ELEMENT,
        &NAVIGATION,
        &ROUTE_PARAM,
    ];
    &KINDS
}

/// The innermost array around an offset.
struct Around<'a> {
    offset: u32,
    found: Option<&'a Expression<'a>>,
}

struct AroundWalker;

impl<'a> Walker<'a, 'a, Around<'a>> for AroundWalker {
    fn walk_in_expression(&self, expr: &'a Expression<'a>, cx: &mut Around<'a>) {
        if !matches!(expr, Expression::Array(_) | Expression::LegacyArray(_)) {
            return;
        }
        let span = expr.span();
        if span.start.offset <= cx.offset && cx.offset <= span.end.offset {
            cx.found = Some(expr);
        }
    }
}

impl LaminasName {
    fn project(cx: &RefContext<'_>) -> Arc<LaminasProject> {
        cx.engine.laminas()
    }

    fn site_path(cx: &RefContext<'_>) -> Option<PathBuf> {
        cx.site.as_ref().map(|site| site.path.clone())
    }

    fn place<'c>(cx: &'c RefContext<'_>) -> Option<&'c CallTarget> {
        cx.site.as_ref()?.call.as_ref().map(|(target, _)| target)
    }

    fn site_text(cx: &RefContext<'_>) -> Option<String> {
        let site = cx.site.as_ref()?;
        match cx.engine.open_document(&site.path) {
            Some(document) => Some(document.text.to_string()),
            None => std::fs::read_to_string(&site.path).ok(),
        }
    }

    /// The other values of the array the string is a value of, by their
    /// keys: strings, and the classes of `Foo::class`.
    fn siblings(cx: &RefContext<'_>) -> HashMap<String, String> {
        let mut out = HashMap::new();
        let (Some(site), Some(text)) = (cx.site.as_ref(), Self::site_text(cx)) else { return out };
        let parsed = crate::syntax::ParsedFile::parse(&site.path.to_string_lossy(), text);
        let mut around = Around { offset: site.start, found: None };
        AroundWalker.walk_program(parsed.program(), &mut around);
        let Some(array) = around.found else { return out };
        let names = FileNames::resolve(&parsed);
        let class = |expr: &Expression<'_>| {
            let Expression::Access(Access::ClassConstant(access)) = expr else { return None };
            let ClassLikeConstantSelector::Identifier(constant) = &access.constant else { return None };
            let Expression::Identifier(identifier) = access.class else { return None };
            if !constant.value.eq_ignore_ascii_case(b"class") {
                return None;
            }
            let name = String::from_utf8_lossy(identifier.value()).into_owned();
            match names.scope_at(identifier.span().start.offset).resolve(NameKind::ClassLike, &name).resolution {
                Resolution::Fqn(fqn) | Resolution::Fallback { namespaced: fqn, .. } => {
                    Some(fqn.trim_start_matches('\\').to_owned())
                }
                Resolution::Relative(_) => None,
            }
        };
        for element in elements(array) {
            let ArrayElement::KeyValue(element) = element else { continue };
            let Some((key, _, _)) = string_literal(element.key) else { continue };
            let value = string_literal(element.value).map(|(value, _, _)| value).or_else(|| class(element.value));
            out.extend(value.map(|value| (key, value)));
        }
        out
    }

    fn manager(&self, cx: &RefContext<'_>) -> Option<Manager> {
        match self.source {
            Source::Service => {
                // A plugin manager's own: `$formElementManager->get('name')`.
                let resolver = cx.engine.resolver();
                let on = |bases: &[&str]| match Self::place(cx) {
                    Some(CallTarget::Method { classes, .. }) => classes.iter().any(|class| {
                        bases.iter().any(|base| class.eq_ignore_ascii_case(base))
                            || resolver
                                .class_model(class)
                                .is_some_and(|model| bases.iter().any(|base| model.is_subclass_of(base)))
                    }),
                    _ => false,
                };
                let plugins =
                    Manager::ALL.iter().find(|(manager, _, _, classes)| *manager != Manager::Services && on(classes));
                Some(plugins.map_or(Manager::Services, |(manager, _, _, _)| *manager))
            }
            Source::Plugin => Some(Manager::Plugins),
            Source::Helper => Some(Manager::Helpers),
            Source::Validator => Some(Manager::Validators),
            Source::Filter => Some(Manager::Filters),
            Source::FormElement => Some(Manager::Forms),
            _ => None,
        }
    }

    /// The kinds whose names are a plugin manager's services.
    fn of_manager(&self) -> bool {
        matches!(
            self.source,
            Source::Service
                | Source::Plugin
                | Source::Helper
                | Source::Validator
                | Source::Filter
                | Source::FormElement
        )
    }

    /// The route the parameters at the site are of: the first argument's.
    fn params_route<'p>(cx: &RefContext<'_>, project: &'p LaminasProject) -> Option<&'p super::project::Route> {
        let name = cx.site.as_ref()?.first_argument.clone()?;
        project.route(&name).into_iter().next()
    }

    /// The navigation containers `value` names: `navigation` is the default
    /// one, a service's name its last part (`Laminas\Navigation\Special`).
    fn containers<'p>(project: &'p LaminasProject, value: &str) -> Vec<&'p super::project::Service> {
        let last = value.rsplit(BACKSLASH).next().unwrap_or(value).to_ascii_lowercase();
        let wanted = if last == "navigation" { "default".to_owned() } else { last };
        project
            .navigation
            .iter()
            .filter(|found| found.name.eq_ignore_ascii_case(&wanted) || found.name == value)
            .collect()
    }

    /// The fully qualified name of the class `short` the file `file`
    /// declares.
    fn class_in(cx: &RefContext<'_>, file: &Path, short: &str) -> Option<String> {
        let wanted = path_key(file).to_ascii_lowercase();
        let found = cx.engine.index()?.by_short_name(short).ok()?;
        found.into_iter().find(|decl| decl.file.to_ascii_lowercase() == wanted).map(|decl| decl.fqn)
    }

    /// The class a controller's name stands for: itself, or the class the
    /// controller manager makes under that name.
    fn controller_class(cx: &RefContext<'_>, project: &LaminasProject, name: &str) -> Option<String> {
        let resolver = cx.engine.resolver();
        let known = |class: &str| resolver.class_model(class).map(|model| model.fqn().to_owned());
        known(name).or_else(|| {
            let service = project.resolved(Manager::Controllers, name)?;
            known(&service.name).or_else(|| service.target.as_deref().and_then(known))
        })
    }

    /// The controller and the method the action `value` is of: of the
    /// controller beside it, of the route the URL is of, else of the
    /// controller asking.
    fn action(cx: &RefContext<'_>, value: &str) -> Option<(String, String)> {
        let plain = !value.is_empty() && value.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-'));
        if !plain {
            return None;
        }
        let project = Self::project(cx);
        let beside = Self::siblings(cx);
        let of_route = || {
            // `url('blog', ['action' => 'show'])`: the route's controller, or its parent's.
            let name = cx.site.as_ref()?.first_argument.clone()?;
            let mut name = name.as_str();
            loop {
                if let Some(controller) = project.route(name).iter().find_map(|route| route.controller.clone()) {
                    return Some(controller);
                }
                name = name.rsplit_once('/')?.0;
            }
        };
        let own = || {
            let site = Self::site_path(cx)?;
            let stem = site.file_stem()?.to_str()?;
            stem.ends_with("Controller").then(|| Self::class_in(cx, &site, stem))?
        };
        // A child route's defaults name no controller: the route the string is written in
        // (the last one named before it in its file), then its parents.
        let of_enclosing = || {
            let site = cx.site.as_ref()?;
            let text = Self::site_text(cx)?;
            let line = text.get(..site.start as usize)?.matches('\n').count() as u32;
            let written =
                project.routes.iter().filter(|route| route.file == site.path && route.range.start.line <= line);
            let mut name = written.max_by_key(|route| route.range.start.line)?.name.as_str();
            loop {
                if let Some(controller) = project.route(name).iter().find_map(|route| route.controller.clone()) {
                    return Some(controller);
                }
                name = name.rsplit_once('/')?.0;
            }
        };
        let named = match Self::place(cx) {
            Some(CallTarget::Method { .. }) => of_route(),
            _ => beside.get("controller").cloned().or_else(of_enclosing),
        };
        let class = named.and_then(|name| Self::controller_class(cx, &project, &name)).or_else(own)?;
        Some((class, format!("{}Action", camel(value))))
    }

    /// The keys read before the string, for a key of the configuration.
    fn config_keys(cx: &RefContext<'_>) -> Option<Vec<String>> {
        let Some(CallTarget::Result { keys, .. }) = Self::place(cx) else { return None };
        // Only what `get('config')` returns is the configuration.
        let argument = cx.site.as_ref()?.first_argument.as_deref()?;
        argument.eq_ignore_ascii_case("config").then(|| keys.clone())
    }

    fn targets(&self, cx: &RefContext<'_>, value: &str) -> Vec<Target> {
        let project = Self::project(cx);
        let whole = |path: PathBuf| Target { path, range: Range::default() };
        match self.source {
            Source::Route => project
                .route(value)
                .into_iter()
                .map(|route| Target { path: route.file.clone(), range: route.range })
                .collect(),
            Source::Template => {
                let found = project.template_files(value).into_iter().find(|file| file.is_file());
                found.into_iter().map(whole).collect()
            }
            Source::Navigation => {
                let found = Self::containers(&project, value);
                found.into_iter().map(|found| Target { path: found.file.clone(), range: found.range }).collect()
            }
            Source::RouteParam => {
                let route =
                    Self::params_route(cx, &project).filter(|route| route.params.iter().any(|param| param == value));
                route.into_iter().map(|route| Target { path: route.file.clone(), range: route.range }).collect()
            }
            Source::Service
            | Source::Plugin
            | Source::Helper
            | Source::Validator
            | Source::Filter
            | Source::FormElement => {
                let Some(manager) = self.manager(cx) else { return Vec::new() };
                // A plugin and a helper are their class; a service is where it is declared.
                let service = project.resolved(manager, value);
                let class = service.and_then(|service| match manager {
                    Manager::Services => None,
                    _ => service.target.clone(),
                });
                if let Some(class) = class {
                    let found = cx.engine.class_targets(&class);
                    if !found.is_empty() {
                        return found;
                    }
                }
                match project.service(manager, value) {
                    Some(service) => vec![Target { path: service.file.clone(), range: service.range }],
                    None => cx.engine.class_targets(value.trim_start_matches('\\')),
                }
            }
            Source::Action => {
                let Some((class, method)) = Self::action(cx, value) else { return Vec::new() };
                method_target(cx, &class, &method).into_iter().collect()
            }
            Source::Config => {
                let Some(above) = Self::config_keys(cx) else { return Vec::new() };
                let found = project.keys_below(&above).into_iter().filter(|key| key.name == value);
                found.map(|key| Target { path: key.file.clone(), range: key.range }).collect()
            }
        }
    }

    /// The names of this kind, each with a detail.
    fn names(&self, cx: &RefContext<'_>) -> Vec<(String, Option<String>)> {
        let project = Self::project(cx);
        let mut out: Vec<(String, Option<String>)> = match self.source {
            Source::Route => {
                project.routes.iter().map(|route| (route.name.clone(), route.controller.clone())).collect()
            }
            Source::Template => project.all_templates().into_iter().map(|name| (name, None)).collect(),
            Source::Navigation => {
                let mut found: Vec<(String, Option<String>)> = vec![("navigation".to_owned(), None)];
                found.extend(project.navigation.iter().map(|found| (found.name.clone(), None)));
                found
            }
            Source::RouteParam => {
                let route = Self::params_route(cx, &project);
                route.map_or(Vec::new(), |route| {
                    route.params.iter().map(|param| (param.clone(), route.path.clone())).collect()
                })
            }
            Source::Service
            | Source::Plugin
            | Source::Helper
            | Source::Validator
            | Source::Filter
            | Source::FormElement => {
                let Some(manager) = self.manager(cx) else { return Vec::new() };
                // A plugin's name is offered once: as a method is written for a plugin and a
                // helper, as a class is for a validator, a filter and a form element.
                let listed: Vec<&super::project::Service> = match manager {
                    Manager::Services => project.list(manager).iter().rev().collect(),
                    Manager::Plugins | Manager::Helpers => project.callable_names(manager),
                    _ => project.spellings(manager, true),
                };
                listed.into_iter().map(|service| (service.name.clone(), service.target.clone())).collect()
            }
            Source::Action => {
                let class = Self::action(cx, "index").map(|(class, _)| class);
                let resolver = cx.engine.resolver();
                let Some(model) = class.and_then(|class| resolver.class_model(&class)) else { return Vec::new() };
                let mut found = Vec::new();
                for method in model.methods.iter() {
                    let own =
                        !method.declaring.fqn.trim_start_matches('\\').to_ascii_lowercase().starts_with("laminas\\");
                    let public = method.declared().is_some_and(|declared| declared.visibility == Visibility::Public);
                    if let Some(action) =
                        method.name.strip_suffix("Action").filter(|action| own && public && !action.is_empty())
                    {
                        found.push((dashed(action), None));
                    }
                }
                found.sort();
                found
            }
            Source::Config => {
                let Some(above) = Self::config_keys(cx) else { return Vec::new() };
                project.keys_below(&above).into_iter().map(|key| (key.name.clone(), key.detail.clone())).collect()
            }
        };
        let mut seen = std::collections::HashSet::new();
        out.retain(|(name, _)| seen.insert(name.clone()));
        out
    }
}

impl ReferenceKind for LaminasName {
    fn id(&self) -> &'static str {
        self.id
    }

    fn patterns(&self) -> &'static [PlacePattern] {
        self.patterns
    }

    /// What every usage of one thing is: a template's file from the
    /// project's root, an action as `<controller>::<method>`, a key of the
    /// configuration with the keys above it.
    fn canonical(&self, cx: &RefContext<'_>, value: &str) -> String {
        let root = cx.engine.root();
        match self.source {
            Source::Template => {
                let files = Self::project(cx).template_files(value);
                let file = files.iter().find(|file| file.is_file()).or(files.first());
                file.map_or_else(
                    || value.to_owned(),
                    |file| file.strip_prefix(&root).unwrap_or(file).to_string_lossy().replace('\\', "/"),
                )
            }
            Source::Action => match Self::action(cx, value) {
                Some((class, method)) => format!("{class}::{method}"),
                None => value.to_owned(),
            },
            Source::RouteParam => match Self::params_route(cx, &Self::project(cx)) {
                Some(route) => format!("{}{SEPARATOR}{value}", route.name),
                None => value.to_owned(),
            },
            Source::Config => match Self::config_keys(cx) {
                Some(above) if !above.is_empty() => format!("{}{SEPARATOR}{value}", above.join(&SEPARATOR.to_string())),
                _ => value.to_owned(),
            },
            _ => value.to_owned(),
        }
    }

    fn usage_text<'v>(&self, value: &'v str) -> &'v str {
        match self.source {
            Source::Template => {
                let name = value.rsplit('/').next().unwrap_or(value);
                // (Without its extension, of however many parts: `home-page.html.twig`.)
                name.split('.').next().unwrap_or(name)
            }
            Source::Action => {
                let method = value.rsplit("::").next().unwrap_or(value);
                method.strip_suffix("Action").unwrap_or(method)
            }
            Source::Config | Source::RouteParam => value.rsplit(SEPARATOR).next().unwrap_or(value),
            _ => value,
        }
    }

    fn usage_words(&self, value: &str) -> Vec<String> {
        crate::index::words(self.usage_text(value))
    }

    fn complete(&self, cx: &RefContext<'_>, value: &str, _at: usize) -> Vec<RefItem> {
        self.names(cx)
            .into_iter()
            .map(|(name, detail)| RefItem {
                label: name.clone(),
                insert: name,
                detail: detail.or_else(|| Some(self.label.to_owned())),
                from: 0,
                to: value.len(),
            })
            .collect()
    }

    fn resolve(&self, cx: &RefContext<'_>, value: &str, _at: usize) -> Vec<Target> {
        let mut out: Vec<Target> = Vec::new();
        for target in self.targets(cx, value) {
            if !out.contains(&target) {
                out.push(target);
            }
        }
        out
    }

    fn is_known(&self, cx: &RefContext<'_>, value: &str) -> bool {
        self.undefined(cx, value).is_none()
    }

    fn undefined(&self, cx: &RefContext<'_>, value: &str) -> Option<Undefined> {
        let project = Self::project(cx);
        let site = Self::site_path(cx)?;
        if value.is_empty() || cx.engine.is_library(&site) {
            return None;
        }
        // An array's entry may be anything's: only what a call takes is checked.
        let Some(CallTarget::Method { classes, name, .. }) = Self::place(cx) else { return None };
        match self.source {
            Source::Route => {
                // Where a route is named, it names itself.
                let declares =
                    VERBS.contains(&name.to_ascii_lowercase().as_str()) || name.eq_ignore_ascii_case("route");
                let plain =
                    value.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '/' | ':'));
                (!declares && plain && !project.routes.is_empty() && project.route(value).is_empty())
                    .then(|| Undefined::warning(format!("Unknown route '{value}'")))
            }
            Source::Template => {
                // `$this->render()` outside a class is a renderer's only in a template.
                let unknown_this = classes.iter().all(|class| class == TEMPLATE_THIS);
                let told = !project.template_paths.is_empty() || !project.namespaces.is_empty();
                let plain = !value.contains(['$', '{']);
                let files = project.template_files(value);
                (told
                    && plain
                    && (!unknown_this || project.is_template(&site))
                    && !files.iter().any(|file| file.is_file()))
                .then(|| Undefined::warning(format!("Unknown template '{value}'")))
            }
            Source::Service => {
                // A class's name may be made without being declared; so may anything an
                // abstract factory makes.
                let plain = !value.contains('\\')
                    && value.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'));
                let builtin = ["config", "application", "request", "response", "servicemanager", "eventmanager"];
                let known = project.service(Manager::Services, value).is_some()
                    || builtin.contains(&value.to_ascii_lowercase().as_str())
                    || cx.engine.resolver().class_model(value).is_some();
                let container = self.manager(cx) == Some(Manager::Services);
                (plain && container && !known && !project.abstract_factories && !project.services.is_empty())
                    .then(|| Undefined::warning(format!("Unknown service '{value}'")))
            }
            Source::Action => {
                let (class, method) = Self::action(cx, value)?;
                let resolver = cx.engine.resolver();
                let model = resolver.class_model(&class)?;
                (model.method(&method).is_none()).then(|| {
                    let short = class.rsplit('\\').next().unwrap_or(&class);
                    Undefined::warning(format!("Controller '{short}' has no action '{value}'"))
                })
            }
            _ => None,
        }
    }

    fn hover(&self, cx: &RefContext<'_>, value: &str) -> Option<String> {
        let project = Self::project(cx);
        let target = self.targets(cx, value).into_iter().next()?;
        let root = cx.engine.root();
        let file = target.path.strip_prefix(&root).unwrap_or(&target.path).to_string_lossy().replace('\\', "/");
        let place =
            if target.range == Range::default() { file } else { format!("{file}:{}", target.range.start.line + 1) };
        let short = |class: &str| class.rsplit('\\').next().unwrap_or(class).to_owned();
        let detail = match self.source {
            Source::Route => project.route(value).first().and_then(|route| {
                let controller = route.controller.as_deref()?;
                Some(match &route.action {
                    Some(action) => format!("{}::{}Action()", short(controller), camel(action)),
                    None => controller.to_owned(),
                })
            }),
            Source::Navigation => None,
            Source::RouteParam => Self::params_route(cx, &project).and_then(|route| route.path.clone()),
            _ if self.of_manager() => self
                .manager(cx)
                .and_then(|manager| project.resolved(manager, value))
                .and_then(|service| service.target.clone().or_else(|| service.made.clone())),
            Source::Action => Self::action(cx, value).map(|(class, method)| format!("{}::{method}()", short(&class))),
            Source::Config => Self::config_keys(cx).and_then(|above| {
                project
                    .keys_below(&above)
                    .into_iter()
                    .rev()
                    .find(|key| key.name == value)
                    .and_then(|key| key.detail.clone())
            }),
            _ => None,
        };
        let mut out = format!("**{}** `{value}`", self.label);
        if let Some(detail) = detail {
            out.push_str(&format!("\n\n`{detail}`"));
        }
        out.push_str(&format!("\n\n{place}"));
        Some(out)
    }
}

/// Between the keys of the configuration, in what the usages of one share.
const BACKSLASH: char = '\\';
const SEPARATOR: char = '\u{1f}';
