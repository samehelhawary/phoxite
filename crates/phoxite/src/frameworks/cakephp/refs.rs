//! CakePHP's reference kinds ([`kinds`]), on the machinery every framework
//! shares ([`crate::frameworks::refs`]). All are one type, [`CakeName`].
//!
//! | kind | names | where |
//! | --- | --- | --- |
//! | `cakeTable` | a table class, by its alias (`Articles`, `Blog.Tags`) | `fetchTable()`, `loadModel()`, a table locator's `get()`, `belongsTo()`, `hasOne()`, `hasMany()`, `belongsToMany()` and their `className` and `through` options |
//! | `cakeComponent` | a component class | `loadComponent()` and its `className`, `$components`, the component registry |
//! | `cakeHelper` | a helper class | `loadHelper()`, `addHelper()`, `$helpers`, the view builder's `addHelper()` and `setHelpers()`, the helper registry |
//! | `cakeAssociation` | an association of a table, where the table declares it (`Comments.Users` follows one into the next) | a query's and a table's `contain()`, `matching()`, `innerJoinWith()`, `leftJoinWith()`, `notMatching()`; `getAssociation()`, `hasAssociation()` |
//! | `cakeBehavior` | a behaviour class | `addBehavior()`, `getBehavior()`, `hasBehavior()`, `removeBehavior()`, the behaviour registry |
//! | `cakeTemplate` | a template's file, found from who asks ([`CakeProject::template_files`]) | a controller's `render()`, `setTemplate()`, a view's `extend()` |
//! | `cakeLayout` | a file of `templates/layout` | `setLayout()`, the second argument of `render()` |
//! | `cakeElement` | a file of `templates/element` | a view's `element()`, `elementExists()` |
//! | `cakeCell` | a cell class, or its method (`Inbox::expanded`) | `cell()` |
//! | `cakeConfig` | a key of the configuration, as its dotted path | `Configure::read()`, `check()`, `readOrFail()`, `consume()`, `write()`, `delete()` |
//! | `cakeController` | a controller class | `'controller' => 'Articles'` of a route array |
//! | `cakeAction` | an action: a method of the controller the array names, else of the one asking | `'action' => 'view'` of a route array; `'Articles::view'` of a route |
//! | `cakePlugin` | a plugin | `'plugin' => 'Blog'` of a route array, `addPlugin()`, a route builder's `plugin()` |
//! | `cakeRoute` | a named route | `'_name' => 'login'` |
//!
//! Each has completion, goto, hover and Find References. Nothing is
//! renamed: the names are classes', files' and methods'.
//!
//! A component, a helper and a behaviour no class is of, a template, a
//! layout and an element whose file isn't there, and an action its
//! controller doesn't have are reported ([`ReferenceKind::undefined`]).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use mago_span::HasSpan;
use mago_syntax::cst::{ArrayElement, Expression};
use mago_syntax::walker::Walker;

use super::classes::{self, Kind, class_of};
use super::inflect::variable;
use super::project::{CakeProject, Def, Overrides, TemplateKind, plugin_split};
use crate::base::Range;
use crate::frameworks::laravel::refs::controller_action::method_target;
use crate::frameworks::php_array::{elements, string_literal, tidy};
use crate::frameworks::refs::{
    CallTarget, CalleePattern, PlacePattern, Position, RefContext, RefItem, ReferenceKind, TEMPLATE_THIS, Undefined,
};
use crate::index::path_key;
use crate::navigate::Target;
use crate::summary::Visibility;

const BACKSLASH: char = '\\';

/// Between a message's catalogue and its text, in what its usages share.
const SEPARATOR: char = '\u{1f}';

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Source {
    Table,
    Component,
    Helper,
    Behavior,
    Association,
    Template,
    Layout,
    Element,
    Cell,
    Config,
    Controller,
    Action,
    Plugin,
    Route,
    Message,
    Domain,
}

/// One of the kinds of the module documentation.
pub struct CakeName {
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

const fn property(classes: &'static [&'static str], name: &'static [&'static str]) -> PlacePattern {
    PlacePattern::argument(CalleePattern::Property { classes, names: name }, 0, "value")
}

const LOCATOR_AWARE: &[&str] = &["Cake\\ORM\\Locator\\LocatorAwareTrait", "Cake\\Datasource\\ModelAwareTrait"];
const LOCATORS: &[&str] = &[
    "Cake\\ORM\\Locator\\LocatorInterface",
    "Cake\\Datasource\\Locator\\LocatorInterface",
    "Cake\\ORM\\TableRegistry",
];
const TABLES: &[&str] = &[classes::TABLE];
const CONTROLLERS: &[&str] = &[classes::CONTROLLER];
const COMPONENT_HOSTS: &[&str] = &[classes::CONTROLLER, classes::COMPONENT];
const COMPONENT_REGISTRY: &[&str] = &["Cake\\Controller\\ComponentRegistry"];
const VIEWS: &[&str] = &[classes::VIEW, TEMPLATE_THIS];
const HELPER_HOSTS: &[&str] = &[classes::VIEW, classes::HELPER];
const VIEW_BUILDER: &[&str] = &["Cake\\View\\ViewBuilder"];
const HELPER_REGISTRY: &[&str] = &["Cake\\View\\HelperRegistry"];
const BEHAVIOR_REGISTRY: &[&str] = &["Cake\\ORM\\BehaviorRegistry"];
const CELLS: &[&str] = &["Cake\\View\\CellTrait", classes::VIEW, TEMPLATE_THIS];
const CONFIGURE: &[&str] = &["Cake\\Core\\Configure"];
const ROUTE_BUILDER: &[&str] = &["Cake\\Routing\\RouteBuilder"];
const ROUTER: &[&str] = &["Cake\\Routing\\Router"];
const HTML: &[&str] = &["Cake\\View\\Helper\\HtmlHelper"];
const URL: &[&str] = &["Cake\\View\\Helper\\UrlHelper"];
const FORM: &[&str] = &["Cake\\View\\Helper\\FormHelper"];
const APPLICATIONS: &[&str] = &["Cake\\Http\\BaseApplication", "Cake\\Core\\PluginApplicationInterface"];
const PLUGIN: &[&str] = &["Cake\\Core\\Plugin"];
const ASSOCIATING: &[&str] = &["belongsTo", "hasOne", "hasMany", "belongsToMany"];
const REGISTRY: &[&str] = &["load", "get", "has", "unload"];
const CONNECTING: &[&str] = &["connect", "get", "post", "put", "patch", "delete", "head", "options"];

pub static TABLE: CakeName = CakeName {
    id: "cakeTable",
    label: "table",
    source: Source::Table,
    patterns: &[
        method(LOCATOR_AWARE, &["fetchTable", "loadModel"], 0, "alias"),
        method(LOCATORS, &["get", "exists", "remove"], 0, "alias"),
        method(TABLES, ASSOCIATING, 0, "associated"),
        method(TABLES, ASSOCIATING, 1, "options").at(Position::ValueOfKey("className")),
        method(TABLES, ASSOCIATING, 1, "options").at(Position::ValueOfKey("through")),
    ],
};

pub static COMPONENT: CakeName = CakeName {
    id: "cakeComponent",
    label: "component",
    source: Source::Component,
    patterns: &[
        method(CONTROLLERS, &["loadComponent"], 0, "name"),
        method(CONTROLLERS, &["loadComponent"], 1, "config").at(Position::ValueOfKey("className")),
        method(COMPONENT_REGISTRY, REGISTRY, 0, "name"),
        property(COMPONENT_HOSTS, &["components"]).at(Position::ListValue),
        property(COMPONENT_HOSTS, &["components"]).at(Position::ArrayKey),
    ],
};

pub static HELPER: CakeName = CakeName {
    id: "cakeHelper",
    label: "helper",
    source: Source::Helper,
    patterns: &[
        method(VIEWS, &["loadHelper", "addHelper"], 0, "name"),
        method(VIEW_BUILDER, &["addHelper"], 0, "helper"),
        method(VIEW_BUILDER, &["addHelpers", "setHelpers"], 0, "helpers").at(Position::ListValue),
        method(VIEW_BUILDER, &["addHelpers", "setHelpers"], 0, "helpers").at(Position::ArrayKey),
        method(HELPER_REGISTRY, REGISTRY, 0, "name"),
        property(HELPER_HOSTS, &["helpers"]).at(Position::ListValue),
        property(HELPER_HOSTS, &["helpers"]).at(Position::ArrayKey),
    ],
};

pub static BEHAVIOR: CakeName = CakeName {
    id: "cakeBehavior",
    label: "behavior",
    source: Source::Behavior,
    patterns: &[
        method(TABLES, &["addBehavior", "removeBehavior", "getBehavior", "hasBehavior"], 0, "name"),
        method(TABLES, &["addBehaviors"], 0, "behaviors").at(Position::ListValue),
        method(TABLES, &["addBehaviors"], 0, "behaviors").at(Position::ArrayKey),
        method(BEHAVIOR_REGISTRY, REGISTRY, 0, "name"),
    ],
};

const QUERIES: &[&str] = &["Cake\\ORM\\Query\\SelectQuery", "Cake\\ORM\\Query", classes::TABLE];
const CONTAINING: &[&str] = &["contain", "matching", "innerJoinWith", "leftJoinWith", "notMatching"];

pub static ASSOCIATION: CakeName = CakeName {
    id: "cakeAssociation",
    label: "association",
    source: Source::Association,
    patterns: &[
        method(QUERIES, CONTAINING, 0, "associations"),
        method(QUERIES, &["contain"], 0, "associations").at(Position::ArrayValue).nested(),
        method(QUERIES, &["contain"], 0, "associations").at(Position::ArrayKey).nested(),
        method(TABLES, &["getAssociation", "hasAssociation"], 0, "name"),
    ],
};

pub static TEMPLATE: CakeName = CakeName {
    id: "cakeTemplate",
    label: "template",
    source: Source::Template,
    patterns: &[
        method(CONTROLLERS, &["render"], 0, "template"),
        method(VIEW_BUILDER, &["setTemplate"], 0, "name"),
        method(VIEWS, &["setTemplate", "extend"], 0, "name"),
        method(&["Cake\\View\\Cell"], &["render"], 0, "template"),
    ],
};

pub static LAYOUT: CakeName = CakeName {
    id: "cakeLayout",
    label: "layout",
    source: Source::Layout,
    patterns: &[
        method(CONTROLLERS, &["render"], 1, "layout"),
        method(VIEW_BUILDER, &["setLayout"], 0, "name"),
        method(VIEWS, &["setLayout"], 0, "name"),
    ],
};

pub static ELEMENT: CakeName = CakeName {
    id: "cakeElement",
    label: "element",
    source: Source::Element,
    patterns: &[method(VIEWS, &["element", "elementExists"], 0, "name")],
};

pub static CELL: CakeName =
    CakeName { id: "cakeCell", label: "cell", source: Source::Cell, patterns: &[method(CELLS, &["cell"], 0, "cell")] };

pub static CONFIG: CakeName = CakeName {
    id: "cakeConfig",
    label: "configuration",
    source: Source::Config,
    patterns: &[method(
        CONFIGURE,
        &["read", "readOrFail", "check", "consume", "consumeOrFail", "write", "delete"],
        0,
        "var",
    )],
};

pub static CONTROLLER: CakeName = CakeName {
    id: "cakeController",
    label: "controller",
    source: Source::Controller,
    patterns: &[entry("controller")],
};

pub static ACTION: CakeName = CakeName {
    id: "cakeAction",
    label: "action",
    source: Source::Action,
    patterns: &[
        // Where a route array is what the method takes: an action it doesn't name is reported.
        method(CONTROLLERS, &["redirect", "setAction"], 0, "url").at(Position::ValueOfKey("action")),
        method(ROUTER, &["url", "reverse"], 0, "url").at(Position::ValueOfKey("action")),
        method(URL, &["build"], 0, "url").at(Position::ValueOfKey("action")),
        method(HTML, &["link"], 1, "url").at(Position::ValueOfKey("action")),
        method(FORM, &["postLink", "postButton"], 1, "url").at(Position::ValueOfKey("action")),
        // Any other route array.
        entry("action"),
        // `'Articles::view'`.
        method(ROUTE_BUILDER, CONNECTING, 1, "defaults"),
        method(ROUTE_BUILDER, &["redirect"], 1, "url"),
        method(ROUTER, &["pathUrl"], 0, "path"),
    ],
};

pub static PLUGIN_NAME: CakeName = CakeName {
    id: "cakePlugin",
    label: "plugin",
    source: Source::Plugin,
    patterns: &[
        entry("plugin"),
        method(ROUTE_BUILDER, &["plugin"], 0, "name"),
        method(APPLICATIONS, &["addPlugin", "addOptionalPlugin"], 0, "name"),
        method(PLUGIN, &["isLoaded", "path", "classPath", "configPath", "templatePath"], 0, "name"),
    ],
};

const fn function(name: &'static str, index: usize, parameter: &'static str) -> PlacePattern {
    PlacePattern::argument(CalleePattern::Function(name), index, parameter)
}

pub static MESSAGE: CakeName = CakeName {
    id: "cakeMessage",
    label: "message",
    source: Source::Message,
    patterns: &[
        function("__", 0, "singular"),
        function("__n", 0, "singular"),
        function("__d", 1, "msg"),
        function("__dn", 1, "singular"),
        function("__x", 1, "singular"),
        function("__xn", 1, "singular"),
        function("__dx", 2, "msg"),
        function("__dxn", 2, "singular"),
    ],
};

pub static DOMAIN: CakeName = CakeName {
    id: "cakeDomain",
    label: "translation domain",
    source: Source::Domain,
    patterns: &[
        function("__d", 0, "domain"),
        function("__dn", 0, "domain"),
        function("__dx", 0, "domain"),
        function("__dxn", 0, "domain"),
    ],
};

pub static ROUTE: CakeName =
    CakeName { id: "cakeRoute", label: "route", source: Source::Route, patterns: &[entry("_name")] };

/// CakePHP's kinds.
pub fn kinds() -> &'static [&'static dyn ReferenceKind] {
    static KINDS: [&dyn ReferenceKind; 16] = [
        &TABLE,
        &COMPONENT,
        &HELPER,
        &BEHAVIOR,
        &ASSOCIATION,
        &TEMPLATE,
        &LAYOUT,
        &ELEMENT,
        &CELL,
        &CONFIG,
        &CONTROLLER,
        &ACTION,
        &PLUGIN_NAME,
        &ROUTE,
        &MESSAGE,
        &DOMAIN,
    ];
    &KINDS
}

/// The methods of a controller that aren't actions.
const NOT_ACTIONS: [&str; 10] = [
    "initialize",
    "beforefilter",
    "beforerender",
    "afterfilter",
    "beforeredirect",
    "implementedevents",
    "startupprocess",
    "shutdownprocess",
    "invokeaction",
    "isaction",
];

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
        // Walked from the outside in: the last one found is the innermost.
        if span.start.offset <= cx.offset && cx.offset <= span.end.offset {
            cx.found = Some(expr);
        }
    }
}

impl CakeName {
    fn project(cx: &RefContext<'_>) -> Arc<CakeProject> {
        cx.engine.cake()
    }

    /// The file the string is in.
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

    /// The other strings of the array the string is a value of, by their
    /// keys: the `controller`, `plugin` and `prefix` beside an `action`.
    fn siblings(cx: &RefContext<'_>) -> HashMap<String, String> {
        let mut out = HashMap::new();
        let (Some(site), Some(text)) = (cx.site.as_ref(), Self::site_text(cx)) else { return out };
        let parsed = crate::syntax::ParsedFile::parse(&site.path.to_string_lossy(), text);
        let mut around = Around { offset: site.start, found: None };
        AroundWalker.walk_program(parsed.program(), &mut around);
        let Some(array) = around.found else { return out };
        for element in elements(array) {
            let ArrayElement::KeyValue(element) = element else { continue };
            if let (Some((key, _, _)), Some((value, _, _))) =
                (string_literal(element.key), string_literal(element.value))
            {
                out.insert(key, value);
            }
        }
        out
    }

    /// The class kind the names of this kind stand for.
    fn class_kind(&self) -> Option<Kind> {
        match self.source {
            Source::Table => Some(Kind::Table),
            Source::Component => Some(Kind::Component),
            Source::Helper => Some(Kind::Helper),
            Source::Behavior => Some(Kind::Behavior),
            _ => None,
        }
    }

    fn template_kind(&self) -> Option<TemplateKind> {
        match self.source {
            Source::Template => Some(TemplateKind::Template),
            Source::Layout => Some(TemplateKind::Layout),
            Source::Element => Some(TemplateKind::Element),
            _ => None,
        }
    }

    /// The files a template, a layout or an element named `value` may be.
    fn files(&self, cx: &RefContext<'_>, value: &str) -> Vec<PathBuf> {
        let (Some(kind), Some(site)) = (self.template_kind(), Self::site_path(cx)) else { return Vec::new() };
        let project = Self::project(cx);
        let mut files = project.template_files_with(&site, value, kind, &Self::overrides(cx));
        // CakePHP's own layouts and elements come last (its error pages use them): the
        // `templates` folder beside the framework's `src`.
        let folder = match kind {
            TemplateKind::Layout => "layout",
            TemplateKind::Element => "element",
            TemplateKind::Template => return files,
        };
        let view = cx.engine.class_targets(classes::VIEW).into_iter().next().map(|target| target.path);
        let core = view.as_deref().and_then(|view| Some(view.parent()?.parent()?.parent()?.join("templates")));
        if let Some(core) = core.filter(|core| core.is_dir() && plugin_split(value).0.is_none()) {
            let file = tidy(&core.join(folder).join(value));
            let named = file.file_name().is_some_and(|name| name.to_string_lossy().contains('.'));
            files.push(if named { file } else { file.with_extension("php") });
        }
        files
    }

    /// What the code says about where the templates asked for are: the
    /// theme and the folders the class asking sets (or a class it
    /// extends), else those of the application's `AppController` and
    /// `AppView`, which every template is rendered through.
    fn overrides(cx: &RefContext<'_>) -> Overrides {
        let project = Self::project(cx);
        let resolver = cx.engine.resolver();
        let mut classes: Vec<String> = Vec::new();
        if let Some(site) = Self::site_path(cx)
            && let Some(stem) = site.file_stem().and_then(|stem| stem.to_str())
            && !site.components().any(|part| part.as_os_str() == "templates")
        {
            classes.extend(Self::class_in(cx, &site, stem));
        }
        let own = classes.len();
        classes.push(format!("{}\\Controller\\AppController", project.namespace));
        classes.push(format!("{}\\View\\AppView", project.namespace));
        let mut out = Overrides::default();
        for (at, class) in classes.iter().enumerate() {
            for facts in classes::facts(&resolver, class) {
                out.theme = out.theme.or_else(|| facts.theme.clone());
                // A folder of templates is the class's own business.
                if at < own {
                    out.template_path = out.template_path.or_else(|| facts.template_path.clone());
                    out.layout_path = out.layout_path.or_else(|| facts.layout_path.clone());
                }
            }
        }
        out
    }

    /// The file of the name `value`: the first that is there, else the
    /// first it could be.
    fn file(&self, cx: &RefContext<'_>, value: &str) -> Option<PathBuf> {
        let files = self.files(cx, value);
        files.iter().find(|file| file.is_file()).or(files.first()).cloned()
    }

    /// The fully qualified name of the class `short` the file `file`
    /// declares.
    fn class_in(cx: &RefContext<'_>, file: &Path, short: &str) -> Option<String> {
        let wanted = path_key(file).to_ascii_lowercase();
        let found = cx.engine.index()?.by_short_name(short).ok()?;
        found.into_iter().find(|decl| decl.file.to_ascii_lowercase() == wanted).map(|decl| decl.fqn)
    }

    /// The plugin and the prefix of the controller the file asking is, or
    /// is a template of, and that controller's name.
    fn own(cx: &RefContext<'_>) -> (Option<String>, String, Option<String>) {
        let project = Self::project(cx);
        let Some(context) = Self::site_path(cx).and_then(|site| project.template_context(&site)) else {
            return (None, String::new(), None);
        };
        match context.controller() {
            Some((prefix, name)) => (context.plugin.clone(), prefix.to_owned(), Some(name.to_owned())),
            None => (context.plugin.clone(), String::new(), None),
        }
    }

    /// The class of the controller `name` of a route array: in the plugin
    /// and under the prefix the array names, else those of the file asking.
    fn controller_class(cx: &RefContext<'_>, name: &str, beside: &HashMap<String, String>) -> Option<String> {
        if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return None;
        }
        let project = Self::project(cx);
        let resolver = cx.engine.resolver();
        let (own_plugin, own_prefix, _) = Self::own(cx);
        let plugin = beside.get("plugin").cloned().or(own_plugin);
        let prefix = beside.get("prefix").cloned().unwrap_or(own_prefix);
        let class = |plugin: Option<&str>, prefix: &str| {
            let namespace = project.namespace_of(plugin);
            let prefix = if prefix.is_empty() { String::new() } else { format!("{}\\", prefix.replace('/', "\\")) };
            let class = format!("{namespace}\\Controller\\{prefix}{name}Controller");
            resolver.class_model(&class).map(|model| model.fqn().to_owned())
        };
        class(plugin.as_deref(), &prefix)
            .or_else(|| class(plugin.as_deref(), ""))
            .or_else(|| class(None, &prefix))
            .or_else(|| class(None, ""))
    }

    /// The controller and the method the action `value` is: of the
    /// controller its array names, else of the one asking; or what
    /// `'Articles::view'` names.
    fn action(cx: &RefContext<'_>, value: &str) -> Option<(String, String)> {
        let resolver = cx.engine.resolver();
        let (class, action) = match value.split_once("::") {
            // `[Plugin.][Prefix/]Controller::action`.
            Some((controller, action)) => {
                let (plugin, name) = plugin_split(controller);
                let (prefix, name) = name.rsplit_once('/').unwrap_or(("", name));
                let mut beside = HashMap::new();
                beside.extend(plugin.map(|plugin| ("plugin".to_owned(), plugin.to_owned())));
                beside.insert("prefix".to_owned(), prefix.to_owned());
                (Self::controller_class(cx, name, &beside)?, action)
            }
            None => {
                // A route's `connect('/x', 'Pages::display')` is the only bare string that is one.
                if !matches!(Self::place(cx), Some(CallTarget::Entry { .. }))
                    && !cx.site.as_ref().is_some_and(|site| site.key.as_deref() == Some("action"))
                {
                    return None;
                }
                let beside = Self::siblings(cx);
                let class = match beside.get("controller") {
                    Some(name) => Self::controller_class(cx, name, &beside)?,
                    None => {
                        let site = Self::site_path(cx)?;
                        let stem = site.file_stem()?.to_str()?;
                        match Self::own(cx) {
                            // A controller's file: the class it declares.
                            (_, _, Some(_)) if stem.ends_with("Controller") => Self::class_in(cx, &site, stem)?,
                            (_, _, Some(name)) => Self::controller_class(cx, &name, &beside)?,
                            _ => return None,
                        }
                    }
                };
                (class, value)
            }
        };
        let plain = !action.is_empty() && action.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-'));
        if !plain {
            return None;
        }
        let model = resolver.class_model(&class)?;
        // `view-all` and `view_all` are `viewAll()`.
        let method = if model.method(action).is_some() { action.to_owned() } else { variable(action) };
        Some((model.fqn().to_owned(), method))
    }

    /// The actions of the controller `class`: its public methods, and those
    /// of the classes of the project it extends.
    fn actions(cx: &RefContext<'_>, class: &str) -> Vec<String> {
        let resolver = cx.engine.resolver();
        let Some(model) = resolver.class_model(class) else { return Vec::new() };
        let mut out = Vec::new();
        for method in model.methods.iter() {
            let own = !method.declaring.fqn.trim_start_matches('\\').to_ascii_lowercase().starts_with("cake\\");
            let public = method.declared().is_some_and(|declared| declared.visibility == Visibility::Public);
            let name = method.name.as_str();
            if own
                && public
                && !method.is_static
                && !name.starts_with('_')
                && !NOT_ACTIONS.contains(&name.to_ascii_lowercase().as_str())
            {
                out.push(name.to_owned());
            }
        }
        out.sort();
        out
    }

    /// The table class the call at the site is on: a table, or the table of
    /// the entity a query finds.
    fn site_table(cx: &RefContext<'_>) -> Option<String> {
        let Some(CallTarget::Method { classes, models, .. }) = Self::place(cx) else { return None };
        let resolver = cx.engine.resolver();
        let table = classes.iter().find_map(|class| {
            let model = resolver.class_model(class)?;
            (model.is_subclass_of(classes::TABLE) && model.fqn() != classes::TABLE).then(|| model.fqn().to_owned())
        });
        table.or_else(|| models.iter().find_map(|entity| classes::table_of(&resolver, entity)))
    }

    /// The association the part of `value` around the byte `at` names
    /// (`Users` of `Comments.Users`), with the table declaring it; and the
    /// table whose associations that part is one of.
    fn association(
        cx: &RefContext<'_>,
        value: &str,
        at: usize,
    ) -> (Option<String>, Option<(String, super::facts::Association)>) {
        let project = Self::project(cx);
        let resolver = cx.engine.resolver();
        let Some(mut table) = Self::site_table(cx) else { return (None, None) };
        // `['Comments' => ['Users']]`: the keys above the string lead to its table.
        for key in cx.site.as_ref().map(|site| site.keys.clone()).unwrap_or_default() {
            for part in key.split('.') {
                let found =
                    classes::associations(&resolver, &table).into_iter().find(|found| found.loaded.alias() == part);
                let Some(next) =
                    found.and_then(|found| class_of(&resolver, &project, Kind::Table, found.loaded.target()))
                else {
                    return (None, None);
                };
                table = next;
            }
        }
        let mut start = 0;
        for part in value.split('.') {
            let end = start + part.len();
            let found = classes::associations(&resolver, &table).into_iter().find(|found| found.loaded.alias() == part);
            if at <= end {
                return (Some(table.clone()), found.map(|found| (table, found)));
            }
            // On to the table the association leads to.
            let Some(next) = found.and_then(|found| class_of(&resolver, &project, Kind::Table, found.loaded.target()))
            else {
                return (None, None);
            };
            table = next;
            start = end + 1;
        }
        (None, None)
    }

    /// The cell class and the method `Inbox::expanded` names (`display`
    /// when it names none).
    fn cell(cx: &RefContext<'_>, value: &str) -> Option<(String, String)> {
        let (name, method) = value.split_once("::").unwrap_or((value, "display"));
        let class = class_of(&cx.engine.resolver(), &Self::project(cx), Kind::Cell, name)?;
        Some((class, method.to_owned()))
    }

    /// The file a plugin is: its plugin class, else its `composer.json`.
    fn plugin_file(project: &CakeProject, name: &str) -> Option<PathBuf> {
        let plugin = project.plugin(name)?;
        let short = name.rsplit('/').next().unwrap_or(name);
        let files = [
            plugin.path.join("src").join(format!("{short}Plugin.php")),
            plugin.path.join("src").join("Plugin.php"),
            plugin.path.join("composer.json"),
        ];
        files.into_iter().find(|file| file.is_file())
    }

    /// The catalogue the message at the site is of: the first argument of
    /// `__d()` and its kin, else `default`.
    fn domain(cx: &RefContext<'_>) -> String {
        let with_domain = matches!(Self::place(cx), Some(CallTarget::Function(names))
            if names.iter().any(|name| name.rsplit(BACKSLASH).next().is_some_and(|short| short.starts_with("__d"))));
        let named = cx.site.as_ref().and_then(|site| site.first_argument.clone()).filter(|_| with_domain);
        named.unwrap_or_else(|| "default".to_owned())
    }

    /// The routes named `value`: by the whole name, else by its end (a
    /// scope's `_namePrefix` comes before a route's own name).
    fn routes<'p>(project: &'p CakeProject, value: &str) -> Vec<&'p Def> {
        let exact: Vec<&Def> = project.routes.iter().filter(|def| def.name == value).collect();
        if !exact.is_empty() || value.is_empty() {
            return exact;
        }
        project.routes.iter().filter(|def| value.ends_with(&def.name)).collect()
    }

    fn targets(&self, cx: &RefContext<'_>, value: &str) -> Vec<Target> {
        let project = Self::project(cx);
        let at = |def: &Def| Target { path: def.file.clone(), range: def.range };
        let whole = |path: PathBuf| Target { path, range: Range::default() };
        if let Some(kind) = self.class_kind() {
            let class = class_of(&cx.engine.resolver(), &project, kind, value);
            return class.map_or(Vec::new(), |class| cx.engine.class_targets(&class));
        }
        match self.source {
            Source::Template | Source::Layout | Source::Element => {
                let found = self.files(cx, value).into_iter().find(|file| file.is_file());
                found.into_iter().map(whole).collect()
            }
            Source::Cell => {
                let Some((class, method)) = Self::cell(cx, value) else { return Vec::new() };
                match method_target(cx, &class, &method) {
                    Some(target) => vec![target],
                    None => cx.engine.class_targets(&class),
                }
            }
            Source::Config => project.config.iter().filter(|def| def.name == value).map(at).collect(),
            Source::Controller => {
                let class = Self::controller_class(cx, value, &Self::siblings(cx));
                class.map_or(Vec::new(), |class| cx.engine.class_targets(&class))
            }
            Source::Action => {
                let Some((class, method)) = Self::action(cx, value) else { return Vec::new() };
                method_target(cx, &class, &method).into_iter().collect()
            }
            Source::Plugin => Self::plugin_file(&project, value).into_iter().map(whole).collect(),
            Source::Route => Self::routes(&project, value).into_iter().map(at).collect(),
            Source::Message => {
                let domain = Self::domain(cx);
                let found =
                    project.messages.iter().filter(|message| message.domain == domain && message.message == value);
                found.map(|message| Target { path: message.file.clone(), range: message.range }).collect()
            }
            Source::Domain => {
                let mut out: Vec<Target> = Vec::new();
                for message in project.messages.iter().filter(|message| message.domain == value) {
                    if !out.iter().any(|known| known.path == message.file) {
                        out.push(whole(message.file.clone()));
                    }
                }
                out
            }
            _ => Vec::new(),
        }
    }

    /// The names of the classes of `kind` the project has: the
    /// application's and CakePHP's as they are, a plugin's after its name.
    fn class_names(cx: &RefContext<'_>, project: &CakeProject, kind: Kind) -> Vec<(String, Option<String>)> {
        let resolver = cx.engine.resolver();
        let mut out = Vec::new();
        let mut add = |plugin: Option<&str>| {
            for namespace in classes::namespaces(project, kind, plugin) {
                for class in resolver.namespace_classes(&namespace) {
                    let short = class.rsplit('\\').next().unwrap_or(&class);
                    let Some(name) = short.strip_suffix(kind.suffix()).filter(|name| !name.is_empty()) else {
                        continue;
                    };
                    let name = match plugin {
                        Some(plugin) => format!("{plugin}.{name}"),
                        None => name.to_owned(),
                    };
                    out.push((name, Some(class.clone())));
                }
            }
        };
        add(None);
        for plugin in &project.plugins {
            add(Some(&plugin.name));
        }
        out
    }

    /// The names of this kind, each with a detail.
    fn names(&self, cx: &RefContext<'_>) -> Vec<(String, Option<String>)> {
        let project = Self::project(cx);
        let mut out: Vec<(String, Option<String>)> = match self.source {
            Source::Table | Source::Component | Source::Helper | Source::Behavior => {
                self.class_kind().map_or(Vec::new(), |kind| Self::class_names(cx, &project, kind))
            }
            Source::Cell => Self::class_names(cx, &project, Kind::Cell),
            Source::Controller => {
                let (plugin, prefix, _) = Self::own(cx);
                let namespace = project.namespace_of(plugin.as_deref());
                let folder = if prefix.is_empty() { String::new() } else { format!("\\{}", prefix.replace('/', "\\")) };
                let resolver = cx.engine.resolver();
                let mut found = resolver.namespace_classes(&format!("{namespace}\\Controller{folder}"));
                found.extend(resolver.namespace_classes(&format!("{namespace}\\Controller")));
                found
                    .into_iter()
                    .filter_map(|class| {
                        let short = class.rsplit('\\').next()?.strip_suffix("Controller")?.to_owned();
                        (!short.is_empty() && short != "App").then_some((short, Some(class)))
                    })
                    .collect()
            }
            Source::Action => {
                let beside = Self::siblings(cx);
                let class = match beside.get("controller") {
                    Some(name) => Self::controller_class(cx, name, &beside),
                    None => Self::action(cx, "index").map(|(class, _)| class).or_else(|| {
                        let (_, _, name) = Self::own(cx);
                        name.and_then(|name| Self::controller_class(cx, &name, &beside))
                    }),
                };
                class.map_or(Vec::new(), |class| {
                    Self::actions(cx, &class).into_iter().map(|name| (name, None)).collect()
                })
            }
            Source::Template | Source::Layout | Source::Element => {
                let Some(site) = Self::site_path(cx) else { return Vec::new() };
                let context = project.template_context(&site);
                let roots =
                    context.as_ref().map_or_else(|| project.template_roots(None), |context| context.roots.clone());
                let folder = match self.source {
                    Source::Layout => project.written("layout"),
                    Source::Element => project.written("element"),
                    _ => context.map(|context| context.folder).unwrap_or_default(),
                };
                let deep = self.source == Source::Element;
                let mut files = Vec::new();
                for root in roots {
                    let folder = tidy(&root.join(&folder));
                    template_names(
                        &folder,
                        &folder,
                        project.template_extension(),
                        if deep { 0 } else { 8 },
                        &mut files,
                    );
                }
                files.sort();
                files.into_iter().map(|name| (name, None)).collect()
            }
            Source::Config => project.config.iter().map(|def| (def.name.clone(), def.detail.clone())).collect(),
            Source::Association => Vec::new(),
            Source::Plugin => project.plugins.iter().map(|plugin| (plugin.name.clone(), None)).collect(),
            Source::Route => project.routes.iter().map(|def| (def.name.clone(), None)).collect(),
            Source::Message => {
                let domain = Self::domain(cx);
                let found = project.messages.iter().filter(|message| message.domain == domain);
                found.map(|message| (message.message.clone(), message.translation.clone())).collect()
            }
            Source::Domain => project.messages.iter().map(|message| (message.domain.clone(), None)).collect(),
        };
        let mut seen = std::collections::HashSet::new();
        out.retain(|(name, _)| seen.insert(name.clone()));
        out
    }
}

/// The files with the extension `extension` below `folder` as names relative
/// to it, without the extension, sorted; `depth` of 8 looks in this folder
/// only.
fn template_names(folder: &Path, below: &Path, extension: &str, depth: usize, out: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(below) else { return };
    let mut entries: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).collect();
    entries.sort();
    for path in entries {
        if out.len() >= 2000 {
            return;
        }
        if path.is_dir() {
            if depth < 8 {
                template_names(folder, &path, extension, depth + 1, out);
            }
        } else if path.extension().is_some_and(|ext| ext == extension)
            && let Ok(relative) = path.strip_prefix(folder)
        {
            out.push(relative.with_extension("").to_string_lossy().replace(BACKSLASH, "/"));
        }
    }
}

impl ReferenceKind for CakeName {
    fn id(&self) -> &'static str {
        self.id
    }

    fn patterns(&self) -> &'static [PlacePattern] {
        self.patterns
    }

    /// What every usage of one thing is: a class's name, a file from the
    /// project's root, an action as `<controller>::<method>`.
    fn canonical(&self, cx: &RefContext<'_>, value: &str) -> String {
        let root = cx.engine.root();
        let shown = |file: &Path| file.strip_prefix(&root).unwrap_or(file).to_string_lossy().replace('\\', "/");
        if let Some(kind) = self.class_kind() {
            let class = class_of(&cx.engine.resolver(), &Self::project(cx), kind, value);
            return class.unwrap_or_else(|| value.to_owned());
        }
        match self.source {
            Source::Template | Source::Layout | Source::Element => {
                self.file(cx, value).map_or_else(|| value.to_owned(), |file| shown(&file))
            }
            Source::Controller => {
                Self::controller_class(cx, value, &Self::siblings(cx)).unwrap_or_else(|| value.to_owned())
            }
            Source::Action => match Self::action(cx, value) {
                Some((class, method)) => format!("{class}::{method}"),
                None => value.to_owned(),
            },
            Source::Cell => match Self::cell(cx, value) {
                Some((class, method)) => format!("{class}::{method}"),
                None => value.to_owned(),
            },
            // A message is one of its catalogue.
            Source::Message => format!("{}{SEPARATOR}{value}", Self::domain(cx)),
            // The first association of a path is the one its usages share.
            Source::Association => match Self::association(cx, value, 0) {
                (_, Some((table, found))) => format!("{table}::{}", found.loaded.alias()),
                _ => value.to_owned(),
            },
            _ => value.to_owned(),
        }
    }

    /// A class is named by its name without what its kind adds, a file by
    /// the end of its name, an action by its method.
    fn usage_text<'v>(&self, value: &'v str) -> &'v str {
        if let Some(kind) = self.class_kind() {
            let short = value.rsplit('\\').next().unwrap_or(value);
            let short = short.rsplit('.').next().unwrap_or(short);
            return short.strip_suffix(kind.suffix()).filter(|name| !name.is_empty()).unwrap_or(short);
        }
        match self.source {
            Source::Template | Source::Layout | Source::Element => {
                let name = value.rsplit('/').next().unwrap_or(value);
                name.strip_suffix(".php").unwrap_or(name)
            }
            Source::Action | Source::Association => value.rsplit("::").next().unwrap_or(value),
            Source::Message => value.rsplit(SEPARATOR).next().unwrap_or(value),
            Source::Cell => {
                let class = value.split("::").next().unwrap_or(value);
                let short = class.rsplit('\\').next().unwrap_or(class);
                short.strip_suffix("Cell").filter(|name| !name.is_empty()).unwrap_or(short)
            }
            Source::Controller => {
                let short = value.rsplit('\\').next().unwrap_or(value);
                short.strip_suffix("Controller").filter(|name| !name.is_empty()).unwrap_or(short)
            }
            _ => value,
        }
    }

    fn usage_words(&self, value: &str) -> Vec<String> {
        crate::index::words(self.usage_text(value))
    }

    fn complete(&self, cx: &RefContext<'_>, value: &str, at: usize) -> Vec<RefItem> {
        // An association: those of the table the part before it leads to.
        if self.source == Source::Association {
            let at = at.min(value.len());
            let from = value[..at].rfind('.').map_or(0, |dot| dot + 1);
            let to = value[at..].find('.').map_or(value.len(), |dot| at + dot);
            let (Some(table), _) = Self::association(cx, value, at) else { return Vec::new() };
            let resolver = cx.engine.resolver();
            return classes::associations(&resolver, &table)
                .into_iter()
                .map(|found| RefItem {
                    label: found.loaded.alias().to_owned(),
                    insert: found.loaded.alias().to_owned(),
                    detail: Some(found.loaded.target().to_owned()),
                    from,
                    to,
                })
                .collect();
        }
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

    fn resolve(&self, cx: &RefContext<'_>, value: &str, at: usize) -> Vec<Target> {
        if self.source == Source::Association {
            let (_, found) = Self::association(cx, value, at);
            let target = found.map(|(_, found)| Target { path: found.loaded.file, range: found.loaded.range });
            return target.into_iter().collect();
        }
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
        let resolver = cx.engine.resolver();
        match self.source {
            // A name no class of the application, of its plugin or of CakePHP is of.
            Source::Component | Source::Helper | Source::Behavior => {
                let kind = self.class_kind()?;
                let plain = value.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '/' | '.'));
                let (plugin, _) = plugin_split(value);
                // A plugin Composer installed may not be indexed.
                let told = plugin.is_none_or(|plugin| project.plugin(plugin).is_some_and(|plugin| plugin.local));
                let known = resolver.class_model(kind.base()).is_some();
                (plain && told && known && class_of(&resolver, &project, kind, value).is_none())
                    .then(|| Undefined::warning(format!("Unknown {} '{value}'", self.label)))
            }
            // A file that isn't there, where the folder of templates is.
            Source::Template | Source::Layout | Source::Element => {
                let files = self.files(cx, value);
                files.first()?;
                let (plugin, _) = plugin_split(value);
                let told = plugin.is_none_or(|plugin| project.plugin(plugin).is_some())
                    && project.template_roots(None).first().is_some_and(|root| root.is_dir())
                    && (self.source != Source::Template
                        || files.iter().any(|file| file.parent().is_some_and(Path::is_dir)));
                // A theme, another view class or another folder of templates may have it.
                let set = Self::overrides(cx);
                let moved = Self::site_text(cx).is_some_and(|text| {
                    // (Read where they are set with a string; not known otherwise.)
                    ["setClassName", "viewClasses"].iter().any(|word| text.contains(word))
                        || (text.contains("setTheme") && set.theme.is_none())
                        || (text.contains("setTemplatePath") && set.template_path.is_none())
                        || (text.contains("setLayoutPath") && set.layout_path.is_none())
                });
                // A theme the project doesn't have the files of.
                let moved = moved || set.theme.as_deref().is_some_and(|theme| project.plugin(theme).is_none());
                // `$this->render()` outside a class is a view's only in a template.
                let template = matches!(Self::place(cx), Some(CallTarget::Method { classes, .. })
                    if classes.iter().all(|class| class == TEMPLATE_THIS));
                let folder = if project.legacy { "Template" } else { "templates" };
                let in_templates = site.components().any(|part| part.as_os_str() == folder);
                (told && !moved && (in_templates || !template) && !files.iter().any(|file| file.is_file()))
                    .then(|| Undefined::warning(format!("Unknown {} '{value}'", self.label)))
            }
            // An action its controller doesn't have.
            Source::Action => {
                if matches!(Self::place(cx), Some(CallTarget::Entry { .. })) {
                    return None;
                }
                let (class, method) = Self::action(cx, value)?;
                let model = resolver.class_model(&class)?;
                let dynamic = model.method("__call").is_some();
                (!dynamic && model.method(&method).is_none()).then(|| {
                    let short = class.rsplit('\\').next().unwrap_or(&class);
                    Undefined::warning(format!("Controller '{short}' has no action '{method}'"))
                })
            }
            _ => None,
        }
    }

    fn hover(&self, cx: &RefContext<'_>, value: &str) -> Option<String> {
        let project = Self::project(cx);
        if self.source == Source::Association {
            let (_, found) = Self::association(cx, value, value.len());
            let (table, found) = found?;
            let short = table.rsplit(BACKSLASH).next().unwrap_or(&table).to_owned();
            return Some(format!(
                "**association** `{}`\n\n`{short}` to `{}`",
                found.loaded.alias(),
                found.loaded.target()
            ));
        }
        let target = self.targets(cx, value).into_iter().next()?;
        let root = cx.engine.root();
        let file = target.path.strip_prefix(&root).unwrap_or(&target.path).to_string_lossy().replace('\\', "/");
        let place =
            if target.range == Range::default() { file } else { format!("{file}:{}", target.range.start.line + 1) };
        let detail = match self.source {
            Source::Table | Source::Component | Source::Helper | Source::Behavior => {
                self.class_kind().and_then(|kind| class_of(&cx.engine.resolver(), &project, kind, value))
            }
            Source::Config => {
                project.config.iter().rev().find(|def| def.name == value).and_then(|def| def.detail.clone())
            }
            Source::Controller => Self::controller_class(cx, value, &Self::siblings(cx)),
            Source::Action => Self::action(cx, value)
                .map(|(class, method)| format!("{}::{method}()", class.rsplit('\\').next().unwrap_or(&class))),
            Source::Cell => Self::cell(cx, value).map(|(class, method)| format!("{class}::{method}()")),
            Source::Message => {
                let domain = Self::domain(cx);
                let translations: Vec<String> = project
                    .messages
                    .iter()
                    .filter(|message| message.domain == domain && message.message == value)
                    .filter_map(|message| Some(format!("{}: {}", message.language, message.translation.as_deref()?)))
                    .collect();
                (!translations.is_empty()).then(|| translations.join("`\n\n`"))
            }
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
