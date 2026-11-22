//! Where a project's Twig templates live, and template names ⇄ files.
//!
//! A name is a path below a template directory (`base.html.twig`,
//! `admin/list.html.twig`), or `@Namespace/path` for a namespaced directory.
//! The directories are Symfony's `templates/` (or the `default_path` of
//! `config/packages/twig.yaml`), the `paths` of that file (with or without
//! a namespace), the legacy `app/Resources/views`, and `resources/views`
//! (TwigBridge for Laravel). In a Drupal project, the `templates` folder of
//! each module and theme is the namespace of its name (`@shop`).

use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, OnceLock};

use crate::base::yaml;

/// The most template names [`TemplatePaths::names`] lists.
pub const MAX_NAMES: usize = 2000;

/// The template directories of one project.
///
/// The template names are listed once ([`names`](Self::names)) and kept:
/// whoever keeps a `TemplatePaths` discovers it again when a template is
/// created or removed.
#[derive(Clone, Debug, Default)]
pub struct TemplatePaths {
    /// `(namespace, directory)`; unnamespaced ones first, in lookup order.
    dirs: Vec<(Option<String>, PathBuf)>,
    names: Arc<OnceLock<Vec<String>>>,
}

impl PartialEq for TemplatePaths {
    fn eq(&self, other: &Self) -> bool {
        self.dirs == other.dirs
    }
}

impl Eq for TemplatePaths {}

/// What the Twig support knows about one project: where its templates are,
/// and whether it has Symfony's Twig bridge (whose tags, filters and
/// functions completion and hover then know).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TwigProject {
    pub paths: TemplatePaths,
    pub symfony: bool,
}

/// The Composer packages that bring Symfony's Twig extensions.
const SYMFONY_PACKAGES: [&str; 3] = ["symfony/twig-bridge", "symfony/twig-bundle", "symfony/framework-bundle"];

impl TwigProject {
    /// Reads the project at `root`: its template directories and its
    /// Composer files.
    pub fn discover(root: &Path) -> Self {
        let composer = crate::frameworks::ComposerPackages::read(root);
        Self {
            paths: TemplatePaths::discover(root),
            symfony: SYMFONY_PACKAGES.iter().any(|package| composer.installed(package)),
        }
    }
}

impl TemplatePaths {
    /// The directories of the project at `root` (only the ones that exist).
    pub fn discover(root: &Path) -> Self {
        let mut dirs: Vec<(Option<String>, PathBuf)> = Vec::new();
        let mut default = root.join("templates");
        let config = ["config/packages/twig.yaml", "config/packages/twig.yml"]
            .iter()
            .find_map(|file| std::fs::read_to_string(root.join(file)).ok());
        let mut configured = Vec::new();
        if let Some(text) = config {
            let doc = yaml::parse(&text);
            if let Some(twig) = doc.get("twig") {
                if let Some(path) = twig.get("default_path").and_then(yaml::Node::as_str) {
                    default = expand(root, path);
                }
                for entry in twig.get("paths").map(yaml::Node::entries).unwrap_or_default() {
                    let namespace = entry.value.as_str().map(|ns| ns.trim_start_matches('@').to_string());
                    configured.push((namespace.filter(|ns| !ns.is_empty()), expand(root, &entry.key)));
                }
            }
        }
        dirs.push((None, default));
        dirs.extend(configured);
        dirs.push((None, root.join("app/Resources/views")));
        dirs.push((None, root.join("resources/views")));
        // Drupal: `@shop/shop-book.html.twig` is in the `templates` folder of the module `shop`.
        let settings = crate::base::settings::FrameworkSettings::default();
        if crate::frameworks::Frameworks::detect(root, &settings).contains(crate::frameworks::Framework::Drupal) {
            let extensions = crate::frameworks::drupal::project::template_dirs(root);
            dirs.extend(extensions.into_iter().map(|(name, dir)| (Some(name), dir)));
        }
        // Mezzio: `app::home-page` is `@app/home-page.html.twig`, in the namespace's folder.
        if crate::frameworks::Frameworks::detect(root, &settings).contains(crate::frameworks::Framework::Laminas) {
            dirs.extend(crate::frameworks::laminas::project::template_dirs(root));
        }
        let mut seen = Vec::new();
        dirs.retain(|entry| {
            entry.1.is_dir() && !seen.contains(entry) && {
                seen.push(entry.clone());
                true
            }
        });
        dirs.sort_by_key(|(ns, _)| ns.is_some());
        Self::from_dirs(dirs)
    }

    /// From explicit directories (tests).
    pub fn from_dirs(dirs: Vec<(Option<String>, PathBuf)>) -> Self {
        Self { dirs, names: Arc::default() }
    }

    /// The name of the template file `path` (`admin/list.html.twig`,
    /// `@Admin/list.html.twig`), when it is below a template directory.
    pub fn name_of(&self, path: &Path) -> Option<String> {
        self.dirs.iter().find_map(|(namespace, dir)| {
            let relative = path.strip_prefix(dir).ok()?;
            let relative: Vec<_> = relative.components().map(|c| c.as_os_str().to_string_lossy()).collect();
            let prefix = namespace.as_ref().map(|ns| format!("@{ns}/")).unwrap_or_default();
            (!relative.is_empty()).then(|| format!("{prefix}{}", relative.join("/")))
        })
    }

    /// The file of template `name`, if it exists.
    pub fn resolve(&self, name: &str) -> Option<PathBuf> {
        let (namespace, path) = split_name(name);
        let relative = Path::new(path);
        if path.is_empty() || relative.components().any(|c| !matches!(c, Component::Normal(_))) {
            return None;
        }
        self.dirs
            .iter()
            .filter(|(ns, _)| ns.as_deref() == namespace)
            .map(|(_, dir)| dir.join(relative))
            .find(|file| file.is_file())
    }

    /// The names of the project's templates (files ending in `.twig`), sorted;
    /// at most [`MAX_NAMES`]. Listed from disk the first time.
    pub fn names(&self) -> Vec<String> {
        self.names.get_or_init(|| self.list_names()).clone()
    }

    fn list_names(&self) -> Vec<String> {
        let mut names = Vec::new();
        for (namespace, dir) in &self.dirs {
            let prefix = namespace.as_ref().map(|ns| format!("@{ns}/")).unwrap_or_default();
            collect(dir, dir, &prefix, &mut names);
            if names.len() >= MAX_NAMES {
                break;
            }
        }
        names.sort();
        names.dedup();
        names.truncate(MAX_NAMES);
        names
    }
}

/// `@Ns/path` → `(Some("Ns"), "path")`.
fn split_name(name: &str) -> (Option<&str>, &str) {
    match name.strip_prefix('@').and_then(|rest| rest.split_once('/')) {
        Some((ns, path)) => (Some(ns), path),
        None => (None, name),
    }
}

/// A path of `twig.yaml`: `%kernel.project_dir%` is the project folder,
/// relative paths are relative to it.
fn expand(root: &Path, path: &str) -> PathBuf {
    let path = path.replace("%kernel.project_dir%", &root.to_string_lossy());
    let path = PathBuf::from(path);
    if path.is_absolute() { path } else { root.join(path) }
}

fn collect(base: &Path, dir: &Path, prefix: &str, out: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        if out.len() >= MAX_NAMES {
            return;
        }
        let path = entry.path();
        let Ok(kind) = entry.file_type() else { continue };
        if kind.is_dir() {
            collect(base, &path, prefix, out);
        } else if path.extension().is_some_and(|ext| ext == "twig")
            && let Ok(relative) = path.strip_prefix(base)
        {
            let relative: Vec<_> = relative.components().map(|c| c.as_os_str().to_string_lossy()).collect();
            out.push(format!("{prefix}{}", relative.join("/")));
        }
    }
}
