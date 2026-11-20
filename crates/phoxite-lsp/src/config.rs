//! The server's settings, as the client sends them (`initializationOptions`
//! and `workspace/didChangeConfiguration`), mapped onto the engine's
//! [`PhpConfig`].
//!
//! ```json
//! {
//!   "phpVersion": "8.3",
//!   "includePaths": ["../shared"],
//!   "stubs": ["redis"],
//!   "exclude": ["storage", "node_modules"],
//!   "inspections": { "PhpUndefinedMethod": { "enabled": false }, "PhpTypeMismatch": { "strict": true, "severity": "error" } },
//!   "laravel": { "routeHints": true, "modules": "auto", "generation": { "module": "Blog" } },
//!   "frameworks": { "laravel": "auto", "symfony": "auto", "wordpress": "disabled" },
//!   "format": { "lineWidth": 120, "tool": "phoxite", "twig": "phoxite" },
//!   "templates": { "path": ".phoxite/templates" },
//!   "analyzers": { "onSave": true, "phpPath": "php", "nodePath": "node" }
//! }
//! ```
//!
//! The settings may also come wrapped in a `phoxite` section, as VS Code
//! sends them.
//!
//! A project's own settings file ([`PROJECT_FILE`], at its root) holds the
//! same object and is laid over what the client sent ([`Settings::layered`]).
//! Its schema is `editors/vscode/schemas/phoxite.schema.json`: a new setting
//! goes there too, and a test here says when it doesn't.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use phoxite::base::Severity;
use phoxite::base::inspection_settings::InspectionOverride;
use phoxite::base::settings::{FrameworkSettings, LaravelSettings, PhpConfig};
use phoxite::inspect::{PROFILE_ID, STRICT_OPTION};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Folders nobody wants indexed, unless the user lists their own.
pub const DEFAULT_EXCLUDES: &[&str] = &["node_modules", ".git", "storage/framework", "bootstrap/cache"];

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub php_version: String,
    pub include_paths: Vec<String>,
    pub stubs: Vec<String>,
    pub exclude: Option<Vec<String>>,
    pub inspections: BTreeMap<String, InspectionSetting>,
    pub laravel: LaravelSettings,
    /// Which frameworks' support is on: `auto`, `enabled` or `disabled`
    /// for each (`{ "wordpress": "enabled" }`).
    pub frameworks: FrameworkSettings,
    pub format: FormatSettings,
    pub templates: TemplateSettings,
    pub analyzers: AnalyzerSettings,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct InspectionSetting {
    pub enabled: Option<bool>,
    /// The type and call checks: report what they can't prove too.
    pub strict: Option<bool>,
    /// `error`, `warning`, `information` or `hint`.
    pub severity: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct FormatSettings {
    pub line_width: u32,
    /// What formats a whole PHP file: `phoxite` (the built-in formatter) or
    /// `pint` (the project's Laravel Pint, `pint.rs`).
    pub tool: String,
    /// What formats a whole Twig template: `phoxite` (the built-in formatter) or
    /// `prettier` (the project's Prettier, `prettier.rs`).
    pub twig: String,
}

/// The generators' templates the user edited.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TemplateSettings {
    /// Their folder, relative to the project (or absolute); empty:
    /// [`DEFAULT_TEMPLATES_PATH`].
    pub path: String,
}

/// PHPStan and Psalm on save (`analyzers.rs`).
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AnalyzerSettings {
    /// Run the analysers the project installs on each saved PHP file.
    pub on_save: bool,
    /// The PHP interpreter that runs them; empty: `php` on `PATH`.
    pub php_path: String,
    /// The Node.js that runs the project's Prettier (`format.twig`); empty: `node` on
    /// `PATH`.
    pub node_path: String,
}

/// Where the edited templates of a project are unless the settings say.
pub const DEFAULT_TEMPLATES_PATH: &str = ".phoxite/templates";

impl Default for FormatSettings {
    fn default() -> Self {
        Self { line_width: 120, tool: "phoxite".to_owned(), twig: "phoxite".to_owned() }
    }
}

impl Settings {
    /// Settings from the client's JSON: the object itself, or its
    /// `phoxite` member. A setting whose value doesn't fit keeps its default,
    /// the others apply; the second value names those left out
    /// (`laravel.modules`).
    pub fn from_json(value: &Value) -> (Self, Vec<String>) {
        let value = value.get("phoxite").unwrap_or(value);
        let (mut settings, mut rejected): (Self, _) = lenient(value);
        for (id, setting) in &mut settings.inspections {
            if setting.severity.as_deref().is_some_and(|name| severity_named(name).is_none()) {
                setting.severity = None;
                rejected.push(format!("inspections.{id}.severity"));
            }
        }
        if !rejected.is_empty() {
            tracing::warn!("settings left at their defaults: {}", rejected.join(", "));
        }
        (settings, rejected)
    }

    pub fn engine_config(&self) -> PhpConfig {
        let excludes = match &self.exclude {
            Some(own) => own.clone(),
            None => DEFAULT_EXCLUDES.iter().map(|s| s.to_string()).collect(),
        };
        let inspections = self
            .inspections
            .iter()
            .map(|(id, setting)| {
                let key = if id == "*" { PROFILE_ID.to_owned() } else { format!("{PROFILE_ID}:{id}") };
                let options = setting.strict.map(|strict| BTreeMap::from([(STRICT_OPTION.to_owned(), strict)]));
                (key, InspectionOverride { enabled: setting.enabled, severity: None, options })
            })
            .collect();
        PhpConfig {
            language_level: self.php_version.clone(),
            include_paths: self.include_paths.clone(),
            stubs: self.stubs.clone(),
            excludes,
            inspections,
            laravel: self.laravel.clone(),
            frameworks: self.frameworks,
        }
    }

    /// Gives `problem` the severity the user chose for its inspection.
    pub fn apply_severity(&self, problem: &mut phoxite::base::Diagnostic) {
        if let Some(severity) = problem.code.as_deref().and_then(|code| self.severity(code)) {
            problem.severity = severity;
        }
    }

    /// Whether the inspection `code` is on: what its own setting says, else
    /// what `*` says, else yes. For the problems the server finds itself
    /// (a Twig template's); the engine reads the same settings for its own.
    pub fn inspection_enabled(&self, code: &str) -> bool {
        let enabled = |id: &str| self.inspections.get(id).and_then(|setting| setting.enabled);
        enabled(code).or_else(|| enabled("*")).unwrap_or(true)
    }

    /// The severity the user chose for inspection `code`.
    pub fn severity(&self, code: &str) -> Option<Severity> {
        severity_named(self.inspections.get(code)?.severity.as_deref()?)
    }
}

fn severity_named(name: &str) -> Option<Severity> {
    match name {
        "error" => Some(Severity::Error),
        "warning" => Some(Severity::Warning),
        "information" | "info" | "weakWarning" => Some(Severity::Info),
        "hint" => Some(Severity::Hint),
        _ => None,
    }
}

/// `value` as a `T`, without the parts that don't fit it: those keep their
/// defaults and are named by their path (`laravel.modules`). `null` is the
/// defaults; another value that isn't an object is left out as a whole
/// (an empty path).
pub fn lenient<T: DeserializeOwned + Default>(value: &Value) -> (T, Vec<String>) {
    if value.is_null() {
        return (T::default(), Vec::new());
    }
    if let Ok(whole) = serde_json::from_value(value.clone()) {
        return (whole, Vec::new());
    }
    let mut rejected = Vec::new();
    let mut kept = value.clone();
    match &mut kept {
        Value::Object(members) => prune::<T>(members, &mut Vec::new(), &mut rejected),
        _ => rejected.push(String::new()),
    }
    (serde_json::from_value(kept).unwrap_or_default(), rejected)
}

/// Removes from `members` (the object at `path`) what a `T` can't be read
/// from: each member is tried alone, and an object that doesn't fit is
/// looked into before it is dropped whole.
fn prune<T: DeserializeOwned>(
    members: &mut serde_json::Map<String, Value>,
    path: &mut Vec<String>,
    rejected: &mut Vec<String>,
) {
    let keys: Vec<String> = members.keys().cloned().collect();
    for key in keys {
        path.push(key.clone());
        let alone = path
            .iter()
            .rev()
            .fold(members[&key].clone(), |inner, name| Value::Object([(name.clone(), inner)].into_iter().collect()));
        if serde_json::from_value::<T>(alone).is_err() {
            let before = rejected.len();
            if let Some(Value::Object(inner)) = members.get_mut(&key) {
                prune::<T>(inner, path, rejected);
            }
            if rejected.len() == before {
                members.remove(&key);
                rejected.push(path.join("."));
            }
        }
        path.pop();
    }
}

/// The settings file of a project, at its root: the settings above, for
/// everyone who works on the project whatever their editor.
pub const PROJECT_FILE: &str = "phoxite.json";

/// What `phoxite.json` of the project at `root` holds; `None` without the
/// file, an error when it isn't a JSON object.
pub fn read_project_file(root: &Path) -> Result<Option<Value>, String> {
    let path = root.join(PROJECT_FILE);
    if !path.exists() {
        return Ok(None);
    }
    read_settings_file(&path).map(Some)
}

/// The settings file at `path`; an error when it can't be read or isn't a
/// JSON object.
pub fn read_settings_file(path: &Path) -> Result<Value, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    match serde_json::from_str::<Value>(text.trim_start_matches('\u{feff}')) {
        Ok(value @ Value::Object(_)) => Ok(value),
        Ok(_) => Err("it has to hold an object".to_owned()),
        Err(e) => Err(e.to_string()),
    }
}

/// What a project's settings file may set. A project one merely opened
/// doesn't choose the programs that run or the files read outside it:
/// `analyzers` and `laravel.database` are never taken from the file, and
/// `includePaths` only in a workspace the user trusts.
fn project_settings(file: &Value, trusted: bool) -> Value {
    let mut file = file.clone();
    if let Some(members) = file.as_object_mut() {
        let mut ignored = Vec::new();
        if members.remove("analyzers").is_some() {
            ignored.push("analyzers");
        }
        if members.get_mut("laravel").and_then(Value::as_object_mut).and_then(|l| l.remove("database")).is_some() {
            ignored.push("laravel.database");
        }
        if !trusted && members.remove("includePaths").is_some() {
            ignored.push("includePaths");
        }
        if !ignored.is_empty() {
            tracing::warn!("{PROJECT_FILE}: {} is only read from the editor's settings", ignored.join(", "));
        }
    }
    file
}

/// Lays `over` over `base`: objects member by member, anything else whole.
fn overlay(base: &mut Value, over: Value) {
    match (base, over) {
        (Value::Object(base), Value::Object(over)) => {
            for (key, value) in over {
                match base.get_mut(&key) {
                    Some(below) => overlay(below, value),
                    None => {
                        base.insert(key, value);
                    }
                }
            }
        }
        (base, over) => *base = over,
    }
}

impl Settings {
    /// The settings of one project: the client's (as for
    /// [`from_json`](Self::from_json)), and over them what the project's
    /// settings file sets. The file wins because a client sends every
    /// setting, the ones the user never touched too.
    pub fn layered(client: &Value, file: Option<&Value>, trusted: bool) -> (Self, Vec<String>) {
        let Some(file) = file else { return Self::from_json(client) };
        let mut merged = match client.get("phoxite").unwrap_or(client) {
            base @ Value::Object(_) => base.clone(),
            _ => Value::Object(Default::default()),
        };
        overlay(&mut merged, project_settings(file, trusted));
        Self::from_json(&merged)
    }
}

/// `initializationOptions`.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct InitOptions {
    /// Where the indexes are kept; default: the user's cache folder.
    pub cache_dir: Option<PathBuf>,
    pub settings: Option<Value>,
    /// Whether the user trusts the workspace (programs of the project may
    /// run); default: yes.
    pub trusted: Option<bool>,
}

/// The default cache folder: `%LOCALAPPDATA%\phoxite`,
/// `~/Library/Caches/phoxite` or `$XDG_CACHE_HOME/phoxite`.
pub fn default_cache_dir() -> PathBuf {
    let env = |name: &str| std::env::var_os(name).filter(|v| !v.is_empty()).map(PathBuf::from);
    let base = if cfg!(windows) {
        env("LOCALAPPDATA")
    } else if cfg!(target_os = "macos") {
        env("HOME").map(|home| home.join("Library/Caches"))
    } else {
        env("XDG_CACHE_HOME").or_else(|| env("HOME").map(|home| home.join(".cache")))
    };
    base.unwrap_or_else(std::env::temp_dir).join("phoxite")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn settings_map_onto_the_engine_config() {
        let (settings, rejected) = Settings::from_json(&json!({
            "phoxite": {
                "phpVersion": "8.1",
                "inspections": {
                    "*": { "enabled": true },
                    "PhpUndefinedMethod": { "enabled": false },
                    "PhpTypeMismatch": { "strict": true, "severity": "error" }
                },
                "laravel": { "routeHints": false, "modules": "nwidart" }
            }
        }));
        assert!(rejected.is_empty(), "{rejected:?}");
        let config = settings.engine_config();
        assert_eq!(config.language_level, "8.1");
        assert_eq!(config.excludes, DEFAULT_EXCLUDES);
        assert_eq!(config.inspections["lsp:php:PhpUndefinedMethod"].enabled, Some(false));
        assert!(config.inspections["lsp:php:PhpTypeMismatch"].options.as_ref().unwrap()["strict"]);
        assert!(config.inspections.contains_key("lsp:php"));
        assert!(!config.laravel.route_hints);
        assert_eq!(settings.severity("PhpTypeMismatch"), Some(Severity::Error));
        assert_eq!(settings.format.line_width, 120);
    }

    #[test]
    fn frameworks_are_forced_on_or_off_by_name() {
        use phoxite::base::settings::Detection;

        let (settings, rejected) = Settings::from_json(&json!({
            "frameworks": { "laravel": "disabled", "wordpress": "enabled", "drupal": "sometimes" }
        }));
        // A value that is no choice keeps its default and is named.
        assert_eq!(rejected, ["frameworks.drupal"]);
        let frameworks = settings.engine_config().frameworks;
        assert_eq!(frameworks.laravel, Detection::Disabled);
        assert_eq!(frameworks.wordpress, Detection::Enabled);
        assert_eq!(frameworks.drupal, Detection::Auto);
        assert_eq!(frameworks.symfony, Detection::Auto);
    }

    #[test]
    fn a_broken_setting_keeps_its_default_and_the_others_apply() {
        let (settings, rejected) = Settings::from_json(&json!({
            "phpVersion": 8,
            "stubs": ["redis"],
            "laravel": { "modules": "bogus", "routeHints": false, "generation": { "typeHints": "no", "module": "Blog" } },
            "inspections": {
                "PhpUndefinedMethod": { "enabled": "off", "strict": true },
                "PhpParams": { "enabled": false, "severity": "loud" }
            },
            "format": "wide",
            "somethingNew": { "ignored": true }
        }));
        assert_eq!(
            rejected,
            [
                "phpVersion",
                "laravel.modules",
                "laravel.generation.typeHints",
                "inspections.PhpUndefinedMethod.enabled",
                "format",
                "inspections.PhpParams.severity",
            ]
        );
        assert_eq!(settings.php_version, "");
        assert_eq!(settings.stubs, ["redis"]);
        assert!(!settings.laravel.route_hints);
        assert_eq!(settings.laravel.modules, LaravelSettings::default().modules);
        assert_eq!(settings.laravel.generation.module, "Blog");
        assert!(settings.laravel.generation.type_hints);
        assert_eq!(
            settings.inspections["PhpUndefinedMethod"],
            InspectionSetting { strict: Some(true), ..Default::default() }
        );
        assert_eq!(settings.inspections["PhpParams"], InspectionSetting { enabled: Some(false), ..Default::default() });
        assert_eq!(settings.format, FormatSettings::default());
    }

    #[test]
    fn settings_that_are_no_object() {
        assert_eq!(Settings::from_json(&Value::Null), (Settings::default(), vec![]));
        assert_eq!(Settings::from_json(&json!({ "phoxite": null })), (Settings::default(), vec![]));
        assert_eq!(Settings::from_json(&json!("on")), (Settings::default(), vec![String::new()]));
        // An object where a text is expected is left out as a whole.
        assert_eq!(Settings::from_json(&json!({ "phpVersion": {} })), (Settings::default(), vec!["phpVersion".into()]));
    }

    #[test]
    fn a_project_file_is_laid_over_the_clients_settings() {
        let client = json!({ "phoxite": {
            "phpVersion": "8.1",
            "stubs": ["redis"],
            "laravel": { "routeHints": false, "database": "auto", "generation": { "module": "Blog" } },
            "analyzers": { "onSave": false, "phpPath": "php8.3" },
            "inspections": { "PhpParams": { "strict": true, "enabled": true } }
        } });
        let file = json!({
            "$schema": "phoxite.schema.json",
            "phpVersion": "8.4",
            "includePaths": ["../shared"],
            "laravel": { "database": "/etc/passwd", "generation": { "controllerSuffix": "Action" } },
            "analyzers": { "onSave": true, "phpPath": "./run-me" },
            "inspections": { "PhpParams": { "enabled": false } }
        });

        let (settings, rejected) = Settings::layered(&client, Some(&file), false);
        assert!(rejected.is_empty(), "{rejected:?}");
        // The file wins where it sets something; the rest is the client's.
        assert_eq!(settings.php_version, "8.4");
        assert_eq!(settings.stubs, ["redis"]);
        assert!(!settings.laravel.route_hints);
        assert_eq!(settings.laravel.generation.module, "Blog");
        assert_eq!(settings.laravel.generation.controller_suffix, "Action");
        assert_eq!(
            settings.inspections["PhpParams"],
            InspectionSetting { enabled: Some(false), strict: Some(true), severity: None }
        );
        // A project doesn't choose the programs that run or the files read outside it.
        assert_eq!(
            settings.analyzers,
            AnalyzerSettings { on_save: false, php_path: "php8.3".into(), node_path: String::new() }
        );
        assert_eq!(settings.laravel.database, "auto");
        assert!(settings.include_paths.is_empty());

        // Include paths are taken from the file of a project the user trusts.
        let (trusted, _) = Settings::layered(&client, Some(&file), true);
        assert_eq!(trusted.include_paths, ["../shared"]);
        assert_eq!(trusted.analyzers.php_path, "php8.3");

        // No file: the client's settings as they are.
        assert_eq!(Settings::layered(&client, None, true), Settings::from_json(&client));
        // A value of the file that doesn't fit is named like the client's.
        let (settings, rejected) =
            Settings::layered(&client, Some(&json!({ "phpVersion": 8, "stubs": ["ffi"] })), true);
        assert_eq!(rejected, ["phpVersion"]);
        assert_eq!((settings.php_version.as_str(), settings.stubs), ("", vec!["ffi".to_owned()]));
    }

    /// The defaults a JSON schema declares, as the object they describe.
    fn schema_defaults(schema: &Value) -> Value {
        match (schema.get("default"), schema.get("properties").and_then(Value::as_object)) {
            (Some(default), _) => default.clone(),
            (None, Some(properties)) => {
                Value::Object(properties.iter().map(|(name, schema)| (name.clone(), schema_defaults(schema))).collect())
            }
            (None, None) => panic!("neither a default nor properties: {schema}"),
        }
    }

    /// The schema of `phoxite.json` (the VS Code extension validates the file with it)
    /// has every setting a project file may set, with the default the server uses.
    #[test]
    fn the_schema_of_the_project_file_has_the_settings_and_their_defaults() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../editors/vscode/schemas/phoxite.schema.json");
        let mut schema: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        schema["properties"].as_object_mut().unwrap().remove("$schema");

        // What `project_settings` never takes from the file isn't in its schema.
        let mut settings = serde_json::to_value(Settings::default()).unwrap();
        settings.as_object_mut().unwrap().remove("analyzers");
        settings["laravel"].as_object_mut().unwrap().remove("database");
        assert_eq!(schema_defaults(&schema), settings);

        // And the server takes what the schema describes.
        let (read, rejected) = Settings::layered(&Value::Null, Some(&schema_defaults(&schema)), true);
        assert_eq!((read, rejected), (Settings::default(), vec![]));
    }

    #[test]
    fn a_broken_initialization_option_keeps_the_others() {
        let (options, rejected): (InitOptions, _) =
            lenient(&json!({ "cacheDir": 7, "trusted": false, "settings": { "phpVersion": "8.2" } }));
        assert_eq!(rejected, ["cacheDir"]);
        assert_eq!(options.trusted, Some(false));
        assert_eq!(options.settings, Some(json!({ "phpVersion": "8.2" })));
    }
}
