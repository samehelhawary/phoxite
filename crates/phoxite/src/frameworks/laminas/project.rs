//! What the configuration of a Laminas MVC or a Mezzio application says
//! ([`LaminasProject`]), read with [`crate::frameworks::php_array`]:
//!
//! - **Where it is.** MVC: each module's `module/<Name>/config/module.config.php`,
//!   the modules `config/modules.config.php` lists that Composer installed
//!   (their `Module::getConfig()` or `ConfigProvider`), then
//!   `config/autoload/*.global.php` and `*.local.php`. Mezzio: the
//!   `ConfigProvider` classes `config/config.php` aggregates and those of
//!   the project's `src/`, then `config/autoload/`, and the routes of
//!   `config/routes.php`.
//! - **Services:** `service_manager` (MVC) and `dependencies` (Mezzio):
//!   `aliases`, `factories`, `invokables`, `services`, `delegators`. The
//!   same of `controllers`, `controller_plugins` and `view_helpers`, with
//!   the plugins and helpers Laminas's own plugin managers list.
//! - **Routes:** `router.routes`, a child route as `parent/child`, each with
//!   the controller and action of its defaults; Mezzio's
//!   `$app->get('/', Handler::class, 'home')`.
//! - **Templates:** `view_manager.template_map` and `template_path_stack`;
//!   Mezzio's `templates.paths` (`app::home-page` is
//!   `templates/app/home-page.phtml`).
//! - **Configuration:** every key, with the keys above it.
//!
//! The behaviour follows Laminas's and Mezzio's documentation; the engine
//! keeps the result for an edit count.

use std::path::{Path, PathBuf};

use mago_span::HasSpan;
use mago_syntax::cst::{
    Access, Argument, ArrayElement, ClassLikeConstantSelector, ClassLikeMemberSelector, Expression, MethodCall,
};
use mago_syntax::walker::Walker;

use crate::base::Range;
use crate::frameworks::php_array::{self, Node, elements, string_literal, tidy};
use crate::names::{FileNames, NameKind, Resolution};
use crate::syntax::ParsedFile;

/// A key of the configuration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigKey {
    /// The keys above it.
    pub above: Vec<String>,
    pub name: String,
    /// Its value, when it is short.
    pub detail: Option<String>,
    pub file: PathBuf,
    pub range: Range,
}

/// How a service manager makes a service.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServiceKind {
    /// Another name of a service.
    Alias,
    /// Made by a factory class.
    Factory,
    /// A class made with `new`.
    Invokable,
    /// An instance the configuration holds.
    Service,
    Delegator,
}

/// A service a configuration declares.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Service {
    pub name: String,
    pub kind: ServiceKind,
    /// An alias's service, a factory's class, an invokable's class, the
    /// last of a service's delegator factories.
    pub target: Option<String>,
    /// The class a factory written as a closure makes.
    pub made: Option<String>,
    pub file: PathBuf,
    pub range: Range,
}

/// A route that has a name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Route {
    /// `blog/post` for the child `post` of `blog`.
    pub name: String,
    /// The controller of its defaults, or its handler.
    pub controller: Option<String>,
    pub action: Option<String>,
    /// Its path as written: `/blog/:id`, `/api/users/{id}`.
    pub path: Option<String>,
    /// The parameters its path has, its parents' first.
    pub params: Vec<String>,
    pub file: PathBuf,
    pub range: Range,
}

/// The parameters a route's path names: `:id` and `[/:action]` of Laminas's
/// router, `{id}` and `{id:\d+}` of FastRoute.
pub fn path_params(path: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let bytes: Vec<char> = path.chars().collect();
    let word = |from: usize| -> String {
        bytes[from..].iter().take_while(|c| c.is_ascii_alphanumeric() || **c == '_').collect()
    };
    for (at, c) in bytes.iter().enumerate() {
        if matches!(c, ':' | '{') {
            // (`{id:\d+}`: the colon after a name starts its pattern, not a name.)
            let after_name = *c == ':' && at > 0 && (bytes[at - 1].is_ascii_alphanumeric() || bytes[at - 1] == '_');
            let name = word(at + 1);
            if !after_name && !name.is_empty() && !out.contains(&name) {
                out.push(name);
            }
        }
    }
    out
}

/// A template a `template_map` names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MappedTemplate {
    pub name: String,
    pub path: PathBuf,
    pub file: PathBuf,
    pub range: Range,
}

/// The plugin managers a name is one of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Manager {
    Services,
    Controllers,
    Plugins,
    Helpers,
    Forms,
    Validators,
    Filters,
    Hydrators,
    InputFilters,
    /// The navigation containers of `navigation`.
    Navigation,
}

impl Manager {
    /// The plugin managers, each with the key of its configuration, the
    /// method of a `Module` returning it, and its classes (Laminas's and
    /// Zend Framework's) whose own defaults are read.
    pub const ALL: [(Manager, &'static str, &'static str, [&'static str; 2]); 9] = [
        (
            Manager::Services,
            "service_manager",
            "getServiceConfig",
            ["Laminas\\ServiceManager\\ServiceManager", "Zend\\ServiceManager\\ServiceManager"],
        ),
        (
            Manager::Controllers,
            "controllers",
            "getControllerConfig",
            ["Laminas\\Mvc\\Controller\\ControllerManager", "Zend\\Mvc\\Controller\\ControllerManager"],
        ),
        (
            Manager::Plugins,
            "controller_plugins",
            "getControllerPluginConfig",
            ["Laminas\\Mvc\\Controller\\PluginManager", "Zend\\Mvc\\Controller\\PluginManager"],
        ),
        (
            Manager::Helpers,
            "view_helpers",
            "getViewHelperConfig",
            ["Laminas\\View\\HelperPluginManager", "Zend\\View\\HelperPluginManager"],
        ),
        (
            Manager::Forms,
            "form_elements",
            "getFormElementConfig",
            ["Laminas\\Form\\FormElementManager", "Zend\\Form\\FormElementManager"],
        ),
        (
            Manager::Validators,
            "validators",
            "getValidatorConfig",
            ["Laminas\\Validator\\ValidatorPluginManager", "Zend\\Validator\\ValidatorPluginManager"],
        ),
        (
            Manager::Filters,
            "filters",
            "getFilterConfig",
            ["Laminas\\Filter\\FilterPluginManager", "Zend\\Filter\\FilterPluginManager"],
        ),
        (
            Manager::Hydrators,
            "hydrators",
            "getHydratorConfig",
            ["Laminas\\Hydrator\\HydratorPluginManager", "Zend\\Hydrator\\HydratorPluginManager"],
        ),
        (
            Manager::InputFilters,
            "input_filters",
            "getInputFilterConfig",
            ["Laminas\\InputFilter\\InputFilterPluginManager", "Zend\\InputFilter\\InputFilterPluginManager"],
        ),
    ];
}

/// See the module documentation.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LaminasProject {
    pub root: PathBuf,
    /// A Mezzio application (`config/config.php`), not an MVC one.
    pub mezzio: bool,
    pub config: Vec<ConfigKey>,
    pub services: Vec<Service>,
    pub controllers: Vec<Service>,
    pub plugins: Vec<Service>,
    pub helpers: Vec<Service>,
    pub forms: Vec<Service>,
    pub validators: Vec<Service>,
    pub filters: Vec<Service>,
    pub hydrators: Vec<Service>,
    pub input_filters: Vec<Service>,
    pub navigation: Vec<Service>,
    /// The layout's template: `view_manager.layout`, Mezzio's `templates.layout`.
    pub layout: Option<String>,
    pub routes: Vec<Route>,
    pub template_map: Vec<MappedTemplate>,
    /// The folders templates are looked for in, the last added first.
    pub template_paths: Vec<PathBuf>,
    /// Mezzio's namespaces: `app` and its folders.
    pub namespaces: Vec<(String, Vec<PathBuf>)>,
    /// The extension of a template's file.
    pub extension: String,
    /// A service manager has abstract factories: a name no key declares may
    /// still be a service.
    pub abstract_factories: bool,
}

/// `IndexController` is `index`, `BlogPost` is `blog-post`.
pub fn dashed(name: &str) -> String {
    let mut out = String::new();
    for (at, c) in name.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if at > 0 && !out.ends_with(['-', '/']) {
                out.push('-');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// `blog-post` is `blogPost`.
pub fn camel(name: &str) -> String {
    let mut out = String::new();
    let mut upper = false;
    for c in name.chars() {
        if matches!(c, '-' | '_' | '.') {
            upper = true;
        } else if upper {
            out.push(c.to_ascii_uppercase());
            upper = false;
        } else {
            out.push(c);
        }
    }
    out
}

fn detail(value: &Node) -> Option<String> {
    let detail = match value {
        Node::String(text) => Some(format!("'{text}'")),
        Node::Int(value) => Some(value.to_string()),
        Node::Bool(value) => Some(value.to_string()),
        Node::Null => Some("null".to_owned()),
        Node::Class(class) => Some(format!("{class}::class")),
        Node::Float(text) | Node::Other(text) => Some(text.clone()),
        _ => None,
    };
    detail.filter(|detail| detail.len() < 120)
}

/// The class or the name a value is.
fn named(value: &Node) -> Option<String> {
    match value {
        Node::Class(class) => Some(class.clone()),
        Node::String(text) => Some(text.trim_start_matches('\\').to_owned()),
        _ => None,
    }
}

/// The `'name' => Foo::class` pairs and the `X::class` names of a file.
struct Classes<'a> {
    parsed: &'a ParsedFile,
    names: &'a FileNames,
    pairs: Vec<(String, String, Range)>,
    /// Every `Foo::class`, in order.
    classes: Vec<String>,
    /// The class whose pairs are wanted (any when `None`), and whether the
    /// walker is in it.
    owner: Option<&'a str>,
    inside: bool,
}

impl Classes<'_> {
    fn class(&self, expr: &Expression<'_>) -> Option<String> {
        let Expression::Access(Access::ClassConstant(access)) = expr else { return None };
        let ClassLikeConstantSelector::Identifier(constant) = &access.constant else { return None };
        let Expression::Identifier(identifier) = access.class else { return None };
        if !constant.value.eq_ignore_ascii_case(b"class") {
            return None;
        }
        let name = String::from_utf8_lossy(identifier.value()).into_owned();
        match self.names.scope_at(identifier.span().start.offset).resolve(NameKind::ClassLike, &name).resolution {
            Resolution::Fqn(fqn) | Resolution::Fallback { namespaced: fqn, .. } => {
                Some(fqn.trim_start_matches('\\').to_owned())
            }
            Resolution::Relative(_) => None,
        }
    }
}

struct ClassWalker;

impl<'a> Walker<'a, 'a, Classes<'a>> for ClassWalker {
    fn walk_in_class(&self, class: &'a mago_syntax::cst::Class<'a>, cx: &mut Classes<'a>) {
        cx.inside = cx.owner.is_some_and(|owner| class.name.value.eq_ignore_ascii_case(owner.as_bytes()));
    }

    fn walk_out_class(&self, _: &'a mago_syntax::cst::Class<'a>, cx: &mut Classes<'a>) {
        cx.inside = false;
    }

    fn walk_in_expression(&self, expr: &'a Expression<'a>, cx: &mut Classes<'a>) {
        if cx.owner.is_some() && !cx.inside {
            return;
        }
        if let Some(class) = cx.class(expr) {
            cx.classes.push(class);
        }
        for element in elements(expr) {
            let ArrayElement::KeyValue(element) = element else { continue };
            if let (Some((name, start, end)), Some(class)) = (string_literal(element.key), cx.class(element.value)) {
                cx.pairs.push((name, class, cx.parsed.line_index().range(start, end)));
            }
        }
    }
}

/// The pairs and the classes of the file, or of its class `owner` alone.
fn classes_of(path: &Path, text: String, owner: Option<&str>) -> (Vec<(String, String, Range)>, Vec<String>) {
    let parsed = ParsedFile::parse(&path.to_string_lossy(), text);
    let names = FileNames::resolve(&parsed);
    let mut found =
        Classes { parsed: &parsed, names: &names, pairs: Vec::new(), classes: Vec::new(), owner, inside: false };
    ClassWalker.walk_program(parsed.program(), &mut found);
    (found.pairs, found.classes)
}

/// The routes of Mezzio's `config/routes.php`.
struct Routes<'a> {
    parsed: &'a ParsedFile,
    names: &'a FileNames,
    file: &'a Path,
    found: Vec<Route>,
}

struct RouteWalker;

impl<'a> Walker<'a, 'a, Routes<'a>> for RouteWalker {
    fn walk_in_method_call(&self, call: &'a MethodCall<'a>, cx: &mut Routes<'a>) {
        let ClassLikeMemberSelector::Identifier(method) = &call.method else { return };
        let method = String::from_utf8_lossy(method.value).to_ascii_lowercase();
        let arguments: Vec<&Expression<'_>> = call
            .argument_list
            .arguments
            .iter()
            .filter_map(|argument| match argument {
                Argument::Positional(argument) => Some(argument.value),
                Argument::Named(_) => None,
            })
            .collect();
        // `$app->get('/', Handler::class, 'home')`; `route()` has the methods before the name.
        let name = match method.as_str() {
            "get" | "post" | "put" | "patch" | "delete" | "any" => arguments.get(2),
            "route" => arguments.get(3),
            "setname" => arguments.first(),
            _ => return,
        };
        let Some((name, start, end)) = name.and_then(|name| string_literal(name)) else { return };
        // The handler: a class, or the last of a pipeline.
        let classes = Classes {
            parsed: cx.parsed,
            names: cx.names,
            pairs: Vec::new(),
            classes: Vec::new(),
            owner: None,
            inside: false,
        };
        let handler = arguments.get(1).filter(|_| method != "setname").and_then(|handler| {
            classes.class(handler).or_else(|| {
                elements(handler).into_iter().rev().find_map(|element| match element {
                    ArrayElement::Value(element) => classes.class(element.value),
                    _ => None,
                })
            })
        });
        // The path is the first argument, where the route is made.
        let path = arguments.first().filter(|_| method != "setname").and_then(|path| string_literal(path));
        let path = path.map(|(path, _, _)| path);
        cx.found.push(Route {
            name,
            controller: handler,
            action: None,
            params: path.as_deref().map(path_params).unwrap_or_default(),
            path,
            file: cx.file.to_path_buf(),
            range: cx.parsed.line_index().range(start, end),
        });
    }
}

/// The folders the templates of the project at `root` are in, each with its
/// Mezzio namespace (none for a folder of a template path): what the Twig
/// side names a template by (`@app/home-page.html.twig`).
pub fn template_dirs(root: &Path) -> Vec<(Option<String>, PathBuf)> {
    let text_of = |path: &Path| std::fs::read_to_string(path).ok();
    let project = LaminasProject::load(root, &text_of, &|_| None);
    let mut out: Vec<(Option<String>, PathBuf)> = Vec::new();
    for (namespace, folders) in &project.namespaces {
        let namespace = (!namespace.is_empty()).then(|| namespace.clone());
        out.extend(folders.iter().map(|folder| (namespace.clone(), folder.clone())));
    }
    out.extend(project.template_paths.iter().map(|folder| (None, folder.clone())));
    out
}

/// The PHP files below `folder` named `name`, sorted.
fn files_named(folder: &Path, name: &str, depth: usize, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(folder) else { return };
    let mut entries: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).collect();
    entries.sort();
    for path in entries {
        let own = path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
        if path.is_dir() {
            if depth < 5 && !own.starts_with('.') && own != "vendor" && own != "node_modules" && own != "test" {
                files_named(&path, name, depth + 1, out);
            }
        } else if own == name {
            out.push(path);
        }
    }
}

impl LaminasProject {
    /// Reads the project at `root`. `text_of` reads a file (an open buffer
    /// first); `class_file` is the file declaring a class, for the modules
    /// and providers Composer installed.
    pub fn load(
        root: &Path,
        text_of: &dyn Fn(&Path) -> Option<String>,
        class_file: &dyn Fn(&str) -> Option<PathBuf>,
    ) -> Self {
        let mut out = Self { root: root.to_path_buf(), ..Self::default() };
        let config = root.join("config");
        out.mezzio = config.join("config.php").is_file() && !config.join("application.config.php").is_file();

        // What Laminas's own plugin managers list comes first: a configuration overrides it.
        for (manager, _, _, classes) in Manager::ALL {
            // (A service manager and a controller manager have none of their own.)
            if matches!(manager, Manager::Services | Manager::Controllers) {
                continue;
            }
            for class in classes {
                let Some(file) = class_file(class) else { continue };
                let Some(text) = text_of(&file) else { continue };
                let short = class.rsplit('\\').next();
                for (name, target, range) in classes_of(&file, text, short).0 {
                    out.list_mut(manager).push(Service {
                        name,
                        kind: ServiceKind::Alias,
                        target: Some(target),
                        made: None,
                        file: file.clone(),
                        range,
                    });
                }
            }
        }

        // The configuration, in the order it is merged.
        let mut nodes: Vec<Node> = Vec::new();
        let mut seen: Vec<PathBuf> = Vec::new();
        // What a `Module` returns from `getServiceConfig()` and the like, by manager.
        let mut sections: Vec<(Manager, Node)> = Vec::new();
        let mut provider = |file: PathBuf, nodes: &mut Vec<Node>| {
            if seen.contains(&file) {
                return;
            }
            let found = php_array::read_method(&file, "__invoke", text_of)
                .or_else(|| php_array::read_method(&file, "getConfig", text_of));
            nodes.extend(found);
            for (manager, _, method, _) in Manager::ALL {
                sections.extend(php_array::read_method(&file, method, text_of).map(|node| (manager, node)));
            }
            seen.push(file);
        };
        if out.mezzio {
            // The providers `config/config.php` aggregates, then the project's own.
            let aggregate = config.join("config.php");
            if let Some(text) = text_of(&aggregate) {
                for class in classes_of(&aggregate, text, None).1 {
                    if class.ends_with("ConfigProvider")
                        && let Some(file) = class_file(&class)
                    {
                        provider(file, &mut nodes);
                    }
                }
            }
            let mut own = Vec::new();
            files_named(&root.join("src"), "ConfigProvider.php", 0, &mut own);
            for file in own {
                provider(file, &mut nodes);
            }
        } else {
            // The installed modules the list names, then the project's.
            let listed = php_array::read(&config.join("modules.config.php"), text_of).or_else(|| {
                php_array::read(&config.join("application.config.php"), text_of)
                    .and_then(|node| node.get("modules").cloned())
            });
            for module in listed.iter().flat_map(|node| node.entries()) {
                let Some(name) = module.value.as_str() else { continue };
                let name = name.trim_matches('\\');
                for class in [format!("{name}\\Module"), format!("{name}\\ConfigProvider")] {
                    if let Some(file) = class_file(&class) {
                        provider(file, &mut nodes);
                    }
                }
            }
            let mut own = Vec::new();
            files_named(&root.join("module"), "Module.php", 0, &mut own);
            for file in own {
                provider(file, &mut nodes);
            }
            let mut modules = Vec::new();
            files_named(&root.join("module"), "module.config.php", 0, &mut modules);
            nodes.extend(modules.iter().filter_map(|file| php_array::read(file, text_of)));
        }
        let mut autoload: Vec<PathBuf> = std::fs::read_dir(config.join("autoload"))
            .into_iter()
            .flatten()
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "php"))
            .collect();
        // `*.global.php` before `*.local.php`, each in the order of their names.
        autoload.sort_by_key(|path| {
            let name = path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
            (name.ends_with("local.php"), name)
        });
        nodes.extend(autoload.iter().take(64).filter_map(|file| php_array::read(file, text_of)));

        for node in &nodes {
            out.add(node);
        }
        for (manager, section) in &sections {
            out.add_services(*manager, section);
        }

        out.dedupe();
        // No `templates.extension`: the renderer's own, `html.twig` for Twig's.
        if out.extension.is_empty() {
            let twig = ["Mezzio\\Twig\\TwigRenderer", "Zend\\Expressive\\Twig\\TwigRenderer"];
            let twig = twig.iter().any(|class| class_file(class).is_some());
            out.extension = if twig { "html.twig" } else { "phtml" }.to_owned();
        }

        if out.mezzio {
            let file = config.join("routes.php");
            if let Some(text) = text_of(&file) {
                let parsed = ParsedFile::parse(&file.to_string_lossy(), text);
                let names = FileNames::resolve(&parsed);
                let mut found = Routes { parsed: &parsed, names: &names, file: &file, found: Vec::new() };
                RouteWalker.walk_program(parsed.program(), &mut found);
                out.routes.extend(found.found);
            }
        }
        out
    }

    /// A module's configuration reaches the model by its file and through
    /// its `Module::getConfig()`: what is written once is kept once.
    fn dedupe(&mut self) {
        fn once<T: Clone + PartialEq>(list: &mut Vec<T>) {
            let mut kept: Vec<T> = Vec::with_capacity(list.len());
            for item in list.drain(..) {
                if !kept.contains(&item) {
                    kept.push(item);
                }
            }
            *list = kept;
        }
        once(&mut self.routes);
        once(&mut self.services);
        once(&mut self.controllers);
        once(&mut self.plugins);
        once(&mut self.helpers);
        once(&mut self.forms);
        once(&mut self.validators);
        once(&mut self.filters);
        once(&mut self.hydrators);
        once(&mut self.input_filters);
        once(&mut self.navigation);
        once(&mut self.template_map);
        // (The keys are many: by where they are written.)
        let mut seen = std::collections::HashSet::new();
        self.config
            .retain(|key| seen.insert((key.file.clone(), key.range.start.line, key.range.start.col, key.name.clone())));
    }

    /// The names the plugins or the helpers of a manager are called by: of
    /// the spellings of one name (`escapeHtml`, `escapehtml`, `EscapeHtml`)
    /// the one a method is written with, a configuration's before
    /// Laminas's own.
    pub fn callable_names(&self, manager: Manager) -> Vec<&Service> {
        self.spellings(manager, false)
    }

    /// One spelling of each name of a manager: the one starting with a
    /// capital (a validator's `StringLength`) or with a small letter (a
    /// helper's `escapeHtml`), with the most capitals after it.
    pub fn spellings(&self, manager: Manager, capital: bool) -> Vec<&Service> {
        let mut out: Vec<&Service> = Vec::new();
        let capitals = |name: &str| name.chars().filter(char::is_ascii_uppercase).count();
        for service in self.list(manager).iter().rev() {
            let name = &service.name;
            let starts =
                name.starts_with(|c: char| if capital { c.is_ascii_uppercase() } else { c.is_ascii_lowercase() });
            if name.contains('\\') || !starts || service.kind == ServiceKind::Delegator {
                continue;
            }
            match out.iter_mut().find(|known| known.name.eq_ignore_ascii_case(name)) {
                Some(known) if capitals(name) > capitals(&known.name) => *known = service,
                Some(_) => {}
                None => out.push(service),
            }
        }
        out
    }

    fn list_mut(&mut self, manager: Manager) -> &mut Vec<Service> {
        match manager {
            Manager::Services => &mut self.services,
            Manager::Controllers => &mut self.controllers,
            Manager::Plugins => &mut self.plugins,
            Manager::Helpers => &mut self.helpers,
            Manager::Forms => &mut self.forms,
            Manager::Validators => &mut self.validators,
            Manager::Filters => &mut self.filters,
            Manager::Hydrators => &mut self.hydrators,
            Manager::InputFilters => &mut self.input_filters,
            Manager::Navigation => &mut self.navigation,
        }
    }

    /// The services of a plugin manager.
    pub fn list(&self, manager: Manager) -> &[Service] {
        match manager {
            Manager::Services => &self.services,
            Manager::Controllers => &self.controllers,
            Manager::Plugins => &self.plugins,
            Manager::Helpers => &self.helpers,
            Manager::Forms => &self.forms,
            Manager::Validators => &self.validators,
            Manager::Filters => &self.filters,
            Manager::Hydrators => &self.hydrators,
            Manager::InputFilters => &self.input_filters,
            Manager::Navigation => &self.navigation,
        }
    }

    /// What one configuration array says.
    fn add(&mut self, node: &Node) {
        self.flatten(node, &mut Vec::new());
        for (manager, key, _, _) in Manager::ALL {
            if let Some(section) = node.get(key) {
                self.add_services(manager, section);
            }
        }
        if let Some(section) = node.get("dependencies") {
            self.add_services(Manager::Services, section);
        }
        // The navigation containers: `default`, and any other.
        for entry in node.get("navigation").map(Node::entries).unwrap_or_default() {
            if let Some(name) = entry.name() {
                self.navigation.push(Service {
                    name: name.to_owned(),
                    kind: ServiceKind::Service,
                    target: None,
                    made: None,
                    file: entry.file.to_path_buf(),
                    range: entry.range,
                });
            }
        }
        if let Some(routes) = node.at(&["router", "routes"]) {
            self.add_routes(routes, "", &[]);
        }
        // Mezzio's routes written as configuration: named by a key, or by `'name'`.
        for entry in node.get("routes").map(Node::entries).unwrap_or_default() {
            let named_by = entry.value.entry("name").filter(|name| name.value.as_str().is_some());
            let (name, range) = match (named_by, entry.name()) {
                (Some(name), _) => (name.value.as_str().unwrap_or_default().to_owned(), name.value_range),
                (None, Some(key)) => (key.to_owned(), entry.range),
                (None, None) => continue,
            };
            self.routes.push(Route {
                name,
                controller: entry.value.get("middleware").and_then(named),
                action: None,
                params: entry.value.get("path").and_then(Node::as_str).map(path_params).unwrap_or_default(),
                path: entry.value.get("path").and_then(Node::as_str).map(str::to_owned),
                file: entry.file.to_path_buf(),
                range,
            });
        }
        let layout = node.at(&["view_manager", "layout"]).or_else(|| node.at(&["templates", "layout"]));
        if let Some(layout) = layout.and_then(Node::as_str) {
            self.layout = Some(layout.to_owned());
        }
        if let Some(view) = node.get("view_manager") {
            for entry in view.get("template_map").map(Node::entries).unwrap_or_default() {
                if let (Some(name), Some(path)) = (entry.name(), entry.value.as_str()) {
                    self.template_map.push(MappedTemplate {
                        name: name.to_owned(),
                        path: tidy(Path::new(path)),
                        file: entry.file.to_path_buf(),
                        range: entry.range,
                    });
                }
            }
            for entry in view.get("template_path_stack").map(Node::entries).unwrap_or_default() {
                if let Some(path) = entry.value.as_str().map(|path| tidy(Path::new(path))) {
                    self.template_paths.retain(|known| *known != path);
                    self.template_paths.insert(0, path);
                }
            }
        }
        if let Some(templates) = node.get("templates") {
            if let Some(extension) = templates.get("extension").and_then(Node::as_str) {
                self.extension = extension.trim_start_matches('.').to_owned();
            }
            for entry in templates.get("paths").map(Node::entries).unwrap_or_default() {
                let namespace = entry.name().unwrap_or_default().to_owned();
                let paths: Vec<PathBuf> = match &entry.value {
                    Node::String(path) => vec![tidy(Path::new(path))],
                    list => {
                        list.entries().iter().filter_map(|path| Some(tidy(Path::new(path.value.as_str()?)))).collect()
                    }
                };
                match self.namespaces.iter_mut().find(|(known, _)| *known == namespace) {
                    Some((_, known)) => known.extend(paths),
                    None => self.namespaces.push((namespace, paths)),
                }
            }
            for entry in templates.get("map").map(Node::entries).unwrap_or_default() {
                if let (Some(name), Some(path)) = (entry.name(), entry.value.as_str()) {
                    self.template_map.push(MappedTemplate {
                        name: name.to_owned(),
                        path: tidy(Path::new(path)),
                        file: entry.file.to_path_buf(),
                        range: entry.range,
                    });
                }
            }
        }
    }

    /// What a plugin manager's configuration declares: its `aliases`,
    /// `factories`, `invokables`, `services` and `delegators`.
    fn add_services(&mut self, manager: Manager, section: &Node) {
        if section.get("abstract_factories").is_some_and(|found| !found.entries().is_empty()) {
            self.abstract_factories |= manager == Manager::Services;
        }
        let kinds = [
            ("aliases", ServiceKind::Alias),
            ("factories", ServiceKind::Factory),
            ("invokables", ServiceKind::Invokable),
            ("services", ServiceKind::Service),
            ("delegators", ServiceKind::Delegator),
        ];
        for (kind_key, kind) in kinds {
            for entry in section.get(kind_key).map(Node::entries).unwrap_or_default() {
                let target = match kind {
                    // `'mailer' => [LoggingDelegator::class]`: the last one wraps the others.
                    ServiceKind::Delegator => entry.value.entries().iter().rev().find_map(|found| named(&found.value)),
                    _ => named(&entry.value),
                };
                let made = match &entry.value {
                    Node::Closure { returns } => returns.clone(),
                    _ => None,
                };
                // `'invokables' => [Foo::class]`: the class is its own name.
                let Some(name) = entry.name().map(str::to_owned).or_else(|| target.clone()) else { continue };
                self.list_mut(manager).push(Service {
                    name: name.trim_start_matches('\\').to_owned(),
                    kind,
                    target,
                    made,
                    file: entry.file.to_path_buf(),
                    range: entry.range,
                });
            }
        }
    }

    /// The delegator factory of the service `name`: the last declared.
    pub fn delegator(&self, manager: Manager, name: &str) -> Option<&Service> {
        let name = name.trim_start_matches('\\');
        self.list(manager).iter().rev().find(|service| service.kind == ServiceKind::Delegator && service.name == name)
    }

    fn flatten(&mut self, node: &Node, above: &mut Vec<String>) {
        if above.len() > 6 || self.config.len() >= 8000 {
            return;
        }
        for entry in node.entries() {
            let Some(name) = entry.name() else { continue };
            self.config.push(ConfigKey {
                above: above.clone(),
                name: name.to_owned(),
                detail: detail(&entry.value),
                file: entry.file.to_path_buf(),
                range: entry.range,
            });
            above.push(name.to_owned());
            self.flatten(&entry.value, above);
            above.pop();
        }
    }

    fn add_routes(&mut self, routes: &Node, prefix: &str, above: &[String]) {
        for entry in routes.entries() {
            let Some(name) = entry.name() else { continue };
            let full = format!("{prefix}{name}");
            let defaults = entry.value.at(&["options", "defaults"]);
            let path = entry.value.at(&["options", "route"]).and_then(Node::as_str).map(str::to_owned);
            let mut params = above.to_vec();
            for param in path.as_deref().map(path_params).unwrap_or_default() {
                if !params.contains(&param) {
                    params.push(param);
                }
            }
            self.routes.push(Route {
                name: full.clone(),
                controller: defaults.and_then(|defaults| defaults.get("controller")).and_then(named),
                action: defaults.and_then(|defaults| defaults.get("action")).and_then(Node::as_str).map(str::to_owned),
                path,
                params: params.clone(),
                file: entry.file.to_path_buf(),
                range: entry.range,
            });
            if let Some(children) = entry.value.get("child_routes") {
                self.add_routes(children, &format!("{full}/"), &params);
            }
        }
    }

    /// The service named `name` of a plugin manager: the last declared, by
    /// its name as written, else whatever its case (plugin managers
    /// normalise names).
    pub fn service(&self, manager: Manager, name: &str) -> Option<&Service> {
        let list = self.list(manager);
        let name = name.trim_start_matches('\\');
        let exact = list.iter().rev().find(|service| service.name == name && service.kind != ServiceKind::Delegator);
        exact.or_else(|| {
            let plain = |name: &str| name.replace(['_', '-', ' '], "").to_ascii_lowercase();
            let wanted = plain(name);
            let loose = manager != Manager::Services;
            list.iter()
                .rev()
                .find(|service| loose && service.kind != ServiceKind::Delegator && plain(&service.name) == wanted)
        })
    }

    /// The service `name` names once its aliases are followed.
    pub fn resolved(&self, manager: Manager, name: &str) -> Option<&Service> {
        let mut found = self.service(manager, name)?;
        for _ in 0..8 {
            match (found.kind, found.target.as_deref()) {
                (ServiceKind::Alias, Some(target)) => match self.service(manager, target) {
                    Some(next) if !std::ptr::eq(next, found) => found = next,
                    _ => return Some(found),
                },
                _ => return Some(found),
            }
        }
        Some(found)
    }

    /// The routes named `name`.
    pub fn route(&self, name: &str) -> Vec<&Route> {
        self.routes.iter().filter(|route| route.name == name).collect()
    }

    /// The files the template `name` may be, the likeliest first: what a
    /// `template_map` says, `ns::name` in the namespace's folders, else in
    /// each folder of the path stack.
    pub fn template_files(&self, name: &str) -> Vec<PathBuf> {
        if name.is_empty() || name.contains(['$', '{']) {
            return Vec::new();
        }
        let mut out: Vec<PathBuf> = self
            .template_map
            .iter()
            .rev()
            .filter(|mapped| mapped.name == name)
            .map(|mapped| mapped.path.clone())
            .collect();
        let file = |folder: &Path, name: &str| {
            let file = tidy(&folder.join(name));
            let named = file.file_name().is_some_and(|name| name.to_string_lossy().contains('.'));
            if named { file } else { PathBuf::from(format!("{}.{}", file.to_string_lossy(), self.extension)) }
        };
        match name.split_once("::") {
            Some((namespace, rest)) => {
                let folders = self.namespaces.iter().filter(|(known, _)| known == namespace);
                out.extend(folders.flat_map(|(_, folders)| folders.iter().map(|folder| file(folder, rest))));
            }
            None => {
                out.extend(self.template_paths.iter().map(|folder| file(folder, name)));
                let default = self.namespaces.iter().filter(|(known, _)| known.is_empty());
                out.extend(default.flat_map(|(_, folders)| folders.iter().map(|folder| file(folder, name))));
            }
        }
        let mut seen = Vec::new();
        out.retain(|file| {
            !seen.contains(file) && {
                seen.push(file.clone());
                true
            }
        });
        out
    }

    /// The names the template file `path` is rendered by: its key of a
    /// `template_map`, its path below a folder of the stack
    /// (`application/index/index`), `ns::name` in Mezzio.
    pub fn template_names(&self, path: &Path) -> Vec<String> {
        let mut out: Vec<String> =
            self.template_map.iter().filter(|mapped| mapped.path == path).map(|mapped| mapped.name.clone()).collect();
        let below = |folder: &Path| {
            let relative = path.strip_prefix(folder).ok()?.to_string_lossy().replace('\\', "/");
            let suffix = format!(".{}", self.extension);
            Some(relative.strip_suffix(&suffix).map(str::to_owned).unwrap_or(relative))
        };
        out.extend(self.template_paths.iter().filter_map(|folder| below(folder)));
        for (namespace, folders) in &self.namespaces {
            for name in folders.iter().filter_map(|folder| below(folder)) {
                out.push(if namespace.is_empty() { name } else { format!("{namespace}::{name}") });
            }
        }
        out.dedup();
        out
    }

    /// Every template's name, from the maps and the folders.
    pub fn all_templates(&self) -> Vec<String> {
        fn walk(root: &Path, folder: &Path, extension: &str, depth: usize, out: &mut Vec<String>) {
            let Ok(entries) = std::fs::read_dir(folder) else { return };
            let mut entries: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).collect();
            entries.sort();
            for path in entries {
                if out.len() >= 4000 {
                    return;
                }
                if path.is_dir() {
                    if depth < 8 {
                        walk(root, &path, extension, depth + 1, out);
                    }
                } else if let Ok(relative) = path.strip_prefix(root)
                    && let Some(name) =
                        relative.to_string_lossy().replace('\\', "/").strip_suffix(&format!(".{extension}"))
                {
                    out.push(name.to_owned());
                }
            }
        }
        let mut out: Vec<String> = self.template_map.iter().map(|mapped| mapped.name.clone()).collect();
        for folder in &self.template_paths {
            walk(folder, folder, &self.extension, 0, &mut out);
        }
        for (namespace, folders) in &self.namespaces {
            for folder in folders {
                let mut found = Vec::new();
                walk(folder, folder, &self.extension, 0, &mut found);
                out.extend(
                    found
                        .into_iter()
                        .map(|name| if namespace.is_empty() { name } else { format!("{namespace}::{name}") }),
                );
            }
        }
        let mut seen = std::collections::HashSet::new();
        out.retain(|name| seen.insert(name.clone()));
        out
    }

    /// The template an MVC action renders when it names none:
    /// `application/index/index` for `indexAction()` of
    /// `Application\Controller\IndexController`.
    pub fn action_template(controller: &str, action: &str) -> Option<String> {
        let controller = controller.trim_start_matches('\\');
        let (module, rest) = controller.split_once('\\')?;
        // What follows `Controller\` names the controller: `Admin\PostController` is `admin/post`.
        let rest = rest.strip_prefix("Controller\\").unwrap_or(rest.rsplit('\\').next()?);
        let name = rest.strip_suffix("Controller").unwrap_or(rest);
        let action = action.strip_suffix("Action").unwrap_or(action);
        Some(format!("{}/{}/{}", dashed(module), dashed(&name.replace('\\', "/")), dashed(action)))
    }

    /// The keys of the configuration below the keys `above`.
    pub fn keys_below(&self, above: &[String]) -> Vec<&ConfigKey> {
        self.config.iter().filter(|key| key.above == above).collect()
    }

    /// Whether the file `path` has the templates' extension (`phtml`, or
    /// one of several parts: `html.twig`).
    fn has_extension(&self, path: &Path) -> bool {
        let name = path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
        name.ends_with(&format!(".{}", self.extension))
    }

    /// Whether `path` is a template: a file with the templates' extension
    /// in a folder templates are looked for in, or one a map names.
    pub fn is_template(&self, path: &Path) -> bool {
        self.template_map.iter().any(|mapped| mapped.path == path)
            || (self.has_extension(path)
                && (self.template_paths.iter().any(|folder| path.starts_with(folder))
                    || self
                        .namespaces
                        .iter()
                        .any(|(_, folders)| folders.iter().any(|folder| path.starts_with(folder)))))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(files: &[(&str, &str)]) -> (tempfile::TempDir, LaminasProject) {
        let tmp = tempfile::tempdir().unwrap();
        for (rel, text) in files {
            let path = tmp.path().join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
        let root = tmp.path().to_path_buf();
        let text_of = |path: &Path| std::fs::read_to_string(path).ok();
        // (A class of `vendor/` is in the file of its name.)
        let class_file = |class: &str| {
            let file = root.join("vendor").join(format!("{}.php", class.replace('\\', "/")));
            file.is_file().then_some(file)
        };
        let project = LaminasProject::load(&root, &text_of, &class_file);
        (tmp, project)
    }

    fn shown(root: &Path, path: &Path) -> String {
        path.strip_prefix(root).unwrap_or(path).to_string_lossy().replace('\\', "/")
    }

    const MODULE: &str = r#"<?php
namespace Application;

use Laminas\ServiceManager\Factory\InvokableFactory;

return [
    'router' => [
        'routes' => [
            'home' => [
                'type' => 'Literal',
                'options' => ['route' => '/', 'defaults' => ['controller' => Controller\IndexController::class, 'action' => 'index']],
            ],
            'blog' => [
                'type' => 'Literal',
                'options' => ['route' => '/blog', 'defaults' => ['controller' => Controller\BlogController::class, 'action' => 'list']],
                'child_routes' => [
                    'post' => ['type' => 'Segment', 'options' => ['route' => '/:id', 'defaults' => ['action' => 'show']]],
                ],
            ],
        ],
    ],
    'controllers' => ['factories' => [Controller\IndexController::class => InvokableFactory::class]],
    'service_manager' => [
        'aliases' => ['mailer' => Service\Mailer::class, 'post' => 'mailer'],
        'factories' => [Service\Mailer::class => Service\MailerFactory::class],
        'invokables' => [Service\Clock::class],
    ],
    'controller_plugins' => ['invokables' => ['greet' => Plugin\Greet::class]],
    'view_helpers' => ['factories' => ['price' => Helper\PriceFactory::class]],
    'view_manager' => [
        'template_map' => ['layout/layout' => __DIR__ . '/../view/layout/layout.phtml'],
        'template_path_stack' => [__DIR__ . '/../view'],
    ],
];
"#;

    #[test]
    fn an_mvc_application_names_its_services_routes_and_templates() {
        let (tmp, project) = project(&[
            (
                "config/application.config.php",
                "<?php\nreturn ['modules' => require __DIR__ . '/modules.config.php'];\n",
            ),
            ("config/modules.config.php", "<?php\nreturn ['Laminas\\Router', 'Application'];\n"),
            ("module/Application/config/module.config.php", MODULE),
            ("config/autoload/global.php", "<?php\nreturn ['db' => ['host' => 'localhost']];\n"),
            ("config/autoload/local.php", "<?php\nreturn ['db' => ['host' => 'db']];\n"),
            (
                "vendor/Laminas/Router/Module.php",
                "<?php\nnamespace Laminas\\Router;\nclass Module {\n    public function getConfig() {\n        return ['service_manager' => ['factories' => ['Router' => RouterFactory::class]]];\n    }\n}\n",
            ),
            (
                "vendor/Laminas/Mvc/Controller/PluginManager.php",
                "<?php\nnamespace Laminas\\Mvc\\Controller;\nclass PluginManager {\n    protected $aliases = ['url' => Plugin\\Url::class, 'Url' => Plugin\\Url::class];\n}\n",
            ),
        ]);
        let root = tmp.path();
        assert!(!project.mezzio);
        let routes: Vec<(&str, Option<&str>, Option<&str>)> = project
            .routes
            .iter()
            .map(|route| (route.name.as_str(), route.controller.as_deref(), route.action.as_deref()))
            .collect();
        assert_eq!(
            routes,
            [
                ("home", Some("Application\\Controller\\IndexController"), Some("index")),
                ("blog", Some("Application\\Controller\\BlogController"), Some("list")),
                ("blog/post", None, Some("show")),
            ]
        );
        // An alias leads to its service; a class without a key is its own name.
        let mailer = project.resolved(Manager::Services, "post").unwrap();
        assert_eq!(
            (mailer.kind, mailer.target.as_deref()),
            (ServiceKind::Factory, Some("Application\\Service\\MailerFactory"))
        );
        assert_eq!(
            project.service(Manager::Services, "Application\\Service\\Clock").unwrap().kind,
            ServiceKind::Invokable
        );
        assert_eq!(
            shown(root, &project.service(Manager::Services, "Router").unwrap().file),
            "vendor/Laminas/Router/Module.php"
        );
        // Plugins and helpers: Laminas's own, then the configuration's, whatever their case.
        assert_eq!(
            project.service(Manager::Plugins, "URL").unwrap().target.as_deref(),
            Some("Laminas\\Mvc\\Controller\\Plugin\\Url")
        );
        assert_eq!(
            project.service(Manager::Plugins, "greet").unwrap().target.as_deref(),
            Some("Application\\Plugin\\Greet")
        );
        assert!(project.service(Manager::Helpers, "price").is_some());
        // Templates: a map's file, the stack's.
        let files =
            |name: &str| -> Vec<String> { project.template_files(name).iter().map(|file| shown(root, file)).collect() };
        assert_eq!(files("layout/layout"), ["module/Application/view/layout/layout.phtml"]);
        assert_eq!(files("application/index/index"), ["module/Application/view/application/index/index.phtml"]);
        let view = root.join("module/Application/view/application/index/index.phtml");
        assert_eq!(project.template_names(&view), ["application/index/index"]);
        assert!(project.is_template(&view));
        assert_eq!(
            LaminasProject::action_template("Application\\Controller\\IndexController", "indexAction").as_deref(),
            Some("application/index/index")
        );
        assert_eq!(
            LaminasProject::action_template("Blog\\Controller\\Admin\\BlogPostController", "showAll").as_deref(),
            Some("blog/admin/blog-post/show-all")
        );
        // The configuration's keys, a later file's after an earlier one's.
        let hosts: Vec<String> = project
            .config
            .iter()
            .filter(|key| key.above == ["db"] && key.name == "host")
            .map(|key| shown(root, &key.file))
            .collect();
        assert_eq!(hosts, ["config/autoload/global.php", "config/autoload/local.php"]);
        let below: Vec<&str> =
            project.keys_below(&["view_manager".to_owned()]).iter().map(|key| key.name.as_str()).collect();
        assert_eq!(below, ["template_map", "template_path_stack"]);
    }

    #[test]
    fn a_path_names_its_parameters() {
        assert_eq!(path_params("/post/:id[/:slug]"), ["id", "slug"]);
        assert_eq!(path_params("/api/users/{id:\\d+}/posts/{post}"), ["id", "post"]);
        assert_eq!(path_params("/blog"), Vec::<String>::new());
    }

    #[test]
    fn a_mezzio_application_names_its_services_routes_and_templates() {
        let provider = "<?php\nnamespace App;\n\nclass ConfigProvider\n{\n    public function __invoke(): array\n    {\n        return ['dependencies' => $this->getDependencies(), 'templates' => $this->getTemplates()];\n    }\n\n    public function getDependencies(): array\n    {\n        return [\n            'invokables' => [Handler\\PingHandler::class => Handler\\PingHandler::class],\n            'factories' => [Handler\\HomePageHandler::class => Handler\\HomePageHandlerFactory::class],\n        ];\n    }\n\n    public function getTemplates(): array\n    {\n        return ['paths' => ['app' => [__DIR__ . '/../templates/app'], 'layout' => [__DIR__ . '/../templates/layout']]];\n    }\n}\n";
        let (tmp, project) = project(&[
            (
                "config/config.php",
                "<?php\n$aggregator = new ConfigAggregator([\n    \\Mezzio\\Helper\\ConfigProvider::class,\n    App\\ConfigProvider::class,\n]);\nreturn $aggregator->getMergedConfig();\n",
            ),
            (
                "config/routes.php",
                "<?php\nuse App\\Handler\\HomePageHandler;\nreturn static function ($app): void {\n    $app->get('/', HomePageHandler::class, 'home');\n    $app->route('/api/ping', [Auth::class, App\\Handler\\PingHandler::class], ['GET'], 'api.ping');\n};\n",
            ),
            ("src/App/src/ConfigProvider.php", provider),
            (
                "vendor/Mezzio/Helper/ConfigProvider.php",
                "<?php\nnamespace Mezzio\\Helper;\nclass ConfigProvider {\n    public function __invoke(): array {\n        return ['dependencies' => ['factories' => [UrlHelper::class => UrlHelperFactory::class]]];\n    }\n}\n",
            ),
            (
                "config/autoload/dependencies.global.php",
                "<?php\nreturn ['dependencies' => ['aliases' => ['url' => Mezzio\\Helper\\UrlHelper::class]]];\n",
            ),
        ]);
        let root = tmp.path();
        assert!(project.mezzio);
        let routes: Vec<(&str, Option<&str>, u32)> = project
            .routes
            .iter()
            .map(|route| (route.name.as_str(), route.controller.as_deref(), route.range.start.line))
            .collect();
        assert_eq!(
            routes,
            [("home", Some("App\\Handler\\HomePageHandler"), 3), ("api.ping", Some("App\\Handler\\PingHandler"), 4)]
        );
        let url = project.resolved(Manager::Services, "url").unwrap();
        assert_eq!(shown(root, &url.file), "vendor/Mezzio/Helper/ConfigProvider.php");
        assert_eq!(url.target.as_deref(), Some("Mezzio\\Helper\\UrlHelperFactory"));
        let home = project.service(Manager::Services, "App\\Handler\\HomePageHandler").unwrap();
        assert_eq!(
            (home.kind, shown(root, &home.file), home.range.start.line),
            (ServiceKind::Factory, "src/App/src/ConfigProvider.php".to_owned(), 14)
        );
        let files: Vec<String> =
            project.template_files("app::home-page").iter().map(|file| shown(root, file)).collect();
        assert_eq!(files, ["src/App/templates/app/home-page.phtml"]);
        let page = root.join("src/App/templates/app/home-page.phtml");
        assert_eq!(project.template_names(&page), ["app::home-page"]);
        assert!(project.is_template(&page) && !project.is_template(&root.join("src/App/src/ConfigProvider.php")));
    }
}
