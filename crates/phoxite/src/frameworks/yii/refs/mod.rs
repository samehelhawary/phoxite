//! Yii 2's reference kinds ([`kinds`]), on the machinery every framework
//! shares ([`crate::frameworks::refs`]). All are one type, [`YiiName`].
//!
//! | kind | names | where |
//! | --- | --- | --- |
//! | `yiiView` | a view's file, found as Yii finds it from who asks ([`YiiProject::view_file`]) | a controller's, a view's and a widget's `render()`, `renderPartial()`, `renderAjax()` |
//! | `yiiLayout` | a file of the `layouts` of the module's views | the `$layout` of a controller or a module |
//! | `yiiMail` | a view of `@app/mail` | a mailer's `compose()` (one view, or `['html' => …, 'text' => …]`) |
//! | `yiiRoute` | an action: `post/view` is `PostController::actionView()`, `view` an action of the controller asking, `admin/user/index` one of the module `admin` or of the folder `controllers/admin` | `Url::to([...])`, `Url::toRoute()`, `redirect([...])`, `Html::a($text, [...])`, `Html::beginForm([...])`, `createUrl()`, `'url' => [...]`, `'defaultRoute'`, `'loginUrl'` |
//! | `yiiComponent` | a key of `components` of the configuration | `get()`, `has()` of an application or a module |
//! | `yiiModule` | a key of `modules` | `getModule()`, `hasModule()` |
//! | `yiiParam` | a key of `params` | `Yii::$app->params['adminEmail']` |
//! | `yiiAlias` | a path alias: the file an aliased path names, else where the alias is declared | `Yii::getAlias()`, `Yii::setAlias()`, `renderFile()`, `beginContent()` |
//! | `yiiCategory` | a message file: `app` is `messages/<language>/app.php` | `Yii::t('app', …)` |
//! | `yiiMessage` | a message of that category's files | `Yii::t(…, 'Hello')` |
//! | `yiiAttribute` | a property of the model (declared, documented with `@property`, or read through `getName()`) | the attributes of a rule of `rules()`, the keys of `attributeLabels()` and `attributeHints()`, the lists of `scenarios()`, `getAttributeLabel()`, `hasErrors()`, `addError()`, `getAttribute()`… |
//!
//! Each has completion, goto, hover and Find References. Nothing is
//! renamed: a view's and a route's names are files' and methods', and the
//! others are shared with the configuration.
//!
//! A view whose file isn't there, a route whose controller has no such
//! action and an attribute a form model doesn't have are reported
//! ([`ReferenceKind::undefined`]).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use mago_span::HasSpan;
use mago_syntax::cst::{Argument, ArrayElement, Call, Expression, Method, Return};
use mago_syntax::walker::Walker;

use crate::base::Range;
use crate::frameworks::laravel::refs::controller_action::method_target;
use crate::frameworks::php_array::{elements, is_array, string_literal};
use crate::frameworks::refs::{
    CallTarget, CalleePattern, PlacePattern, Position, RefContext, RefItem, ReferenceKind, TEMPLATE_THIS, Undefined,
};
use crate::frameworks::yii::YiiProject;
use crate::frameworks::yii::migrations;
use crate::frameworks::yii::project::{Def, id_of, name_of};
use crate::index::path_key;
use crate::navigate::Target;
use crate::summary::{FileSummary, Visibility};

const BACKSLASH: char = '\\';

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Source {
    View,
    Layout,
    Mail,
    Route,
    Component,
    Module,
    Alias,
    Category,
    Message,
    Attribute,
    Param,
}

/// One of the kinds of the module documentation.
pub struct YiiName {
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

const YII: &[&str] = &["yii\\BaseYii"];
const CONTROLLERS: &[&str] = &["yii\\base\\Controller"];
const VIEWS: &[&str] = &["yii\\base\\View", TEMPLATE_THIS];
const WIDGETS: &[&str] = &["yii\\base\\Widget"];
const MODULES: &[&str] = &["yii\\base\\Module"];
const LOCATORS: &[&str] = &["yii\\di\\ServiceLocator"];
const MAILERS: &[&str] = &["yii\\mail\\MailerInterface", "yii\\mail\\BaseMailer"];
const URL: &[&str] = &["yii\\helpers\\BaseUrl"];
const HTML: &[&str] = &["yii\\helpers\\BaseHtml"];
const URL_MANAGER: &[&str] = &["yii\\web\\UrlManager"];
const MODELS: &[&str] = &["yii\\base\\Model"];
const RECORDS: &[&str] = &["yii\\db\\BaseActiveRecord"];
const RECORD: &str = "yii\\db\\BaseActiveRecord";
const RENDERING: &[&str] = &["render", "renderPartial", "renderAjax"];

pub static VIEW: YiiName = YiiName {
    id: "yiiView",
    label: "view",
    source: Source::View,
    patterns: &[
        method(CONTROLLERS, RENDERING, 0, "view"),
        method(VIEWS, &["render", "renderAjax"], 0, "view"),
        method(WIDGETS, &["render"], 0, "view"),
    ],
};

pub static LAYOUT: YiiName = YiiName {
    id: "yiiLayout",
    label: "layout",
    source: Source::Layout,
    patterns: &[
        PlacePattern::argument(CalleePattern::Property { classes: CONTROLLERS, names: &["layout"] }, 0, "layout"),
        PlacePattern::argument(CalleePattern::Property { classes: MODULES, names: &["layout"] }, 0, "layout"),
    ],
};

pub static MAIL: YiiName = YiiName {
    id: "yiiMail",
    label: "mail view",
    source: Source::Mail,
    patterns: &[
        method(MAILERS, &["compose"], 0, "view"),
        method(MAILERS, &["compose"], 0, "view").at(Position::ArrayValue),
    ],
};

pub static ROUTE: YiiName = YiiName {
    id: "yiiRoute",
    label: "route",
    source: Source::Route,
    patterns: &[
        method(URL, &["to", "toRoute"], 0, "url").at(Position::ListValue),
        method(URL, &["toRoute"], 0, "route"),
        method(CONTROLLERS, &["redirect"], 0, "url").at(Position::ListValue),
        method(CONTROLLERS, &["run"], 0, "route"),
        method(MODULES, &["runAction"], 0, "route"),
        method(HTML, &["a"], 1, "url").at(Position::ListValue),
        method(HTML, &["beginForm"], 0, "action").at(Position::ListValue),
        method(URL_MANAGER, &["createUrl", "createAbsoluteUrl"], 0, "params"),
        method(URL_MANAGER, &["createUrl", "createAbsoluteUrl"], 0, "params").at(Position::ListValue),
        // A menu's items, a form's action, the configuration.
        entry("url").at(Position::ListValue),
        entry("loginUrl").at(Position::ListValue),
        entry("defaultRoute"),
        // The URL manager's rules: `'posts' => 'post/index'`, `['pattern' => 'posts', 'route' => 'post/index']`.
        entry("rules").at(Position::ArrayValue),
        entry("route"),
        // The actions a filter is about: `'only' => ['logout']`, `'actions' => ['delete' => ['post']]`.
        entry("only").at(Position::ListValue),
        entry("except").at(Position::ListValue),
        entry("actions").at(Position::ListValue),
        entry("actions").at(Position::ArrayKey),
    ],
};

pub static COMPONENT: YiiName = YiiName {
    id: "yiiComponent",
    label: "component",
    source: Source::Component,
    patterns: &[method(LOCATORS, &["get", "has", "clear"], 0, "id")],
};

pub static MODULE: YiiName = YiiName {
    id: "yiiModule",
    label: "module",
    source: Source::Module,
    patterns: &[method(MODULES, &["getModule", "hasModule"], 0, "id")],
};

pub static ALIAS: YiiName = YiiName {
    id: "yiiAlias",
    label: "alias",
    source: Source::Alias,
    patterns: &[
        method(YII, &["getAlias", "setAlias", "getRootAlias"], 0, "alias"),
        method(VIEWS, &["renderFile", "beginContent"], 0, "viewFile"),
    ],
};

pub static CATEGORY: YiiName = YiiName {
    id: "yiiCategory",
    label: "message category",
    source: Source::Category,
    patterns: &[method(YII, &["t"], 0, "category")],
};

pub static MESSAGE: YiiName = YiiName {
    id: "yiiMessage",
    label: "message",
    source: Source::Message,
    patterns: &[method(YII, &["t"], 1, "message")],
};

pub static PARAM: YiiName = YiiName {
    id: "yiiParam",
    label: "parameter",
    source: Source::Param,
    patterns: &[PlacePattern::argument(CalleePattern::Read { classes: MODULES, names: &["params"] }, 0, "key")],
};

/// Between the keys of a parameter below another, in what its usages share.
const BELOW: char = '\u{1f}';

const fn returned(names: &'static [&'static str]) -> PlacePattern {
    PlacePattern::argument(CalleePattern::Return { classes: MODELS, names }, 0, "value")
}

pub static ATTRIBUTE: YiiName = YiiName {
    id: "yiiAttribute",
    label: "attribute",
    source: Source::Attribute,
    patterns: &[
        returned(&["attributeLabels", "attributeHints"]).at(Position::ArrayKey),
        // A rule's attributes: which strings of `rules()` they are is looked at again.
        returned(&["rules"]).at(Position::ListValue).nested(),
        returned(&["scenarios"]).at(Position::ListValue).nested(),
        method(
            MODELS,
            &[
                "getAttributeLabel",
                "getAttributeHint",
                "hasErrors",
                "getErrors",
                "getFirstError",
                "addError",
                "clearErrors",
                "isAttributeRequired",
                "isAttributeSafe",
                "isAttributeActive",
            ],
            0,
            "attribute",
        ),
        method(
            RECORDS,
            &[
                "getAttribute",
                "setAttribute",
                "hasAttribute",
                "getOldAttribute",
                "isAttributeChanged",
                "markAttributeDirty",
            ],
            0,
            "name",
        ),
    ],
};

/// Yii's kinds.
pub fn kinds() -> &'static [&'static dyn ReferenceKind] {
    static KINDS: [&dyn ReferenceKind; 11] =
        [&VIEW, &LAYOUT, &MAIL, &ROUTE, &COMPONENT, &MODULE, &ALIAS, &CATEGORY, &MESSAGE, &ATTRIBUTE, &PARAM];
    &KINDS
}

/// An action a route names.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Action {
    /// The controller's file.
    file: PathBuf,
    /// The controller's short name (`PostController`).
    controller: String,
    /// `view-all`.
    action: String,
}

impl Action {
    fn method(&self) -> String {
        format!("action{}", name_of(&self.action))
    }
}

/// The index of each array element on the way from `array` to the string at
/// `offset`, and whether the element has a key.
fn chain(array: &Expression<'_>, offset: u32, out: &mut Vec<(usize, bool)>) {
    let mut index = 0;
    for element in elements(array) {
        let (value, keyed) = match element {
            ArrayElement::KeyValue(element) => (element.value, true),
            ArrayElement::Value(element) => (element.value, false),
            _ => continue,
        };
        let span = value.span();
        if span.start.offset <= offset && offset <= span.end.offset {
            out.push((index, keyed));
            chain(value, offset, out);
            return;
        }
        if !keyed {
            index += 1;
        }
    }
}

/// The outermost returned array around an offset.
struct Returned<'a> {
    offset: u32,
    found: Option<&'a Expression<'a>>,
}

struct ReturnWalker;

impl<'a> Walker<'a, 'a, Returned<'a>> for ReturnWalker {
    fn walk_in_return(&self, r#return: &'a Return<'a>, cx: &mut Returned<'a>) {
        let Some(value) = r#return.value else { return };
        let span = value.span();
        if cx.found.is_none() && span.start.offset <= cx.offset && cx.offset <= span.end.offset {
            cx.found = Some(value);
        }
    }
}

/// The keys of the array `actions()` returns.
struct Listed {
    inside: bool,
    keys: Vec<(String, u32, u32)>,
    /// Every action is a key read here: one array, written out.
    complete: bool,
    returned: bool,
}

struct ListedWalker;

impl<'a> Walker<'a, 'a, Listed> for ListedWalker {
    fn walk_in_method(&self, method: &'a Method<'a>, cx: &mut Listed) {
        cx.inside = method.name.value.eq_ignore_ascii_case(b"actions");
    }

    fn walk_out_method(&self, _: &'a Method<'a>, cx: &mut Listed) {
        cx.inside = false;
    }

    fn walk_in_return(&self, r#return: &'a Return<'a>, cx: &mut Listed) {
        let Some(value) = r#return.value.filter(|_| cx.inside) else { return };
        let mut arrays: Vec<&Expression<'_>> = Vec::new();
        if is_array(value) && !cx.returned {
            arrays.push(value);
        } else {
            // `array_merge(parent::actions(), [...])`: the arrays written out, and more.
            cx.complete = false;
            if let Expression::Call(Call::Function(call)) = value {
                arrays.extend(call.argument_list.arguments.iter().filter_map(|argument| match argument {
                    Argument::Positional(argument) if is_array(argument.value) => Some(argument.value),
                    _ => None,
                }));
            }
        }
        cx.returned = true;
        for array in arrays {
            for element in elements(array) {
                match element {
                    ArrayElement::KeyValue(element) => match string_literal(element.key) {
                        Some(key) => cx.keys.push(key),
                        None => cx.complete = false,
                    },
                    _ => cx.complete = false,
                }
            }
        }
    }
}

impl YiiName {
    fn project(cx: &RefContext<'_>) -> Arc<YiiProject> {
        cx.engine.yii()
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

    /// Whether the string of `rules()` at the site is an attribute of a
    /// rule: its first element, or an item of the list its first element
    /// is. (The validator's name and its options aren't.)
    fn rule_attribute(cx: &RefContext<'_>) -> bool {
        let Some(CallTarget::Return { method, .. }) = Self::place(cx) else { return true };
        if !method.eq_ignore_ascii_case("rules") {
            return true;
        }
        let (Some(site), Some(text)) = (cx.site.as_ref(), Self::site_text(cx)) else { return false };
        let parsed = crate::syntax::ParsedFile::parse(&site.path.to_string_lossy(), text);
        let mut returned = Returned { offset: site.start, found: None };
        ReturnWalker.walk_program(parsed.program(), &mut returned);
        let Some(array) = returned.found else { return false };
        let mut steps = Vec::new();
        chain(array, site.start, &mut steps);
        matches!(steps.as_slice(), [(_, false), (0, false)] | [(_, false), (0, false), (_, false)])
    }

    /// The file a view, a layout or a mail view names from the site.
    fn file(&self, cx: &RefContext<'_>, value: &str) -> Option<PathBuf> {
        let project = Self::project(cx);
        let site = Self::site_path(cx)?;
        match self.source {
            Source::View => project.view_file(&site, value),
            Source::Layout => project.layout_file(&site, value),
            Source::Mail if value.starts_with(['@', '/']) => project.view_file(&site, value),
            Source::Mail => {
                let file = crate::frameworks::php_array::tidy(&project.mail_dir(&site).join(value));
                Some(if file.extension().is_some() { file } else { file.with_extension("php") })
            }
            _ => None,
        }
    }

    /// The folder the names of this kind are files of, from the site.
    fn folder(&self, cx: &RefContext<'_>) -> Option<PathBuf> {
        let project = Self::project(cx);
        let site = Self::site_path(cx)?;
        match self.source {
            Source::View => project.view_dir(&site),
            Source::Layout => Some(project.views_root(&site).join("layouts")),
            Source::Mail => Some(project.mail_dir(&site)),
            _ => None,
        }
    }

    /// The fully qualified name of the class `short` the file `file`
    /// declares.
    fn class_in(cx: &RefContext<'_>, file: &Path, short: &str) -> Option<String> {
        let wanted = path_key(file).to_ascii_lowercase();
        let found = cx.engine.index()?.by_short_name(short).ok()?;
        found.into_iter().find(|decl| decl.file.to_ascii_lowercase() == wanted).map(|decl| decl.fqn)
    }

    /// The folders that have a `controllers` folder of their own, with the
    /// route prefix of each: the applications, then the modules.
    fn controller_roots(cx: &RefContext<'_>) -> Vec<(String, PathBuf)> {
        let project = Self::project(cx);
        let mut roots: Vec<(String, PathBuf)> = project.apps.iter().map(|app| (String::new(), app.clone())).collect();
        for module in &project.modules {
            let Some(class) = module.detail.as_deref() else { continue };
            let folder = cx
                .engine
                .class_targets(class)
                .into_iter()
                .next()
                .and_then(|target| target.path.parent().map(Path::to_path_buf));
            if let Some(folder) = folder {
                roots.push((format!("{}/", module.name), folder));
            }
        }
        roots
    }

    /// The controller the file `site` is, or is a view of: its file and its
    /// short name.
    fn own_controller(site: &Path) -> Option<(PathBuf, String)> {
        let stem = site.file_stem()?.to_str()?;
        let folder = site.parent()?;
        let parts: Vec<&std::ffi::OsStr> = folder.components().map(|part| part.as_os_str()).collect();
        if stem.ends_with("Controller") && !parts.iter().any(|part| *part == "views") {
            return Some((site.to_path_buf(), stem.to_owned()));
        }
        // `views/<folders>/<id>/file.php` is of `controllers/<folders>/<Id>Controller.php`.
        let at = parts.iter().rposition(|part| *part == "views")?;
        let (id, between) = parts[at + 1..].split_last()?;
        let name = format!("{}Controller", name_of(id.to_str()?));
        let mut file: PathBuf = parts[..at].iter().collect();
        file.push("controllers");
        file.extend(between);
        file.push(format!("{name}.php"));
        Some((file, name))
    }

    /// The action the route `value` names from the site, as Yii's guide
    /// resolves a route: an action of the controller asking, else
    /// `[module/][folders/]controller/action`.
    fn action(cx: &RefContext<'_>, value: &str) -> Option<Action> {
        // Several applications may each have the controller (the advanced template's
        // `frontend` and `backend`): the one that has the action.
        let mut found = Self::actions(cx, value);
        let has = |action: &Action| {
            let Some(class) = Self::class_in(cx, &action.file, &action.controller) else { return false };
            cx.engine.resolver().class_model(&class).is_some_and(|model| model.method(&action.method()).is_some())
                || Self::listed_actions(cx, &class).0.iter().any(|(key, _)| *key == action.action)
        };
        let at = if found.len() > 1 { found.iter().position(has).unwrap_or(0) } else { 0 };
        (at < found.len()).then(|| found.swap_remove(at))
    }

    /// The actions the route `value` may name from the site, the likeliest
    /// first.
    fn actions(cx: &RefContext<'_>, value: &str) -> Vec<Action> {
        Self::route_actions(cx, value).unwrap_or_default()
    }

    fn route_actions(cx: &RefContext<'_>, value: &str) -> Option<Vec<Action>> {
        let plain = |c: char| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '/');
        if value.is_empty() || !value.chars().all(plain) {
            return None;
        }
        let project = Self::project(cx);
        let site = Self::site_path(cx).map(|site| project.logical(&site));
        let roots = Self::controller_roots(cx);
        if !value.contains('/') {
            let site = site.as_deref()?;
            let (file, controller) = Self::own_controller(site)?;
            if file.is_file() {
                return Some(vec![Action { file, controller, action: value.to_owned() }]);
            }
            // A view of a controller `controllerMap` names: `views/<id>/`.
            let folder = site.parent()?;
            let mapped = roots.iter().find_map(|(prefix, root)| {
                let id = folder.strip_prefix(root.join("views")).ok()?.to_string_lossy().replace(BACKSLASH, "/");
                Self::mapped(cx, &project, &format!("{prefix}{id}"), value)
            });
            return Some(mapped.into_iter().collect());
        }
        let segments: Vec<&str> = value.split('/').filter(|segment| !segment.is_empty()).collect();
        // The module asking: a route without a leading slash is its own first.
        let own = site.as_deref().filter(|_| !value.starts_with('/')).and_then(|site| {
            let modules = roots.iter().filter(|(prefix, folder)| !prefix.is_empty() && site.starts_with(folder));
            modules.max_by_key(|(_, folder)| folder.as_os_str().len())
        });
        // The whole route is `controller/action`, else it names a controller and its default action.
        let last = segments.len().checked_sub(1)?;
        let mut out = Vec::new();
        for (controller, action) in [(last, segments[last]), (segments.len(), "index")] {
            let Some((id, folders)) = segments[..controller].split_last() else { continue };
            // `controllerMap` first, as Yii looks: the route's own, then the asking module's.
            let written = segments[..controller].join("/");
            let own_prefix = own.map(|(prefix, _)| prefix.as_str()).unwrap_or_default();
            let mapped = Self::mapped(cx, &project, &written, action)
                .or_else(|| Self::mapped(cx, &project, &format!("{own_prefix}{written}"), action));
            if let Some(mapped) = mapped {
                out.push(mapped);
                continue;
            }
            let name = format!("{}Controller", name_of(id));
            let mut candidates: Vec<(&PathBuf, &[&str])> = Vec::new();
            candidates.extend(own.map(|(_, folder)| (folder, folders)));
            for (prefix, folder) in &roots {
                let module: Vec<&str> = prefix.split('/').filter(|part| !part.is_empty()).collect();
                if module.is_empty() {
                    continue;
                }
                if folders.starts_with(&module) {
                    candidates.push((folder, &folders[module.len()..]));
                }
            }
            // The applications, the asking file's first.
            let inside = |folder: &PathBuf| site.as_deref().is_some_and(|site| site.starts_with(folder));
            let apps = roots.iter().filter(|(prefix, _)| prefix.is_empty());
            let (own_app, other_apps): (Vec<_>, Vec<_>) = apps.partition(|(_, folder)| inside(folder));
            candidates.extend(own_app.into_iter().chain(other_apps).map(|(_, folder)| (folder, folders)));
            for (root, below) in candidates {
                let mut file = root.join("controllers");
                file.extend(below);
                file.push(format!("{name}.php"));
                if file.is_file() && !out.iter().any(|known: &Action| known.file == file) {
                    out.push(Action { file, controller: name.clone(), action: action.to_owned() });
                }
            }
        }
        Some(out)
    }

    /// The action `action` of the controller `controllerMap` gives the id `id`.
    fn mapped(cx: &RefContext<'_>, project: &YiiProject, id: &str, action: &str) -> Option<Action> {
        let class = project.mapped_controller(id)?.detail.clone()?;
        let file = cx.engine.class_targets(&class).into_iter().next()?.path;
        let controller = class.rsplit(BACKSLASH).next().unwrap_or(&class).to_owned();
        Some(Action { file, controller, action: action.to_owned() })
    }

    /// The actions the `actions()` of the controller `class` (or of a
    /// class of the project it extends) lists, each with its key, and
    /// whether those are all it lists.
    fn listed_actions(cx: &RefContext<'_>, class: &str) -> (Vec<(String, Target)>, bool) {
        let resolver = cx.engine.resolver();
        let Some(model) = resolver.class_model(class) else { return (Vec::new(), false) };
        let Some(method) = model.method("actions").filter(|method| {
            !method.declaring.fqn.trim_start_matches(BACKSLASH).to_ascii_lowercase().starts_with("yii")
        }) else {
            return (Vec::new(), true);
        };
        let Some(path) = cx.engine.class_path(&method.declaring, Path::new(""), &FileSummary::default()) else {
            return (Vec::new(), false);
        };
        let text = match cx.engine.open_document(&path) {
            Some(document) => Some(document.text.to_string()),
            None => std::fs::read_to_string(&path).ok(),
        };
        let Some(text) = text else { return (Vec::new(), false) };
        let parsed = crate::syntax::ParsedFile::parse(&path.to_string_lossy(), text);
        let mut listed = Listed { inside: false, keys: Vec::new(), complete: true, returned: false };
        ListedWalker.walk_program(parsed.program(), &mut listed);
        let keys = listed.keys.into_iter().map(|(key, start, end)| {
            (key, Target { path: path.clone(), range: parsed.line_index().range(start, end) })
        });
        (keys.collect(), listed.complete && listed.returned)
    }

    /// Where an action is: its method, its key in `actions()`, else its controller.
    fn action_targets(cx: &RefContext<'_>, action: &Action) -> Vec<Target> {
        let class = Self::class_in(cx, &action.file, &action.controller);
        let method = class.as_deref().and_then(|class| method_target(cx, class, &action.method()));
        let listed = || {
            let (listed, _) = Self::listed_actions(cx, class.as_deref()?);
            listed.into_iter().find(|(key, _)| *key == action.action).map(|(_, target)| target)
        };
        match (method.or_else(listed), class.clone()) {
            (Some(target), _) => vec![target],
            (None, Some(class)) => cx.engine.class_targets(&class),
            (None, None) => vec![Target { path: action.file.clone(), range: Range::default() }],
        }
    }

    /// Every route of the project: `controller/action`, a module's with its id.
    fn routes(cx: &RefContext<'_>) -> Vec<(String, String)> {
        let resolver = cx.engine.resolver();
        let mut out = Vec::new();
        let mut add = |route: String, class: &str, short: &str| {
            let Some(model) = resolver.class_model(class) else { return };
            for method in model.methods.iter() {
                let Some(action) = method.name.strip_prefix("action") else { continue };
                let public = method.declared().is_some_and(|declared| declared.visibility == Visibility::Public);
                if public && action.starts_with(|c: char| c.is_ascii_uppercase()) {
                    out.push((format!("{route}/{}", id_of(action)), short.to_owned()));
                }
            }
            for (action, _) in Self::listed_actions(cx, class).0 {
                out.push((format!("{route}/{action}"), short.to_owned()));
            }
        };
        for def in &Self::project(cx).controller_map {
            let Some(class) = def.detail.as_deref() else { continue };
            add(def.name.clone(), class, class.rsplit(BACKSLASH).next().unwrap_or(class));
        }
        for (prefix, root) in Self::controller_roots(cx) {
            let folder = root.join("controllers");
            let mut files = Vec::new();
            php_files(&folder, &folder, 0, &mut files);
            for relative in files {
                let Some(short) = relative.rsplit('/').next().filter(|name| name.ends_with("Controller")) else {
                    continue;
                };
                let file = folder.join(format!("{relative}.php"));
                let Some(class) = Self::class_in(cx, &file, short) else { continue };
                let folders = relative.rsplit_once('/').map_or(String::new(), |(folders, _)| format!("{folders}/"));
                let controller = id_of(short.trim_end_matches("Controller"));
                add(format!("{prefix}{folders}{controller}"), &class, short);
            }
        }
        out.sort();
        out.dedup();
        out
    }

    /// The model the attribute at the site is of.
    fn model(cx: &RefContext<'_>) -> Option<String> {
        match Self::place(cx)? {
            CallTarget::Return { class, .. } => Some(class.clone()),
            CallTarget::Method { classes, .. } => {
                let resolver = cx.engine.resolver();
                classes
                    .iter()
                    .find(|class| resolver.class_model(class).is_some_and(|model| model.is_subclass_of(MODELS[0])))
                    .cloned()
            }
            _ => None,
        }
    }

    /// The attributes of the model `class`: its own public properties and
    /// those its docblocks document, not the framework's.
    fn attributes(cx: &RefContext<'_>, class: &str) -> Vec<String> {
        let resolver = cx.engine.resolver();
        let Some(model) = resolver.class_model(class) else { return Vec::new() };
        let mut out: Vec<String> = model
            .properties
            .iter()
            .filter(|property| {
                !property.is_static
                    && property.visibility == Visibility::Public
                    && !property.declaring.fqn.trim_start_matches('\\').to_ascii_lowercase().starts_with("yii\\")
            })
            .map(|property| property.name.clone())
            .collect();
        // A record's columns, as the migrations make them.
        if let Some((_, columns)) = migrations::columns_of(&resolver, class) {
            out.extend(columns.into_iter().map(|column| column.name));
        }
        out.sort();
        out.dedup();
        out
    }

    fn attribute_targets(cx: &RefContext<'_>, class: &str, name: &str) -> Vec<Target> {
        let resolver = cx.engine.resolver();
        let Some(model) = resolver.class_model(class) else { return Vec::new() };
        if let Some(property) = model.properties.get(name) {
            let Some(path) = cx.engine.class_path(&property.declaring, Path::new(""), &FileSummary::default()) else {
                return Vec::new();
            };
            let range = match property.declared() {
                Some(declared) => declared.name_loc.range,
                // Documented with `@property`: the class.
                None => property.declaring.name_loc.range,
            };
            return vec![Target { path, range }];
        }
        if let Some((_, columns)) = migrations::columns_of(&resolver, class)
            && let Some(column) = columns.into_iter().find(|column| column.name == name)
        {
            return vec![Target { path: column.file, range: column.range }];
        }
        method_target(cx, class, &format!("get{name}")).into_iter().collect()
    }

    /// Where the configuration declares `name` among `defs`: in the
    /// application the asking file is in when it does (the advanced
    /// template's `frontend` and `backend` each have a configuration), else
    /// wherever it is declared.
    fn own(cx: &RefContext<'_>, project: &YiiProject, defs: &[Def], name: &str) -> Vec<Target> {
        let named: Vec<&Def> = defs.iter().filter(|def| def.name == name).collect();
        let app = Self::site_path(cx).map(|site| project.app_of(&site));
        let inside: Vec<&Def> = match &app {
            Some(app) if project.apps.len() > 1 => {
                named.iter().copied().filter(|def| def.file.starts_with(app)).collect()
            }
            _ => Vec::new(),
        };
        let chosen = if inside.is_empty() { named } else { inside };
        chosen.into_iter().map(|def| Target { path: def.file.clone(), range: def.range }).collect()
    }

    /// The parameters the string at the site is one of: those of `params`,
    /// or those below the keys read before it.
    fn params(cx: &RefContext<'_>, project: &YiiProject) -> Vec<Def> {
        match Self::place(cx) {
            Some(CallTarget::Read { keys, .. }) if !keys.is_empty() => {
                let below = project.nested_params.iter().filter(|(above, _)| above == keys);
                below.map(|(_, def)| def.clone()).collect()
            }
            _ => project.params.clone(),
        }
    }

    fn targets(&self, cx: &RefContext<'_>, value: &str) -> Vec<Target> {
        let project = Self::project(cx);
        let at = |file: &Path, range: Range| Target { path: file.to_path_buf(), range };
        match self.source {
            Source::View | Source::Layout | Source::Mail => {
                let file = self.file(cx, value).filter(|file| file.is_file());
                file.into_iter().map(|path| Target { path, range: Range::default() }).collect()
            }
            Source::Route => Self::action(cx, value).map_or(Vec::new(), |action| Self::action_targets(cx, &action)),
            Source::Component => Self::own(cx, &project, &project.components, value),
            Source::Module => Self::own(cx, &project, &project.modules, value),
            Source::Param => Self::own(cx, &project, &Self::params(cx, &project), value),
            Source::Alias => {
                if !value.starts_with('@') {
                    return Vec::new();
                }
                if let Some(file) = project.alias_path(value).filter(|file| file.is_file()) {
                    return vec![Target { path: file, range: Range::default() }];
                }
                let declared = project.alias_of(value).and_then(|alias| alias.declared.clone());
                declared.into_iter().map(|(path, range)| Target { path, range }).collect()
            }
            Source::Category => {
                let mut out: Vec<Target> = Vec::new();
                for message in project.messages.iter().filter(|message| message.category == value) {
                    if !out.iter().any(|known| known.path == message.file) {
                        out.push(at(&message.file, Range::default()));
                    }
                }
                out
            }
            Source::Message => {
                let category = cx.site.as_ref().and_then(|site| site.first_argument.clone());
                project
                    .messages
                    .iter()
                    .filter(|message| {
                        message.message == value
                            && category.as_ref().is_none_or(|category| message.category == *category)
                    })
                    .map(|message| at(&message.file, message.range))
                    .collect()
            }
            Source::Attribute => {
                if !Self::rule_attribute(cx) {
                    return Vec::new();
                }
                Self::model(cx).map_or(Vec::new(), |class| Self::attribute_targets(cx, &class, value))
            }
        }
    }

    /// The names of this kind, each with a detail.
    fn names(&self, cx: &RefContext<'_>) -> Vec<(String, Option<String>)> {
        let project = Self::project(cx);
        let mut out: Vec<(String, Option<String>)> = match self.source {
            Source::View | Source::Layout | Source::Mail => {
                let Some(folder) = self.folder(cx) else { return Vec::new() };
                let mut files = Vec::new();
                for folder in project.candidates(&folder) {
                    php_files(&folder, &folder, if self.source == Source::Mail { 0 } else { 8 }, &mut files);
                }
                files.sort();
                files.into_iter().map(|name| (name, None)).collect()
            }
            Source::Route => {
                Self::routes(cx).into_iter().map(|(route, controller)| (route, Some(controller))).collect()
            }
            Source::Component => project.components.iter().map(|def| (def.name.clone(), def.detail.clone())).collect(),
            Source::Module => project.modules.iter().map(|def| (def.name.clone(), def.detail.clone())).collect(),
            Source::Param => {
                Self::params(cx, &project).into_iter().map(|def| (def.name.clone(), def.detail.clone())).collect()
            }
            Source::Alias => project.aliases.iter().map(|alias| (alias.name.clone(), alias.path.clone())).collect(),
            Source::Category => project.messages.iter().map(|message| (message.category.clone(), None)).collect(),
            Source::Message => {
                let category = cx.site.as_ref().and_then(|site| site.first_argument.clone());
                project
                    .messages
                    .iter()
                    .filter(|message| category.as_ref().is_none_or(|category| message.category == *category))
                    .map(|message| (message.message.clone(), message.translation.clone()))
                    .collect()
            }
            Source::Attribute => {
                if !Self::rule_attribute(cx) {
                    return Vec::new();
                }
                let class = Self::model(cx);
                class.map_or(Vec::new(), |class| {
                    Self::attributes(cx, &class).into_iter().map(|name| (name, None)).collect()
                })
            }
        };
        let mut seen = std::collections::HashSet::new();
        out.retain(|(name, _)| seen.insert(name.clone()));
        out
    }
}

/// The PHP files below `folder` as names relative to it, without the
/// extension, sorted; `depth` of 8 looks in this folder only.
fn php_files(folder: &Path, below: &Path, depth: usize, out: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(below) else { return };
    let mut entries: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).collect();
    entries.sort();
    for path in entries {
        if out.len() >= 2000 {
            return;
        }
        if path.is_dir() {
            if depth < 8 {
                php_files(folder, &path, depth + 1, out);
            }
        } else if path.extension().is_some_and(|ext| ext == "php")
            && let Ok(relative) = path.strip_prefix(folder)
        {
            out.push(relative.with_extension("").to_string_lossy().replace('\\', "/"));
        }
    }
}

impl ReferenceKind for YiiName {
    fn id(&self) -> &'static str {
        self.id
    }

    fn patterns(&self) -> &'static [PlacePattern] {
        self.patterns
    }

    /// What every usage of one thing is: a view's file from the project's
    /// root, an action as `<controller's file>::<action>`.
    fn canonical(&self, cx: &RefContext<'_>, value: &str) -> String {
        let root = cx.engine.root();
        let shown = |file: &Path| file.strip_prefix(&root).unwrap_or(file).to_string_lossy().replace('\\', "/");
        match self.source {
            Source::View | Source::Layout | Source::Mail => {
                self.file(cx, value).map_or_else(|| value.to_owned(), |file| shown(&file))
            }
            Source::Route => Self::action(cx, value)
                .map_or_else(|| value.to_owned(), |action| format!("{}::{}", shown(&action.file), action.action)),
            Source::Attribute => match Self::model(cx).filter(|_| Self::rule_attribute(cx)) {
                Some(class) => format!("{class}::{value}"),
                None => value.to_owned(),
            },
            Source::Param => match Self::place(cx) {
                Some(CallTarget::Read { keys, .. }) if !keys.is_empty() => {
                    format!("{}{BELOW}{value}", keys.join(&BELOW.to_string()))
                }
                _ => value.to_owned(),
            },
            _ => value.to_owned(),
        }
    }

    /// A view is named by the end of its file's name, an action by its id.
    fn usage_text<'v>(&self, value: &'v str) -> &'v str {
        match self.source {
            Source::View | Source::Layout | Source::Mail => {
                let name = value.rsplit('/').next().unwrap_or(value);
                name.strip_suffix(".php").unwrap_or(name)
            }
            Source::Route | Source::Attribute => value.rsplit("::").next().unwrap_or(value),
            Source::Param => value.rsplit(BELOW).next().unwrap_or(value),
            _ => value,
        }
    }

    fn usage_words(&self, value: &str) -> Vec<String> {
        crate::index::words(self.usage_text(value))
    }

    fn complete(&self, cx: &RefContext<'_>, value: &str, at: usize) -> Vec<RefItem> {
        let at = at.min(value.len());
        // An alias is the first part of its path.
        let to = match self.source {
            Source::Alias => value[at..].find('/').map_or(value.len(), |slash| at + slash),
            _ => value.len(),
        };
        self.names(cx)
            .into_iter()
            .map(|(name, detail)| RefItem {
                label: name.clone(),
                insert: name,
                detail: detail.or_else(|| Some(self.label.to_owned())),
                from: 0,
                to,
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
        match self.source {
            // A view of a folder that is there: a theme or a `viewPath` may hold the others.
            Source::View | Source::Layout => {
                let file = self.file(cx, value)?;
                let told = file.parent().is_some_and(Path::is_dir);
                // `$this->render()` outside a class is a view's only in a view.
                let template = matches!(Self::place(cx), Some(CallTarget::Method { classes, .. })
                    if classes.iter().all(|class| class == TEMPLATE_THIS));
                let view = site.components().any(|part| part.as_os_str() == "views");
                (told && (view || !template) && !file.is_file())
                    .then(|| Undefined::warning(format!("Unknown {} '{value}'", self.label)))
            }
            // An action its controller doesn't have, unless the controller lists others.
            Source::Route => {
                if matches!(Self::place(cx), Some(CallTarget::Entry { .. })) {
                    return None;
                }
                let plain =
                    value.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '-' | '/'));
                if !plain || project.apps.is_empty() {
                    return None;
                }
                let Some(action) = Self::action(cx, value) else {
                    // `controller/action` of an application that maps no controller by hand.
                    let two = value.trim_matches('/').split('/').count() == 2;
                    let module =
                        value.trim_matches('/').split('/').next().is_some_and(|id| project.module(id).is_some());
                    return (two && !module && !Self::routes(cx).is_empty())
                        .then(|| Undefined::warning(format!("Unknown route '{value}'")));
                };
                let class = Self::class_in(cx, &action.file, &action.controller)?;
                let resolver = cx.engine.resolver();
                let model = resolver.class_model(&class)?;
                if model.method(&action.method()).is_some() {
                    return None;
                }
                // Actions of their own classes are listed by `actions()`: reported where the
                // list is one array written out.
                let (listed, complete) = Self::listed_actions(cx, &class);
                (complete && !listed.iter().any(|(key, _)| *key == action.action)).then(|| {
                    Undefined::warning(format!("Controller '{}' has no action '{}'", action.controller, action.action))
                })
            }
            // A form model's attributes are its properties; a record's are its table's columns too.
            Source::Attribute => {
                if !Self::rule_attribute(cx) || !matches!(Self::place(cx), Some(CallTarget::Return { .. })) {
                    return None;
                }
                let plain = value.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
                let class = Self::model(cx)?;
                let resolver = cx.engine.resolver();
                let model = resolver.class_model(&class)?;
                let dynamic = model.is_subclass_of(RECORD)
                    || ["attributes", "__get"].iter().any(|name| {
                        model.method(name).is_some_and(|method| {
                            !method.declaring.fqn.trim_start_matches('\\').to_ascii_lowercase().starts_with("yii\\")
                        })
                    });
                (plain && !dynamic && Self::attribute_targets(cx, &class, value).is_empty())
                    .then(|| Undefined::warning(format!("Unknown attribute '{value}'")))
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
        let detail = match self.source {
            Source::Component => project.component(value).and_then(|def| def.detail.clone()),
            Source::Module => project.module(value).and_then(|def| def.detail.clone()),
            Source::Param => {
                Self::params(cx, &project).into_iter().find(|def| def.name == value).and_then(|def| def.detail)
            }
            Source::Alias => project.alias_path(value).map(|path| path.to_string_lossy().replace('\\', "/")),
            Source::Message => {
                let category = cx.site.as_ref().and_then(|site| site.first_argument.clone());
                let translations: Vec<String> = project
                    .messages
                    .iter()
                    .filter(|message| {
                        message.message == value
                            && category.as_ref().is_none_or(|category| message.category == *category)
                    })
                    .filter_map(|message| Some(format!("{}: {}", message.language, message.translation.as_deref()?)))
                    .collect();
                (!translations.is_empty()).then(|| translations.join("`\n\n`"))
            }
            Source::Route => {
                Self::action(cx, value).map(|action| format!("{}::{}()", action.controller, action.method()))
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
