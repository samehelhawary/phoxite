//! What a CakePHP application's files say that isn't a PHP declaration
//! ([`CakeProject`]):
//!
//! - **The application's namespace:** `App.namespace` of `config/app.php`,
//!   else the namespace Composer loads from `src/`, else `App`.
//! - **Plugins:** the folders of `plugins/` (`plugins/Blog`,
//!   `plugins/Company/Blog`) and the ones Composer installed
//!   (`vendor/cakephp-plugins.php`). A plugin's classes are in the namespace
//!   of its name (`Company\Blog`), its templates in its `templates` folder.
//! - **Configuration:** the keys of `config/app.php`, `config/app_local.php`
//!   and the files `config/bootstrap.php` loads with `Configure::load()`,
//!   each as its dotted path (`Datasources.default.host`).
//! - **Named routes:** the `'_name' => 'login'` options and the names given
//!   to `get()`, `post()` and the like in `config/routes.php`, a plugin's
//!   routes file and the application class.
//!
//! Templates are files, found when a name is resolved
//! ([`CakeProject::template_files`]). The behaviour follows CakePHP's book;
//! the engine keeps the result for an edit count.

use std::path::{Path, PathBuf};

use mago_syntax::cst::{
    Argument, ArrayElement, BinaryOperator, ClassLikeMemberSelector, Expression, MethodCall, Variable,
};
use mago_syntax::walker::Walker;

use crate::base::Range;
use crate::frameworks::php_array::{self, Node, elements, path_of, string_literal, tidy};
use crate::syntax::ParsedFile;

/// A name a file declares.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Def {
    pub name: String,
    /// A configuration value.
    pub detail: Option<String>,
    pub file: PathBuf,
    pub range: Range,
}

/// A plugin.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plugin {
    /// `Blog`, `Company/Blog`.
    pub name: String,
    /// Its folder.
    pub path: PathBuf,
    /// One of the application's `plugins` folder.
    pub local: bool,
}

impl Plugin {
    /// The namespace of the plugin's classes.
    pub fn namespace(&self) -> String {
        plugin_namespace(&self.name)
    }
}

/// The namespace of the classes of the plugin `name`.
pub fn plugin_namespace(name: &str) -> String {
    name.replace('/', "\\")
}

/// See the module documentation.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CakeProject {
    pub root: PathBuf,
    /// The application's namespace, without backslashes around it.
    pub namespace: String,
    pub plugins: Vec<Plugin>,
    /// The configuration's keys, each a dotted path.
    pub config: Vec<Def>,
    /// The routes that have a name.
    pub routes: Vec<Def>,
    /// The messages of the translation catalogues.
    pub messages: Vec<Message>,
    /// The folders migrations are in.
    pub migration_paths: Vec<PathBuf>,
    /// CakePHP 3's layout: templates in `src/Template`, as `.ctp` files.
    pub legacy: bool,
}

/// A message of a translation catalogue (`resources/locales/fr/default.po`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Message {
    /// The catalogue: `default`, or a plugin's.
    pub domain: String,
    /// `msgctxt`, for `__x()`.
    pub context: Option<String>,
    pub message: String,
    pub language: String,
    pub translation: Option<String>,
    pub file: PathBuf,
    pub range: Range,
}

/// What the code asking says about where its templates are.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Overrides {
    /// `setTheme('Modern')`: a plugin whose templates come first.
    pub theme: Option<String>,
    /// `setTemplatePath('Common')`: the folder of its templates.
    pub template_path: Option<String>,
    /// `setLayoutPath('admin')`: a folder below `layout`.
    pub layout_path: Option<String>,
}

/// The folders of `templates` that aren't a controller's.
const RESERVED: [&str; 5] = ["cell", "element", "layout", "email", "plugin"];

/// Where the templates a file asks for by name are.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TemplateContext {
    /// The `templates` folders, the first winning: the application's
    /// override of a plugin's, then the plugin's own.
    pub roots: Vec<PathBuf>,
    /// The folder below a root the file's own templates are in:
    /// `Articles`, `Admin/Articles`, `cell/Inbox`, `email/html`.
    pub folder: String,
    /// The plugin the file is of.
    pub plugin: Option<String>,
}

impl TemplateContext {
    /// The controller the folder is of (`Articles` of `Admin/Articles`) and
    /// the prefix before it (`Admin`); `None` for a cell's, a layout's and
    /// the like.
    pub fn controller(&self) -> Option<(&str, &str)> {
        let (prefix, name) = self.folder.rsplit_once('/').unwrap_or(("", &self.folder));
        let reserved = RESERVED;
        let first = self.folder.split('/').next().unwrap_or_default();
        (!name.is_empty() && name.starts_with(|c: char| c.is_ascii_uppercase()) && !reserved.contains(&first))
            .then_some((prefix, name))
    }
}

/// The kind of file a template name is of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TemplateKind {
    /// A template of the asking file's folder.
    Template,
    /// A file of `layout`.
    Layout,
    /// A file of `element`.
    Element,
}

/// What a value is, for a hover.
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

/// The keys below `node`, each with the keys above it joined with dots.
fn flatten(node: &Node, above: &str, depth: usize, out: &mut Vec<Def>) {
    if depth > 6 || out.len() >= 5000 {
        return;
    }
    for entry in node.entries() {
        let Some(name) = entry.name() else { continue };
        let path = if above.is_empty() { name.to_owned() } else { format!("{above}.{name}") };
        out.push(Def {
            name: path.clone(),
            detail: detail(&entry.value),
            file: entry.file.to_path_buf(),
            range: entry.range,
        });
        flatten(&entry.value, &path, depth + 1, out);
    }
}

/// `Blog.Articles` is the plugin `Blog` and `Articles`; a name without a
/// plugin has none. (A name's first dot ends its plugin, as CakePHP's
/// `pluginSplit()` has it.)
pub fn plugin_split(name: &str) -> (Option<&str>, &str) {
    match name.split_once('.') {
        Some((plugin, rest)) if plugin.starts_with(|c: char| c.is_ascii_uppercase()) && !rest.is_empty() => {
            (Some(plugin), rest)
        }
        _ => (None, name),
    }
}

/// The named routes of a routes file.
struct Routes<'a> {
    parsed: &'a ParsedFile,
    file: &'a Path,
    found: Vec<Def>,
}

impl Routes<'_> {
    fn add(&mut self, name: (String, u32, u32)) {
        let (name, start, end) = name;
        if !name.is_empty() {
            self.found.push(Def {
                name,
                detail: None,
                file: self.file.to_path_buf(),
                range: self.parsed.line_index().range(start, end),
            });
        }
    }
}

struct RouteWalker;

impl<'a> Walker<'a, 'a, Routes<'a>> for RouteWalker {
    fn walk_in_method_call(&self, call: &'a MethodCall<'a>, cx: &mut Routes<'a>) {
        let ClassLikeMemberSelector::Identifier(method) = &call.method else { return };
        let name = String::from_utf8_lossy(method.value).to_ascii_lowercase();
        let arguments: Vec<&Expression<'_>> = call
            .argument_list
            .arguments
            .iter()
            .filter_map(|argument| match argument {
                Argument::Positional(argument) => Some(argument.value),
                Argument::Named(_) => None,
            })
            .collect();
        // `$routes->get('/login', [...], 'login')`.
        if ["get", "post", "put", "patch", "delete", "head", "options"].contains(&name.as_str())
            && let Some(named) = arguments.get(2).and_then(|argument| string_literal(argument))
        {
            cx.add(named);
        }
        // `$routes->connect('/login', [...], ['_name' => 'login'])`.
        if ["connect", "redirect", "resources"].contains(&name.as_str()) {
            for options in arguments.iter().skip(1) {
                for element in elements(options) {
                    let ArrayElement::KeyValue(element) = element else { continue };
                    if string_literal(element.key).is_some_and(|(key, _, _)| key == "_name")
                        && let Some(named) = string_literal(element.value)
                    {
                        cx.add(named);
                    }
                }
            }
        }
    }
}

/// The plugins `vendor/cakephp-plugins.php` lists:
/// `'plugins' => ['Bake' => $baseDir . '/vendor/cakephp/bake/']`.
fn installed_plugins(file: &Path, root: &Path, text: String) -> Vec<Plugin> {
    fn path(file: &Path, root: &Path, expr: &Expression<'_>) -> Option<String> {
        match expr {
            Expression::Binary(binary) if matches!(binary.operator, BinaryOperator::StringConcat(_)) => {
                Some(path(file, root, binary.lhs)? + &path(file, root, binary.rhs)?)
            }
            Expression::Variable(Variable::Direct(variable)) if variable.name == b"$baseDir" => {
                Some(root.to_string_lossy().into_owned())
            }
            other => path_of(file, other),
        }
    }
    let parsed = ParsedFile::parse(&file.to_string_lossy(), text);
    let Some(array) = php_array::returned_array(&parsed) else { return Vec::new() };
    let mut out = Vec::new();
    for field in elements(array) {
        let ArrayElement::KeyValue(field) = field else { continue };
        if string_literal(field.key).is_none_or(|(key, _, _)| key != "plugins") {
            continue;
        }
        for plugin in elements(field.value) {
            let ArrayElement::KeyValue(plugin) = plugin else { continue };
            let (Some((name, _, _)), Some(folder)) = (string_literal(plugin.key), path(file, root, plugin.value))
            else {
                continue;
            };
            out.push(Plugin { name, path: tidy(Path::new(&folder)), local: false });
        }
    }
    out
}

/// The namespace Composer loads from `src/`.
fn composer_namespace(text: &str) -> Option<String> {
    let json: serde_json::Value = serde_json::from_str(text).ok()?;
    let map = json.get("autoload")?.get("psr-4")?.as_object()?;
    map.iter().find_map(|(namespace, folder)| {
        let folders: Vec<&str> = match folder {
            serde_json::Value::String(folder) => vec![folder.as_str()],
            serde_json::Value::Array(folders) => folders.iter().filter_map(|folder| folder.as_str()).collect(),
            _ => Vec::new(),
        };
        let is_src = folders.iter().any(|folder| folder.trim_matches('/') == "src");
        (is_src && !namespace.trim_matches('\\').is_empty()).then(|| namespace.trim_matches('\\').to_owned())
    })
}

impl CakeProject {
    /// Reads the project at `root`; `text_of` reads a file (an open buffer
    /// first).
    pub fn load(root: &Path, text_of: impl Fn(&Path) -> Option<String>) -> Self {
        let mut out = Self { root: root.to_path_buf(), ..Self::default() };
        let config = root.join("config");

        // The plugins: the application's folders, then Composer's.
        let folders = |folder: &Path| -> Vec<PathBuf> {
            let mut found: Vec<PathBuf> = std::fs::read_dir(folder)
                .into_iter()
                .flatten()
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| {
                    path.is_dir() && path.file_name().is_some_and(|name| !name.to_string_lossy().starts_with('.'))
                })
                .collect();
            found.sort();
            found
        };
        let named = |path: &Path| path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
        for folder in folders(&root.join("plugins")) {
            if folder.join("src").is_dir() || folder.join("templates").is_dir() || folder.join("config").is_dir() {
                out.plugins.push(Plugin { name: named(&folder), path: folder, local: true });
                continue;
            }
            // `plugins/Company/Blog`.
            for inner in folders(&folder) {
                out.plugins.push(Plugin {
                    name: format!("{}/{}", named(&folder), named(&inner)),
                    path: inner,
                    local: true,
                });
            }
        }
        let installed = root.join("vendor").join("cakephp-plugins.php");
        if let Some(text) = text_of(&installed) {
            for plugin in installed_plugins(&installed, root, text) {
                if !out.plugins.iter().any(|known| known.name == plugin.name) {
                    out.plugins.push(plugin);
                }
            }
        }

        // The configuration: `app.php`, `app_local.php`, what the bootstrap file loads.
        let mut files = vec!["app".to_owned(), "app_local".to_owned()];
        if let Some(bootstrap) = text_of(&config.join("bootstrap.php")) {
            let mut rest = bootstrap.as_str();
            while let Some(at) = rest.find("Configure::load(") {
                rest = &rest[at + "Configure::load(".len()..];
                let quoted = rest.trim_start();
                if let Some(quote) = quoted.chars().next().filter(|c| matches!(c, '\'' | '"'))
                    && let Some(end) = quoted[1..].find(quote)
                {
                    let name = quoted[1..1 + end].to_owned();
                    if !files.contains(&name) && !name.contains('.') {
                        files.push(name);
                    }
                }
            }
        }
        for name in files {
            let file = config.join(format!("{name}.php"));
            if let Some(node) = php_array::read(&file, &text_of) {
                flatten(&node, "", 0, &mut out.config);
            }
        }

        let configured = out.config.iter().rev().find(|def| def.name == "App.namespace");
        let configured =
            configured.and_then(|def| def.detail.as_deref()).map(|text| text.trim_matches('\'').to_owned());
        let composer = || text_of(&root.join("composer.json")).and_then(|text| composer_namespace(&text));
        let namespace = configured.filter(|name| !name.is_empty() && !name.contains(' ')).or_else(composer);
        out.namespace = namespace.unwrap_or_else(|| "App".to_owned()).trim_matches('\\').to_owned();

        // The routes that have a name.
        let mut routes = vec![config.join("routes.php"), root.join("src").join("Application.php")];
        for plugin in out.plugins.iter().filter(|plugin| plugin.local) {
            routes.push(plugin.path.join("config").join("routes.php"));
            let class = plugin.name.rsplit('/').next().unwrap_or(&plugin.name);
            routes.push(plugin.path.join("src").join(format!("{class}Plugin.php")));
            routes.push(plugin.path.join("src").join("Plugin.php"));
        }
        for file in routes {
            let Some(text) = text_of(&file).filter(|text| text.contains("_name") || text.contains("->")) else {
                continue;
            };
            let parsed = ParsedFile::parse(&file.to_string_lossy(), text);
            let mut found = Routes { parsed: &parsed, file: &file, found: Vec::new() };
            RouteWalker.walk_program(parsed.program(), &mut found);
            out.routes.extend(found.found);
        }

        // Translations: `resources/locales/<language>/<domain>.po` (`src/Locale` in CakePHP 3),
        // the application's and each plugin's.
        out.legacy = root.join("src").join("Template").is_dir() && !root.join("templates").is_dir();
        let mut bases = vec![root.to_path_buf()];
        bases.extend(out.plugins.iter().filter(|plugin| plugin.local).map(|plugin| plugin.path.clone()));
        for base in &bases {
            let migrations = base.join("config").join("Migrations");
            if migrations.is_dir() {
                out.migration_paths.push(migrations);
            }
            for locales in [base.join("resources").join("locales"), base.join("src").join("Locale")] {
                for language in folders(&locales) {
                    let mut catalogues: Vec<PathBuf> = std::fs::read_dir(&language)
                        .into_iter()
                        .flatten()
                        .flatten()
                        .map(|entry| entry.path())
                        .filter(|path| path.extension().is_some_and(|ext| ext == "po"))
                        .collect();
                    catalogues.sort();
                    for file in catalogues.into_iter().take(64) {
                        let (Some(text), Some(domain)) = (text_of(&file), file.file_stem()) else { continue };
                        let domain = domain.to_string_lossy().into_owned();
                        for (context, message, translation, line, (start, end)) in
                            crate::frameworks::po::messages(&text)
                        {
                            out.messages.push(Message {
                                domain: domain.clone(),
                                context,
                                message,
                                language: named(&language),
                                translation,
                                file: file.clone(),
                                range: Range::new(crate::base::Pos::new(line, start), crate::base::Pos::new(line, end)),
                            });
                        }
                    }
                }
            }
        }
        out
    }

    pub fn plugin(&self, name: &str) -> Option<&Plugin> {
        self.plugins.iter().find(|plugin| plugin.name == name)
    }

    /// The plugin the file `path` is in.
    pub fn plugin_of(&self, path: &Path) -> Option<&Plugin> {
        let inside = self.plugins.iter().filter(|plugin| path.starts_with(&plugin.path));
        inside.max_by_key(|plugin| plugin.path.as_os_str().len())
    }

    /// The namespace of the classes of the plugin `plugin`, or of the
    /// application.
    pub fn namespace_of(&self, plugin: Option<&str>) -> String {
        match plugin {
            Some(plugin) => plugin_namespace(plugin),
            None => self.namespace.clone(),
        }
    }

    /// The `templates` folders of the plugin `plugin`, or of the
    /// application: for a plugin, the application's own versions first
    /// (`templates/plugin/Blog`).
    pub fn template_roots(&self, plugin: Option<&str>) -> Vec<PathBuf> {
        let own = self.templates_dir(&self.root);
        match plugin {
            Some(name) => {
                let mut roots = vec![own.join(if self.legacy { "Plugin" } else { "plugin" }).join(name)];
                roots.extend(self.plugin(name).map(|plugin| self.templates_dir(&plugin.path)));
                roots
            }
            None => vec![own],
        }
    }

    /// Where the templates the file `site` names are: the folder of a
    /// controller's, a cell's or a mailer's templates, or the folder of the
    /// template `site` is.
    pub fn template_context(&self, site: &Path) -> Option<TemplateContext> {
        let parts: Vec<String> =
            site.parent()?.components().map(|part| part.as_os_str().to_string_lossy().into_owned()).collect();
        let folders = |parts: &[String]| parts.join("/");
        let in_plugin = self.plugin_of(site).map(|plugin| plugin.name.clone());
        // A template: below `templates`, or `templates/plugin/<Name>`.
        let templates = if self.legacy {
            (1..parts.len()).rev().find(|at| parts[*at] == "Template" && parts[*at - 1] == "src")
        } else {
            parts.iter().rposition(|part| part == "templates")
        };
        if let Some(at) = templates {
            // (CakePHP 3 writes `Element`, `Layout`, `Plugin`.)
            let mut below = parts[at + 1..].to_vec();
            if let Some(first) =
                below.first_mut().filter(|first| RESERVED.contains(&first.to_ascii_lowercase().as_str()))
            {
                *first = first.to_ascii_lowercase();
            }
            let below = &below[..];
            if in_plugin.is_none()
                && below.first().is_some_and(|part| part == "plugin")
                && let Some(found) = self.plugins.iter().find(|plugin| {
                    let name: Vec<&str> = plugin.name.split('/').collect();
                    below.len() > name.len()
                        && below[1..=name.len()].iter().map(String::as_str).eq(name.iter().copied())
                })
            {
                let skip = 1 + found.name.split('/').count();
                return Some(TemplateContext {
                    roots: self.template_roots(Some(&found.name)),
                    folder: folders(&below[skip..]),
                    plugin: Some(found.name.clone()),
                });
            }
            return Some(TemplateContext {
                roots: self.template_roots(in_plugin.as_deref()),
                folder: folders(below),
                plugin: in_plugin,
            });
        }
        // A class: `src/Controller/Admin/ArticlesController.php` asks for `Admin/Articles`.
        let at = parts.iter().rposition(|part| part == "src")?;
        let below = &parts[at + 1..];
        let stem = site.file_stem()?.to_str()?;
        let folder = match below.first().map(String::as_str) {
            Some("Controller") if below.get(1).is_none_or(|part| part != "Component") => {
                let name = stem.strip_suffix("Controller")?;
                let mut folder = below[1..].to_vec();
                folder.push(name.to_owned());
                folders(&folder)
            }
            Some("View") if below.get(1).is_some_and(|part| part == "Cell") => {
                format!("cell/{}", stem.strip_suffix("Cell")?)
            }
            Some("Mailer") => "email/html".to_owned(),
            _ => return None,
        };
        Some(TemplateContext { roots: self.template_roots(in_plugin.as_deref()), folder, plugin: in_plugin })
    }

    /// The files the name `name` of a template, a layout or an element may
    /// be, asked for from the file `site`, the likeliest first: as CakePHP's
    /// book says they are found. `/Common/view` is from the `templates`
    /// folder, `Blog.sidebar` is the plugin's, and a name without an
    /// extension is a `.php` file.
    pub fn template_files(&self, site: &Path, name: &str, kind: TemplateKind) -> Vec<PathBuf> {
        self.template_files_with(site, name, kind, &Overrides::default())
    }

    /// The `templates` folder of the application or the plugin at `base`.
    fn templates_dir(&self, base: &Path) -> PathBuf {
        if self.legacy { base.join("src").join("Template") } else { base.join("templates") }
    }

    /// The extension of a template's file.
    pub fn template_extension(&self) -> &'static str {
        if self.legacy { "ctp" } else { "php" }
    }

    /// The folder below `templates` as it is written: CakePHP 3 has
    /// `Element` and `Layout` for `element` and `layout`.
    pub fn written(&self, below: &str) -> String {
        if !self.legacy {
            return below.to_owned();
        }
        let (first, rest) = below.split_once('/').map_or((below, ""), |(first, rest)| (first, rest));
        if !RESERVED.contains(&first) {
            return below.to_owned();
        }
        let mut chars = first.chars();
        let first = chars.next().map_or(String::new(), |c| c.to_ascii_uppercase().to_string() + chars.as_str());
        if rest.is_empty() { first } else { format!("{first}/{rest}") }
    }

    /// [`template_files`](Self::template_files), with what the code asking
    /// says about where its templates are: a theme's come first, and a
    /// template path replaces the folder.
    pub fn template_files_with(
        &self,
        site: &Path,
        name: &str,
        kind: TemplateKind,
        overrides: &Overrides,
    ) -> Vec<PathBuf> {
        if name.is_empty() || name.contains(['$', '{']) {
            return Vec::new();
        }
        let context = self.template_context(site);
        let (plugin, name) = plugin_split(name);
        let own_plugin = context.as_ref().and_then(|context| context.plugin.clone());
        let mut roots = match plugin {
            Some(plugin) => self.template_roots(Some(plugin)),
            None => context.as_ref().map_or_else(|| self.template_roots(None), |context| context.roots.clone()),
        };
        // A plugin's layouts and elements fall back to the application's.
        if plugin.is_some() || own_plugin.is_some() {
            roots.extend(self.template_roots(None));
        }
        if let Some(theme) = overrides.theme.as_deref().filter(|_| plugin.is_none()) {
            let mut themed = self.template_roots(Some(theme));
            themed.extend(roots);
            roots = themed;
        }
        let folder = context.as_ref().map(|context| context.folder.clone()).unwrap_or_default();
        let folder = match &overrides.template_path {
            Some(path) if kind == TemplateKind::Template && !folder.starts_with("email/") => path.clone(),
            _ => folder,
        };
        let layouts = match &overrides.layout_path {
            Some(path) => format!("layout/{}", path.trim_matches('/')),
            None => "layout".to_owned(),
        };
        // A mailer's templates and layouts are an HTML and a text one.
        let mail = folder.starts_with("email/");
        let mut below: Vec<String> = Vec::new();
        match kind {
            TemplateKind::Element => below.push(format!("element/{name}")),
            TemplateKind::Layout if mail => {
                below.extend(["html", "text"].map(|format| format!("layout/email/{format}/{name}")));
            }
            TemplateKind::Layout => below.push(format!("{layouts}/{name}")),
            TemplateKind::Template => match name.strip_prefix('/') {
                Some(rest) => below.push(rest.to_owned()),
                None if mail => below.extend(["html", "text"].map(|format| format!("email/{format}/{name}"))),
                None if plugin.is_some() && name.contains('/') => below.push(name.to_owned()),
                None => below.push(if folder.is_empty() { name.to_owned() } else { format!("{folder}/{name}") }),
            },
        }
        let mut out = Vec::new();
        for root in &roots {
            for below in &below {
                let file = tidy(&root.join(self.written(below)));
                let named = file.file_name().is_some_and(|name| name.to_string_lossy().contains('.'));
                let file = if named { file } else { file.with_extension(self.template_extension()) };
                if !out.contains(&file) {
                    out.push(file);
                }
            }
        }
        out
    }

    /// The template of the action `action` (`viewAll`, as its method is
    /// named) of the controller, the cell or the mailer the file `site` is.
    pub fn action_template(&self, site: &Path, action: &str) -> Vec<PathBuf> {
        self.template_files(site, &super::inflect::underscore(action), TemplateKind::Template)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(files: &[(&str, &str)]) -> (tempfile::TempDir, CakeProject) {
        let tmp = tempfile::tempdir().unwrap();
        for (rel, text) in files {
            let path = tmp.path().join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
        let project = CakeProject::load(tmp.path(), |path| std::fs::read_to_string(path).ok());
        (tmp, project)
    }

    fn shown(root: &Path, path: &Path) -> String {
        path.strip_prefix(root).unwrap_or(path).to_string_lossy().replace('\\', "/")
    }

    #[test]
    fn the_files_name_the_namespace_plugins_configuration_and_routes() {
        let (tmp, project) = project(&[
            ("composer.json", r#"{"autoload": {"psr-4": {"Shop\\": "src/", "Blog\\": "plugins/Blog/src/"}}}"#),
            (
                "config/app.php",
                "<?php\nreturn [\n    'debug' => true,\n    'App' => ['encoding' => 'UTF-8'],\n    'Datasources' => ['default' => ['host' => 'localhost']],\n];\n",
            ),
            ("config/app_local.php", "<?php\nreturn ['Datasources' => ['default' => ['host' => 'db']]];\n"),
            ("config/bootstrap.php", "<?php\nConfigure::load('app', 'default', false);\nConfigure::load('shop');\n"),
            ("config/shop.php", "<?php\nreturn ['Shop' => ['currency' => 'EUR']];\n"),
            (
                "config/routes.php",
                "<?php\nreturn function ($routes) {\n    $routes->connect('/login', ['controller' => 'Users', 'action' => 'login'], ['_name' => 'login']);\n    $routes->get('/a/{id}', ['controller' => 'Articles', 'action' => 'view'], 'articles:view');\n};\n",
            ),
            ("plugins/Blog/src/BlogPlugin.php", "<?php\n"),
            ("plugins/Company/Tools/src/ToolsPlugin.php", "<?php\n"),
            (
                "vendor/cakephp-plugins.php",
                "<?php\n$baseDir = dirname(dirname(__FILE__));\nreturn [\n    'plugins' => [\n        'Bake' => $baseDir . '/vendor/cakephp/bake/',\n    ],\n];\n",
            ),
        ]);
        let root = tmp.path();
        assert_eq!(project.namespace, "Shop");
        let plugins: Vec<(&str, String, bool)> = project
            .plugins
            .iter()
            .map(|plugin| (plugin.name.as_str(), shown(root, &plugin.path), plugin.local))
            .collect();
        assert_eq!(
            plugins,
            [
                ("Blog", "plugins/Blog".to_owned(), true),
                ("Company/Tools", "plugins/Company/Tools".to_owned(), true),
                ("Bake", "vendor/cakephp/bake".to_owned(), false),
            ]
        );
        assert_eq!(project.plugin("Company/Tools").unwrap().namespace(), "Company\\Tools");
        let keys: Vec<&str> = project.config.iter().map(|def| def.name.as_str()).collect();
        for key in ["debug", "App.encoding", "Datasources.default.host", "Shop.currency"] {
            assert!(keys.contains(&key), "{key}: {keys:?}");
        }
        let host: Vec<String> = project
            .config
            .iter()
            .filter(|def| def.name == "Datasources.default.host")
            .map(|def| shown(root, &def.file))
            .collect();
        assert_eq!(host, ["config/app.php", "config/app_local.php"]);
        let routes: Vec<(&str, u32)> =
            project.routes.iter().map(|def| (def.name.as_str(), def.range.start.line)).collect();
        assert_eq!(routes, [("login", 2), ("articles:view", 3)]);
    }

    #[test]
    fn cakephp_3_keeps_its_templates_in_src() {
        let (tmp, project) = project(&[
            ("config/app.php", "<?php\nreturn ['App' => ['namespace' => 'App']];\n"),
            ("src/Template/Articles/index.ctp", "<p></p>\n"),
            ("src/Locale/fr/default.po", "msgid \"Hello\"\nmsgstr \"Bonjour\"\n"),
            ("config/Migrations/1_Create.php", "<?php\n"),
        ]);
        let root = tmp.path();
        assert!(project.legacy);
        let found = |site: &str, name: &str, kind: TemplateKind| -> Vec<String> {
            project.template_files(&root.join(site), name, kind).iter().map(|file| shown(root, file)).collect()
        };
        assert_eq!(
            found("src/Controller/ArticlesController.php", "index", TemplateKind::Template),
            ["src/Template/Articles/index.ctp"]
        );
        assert_eq!(
            found("src/Template/Articles/index.ctp", "sidebar", TemplateKind::Element),
            ["src/Template/Element/sidebar.ctp"]
        );
        assert_eq!(
            found("src/Template/Element/sidebar.ctp", "default", TemplateKind::Layout),
            ["src/Template/Layout/default.ctp"]
        );
        let context = project.template_context(&root.join("src/Template/Admin/Articles/edit.ctp")).unwrap();
        assert_eq!(context.controller(), Some(("Admin", "Articles")));
        let messages: Vec<(&str, &str, &str)> = project
            .messages
            .iter()
            .map(|message| (message.domain.as_str(), message.message.as_str(), message.language.as_str()))
            .collect();
        assert_eq!(messages, [("default", "Hello", "fr")]);
        assert_eq!(shown(root, &project.migration_paths[0]), "config/Migrations");
    }

    #[test]
    fn a_theme_and_a_template_path_move_templates() {
        let (tmp, project) = project(&[
            ("config/app.php", "<?php\nreturn [];\n"),
            ("plugins/Modern/templates/layout/default.php", "<html></html>\n"),
        ]);
        let root = tmp.path();
        let overrides = Overrides {
            theme: Some("Modern".to_owned()),
            template_path: Some("Common".to_owned()),
            layout_path: Some("admin".to_owned()),
        };
        let found = |name: &str, kind: TemplateKind| -> Vec<String> {
            let site = root.join("src/Controller/ArticlesController.php");
            project.template_files_with(&site, name, kind, &overrides).iter().map(|file| shown(root, file)).collect()
        };
        assert_eq!(
            found("view", TemplateKind::Template),
            [
                "templates/plugin/Modern/Common/view.php",
                "plugins/Modern/templates/Common/view.php",
                "templates/Common/view.php"
            ]
        );
        assert_eq!(
            found("default", TemplateKind::Layout),
            [
                "templates/plugin/Modern/layout/admin/default.php",
                "plugins/Modern/templates/layout/admin/default.php",
                "templates/layout/admin/default.php"
            ]
        );
    }

    #[test]
    fn a_template_is_found_from_who_asks_for_it() {
        let (tmp, project) = project(&[
            ("config/app.php", "<?php\nreturn ['App' => ['namespace' => 'App']];\n"),
            ("plugins/Blog/src/BlogPlugin.php", "<?php\n"),
        ]);
        let root = tmp.path();
        assert_eq!(project.namespace, "App");
        let found = |site: &str, name: &str, kind: TemplateKind| -> Vec<String> {
            project.template_files(&root.join(site), name, kind).iter().map(|file| shown(root, file)).collect()
        };
        let template = TemplateKind::Template;
        assert_eq!(found("src/Controller/ArticlesController.php", "edit", template), ["templates/Articles/edit.php"]);
        assert_eq!(
            found("src/Controller/Admin/ArticlesController.php", "edit", template),
            ["templates/Admin/Articles/edit.php"]
        );
        assert_eq!(
            found("src/Controller/ArticlesController.php", "/Common/view", template),
            ["templates/Common/view.php"]
        );
        assert_eq!(
            found("templates/Articles/index.php", "sidebar", TemplateKind::Element),
            ["templates/element/sidebar.php"]
        );
        assert_eq!(
            found("templates/Articles/index.php", "admin", TemplateKind::Layout),
            ["templates/layout/admin.php"]
        );
        assert_eq!(found("src/View/Cell/InboxCell.php", "expanded", template), ["templates/cell/Inbox/expanded.php"]);
        assert_eq!(
            found("src/Mailer/UserMailer.php", "welcome", template),
            ["templates/email/html/welcome.php", "templates/email/text/welcome.php"]
        );
        // A plugin's: the application's own version first, the application's elements last.
        assert_eq!(
            found("plugins/Blog/src/Controller/PostsController.php", "view", template),
            [
                "templates/plugin/Blog/Posts/view.php",
                "plugins/Blog/templates/Posts/view.php",
                "templates/Posts/view.php"
            ]
        );
        assert_eq!(
            found("templates/Articles/index.php", "Blog.tags", TemplateKind::Element),
            [
                "templates/plugin/Blog/element/tags.php",
                "plugins/Blog/templates/element/tags.php",
                "templates/element/tags.php"
            ]
        );
        let context = project.template_context(&root.join("templates/plugin/Blog/Posts/view.php")).unwrap();
        assert_eq!((context.folder.as_str(), context.plugin.as_deref()), ("Posts", Some("Blog")));
        assert_eq!(context.controller(), Some(("", "Posts")));
        let context = project.template_context(&root.join("templates/Admin/Articles/edit.php")).unwrap();
        assert_eq!(context.controller(), Some(("Admin", "Articles")));
        let context = project.template_context(&root.join("templates/element/sidebar.php")).unwrap();
        assert_eq!(context.controller(), None);
        let action = project.action_template(&root.join("src/Controller/ArticlesController.php"), "viewAll");
        assert_eq!(shown(root, &action[0]), "templates/Articles/view_all.php");
        assert_eq!(plugin_split("Blog.Articles"), (Some("Blog"), "Articles"));
        assert_eq!(plugin_split("Articles"), (None, "Articles"));
        assert_eq!(plugin_split("view.json"), (None, "view.json"));
    }
}
