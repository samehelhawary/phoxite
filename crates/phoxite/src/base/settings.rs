//! The engine's settings: what a front end maps its own configuration onto.
//!
//! [`PhpConfig`] carries the project-wide options (language level, include
//! paths, stub sets, excluded folders, inspection overrides, Laravel), and
//! resolves the ones that fall back to `composer.json`.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::inspection_settings::InspectionOverride;

/// Project-wide PHP options.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PhpConfig {
    /// The PHP version the code is checked against (`8.2`); empty = from
    /// `composer.json` (`config.platform.php`, else the lowest version
    /// `require.php` allows), else the latest.
    pub language_level: String,
    /// Folders outside the project whose code is indexed too; absolute, `~/`,
    /// or relative to the project.
    pub include_paths: Vec<String>,
    /// Stub sets on top of the defaults and the `ext-*` entries of
    /// `composer.json` (`swoole`).
    pub stubs: Vec<String>,
    /// Globs of excluded files and folders, relative to the project
    /// (`node_modules`, `/storage`).
    pub excludes: Vec<String>,
    /// Inspection overrides by id (`php:PhpUndefinedClassInspection`).
    pub inspections: BTreeMap<String, InspectionOverride>,
    pub laravel: LaravelSettings,
    pub frameworks: FrameworkSettings,
}

impl PhpConfig {
    /// The project's PHP version `X.Y.Z`: the configured one, else the one
    /// `composer.json` declares.
    pub fn language_level(&self, root: &Path) -> Option<String> {
        let explicit = self.language_level.trim();
        if !explicit.is_empty() {
            return version(explicit).map(format_version);
        }
        composer_level(root)
    }

    /// The stub sets the project uses: the [defaults](default_stub_sets), a
    /// set for each `ext-*` the root `composer.json` requires, then the
    /// configured ones, without duplicates (compared by [`stub_key`]).
    pub fn stub_sets(&self, root: &Path) -> Vec<String> {
        let composer = read_json(&root.join("composer.json"));
        let extensions = ["require", "require-dev"]
            .into_iter()
            .filter_map(|section| composer.as_ref()?.get(section)?.as_object())
            .flat_map(|deps| deps.keys())
            .filter_map(|dep| dep.strip_prefix("ext-"))
            .map(|ext| {
                let key = stub_key(ext);
                MIXED_CASE_STUBS
                    .iter()
                    .find(|stub| stub_key(stub) == key)
                    .map_or(ext.to_ascii_lowercase(), |s| s.to_string())
            })
            .collect::<Vec<_>>();
        let configured = self.stubs.iter().map(|stub| stub.trim().to_owned());
        let mut stubs: Vec<String> = DEFAULT_STUBS.iter().map(|s| s.to_string()).collect();
        let mut known: HashSet<String> = stubs.iter().map(|s| stub_key(s)).collect();
        for stub in extensions.into_iter().chain(configured) {
            if !stub.is_empty() && known.insert(stub_key(&stub)) {
                stubs.push(stub);
            }
        }
        stubs
    }

    /// The include paths, resolved against the project.
    pub fn include_paths(&self, root: &Path) -> Vec<PathBuf> {
        self.include_paths.iter().filter_map(|p| setting_path(root, p)).collect()
    }
}

/// A configured path: absolute, `~/…`, or relative to `root`. `None` when empty.
pub fn setting_path(root: &Path, s: &str) -> Option<PathBuf> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let p = match s.strip_prefix("~/") {
        Some(rest) => home_dir()?.join(rest),
        None => PathBuf::from(s),
    };
    Some(if p.is_absolute() { p } else { root.join(p) })
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).filter(|h| !h.is_empty()).map(PathBuf::from)
}

/// The stub sets loaded when none are configured (the phpstorm-stubs folders
/// a stock PHP has).
#[rustfmt::skip]
const DEFAULT_STUBS: &[&str] = &[
    "apache", "bcmath", "bz2", "calendar", "com_dotnet", "Core", "ctype", "curl", "date", "dba", "dom", "enchant",
    "exif", "fileinfo", "filter", "fpm", "ftp", "gd", "hash", "iconv", "imap", "intl", "json", "ldap", "libxml",
    "mbstring", "mcrypt", "mssql", "mysqli", "oci8", "odbc", "openssl", "pcntl", "pcre", "PDO", "pgsql", "Phar",
    "posix", "pspell", "random", "readline", "Reflection", "regex", "session", "shmop", "SimpleXML", "snmp", "soap",
    "sockets", "sodium", "SPL", "sqlite3", "standard", "superglobals", "sybase", "sysvmsg", "sysvsem", "sysvshm",
    "tidy", "tokenizer", "uri", "xml", "xmlreader", "xmlrpc", "xmlwriter", "Zend OPcache", "zip", "zlib",
];

/// Stub sets outside the defaults whose names are not lower case.
const MIXED_CASE_STUBS: &[&str] =
    &["Ev", "FFI", "LuaSandbox", "Parle", "SQLite", "SaxonC", "SplType", "ZendCache", "ZendDebugger", "ZendUtils"];

/// The default stub sets.
pub fn default_stub_sets() -> &'static [&'static str] {
    DEFAULT_STUBS
}

/// `Zend OPcache`, `zend-opcache` and `zend_opcache` name the same stubs.
pub fn stub_key(name: &str) -> String {
    name.chars().filter(char::is_ascii_alphanumeric).map(|c| c.to_ascii_lowercase()).collect()
}

fn read_json(path: &Path) -> Option<Value> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

type Version = (u32, u32, u32);

fn format_version((major, minor, patch): Version) -> String {
    format!("{major}.{minor}.{patch}")
}

/// The PHP version the root `composer.json` declares: `config.platform.php`,
/// else the lowest version `require.php` allows.
pub fn composer_level(root: &Path) -> Option<String> {
    let composer = read_json(&root.join("composer.json"))?;
    if let Some(platform) = composer.pointer("/config/platform/php").and_then(Value::as_str).and_then(version) {
        return Some(format_version(platform));
    }
    lowest_allowed(composer.pointer("/require/php")?.as_str()?).map(format_version)
}

/// `7`, `7.4`, `v7.4.3`, `8.1.*`, `7.4.0-dev`: missing parts are 0.
fn version(s: &str) -> Option<Version> {
    let s = s.trim().trim_start_matches(['v', 'V']);
    let mut parts = s.split('.').map(|p| {
        let digits: &str = &p[..p.find(|c: char| !c.is_ascii_digit()).unwrap_or(p.len())];
        digits.parse::<u32>().ok()
    });
    let major = parts.next()??;
    let minor = parts.next().flatten().unwrap_or(0);
    let patch = parts.next().flatten().unwrap_or(0);
    Some((major, minor, patch))
}

/// The lowest version a Composer constraint allows: the lowest lower bound of
/// its `||` alternatives. `None` when an alternative has no lower bound.
fn lowest_allowed(constraint: &str) -> Option<Version> {
    let bounds: Option<Vec<Version>> =
        constraint.split('|').filter(|alt| !alt.trim().is_empty()).map(lower_bound).collect();
    bounds?.into_iter().min()
}

/// The lower bound of constraints that must all hold (`>=7.2 <8.4`,
/// `^7.4, !=7.4.1`, `7.2 - 8.0`): the highest of their lower bounds.
fn lower_bound(alternative: &str) -> Option<Version> {
    let alternative = alternative.split(" - ").next().unwrap_or(alternative);
    let mut tokens: Vec<String> = Vec::new();
    for token in alternative.split([' ', ',']).filter(|t| !t.is_empty()) {
        match tokens.last_mut() {
            Some(op) if op.chars().all(|c| "<>=!~^".contains(c)) => op.push_str(token),
            _ => tokens.push(token.to_owned()),
        }
    }
    tokens
        .iter()
        .filter_map(|t| {
            let t = t.split('@').next().unwrap_or(t);
            if t.starts_with('<') || t.starts_with("!=") {
                return None;
            }
            version(t.trim_start_matches(['>', '=', '^', '~']))
        })
        .max()
}

/// Laravel support. Defaults suit a stock Laravel application.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LaravelSettings {
    /// A code lens on controller methods listing their routes, and the full
    /// URI after each route registration.
    pub route_hints: bool,
    /// The id of the data source whose live schema the front end supplies
    /// (see `PhpEngine::set_live_schema`); empty = the migrations alone.
    pub database: String,
    /// What the Laravel generators write.
    pub generation: LaravelCodeGeneration,
    /// Whether the project is a Laravel project.
    pub detection: LaravelDetection,
    /// Which modules of a modular application are read.
    pub modules: LaravelModuleSystem,
    /// The folders of Inertia pages, relative to the project; empty = the
    /// `ide.json` page paths, else `resources/js/Pages`.
    pub inertia_pages: Vec<String>,
}

impl Default for LaravelSettings {
    fn default() -> Self {
        Self {
            route_hints: true,
            database: String::new(),
            generation: LaravelCodeGeneration::default(),
            detection: LaravelDetection::default(),
            modules: LaravelModuleSystem::default(),
            inertia_pages: Vec::new(),
        }
    }
}

/// Whether a framework's support is on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Detection {
    /// On when the project looks like one of the framework: Composer
    /// installs it, or one of its marker files exists (for Laravel:
    /// `composer.json` requires `laravel/framework`, a lock file lists it,
    /// or `artisan` exists). See [`crate::frameworks::detect`].
    #[default]
    Auto,
    Enabled,
    Disabled,
}

/// The name [`Detection`] had when Laravel was the only framework.
pub type LaravelDetection = Detection;

/// Which frameworks' support is on, by framework
/// ([`crate::frameworks::Framework::key`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct FrameworkSettings {
    /// Wins over [`LaravelSettings::detection`] unless it is `auto`.
    pub laravel: Detection,
    pub symfony: Detection,
    pub wordpress: Detection,
    pub drupal: Detection,
    pub codeigniter: Detection,
    pub yii: Detection,
    pub cakephp: Detection,
    pub laminas: Detection,
    pub magento: Detection,
}

impl FrameworkSettings {
    /// These settings with Laravel's taken from `laravel.detection` when
    /// `frameworks.laravel` doesn't say.
    pub fn with_laravel(mut self, laravel: Detection) -> Self {
        if self.laravel == Detection::Auto {
            self.laravel = laravel;
        }
        self
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LaravelModuleSystem {
    /// `nwidart/laravel-modules` modules when the package is installed, and
    /// module folders.
    #[default]
    Auto,
    /// Only `nwidart/laravel-modules` modules.
    Nwidart,
    /// Only the folders of `modules/`, `app-modules/`, `app/Modules/` and
    /// `src/Modules/`.
    Directory,
    None,
}

/// What the Laravel generators write.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LaravelCodeGeneration {
    /// The application's root namespace (`App\`); empty = the one
    /// `composer.json` maps to `app/`.
    pub namespace: String,
    /// The module new classes go to; empty = the application.
    pub module: String,
    pub controller_suffix: String,
    pub request_suffix: String,
    pub resource_suffix: String,
    pub policy_suffix: String,
    pub seeder_suffix: String,
    /// Constructors promote their parameters to properties.
    pub constructor_promotion: bool,
    /// Generated methods declare their return types.
    pub type_hints: bool,
}

impl Default for LaravelCodeGeneration {
    fn default() -> Self {
        Self {
            namespace: String::new(),
            module: String::new(),
            controller_suffix: "Controller".to_owned(),
            request_suffix: "Request".to_owned(),
            resource_suffix: "Resource".to_owned(),
            policy_suffix: "Policy".to_owned(),
            seeder_suffix: "Seeder".to_owned(),
            constructor_promotion: true,
            type_hints: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn composer_constraints_give_their_lowest_version() {
        assert_eq!(lowest_allowed("^8.1"), Some((8, 1, 0)));
        assert_eq!(lowest_allowed("^7.4 || ^8.0"), Some((7, 4, 0)));
        assert_eq!(lowest_allowed(">=7.2 <8.4"), Some((7, 2, 0)));
        assert_eq!(lowest_allowed("7.2 - 8.0"), Some((7, 2, 0)));
        assert_eq!(lowest_allowed("<8.0"), None);
        assert_eq!(version("v8.1.*"), Some((8, 1, 0)));
    }

    #[test]
    fn stub_sets_add_the_required_extensions_once() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("composer.json"),
            r#"{"require": {"ext-redis": "*", "ext-ffi": "*", "ext-json": "*"}}"#,
        )
        .unwrap();
        let config = PhpConfig { stubs: vec!["swoole".into(), "Redis".into()], ..PhpConfig::default() };
        let sets = config.stub_sets(dir.path());
        assert_eq!(&sets[DEFAULT_STUBS.len()..], ["redis", "FFI", "swoole"]);
    }
}
