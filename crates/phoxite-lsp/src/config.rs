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
//!   "format": { "lineWidth": 120, "tool": "phoxite" },
//!   "templates": { "path": ".phoxite/templates" },
//!   "analyzers": { "onSave": true, "phpPath": "php" }
//! }
//! ```
//!
//! The settings may also come wrapped in a `phoxite` section, as VS Code
//! sends them.

use std::collections::BTreeMap;
use std::path::PathBuf;

use phoxite::base::Severity;
use phoxite::base::inspection_settings::InspectionOverride;
use phoxite::base::settings::{LaravelSettings, PhpConfig};
use phoxite::inspect::{PROFILE_ID, STRICT_OPTION};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

/// Folders nobody wants indexed, unless the user lists their own.
pub const DEFAULT_EXCLUDES: &[&str] = &["node_modules", ".git", "storage/framework", "bootstrap/cache"];

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub php_version: String,
    pub include_paths: Vec<String>,
    pub stubs: Vec<String>,
    pub exclude: Option<Vec<String>>,
    pub inspections: BTreeMap<String, InspectionSetting>,
    pub laravel: LaravelSettings,
    pub format: FormatSettings,
    pub templates: TemplateSettings,
    pub analyzers: AnalyzerSettings,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct InspectionSetting {
    pub enabled: Option<bool>,
    /// The type and call checks: report what they can't prove too.
    pub strict: Option<bool>,
    /// `error`, `warning`, `information` or `hint`.
    pub severity: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct FormatSettings {
    pub line_width: u32,
    /// What formats a whole PHP file: `phoxite` (the built-in formatter) or
    /// `pint` (the project's Laravel Pint, `pint.rs`).
    pub tool: String,
}

/// The generators' templates the user edited.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct TemplateSettings {
    /// Their folder, relative to the project (or absolute); empty:
    /// [`DEFAULT_TEMPLATES_PATH`].
    pub path: String,
}

/// PHPStan and Psalm on save (`analyzers.rs`).
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct AnalyzerSettings {
    /// Run the analysers the project installs on each saved PHP file.
    pub on_save: bool,
    /// The PHP interpreter that runs them; empty: `php` on `PATH`.
    pub php_path: String,
}

/// Where the edited templates of a project are unless the settings say.
pub const DEFAULT_TEMPLATES_PATH: &str = ".phoxite/templates";

impl Default for FormatSettings {
    fn default() -> Self {
        Self { line_width: 120, tool: "phoxite".to_owned() }
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
        }
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
    fn a_broken_initialization_option_keeps_the_others() {
        let (options, rejected): (InitOptions, _) =
            lenient(&json!({ "cacheDir": 7, "trusted": false, "settings": { "phpVersion": "8.2" } }));
        assert_eq!(rejected, ["cacheDir"]);
        assert_eq!(options.trusted, Some(false));
        assert_eq!(options.settings, Some(json!({ "phpVersion": "8.2" })));
    }
}
