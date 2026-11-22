//! Joomla's reference kinds ([`kinds`]), on the machinery every framework
//! shares ([`crate::frameworks::refs`]). All are one type, [`JoomlaName`].
//!
//! | kind | names | where |
//! | --- | --- | --- |
//! | `joomlaLanguage` | a language constant, where a `.ini` file declares it (British English first) | `Text::_('COM_X_Y')`, `sprintf()`, `plural()`, `script()`, `alt()` |
//! | `joomlaLayout` | a layout's file, a template's override first | `LayoutHelper::render('joomla.content.tags')`, `debug()`, `new FileLayout('id')` |
//! | `joomlaHtml` | an HTML helper's method: `behavior.core` is `core()` of `Joomla\CMS\HTML\Helpers\Behavior` | `HTMLHelper::_('behavior.core')` |
//! | `joomlaAsset` | a web asset of a `joomla.asset.json` | `useScript()`, `useStyle()`, `usePreset()`, `registerAndUse…()`, `useAsset()`, `getAsset()` and their kin |
//! | `joomlaExtension` | a component, a module or a plugin: its manifest | `bootComponent('com_content')`, `ComponentHelper::getParams()`, `ModuleHelper::getModule()`, `PluginHelper::getPlugin('content', 'pagebreak')` |
//! | `joomlaEvent` | an event: its class, by Joomla's map of them | `$dispatcher->dispatch('onContentPrepare', …)`, the keys of `getSubscribedEvents()` |
//! | `joomlaHandler` | the method handling an event | the values of `getSubscribedEvents()` |
//! | `joomlaAction` | an ACL action of an `access.xml`, the component's own first | `$user->authorise('core.edit', …)` |
//! | `joomlaForm` | a form: `forms/<name>.xml` of the extension | the second argument of `loadForm('com_content.article', 'article')` |
//! | `joomlaField` | a field of the extension's forms | `$form->getValue('title')`, `setValue()`, `getField()`, `getInput()`, `getLabel()`, `setFieldAttribute()`, `removeField()` |
//! | `joomlaParam` | a parameter: a field of `config.xml` or of a manifest's `<config>` | `$params->get('show_title')` on a registry |
//! | `joomlaTable` | a table of the SQL scripts, written with its prefix | `'#__content'` given to a query's `from()`, `join()`, `update()`…, to `quoteName()`, to a table class's constructor |
//! | `joomlaColumn` | a column of the table named before it | the key a table class's constructor is given |
//! | `joomlaTemplate` | a view's sub-template: `default_links.php` beside `default.php`, a template's override of it first | `$this->loadTemplate('links')` in a view's template |
//! | `joomlaRoute` | the parts of a URL: `option=com_content` is the component, `view=article` its view class, `task=article.save` the controller's method | `Route::_('index.php?option=com_content&view=article')`, `Route::link()` |
//!
//! Each has completion, goto, hover and Find References; the XML naming a
//! constant, a layout or an asset is listed with the PHP
//! ([`JoomlaProject::mentions`]). A language constant is renamed, in PHP,
//! in the language files and in the XML.
//!
//! A language constant and a layout that name nothing are reported
//! ([`ReferenceKind::undefined`]): a constant only when it starts with the
//! prefix of the extension the file is in and that extension has language
//! files; a layout only when it is one of Joomla's (`joomla.…`) and the
//! project has those.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::ini;
use super::project::{Extension, JoomlaProject, What};
use crate::base::Range;
use crate::frameworks::laravel::refs::controller_action::method_target;
use crate::frameworks::refs::{
    CallTarget, CalleePattern, PlacePattern, Position, RefContext, RefItem, ReferenceKind, RenameTarget, TEMPLATE_THIS,
    Undefined,
};
use crate::navigate::Target;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Source {
    Language,
    Layout,
    Html,
    Asset,
    Extension,
    Event,
    Handler,
    Action,
    Form,
    Field,
    Param,
    Table,
    Column,
    Route,
    Template,
}

/// One of the kinds of the module documentation.
pub struct JoomlaName {
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

const TEXT: &[&str] = &["Joomla\\CMS\\Language\\Text"];
const LANGUAGE: &[&str] = &["Joomla\\CMS\\Language\\Language"];
const LAYOUT_HELPER: &[&str] = &["Joomla\\CMS\\Layout\\LayoutHelper"];
const FILE_LAYOUT: &[&str] = &["Joomla\\CMS\\Layout\\FileLayout", "Joomla\\CMS\\Layout\\LayoutInterface"];
const HTML_HELPER: &[&str] = &["Joomla\\CMS\\HTML\\HTMLHelper"];
const ASSETS: &[&str] = &["Joomla\\CMS\\WebAsset\\WebAssetManager", "Joomla\\CMS\\WebAsset\\WebAssetManagerInterface"];
const EXTENSION_MANAGER: &[&str] = &["Joomla\\CMS\\Extension\\ExtensionManagerInterface"];
const COMPONENT_HELPER: &[&str] = &["Joomla\\CMS\\Component\\ComponentHelper"];
const MODULE_HELPER: &[&str] = &["Joomla\\CMS\\Helper\\ModuleHelper"];
const PLUGIN_HELPER: &[&str] = &["Joomla\\CMS\\Plugin\\PluginHelper"];
const DISPATCHERS: &[&str] = &["Joomla\\Event\\DispatcherInterface", "Joomla\\CMS\\Application\\EventAwareInterface"];
const SUBSCRIBERS: &[&str] = &["Joomla\\Event\\SubscriberInterface", "Joomla\\CMS\\Plugin\\CMSPlugin"];
const USERS: &[&str] = &["Joomla\\CMS\\User\\User"];
const FORM_MODELS: &[&str] = &[
    "Joomla\\CMS\\MVC\\Model\\FormBehaviorTrait",
    "Joomla\\CMS\\MVC\\Model\\FormModel",
    "Joomla\\CMS\\MVC\\Model\\ListModel",
    "Joomla\\CMS\\MVC\\Model\\FormModelInterface",
];
const FORMS: &[&str] = &["Joomla\\CMS\\Form\\Form"];
const REGISTRIES: &[&str] = &["Joomla\\Registry\\Registry"];
const QUERIES: &[&str] = &[
    "Joomla\\Database\\QueryInterface",
    "Joomla\\Database\\DatabaseQuery",
    "Joomla\\Database\\DatabaseInterface",
    "Joomla\\Database\\DatabaseDriver",
];
const TABLES: &[&str] = &["Joomla\\CMS\\Table\\Table"];
const ROUTES: &[&str] = &["Joomla\\CMS\\Router\\Route"];
const VIEWS: &[&str] = &["Joomla\\CMS\\MVC\\View\\HtmlView", "Joomla\\CMS\\MVC\\View\\AbstractView", TEMPLATE_THIS];

pub static LANGUAGE_CONSTANT: JoomlaName = JoomlaName {
    id: "joomlaLanguage",
    label: "language constant",
    source: Source::Language,
    patterns: &[
        method(TEXT, &["_", "sprintf", "plural", "script", "alt", "printf"], 0, "string"),
        method(LANGUAGE, &["_", "hasKey"], 0, "string"),
    ],
};

pub static LAYOUT: JoomlaName = JoomlaName {
    id: "joomlaLayout",
    label: "layout",
    source: Source::Layout,
    patterns: &[
        method(LAYOUT_HELPER, &["render", "debug"], 0, "layoutFile"),
        method(FILE_LAYOUT, &["__construct", "setLayoutId", "sublayout"], 0, "layoutId"),
    ],
};

pub static HTML: JoomlaName = JoomlaName {
    id: "joomlaHtml",
    label: "HTML helper",
    source: Source::Html,
    patterns: &[method(HTML_HELPER, &["_"], 0, "key")],
};

pub static ASSET: JoomlaName = JoomlaName {
    id: "joomlaAsset",
    label: "web asset",
    source: Source::Asset,
    patterns: &[
        method(
            ASSETS,
            &[
                "useScript",
                "useStyle",
                "usePreset",
                "disableScript",
                "disableStyle",
                "disablePreset",
                "registerAndUseScript",
                "registerAndUseStyle",
                "registerAndUsePreset",
                "registerScript",
                "registerStyle",
                "registerPreset",
            ],
            0,
            "name",
        ),
        method(
            ASSETS,
            &["useAsset", "getAsset", "assetExists", "isAssetActive", "disableAsset", "getAssetState"],
            1,
            "name",
        ),
    ],
};

pub static EXTENSION: JoomlaName = JoomlaName {
    id: "joomlaExtension",
    label: "extension",
    source: Source::Extension,
    patterns: &[
        method(EXTENSION_MANAGER, &["bootComponent", "bootModule"], 0, "component"),
        method(COMPONENT_HELPER, &["getParams", "isEnabled", "getComponent", "isInstalled"], 0, "option"),
        method(MODULE_HELPER, &["getModule", "isEnabled", "getLayoutPath"], 0, "module"),
        method(PLUGIN_HELPER, &["getPlugin", "isEnabled", "importPlugin"], 1, "plugin"),
        method(EXTENSION_MANAGER, &["bootPlugin"], 0, "plugin"),
    ],
};

pub static EVENT: JoomlaName = JoomlaName {
    id: "joomlaEvent",
    label: "event",
    source: Source::Event,
    patterns: &[
        method(DISPATCHERS, &["dispatch", "triggerEvent", "getListeners", "hasListener"], 0, "name"),
        PlacePattern::argument(
            CalleePattern::Return { classes: SUBSCRIBERS, names: &["getSubscribedEvents"] },
            0,
            "value",
        )
        .at(Position::ArrayKey),
    ],
};

pub static HANDLER: JoomlaName = JoomlaName {
    id: "joomlaHandler",
    label: "event handler",
    source: Source::Handler,
    patterns: &[PlacePattern::argument(
        CalleePattern::Return { classes: SUBSCRIBERS, names: &["getSubscribedEvents"] },
        0,
        "value",
    )
    .at(Position::ArrayValue)],
};

pub static ACTION: JoomlaName = JoomlaName {
    id: "joomlaAction",
    label: "ACL action",
    source: Source::Action,
    patterns: &[method(USERS, &["authorise"], 0, "action")],
};

pub static FORM: JoomlaName = JoomlaName {
    id: "joomlaForm",
    label: "form",
    source: Source::Form,
    patterns: &[method(FORM_MODELS, &["loadForm"], 1, "source")],
};

pub static FIELD: JoomlaName = JoomlaName {
    id: "joomlaField",
    label: "form field",
    source: Source::Field,
    patterns: &[method(
        FORMS,
        &[
            "getValue",
            "setValue",
            "getField",
            "getInput",
            "getLabel",
            "setFieldAttribute",
            "getFieldAttribute",
            "removeField",
            "renderField",
        ],
        0,
        "name",
    )],
};

pub static PARAM: JoomlaName = JoomlaName {
    id: "joomlaParam",
    label: "parameter",
    source: Source::Param,
    patterns: &[method(REGISTRIES, &["get", "set", "exists", "def"], 0, "path")],
};

pub static TABLE: JoomlaName = JoomlaName {
    id: "joomlaTable",
    label: "table",
    source: Source::Table,
    patterns: &[
        method(
            QUERIES,
            &[
                "from",
                "join",
                "leftJoin",
                "rightJoin",
                "innerJoin",
                "outerJoin",
                "update",
                "insert",
                "delete",
                "quoteName",
                "qn",
            ],
            0,
            "table",
        ),
        method(TABLES, &["__construct"], 0, "table"),
    ],
};

pub static COLUMN: JoomlaName = JoomlaName {
    id: "joomlaColumn",
    label: "column",
    source: Source::Column,
    patterns: &[method(TABLES, &["__construct"], 1, "key")],
};

pub static ROUTE: JoomlaName = JoomlaName {
    id: "joomlaRoute",
    label: "route",
    source: Source::Route,
    patterns: &[method(ROUTES, &["_"], 0, "url"), method(ROUTES, &["link"], 1, "url")],
};

pub static TEMPLATE: JoomlaName = JoomlaName {
    id: "joomlaTemplate",
    label: "template",
    source: Source::Template,
    patterns: &[method(VIEWS, &["loadTemplate"], 0, "tpl")],
};

/// Joomla's kinds.
pub fn kinds() -> &'static [&'static dyn ReferenceKind] {
    static KINDS: [&dyn ReferenceKind; 15] = [
        &LANGUAGE_CONSTANT,
        &LAYOUT,
        &HTML,
        &ASSET,
        &EXTENSION,
        &EVENT,
        &HANDLER,
        &ACTION,
        &FORM,
        &FIELD,
        &PARAM,
        &TABLE,
        &COLUMN,
        &ROUTE,
        &TEMPLATE,
    ];
    &KINDS
}

/// The kind of what an XML file names ([`What`]), when it has one.
pub fn kind_of(what: What) -> Option<&'static JoomlaName> {
    Some(match what {
        What::Language => &LANGUAGE_CONSTANT,
        What::Layout => &LAYOUT,
        What::Asset => &ASSET,
        What::Action => &ACTION,
        What::Extension => &EXTENSION,
        What::Class | What::FieldType => return None,
    })
}

const SEPARATOR: char = '\u{1f}';

fn studly(name: &str) -> String {
    let mut chars = name.chars();
    chars.next().map_or_else(String::new, |first| first.to_ascii_uppercase().to_string() + chars.as_str())
}

/// The table a string names: without the alias after it (`#__content AS a`).
fn table_name(value: &str) -> &str {
    value.split_whitespace().next().unwrap_or(value)
}

/// The parameters of a URL's query, each with the bytes of its value in
/// `value`: `index.php?option=com_content&view=article`.
fn query(value: &str) -> Vec<(&str, &str, usize)> {
    let start = value.find('?').map_or(0, |at| at + 1);
    let mut out = Vec::new();
    let mut offset = start;
    for part in value[start..].split('&') {
        if let Some((key, found)) = part.split_once('=') {
            out.push((key, found, offset + key.len() + 1));
        }
        offset += part.len() + 1;
    }
    out
}

impl JoomlaName {
    fn project(cx: &RefContext<'_>) -> Arc<JoomlaProject> {
        cx.engine.joomla()
    }

    /// The extension folder the string's file is in.
    fn site_extension<'p>(cx: &RefContext<'_>, project: &'p JoomlaProject) -> Option<&'p Extension> {
        project.extension_of(&cx.site.as_ref()?.path)
    }

    /// The folders of the extension the string's file is in: a component's
    /// site and administrator ones.
    fn own<'p>(cx: &RefContext<'_>, project: &'p JoomlaProject) -> Vec<&'p Extension> {
        match Self::site_extension(cx, project) {
            Some(extension) => project.extensions.iter().filter(|other| other.name == extension.name).collect(),
            None => Vec::new(),
        }
    }

    /// `found` with those of the string's own extension first.
    fn own_first<'p, T>(
        cx: &RefContext<'_>,
        project: &'p JoomlaProject,
        found: Vec<&'p T>,
        file: impl Fn(&T) -> &Path,
    ) -> Vec<&'p T> {
        let own = Self::own(cx, project);
        let inside = |item: &&T| own.iter().any(|extension| file(item).starts_with(&extension.path));
        let (mut first, rest): (Vec<&T>, Vec<&T>) = found.into_iter().partition(inside);
        first.extend(rest);
        first
    }

    /// The class the method returning the string is of (a subscriber's).
    fn returning_class(cx: &RefContext<'_>) -> Option<String> {
        match &cx.site.as_ref()?.call.as_ref()?.0 {
            CallTarget::Return { class, .. } => Some(class.clone()),
            _ => None,
        }
    }

    /// The class and method an HTML helper's key names.
    fn html_target(value: &str) -> (String, String) {
        match value.rsplit_once('.') {
            Some((helper, method)) => {
                let helper = helper.split('.').map(studly).collect::<Vec<_>>().join("\\");
                (format!("Joomla\\CMS\\HTML\\Helpers\\{helper}"), method.to_owned())
            }
            None => ("Joomla\\CMS\\HTML\\HTMLHelper".to_owned(), value.to_owned()),
        }
    }

    /// What the part of a URL at byte `at` names.
    fn route_targets(cx: &RefContext<'_>, project: &JoomlaProject, value: &str, at: usize) -> Vec<Target> {
        let parts = query(value);
        let option = parts.iter().find(|(key, _, _)| *key == "option").map(|(_, found, _)| *found);
        // The component: the one named, else the file's own.
        let client = Self::site_extension(cx, project).and_then(|extension| extension.client).unwrap_or("Site");
        let folders: Vec<&Extension> = match option {
            Some(option) => project.extension(option),
            None => Self::own(cx, project),
        };
        let folder = folders.iter().find(|folder| folder.client == Some(client)).or(folders.first()).copied();
        let Some((key, found, _)) = parts.iter().find(|(_, found, start)| *start <= at && at <= start + found.len())
        else {
            return Vec::new();
        };
        let manifest = |folder: &Extension| {
            folder.manifest.clone().map(|path| Target { path, range: Range::default() }).into_iter().collect()
        };
        match (*key, folder) {
            ("option", Some(folder)) => manifest(folder),
            ("view", Some(folder)) => {
                let Some(namespace) = folder.namespace.as_deref() else { return Vec::new() };
                let base = format!("{namespace}\\View\\{}", studly(found));
                let classes = ["HtmlView", "FeedView", "JsonView", "RawView"];
                classes.iter().flat_map(|class| cx.engine.class_targets(&format!("{base}\\{class}"))).take(1).collect()
            }
            ("task", Some(folder)) => {
                let Some(namespace) = folder.namespace.as_deref() else { return Vec::new() };
                let (controller, task) = found.split_once('.').unwrap_or(("display", found));
                let class = format!("{namespace}\\Controller\\{}Controller", studly(controller));
                let method = method_target(cx, &class, task);
                method.map_or_else(|| cx.engine.class_targets(&class), |target| vec![target])
            }
            _ => Vec::new(),
        }
    }

    fn targets(&self, cx: &RefContext<'_>, value: &str, at: usize) -> Vec<Target> {
        let project = Self::project(cx);
        let place = |file: &Path, range: Range| Target { path: file.to_path_buf(), range };
        match self.source {
            Source::Language => {
                let found = Self::own_first(cx, &project, project.constant(value), |def| &def.file);
                found.into_iter().map(|def| place(&def.file, def.range)).collect()
            }
            Source::Layout => project.layout(value).into_iter().map(|file| place(file, Range::default())).collect(),
            Source::Html => {
                let (class, method) = Self::html_target(value);
                method_target(cx, &class, &method).into_iter().collect()
            }
            Source::Asset => {
                let found = project.assets.iter().filter(|asset| asset.name == value);
                found.map(|asset| place(&asset.file, asset.range)).collect()
            }
            Source::Extension => {
                let found = project.extension(value);
                let mut out: Vec<Target> = found
                    .iter()
                    .filter_map(|folder| folder.manifest.as_deref())
                    .map(|file| place(file, Range::default()))
                    .collect();
                out.dedup();
                out
            }
            Source::Event => project.event_class(value).map(|class| cx.engine.class_targets(class)).unwrap_or_default(),
            Source::Handler => match Self::returning_class(cx) {
                Some(class) => method_target(cx, &class, value).into_iter().collect(),
                None => Vec::new(),
            },
            Source::Action => {
                let found: Vec<_> = project.actions.iter().filter(|action| action.name == value).collect();
                let found = Self::own_first(cx, &project, found, |def| &def.file);
                found.into_iter().map(|def| place(&def.file, def.range)).take(1).collect()
            }
            Source::Form => {
                let found: Vec<_> = project.forms.iter().filter(|form| !form.params && form.name == value).collect();
                let found = Self::own_first(cx, &project, found, |form| &form.file);
                found.into_iter().map(|form| place(&form.file, Range::default())).take(1).collect()
            }
            Source::Field | Source::Param => {
                let params = self.source == Source::Param;
                let mut found: Vec<(&Path, Range)> = Vec::new();
                for form in project.forms.iter().filter(|form| form.params == params) {
                    found.extend(
                        form.fields.iter().filter(|field| field.0 == value).map(|field| (form.file.as_path(), field.2)),
                    );
                }
                let own = Self::own(cx, &project);
                let inside: Vec<_> = found
                    .iter()
                    .filter(|(file, _)| own.iter().any(|folder| file.starts_with(&folder.path)))
                    .copied()
                    .collect();
                let found = if inside.is_empty() { found } else { inside };
                found.into_iter().map(|(file, range)| place(file, range)).collect()
            }
            Source::Table => {
                project.table(table_name(value)).map(|table| place(&table.file, table.range)).into_iter().collect()
            }
            Source::Column => {
                let table = cx.site.as_ref().and_then(|site| site.first_argument.clone());
                let table = table.as_deref().and_then(|table| project.table(table_name(table)));
                let column =
                    table.and_then(|table| Some((table, table.columns.iter().find(|column| column.0 == value)?)));
                column.map(|(table, column)| place(&table.file, column.2)).into_iter().collect()
            }
            Source::Route => Self::route_targets(cx, &project, value, at),
            Source::Template => {
                let files = Self::sub_templates(cx, &project);
                let found = files.into_iter().filter(|(name, _)| name == value);
                found.map(|(_, file)| place(&file, Range::default())).collect()
            }
        }
    }

    /// The sub-templates of the view template the string is in: for
    /// `tmpl/article/default.php`, each `default_<name>.php` beside it and
    /// in a template's override of the view, by its name.
    fn sub_templates(cx: &RefContext<'_>, project: &JoomlaProject) -> Vec<(String, PathBuf)> {
        let Some(site) = cx.site.as_ref() else { return Vec::new() };
        let Some(folder) = site.path.parent() else { return Vec::new() };
        let Some(stem) = site.path.file_stem().map(|stem| stem.to_string_lossy().into_owned()) else {
            return Vec::new();
        };
        let layout = stem.split('_').next().unwrap_or(&stem).to_owned();
        let mut folders = vec![folder.to_path_buf()];
        // A template's override of this view, and the view a template's override is of.
        if let Some((extension, view, _)) = project.view_template(&site.path) {
            for template in
                project.extensions.iter().filter(|other| other.kind == super::project::ExtensionKind::Template)
            {
                folders.insert(0, template.path.join("html").join(&extension.name).join(&view));
            }
        }
        let mut out = Vec::new();
        for folder in folders {
            let mut files: Vec<PathBuf> =
                std::fs::read_dir(&folder).into_iter().flatten().flatten().map(|entry| entry.path()).collect();
            files.sort();
            for file in files {
                let Some(name) = file.file_name().map(|name| name.to_string_lossy().into_owned()) else { continue };
                if let Some(sub) = name.strip_prefix(&format!("{layout}_")).and_then(|rest| rest.strip_suffix(".php")) {
                    out.push((sub.to_owned(), file));
                }
            }
        }
        out
    }

    /// The names completion offers for what is written before the caret.
    fn names(&self, cx: &RefContext<'_>, typed: &str) -> Vec<(String, Option<String>)> {
        let project = Self::project(cx);
        let own = Self::own(cx, &project);
        let mut out: Vec<(String, Option<String>)> = match self.source {
            Source::Language => {
                // By what is typed; before anything is, the constants of the file's extension.
                let prefix = match (typed.is_empty(), own.first()) {
                    (true, Some(extension)) => extension.language_prefix(),
                    _ => typed.to_ascii_uppercase(),
                };
                let found = project.constants.iter().filter(|def| def.name.starts_with(&prefix));
                found.take(2000).map(|def| (def.name.clone(), def.detail.clone())).collect()
            }
            Source::Layout => project.layouts.iter().map(|(id, _)| (id.clone(), None)).collect(),
            Source::Html => Vec::new(),
            Source::Asset => {
                project.assets.iter().map(|asset| (asset.name.clone(), Some(asset.kind.clone()))).collect()
            }
            Source::Extension => {
                let named = project.extensions.iter().map(|folder| match (&folder.group, folder.kind) {
                    (Some(group), _) => (folder.name.clone(), Some(format!("{group} plugin"))),
                    _ => (folder.name.clone(), None),
                });
                named.collect()
            }
            Source::Event => project.events.iter().map(|(name, class)| (name.clone(), Some(class.clone()))).collect(),
            Source::Handler => {
                let class = Self::returning_class(cx);
                let model = class.as_deref().and_then(|class| cx.engine.resolver().class_model(class));
                let methods: Vec<String> = model
                    .map(|model| model.class.methods.iter().map(|method| method.name.clone()).collect())
                    .unwrap_or_default();
                methods.into_iter().filter(|name| !name.starts_with("__")).map(|name| (name, None)).collect()
            }
            Source::Action => project.actions.iter().map(|def| (def.name.clone(), def.detail.clone())).collect(),
            Source::Form | Source::Field | Source::Param => {
                let inside = |file: &Path| own.is_empty() || own.iter().any(|folder| file.starts_with(&folder.path));
                let forms = project.forms.iter().filter(|form| inside(&form.file));
                match self.source {
                    Source::Form => forms.filter(|form| !form.params).map(|form| (form.name.clone(), None)).collect(),
                    source => forms
                        .filter(|form| form.params == (source == Source::Param))
                        .flat_map(|form| form.fields.iter().map(|field| (field.0.clone(), Some(field.1.clone()))))
                        .collect(),
                }
            }
            Source::Table => project.tables.iter().map(|table| (table.name.clone(), None)).collect(),
            Source::Column => {
                let table = cx.site.as_ref().and_then(|site| site.first_argument.clone());
                let table = table.as_deref().and_then(|table| project.table(table_name(table)));
                let columns = table.map(|table| table.columns.as_slice()).unwrap_or_default();
                columns.iter().map(|column| (column.0.clone(), Some(column.1.clone()))).collect()
            }
            Source::Route => Vec::new(),
            Source::Template => Self::sub_templates(cx, &project).into_iter().map(|(name, _)| (name, None)).collect(),
        };
        out.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.is_some().cmp(&a.1.is_some())));
        out.dedup_by(|b, a| a.0 == b.0);
        out
    }

    fn what(&self) -> Option<What> {
        Some(match self.source {
            Source::Language => What::Language,
            Source::Layout => What::Layout,
            Source::Asset => What::Asset,
            Source::Action => What::Action,
            Source::Extension => What::Extension,
            _ => return None,
        })
    }
}

impl ReferenceKind for JoomlaName {
    fn id(&self) -> &'static str {
        self.id
    }

    fn patterns(&self) -> &'static [PlacePattern] {
        self.patterns
    }

    fn canonical(&self, cx: &RefContext<'_>, value: &str) -> String {
        match self.source {
            Source::Language => value.to_ascii_uppercase(),
            Source::Table => table_name(value).to_owned(),
            Source::Column => match cx.site.as_ref().and_then(|site| site.first_argument.clone()) {
                Some(table) => format!("{}{SEPARATOR}{value}", table_name(&table)),
                None => value.to_owned(),
            },
            Source::Handler => match Self::returning_class(cx) {
                Some(class) => format!("{class}{SEPARATOR}{value}"),
                None => value.to_owned(),
            },
            _ => value.to_owned(),
        }
    }

    fn usage_text<'v>(&self, value: &'v str) -> &'v str {
        match self.source {
            Source::Column | Source::Handler => value.rsplit(SEPARATOR).next().unwrap_or(value),
            _ => value,
        }
    }

    fn usage_words(&self, value: &str) -> Vec<String> {
        crate::index::words(self.usage_text(value))
    }

    fn refers_to(&self, value: &str, key: &str) -> bool {
        match self.source {
            Source::Language => value.eq_ignore_ascii_case(key),
            _ => value == key,
        }
    }

    fn complete(&self, cx: &RefContext<'_>, value: &str, at: usize) -> Vec<RefItem> {
        let typed = value.get(..at.min(value.len())).unwrap_or_default();
        self.names(cx, typed)
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

    /// The XML and asset files naming `key`.
    fn other_usages(&self, cx: &RefContext<'_>, key: &str) -> Vec<(PathBuf, Range)> {
        let Some(what) = self.what() else { return Vec::new() };
        let project = Self::project(cx);
        let found =
            project.mentions().filter(|(_, mention)| mention.what == what && !mention.declares && mention.name == key);
        found.map(|(file, mention)| (file.file.clone(), mention.range)).collect()
    }

    /// A language constant is renamed where the project's own files
    /// declare it.
    fn rename_target(&self, cx: &RefContext<'_>, value: &str, _at: usize) -> Option<RenameTarget> {
        if self.source != Source::Language {
            return None;
        }
        let project = Self::project(cx);
        let key = value.to_ascii_uppercase();
        let declarations: Vec<(PathBuf, Range)> = project
            .constant(&key)
            .into_iter()
            .filter(|def| !cx.engine.is_library(&def.file))
            .map(|def| (def.file.clone(), def.range))
            .collect();
        (!declarations.is_empty()).then_some(RenameTarget { range: (0, key.len()), key, declarations })
    }

    fn valid_name(&self, name: &str) -> bool {
        self.source == Source::Language && ini::is_constant(name)
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
            Source::Language => {
                // Only a constant of the file's own extension, where it has language files.
                let extension = Self::site_extension(cx, &project)?;
                let prefix = format!("{}_", extension.language_prefix());
                let declares = project.constants.iter().any(|def| def.name.starts_with(&prefix));
                (ini::is_constant(value)
                    && value.starts_with(&prefix)
                    && declares
                    && project.constant(value).is_empty())
                .then(|| Undefined::warning(format!("Unknown language constant '{value}'")))
            }
            Source::Layout => {
                let plain = value.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
                let has = project.layouts.iter().any(|(id, _)| id.starts_with("joomla."));
                (plain && value.starts_with("joomla.") && has && project.layout(value).is_empty())
                    .then(|| Undefined::warning(format!("Unknown layout '{value}'")))
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
            Source::Language => {
                let found = Self::own_first(cx, &project, project.constant(value), |def| &def.file);
                found.first().and_then(|def| def.detail.clone())
            }
            Source::Asset => project.assets.iter().find(|asset| asset.name == value).map(|asset| {
                let needs: Vec<&str> = asset.dependencies.iter().map(|(name, _)| name.as_str()).collect();
                if needs.is_empty() {
                    asset.kind.clone()
                } else {
                    format!("{}, needs {}", asset.kind, needs.join(", "))
                }
            }),
            Source::Event => project.event_class(value).map(str::to_owned),
            Source::Action => project.actions.iter().find(|def| def.name == value).and_then(|def| def.detail.clone()),
            Source::Html => {
                let (class, method) = Self::html_target(value);
                Some(format!("{class}::{method}()"))
            }
            Source::Field | Source::Param => {
                let params = self.source == Source::Param;
                let forms = project.forms.iter().filter(|form| form.params == params);
                let mut fields = forms.flat_map(|form| form.fields.iter());
                fields.find(|field| field.0 == value).map(|field| field.1.clone()).filter(|ty| !ty.is_empty())
            }
            Source::Column => {
                let table = cx.site.as_ref().and_then(|site| site.first_argument.clone());
                let table = table.as_deref().and_then(|table| project.table(table_name(table)));
                table
                    .and_then(|table| table.columns.iter().find(|column| column.0 == value))
                    .map(|column| column.1.clone())
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
