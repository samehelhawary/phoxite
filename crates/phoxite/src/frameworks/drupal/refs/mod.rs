//! Drupal's reference kinds ([`kinds`]), on the machinery every framework
//! shares ([`crate::frameworks::refs`]).
//!
//! | kind | declared by | named in |
//! | --- | --- | --- |
//! | `drupalService` | a key of `services:` in a `*.services.yml` | `\Drupal::service()`, `$container->get()`; `@id`, `alias:` and `parent:` in those files |
//! | `drupalRoute` | a key of a `*.routing.yml` | `Url::fromRoute()`, `Link::createFromRoute()`, `redirect()`, `setRedirect()`; `route_name:` and `base_route:` of the `*.links.*.yml` files |
//! | `drupalPermission` | a key of a `*.permissions.yml` | `hasPermission()`, `AccessResult::allowedIfHasPermission()`; `_permission:` of the routing files |
//! | `drupalLibrary` | a key of a `*.libraries.yml` (`<extension>/<key>`) | `'#attached' => ['library' => […]]`; the lists of the info and libraries files; `attach_library()` in a template |
//! | `drupalConfig` | a file of `config/install` or `config/optional` | `\Drupal::config()`, `$this->config()`, the config factory's `get()` and `getEditable()` |
//! | `drupalConfigKey` ([`config_key`]) | a key of that file | `->get('…')` and `->set('…')` on the configuration |
//! | `drupalModule` | a `<name>.info.yml` | `moduleExists()`, `getModule()` |
//! | `drupalEntityType` | the `id` of an entity type's attribute or annotation | `getStorage()`, `getDefinition()` and the other handlers of the entity type manager, `\Drupal::entityQuery()` |
//! | `drupalPlugin` | the `id` of a plugin's attribute or annotation; a key of a `*.links.*.yml` or `*.layouts.yml` | `createInstance()`, `getDefinition()`, `hasDefinition()` of a plugin manager |
//! | `drupalThemeHook` | a key of what `hook_theme()` returns | `'#theme' => '…'`, a suggestion too (`node__article`); its preprocess functions are usages |
//! | `drupalHook` ([`hook`]) | a function `hook_…` of an `*.api.php` | `invokeAll()`, `alter()`, `#[Hook('…')]` |
//!
//! Drupal 7 has the same hooks, theme hooks, permissions and modules under
//! other functions: `module_invoke_all()`, `drupal_alter()`, `theme()`,
//! `user_access()` (the keys of `hook_permission()`), `module_exists()` (a
//! `<name>.info`). Its other names (menu paths, variables) aren't read.
//!
//! The first four are renamed: the key, the PHP strings and what the YAML
//! files write, when the project declares the name itself (not Drupal's
//! core, a contributed module or a library).
//!
//! None has an "unknown" inspection: a module opened on its own doesn't
//! have the core's files, so a name nothing declares may well exist.

pub mod column;
pub mod config_key;
pub mod hook;

pub use column::Column;
pub use config_key::ConfigKey;
pub use hook::DrupalHook;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::base::{Pos, Range};
use crate::frameworks::drupal::project::Def;
use crate::frameworks::drupal::{DrupalProject, FileKind};
use crate::frameworks::refs::{
    CallTarget, CalleePattern, PlacePattern, Position, RefContext, RefItem, ReferenceKind, RenameTarget,
};
use crate::navigate::Target;
use crate::summary::DeclaredKind;

/// What a [`DrupalName`] is the name of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Source {
    Service,
    Route,
    Permission,
    Library,
    Config,
    Module,
    EntityType,
    Plugin,
    ThemeHook,
    /// Drupal 7: a path of `hook_menu()`.
    MenuPath,
    /// Drupal 7: a variable `variable_set()` sets.
    Variable,
    /// A table `hook_schema()` describes.
    Table,
}

/// How a YAML file writes a name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Yaml {
    /// Right after this text: `@mailer`.
    Prefixed(&'static str),
    /// The value of this key, on its line: `route_name: shop.books`.
    Value(&'static str),
    /// Among what the value of this key lists on its line, between `+` and
    /// `,`: `_permission: 'administer shop+access content'`.
    Among(&'static str),
    /// An item of a list: `- shop/cart`.
    Item,
}

/// One of the kinds of the module documentation but the last.
pub struct DrupalName {
    id: &'static str,
    /// What a name is called in a hover.
    label: &'static str,
    source: Source,
    patterns: &'static [PlacePattern],
    /// Where the YAML files name one.
    yaml: &'static [(FileKind, Yaml)],
    renamed: bool,
}

const fn method(
    classes: &'static [&'static str],
    names: &'static [&'static str],
    index: usize,
    name: &'static str,
) -> PlacePattern {
    PlacePattern::argument(CalleePattern::Method { classes, names }, index, name)
}

const DRUPAL: &[&str] = &["Drupal"];
const CONTAINER: &[&str] =
    &["Psr\\Container\\ContainerInterface", "Symfony\\Component\\DependencyInjection\\ContainerInterface"];
const ENTITY_TYPE_MANAGER: &[&str] = &["Drupal\\Core\\Entity\\EntityTypeManagerInterface"];
const PLUGIN_MANAGER: &[&str] = &[
    "Drupal\\Component\\Plugin\\PluginManagerInterface",
    "Drupal\\Component\\Plugin\\Factory\\FactoryInterface",
    "Drupal\\Component\\Plugin\\Discovery\\DiscoveryInterface",
];

pub static SERVICE: DrupalName = DrupalName {
    id: "drupalService",
    label: "service",
    source: Source::Service,
    patterns: &[method(DRUPAL, &["service", "hasService"], 0, "id"), method(CONTAINER, &["get", "has"], 0, "id")],
    yaml: &[
        (FileKind::Services, Yaml::Prefixed("@")),
        (FileKind::Services, Yaml::Prefixed("@?")),
        (FileKind::Services, Yaml::Value("alias:")),
        (FileKind::Services, Yaml::Value("parent:")),
    ],
    renamed: true,
};

pub static ROUTE: DrupalName = DrupalName {
    id: "drupalRoute",
    label: "route",
    source: Source::Route,
    patterns: &[
        method(&["Drupal\\Core\\Url"], &["fromRoute", "__construct"], 0, "route_name"),
        method(&["Drupal\\Core\\Link"], &["createFromRoute"], 1, "route_name"),
        method(
            &["Drupal\\Core\\Controller\\ControllerBase", "Drupal\\Core\\Form\\FormBase"],
            &["redirect"],
            0,
            "route_name",
        ),
        method(&["Drupal\\Core\\Form\\FormStateInterface"], &["setRedirect"], 0, "route_name"),
        method(
            &["Drupal\\Core\\Routing\\UrlGeneratorInterface"],
            &["generateFromRoute", "getPathFromRoute"],
            0,
            "name",
        ),
    ],
    yaml: &[(FileKind::Links, Yaml::Value("route_name:")), (FileKind::Links, Yaml::Value("base_route:"))],
    renamed: true,
};

pub static PERMISSION: DrupalName = DrupalName {
    id: "drupalPermission",
    label: "permission",
    source: Source::Permission,
    patterns: &[
        method(
            &["Drupal\\Core\\Session\\AccountInterface", "Drupal\\user\\RoleInterface"],
            &["hasPermission", "grantPermission", "revokePermission"],
            0,
            "permission",
        ),
        method(&["Drupal\\Core\\Access\\AccessResult"], &["allowedIfHasPermission"], 1, "permission"),
        method(&["Drupal\\Core\\Access\\AccessResult"], &["allowedIfHasPermissions"], 1, "permissions")
            .at(Position::ListValue),
        PlacePattern::argument(CalleePattern::Function("user_role_grant_permissions"), 1, "permissions")
            .at(Position::ListValue),
        // Drupal 7.
        PlacePattern::argument(CalleePattern::Function("user_access"), 0, "string"),
    ],
    yaml: &[(FileKind::Routing, Yaml::Among("_permission:"))],
    renamed: true,
};

pub static LIBRARY: DrupalName = DrupalName {
    id: "drupalLibrary",
    label: "library",
    source: Source::Library,
    patterns: &[
        PlacePattern::argument(CalleePattern::Entry("#attached"), 0, "attached").at(Position::ListOfKey("library"))
    ],
    yaml: &[(FileKind::Info, Yaml::Item), (FileKind::Libraries, Yaml::Item)],
    renamed: true,
};

pub static CONFIG: DrupalName = DrupalName {
    id: "drupalConfig",
    label: "configuration",
    source: Source::Config,
    patterns: &[
        method(DRUPAL, &["config"], 0, "name"),
        method(&["Drupal\\Core\\Config\\ConfigFactoryInterface"], &["get", "getEditable"], 0, "name"),
        method(
            &["Drupal\\Core\\Form\\ConfigFormBaseTrait", "Drupal\\Core\\Controller\\ControllerBase"],
            &["config"],
            0,
            "name",
        ),
    ],
    yaml: &[],
    renamed: false,
};

pub static MODULE: DrupalName = DrupalName {
    id: "drupalModule",
    label: "module",
    source: Source::Module,
    patterns: &[
        method(&["Drupal\\Core\\Extension\\ModuleHandlerInterface"], &["moduleExists", "getModule"], 0, "module"),
        // Drupal 7.
        PlacePattern::argument(CalleePattern::Function("module_exists"), 0, "module"),
    ],
    yaml: &[],
    renamed: false,
};

pub static ENTITY_TYPE: DrupalName = DrupalName {
    id: "drupalEntityType",
    label: "entity type",
    source: Source::EntityType,
    patterns: &[
        method(
            ENTITY_TYPE_MANAGER,
            &[
                "getStorage",
                "getDefinition",
                "hasDefinition",
                "getViewBuilder",
                "getListBuilder",
                "getFormObject",
                "getAccessControlHandler",
                "hasHandler",
            ],
            0,
            "entity_type_id",
        ),
        method(DRUPAL, &["entityQuery", "entityQueryAggregate"], 0, "entity_type"),
    ],
    yaml: &[],
    renamed: false,
};

pub static PLUGIN: DrupalName = DrupalName {
    id: "drupalPlugin",
    label: "plugin",
    source: Source::Plugin,
    patterns: &[method(PLUGIN_MANAGER, &["createInstance", "getDefinition", "hasDefinition"], 0, "plugin_id")],
    // A menu link's parent, a local task's.
    yaml: &[(FileKind::Links, Yaml::Value("parent:")), (FileKind::Links, Yaml::Value("parent_id:"))],
    renamed: false,
};

pub static THEME_HOOK: DrupalName = DrupalName {
    id: "drupalThemeHook",
    label: "theme hook",
    source: Source::ThemeHook,
    patterns: &[
        PlacePattern::argument(CalleePattern::Entry("#theme"), 0, "theme"),
        // Drupal 7.
        PlacePattern::argument(CalleePattern::Function("theme"), 0, "hook"),
    ],
    yaml: &[],
    renamed: false,
};

const fn function(name: &'static str, index: usize, parameter: &'static str) -> PlacePattern {
    PlacePattern::argument(CalleePattern::Function(name), index, parameter)
}

pub static MENU_PATH: DrupalName = DrupalName {
    id: "drupalMenuPath",
    label: "menu path",
    source: Source::MenuPath,
    patterns: &[
        function("drupal_goto", 0, "path"),
        function("url", 0, "path"),
        function("l", 1, "path"),
        function("menu_get_item", 0, "path"),
        function("drupal_get_path_alias", 0, "path"),
    ],
    yaml: &[],
    renamed: false,
};

pub static VARIABLE: DrupalName = DrupalName {
    id: "drupalVariable",
    label: "variable",
    source: Source::Variable,
    patterns: &[
        function("variable_get", 0, "name"),
        function("variable_set", 0, "name"),
        function("variable_del", 0, "name"),
    ],
    yaml: &[],
    renamed: false,
};

const CONNECTION: &[&str] = &["Drupal\\Core\\Database\\Connection", "DatabaseConnection"];
const SELECT: &[&str] = &["Drupal\\Core\\Database\\Query\\SelectInterface", "SelectQueryInterface"];

pub static TABLE: DrupalName = DrupalName {
    id: "drupalTable",
    label: "table",
    source: Source::Table,
    patterns: &[
        method(CONNECTION, &["select", "insert", "update", "delete", "merge", "truncate", "upsert"], 0, "table"),
        method(SELECT, &["join", "innerJoin", "leftJoin", "rightJoin", "addJoin"], 0, "table"),
        // Drupal 7's functions.
        function("db_select", 0, "table"),
        function("db_insert", 0, "table"),
        function("db_update", 0, "table"),
        function("db_delete", 0, "table"),
        function("db_merge", 0, "table"),
        function("db_truncate", 0, "table"),
        function("db_table_exists", 0, "table"),
        function("db_field_exists", 0, "table"),
        function("drupal_write_record", 0, "table"),
        function("drupal_get_schema", 0, "table"),
    ],
    yaml: &[],
    renamed: false,
};

/// Drupal's kinds. An entity type manager is a plugin manager too: its
/// `getDefinition('node')` is an entity type's, so that kind is asked first.
pub fn kinds() -> &'static [&'static dyn ReferenceKind] {
    static KINDS: [&dyn ReferenceKind; 15] = [
        &SERVICE,
        &ROUTE,
        &PERMISSION,
        &LIBRARY,
        &CONFIG,
        &ConfigKey,
        &MODULE,
        &ENTITY_TYPE,
        &PLUGIN,
        &THEME_HOOK,
        &MENU_PATH,
        &VARIABLE,
        &TABLE,
        &Column,
        &DrupalHook,
    ];
    &KINDS
}

/// Whether the file at `path` is Drupal's or a contributed project's, not
/// the site's own: below a `core` or a `contrib` folder.
pub(super) fn foreign(path: &Path) -> bool {
    path.components().any(|part| matches!(part.as_os_str().to_str(), Some("core" | "contrib")))
}

fn word(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '\\' | '-' | '/')
}

/// Where the text of a YAML file writes `name` as `how`: the line, and the
/// columns of the name.
fn written(text: &str, name: &str, how: Yaml) -> Vec<Range> {
    let mut out = Vec::new();
    if name.is_empty() || !text.contains(name) {
        return out;
    }
    for (number, line) in text.lines().enumerate() {
        let mut from = 0;
        while let Some(found) = line[from..].find(name) {
            let start = from + found;
            let end = start + name.len();
            from = end;
            let before = &line[..start];
            let after = &line[end..];
            let bare = before.trim_end_matches(['\'', '"']).trim_end();
            let ends = !after.starts_with(word);
            let found = match how {
                Yaml::Prefixed(open) => before.ends_with(open) && !before.ends_with("@@") && ends,
                Yaml::Value(key) => bare.trim_start() == key && ends,
                Yaml::Among(key) => {
                    let listed = |c: char| matches!(c, '+' | ',' | '\'' | '"') || c.is_whitespace();
                    before.trim_start().starts_with(key)
                        && before.ends_with(listed)
                        && (after.is_empty() || after.starts_with(listed))
                }
                Yaml::Item => bare.trim_start() == "-" && ends,
            };
            if found {
                let column = |byte: usize| line[..byte].encode_utf16().count() as u32;
                out.push(Range::new(Pos::new(number as u32, column(start)), Pos::new(number as u32, column(end))));
            }
        }
    }
    out
}

/// Where a template's text gives `library` to `attach_library()`.
fn attached(text: &str, library: &str) -> Vec<Range> {
    let mut out = Vec::new();
    for (number, line) in text.lines().enumerate() {
        let mut from = 0;
        while let Some(found) = line[from..].find(library) {
            let start = from + found;
            let end = start + library.len();
            from = end;
            let before = line[..start].trim_end_matches(['\'', '"']).trim_end();
            let quoted = line[..start].ends_with(['\'', '"']) && line[end..].starts_with(['\'', '"']);
            if quoted && before.ends_with("attach_library(") {
                let column = |byte: usize| line[..byte].encode_utf16().count() as u32;
                out.push(Range::new(Pos::new(number as u32, column(start)), Pos::new(number as u32, column(end))));
            }
        }
    }
    out
}

impl DrupalName {
    fn project(cx: &RefContext<'_>) -> Arc<DrupalProject> {
        cx.engine.drupal()
    }

    /// Everything of this kind the project declares.
    fn defs(&self, cx: &RefContext<'_>) -> Vec<Def> {
        let project = Self::project(cx);
        let declared = |kind: DeclaredKind, detail: usize| {
            let names = cx.engine.names();
            // Not what a test declares: a test module's entity types aren't the site's.
            // Nor what an `*.api.php` file writes as an example of a hook.
            let tested = |path: &Path| {
                path.components().any(|part| part.as_os_str() == "tests")
                    || path.file_name().and_then(|name| name.to_str()).is_some_and(|name| name.ends_with(".api.php"))
            };
            names
                .of(&[kind])
                .filter(|declared| !tested(&declared.path))
                .map(|declared| Def {
                    name: declared.name.clone(),
                    detail: declared.classes.get(detail).filter(|class| !class.is_empty()).cloned(),
                    file: declared.path.clone(),
                    range: declared.range,
                })
                .collect::<Vec<Def>>()
        };
        match self.source {
            Source::Service => project
                .services
                .services
                .iter()
                .map(|service| Def {
                    name: service.id.clone(),
                    detail: project.services.service_class(&service.id),
                    file: service.file.clone(),
                    range: service.range,
                })
                .collect(),
            Source::Route => project.routes.clone(),
            Source::Permission => {
                // Drupal 7: the keys of what `hook_permission()` returns.
                let mut permissions = project.permissions.clone();
                permissions.extend(declared(DeclaredKind::DrupalPermission, usize::MAX));
                permissions
            }
            Source::Library => project.libraries.clone(),
            Source::Config => {
                // The files installing one, then those only a schema describes.
                let mut configs = project.configs.clone();
                for schema in &project.schemas {
                    let name = &schema.name.name;
                    if !name.contains('*') && !configs.iter().any(|known| known.name == *name) {
                        configs.push(schema.name.clone());
                    }
                }
                configs
            }
            Source::Module => project
                .extensions
                .iter()
                .map(|extension| Def {
                    name: extension.name.clone(),
                    detail: Some(extension.kind.clone()),
                    file: extension.file.clone(),
                    range: Range::default(),
                })
                .collect(),
            Source::EntityType => declared(DeclaredKind::DrupalEntityType, 0),
            // A plugin's detail is the attribute's name: the plugin's type.
            Source::Plugin => {
                let mut plugins = declared(DeclaredKind::DrupalPlugin, 1);
                plugins.extend(project.plugins.iter().cloned());
                plugins
            }
            Source::ThemeHook => declared(DeclaredKind::DrupalThemeHook, usize::MAX),
            Source::MenuPath => declared(DeclaredKind::DrupalMenuPath, usize::MAX),
            Source::Variable => declared(DeclaredKind::DrupalVariable, usize::MAX),
            Source::Table => declared(DeclaredKind::DrupalTable, usize::MAX),
        }
    }

    /// Those of [`defs`](Self::defs) the string's place can name: for a
    /// plugin, the ones of the manager's type (`Block` for a
    /// `BlockManagerInterface`) when its name says it.
    fn offered(&self, cx: &RefContext<'_>) -> Vec<Def> {
        let defs = self.defs(cx);
        if self.source != Source::Plugin {
            return defs;
        }
        let Some((CallTarget::Method { classes, .. }, _)) = cx.site.as_ref().and_then(|site| site.call.as_ref()) else {
            return defs;
        };
        let managed = |def: &Def| {
            let Some(kind) = &def.detail else { return false };
            classes.iter().any(|class| {
                let short = class.rsplit('\\').next().unwrap_or(class).to_ascii_lowercase();
                short.starts_with(&kind.to_ascii_lowercase())
            })
        };
        let typed: Vec<Def> = defs.iter().filter(|def| managed(def)).cloned().collect();
        if typed.is_empty() { defs } else { typed }
    }

    /// What `value` names: the one of that name, else what it is made
    /// from. A theme hook suggestion (`node__article`) is its base hook's
    /// (`node`), a derivative plugin (`system_menu_block:main`) its base
    /// plugin's.
    fn named(&self, cx: &RefContext<'_>, value: &str) -> Vec<Def> {
        let offered = self.offered(cx);
        let exact = |name: &str| offered.iter().filter(|def| def.name == name).cloned().collect::<Vec<Def>>();
        let found = exact(value);
        if !found.is_empty() {
            return found;
        }
        match self.source {
            Source::ThemeHook => {
                let mut base = value;
                while let Some((shorter, _)) = base.rsplit_once("__") {
                    let found = exact(shorter);
                    if !found.is_empty() {
                        return found;
                    }
                    base = shorter;
                }
                Vec::new()
            }
            Source::Plugin => value.split_once(':').map(|(base, _)| exact(base)).unwrap_or_default(),
            // `node/%node/edit` is the path of `node/12/edit`.
            Source::MenuPath => {
                let fits = |def: &&Def| {
                    let parts: Vec<&str> = def.name.split('/').collect();
                    let written: Vec<&str> = value.trim_matches('/').split('/').collect();
                    parts.len() == written.len()
                        && parts.iter().any(|part| part.starts_with('%'))
                        && parts.iter().zip(&written).all(|(part, written)| part.starts_with('%') || part == written)
                };
                offered.iter().filter(fits).cloned().collect()
            }
            _ => Vec::new(),
        }
    }

    /// The template of the theme hook `name` that `def` declares:
    /// `<name>.html.twig` with dashes, below the `templates` folder of the
    /// extension declaring it.
    fn template(cx: &RefContext<'_>, def: &Def, name: &str) -> Option<PathBuf> {
        let project = Self::project(cx);
        let folder = project.extension_of(&def.file)?.dir.join("templates");
        let wanted = format!("{}.html.twig", name.replace('_', "-"));
        fn find(folder: &Path, wanted: &str, depth: usize) -> Option<PathBuf> {
            let mut entries: Vec<PathBuf> =
                std::fs::read_dir(folder).ok()?.flatten().map(|entry| entry.path()).collect();
            entries.sort();
            entries.iter().find(|path| path.file_name().is_some_and(|name| name == wanted)).cloned().or_else(|| {
                entries.iter().filter(|path| depth < 4 && path.is_dir()).find_map(|path| find(path, wanted, depth + 1))
            })
        }
        find(&folder, &wanted, 0)
    }
}

impl ReferenceKind for DrupalName {
    fn id(&self) -> &'static str {
        self.id
    }

    fn patterns(&self) -> &'static [PlacePattern] {
        self.patterns
    }

    fn complete(&self, cx: &RefContext<'_>, value: &str, _at: usize) -> Vec<RefItem> {
        let mut seen = std::collections::HashSet::new();
        self.offered(cx)
            .into_iter()
            .filter(|def| seen.insert(def.name.clone()))
            .map(|def| RefItem {
                label: def.name.clone(),
                insert: def.name,
                detail: def.detail.or_else(|| Some(self.label.to_owned())),
                from: 0,
                to: value.len(),
            })
            .collect()
    }

    fn resolve(&self, cx: &RefContext<'_>, value: &str, _at: usize) -> Vec<Target> {
        let mut out: Vec<Target> = Vec::new();
        for def in self.named(cx, value) {
            let mut targets = vec![Target { path: def.file.clone(), range: def.range }];
            if self.source == Source::ThemeHook {
                // A suggestion's own template before its base hook's.
                let template = Self::template(cx, &def, value).or_else(|| Self::template(cx, &def, &def.name));
                targets.extend(template.map(|path| Target { path, range: Range::default() }));
                // Drupal 7: the function rendering it, `theme_<hook>()`.
                targets.extend(cx.engine.declarations_of(
                    crate::names::NameKind::Function,
                    &format!("theme_{}", def.name),
                    Path::new(""),
                    &crate::summary::FileSummary::default(),
                ));
            }
            for target in targets {
                if !out.contains(&target) {
                    out.push(target);
                }
            }
        }
        out
    }

    fn other_usages(&self, cx: &RefContext<'_>, key: &str) -> Vec<(PathBuf, Range)> {
        let project = Self::project(cx);
        let mut out: Vec<(PathBuf, Range)> = Vec::new();
        let mut read: Vec<(&PathBuf, String)> = Vec::new();
        for (kind, how) in self.yaml {
            for file in project.files_of(*kind) {
                if !read.iter().any(|(known, _)| *known == file) {
                    let Ok(text) = std::fs::read_to_string(file) else { continue };
                    read.push((file, text));
                }
                let Some((_, text)) = read.iter().find(|(known, _)| *known == file) else { continue };
                for range in written(text, key, *how) {
                    if !out.contains(&(file.clone(), range)) {
                        out.push((file.clone(), range));
                    }
                }
            }
        }
        // The functions preparing a theme hook's variables: `<extension>_preprocess_<hook>()`.
        if self.source == Source::ThemeHook {
            let extensions = project.extensions.iter().map(|extension| extension.name.as_str());
            for prefix in extensions.chain(std::iter::once("template")) {
                let function = format!("{prefix}_preprocess_{key}");
                let found = cx.engine.declarations_of(
                    crate::names::NameKind::Function,
                    &function,
                    Path::new(""),
                    &crate::summary::FileSummary::default(),
                );
                out.extend(found.into_iter().map(|target| (target.path, target.range)));
            }
        }
        // A template attaches a library: `{{ attach_library('shop/cart') }}`.
        if self.source == Source::Library {
            let paths = cx.engine.twig_paths();
            for template in paths.names() {
                let Some(file) = paths.resolve(&template) else { continue };
                let Some(text) = cx.engine.template_text(&file).filter(|text| text.contains(key)) else { continue };
                for range in attached(&text, key) {
                    out.push((file.clone(), range));
                }
            }
        }
        out
    }

    fn rename_target(&self, cx: &RefContext<'_>, value: &str, _at: usize) -> Option<RenameTarget> {
        if !self.renamed || (self.source == Source::Service && crate::symfony::is_class_id(value)) {
            return None;
        }
        let defs = self.named(cx, value);
        if defs.is_empty() || defs.iter().any(|def| foreign(&def.file) || cx.engine.is_library(&def.file)) {
            return None;
        }
        // A library's key is written after its extension's name.
        let from = match self.source {
            Source::Library => value.find('/')? + 1,
            _ => 0,
        };
        let declarations = defs.into_iter().map(|def| (def.file, def.range)).collect();
        Some(RenameTarget { key: value.to_owned(), range: (from, value.len()), declarations })
    }

    fn valid_name(&self, name: &str) -> bool {
        let allowed = |c: char| match self.source {
            Source::Permission => c.is_ascii_alphanumeric() || matches!(c, ' ' | '_' | '-' | '.'),
            Source::Library => c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'),
            _ => c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | ':'),
        };
        !name.is_empty() && name.chars().all(allowed) && name.trim() == name
    }

    fn is_known(&self, cx: &RefContext<'_>, value: &str) -> bool {
        let defs = self.defs(cx);
        defs.is_empty() || defs.iter().any(|def| def.name == value)
    }

    fn hover(&self, cx: &RefContext<'_>, value: &str) -> Option<String> {
        let defs = self.named(cx, value);
        let first = defs.first()?;
        let root = cx.engine.root();
        let file = first.file.strip_prefix(&root).unwrap_or(&first.file).to_string_lossy().replace('\\', "/");
        let place =
            if first.range == Range::default() { file } else { format!("{file}:{}", first.range.start.line + 1) };
        let mut out = format!("**{}** `{value}`", self.label);
        if let Some(detail) = &first.detail {
            out.push_str(&format!("\n\n`{detail}`"));
        }
        out.push_str(&format!("\n\n{place}"));
        Some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::{Yaml, written};

    fn columns(text: &str, name: &str, how: Yaml) -> Vec<(u32, u32)> {
        written(text, name, how).into_iter().map(|range| (range.start.line, range.start.col)).collect()
    }

    #[test]
    fn names_are_found_where_a_yaml_file_writes_them() {
        let services = "services:\n  shop.cart:\n    arguments: ['@shop.catalog', '@?shop.catalog', '@shop.catalog_old', '@@shop.catalog']\n  shop.other:\n    parent: shop.catalog\n";
        assert_eq!(columns(services, "shop.catalog", Yaml::Prefixed("@")), [(2, 18)]);
        assert_eq!(columns(services, "shop.catalog", Yaml::Prefixed("@?")), [(2, 36)]);
        assert_eq!(columns(services, "shop.catalog", Yaml::Value("parent:")), [(4, 12)]);

        let links = "shop.books_link:\n  title: 'Books'\n  route_name: shop.books\n  base_route: 'shop.books'\n  description: shop.books\n";
        assert_eq!(columns(links, "shop.books", Yaml::Value("route_name:")), [(2, 14)]);
        assert_eq!(columns(links, "shop.books", Yaml::Value("base_route:")), [(3, 15)]);

        let routing = "shop.books:\n  requirements:\n    _permission: 'administer shop+access content'\n    _role: 'administer shop'\n";
        assert_eq!(columns(routing, "administer shop", Yaml::Among("_permission:")), [(2, 18)]);
        assert_eq!(columns(routing, "access content", Yaml::Among("_permission:")), [(2, 34)]);

        let info = "name: Shop\nlibraries:\n  - shop/cart\n  - 'shop/cart-old'\ndescription: shop/cart\n";
        assert_eq!(columns(info, "shop/cart", Yaml::Item), [(2, 4)]);
    }
}
