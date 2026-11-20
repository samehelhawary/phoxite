//! What a CodeIgniter 4 project's files say that isn't a PHP declaration
//! ([`CodeIgniterProject`]):
//!
//! - **Where things are:** the application in `app/` and the framework in
//!   `vendor/codeigniter4/framework/system/` (or `system/` beside `app/`),
//!   unless `Config/Paths.php` says otherwise (`$appDirectory`,
//!   `$systemDirectory`, `$viewDirectory`); the paths file itself is the one
//!   `public/index.php` or `spark` names when it isn't `app/Config/Paths.php`.
//! - **Namespaces:** the `$psr4` of `Config/Autoload.php`, a module's folder
//!   by its namespace (`'Blog' => ROOTPATH . 'modules/Blog'`). A module has
//!   views (`view('Blog\Views\list')`), helpers, language files and routes of
//!   its own.
//! - **Language lines:** the files of `Language/<locale>/` of the application
//!   and of each module, and of the framework's own `system/Language/en/`,
//!   return arrays; a line is named `File.key`, nested ones with more dots
//!   (`lang('Shop.cart.empty')`).
//! - **Filter aliases:** the keys of the `$aliases` property of
//!   `Config/Filters.php`.
//! - **Routes named by their path:** a route without an `as` is named by its
//!   path, the prefixes of the groups around it before it
//!   (`Config/Routes.php` of the application and of each module).
//! - **Rule sets:** the classes of `$ruleSets` of `Config/Validation.php`,
//!   whose methods are the validation rules.
//!
//! Views and helpers are files asked for when a name is resolved.
//!
//! **CodeIgniter 3** ([`legacy`](CodeIgniterProject::legacy): no `spark`,
//! and `system/core/CodeIgniter.php` or an `index.php` setting
//! `$system_path`) keeps the application in `application/` and the framework
//! in `system/`, or where `index.php` says (`$application_folder`,
//! `$system_path`, `$view_folder`). Its language lines are the `$lang['key']`
//! of `language/<idiom>/*_lang.php`, its configuration items the
//! `$config['key']` of `config/*.php`.
//!
//! The engine keeps the result for an edit count.

use std::path::{Component, Path, PathBuf};

use mago_span::HasSpan;
use mago_syntax::cst::{Argument, ArrayElement, ClassLikeMemberSelector, Expression, Literal, MethodCall};
use mago_syntax::walker::Walker;

use crate::base::Range;
use crate::frameworks::laravel::config::ConfigKeys;
use crate::syntax::ParsedFile;

/// A name a file declares.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Def {
    pub name: String,
    /// A language line's text, a filter's class, a route's handler.
    pub detail: Option<String>,
    pub file: PathBuf,
    pub range: Range,
}

/// See the module documentation.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CodeIgniterProject {
    pub root: PathBuf,
    app: PathBuf,
    system: Option<PathBuf>,
    views: PathBuf,
    /// A module's namespace and its folder (not the application's own).
    pub namespaces: Vec<(String, PathBuf)>,
    /// The language lines, the application's first.
    pub lines: Vec<Def>,
    pub filters: Vec<Def>,
    /// The routes without a name of their own, by their path.
    pub route_paths: Vec<Def>,
    /// The classes whose methods are validation rules (FQNs).
    pub rule_sets: Vec<String>,
    /// CodeIgniter 3.
    pub legacy: bool,
    /// CodeIgniter 3: the items of the configuration files.
    pub config_items: Vec<Def>,
    /// CodeIgniter 3: the routes of `config/routes.php`, each with what it
    /// is sent to.
    pub legacy_routes: Vec<Def>,
}

/// The rule sets of an application that configures none: the framework's.
const RULE_SETS: [&str; 4] = [
    "CodeIgniter\\Validation\\Rules",
    "CodeIgniter\\Validation\\FormatRules",
    "CodeIgniter\\Validation\\FileRules",
    "CodeIgniter\\Validation\\CreditCardRules",
];

/// The default value of the property `$name` of a file.
struct Wanted<'a> {
    name: &'a [u8],
    found: Option<&'a Expression<'a>>,
}

struct PropertyWalker;

impl<'a> Walker<'a, 'a, Wanted<'a>> for PropertyWalker {
    fn walk_in_property_concrete_item(
        &self,
        item: &'a mago_syntax::cst::PropertyConcreteItem<'a>,
        cx: &mut Wanted<'a>,
    ) {
        if item.variable.name == cx.name && cx.found.is_none() {
            cx.found = Some(item.value);
        }
    }
}

fn property<'a>(parsed: &'a ParsedFile, name: &'a [u8]) -> Option<&'a Expression<'a>> {
    let mut wanted = Wanted { name, found: None };
    PropertyWalker.walk_program(parsed.program(), &mut wanted);
    wanted.found
}

fn elements<'a>(expr: Option<&'a Expression<'a>>) -> Vec<&'a ArrayElement<'a>> {
    match expr {
        Some(Expression::Array(array)) => array.elements.iter().collect(),
        Some(Expression::LegacyArray(array)) => array.elements.iter().collect(),
        _ => Vec::new(),
    }
}

fn string(expr: &Expression<'_>) -> Option<String> {
    match expr {
        Expression::Literal(Literal::String(string)) => {
            string.value.map(|value| String::from_utf8_lossy(value).into_owned())
        }
        _ => None,
    }
}

/// `path` without its `.` and `..` parts.
fn tidy(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// The routes of one routes file, by their path.
struct Routes<'a> {
    parsed: &'a ParsedFile,
    file: &'a Path,
    /// The groups around the walker: where each ends and its prefix.
    groups: Vec<(u32, String)>,
    found: Vec<Def>,
}

struct RouteWalker;

impl<'a> Walker<'a, 'a, Routes<'a>> for RouteWalker {
    fn walk_in_method_call(&self, call: &'a MethodCall<'a>, cx: &mut Routes<'a>) {
        const VERBS: [&[u8]; 10] =
            [b"get", b"post", b"put", b"patch", b"delete", b"options", b"head", b"add", b"cli", b"view"];
        let span = call.span();
        cx.groups.retain(|(end, _)| *end > span.start.offset);
        let ClassLikeMemberSelector::Identifier(name) = &call.method else { return };
        let arguments: Vec<&Expression<'_>> = call
            .argument_list
            .arguments
            .iter()
            .filter_map(|argument| match argument {
                Argument::Positional(argument) => Some(argument.value),
                _ => None,
            })
            .collect();
        let Some(first) = arguments.first() else { return };
        let Some(from) = string(first) else { return };
        if name.value.eq_ignore_ascii_case(b"group") {
            cx.groups.push((span.end.offset, from));
            return;
        }
        // The routes `resource('photos')` and `presenter('photos')` make, as the user guide
        // lists them.
        let resource = name.value.eq_ignore_ascii_case(b"resource");
        if resource || name.value.eq_ignore_ascii_case(b"presenter") {
            let option = |wanted: &str| {
                arguments.iter().skip(1).flat_map(|options| elements(Some(options))).find_map(|element| match element {
                    ArrayElement::KeyValue(element) if string(element.key).as_deref() == Some(wanted) => {
                        string(element.value)
                    }
                    _ => None,
                })
            };
            let mut base: Vec<&str> = cx.groups.iter().map(|(_, prefix)| prefix.trim_matches('/')).collect();
            base.push(from.trim_matches('/'));
            base.retain(|part| !part.is_empty());
            let base = base.join("/");
            let id = option("placeholder").unwrap_or_else(|| "(:segment)".to_owned());
            let controller = option("controller").unwrap_or_else(|| {
                let last = from.trim_matches('/').rsplit('/').next().unwrap_or_default();
                let mut name = last.to_owned();
                if let Some(first) = name.get_mut(..1) {
                    first.make_ascii_uppercase();
                }
                name
            });
            let made: Vec<(String, &str)> = if resource {
                vec![
                    (base.clone(), "index"),
                    (format!("{base}/new"), "new"),
                    (format!("{base}/{id}/edit"), "edit"),
                    (format!("{base}/{id}"), "show"),
                ]
            } else {
                vec![
                    (base.clone(), "index"),
                    (format!("{base}/show/{id}"), "show"),
                    (format!("{base}/new"), "new"),
                    (format!("{base}/create"), "create"),
                    (format!("{base}/edit/{id}"), "edit"),
                    (format!("{base}/update/{id}"), "update"),
                    (format!("{base}/remove/{id}"), "remove"),
                    (format!("{base}/delete/{id}"), "delete"),
                    (format!("{base}/{id}"), "show"),
                ]
            };
            let at = first.span();
            let range = cx.parsed.line_index().range(at.start.offset + 1, at.end.offset.saturating_sub(1));
            for (path, method) in made {
                cx.found.push(Def {
                    name: path,
                    detail: Some(format!("{controller}::{method}")),
                    file: cx.file.to_path_buf(),
                    range,
                });
            }
            return;
        }
        if !VERBS.iter().any(|verb| name.value.eq_ignore_ascii_case(verb)) || arguments.len() < 2 {
            return;
        }
        // One with an `as` is named by it.
        let named = arguments.iter().skip(2).any(|options| {
            elements(Some(options)).into_iter().any(|element| match element {
                ArrayElement::KeyValue(element) => string(element.key).as_deref() == Some("as"),
                _ => false,
            })
        });
        if named {
            return;
        }
        let mut parts: Vec<&str> = cx.groups.iter().map(|(_, prefix)| prefix.trim_matches('/')).collect();
        parts.push(from.trim_matches('/'));
        parts.retain(|part| !part.is_empty());
        let path = if parts.is_empty() { "/".to_owned() } else { parts.join("/") };
        let at = first.span();
        cx.found.push(Def {
            name: path,
            detail: string(arguments[1]),
            file: cx.file.to_path_buf(),
            range: cx.parsed.line_index().range(at.start.offset + 1, at.end.offset.saturating_sub(1)),
        });
    }
}

/// The namespace the handler at byte `offset` of a routes file is in: the
/// `namespace` of the options of the innermost group around it, else what
/// `setDefaultNamespace()` was given.
pub fn route_namespace(parsed: &ParsedFile, offset: u32) -> Option<String> {
    struct Found {
        offset: u32,
        group: Option<String>,
        default: Option<String>,
    }
    struct GroupWalker;
    impl<'a> Walker<'a, 'a, Found> for GroupWalker {
        fn walk_in_method_call(&self, call: &'a MethodCall<'a>, cx: &mut Found) {
            let ClassLikeMemberSelector::Identifier(name) = &call.method else { return };
            let mut arguments = call.argument_list.arguments.iter().filter_map(|argument| match argument {
                Argument::Positional(argument) => Some(argument.value),
                _ => None,
            });
            if name.value.eq_ignore_ascii_case(b"setDefaultNamespace") {
                if let Some(namespace) = arguments.next().and_then(string) {
                    cx.default = Some(namespace);
                }
                return;
            }
            let span = call.span();
            let around = span.start.offset <= cx.offset && cx.offset <= span.end.offset;
            if !name.value.eq_ignore_ascii_case(b"group") || !around {
                return;
            }
            for options in arguments {
                for element in elements(Some(options)) {
                    let ArrayElement::KeyValue(element) = element else { continue };
                    if string(element.key).as_deref() == Some("namespace")
                        && let Some(namespace) = string(element.value)
                    {
                        cx.group = Some(namespace);
                    }
                }
            }
        }
    }
    let mut found = Found { offset, group: None, default: None };
    GroupWalker.walk_program(parsed.program(), &mut found);
    let namespace = found.group.or(found.default)?;
    Some(namespace.trim_matches('\\').to_owned()).filter(|namespace| !namespace.is_empty())
}

impl CodeIgniterProject {
    /// The application's folder.
    pub fn app(&self) -> PathBuf {
        self.app.clone()
    }

    /// The framework's folder, when the project has it.
    pub fn system(&self) -> Option<PathBuf> {
        self.system.clone()
    }

    pub fn views_dir(&self) -> PathBuf {
        self.views.clone()
    }

    /// The folders helpers are in: the application's, the modules', the
    /// framework's.
    pub fn helper_dirs(&self) -> Vec<PathBuf> {
        let folder = if self.legacy { "helpers" } else { "Helpers" };
        let mut dirs = vec![self.app.join(folder)];
        dirs.extend(self.namespaces.iter().map(|(_, folder)| folder.join("Helpers")));
        dirs.extend(self.system.iter().map(|system| system.join(folder)));
        dirs
    }

    /// CodeIgniter 3: what the routes of `config/routes.php` send `uri` to
    /// (`products/7` to `catalog/product_lookup/$1` by `products/(:num)`).
    pub fn routed(&self, uri: &str) -> Option<String> {
        let wanted: Vec<&str> = uri.trim_matches('/').split('/').collect();
        self.legacy_routes.iter().find_map(|route| {
            let pattern: Vec<&str> = route.name.trim_matches('/').split('/').collect();
            let matches = pattern.len() == wanted.len()
                && pattern.iter().zip(&wanted).all(|(pattern, segment)| match *pattern {
                    "(:num)" => !segment.is_empty() && segment.chars().all(|c| c.is_ascii_digit()),
                    pattern if pattern.starts_with('(') => !segment.is_empty(),
                    pattern => pattern == *segment,
                });
            let target = route.detail.as_deref()?.trim_matches(['\'', '"']);
            (matches && !target.is_empty()).then(|| target.to_owned())
        })
    }

    /// CodeIgniter 3: the folders libraries are in, the application's first.
    pub fn library_dirs(&self) -> Vec<PathBuf> {
        let mut dirs = vec![self.app.join("libraries")];
        dirs.extend(self.system.iter().map(|system| system.join("libraries")));
        dirs
    }

    /// CodeIgniter 3: the assignments `$name['key'] = …;` of the files of
    /// `folder`, each with what follows the `=`.
    fn assigned(folder: &Path, name: &str, text_of: &impl Fn(&Path) -> Option<String>, out: &mut Vec<Def>) {
        let Ok(files) = std::fs::read_dir(folder) else { return };
        let mut files: Vec<PathBuf> = files.flatten().map(|entry| entry.path()).collect();
        files.sort();
        let opening = format!("${name}[");
        for file in files {
            if file.extension().is_none_or(|ext| ext != "php") {
                continue;
            }
            let Some(text) = text_of(&file) else { continue };
            let lines = crate::syntax::LineIndex::new(&text);
            let mut offset = 0;
            for line in text.split_inclusive('\n') {
                let start = offset;
                offset += line.len();
                let indent = line.len() - line.trim_start().len();
                let Some(rest) = line.trim_start().strip_prefix(&opening) else { continue };
                let Some(quote) = rest.chars().next().filter(|c| matches!(c, '\'' | '"')) else { continue };
                let Some(close) = rest[1..].find(quote) else { continue };
                let key = &rest[1..1 + close];
                let after = rest[close + 2..].trim_start();
                let Some(value) = after.strip_prefix(']').map(str::trim_start).and_then(|rest| rest.strip_prefix('='))
                else {
                    continue;
                };
                let at = (start + indent + opening.len() + 1) as u32;
                let value = value.trim().trim_end_matches(';').trim();
                out.push(Def {
                    name: key.to_owned(),
                    detail: Some(value.to_owned()).filter(|value| !value.is_empty() && value.len() < 120),
                    file: file.clone(),
                    range: lines.range(at, at + key.len() as u32),
                });
            }
        }
    }

    /// What `index.php` sets `$name` to, when it is a string.
    fn index_setting(text: &str, name: &str) -> Option<String> {
        let wanted = format!("${name}");
        text.lines().find_map(|line| {
            let rest = line.trim_start().strip_prefix(&wanted)?.trim_start().strip_prefix('=')?.trim_start();
            let quote = rest.chars().next().filter(|c| matches!(c, '\'' | '"'))?;
            let close = rest[1..].find(quote)?;
            Some(rest[1..1 + close].to_owned()).filter(|value| !value.is_empty())
        })
    }

    /// Reads a CodeIgniter 3 project.
    fn load_legacy(root: &Path, index: Option<String>, text_of: impl Fn(&Path) -> Option<String>) -> Self {
        let setting = |name: &str| index.as_deref().and_then(|text| Self::index_setting(text, name));
        let folder = |written: String| tidy(&root.join(written.trim_end_matches('/')));
        let app = setting("application_folder").map(folder).filter(|app| app.is_dir());
        let app = app.unwrap_or_else(|| root.join("application"));
        let system = setting("system_path").map(folder).unwrap_or_else(|| root.join("system"));
        let views = setting("view_folder").map(folder).filter(|views| views.is_dir());
        let mut out = Self {
            root: root.to_path_buf(),
            views: views.unwrap_or_else(|| app.join("views")),
            system: Some(system).filter(|system| system.is_dir()),
            app,
            legacy: true,
            ..Self::default()
        };
        let mut languages = vec![out.app.join("language")];
        languages.extend(out.system.iter().map(|system| system.join("language")));
        for language in languages {
            let Ok(idioms) = std::fs::read_dir(&language) else { continue };
            let mut idioms: Vec<PathBuf> =
                idioms.flatten().map(|entry| entry.path()).filter(|path| path.is_dir()).collect();
            idioms.sort();
            for idiom in idioms {
                Self::assigned(&idiom, "lang", &text_of, &mut out.lines);
            }
        }
        let mut items = Vec::new();
        Self::assigned(&out.app.join("config"), "config", &text_of, &mut items);
        out.config_items = items;
        let mut routes = Vec::new();
        Self::assigned(&out.app.join("config"), "route", &text_of, &mut routes);
        routes.retain(|route| {
            !matches!(route.name.as_str(), "default_controller" | "404_override" | "translate_uri_dashes")
        });
        out.legacy_routes = routes;
        out
    }

    /// The module a namespaced name is of, and what follows its namespace:
    /// `Blog\Views\list` is `list` below `Views` of `Blog`'s folder.
    fn module<'n>(&self, name: &'n str) -> Option<(&(String, PathBuf), &'n str)> {
        let name = name.trim_start_matches('\\');
        self.namespaces
            .iter()
            .filter(|(namespace, _)| {
                name.len() > namespace.len()
                    && name.starts_with(namespace.as_str())
                    && name.as_bytes()[namespace.len()] == b'\\'
            })
            .max_by_key(|(namespace, _)| namespace.len())
            .map(|module| (module, &name[module.0.len() + 1..]))
    }

    /// Whether a namespaced name is of a namespace the project declares.
    pub fn knows_namespace(&self, name: &str) -> bool {
        self.module(name).is_some()
    }

    /// The file of the view named `name`: `books/list` of the views folder,
    /// `Blog\Views\list` of a module's.
    pub fn view_file(&self, name: &str) -> Option<PathBuf> {
        let file = |name: &str| format!("{}.php", name.trim_end_matches(".php"));
        if !name.contains('\\') {
            return Some(self.views.join(file(name)));
        }
        let ((_, folder), rest) = self.module(name)?;
        Some(folder.join(file(&rest.replace('\\', "/"))))
    }

    /// The name of the view the file at `path` is, as [`view_file`](Self::view_file) reads it.
    pub fn view_name(&self, path: &Path) -> Option<String> {
        let plain = |relative: &Path| relative.with_extension("").to_string_lossy().replace('\\', "/");
        if let Ok(relative) = path.strip_prefix(&self.views) {
            return Some(plain(relative));
        }
        self.namespaces.iter().find_map(|(namespace, folder)| {
            let relative = path.strip_prefix(folder.join("Views")).ok()?;
            Some(format!("{namespace}\\Views\\{}", plain(relative)))
        })
    }

    /// The folders views are in, each with what its views' names start with.
    pub fn view_dirs(&self) -> Vec<(String, PathBuf)> {
        let mut dirs = vec![(String::new(), self.views.clone())];
        dirs.extend(
            self.namespaces.iter().map(|(namespace, folder)| (format!("{namespace}\\Views\\"), folder.join("Views"))),
        );
        dirs
    }

    /// A path a configuration file writes: constants and `__DIR__` joined
    /// with strings (`ROOTPATH . 'modules/Blog'`).
    fn path_of(&self, source: &str, beside: &Path) -> Option<PathBuf> {
        let mut out = PathBuf::new();
        let mut rest = source.trim();
        // `realpath(…)` and `rtrim(…, '/')` around a path say nothing of where it is.
        loop {
            let inner = ["realpath(", "rtrim("].iter().find_map(|open| rest.strip_prefix(open)?.strip_suffix(')'));
            let Some(inner) = inner else { break };
            let cut =
                inner.rfind(", '").or_else(|| inner.rfind(", \"")).filter(|_| source.trim().starts_with("rtrim("));
            rest = inner[..cut.unwrap_or(inner.len())].trim();
        }
        let mut any = false;
        while !rest.is_empty() {
            let quote = rest.chars().next().filter(|c| matches!(c, '\'' | '"'));
            let taken = if let Some(quote) = quote {
                let close = rest[1..].find(quote)?;
                let text = &rest[1..1 + close];
                out = PathBuf::from(format!("{}{text}", out.to_string_lossy()));
                close + 2
            } else if let Some(after) = rest.strip_prefix("dirname(") {
                // `dirname(__DIR__)`, `dirname(__DIR__, 2)`.
                let close = after.find(')')?;
                let (what, levels) = match after[..close].split_once(',') {
                    Some((what, levels)) => (what, levels.trim().parse::<usize>().ok()?),
                    None => (&after[..close], 1),
                };
                if what.trim() != "__DIR__" {
                    return None;
                }
                let mut base = beside.to_path_buf();
                for _ in 0..levels {
                    base.pop();
                }
                out = base;
                "dirname(".len() + close + 1
            } else if let Some(after) = rest.strip_prefix("DIRECTORY_SEPARATOR") {
                out = PathBuf::from(format!("{}/", out.to_string_lossy()));
                rest.len() - after.len()
            } else {
                let end = rest.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).unwrap_or(rest.len());
                let mut base = match &rest[..end] {
                    "__DIR__" => beside.to_path_buf(),
                    "APPPATH" => self.app.clone(),
                    "ROOTPATH" => self.root.clone(),
                    "SYSTEMPATH" => self.system.clone()?,
                    "FCPATH" => self.root.join("public"),
                    "WRITEPATH" => self.root.join("writable"),
                    _ => return None,
                };
                // The constants end with a separator; `__DIR__` doesn't.
                if &rest[..end] != "__DIR__" {
                    base.push("");
                }
                out = base;
                end
            };
            any = true;
            rest = rest[taken..].trim_start();
            rest = match rest.strip_prefix('.') {
                Some(more) => more.trim_start(),
                None if rest.is_empty() => rest,
                None => return None,
            };
        }
        any.then(|| tidy(&out))
    }

    /// The paths file: `app/Config/Paths.php`, else the one the front
    /// controller or `spark` names.
    fn paths_file(root: &Path, text_of: &impl Fn(&Path) -> Option<String>) -> PathBuf {
        let usual = root.join("app/Config/Paths.php");
        if usual.is_file() {
            return usual;
        }
        for entry in ["public/index.php", "index.php", "spark"] {
            let file = root.join(entry);
            let Some(text) = text_of(&file) else { continue };
            let Some(end) = text.find("Config/Paths.php") else { continue };
            let Some(start) = text[..end].rfind(['\'', '"']) else { continue };
            let written = format!("{}Config/Paths.php", &text[start + 1..end]);
            let beside = file.parent().unwrap_or(root);
            let found = tidy(&beside.join(written.trim_start_matches('/')));
            if found.is_file() {
                return found;
            }
        }
        usual
    }

    /// Reads the project at `root`; `text_of` gives a file's text (an open
    /// buffer's first).
    pub fn load(root: &Path, text_of: impl Fn(&Path) -> Option<String>) -> Self {
        if !root.join("spark").is_file() {
            let index = text_of(&root.join("index.php")).filter(|text| text.contains("$system_path"));
            if index.is_some() || root.join("system/core/CodeIgniter.php").is_file() {
                return Self::load_legacy(root, index, text_of);
            }
        }
        let parse = |file: &Path| text_of(file).map(|text| ParsedFile::parse(&file.to_string_lossy(), text));
        let source = |parsed: &ParsedFile, expr: &Expression<'_>| {
            let span = expr.span();
            parsed.text().get(span.start.offset as usize..span.end.offset as usize).unwrap_or_default().to_owned()
        };
        let paths = Self::paths_file(root, &text_of);
        let config = paths.parent().unwrap_or(root).to_path_buf();
        let app = config.parent().unwrap_or(root).to_path_buf();
        let mut out = Self { root: root.to_path_buf(), views: app.join("Views"), app, ..Self::default() };
        let usual = [root.join("vendor/codeigniter4/framework/system"), root.join("system")];
        out.system = usual.into_iter().find(|folder| folder.is_dir());
        if let Some(parsed) = parse(&paths) {
            let said =
                |name: &[u8]| property(&parsed, name).and_then(|expr| out.path_of(&source(&parsed, expr), &config));
            let (app, system, views) = (said(b"$appDirectory"), said(b"$systemDirectory"), said(b"$viewDirectory"));
            if let Some(app) = app.filter(|folder| folder.is_dir()) {
                out.views = app.join("Views");
                out.app = app;
            }
            if let Some(system) = system.filter(|folder| folder.is_dir()) {
                out.system = Some(system);
            }
            if let Some(views) = views.filter(|folder| folder.is_dir()) {
                out.views = views;
            }
        }
        let config = out.app.join("Config");

        if let Some(parsed) = parse(&config.join("Autoload.php")) {
            for element in elements(property(&parsed, b"$psr4")) {
                let ArrayElement::KeyValue(element) = element else { continue };
                let Some(namespace) = string(element.key) else { continue };
                let namespace = namespace.trim_matches('\\').to_owned();
                let Some(folder) = out.path_of(&source(&parsed, element.value), &config) else { continue };
                if !namespace.is_empty() && !folder.starts_with(&out.app) && !out.app.starts_with(&folder) {
                    out.namespaces.push((namespace, folder));
                }
            }
        }

        // The packages Composer installed: CodeIgniter finds what they bring by their namespaces.
        if let Some(text) = text_of(&root.join("vendor/composer/autoload_psr4.php")) {
            for (namespace, folder) in composer_namespaces(&text, root) {
                let own = out.system.as_ref().is_some_and(|system| folder.starts_with(system));
                let known = out.namespaces.iter().any(|(known, _)| *known == namespace);
                if !own && !known && !folder.starts_with(&out.app) && !out.app.starts_with(&folder) {
                    out.namespaces.push((namespace, folder));
                }
            }
        }

        let mut folders: Vec<PathBuf> = Vec::new();
        // Every locale of the application and of each module: a line is declared in each.
        let languages = std::iter::once(&out.app).chain(out.namespaces.iter().map(|(_, folder)| folder));
        for folder in languages {
            if let Ok(locales) = std::fs::read_dir(folder.join("Language")) {
                let mut locales: Vec<PathBuf> =
                    locales.flatten().map(|entry| entry.path()).filter(|path| path.is_dir()).collect();
                locales.sort();
                folders.extend(locales);
            }
        }
        folders.extend(out.system.iter().map(|system| system.join("Language/en")));
        for folder in folders {
            let Ok(files) = std::fs::read_dir(&folder) else { continue };
            let mut files: Vec<PathBuf> = files.flatten().map(|entry| entry.path()).collect();
            files.sort();
            for file in files {
                if file.extension().is_none_or(|ext| ext != "php") {
                    continue;
                }
                let Some(name) = file.file_stem().and_then(|name| name.to_str()) else { continue };
                let Some(parsed) = parse(&file) else { continue };
                for entry in ConfigKeys::file_entries(name, &file, &parsed) {
                    if entry.is_file || entry.is_array {
                        continue;
                    }
                    out.lines.push(Def {
                        name: entry.key,
                        detail: entry.value,
                        file: file.clone(),
                        range: entry.range,
                    });
                }
            }
        }

        let filters = config.join("Filters.php");
        if let Some(parsed) = parse(&filters) {
            for element in elements(property(&parsed, b"$aliases")) {
                let ArrayElement::KeyValue(element) = element else { continue };
                let Some(name) = string(element.key) else { continue };
                let span = element.key.span();
                out.filters.push(Def {
                    name,
                    detail: Some(source(&parsed, element.value)).filter(|class| class.len() < 120),
                    file: filters.clone(),
                    range: parsed.line_index().range(span.start.offset + 1, span.end.offset.saturating_sub(1)),
                });
            }
        }

        let routes = std::iter::once(&out.app).chain(out.namespaces.iter().map(|(_, folder)| folder));
        let routes: Vec<PathBuf> = routes.map(|folder| folder.join("Config/Routes.php")).collect();
        for file in routes {
            let Some(parsed) = parse(&file) else { continue };
            let mut found = Routes { parsed: &parsed, file: &file, groups: Vec::new(), found: Vec::new() };
            RouteWalker.walk_program(parsed.program(), &mut found);
            out.route_paths.extend(found.found);
        }

        out.rule_sets = RULE_SETS.iter().map(|class| (*class).to_owned()).collect();
        if let Some(parsed) = parse(&config.join("Validation.php")) {
            let text = parsed.text();
            let sets: Vec<String> = elements(property(&parsed, b"$ruleSets"))
                .into_iter()
                .filter_map(|element| match element {
                    ArrayElement::Value(element) => Some(source(&parsed, element.value)),
                    _ => None,
                })
                .filter_map(|written| written.strip_suffix("::class").map(str::to_owned))
                .map(|class| match class.strip_prefix('\\') {
                    Some(fqn) => fqn.to_owned(),
                    None => imported(text, &class).unwrap_or_else(|| format!("Config\\{class}")),
                })
                .collect();
            if !sets.is_empty() {
                out.rule_sets = sets;
            }
        }
        out
    }
}

/// The namespaces of Composer's `autoload_psr4.php`, each with its first
/// folder: `'Vendor\\\\Package\\\\' => array($vendorDir . '/vendor/package/src')`.
fn composer_namespaces(text: &str, root: &Path) -> Vec<(String, PathBuf)> {
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix('\'') else { continue };
        let Some((namespace, rest)) = rest.split_once("' =>") else { continue };
        let namespace = namespace.replace("\\\\", "\\").trim_matches('\\').to_owned();
        let (base, rest) = match (rest.find("$vendorDir"), rest.find("$baseDir")) {
            (Some(at), other) if other.is_none_or(|other| at < other) => (root.join("vendor"), &rest[at..]),
            (_, Some(at)) => (root.to_path_buf(), &rest[at..]),
            _ => continue,
        };
        let Some(open) = rest.find('\'') else { continue };
        let Some(close) = rest[open + 1..].find('\'') else { continue };
        let folder = rest[open + 1..open + 1 + close].trim_start_matches('/');
        if !namespace.is_empty() {
            out.push((namespace, tidy(&base.join(folder))));
        }
    }
    out
}

/// The class the `use` statements of `text` make `name` (its first part).
fn imported(text: &str, name: &str) -> Option<String> {
    let (first, rest) = name.split_once('\\').map_or((name, ""), |(first, rest)| (first, rest));
    text.lines().find_map(|line| {
        let import = line.trim().strip_prefix("use ")?.trim_end_matches(';').trim();
        let (class, alias) = match import.split_once(" as ") {
            Some((class, alias)) => (class.trim(), alias.trim()),
            None => (import, import.rsplit('\\').next().unwrap_or(import)),
        };
        (alias == first).then(|| {
            let class = class.trim_start_matches('\\');
            if rest.is_empty() { class.to_owned() } else { format!("{class}\\{rest}") }
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_is_constants_and_strings_joined() {
        let project = CodeIgniterProject {
            root: PathBuf::from("/site"),
            app: PathBuf::from("/site/app"),
            ..CodeIgniterProject::default()
        };
        let beside = Path::new("/site/app/Config");
        let path = |source: &str| project.path_of(source, beside).map(|path| path.to_string_lossy().replace('\\', "/"));
        assert_eq!(path("ROOTPATH . 'modules/Blog'").as_deref(), Some("/site/modules/Blog"));
        assert_eq!(path("APPPATH . 'Modules/Shop/'").as_deref(), Some("/site/app/Modules/Shop"));
        assert_eq!(path("__DIR__ . '/../../framework'").as_deref(), Some("/site/framework"));
        assert_eq!(path("dirname(__DIR__, 2) . DIRECTORY_SEPARATOR . 'ci'").as_deref(), Some("/site/ci"));
        assert_eq!(path("realpath(dirname(__DIR__) . '/Views')").as_deref(), Some("/site/app/Views"));
        assert_eq!(path("$somewhere"), None);
    }

    #[test]
    fn composers_namespaces_are_read() {
        let text = "<?php\nreturn array(\n    'Shop\\\\Blog\\\\' => array($vendorDir . '/shop/blog/src'),\n    'App\\\\' => array($baseDir . '/app'),\n);\n";
        let found = composer_namespaces(text, Path::new("/site"));
        let found: Vec<(String, String)> =
            found.into_iter().map(|(name, folder)| (name, folder.to_string_lossy().replace('\\', "/"))).collect();
        assert_eq!(
            found,
            [
                ("Shop\\Blog".to_owned(), "/site/vendor/shop/blog/src".to_owned()),
                ("App".to_owned(), "/site/app".to_owned())
            ]
        );
    }

    #[test]
    fn index_php_says_where_a_codeigniter_3_application_is() {
        let text = "<?php\n\t$system_path = 'ci/system';\n\t$application_folder = \"app\";\n\t$view_folder = '';\n";
        let setting = |name: &str| CodeIgniterProject::index_setting(text, name);
        assert_eq!(setting("system_path").as_deref(), Some("ci/system"));
        assert_eq!(setting("application_folder").as_deref(), Some("app"));
        assert_eq!(setting("view_folder"), None);
    }

    #[test]
    fn an_import_names_the_class() {
        let text = "<?php\nuse CodeIgniter\\Validation\\Rules;\nuse App\\Validation\\ShopRules as Shop;\n";
        assert_eq!(imported(text, "Rules").as_deref(), Some("CodeIgniter\\Validation\\Rules"));
        assert_eq!(imported(text, "Shop").as_deref(), Some("App\\Validation\\ShopRules"));
        assert_eq!(imported(text, "Other"), None);
    }
}
