//! Which frameworks a project uses ([`Frameworks::detect`]).
//!
//! A framework is detected from what Composer says is installed
//! ([`ComposerPackages`]: `composer.json`, `composer.lock`,
//! `vendor/composer/installed.json`) and from the files that framework's
//! own documentation names as part of every project of its kind (`artisan`,
//! `wp-config.php`, `spark`…). The `frameworks` settings
//! ([`FrameworkSettings`]) force one on or off.
//!
//! Detection reads a handful of small files and the Composer ones; the
//! engine keeps the answer until one of them changes
//! ([`crate::engine::PhpEngine::frameworks`]).

use std::path::Path;

use serde_json::Value;

use crate::base::settings::{Detection, FrameworkSettings};
use crate::project::composer::RootManifest;

/// A framework the engine knows how to detect.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Framework {
    Laravel,
    Symfony,
    WordPress,
    Drupal,
    CodeIgniter,
    Yii,
    CakePhp,
    Laminas,
    Magento,
}

impl Framework {
    pub const ALL: [Framework; 9] = [
        Framework::Laravel,
        Framework::Symfony,
        Framework::WordPress,
        Framework::Drupal,
        Framework::CodeIgniter,
        Framework::Yii,
        Framework::CakePhp,
        Framework::Laminas,
        Framework::Magento,
    ];

    /// Its key in the `frameworks` settings.
    pub fn key(self) -> &'static str {
        match self {
            Framework::Laravel => "laravel",
            Framework::Symfony => "symfony",
            Framework::WordPress => "wordpress",
            Framework::Drupal => "drupal",
            Framework::CodeIgniter => "codeigniter",
            Framework::Yii => "yii",
            Framework::CakePhp => "cakephp",
            Framework::Laminas => "laminas",
            Framework::Magento => "magento",
        }
    }

    /// Its name as users write it.
    pub fn title(self) -> &'static str {
        match self {
            Framework::Laravel => "Laravel",
            Framework::Symfony => "Symfony",
            Framework::WordPress => "WordPress",
            Framework::Drupal => "Drupal",
            Framework::CodeIgniter => "CodeIgniter",
            Framework::Yii => "Yii",
            Framework::CakePhp => "CakePHP",
            Framework::Laminas => "Laminas",
            Framework::Magento => "Magento",
        }
    }

    /// The Composer packages one of which every project of the framework
    /// installs.
    fn packages(self) -> &'static [&'static str] {
        match self {
            Framework::Laravel => &["laravel/framework"],
            Framework::Symfony => &["symfony/framework-bundle"],
            Framework::WordPress => &[
                "johnpbloch/wordpress",
                "johnpbloch/wordpress-core",
                "roots/wordpress",
                "roots/wordpress-no-content",
                "php-stubs/wordpress-stubs",
            ],
            Framework::Drupal => &["drupal/core", "drupal/core-recommended"],
            Framework::CodeIgniter => &["codeigniter4/framework"],
            Framework::Yii => &["yiisoft/yii2"],
            Framework::CakePhp => &["cakephp/cakephp"],
            Framework::Laminas => &["laminas/laminas-mvc", "mezzio/mezzio"],
            Framework::Magento => {
                &["magento/framework", "magento/product-community-edition", "magento/product-enterprise-edition"]
            }
        }
    }

    /// The files, relative to the project, one of which says the project
    /// is of the framework.
    fn marker_files(self) -> &'static [&'static str] {
        match self {
            Framework::Laravel => &["artisan"],
            Framework::Symfony => &[],
            Framework::WordPress => &["wp-config.php", "wp-load.php", "wp-includes/version.php"],
            // The last is Drupal 7's.
            Framework::Drupal => &[
                "core/lib/Drupal.php",
                "web/core/lib/Drupal.php",
                "docroot/core/lib/Drupal.php",
                "modules/system/system.module",
            ],
            Framework::CodeIgniter => &["spark"],
            Framework::Yii | Framework::CakePhp | Framework::Laminas => &[],
            Framework::Magento => &["app/etc/di.xml"],
        }
    }

    fn setting(self, settings: &FrameworkSettings) -> Detection {
        match self {
            Framework::Laravel => settings.laravel,
            Framework::Symfony => settings.symfony,
            Framework::WordPress => settings.wordpress,
            Framework::Drupal => settings.drupal,
            Framework::CodeIgniter => settings.codeigniter,
            Framework::Yii => settings.yii,
            Framework::CakePhp => settings.cakephp,
            Framework::Laminas => settings.laminas,
            Framework::Magento => settings.magento,
        }
    }
}

/// A set of [`Framework`]s.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Frameworks(u16);

impl Frameworks {
    pub fn contains(self, framework: Framework) -> bool {
        self.0 & (1 << framework as u16) != 0
    }

    pub fn insert(&mut self, framework: Framework) {
        self.0 |= 1 << framework as u16;
    }

    pub fn remove(&mut self, framework: Framework) {
        self.0 &= !(1 << framework as u16);
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// The frameworks of the set, in the order of [`Framework::ALL`].
    pub fn iter(self) -> impl Iterator<Item = Framework> {
        Framework::ALL.into_iter().filter(move |framework| self.contains(*framework))
    }

    /// The frameworks of the project at `root`: for each one, what
    /// `settings` say, and when they say `auto`, whether Composer installs
    /// one of its packages or one of its marker files exists.
    pub fn detect(root: &Path, settings: &FrameworkSettings) -> Self {
        // Without a framework left on `auto`, nothing is read.
        let composer = Framework::ALL
            .iter()
            .any(|framework| framework.setting(settings) == Detection::Auto)
            .then(|| ComposerPackages::read(root));
        let mut found = Self::default();
        for framework in Framework::ALL {
            let on = match framework.setting(settings) {
                Detection::Enabled => true,
                Detection::Disabled => false,
                Detection::Auto => composer.as_ref().is_some_and(|composer| looks_like(framework, root, composer)),
            };
            if on {
                found.insert(framework);
            }
        }
        found
    }
}

fn looks_like(framework: Framework, root: &Path, composer: &ComposerPackages) -> bool {
    framework.packages().iter().any(|name| composer.installed(name))
        || framework.marker_files().iter().any(|file| root.join(file).is_file())
        || match framework {
            Framework::WordPress => wordpress_extension(root),
            Framework::Drupal => drupal_extension(root),
            Framework::Magento => root.join("registration.php").is_file() && root.join("etc/module.xml").is_file(),
            _ => false,
        }
}

/// How much of a file's start is read for a header.
const HEADER_BYTES: usize = 8192;

/// How many files of the root folder are looked at for a header.
const MAX_ROOT_FILES: usize = 64;

/// Whether the folder is a WordPress plugin or theme on its own: a PHP file
/// at its root has a `Plugin Name:` header, or its `style.css` a
/// `Theme Name:` one (the file headers WordPress's plugin and theme
/// handbooks describe).
fn wordpress_extension(root: &Path) -> bool {
    if header_has(&root.join("style.css"), "Theme Name:") {
        return true;
    }
    let Ok(entries) = std::fs::read_dir(root) else { return false };
    entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("php")))
        .take(MAX_ROOT_FILES)
        .any(|path| header_has(&path, "Plugin Name:"))
}

/// Whether the folder is a Drupal module, theme or profile on its own: it
/// has a `<name>.info.yml` with a `core_version_requirement` or `core` key,
/// or Drupal 7's `<name>.info` with a `core = 7.x` line.
fn drupal_extension(root: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(root) else { return false };
    let info = |path: &std::path::PathBuf, suffix: &str| {
        path.file_name().and_then(|name| name.to_str()).is_some_and(|name| name.ends_with(suffix))
    };
    entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| info(path, ".info.yml") || info(path, ".info"))
        .take(MAX_ROOT_FILES)
        .any(|path| {
            let legacy = info(&path, ".info");
            std::fs::read_to_string(path).is_ok_and(|text| {
                text.lines().any(|line| {
                    if legacy {
                        line.split_once('=')
                            .is_some_and(|(key, value)| key.trim() == "core" && value.trim().starts_with('7'))
                    } else {
                        line.starts_with("core_version_requirement:") || line.starts_with("core:")
                    }
                })
            })
        })
}

fn header_has(path: &Path, needle: &str) -> bool {
    use std::io::Read;
    let Ok(file) = std::fs::File::open(path) else { return false };
    let mut head = Vec::with_capacity(HEADER_BYTES);
    if file.take(HEADER_BYTES as u64).read_to_end(&mut head).is_err() {
        return false;
    }
    String::from_utf8_lossy(&head).contains(needle)
}

/// What Composer says about the project's packages: the root
/// `composer.json`, `composer.lock` and `vendor/composer/installed.json`.
#[derive(Clone, Debug, Default)]
pub struct ComposerPackages {
    pub(crate) composer: Option<Value>,
    pub(crate) lock: Option<Value>,
    pub(crate) installed: Option<Value>,
}

impl ComposerPackages {
    /// Reads the three files of the project at `root` (its vendor folder is
    /// the one `composer.json` names).
    pub fn read(root: &Path) -> Self {
        let manifest = RootManifest::read(root);
        let vendor = root.join(manifest.as_ref().map_or("vendor", |m| m.vendor_dir.as_str()));
        Self::read_with(root, &vendor)
    }

    /// [`read`](Self::read) with the vendor folder already known.
    pub(crate) fn read_with(root: &Path, vendor: &Path) -> Self {
        // `composer.lock` and `installed.json` are large (700 KB each in
        // Monica): parsed at the same time.
        let (composer, (lock, installed)) = rayon::join(
            || read_json(&root.join("composer.json")),
            || {
                rayon::join(
                    || read_json(&root.join("composer.lock")),
                    || read_json(&vendor.join("composer").join("installed.json")),
                )
            },
        );
        Self { composer, lock, installed }
    }

    /// Whether the root `composer.json` requires `name` (`require` or
    /// `require-dev`).
    pub fn required(&self, name: &str) -> bool {
        self.constraint(name).is_some()
    }

    /// The version constraint the root `composer.json` gives `name`.
    pub fn constraint(&self, name: &str) -> Option<&str> {
        let composer = self.composer.as_ref()?;
        ["require", "require-dev"].iter().find_map(|section| composer.get(section)?.get(name)?.as_str())
    }

    /// The version of `name` that is locked (`composer.lock`, else
    /// `installed.json`), without the `v`.
    pub fn locked(&self, name: &str) -> Option<String> {
        self.lock
            .as_ref()
            .and_then(|lock| locked_version(lock, name))
            .or_else(|| self.installed.as_ref().and_then(|installed| locked_version(installed, name)))
    }

    /// Whether `name` is required or locked.
    pub fn installed(&self, name: &str) -> bool {
        self.required(name) || self.locked(name).is_some()
    }
}

fn read_json(path: &Path) -> Option<Value> {
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

/// The version of package `name` in a `composer.lock` (`packages`,
/// `packages-dev`) or an `installed.json` (either format), without `v`.
fn locked_version(json: &Value, name: &str) -> Option<String> {
    let lists: Vec<&Vec<Value>> = match json {
        Value::Array(packages) => vec![packages],
        Value::Object(map) => {
            ["packages", "packages-dev"].iter().filter_map(|key| map.get(*key).and_then(Value::as_array)).collect()
        }
        _ => Vec::new(),
    };
    lists.into_iter().flatten().find(|package| package.get("name").and_then(Value::as_str) == Some(name)).map(
        |package| {
            let version = package.get("version").and_then(Value::as_str).unwrap_or_default();
            version.strip_prefix('v').unwrap_or(version).to_owned()
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(files: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for (path, text) in files {
            let path = dir.path().join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
        dir
    }

    fn detected(files: &[(&str, &str)]) -> Vec<Framework> {
        let dir = project(files);
        Frameworks::detect(dir.path(), &FrameworkSettings::default()).iter().collect()
    }

    #[test]
    fn composer_requirements_name_the_framework() {
        for (package, framework) in [
            ("laravel/framework", Framework::Laravel),
            ("symfony/framework-bundle", Framework::Symfony),
            ("roots/wordpress", Framework::WordPress),
            ("drupal/core-recommended", Framework::Drupal),
            ("codeigniter4/framework", Framework::CodeIgniter),
            ("yiisoft/yii2", Framework::Yii),
            ("cakephp/cakephp", Framework::CakePhp),
            ("mezzio/mezzio", Framework::Laminas),
            ("magento/product-community-edition", Framework::Magento),
        ] {
            let composer = format!(r#"{{"require": {{"{package}": "*"}}}}"#);
            assert_eq!(detected(&[("composer.json", &composer)]), [framework], "{package}");
        }
    }

    #[test]
    fn a_locked_package_counts_without_a_requirement() {
        let lock = r#"{"packages": [{"name": "symfony/framework-bundle", "version": "v7.1.0"}], "packages-dev": []}"#;
        assert_eq!(detected(&[("composer.json", "{}"), ("composer.lock", lock)]), [Framework::Symfony]);
        let installed = r#"{"packages": [{"name": "yiisoft/yii2", "version": "2.0.50"}]}"#;
        assert_eq!(detected(&[("vendor/composer/installed.json", installed)]), [Framework::Yii]);
    }

    #[test]
    fn marker_files_name_the_framework_without_composer() {
        assert_eq!(detected(&[("artisan", "")]), [Framework::Laravel]);
        assert_eq!(detected(&[("wp-config.php", "<?php")]), [Framework::WordPress]);
        assert_eq!(detected(&[("web/core/lib/Drupal.php", "<?php")]), [Framework::Drupal]);
        assert_eq!(detected(&[("spark", "")]), [Framework::CodeIgniter]);
        assert_eq!(detected(&[("app/etc/di.xml", "<config/>")]), [Framework::Magento]);
        assert_eq!(detected(&[("index.php", "<?php")]), []);
    }

    #[test]
    fn a_plugin_a_theme_or_a_module_alone_is_recognised() {
        assert_eq!(detected(&[("my-plugin.php", "<?php\n/**\n * Plugin Name: Mine\n */")]), [Framework::WordPress]);
        assert_eq!(detected(&[("style.css", "/*\nTheme Name: Mine\n*/")]), [Framework::WordPress]);
        assert_eq!(detected(&[("other.php", "<?php // Plugin, by name only")]), []);
        assert_eq!(
            detected(&[("mine.info.yml", "name: Mine\ntype: module\ncore_version_requirement: ^10\n")]),
            [Framework::Drupal]
        );
        assert_eq!(detected(&[("registration.php", "<?php"), ("etc/module.xml", "<config/>")]), [Framework::Magento]);
    }

    #[test]
    fn settings_force_a_framework_on_or_off() {
        let dir = project(&[("artisan", "")]);
        let settings = FrameworkSettings {
            laravel: Detection::Disabled,
            wordpress: Detection::Enabled,
            ..FrameworkSettings::default()
        };
        let found: Vec<Framework> = Frameworks::detect(dir.path(), &settings).iter().collect();
        assert_eq!(found, [Framework::WordPress]);
    }

    #[test]
    fn several_frameworks_can_be_detected_at_once() {
        let composer = r#"{"require": {"drupal/core": "^10", "symfony/framework-bundle": "^6"}}"#;
        assert_eq!(detected(&[("composer.json", composer)]), [Framework::Symfony, Framework::Drupal]);
    }

    #[test]
    fn a_set_holds_what_was_inserted() {
        let mut set = Frameworks::default();
        assert!(set.is_empty());
        set.insert(Framework::Magento);
        set.insert(Framework::Laravel);
        assert!(set.contains(Framework::Magento) && !set.contains(Framework::Yii));
        set.remove(Framework::Magento);
        assert_eq!(set.iter().collect::<Vec<_>>(), [Framework::Laravel]);
    }
}
