//! What a Drupal project's YAML files say ([`DrupalProject`]): the files
//! Drupal's documentation on modules and themes describes, found by name
//! wherever they are in the project.
//!
//! | file | read for |
//! | --- | --- |
//! | `*.services.yml` | services and parameters ([`SymfonyConfig`]: the format is Symfony's) |
//! | `*.routing.yml` | route names and their paths |
//! | `*.permissions.yml` | permission names |
//! | `*.libraries.yml` | library names: `<extension>/<key>` |
//! | `*.info.yml` | the modules, themes and profiles |
//! | `config/install/*.yml`, `config/optional/*.yml` | configuration names: the file's name |
//! | `config/schema/*.yml` | the configurations it describes and their keys |
//! | `*.breakpoints.yml`, `migrations/*.yml` | more plugins: breakpoints, migrations (by their `id`) |
//! | `<name>.info` | Drupal 7's modules and themes |
//! | `*.links.{menu,task,action,contextual}.yml`, `*.layouts.yml` | the plugins they declare: menu links, local tasks and actions, contextual links, layouts |
//!
//! The folders `vendor`, `node_modules`, `tests` and those whose name starts
//! with a dot aren't looked into: a test module's routes are no route of the
//! site. The engine keeps the result until a YAML file changes.

use std::path::{Path, PathBuf};

use crate::base::{Range, yaml};
use crate::symfony::SymfonyConfig;

/// How deep below the project's folder files are looked for.
const MAX_DEPTH: usize = 16;

/// The most YAML files one load reads.
const MAX_FILES: usize = 20_000;

/// A name a YAML file declares.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Def {
    pub name: String,
    /// What a hover and completion say of it: a route's path, a
    /// permission's title.
    pub detail: Option<String>,
    pub file: PathBuf,
    /// The name's key; empty when the file is the declaration.
    pub range: Range,
}

/// The description of a configuration in a `config/schema` file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Schema {
    /// The configuration's name; with a `*` for those of several
    /// (`node.type.*`).
    pub name: Def,
    /// The keys its `mapping` describes, nested ones with dots.
    pub keys: Vec<Def>,
    /// The `type` the schema gives each of those keys (`string`,
    /// `integer`, `mapping`…), in their order.
    pub types: Vec<String>,
}

/// A module, theme or profile: its `<name>.info.yml`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Extension {
    pub name: String,
    /// `module`, `theme`, `profile`, `theme_engine`.
    pub kind: String,
    /// The folder of its files.
    pub dir: PathBuf,
    pub file: PathBuf,
}

/// See the module documentation.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DrupalProject {
    /// The services and parameters of every `*.services.yml`.
    pub services: SymfonyConfig,
    pub routes: Vec<Def>,
    pub permissions: Vec<Def>,
    pub libraries: Vec<Def>,
    pub configs: Vec<Def>,
    /// The plugins YAML files declare: menu links, local tasks and actions,
    /// contextual links and layouts. The detail is the plugin's type.
    pub plugins: Vec<Def>,
    /// What the files of `config/schema` describe.
    pub schemas: Vec<Schema>,
    pub extensions: Vec<Extension>,
    /// The files read, by what they are, for the usages a YAML file writes.
    pub files: Vec<(FileKind, PathBuf)>,
}

/// What a YAML file of a Drupal project is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileKind {
    Services,
    Routing,
    Permissions,
    Libraries,
    Info,
    Links,
    Layouts,
    Breakpoints,
    /// A file of a `migrations` or `migration_templates` folder.
    Migration,
    Config,
    /// A file of `config/schema`.
    Schema,
    /// Drupal 7's `<name>.info`.
    InfoLegacy,
}

impl FileKind {
    /// What the file at `path` is, by its name and its folder.
    pub fn of(path: &Path) -> Option<Self> {
        let name = path.file_name()?.to_str()?;
        if name.len() > ".info".len() && name.ends_with(".info") {
            return Some(Self::InfoLegacy);
        }
        let stem = name.strip_suffix(".yml")?;
        let ends = |suffix: &str| stem.len() > suffix.len() && stem.ends_with(suffix);
        if ends(".services") {
            Some(Self::Services)
        } else if ends(".routing") {
            Some(Self::Routing)
        } else if ends(".permissions") {
            Some(Self::Permissions)
        } else if ends(".libraries") {
            Some(Self::Libraries)
        } else if ends(".info") {
            Some(Self::Info)
        } else if LINKS.iter().any(|(suffix, _)| ends(suffix)) {
            Some(Self::Links)
        } else if ends(".layouts") {
            Some(Self::Layouts)
        } else if ends(".breakpoints") {
            Some(Self::Breakpoints)
        } else {
            let folder = path.parent()?;
            let within = folder.file_name()?.to_str()?;
            if matches!(within, "migrations" | "migration_templates") {
                return Some(Self::Migration);
            }
            let config = folder.parent()?.file_name()? == "config";
            match within {
                "install" | "optional" if config => Some(Self::Config),
                "schema" if config => Some(Self::Schema),
                _ => None,
            }
        }
    }
}

/// The links files, and the type of the plugins each declares.
const LINKS: [(&str, &str); 4] = [
    (".links.menu", "MenuLink"),
    (".links.task", "LocalTask"),
    (".links.action", "LocalAction"),
    (".links.contextual", "ContextualLink"),
];

fn walk(folder: &Path, depth: usize, out: &mut Vec<(FileKind, PathBuf)>) {
    let Ok(entries) = std::fs::read_dir(folder) else { return };
    let mut entries: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).collect();
    entries.sort();
    for path in entries {
        if out.len() >= MAX_FILES {
            return;
        }
        let name = path.file_name().and_then(|name| name.to_str()).unwrap_or_default();
        if name.ends_with(".yml") || name.ends_with(".info") {
            if let Some(kind) = FileKind::of(&path).filter(|_| path.is_file()) {
                out.push((kind, path));
            }
        } else if !name.contains('.') || path.is_dir() {
            let skipped = name.starts_with('.') || matches!(name, "vendor" | "node_modules" | "tests");
            if depth < MAX_DEPTH && !skipped && path.is_dir() {
                walk(&path, depth + 1, out);
            }
        }
    }
}

/// The `templates` folders of the modules, themes and profiles below
/// `root`, each with its extension's name: Twig's `@shop/…` namespace.
pub fn template_dirs(root: &Path) -> Vec<(String, PathBuf)> {
    let mut files = Vec::new();
    walk(root, 0, &mut files);
    let info = files.into_iter().filter(|(kind, _)| *kind == FileKind::Info);
    info.filter_map(|(_, file)| {
        let templates = file.parent()?.join("templates");
        templates.is_dir().then(|| (stem(&file, ".info.yml").to_owned(), templates))
    })
    .collect()
}

/// The part of a file's name before `suffix` (`node` of `node.routing.yml`).
fn stem<'p>(path: &'p Path, suffix: &str) -> &'p str {
    let name = path.file_name().and_then(|name| name.to_str()).unwrap_or_default();
    name.strip_suffix(suffix).unwrap_or(name)
}

impl DrupalProject {
    /// Reads the YAML files of the project at `root`.
    pub fn load(root: &Path) -> Self {
        let mut files = Vec::new();
        walk(root, 0, &mut files);
        let mut out = Self::default();
        for (kind, file) in &files {
            // A configuration's name is its file's: nothing to read.
            if *kind == FileKind::Config {
                out.configs.push(Def {
                    name: stem(file, ".yml").to_owned(),
                    detail: None,
                    file: file.clone(),
                    range: Range::default(),
                });
                continue;
            }
            let Ok(text) = std::fs::read_to_string(file) else { continue };
            out.add(*kind, file, &text);
        }
        out.files = files;
        out
    }

    /// What the file `file` of kind `kind` with `text` declares.
    pub fn add(&mut self, kind: FileKind, file: &Path, text: &str) {
        match kind {
            FileKind::Services => {
                self.services.add_services(file, text);
            }
            FileKind::Routing => {
                let before = self.services.routes.len();
                self.services.add_routes(file, text);
                for route in self.services.routes.drain(before..) {
                    self.routes.push(Def {
                        name: route.name,
                        detail: route.path,
                        file: file.to_path_buf(),
                        range: route.range,
                    });
                }
            }
            FileKind::Permissions => {
                for entry in yaml::parse(text).entries() {
                    if entry.key == "permission_callbacks" {
                        continue;
                    }
                    let title = entry.value.get("title").and_then(yaml::Node::as_str).map(str::to_owned);
                    self.permissions.push(Def {
                        name: entry.key.clone(),
                        detail: title,
                        file: file.to_path_buf(),
                        range: entry.range,
                    });
                }
            }
            FileKind::Libraries => {
                let extension = stem(file, ".libraries.yml");
                for entry in yaml::parse(text).entries() {
                    self.libraries.push(Def {
                        name: format!("{extension}/{}", entry.key),
                        detail: None,
                        file: file.to_path_buf(),
                        range: entry.range,
                    });
                }
            }
            FileKind::Info => {
                let doc = yaml::parse(text);
                let kind = doc.get("type").and_then(yaml::Node::as_str).unwrap_or("module").to_owned();
                self.extensions.push(Extension {
                    name: stem(file, ".info.yml").to_owned(),
                    kind,
                    dir: file.parent().unwrap_or(Path::new("")).to_path_buf(),
                    file: file.to_path_buf(),
                });
            }
            FileKind::InfoLegacy => {
                let theme =
                    text.lines().any(|line| line.trim_start().starts_with("engine") || line.starts_with("base theme"));
                self.extensions.push(Extension {
                    name: stem(file, ".info").to_owned(),
                    kind: if theme { "theme" } else { "module" }.to_owned(),
                    dir: file.parent().unwrap_or(Path::new("")).to_path_buf(),
                    file: file.to_path_buf(),
                });
            }
            // A migration is named by its `id`.
            FileKind::Migration => {
                let doc = yaml::parse(text);
                let id = doc.entries().iter().find(|entry| entry.key == "id");
                if let Some((entry, id)) = id.and_then(|entry| Some((entry, entry.value.as_str()?))) {
                    self.plugins.push(Def {
                        name: id.to_owned(),
                        detail: Some("Migration".to_owned()),
                        file: file.to_path_buf(),
                        range: entry.range,
                    });
                }
            }
            FileKind::Schema => {
                fn keys(node: &yaml::Node, prefix: &str, file: &Path, out: &mut Vec<Def>, types: &mut Vec<String>) {
                    for entry in node.get("mapping").map(yaml::Node::entries).unwrap_or_default() {
                        let path =
                            if prefix.is_empty() { entry.key.clone() } else { format!("{prefix}.{}", entry.key) };
                        let label = entry.value.get("label").and_then(yaml::Node::as_str).map(str::to_owned);
                        out.push(Def {
                            name: path.clone(),
                            detail: label,
                            file: file.to_path_buf(),
                            range: entry.range,
                        });
                        types.push(entry.value.get("type").and_then(yaml::Node::as_str).unwrap_or_default().to_owned());
                        keys(&entry.value, &path, file, out, types);
                    }
                }
                for entry in yaml::parse(text).entries() {
                    let label = entry.value.get("label").and_then(yaml::Node::as_str).map(str::to_owned);
                    let name =
                        Def { name: entry.key.clone(), detail: label, file: file.to_path_buf(), range: entry.range };
                    let mut described = Vec::new();
                    let mut types = Vec::new();
                    keys(&entry.value, "", file, &mut described, &mut types);
                    self.schemas.push(Schema { name, keys: described, types });
                }
            }
            FileKind::Links | FileKind::Layouts | FileKind::Breakpoints => {
                let name = file.file_name().and_then(|name| name.to_str()).unwrap_or_default();
                let found = LINKS.iter().find(|(suffix, _)| name.ends_with(&format!("{suffix}.yml")));
                let other = if kind == FileKind::Breakpoints { "Breakpoint" } else { "Layout" };
                let kind = found.map_or(other, |(_, kind)| kind);
                for entry in yaml::parse(text).entries() {
                    self.plugins.push(Def {
                        name: entry.key.clone(),
                        detail: Some(kind.to_owned()),
                        file: file.to_path_buf(),
                        range: entry.range,
                    });
                }
            }
            FileKind::Config => {}
        }
    }

    /// The files of kind `kind`.
    pub fn files_of(&self, kind: FileKind) -> impl Iterator<Item = &PathBuf> {
        self.files.iter().filter(move |(known, _)| *known == kind).map(|(_, file)| file)
    }

    /// The extension the file at `path` is in: the nearest one above it.
    pub fn extension_of(&self, path: &Path) -> Option<&Extension> {
        self.extensions
            .iter()
            .filter(|extension| path.starts_with(&extension.dir))
            .max_by_key(|extension| extension.dir.components().count())
    }
}

impl DrupalProject {
    /// The type of each key of the configurations a file installs, as a
    /// schema would name it, from the value the file gives the key: the
    /// configuration, the key (nested ones with dots), and `string`,
    /// `integer`, `float`, `boolean`, `mapping` or `sequence`.
    pub fn installed_types(&self) -> Vec<(String, String, String)> {
        fn types(node: &yaml::Node, prefix: &str, config: &str, out: &mut Vec<(String, String, String)>) {
            for entry in node.entries() {
                let key = if prefix.is_empty() { entry.key.clone() } else { format!("{prefix}.{}", entry.key) };
                let ty = match &entry.value {
                    yaml::Node::Null => continue,
                    yaml::Node::Map(_) => "mapping",
                    yaml::Node::Seq(_) => "sequence",
                    yaml::Node::Scalar(text) => match text.as_str() {
                        "true" | "false" => "boolean",
                        text if text.parse::<i64>().is_ok() => "integer",
                        text if text.parse::<f64>().is_ok() => "float",
                        _ => "string",
                    },
                };
                out.push((config.to_owned(), key.clone(), ty.to_owned()));
                types(&entry.value, &key, config, out);
            }
        }
        let mut out = Vec::new();
        for config in &self.configs {
            let Ok(text) = std::fs::read_to_string(&config.file) else { continue };
            types(&yaml::parse(&text), "", &config.name, &mut out);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_are_told_by_their_names() {
        let kind = |path: &str| FileKind::of(Path::new(path));
        assert_eq!(kind("modules/shop/shop.services.yml"), Some(FileKind::Services));
        assert_eq!(kind("modules/shop/shop.routing.yml"), Some(FileKind::Routing));
        assert_eq!(kind("modules/shop/shop.links.menu.yml"), Some(FileKind::Links));
        assert_eq!(kind("modules/shop/shop.info.yml"), Some(FileKind::Info));
        assert_eq!(kind("modules/shop/config/install/shop.settings.yml"), Some(FileKind::Config));
        assert_eq!(kind("modules/shop/config/schema/shop.schema.yml"), Some(FileKind::Schema));
        assert_eq!(kind("modules/shop/.routing.yml"), None);
        assert_eq!(kind("docker-compose.yml"), None);
    }

    #[test]
    fn a_modules_files_are_read() {
        let mut project = DrupalProject::default();
        let file = Path::new("modules/shop/shop.routing.yml");
        project.add(
            FileKind::Routing,
            file,
            "shop.books:\n  path: '/books'\n  defaults:\n    _controller: 'x'\nroute_callbacks:\n  - 'y'\n",
        );
        assert_eq!(project.routes.len(), 1);
        assert_eq!(
            (project.routes[0].name.as_str(), project.routes[0].detail.as_deref()),
            ("shop.books", Some("/books"))
        );
        let file = Path::new("modules/shop/shop.permissions.yml");
        project.add(
            FileKind::Permissions,
            file,
            "administer shop:\n  title: 'Administer the shop'\npermission_callbacks:\n  - x\n",
        );
        assert_eq!(project.permissions.len(), 1);
        assert_eq!(project.permissions[0].detail.as_deref(), Some("Administer the shop"));
        let file = Path::new("modules/shop/shop.libraries.yml");
        project.add(FileKind::Libraries, file, "cart:\n  js:\n    js/cart.js: {}\n");
        assert_eq!(project.libraries[0].name, "shop/cart");
        let file = Path::new("modules/shop/shop.services.yml");
        project.add(FileKind::Services, file, "services:\n  shop.catalog:\n    class: Drupal\\shop\\Catalog\n");
        assert_eq!(project.services.service_class("shop.catalog").as_deref(), Some("Drupal\\shop\\Catalog"));
        let file = Path::new("themes/shelf/shelf.info.yml");
        project.add(FileKind::Info, file, "name: Shelf\ntype: theme\n");
        assert_eq!(project.extensions[0].kind, "theme");
        assert_eq!(
            project.extension_of(Path::new("themes/shelf/templates/page.html.twig")).map(|e| e.name.as_str()),
            Some("shelf")
        );
    }

    #[test]
    fn more_files_declare_plugins_and_modules() {
        let kind = |path: &str| FileKind::of(Path::new(path));
        assert_eq!(kind("modules/shop/shop.breakpoints.yml"), Some(FileKind::Breakpoints));
        assert_eq!(kind("modules/shop/migrations/shop_books.yml"), Some(FileKind::Migration));
        assert_eq!(kind("sites/all/modules/shop/shop.info"), Some(FileKind::InfoLegacy));
        let mut project = DrupalProject::default();
        let file = Path::new("modules/shop/migrations/shop_books.yml");
        project.add(
            FileKind::Migration,
            file,
            "id: shop_books
label: Books
source:
  plugin: csv
",
        );
        let file = Path::new("modules/shop/shop.breakpoints.yml");
        project.add(
            FileKind::Breakpoints,
            file,
            "shop.mobile:
  label: mobile
",
        );
        let file = Path::new("modules/shop/shop.links.task.yml");
        project.add(
            FileKind::Links,
            file,
            "shop.tab:
  route_name: shop.books
",
        );
        let plugins: Vec<(&str, Option<&str>)> =
            project.plugins.iter().map(|plugin| (plugin.name.as_str(), plugin.detail.as_deref())).collect();
        assert_eq!(
            plugins,
            [("shop_books", Some("Migration")), ("shop.mobile", Some("Breakpoint")), ("shop.tab", Some("LocalTask"))]
        );
        let file = Path::new("sites/all/modules/shop/shop.info");
        project.add(
            FileKind::InfoLegacy,
            file,
            "name = Shop
core = 7.x
",
        );
        assert_eq!((project.extensions[0].name.as_str(), project.extensions[0].kind.as_str()), ("shop", "module"));
    }
}
