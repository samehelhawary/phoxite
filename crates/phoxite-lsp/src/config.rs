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
//!   "laravel": { "routeHints": true, "modules": "auto" },
//!   "format": { "lineWidth": 120 }
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
}

impl Default for FormatSettings {
    fn default() -> Self {
        Self { line_width: 120 }
    }
}

impl Settings {
    /// Settings from the client's JSON: the object itself, or its
    /// `phoxite` member. Invalid parts fall back to the defaults.
    pub fn from_json(value: &Value) -> Self {
        let value = value.get("phoxite").unwrap_or(value);
        match serde_json::from_value(value.clone()) {
            Ok(settings) => settings,
            Err(e) => {
                tracing::warn!("invalid settings, using the defaults: {e}");
                Self::default()
            }
        }
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
        match self.inspections.get(code)?.severity.as_deref()? {
            "error" => Some(Severity::Error),
            "warning" => Some(Severity::Warning),
            "information" | "info" | "weakWarning" => Some(Severity::Info),
            "hint" => Some(Severity::Hint),
            other => {
                tracing::warn!("unknown severity {other:?} for {code}");
                None
            }
        }
    }
}

/// `initializationOptions`.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct InitOptions {
    /// Where the indexes are kept; default: the user's cache folder.
    pub cache_dir: Option<PathBuf>,
    pub settings: Option<Value>,
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
        let settings = Settings::from_json(&json!({
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
    fn broken_settings_fall_back_to_the_defaults() {
        assert_eq!(Settings::from_json(&json!({ "phpVersion": 8 })), Settings::default());
    }
}
