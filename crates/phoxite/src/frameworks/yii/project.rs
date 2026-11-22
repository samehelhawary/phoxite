//! What a Yii 2 application's files say that isn't a PHP declaration
//! ([`YiiProject`]), read with [`crate::frameworks::php_array`]:
//!
//! - **The applications:** a folder with a `config` folder holding
//!   `web.php`, `console.php`, `main.php` or `main-local.php`: the project
//!   itself (the basic template), or its first-level folders (the advanced
//!   template's `common`, `frontend`, `backend`, `console`). The folder
//!   above `config` is the application's base path (`@app`).
//! - **Components and modules:** the keys of `components` and `modules`,
//!   each with the class its value names (a class name, `['class' => …]`,
//!   `['__class' => …]`); the `identityClass` of the `user` component.
//! - **Parameters:** the keys of `params`.
//! - **Aliases:** Yii's own (`@app`, `@vendor`, `@runtime`, `@webroot`,
//!   `@yii`, and in the advanced template the application folders'), the
//!   keys of `aliases`, and the `Yii::setAlias('@name', path)` calls of the
//!   `bootstrap.php` files beside the configuration.
//! - **Controller namespaces:** `controllerNamespace`, else
//!   `app\controllers`.
//! - **Messages:** the files of `messages/<language>/` of each application,
//!   and of the `basePath` of the sources of the `i18n` component: a file
//!   is its category (`app.php` is `app`, or what `fileMap` says), its
//!   string keys the messages.
//!
//! Views are files, found when a name is resolved ([`YiiProject::view_file`]).
//! The behaviour follows Yii's guide; the engine keeps the result for an
//! edit count.

use std::path::{Path, PathBuf};

use mago_syntax::cst::{
    Argument, ArrayElement, BinaryOperator, ClassLikeMemberSelector, Expression, StaticMethodCall, Variable,
};
use mago_syntax::walker::Walker;

use crate::base::{Pos, Range};
use crate::frameworks::php_array::{self, Node, path_of, tidy};
use crate::syntax::ParsedFile;

/// A name a configuration file declares.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Def {
    pub name: String,
    /// A component's or a module's class, a parameter's value.
    pub detail: Option<String>,
    pub file: PathBuf,
    pub range: Range,
}

/// A path alias (`@app`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Alias {
    /// With its `@`.
    pub name: String,
    /// The folder, when the source says it: a path, or another alias.
    pub path: Option<String>,
    /// Where the project declares it; `None` for Yii's own.
    pub declared: Option<(PathBuf, Range)>,
}

/// A message of a message file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Message {
    pub category: String,
    pub message: String,
    pub language: String,
    pub translation: Option<String>,
    pub file: PathBuf,
    pub range: Range,
}

/// See the module documentation.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct YiiProject {
    pub root: PathBuf,
    /// The base paths of the applications, the first being `@app`.
    pub apps: Vec<PathBuf>,
    pub components: Vec<Def>,
    pub modules: Vec<Def>,
    pub params: Vec<Def>,
    pub aliases: Vec<Alias>,
    pub controller_namespaces: Vec<String>,
    /// The `identityClass` of the `user` component.
    pub identity: Option<String>,
    pub messages: Vec<Message>,
    /// `controllerMap`: a controller's id (a module's after the module's
    /// id, `admin/account`) and its class.
    pub controller_map: Vec<Def>,
    /// The parameters below another (`'mail' => ['from' => …]`): the keys
    /// above each, and the parameter.
    pub nested_params: Vec<(Vec<String>, Def)>,
    /// Where views are when not in `views`: a `viewPath`, a `layoutPath`
    /// and the themes' `pathMap`.
    pub view_maps: Vec<ViewMap>,
    /// The `viewPath` of the mailer of an application: its base path and
    /// the folder.
    pub mail_paths: Vec<(PathBuf, PathBuf)>,
    /// The folders migrations are in.
    pub migration_paths: Vec<PathBuf>,
}

/// Files of `from` are those of `to`: always for a `viewPath`, where the
/// file of `to` is there for a theme.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ViewMap {
    pub from: PathBuf,
    pub to: PathBuf,
    pub theme: bool,
}

/// What the configuration says with paths, before the aliases are known.
#[derive(Default)]
struct Raw {
    sources: Vec<MessageSource>,
    /// The folder a `viewPath` or a `layoutPath` replaces, and the path.
    view_paths: Vec<(PathBuf, String)>,
    /// A module's class, `views` or `views/layouts`, and the path.
    module_views: Vec<(String, &'static str, String)>,
    /// A theme's `pathMap`: from and to.
    themes: Vec<(String, String)>,
    mail: Vec<(PathBuf, String)>,
    migrations: Vec<String>,
}

/// A message source of the `i18n` component: the file configuring it, its
/// `basePath` and its `fileMap` (category and file).
type MessageSource = (PathBuf, Option<String>, Vec<(String, String)>);

/// The configuration files of an application, in the order Yii's templates
/// merge them.
const CONFIG_FILES: [&str; 6] = ["main.php", "main-local.php", "web.php", "web-local.php", "console.php", "test.php"];

/// First-level folders that are never an application.
const NOT_APPS: [&str; 6] = ["vendor", "node_modules", "runtime", "web", "environments", "tests"];

/// The class a component's or a module's configuration names.
fn configured_class(value: &Node) -> Option<String> {
    let named = match value {
        Node::Array(_) => value.get("class").or_else(|| value.get("__class"))?.class(),
        other => other.class(),
    };
    // A plain word is an id or a path, not a class.
    named.filter(|class| class.contains('\\')).map(str::to_owned)
}

/// What a parameter's value is, for a hover.
fn param_detail(value: &Node) -> Option<String> {
    let detail = match value {
        Node::String(text) => Some(format!("'{text}'")),
        Node::Int(value) => Some(value.to_string()),
        Node::Bool(value) => Some(value.to_string()),
        Node::Float(text) | Node::Other(text) => Some(text.clone()),
        _ => None,
    };
    detail.filter(|detail| detail.len() < 120)
}

/// The parameters below `entry`, with the keys above each.
fn nested_params(entry: &php_array::Entry, above: &mut Vec<String>, out: &mut Vec<(Vec<String>, Def)>) {
    let Some(name) = entry.name() else { return };
    if above.len() >= 6 {
        return;
    }
    above.push(name.to_owned());
    for child in entry.value.entries() {
        let Some(key) = child.name() else { continue };
        out.push((
            above.clone(),
            Def {
                name: key.to_owned(),
                detail: param_detail(&child.value),
                file: child.file.to_path_buf(),
                range: child.range,
            },
        ));
        nested_params(child, above, out);
    }
    above.pop();
}

/// The strings a value is: one, or a list of them.
fn strings(value: &Node) -> Vec<String> {
    match value {
        Node::String(text) => vec![text.clone()],
        Node::Array(entries) => entries.iter().filter_map(|entry| entry.value.as_str().map(str::to_owned)).collect(),
        _ => Vec::new(),
    }
}

/// The alias of the folder of a namespace, as Yii's autoloader has it:
/// `@app/modules/admin` for `app\modules\admin`.
fn namespace_alias(namespace: &str) -> String {
    format!("@{}", namespace.trim_matches('\\').replace('\\', "/"))
}

/// A message of a `.po` file: its category, its text, its translation, its line and columns.
type PoMessage = (String, String, Option<String>, u32, (u32, u32));

/// The messages of a `.po` file that have a context (Yii's category):
/// `(category, message, translation, line, columns)`.
fn po_messages(text: &str) -> Vec<PoMessage> {
    fn quoted(line: &str) -> Option<(usize, usize)> {
        let start = line.find('"')? + 1;
        let end = line.rfind('"').filter(|end| *end >= start)?;
        Some((start, end))
    }
    fn unescape(text: &str) -> String {
        let mut out = String::new();
        let mut chars = text.chars();
        while let Some(c) = chars.next() {
            if c != '\\' {
                out.push(c);
                continue;
            }
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some(other) => out.push(other),
                None => {}
            }
        }
        out
    }
    #[derive(PartialEq)]
    enum Part {
        None,
        Context,
        Id,
        Translation,
    }
    let mut out = Vec::new();
    let (mut context, mut id, mut translation) = (None::<String>, None::<String>, None::<String>);
    let mut place = (0u32, (0u32, 0u32));
    let mut part = Part::None;
    let mut flush = |context: &mut Option<String>, id: &mut Option<String>, translation: &mut Option<String>, place| {
        let (line, columns): (u32, (u32, u32)) = place;
        if let (Some(context), Some(id)) = (context.take(), id.take())
            && !id.is_empty()
        {
            out.push((context, id, translation.take().filter(|text| !text.is_empty()), line, columns));
        }
        *translation = None;
    };
    for (number, line) in text.lines().enumerate() {
        let trimmed = line.trim_start();
        let column = |byte: usize| line[..byte].encode_utf16().count() as u32;
        let content = quoted(line).map(|(start, end)| (unescape(&line[start..end]), (column(start), column(end))));
        if trimmed.is_empty() || trimmed.starts_with('#') {
            if trimmed.is_empty() {
                flush(&mut context, &mut id, &mut translation, place);
                part = Part::None;
            }
            continue;
        }
        let Some((content, columns)) = content else { continue };
        if trimmed.starts_with("msgctxt") {
            flush(&mut context, &mut id, &mut translation, place);
            context = Some(content);
            part = Part::Context;
        } else if trimmed.starts_with("msgid_plural") {
            part = Part::None;
        } else if trimmed.starts_with("msgid") {
            if part != Part::Context {
                flush(&mut context, &mut id, &mut translation, place);
            }
            id = Some(content);
            place = (number as u32, columns);
            part = Part::Id;
        } else if trimmed.starts_with("msgstr") {
            if translation.is_none() {
                translation = Some(content);
                part = Part::Translation;
            } else {
                part = Part::None;
            }
        } else if trimmed.starts_with('"') {
            match part {
                Part::Context => context.get_or_insert_with(String::new).push_str(&content),
                Part::Id => {
                    // `msgid ""` and the text on the lines after it.
                    if id.as_deref() == Some("") {
                        place = (number as u32, columns);
                    }
                    id.get_or_insert_with(String::new).push_str(&content);
                }
                Part::Translation => translation.get_or_insert_with(String::new).push_str(&content),
                Part::None => {}
            }
        }
    }
    flush(&mut context, &mut id, &mut translation, place);
    out
}

/// `PostComment` as an id: `post-comment`.
pub fn id_of(name: &str) -> String {
    let mut out = String::new();
    for (at, c) in name.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if at > 0 {
                out.push('-');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// `post-comment` as a name: `PostComment`.
pub fn name_of(id: &str) -> String {
    id.split('-')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

/// The `Yii::setAlias('@name', path)` calls of a file.
struct SetAlias<'a> {
    parsed: &'a ParsedFile,
    file: &'a Path,
    found: Vec<Alias>,
}

struct SetAliasWalker;

impl<'a> Walker<'a, 'a, SetAlias<'a>> for SetAliasWalker {
    fn walk_in_static_method_call(&self, call: &'a StaticMethodCall<'a>, cx: &mut SetAlias<'a>) {
        let ClassLikeMemberSelector::Identifier(method) = &call.method else { return };
        if !method.value.eq_ignore_ascii_case(b"setAlias") {
            return;
        }
        let mut arguments = call.argument_list.arguments.iter().filter_map(|argument| match argument {
            Argument::Positional(argument) => Some(argument.value),
            _ => None,
        });
        let Some((name, start, end)) =
            arguments.next().and_then(|first: &Expression<'_>| php_array::string_literal(first))
        else {
            return;
        };
        let name = if name.starts_with('@') { name } else { format!("@{name}") };
        let path = arguments.next().and_then(|path| path_of(cx.file, path));
        cx.found.push(Alias {
            name,
            path,
            declared: Some((cx.file.to_path_buf(), cx.parsed.line_index().range(start, end))),
        });
    }
}

impl YiiProject {
    /// Reads the project at `root`; `text_of` reads a file (an open buffer
    /// first).
    pub fn load(root: &Path, text_of: impl Fn(&Path) -> Option<String>) -> Self {
        let mut out = Self { root: root.to_path_buf(), ..Self::default() };
        let mut bases = vec![root.to_path_buf()];
        let mut folders: Vec<PathBuf> = std::fs::read_dir(root)
            .into_iter()
            .flatten()
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| {
                let name = path.file_name().and_then(|name| name.to_str()).unwrap_or_default();
                path.is_dir() && !name.starts_with('.') && !NOT_APPS.contains(&name)
            })
            .collect();
        folders.sort();
        bases.extend(folders);

        let mut fallback_aliases = Vec::new();
        let mut raw = Raw::default();
        for base in bases {
            let config = base.join("config");
            let files: Vec<PathBuf> =
                CONFIG_FILES.iter().map(|name| config.join(name)).filter(|file| file.is_file()).collect();
            if files.is_empty() {
                continue;
            }
            out.apps.push(base.clone());
            if base != root
                && let Some(name) = base.file_name().and_then(|name| name.to_str())
            {
                // The advanced template names each of its folders.
                fallback_aliases.push(Alias {
                    name: format!("@{name}"),
                    path: Some(base.to_string_lossy().into_owned()),
                    declared: None,
                });
            }
            for file in files {
                let Some(node) = php_array::read(&file, &text_of) else { continue };
                out.add_config(&base, &node, &mut raw);
            }
            // `Yii::setAlias()`: the bootstrap file first, then the other files of the
            // configuration and the entry scripts.
            let mut scripts = vec![config.join("bootstrap.php")];
            let mut others: Vec<PathBuf> = std::fs::read_dir(&config)
                .into_iter()
                .flatten()
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| path.extension().is_some_and(|ext| ext == "php") && !scripts.contains(path))
                .collect();
            others.sort();
            scripts.extend(others.into_iter().take(32));
            scripts.extend([base.join("web").join("index.php"), base.join("web").join("index-test.php")]);
            if base == root {
                scripts.push(base.join("yii"));
            }
            for script in scripts {
                let Some(text) = text_of(&script).filter(|text| text.contains("setAlias")) else { continue };
                let parsed = ParsedFile::parse(&script.to_string_lossy(), text);
                let mut found = SetAlias { parsed: &parsed, file: &script, found: Vec::new() };
                SetAliasWalker.walk_program(parsed.program(), &mut found);
                for alias in found.found {
                    if !out.aliases.iter().any(|known| known.name == alias.name) {
                        out.aliases.push(alias);
                    }
                }
            }
        }
        // The aliases of the extensions Composer installed.
        let vendor = root.join("vendor");
        let extensions = vendor.join("yiisoft").join("extensions.php");
        if let Some(text) = text_of(&extensions) {
            for alias in extension_aliases(&extensions, &vendor, text) {
                if !out.aliases.iter().any(|known| known.name == alias.name) {
                    out.aliases.push(alias);
                }
            }
        }
        if out.controller_namespaces.is_empty() {
            out.controller_namespaces.push("app\\controllers".to_owned());
        }

        // Yii's own aliases, below what the project declares.
        let app = out.apps.first().cloned().unwrap_or_else(|| root.to_path_buf());
        let own = |name: &str, path: PathBuf| Alias {
            name: name.to_owned(),
            path: Some(path.to_string_lossy().into_owned()),
            declared: None,
        };
        let mut defaults = vec![
            own("@app", app.clone()),
            own("@vendor", vendor.clone()),
            own("@runtime", app.join("runtime")),
            own("@webroot", app.join("web")),
            own("@yii", vendor.join("yiisoft").join("yii2")),
            own("@bower", vendor.join("bower-asset")),
            own("@npm", vendor.join("npm-asset")),
        ];
        defaults.extend(fallback_aliases);
        for alias in defaults {
            if !out.aliases.iter().any(|known| known.name == alias.name) {
                out.aliases.push(alias);
            }
        }

        // Messages: each application's `messages`, and where the sources say.
        let mut folders: Vec<(PathBuf, Vec<(String, String)>)> =
            out.apps.iter().map(|app| (app.join("messages"), Vec::new())).collect();
        for (_, base, file_map) in std::mem::take(&mut raw.sources) {
            let Some(base) = base.and_then(|base| out.alias_path(&base)) else { continue };
            match folders.iter_mut().find(|(known, _)| *known == base) {
                Some((_, known)) => known.extend(file_map),
                None => folders.push((base, file_map)),
            }
        }
        for (folder, file_map) in folders {
            out.add_messages(&folder, &file_map, &text_of);
        }

        // Where views are when the configuration moves them.
        for (from, to) in raw.view_paths {
            if let Some(to) = out.alias_path(&to) {
                out.view_maps.push(ViewMap { from, to, theme: false });
            }
        }
        for (class, below, to) in raw.module_views {
            let namespace = class.rsplit_once('\\').map_or("", |(namespace, _)| namespace);
            let (Some(folder), Some(to)) = (out.alias_path(&namespace_alias(namespace)), out.alias_path(&to)) else {
                continue;
            };
            out.view_maps.push(ViewMap { from: folder.join(below), to, theme: false });
        }
        for (from, to) in raw.themes {
            if let (Some(from), Some(to)) = (out.alias_path(&from), out.alias_path(&to)) {
                out.view_maps.push(ViewMap { from, to, theme: true });
            }
        }
        for (app, folder) in raw.mail {
            if let Some(folder) = out.alias_path(&folder) {
                out.mail_paths.push((app, folder));
            }
        }
        let mut migrations: Vec<PathBuf> = out.apps.iter().map(|app| app.join("migrations")).collect();
        migrations.extend(raw.migrations.iter().filter_map(|path| out.alias_path(path)));
        for folder in migrations {
            if folder.is_dir() && !out.migration_paths.contains(&folder) {
                out.migration_paths.push(folder);
            }
        }
        out
    }

    fn add_config(&mut self, base: &Path, node: &Node, raw: &mut Raw) {
        let def = |entry: &php_array::Entry, detail: Option<String>| {
            Some(Def { name: entry.name()?.to_owned(), detail, file: entry.file.to_path_buf(), range: entry.range })
        };
        let section = |key: &str| node.get(key).map(Node::entries).unwrap_or_default();
        for entry in section("components") {
            self.components.extend(def(entry, configured_class(&entry.value)));
            if entry.name() == Some("user")
                && let Some(identity) = entry.value.get("identityClass").and_then(Node::class)
            {
                self.identity = Some(identity.to_owned());
            }
            if entry.name() == Some("i18n") {
                for source in entry.value.get("translations").map(Node::entries).unwrap_or_default() {
                    let base = source.value.get("basePath").and_then(Node::as_str).map(str::to_owned);
                    let file_map = source.value.get("fileMap").map(Node::entries).unwrap_or_default();
                    let file_map = file_map
                        .iter()
                        .filter_map(|mapped| Some((mapped.name()?.to_owned(), mapped.value.as_str()?.to_owned())))
                        .collect();
                    raw.sources.push((source.file.to_path_buf(), base, file_map));
                }
            }
            if entry.name() == Some("mailer")
                && let Some(path) = entry.value.get("viewPath").and_then(Node::as_str)
            {
                raw.mail.push((base.to_path_buf(), path.to_owned()));
            }
            // A theme: its `pathMap`, else the application's folder to the theme's.
            if entry.name() == Some("view")
                && let Some(theme) = entry.value.get("theme")
            {
                let mapped = theme.get("pathMap").map(Node::entries).unwrap_or_default();
                for map in mapped {
                    let Some(from) = map.name() else { continue };
                    raw.themes.extend(strings(&map.value).into_iter().map(|to| (from.to_owned(), to)));
                }
                if mapped.is_empty()
                    && let Some(to) = theme.get("basePath").and_then(Node::as_str)
                {
                    raw.themes.push((base.to_string_lossy().into_owned(), to.to_owned()));
                }
            }
        }
        if let Some(path) = node.get("viewPath").and_then(Node::as_str) {
            raw.view_paths.push((base.join("views"), path.to_owned()));
        }
        if let Some(path) = node.get("layoutPath").and_then(Node::as_str) {
            raw.view_paths.push((base.join("views").join("layouts"), path.to_owned()));
        }
        let mut controllers = |prefix: &str, map: &Node, raw: &mut Raw| {
            for entry in map.entries() {
                let Some(name) = entry.name() else { continue };
                self.controller_map.push(Def {
                    name: format!("{prefix}{name}"),
                    detail: configured_class(&entry.value),
                    file: entry.file.to_path_buf(),
                    range: entry.range,
                });
                // The console's `migrate` says where the migrations are.
                if let Some(paths) = entry.value.get("migrationPath") {
                    raw.migrations.extend(strings(paths));
                }
                if let Some(namespaces) = entry.value.get("migrationNamespaces") {
                    raw.migrations.extend(strings(namespaces).iter().map(|namespace| namespace_alias(namespace)));
                }
            }
        };
        if let Some(map) = node.get("controllerMap") {
            controllers("", map, raw);
        }
        for entry in section("modules") {
            let class = configured_class(&entry.value);
            if let (Some(name), Some(map)) = (entry.name(), entry.value.get("controllerMap")) {
                controllers(&format!("{name}/"), map, raw);
            }
            if let Some(class) = &class {
                for (key, below) in [("viewPath", "views"), ("layoutPath", "views/layouts")] {
                    if let Some(path) = entry.value.get(key).and_then(Node::as_str) {
                        raw.module_views.push((class.clone(), below, path.to_owned()));
                    }
                }
            }
            self.modules.extend(def(entry, class));
        }
        for entry in section("params") {
            self.params.extend(def(entry, param_detail(&entry.value)));
            nested_params(entry, &mut Vec::new(), &mut self.nested_params);
        }
        for entry in section("aliases") {
            let Some(name) = entry.name() else { continue };
            self.aliases.push(Alias {
                name: if name.starts_with('@') { name.to_owned() } else { format!("@{name}") },
                path: entry.value.as_str().map(str::to_owned),
                declared: Some((entry.file.to_path_buf(), entry.range)),
            });
        }
        if let Some(namespace) = node.get("controllerNamespace").and_then(Node::as_str) {
            let namespace = namespace.trim_matches('\\').to_owned();
            if !namespace.is_empty() && !self.controller_namespaces.contains(&namespace) {
                self.controller_namespaces.push(namespace);
            }
        }
    }

    /// The messages of the files of `folder/<language>/`.
    fn add_messages(
        &mut self,
        folder: &Path,
        file_map: &[(String, String)],
        text_of: &impl Fn(&Path) -> Option<String>,
    ) {
        let mut languages: Vec<PathBuf> = std::fs::read_dir(folder)
            .into_iter()
            .flatten()
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.is_dir())
            .collect();
        languages.sort();
        for language in languages {
            let Some(name) = language.file_name().and_then(|name| name.to_str()).map(str::to_owned) else { continue };
            // Gettext: a message's context is its category.
            let mut catalogues: Vec<PathBuf> = std::fs::read_dir(&language)
                .into_iter()
                .flatten()
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| path.extension().is_some_and(|ext| ext == "po"))
                .collect();
            catalogues.sort();
            for file in catalogues.into_iter().take(32) {
                let Some(text) = text_of(&file) else { continue };
                for (category, message, translation, line, (start, end)) in po_messages(&text) {
                    self.messages.push(Message {
                        category,
                        message,
                        language: name.clone(),
                        translation,
                        file: file.clone(),
                        range: Range::new(Pos::new(line, start), Pos::new(line, end)),
                    });
                }
            }
            let mut files = Vec::new();
            php_files(&language, 0, &mut files);
            for file in files.into_iter().take(512) {
                let Ok(relative) = file.strip_prefix(&language) else { continue };
                let relative = relative.to_string_lossy().replace('\\', "/");
                let mapped: Vec<String> = file_map
                    .iter()
                    .filter(|(_, mapped)| *mapped == relative)
                    .map(|(category, _)| category.clone())
                    .collect();
                let categories =
                    if mapped.is_empty() { vec![relative.trim_end_matches(".php").to_owned()] } else { mapped };
                let Some(node) = php_array::read(&file, text_of) else { continue };
                for entry in node.entries() {
                    let Some(message) = entry.name() else { continue };
                    for category in &categories {
                        self.messages.push(Message {
                            category: category.clone(),
                            message: message.to_owned(),
                            language: name.clone(),
                            translation: entry.value.as_str().filter(|text| !text.is_empty()).map(str::to_owned),
                            file: entry.file.to_path_buf(),
                            range: entry.range,
                        });
                    }
                }
            }
        }
    }

    /// The folder or file an aliased path names (`@app/views/site`): the
    /// longest alias it starts with, followed through other aliases.
    pub fn alias_path(&self, value: &str) -> Option<PathBuf> {
        let mut value = value.to_owned();
        for _ in 0..8 {
            if !value.starts_with('@') {
                return Some(tidy(Path::new(&value)));
            }
            let alias = self
                .aliases
                .iter()
                .filter(|alias| {
                    value == alias.name || value.strip_prefix(&alias.name).is_some_and(|rest| rest.starts_with('/'))
                })
                .max_by_key(|alias| alias.name.len())?;
            value = format!("{}{}", alias.path.as_deref()?, &value[alias.name.len()..]);
        }
        None
    }

    /// The alias a path starts with (`@app` of `@app/runtime`), the longest
    /// known.
    pub fn alias_of(&self, value: &str) -> Option<&Alias> {
        self.aliases
            .iter()
            .filter(|alias| {
                value == alias.name || value.strip_prefix(&alias.name).is_some_and(|rest| rest.starts_with('/'))
            })
            .max_by_key(|alias| alias.name.len())
    }

    /// The base path of the application the file `path` is in, else the
    /// first application's.
    pub fn app_of(&self, path: &Path) -> PathBuf {
        let inside = self.apps.iter().filter(|app| path.starts_with(app)).max_by_key(|app| app.as_os_str().len());
        inside.or(self.apps.first()).cloned().unwrap_or_else(|| self.root.clone())
    }

    /// The folder a view named without a path is looked for in, from the
    /// file `site` naming it: beside a view, the controller's folder of
    /// views (`views/post-comment` for `controllers/PostCommentController.php`),
    /// the `views` beside any other class (a widget).
    pub fn view_dir(&self, site: &Path) -> Option<PathBuf> {
        let site = &self.logical(site);
        let folder = site.parent()?;
        let stem = site.file_stem()?.to_str()?;
        let parts: Vec<&std::ffi::OsStr> = folder.components().map(|part| part.as_os_str()).collect();
        if let Some(controller) = stem.strip_suffix("Controller")
            && let Some(at) = parts.iter().rposition(|part| *part == "controllers")
        {
            let mut out: PathBuf = parts[..at].iter().collect();
            out.push("views");
            out.extend(&parts[at + 1..]);
            out.push(id_of(controller));
            return Some(out);
        }
        if parts.iter().any(|part| *part == "views") {
            return Some(folder.to_path_buf());
        }
        Some(folder.join("views"))
    }

    /// The `views` folder of the module (or the application) the file
    /// `site` is in: what a name starting with one slash is relative to.
    pub fn views_root(&self, site: &Path) -> PathBuf {
        let site = &self.logical(site);
        let parts: Vec<&std::ffi::OsStr> = site.components().map(|part| part.as_os_str()).collect();
        let at = parts.iter().rposition(|part| *part == "views" || *part == "controllers" || *part == "widgets");
        match at {
            Some(at) => parts[..at].iter().collect::<PathBuf>().join("views"),
            None => self.app_of(site).join("views"),
        }
    }

    /// The file of the view `name` asked for from the file `site`, as Yii's
    /// guide says a view is found: an alias, `//` from the application's
    /// views, `/` from the module's, else the folder of
    /// [`view_dir`](Self::view_dir). A name without an extension is a
    /// `.php` file.
    pub fn view_file(&self, site: &Path, name: &str) -> Option<PathBuf> {
        self.named_file(site, name, self.view_dir(site)?)
    }

    /// The file of the layout `name`: like a view, in the `layouts` of the
    /// module's views.
    pub fn layout_file(&self, site: &Path, name: &str) -> Option<PathBuf> {
        self.named_file(site, name, self.views_root(site).join("layouts"))
    }

    fn named_file(&self, site: &Path, name: &str, folder: PathBuf) -> Option<PathBuf> {
        if name.is_empty() || name.contains(['$', '{']) {
            return None;
        }
        let file = if name.starts_with('@') {
            self.alias_path(name)?
        } else if let Some(rest) = name.strip_prefix("//") {
            self.app_of(site).join("views").join(rest)
        } else if let Some(rest) = name.strip_prefix('/') {
            self.views_root(site).join(rest)
        } else {
            folder.join(name)
        };
        let file = tidy(&file);
        Some(self.actual(&if file.extension().is_some() { file } else { file.with_extension("php") }))
    }

    /// The file `file` stands for where views are by default: a file of a
    /// theme or of a `viewPath` as the file of `views` it is used for.
    pub fn logical(&self, file: &Path) -> PathBuf {
        let mut out = file.to_path_buf();
        for theme in [true, false] {
            let maps = self.view_maps.iter().filter(|map| map.theme == theme && out.starts_with(&map.to));
            if let Some(map) = maps.max_by_key(|map| map.to.as_os_str().len())
                && let Ok(rest) = out.strip_prefix(&map.to)
            {
                out = map.from.join(rest);
            }
        }
        out
    }

    /// The file used for the view file `file`: the one of the `viewPath`,
    /// and a theme's when the theme has it.
    pub fn actual(&self, file: &Path) -> PathBuf {
        self.candidates(file).into_iter().find(|candidate| candidate.is_file()).unwrap_or_else(|| self.moved(file))
    }

    /// `file` in the `viewPath` replacing its folder.
    fn moved(&self, file: &Path) -> PathBuf {
        let maps = self.view_maps.iter().filter(|map| !map.theme && file.starts_with(&map.from));
        match maps.max_by_key(|map| map.from.as_os_str().len()) {
            Some(map) => map.to.join(file.strip_prefix(&map.from).unwrap_or(file)),
            None => file.to_path_buf(),
        }
    }

    /// Where the view file (or folder) `file` may be: in the themes, the
    /// longest mapped folder first, then where the `viewPath` says.
    pub fn candidates(&self, file: &Path) -> Vec<PathBuf> {
        let moved = self.moved(file);
        let mut themes: Vec<&ViewMap> =
            self.view_maps.iter().filter(|map| map.theme && moved.starts_with(&map.from)).collect();
        themes.sort_by_key(|map| std::cmp::Reverse(map.from.as_os_str().len()));
        let mut out: Vec<PathBuf> =
            themes.into_iter().filter_map(|map| Some(map.to.join(moved.strip_prefix(&map.from).ok()?))).collect();
        out.push(moved);
        out
    }

    /// The folder of the mail views asked for from the file `site`: the
    /// `viewPath` of the mailer of its application, else of any (the
    /// advanced template's `@common/mail`), else its `mail`, else the
    /// `mail` folder another application has.
    pub fn mail_dir(&self, site: &Path) -> PathBuf {
        let app = self.app_of(site);
        let own = self.mail_paths.iter().find(|(known, _)| *known == app);
        if let Some((_, folder)) = own.or(self.mail_paths.first()) {
            return folder.clone();
        }
        // Not configured (the advanced template's local files aren't there before `init`): the
        // application's `mail`, else the one that has such a folder.
        let own = app.join("mail");
        if own.is_dir() {
            return own;
        }
        self.apps.iter().map(|app| app.join("mail")).find(|folder| folder.is_dir()).unwrap_or(own)
    }

    /// Whether `path` is in a folder of views: `views`, `mail`, or where
    /// the configuration puts them.
    pub fn is_view(&self, path: &Path) -> bool {
        let Some(folder) = path.parent() else { return false };
        folder.components().any(|part| part.as_os_str() == "views" || part.as_os_str() == "mail")
            || self.view_maps.iter().any(|map| folder.starts_with(&map.to))
            || self.mail_paths.iter().any(|(_, mail)| folder.starts_with(mail))
    }

    /// The class `controllerMap` gives the controller `id` (a module's
    /// after the module's id).
    pub fn mapped_controller(&self, id: &str) -> Option<&Def> {
        self.controller_map.iter().find(|def| def.name == id && def.detail.is_some())
    }

    pub fn component(&self, name: &str) -> Option<&Def> {
        // The first application's, a class before none.
        let found = self.components.iter().filter(|def| def.name == name);
        found.clone().find(|def| def.detail.is_some()).or_else(|| found.clone().next())
    }

    pub fn module(&self, name: &str) -> Option<&Def> {
        self.modules.iter().find(|def| def.name == name)
    }
}

/// The aliases `vendor/yiisoft/extensions.php` lists: each extension's
/// `'alias' => ['@yii/bootstrap' => $vendorDir . '/yiisoft/yii2-bootstrap/src']`.
fn extension_aliases(file: &Path, vendor: &Path, text: String) -> Vec<Alias> {
    fn path(file: &Path, vendor: &Path, expr: &Expression<'_>) -> Option<String> {
        match expr {
            Expression::Binary(binary) if matches!(binary.operator, BinaryOperator::StringConcat(_)) => {
                Some(path(file, vendor, binary.lhs)? + &path(file, vendor, binary.rhs)?)
            }
            Expression::Variable(Variable::Direct(variable)) if variable.name == b"$vendorDir" => {
                Some(vendor.to_string_lossy().into_owned())
            }
            other => path_of(file, other),
        }
    }
    let parsed = ParsedFile::parse(&file.to_string_lossy(), text);
    let Some(array) = php_array::returned_array(&parsed) else { return Vec::new() };
    let mut out = Vec::new();
    for extension in php_array::elements(array) {
        let ArrayElement::KeyValue(extension) = extension else { continue };
        for field in php_array::elements(extension.value) {
            let ArrayElement::KeyValue(field) = field else { continue };
            if php_array::string_literal(field.key).is_none_or(|(key, _, _)| key != "alias") {
                continue;
            }
            for alias in php_array::elements(field.value) {
                let ArrayElement::KeyValue(alias) = alias else { continue };
                let Some((name, start, end)) = php_array::string_literal(alias.key) else { continue };
                out.push(Alias {
                    name: if name.starts_with('@') { name } else { format!("@{name}") },
                    path: path(file, vendor, alias.value),
                    declared: Some((file.to_path_buf(), parsed.line_index().range(start, end))),
                });
            }
        }
    }
    out
}

/// The PHP files below `folder`, sorted.
fn php_files(folder: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(folder) else { return };
    let mut entries: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            if depth < 6 {
                php_files(&path, depth + 1, out);
            }
        } else if path.extension().is_some_and(|ext| ext == "php") {
            out.push(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(files: &[(&str, &str)]) -> (tempfile::TempDir, YiiProject) {
        let tmp = tempfile::tempdir().unwrap();
        for (rel, text) in files {
            let path = tmp.path().join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
        let project = YiiProject::load(tmp.path(), |path| std::fs::read_to_string(path).ok());
        (tmp, project)
    }

    fn shown(root: &Path, path: &Path) -> String {
        path.strip_prefix(root).unwrap_or(path).to_string_lossy().replace('\\', "/")
    }

    const WEB: &str = "<?php\n$params = require __DIR__ . '/params.php';\n$config = [\n    'id' => 'basic',\n    'basePath' => dirname(__DIR__),\n    'aliases' => ['@uploads' => '@app/web/uploads'],\n    'components' => [\n        'cache' => ['class' => 'yii\\caching\\FileCache'],\n        'user' => ['identityClass' => 'app\\models\\User'],\n        'mailer' => \\yii\\symfonymailer\\Mailer::class,\n        'db' => require __DIR__ . '/db.php',\n        'i18n' => ['translations' => ['shop*' => ['class' => 'yii\\i18n\\PhpMessageSource', 'basePath' => '@app/lang', 'fileMap' => ['shop/cart' => 'cart.php']]]],\n    ],\n    'modules' => ['admin' => ['class' => 'app\\modules\\admin\\Module']],\n    'params' => $params,\n];\nif (YII_ENV_DEV) {\n    $config['modules']['debug'] = ['class' => 'yii\\debug\\Module'];\n}\nreturn $config;\n";

    #[test]
    fn the_configuration_names_components_modules_params_and_aliases() {
        let (tmp, project) = project(&[
            ("config/web.php", WEB),
            ("config/params.php", "<?php\nreturn ['adminEmail' => 'admin@example.com', 'pageSize' => 20];\n"),
            ("config/db.php", "<?php\nreturn ['class' => 'yii\\db\\Connection', 'dsn' => 'sqlite::memory:'];\n"),
            ("config/bootstrap.php", "<?php\nYii::setAlias('@docs', dirname(__DIR__) . '/docs');\n"),
            ("messages/de/app.php", "<?php\nreturn ['Hello' => 'Hallo', 'Bye' => ''];\n"),
            ("lang/de/cart.php", "<?php\nreturn ['Empty' => 'Leer'];\n"),
        ]);
        let root = tmp.path();
        assert_eq!(project.apps, [root.to_path_buf()]);
        let components: Vec<(&str, Option<&str>)> =
            project.components.iter().map(|def| (def.name.as_str(), def.detail.as_deref())).collect();
        assert_eq!(
            components,
            [
                ("cache", Some("yii\\caching\\FileCache")),
                ("user", None),
                ("mailer", Some("yii\\symfonymailer\\Mailer")),
                ("db", Some("yii\\db\\Connection")),
                ("i18n", None),
            ]
        );
        assert_eq!(project.identity.as_deref(), Some("app\\models\\User"));
        let modules: Vec<&str> = project.modules.iter().map(|def| def.name.as_str()).collect();
        assert_eq!(modules, ["admin", "debug"]);
        // A parameter is where its file writes it.
        let email = project.params.iter().find(|def| def.name == "adminEmail").unwrap();
        assert_eq!(shown(root, &email.file), "config/params.php");
        assert_eq!(email.detail.as_deref(), Some("'admin@example.com'"));
        assert_eq!(project.controller_namespaces, ["app\\controllers"]);

        let path = |value: &str| project.alias_path(value).map(|path| shown(root, &path));
        assert_eq!(path("@app/models").as_deref(), Some("models"));
        assert_eq!(path("@uploads/a.png").as_deref(), Some("web/uploads/a.png"));
        assert_eq!(path("@docs").as_deref(), Some("docs"));
        assert_eq!(path("@yii/base").as_deref(), Some("vendor/yiisoft/yii2/base"));
        assert_eq!(path("@nothing/x"), None);
        assert!(project.alias_of("@docs/x").unwrap().declared.is_some());

        let messages: Vec<(&str, &str, Option<&str>)> = project
            .messages
            .iter()
            .map(|message| (message.category.as_str(), message.message.as_str(), message.translation.as_deref()))
            .collect();
        assert_eq!(
            messages,
            [("app", "Hello", Some("Hallo")), ("app", "Bye", None), ("shop/cart", "Empty", Some("Leer"))]
        );
    }

    #[test]
    fn the_advanced_template_has_an_application_per_folder() {
        let (tmp, project) = project(&[
            (
                "common/config/main.php",
                "<?php\nreturn ['components' => ['cache' => ['class' => 'yii\\caching\\FileCache']]];\n",
            ),
            (
                "common/config/bootstrap.php",
                "<?php\nYii::setAlias('@common', dirname(__DIR__));\nYii::setAlias('@frontend', dirname(dirname(__DIR__)) . '/frontend');\n",
            ),
            (
                "frontend/config/main.php",
                "<?php\nreturn ['id' => 'app-frontend', 'controllerNamespace' => 'frontend\\controllers', 'components' => ['user' => ['identityClass' => 'common\\models\\User']]];\n",
            ),
            ("backend/config/main.php", "<?php\nreturn ['controllerNamespace' => 'backend\\controllers'];\n"),
            ("vendor/x/config/main.php", "<?php\nreturn ['components' => ['no' => 'a\\B']];\n"),
        ]);
        let root = tmp.path();
        let apps: Vec<String> = project.apps.iter().map(|app| shown(root, app)).collect();
        assert_eq!(apps, ["backend", "common", "frontend"]);
        assert_eq!(project.controller_namespaces, ["backend\\controllers", "frontend\\controllers"]);
        assert_eq!(project.identity.as_deref(), Some("common\\models\\User"));
        assert!(project.component("no").is_none());
        let path = |value: &str| project.alias_path(value).map(|path| shown(root, &path));
        assert_eq!(path("@frontend/views").as_deref(), Some("frontend/views"));
        // Not written in a bootstrap file: the folder's name.
        assert_eq!(path("@backend/web").as_deref(), Some("backend/web"));
        assert_eq!(shown(root, &project.app_of(&root.join("frontend/controllers/SiteController.php"))), "frontend");
    }

    #[test]
    fn the_configuration_moves_views_and_maps_controllers() {
        let (tmp, project) = project(&[
            (
                "config/web.php",
                "<?php\nreturn [\n    'layoutPath' => '@app/chrome',\n    'controllerMap' => ['account' => 'app\\extra\\ProfileController'],\n    'components' => ['view' => ['theme' => ['basePath' => '@app/themes/dark']]],\n    'modules' => ['admin' => ['class' => 'app\\modules\\admin\\Module', 'viewPath' => '@app/adminviews', 'controllerMap' => ['log' => 'app\\extra\\LogController']]],\n    'params' => ['mail' => ['from' => 'a@b.c', 'smtp' => ['host' => 'h']]],\n];\n",
            ),
            (
                "config/console.php",
                "<?php\nreturn ['controllerMap' => ['migrate' => ['class' => 'yii\\console\\controllers\\MigrateController', 'migrationPath' => ['@app/db/changes'], 'migrationNamespaces' => ['app\\extra\\migrations']]]];\n",
            ),
            ("themes/dark/views/site/index.php", "<p></p>\n"),
            ("db/changes/m1.php", "<?php\n"),
            ("extra/migrations/m2.php", "<?php\n"),
            ("migrations/m3.php", "<?php\n"),
            ("messages/fr/messages.po", "msgctxt \"shop\"\nmsgid \"\"\n\"Cart \\\"A\\\"\"\nmsgstr \"Panier\"\n"),
        ]);
        let root = tmp.path();
        let view = |site: &str, name: &str| project.view_file(&root.join(site), name).map(|file| shown(root, &file));
        // A theme without a `pathMap` maps the application's folder; its file where it has one.
        assert_eq!(
            view("controllers/SiteController.php", "index").as_deref(),
            Some("themes/dark/views/site/index.php")
        );
        assert_eq!(view("controllers/SiteController.php", "about").as_deref(), Some("views/site/about.php"));
        assert_eq!(view("themes/dark/views/site/index.php", "_row").as_deref(), Some("views/site/_row.php"));
        // A module's `viewPath`, the application's `layoutPath`.
        assert_eq!(
            view("modules/admin/controllers/UserController.php", "list").as_deref(),
            Some("adminviews/user/list.php")
        );
        assert_eq!(view("adminviews/user/list.php", "_row").as_deref(), Some("adminviews/user/_row.php"));
        let layout = project.layout_file(&root.join("controllers/SiteController.php"), "main");
        assert_eq!(layout.map(|file| shown(root, &file)).as_deref(), Some("chrome/main.php"));
        assert!(
            project.is_view(&root.join("adminviews/user/list.php")) && !project.is_view(&root.join("models/A.php"))
        );

        let mapped = |id: &str| project.mapped_controller(id).and_then(|def| def.detail.clone());
        assert_eq!(mapped("account").as_deref(), Some("app\\extra\\ProfileController"));
        assert_eq!(mapped("admin/log").as_deref(), Some("app\\extra\\LogController"));
        let nested: Vec<(String, &str)> =
            project.nested_params.iter().map(|(above, def)| (above.join("."), def.name.as_str())).collect();
        assert_eq!(
            nested,
            [("mail".to_owned(), "from"), ("mail".to_owned(), "smtp"), ("mail.smtp".to_owned(), "host")]
        );
        let migrations: Vec<String> = project.migration_paths.iter().map(|path| shown(root, path)).collect();
        assert_eq!(migrations, ["migrations", "db/changes", "extra/migrations"]);
        // A gettext message under its context, where its text is.
        let message = project.messages.iter().find(|message| message.category == "shop").unwrap();
        assert_eq!((message.message.as_str(), message.translation.as_deref()), ("Cart \"A\"", Some("Panier")));
        assert_eq!(message.range.start.line, 2);
    }

    #[test]
    fn a_view_is_found_from_who_asks_for_it() {
        let (tmp, project) = project(&[("config/web.php", "<?php\nreturn ['id' => 'basic'];\n")]);
        let root = tmp.path();
        let view = |site: &str, name: &str| project.view_file(&root.join(site), name).map(|file| shown(root, &file));
        assert_eq!(view("controllers/SiteController.php", "index").as_deref(), Some("views/site/index.php"));
        assert_eq!(
            view("controllers/admin/PostCommentController.php", "view").as_deref(),
            Some("views/admin/post-comment/view.php")
        );
        assert_eq!(
            view("modules/shop/controllers/CartController.php", "show").as_deref(),
            Some("modules/shop/views/cart/show.php")
        );
        assert_eq!(
            view("modules/shop/controllers/CartController.php", "/layouts/x").as_deref(),
            Some("modules/shop/views/layouts/x.php")
        );
        assert_eq!(
            view("modules/shop/controllers/CartController.php", "//site/about").as_deref(),
            Some("views/site/about.php")
        );
        assert_eq!(view("views/site/index.php", "_form").as_deref(), Some("views/site/_form.php"));
        assert_eq!(view("views/site/index.php", "../shared/menu.tpl").as_deref(), Some("views/shared/menu.tpl"));
        assert_eq!(view("widgets/Menu.php", "menu").as_deref(), Some("widgets/views/menu.php"));
        assert_eq!(view("views/site/index.php", "@app/mail/hello").as_deref(), Some("mail/hello.php"));
        assert_eq!(view("views/site/index.php", "$name"), None);
        let layout = project.layout_file(&root.join("controllers/SiteController.php"), "main");
        assert_eq!(layout.map(|file| shown(root, &file)).as_deref(), Some("views/layouts/main.php"));
        assert_eq!((id_of("PostComment").as_str(), name_of("post-comment").as_str()), ("post-comment", "PostComment"));
    }
}
