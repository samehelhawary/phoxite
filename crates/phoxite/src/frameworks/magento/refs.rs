//! Magento's reference kinds ([`kinds`]), on the machinery every framework
//! shares ([`crate::frameworks::refs`]). All are one type, [`MagentoName`].
//!
//! | kind | names | where |
//! | --- | --- | --- |
//! | `magentoEvent` | an event, where an `events.xml` observes it | `dispatch('name')` of the event manager |
//! | `magentoConfigPath` | a configuration path, where `system.xml` has its field or `config.xml` its default | `getValue('a/b/c')`, `isSetFlag()` of the scope configuration |
//! | `magentoAcl` | an ACL resource of an `acl.xml` | `isAllowed('Vendor_Module::res')`, `const ADMIN_RESOURCE` of a backend action |
//! | `magentoTemplate` | a template's file, a theme's version first (a theme before its parents) | `setTemplate('Vendor_Module::x.phtml')`, a block's `$_template` |
//! | `magentoHandle` | a layout handle: its layout files | `addHandle('name')` |
//! | `magentoBlock` | a block or a container of the layout, by its name or the alias its parent has it under | `getBlock('name')`, `getChildBlock('alias')`, `getChildHtml('alias')` |
//! | `magentoModule` | a module, where it is registered | `isEnabled('Vendor_Module')`, `getDir()`, `getModuleDir()`, the name a `registration.php` registers |
//! | `magentoRoute` | a URL path `front/controller/action`: the action's class, by the modules `routes.xml` gives the front name; the front name itself is its route | `getUrl('wishlist/index/add')`, `_redirect()`, `setPath()` |
//! | `magentoTable` | a table of a `db_schema.xml` | `getTableName('wishlist')`, `getTable()`, a resource model's `_init('wishlist', 'wishlist_id')` |
//! | `magentoColumn` | a column of the table named before it | the second argument of `_init()` |
//!
//! Each has completion, goto (into the XML file, at the declaration), hover
//! and Find References: the XML that declares or names it is listed with
//! the PHP usages ([`MagentoProject::mentions`]).
//!
//! **Renamed:** an event, a block's name, an ACL resource (the part after
//! `Vendor_Module::`) and a template (its file moves, in the module and in
//! the themes), in PHP and in every XML file naming it. Not a
//! configuration path (its parts are separate elements of `system.xml` and
//! `config.xml`), a module (its folder, namespace and every id), a route
//! or a table.
//!
//! A template whose file isn't there, an ACL resource and a module the
//! project doesn't have are reported ([`ReferenceKind::undefined`]), only
//! where the project has what would declare them: the modules of a vendor
//! that isn't installed are not known to be missing. An event, a
//! configuration path, a route and a table are never reported: all are
//! used without being declared where the engine reads.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::project::{Def, MagentoProject, What, area_of};
use crate::base::{FileEdit, Pos, Range};
use crate::frameworks::refs::{
    CalleePattern, PlacePattern, RefContext, RefItem, ReferenceKind, RenameTarget, Undefined,
};
use crate::navigate::Target;
use crate::syntax::LineIndex;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Source {
    Event,
    Config,
    Acl,
    Template,
    Handle,
    Block,
    Module,
    Route,
    Table,
    Column,
}

/// One of the kinds of the module documentation.
pub struct MagentoName {
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

const fn property(classes: &'static [&'static str], names: &'static [&'static str]) -> PlacePattern {
    PlacePattern::argument(CalleePattern::Property { classes, names }, 0, "value")
}

const EVENTS: &[&str] = &["Magento\\Framework\\Event\\ManagerInterface"];
const SCOPE_CONFIG: &[&str] = &["Magento\\Framework\\App\\Config\\ScopeConfigInterface"];
const AUTHORIZATION: &[&str] = &["Magento\\Framework\\AuthorizationInterface"];
const BACKEND_ACTIONS: &[&str] = &["Magento\\Backend\\App\\AbstractAction"];
pub const TEMPLATES: &[&str] = &["Magento\\Framework\\View\\Element\\Template"];
const LAYOUT_UPDATES: &[&str] =
    &["Magento\\Framework\\View\\Layout\\ProcessorInterface", "Magento\\Framework\\View\\Result\\Layout"];
const LAYOUTS: &[&str] = &["Magento\\Framework\\View\\LayoutInterface"];
const BLOCKS: &[&str] = &["Magento\\Framework\\View\\Element\\AbstractBlock"];
const MODULE_MANAGERS: &[&str] =
    &["Magento\\Framework\\Module\\Manager", "Magento\\Framework\\Module\\ModuleManagerInterface"];
const MODULE_LISTS: &[&str] = &["Magento\\Framework\\Module\\ModuleListInterface"];
const MODULE_DIRS: &[&str] = &["Magento\\Framework\\Module\\Dir"];
const MODULE_READERS: &[&str] = &["Magento\\Framework\\Module\\Dir\\Reader"];
const REGISTRARS: &[&str] = &["Magento\\Framework\\Component\\ComponentRegistrar"];
const URL_BUILDERS: &[&str] = &[
    "Magento\\Framework\\UrlInterface",
    "Magento\\Framework\\View\\Element\\AbstractBlock",
    "Magento\\Framework\\App\\Action\\AbstractAction",
    "Magento\\Framework\\App\\Action\\Action",
];
const REDIRECTS: &[&str] = &["Magento\\Framework\\Controller\\Result\\Redirect"];
const TABLE_NAMERS: &[&str] = &[
    "Magento\\Framework\\App\\ResourceConnection",
    "Magento\\Framework\\Model\\ResourceModel\\AbstractResource",
    "Magento\\Framework\\Model\\ResourceModel\\Db\\AbstractDb",
    "Magento\\Framework\\DB\\Adapter\\AdapterInterface",
    "Magento\\Framework\\Setup\\SetupInterface",
];
const RESOURCE_MODELS: &[&str] = &["Magento\\Framework\\Model\\ResourceModel\\Db\\AbstractDb"];

pub static EVENT: MagentoName = MagentoName {
    id: "magentoEvent",
    label: "event",
    source: Source::Event,
    patterns: &[method(EVENTS, &["dispatch"], 0, "eventName")],
};

pub static CONFIG: MagentoName = MagentoName {
    id: "magentoConfigPath",
    label: "configuration path",
    source: Source::Config,
    patterns: &[method(SCOPE_CONFIG, &["getValue", "isSetFlag"], 0, "path")],
};

pub static ACL: MagentoName = MagentoName {
    id: "magentoAcl",
    label: "ACL resource",
    source: Source::Acl,
    patterns: &[method(AUTHORIZATION, &["isAllowed"], 0, "resource"), property(BACKEND_ACTIONS, &["::ADMIN_RESOURCE"])],
};

pub static TEMPLATE: MagentoName = MagentoName {
    id: "magentoTemplate",
    label: "template",
    source: Source::Template,
    patterns: &[method(TEMPLATES, &["setTemplate"], 0, "template"), property(TEMPLATES, &["_template"])],
};

pub static HANDLE: MagentoName = MagentoName {
    id: "magentoHandle",
    label: "layout handle",
    source: Source::Handle,
    patterns: &[method(LAYOUT_UPDATES, &["addHandle"], 0, "handleName")],
};

pub static BLOCK: MagentoName = MagentoName {
    id: "magentoBlock",
    label: "block",
    source: Source::Block,
    patterns: &[
        method(
            LAYOUTS,
            &["getBlock", "getChildBlock", "getChildName", "unsetElement", "isBlock", "hasElement"],
            0,
            "name",
        ),
        method(BLOCKS, &["getChildBlock", "getChildHtml", "getChildChildHtml", "unsetChild"], 0, "alias"),
    ],
};

pub static MODULE: MagentoName = MagentoName {
    id: "magentoModule",
    label: "module",
    source: Source::Module,
    patterns: &[
        method(MODULE_MANAGERS, &["isEnabled", "isOutputEnabled"], 0, "moduleName"),
        method(MODULE_LISTS, &["has", "getOne"], 0, "name"),
        method(MODULE_DIRS, &["getDir"], 0, "moduleName"),
        method(MODULE_READERS, &["getModuleDir"], 1, "moduleName"),
        // Where a module is registered.
        method(REGISTRARS, &["register"], 1, "componentName"),
    ],
};

pub static ROUTE: MagentoName = MagentoName {
    id: "magentoRoute",
    label: "route",
    source: Source::Route,
    patterns: &[
        method(URL_BUILDERS, &["getUrl", "_redirect"], 0, "routePath"),
        method(REDIRECTS, &["setPath"], 0, "path"),
    ],
};

pub static TABLE: MagentoName = MagentoName {
    id: "magentoTable",
    label: "table",
    source: Source::Table,
    patterns: &[
        method(TABLE_NAMERS, &["getTableName", "getTable"], 0, "tableName"),
        method(RESOURCE_MODELS, &["_init"], 0, "mainTable"),
    ],
};

pub static COLUMN: MagentoName = MagentoName {
    id: "magentoColumn",
    label: "column",
    source: Source::Column,
    patterns: &[method(RESOURCE_MODELS, &["_init"], 1, "idFieldName")],
};

/// Magento's kinds.
pub fn kinds() -> &'static [&'static dyn ReferenceKind] {
    static KINDS: [&dyn ReferenceKind; 10] =
        [&EVENT, &CONFIG, &ACL, &TEMPLATE, &HANDLE, &BLOCK, &MODULE, &ROUTE, &TABLE, &COLUMN];
    &KINDS
}

/// The kind of what an XML file names ([`What`]), when it has one.
pub fn kind_of(what: What) -> Option<&'static MagentoName> {
    Some(match what {
        What::Event => &EVENT,
        What::Acl => &ACL,
        What::Config => &CONFIG,
        What::Template => &TEMPLATE,
        What::Handle => &HANDLE,
        What::Block => &BLOCK,
        What::Module => &MODULE,
        What::Table => &TABLE,
        What::Route => &ROUTE,
        What::Class | What::Component => return None,
    })
}

/// Between a table and its column, in what the usages of one share.
const SEPARATOR: char = '\u{1f}';

/// `Vendor_Module`: letters and digits around one underscore at least.
fn is_module_name(name: &str) -> bool {
    name.contains('_')
        && !name.starts_with('_')
        && !name.ends_with('_')
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Where the module `name` is written in its registration.
fn registration(path: &Path, name: &str) -> Target {
    let file = path.join("registration.php");
    let text = std::fs::read_to_string(&file).unwrap_or_default();
    let range = text
        .find(name)
        .map_or_else(Range::default, |at| LineIndex::new(&text).range(at as u32, (at + name.len()) as u32));
    Target { path: file, range }
}

/// The files below `folder` with the extension `extension`, as paths from
/// it with `/`.
fn files_below(folder: &Path, extension: &str, above: &str, depth: usize, out: &mut Vec<String>) {
    if depth > 8 {
        return;
    }
    let mut entries: Vec<PathBuf> =
        std::fs::read_dir(folder).into_iter().flatten().flatten().map(|entry| entry.path()).collect();
    entries.sort();
    for entry in entries {
        let Some(name) = entry.file_name().map(|name| name.to_string_lossy().into_owned()) else { continue };
        let path = if above.is_empty() { name } else { format!("{above}/{name}") };
        if entry.is_dir() {
            files_below(&entry, extension, &path, depth + 1, out);
        } else if entry.extension().is_some_and(|ext| ext == extension) {
            out.push(path);
        }
    }
}

/// `product_view` is `Product\View`; `index` is `Index`.
fn studly_path(part: &str) -> String {
    let studly = |word: &str| {
        let mut chars = word.chars();
        chars.next().map_or_else(String::new, |first| first.to_ascii_uppercase().to_string() + chars.as_str())
    };
    part.split('_').map(studly).collect::<Vec<_>>().join("\\")
}

/// The three parts of a URL path, the ones left out being `index`:
/// `wishlist` is `wishlist/index/index`. None for a path with a part that
/// stands for the current request's (`*`) or that isn't one.
fn route_parts(value: &str) -> Option<[String; 3]> {
    let value = value.split(['?', '#']).next().unwrap_or(value).trim_matches('/');
    let mut parts = value.split('/');
    let mut next = || parts.next().filter(|part| !part.is_empty()).unwrap_or("index").to_owned();
    let found = [next(), next(), next()];
    let plain = |part: &String| part.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    (!value.is_empty() && found.iter().all(plain)).then_some(found)
}

impl MagentoName {
    fn project(cx: &RefContext<'_>) -> Arc<MagentoProject> {
        cx.engine.magento()
    }

    /// The area of the file the string is in.
    fn site_area(cx: &RefContext<'_>) -> Option<String> {
        area_of(&cx.site.as_ref()?.path).filter(|area| matches!(area.as_str(), "frontend" | "adminhtml"))
    }

    /// A template's id with its module: the one written, else the module of
    /// the file naming it.
    fn template_id(cx: &RefContext<'_>, project: &MagentoProject, value: &str) -> String {
        if value.contains("::") {
            return value.to_owned();
        }
        let module = cx.site.as_ref().and_then(|site| project.module_name_of(&site.path));
        match module {
            Some(module) => format!("{module}::{value}"),
            None => value.to_owned(),
        }
    }

    fn defs<'p>(&self, project: &'p MagentoProject) -> &'p [Def] {
        match self.source {
            Source::Config => &project.config,
            Source::Acl => &project.acl,
            Source::Handle => &project.handles,
            _ => &[],
        }
    }

    fn what(&self) -> Option<What> {
        Some(match self.source {
            Source::Event => What::Event,
            Source::Config => What::Config,
            Source::Acl => What::Acl,
            Source::Template => What::Template,
            Source::Handle => What::Handle,
            Source::Block => What::Block,
            Source::Module => What::Module,
            Source::Route => What::Route,
            Source::Table => What::Table,
            Source::Column => return None,
        })
    }

    /// The table a column's string is of: the argument before it.
    fn column_table(cx: &RefContext<'_>) -> Option<String> {
        cx.site.as_ref()?.first_argument.clone()
    }

    /// The classes a URL path may be the action of: the route's modules,
    /// each as `Vendor\Module\Controller\[Adminhtml\]Controller\Action`.
    fn route_classes(cx: &RefContext<'_>, project: &MagentoProject, value: &str) -> Vec<String> {
        let Some([front, controller, action]) = route_parts(value) else { return Vec::new() };
        let mut out: Vec<String> = Vec::new();
        for route in project.routes.iter().filter(|route| route.front_name == front) {
            let admin = route.area.as_deref() == Some("adminhtml");
            for module in &route.modules {
                let namespace = module.replacen('_', "\\", 1);
                let (controller, action) = (studly_path(&controller), studly_path(&action));
                let class = if admin {
                    format!("{namespace}\\Controller\\Adminhtml\\{controller}\\{action}")
                } else {
                    format!("{namespace}\\Controller\\{controller}\\{action}")
                };
                if !out.contains(&class) && cx.engine.resolver().class_model(&class).is_some() {
                    out.push(class);
                }
            }
        }
        out
    }

    /// Where `value` is declared.
    fn targets(&self, cx: &RefContext<'_>, value: &str, at: usize) -> Vec<Target> {
        let project = Self::project(cx);
        let place = |file: &Path, range: Range| Target { path: file.to_path_buf(), range };
        match self.source {
            Source::Event => project
                .observers
                .iter()
                .filter(|observer| observer.event == value)
                .map(|observer| place(&observer.file, observer.range))
                .collect(),
            Source::Config | Source::Acl | Source::Handle => {
                let found = self.defs(&project).iter().filter(|def| def.name == value);
                let mut out: Vec<Target> = found.map(|def| place(&def.file, def.range)).collect();
                // A resource's declaration is where it has its title.
                if self.source == Source::Acl {
                    let titled: Vec<Target> = self
                        .defs(&project)
                        .iter()
                        .filter(|def| def.name == value && def.detail.is_some())
                        .map(|def| place(&def.file, def.range))
                        .collect();
                    if !titled.is_empty() {
                        out = titled;
                    }
                }
                out
            }
            Source::Template => {
                let id = Self::template_id(cx, &project, value);
                let files = project.template_files(&id, None, Self::site_area(cx).as_deref());
                files.into_iter().filter(|file| file.is_file()).map(|file| place(&file, Range::default())).collect()
            }
            Source::Block => {
                let named: Vec<Target> = project
                    .blocks
                    .iter()
                    .filter(|block| block.name == value && (block.class.is_some() || block.template.is_none()))
                    .map(|block| place(&block.file, block.range))
                    .collect();
                if !named.is_empty() {
                    return named;
                }
                let declared: Vec<Target> = project
                    .mentions_of(What::Block, value)
                    .filter(|(_, mention)| mention.declares)
                    .map(|(file, mention)| place(&file.file, mention.range))
                    .collect();
                if !declared.is_empty() {
                    return declared;
                }
                let aliased = project.blocks.iter().filter(|block| block.alias.as_deref() == Some(value));
                aliased.map(|block| place(&block.file, block.range)).collect()
            }
            Source::Module => {
                project.module(value).map(|module| registration(&module.path, &module.name)).into_iter().collect()
            }
            Source::Route => {
                // On the front name: the route. Else the action's class.
                let front = value.trim_start_matches('/').split('/').next().unwrap_or_default();
                let on_front = at <= value.find(front).unwrap_or(0) + front.len() && value.contains('/');
                let routes = || -> Vec<Target> {
                    let found = project.routes.iter().filter(|route| route.front_name == front);
                    found.map(|route| place(&route.file, route.range)).collect()
                };
                if on_front {
                    return routes();
                }
                let classes = Self::route_classes(cx, &project, value);
                let found: Vec<Target> = classes.iter().flat_map(|class| cx.engine.class_targets(class)).collect();
                if found.is_empty() { routes() } else { found }
            }
            Source::Table => {
                let found = project.tables.iter().filter(|table| table.name == value);
                found.map(|table| place(&table.file, table.range)).collect()
            }
            Source::Column => {
                let Some(table) = Self::column_table(cx) else { return Vec::new() };
                let columns = project.columns(&table);
                let found = columns.into_iter().filter(|(_, column)| column.0 == value);
                found.map(|(table, column)| place(&table.file, column.2)).collect()
            }
        }
    }

    /// The names completion offers, each with what it is.
    fn names(&self, cx: &RefContext<'_>) -> Vec<(String, Option<String>)> {
        let project = Self::project(cx);
        let mut out: Vec<(String, Option<String>)> = match self.source {
            Source::Event => project.observers.iter().map(|observer| (observer.event.clone(), None)).collect(),
            Source::Config | Source::Acl | Source::Handle => {
                self.defs(&project).iter().map(|def| (def.name.clone(), def.detail.clone())).collect()
            }
            Source::Template => {
                let mut out = Vec::new();
                for module in &project.modules {
                    for area in ["frontend", "adminhtml", "base"] {
                        let mut found = Vec::new();
                        let folder = module.path.join("view").join(area).join("templates");
                        files_below(&folder, "phtml", "", 0, &mut found);
                        out.extend(found.into_iter().map(|path| (format!("{}::{path}", module.name), None)));
                    }
                }
                out
            }
            Source::Block => {
                let mut out: Vec<(String, Option<String>)> = Vec::new();
                for block in &project.blocks {
                    out.push((block.name.clone(), block.class.clone()));
                    if let Some(alias) = &block.alias {
                        out.push((alias.clone(), block.class.clone()));
                    }
                }
                for (_, mention) in project.mentions().filter(|(_, mention)| mention.what == What::Block) {
                    if mention.declares {
                        out.push((mention.name.clone(), None));
                    }
                }
                out
            }
            Source::Module => project.modules.iter().map(|module| (module.name.clone(), None)).collect(),
            Source::Route => {
                // Each action of each route's modules: the classes of their `Controller` folder.
                let mut out = Vec::new();
                for route in &project.routes {
                    let admin = route.area.as_deref() == Some("adminhtml");
                    for module in route.modules.iter().filter_map(|module| project.module(module)) {
                        let folder = module.path.join("Controller");
                        let folder = if admin { folder.join("Adminhtml") } else { folder };
                        let mut found = Vec::new();
                        files_below(&folder, "php", "", 0, &mut found);
                        for file in found {
                            let class = file.trim_end_matches(".php");
                            if !admin && class.starts_with("Adminhtml/") {
                                continue;
                            }
                            let Some((controller, action)) = class.rsplit_once('/') else { continue };
                            let path = format!(
                                "{}/{}/{}",
                                route.front_name,
                                controller.replace('/', "_").to_ascii_lowercase(),
                                action.to_ascii_lowercase()
                            );
                            out.push((path, Some(format!("{}: {controller}\\{action}", module.name))));
                        }
                    }
                }
                out
            }
            Source::Table => project.tables.iter().map(|table| (table.name.clone(), table.comment.clone())).collect(),
            Source::Column => match Self::column_table(cx) {
                Some(table) => {
                    let columns = project.columns(&table);
                    columns.into_iter().map(|(_, column)| (column.0.clone(), Some(column.1.clone()))).collect()
                }
                None => Vec::new(),
            },
        };
        // One of each, the one that says what it is first.
        out.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.is_some().cmp(&a.1.is_some())));
        out.dedup_by(|b, a| a.0 == b.0);
        out
    }

    /// The XML that declares or names `key`, each with whether what is
    /// written is the whole key (a template written without its module
    /// isn't).
    fn xml_places(&self, cx: &RefContext<'_>, key: &str) -> Vec<(PathBuf, Range, bool)> {
        let project = Self::project(cx);
        let Some(what) = self.what() else { return Vec::new() };
        // A front name is no usage of a path of it.
        if self.source == Source::Route {
            return Vec::new();
        }
        let mut out: Vec<(PathBuf, Range, bool)> =
            project.mentions_of(what, key).map(|(file, mention)| (file.file.clone(), mention.range, true)).collect();
        if self.source == Source::Template {
            // Named without its module, in a layout file of that module.
            for block in project.blocks_of(key) {
                if let Some(range) = block.template_range
                    && block.template.as_deref() != Some(key)
                {
                    out.push((block.file.clone(), range, false));
                }
            }
        }
        out.sort_by(|a, b| (&a.0, a.1.start).cmp(&(&b.0, b.1.start)));
        out.dedup_by(|b, a| a.0 == b.0 && a.1 == b.1);
        out
    }
}

impl ReferenceKind for MagentoName {
    fn id(&self) -> &'static str {
        self.id
    }

    fn patterns(&self) -> &'static [PlacePattern] {
        self.patterns
    }

    /// A template is its id with its module, however it is written; a
    /// column is its table's; a URL path has its three parts.
    fn canonical(&self, cx: &RefContext<'_>, value: &str) -> String {
        match self.source {
            Source::Template => Self::template_id(cx, &Self::project(cx), value),
            Source::Column => match Self::column_table(cx) {
                Some(table) => format!("{table}{SEPARATOR}{value}"),
                None => value.to_owned(),
            },
            Source::Route => route_parts(value).map_or_else(|| value.to_owned(), |parts| parts.join("/")),
            _ => value.to_owned(),
        }
    }

    fn usage_text<'v>(&self, value: &'v str) -> &'v str {
        match self.source {
            // (A usage may leave the module out.)
            Source::Template => value.rsplit_once("::").map_or(value, |(_, path)| path),
            Source::Column => value.rsplit(SEPARATOR).next().unwrap_or(value),
            // (And the parts that are `index`.)
            Source::Route => value.split('/').next().unwrap_or(value),
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

    fn resolve(&self, cx: &RefContext<'_>, value: &str, at: usize) -> Vec<Target> {
        let mut out: Vec<Target> = Vec::new();
        for target in self.targets(cx, value, at) {
            if !out.contains(&target) {
                out.push(target);
            }
        }
        out
    }

    /// The XML that declares or names `key`.
    fn other_usages(&self, cx: &RefContext<'_>, key: &str) -> Vec<(PathBuf, Range)> {
        self.xml_places(cx, key).into_iter().map(|(file, range, _)| (file, range)).collect()
    }

    /// What a rename changes: an event's and a block's whole name, an ACL
    /// resource's part after its module, a template's path.
    fn rename_target(&self, cx: &RefContext<'_>, value: &str, _at: usize) -> Option<RenameTarget> {
        let project = Self::project(cx);
        let key = self.canonical(cx, value);
        let declared = || {
            let what = self.what()?;
            let mut found = project.mentions_of(what, &key);
            found.find(|(file, mention)| mention.declares && !cx.engine.is_library(&file.file)).map(|_| ())
        };
        let from = match self.source {
            Source::Event | Source::Block => {
                declared()?;
                0
            }
            Source::Acl => {
                declared()?;
                key.find("::")? + 2
            }
            Source::Template => {
                let files = project.template_files(&key, None, None);
                files.iter().find(|file| file.is_file() && !cx.engine.is_library(file))?;
                key.find("::")? + 2
            }
            _ => return None,
        };
        Some(RenameTarget { range: (from, key.len()), key, declarations: Vec::new() })
    }

    /// A usage written without the module has only the part renamed.
    fn site_rename_range(&self, value: &str, target: &RenameTarget) -> (usize, usize) {
        match self.source {
            Source::Template if !value.contains("::") => (0, value.len()),
            _ => target.range,
        }
    }

    fn other_renames(&self, cx: &RefContext<'_>, target: &RenameTarget) -> Vec<(PathBuf, Range)> {
        let part = |range: Range, whole: bool| {
            let column = |offset: usize| Pos::new(range.start.line, range.start.col + offset as u32);
            if whole { Range::new(column(target.range.0), column(target.range.1)) } else { range }
        };
        let places = self.xml_places(cx, &target.key);
        places
            .into_iter()
            .filter(|(file, _, _)| !cx.engine.is_library(file))
            .map(|(file, range, whole)| (file, part(range, whole)))
            .collect()
    }

    /// A template's files move with its name: the module's and the themes'.
    fn rename_files(&self, cx: &RefContext<'_>, key: &str, new_value: &str) -> Vec<FileEdit> {
        if self.source != Source::Template {
            return Vec::new();
        }
        let project = Self::project(cx);
        let from = project.template_files(key, None, None);
        let to = project.template_files(new_value, None, None);
        let moved = from.into_iter().zip(to).filter(|(from, to)| from != to && from.is_file());
        moved.filter(|(from, _)| !cx.engine.is_library(from)).map(|(from, to)| FileEdit::Rename { from, to }).collect()
    }

    fn valid_name(&self, name: &str) -> bool {
        let plain =
            |extra: &[char]| !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || extra.contains(&c));
        match self.source {
            Source::Event | Source::Acl => plain(&['_']),
            Source::Block => plain(&['_', '.', '-']),
            Source::Template => plain(&['_', '.', '-', '/']) && name.ends_with(".phtml") && !name.starts_with('/'),
            _ => false,
        }
    }

    fn is_known(&self, cx: &RefContext<'_>, value: &str) -> bool {
        self.undefined(cx, value).is_none()
    }

    fn undefined(&self, cx: &RefContext<'_>, value: &str) -> Option<Undefined> {
        let site = cx.site.as_ref()?;
        if value.is_empty() || cx.engine.is_library(&site.path) {
            return None;
        }
        let project = Self::project(cx);
        match self.source {
            Source::Template => {
                let id = Self::template_id(cx, &project, value);
                let (module, path) = id.split_once("::")?;
                let plain = !value.contains(['$', '{', ' ']) && path.ends_with(".phtml");
                // A module that isn't here may well have it.
                let files = project.template_files(&id, None, None);
                (plain && project.module(module).is_some() && !files.iter().any(|file| file.is_file()))
                    .then(|| Undefined::warning(format!("Unknown template '{value}'")))
            }
            Source::Acl => {
                let (module, resource) = value.split_once("::")?;
                let plain = is_module_name(module) && !resource.is_empty() && !resource.contains(['$', '{', ' ']);
                let declares = project.acl.iter().any(|def| def.name.starts_with(module) && def.detail.is_some());
                (plain && declares && !project.acl.iter().any(|def| def.name == value))
                    .then(|| Undefined::warning(format!("Unknown ACL resource '{value}'")))
            }
            Source::Module => {
                // Only among the modules of a vendor the project has modules of.
                let vendor = value.split('_').next()?;
                let has_vendor = project.modules.iter().any(|module| {
                    module.name.split('_').next().is_some_and(|known| known.eq_ignore_ascii_case(vendor))
                });
                (is_module_name(value) && has_vendor && project.module(value).is_none())
                    .then(|| Undefined::warning(format!("Unknown module '{value}'")))
            }
            _ => None,
        }
    }

    fn hover(&self, cx: &RefContext<'_>, value: &str) -> Option<String> {
        let project = Self::project(cx);
        let target = self.targets(cx, value, value.len()).into_iter().next()?;
        let root = cx.engine.root();
        let file = target.path.strip_prefix(&root).unwrap_or(&target.path).to_string_lossy().replace('\\', "/");
        let place =
            if target.range == Range::default() { file } else { format!("{file}:{}", target.range.start.line + 1) };
        let detail = match self.source {
            Source::Event => {
                let observers: Vec<&str> = project
                    .observers
                    .iter()
                    .filter(|observer| observer.event == value)
                    .filter_map(|observer| observer.class.as_deref())
                    .collect();
                (!observers.is_empty()).then(|| observers.join("`, `"))
            }
            Source::Config | Source::Acl => {
                self.defs(&project).iter().filter(|def| def.name == value).find_map(|def| def.detail.clone())
            }
            Source::Block => project
                .blocks
                .iter()
                .filter(|block| block.name == value || block.alias.as_deref() == Some(value))
                .find_map(|block| block.class.clone()),
            Source::Template => {
                let id = Self::template_id(cx, &project, value);
                project.blocks_of(&id).into_iter().find_map(|block| block.class.clone())
            }
            Source::Route => Self::route_classes(cx, &project, value).into_iter().next(),
            Source::Table => project.table(value).and_then(|table| table.comment.clone()),
            Source::Column => Self::column_table(cx).and_then(|table| {
                let columns = project.columns(&table);
                columns.into_iter().find(|(_, column)| column.0 == value).map(|(_, column)| column.1.clone())
            }),
            Source::Handle | Source::Module => None,
        };
        let mut out = format!("**{}** `{value}`", self.label);
        if let Some(detail) = detail {
            out.push_str(&format!("\n\n`{detail}`"));
        }
        out.push_str(&format!("\n\n{place}"));
        Some(out)
    }
}
