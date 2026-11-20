//! The `drupalConfigKey` reference kind: a key of a configuration object,
//! `currency` in `\Drupal::config('shop.settings')->get('currency')`.
//!
//! The configuration is the one the receiver was made for: the type
//! provider types `config('shop.settings')` as a `Config` of that name
//! ([`crate::types::provider::DrupalProvider`]), which a variable keeps. Its
//! keys are those of the file declaring it (`config/install/shop.settings.yml`),
//! nested ones with dots (`mail.from`); when no file installs it, those its
//! schema describes (`config/schema`).
//!
//! Goto goes to the key in the file, completion offers the file's keys, and
//! Find References lists the strings naming the same key of the same
//! configuration. A key isn't renamed here: the stored configuration of a
//! site has it too.

use std::path::PathBuf;

use crate::base::{Range, yaml};
use crate::frameworks::refs::{CallTarget, CalleePattern, PlacePattern, RefContext, RefItem, ReferenceKind};
use crate::navigate::Target;

const CONFIG: CalleePattern = CalleePattern::Method {
    classes: &[
        "Drupal\\Core\\Config\\ConfigBase",
        "Drupal\\Core\\Config\\Config",
        "Drupal\\Core\\Config\\ImmutableConfig",
    ],
    names: &["get", "set", "clear", "getOriginal"],
};

static PATTERNS: [PlacePattern; 1] = [PlacePattern::argument(CONFIG, 0, "key")];

/// See the module documentation.
pub struct ConfigKey;

/// A key of a configuration file: its dotted path, its value when it is a
/// scalar, and where it is written.
struct Key {
    path: String,
    value: Option<String>,
    file: PathBuf,
    range: Range,
}

fn flatten(node: &yaml::Node, prefix: &str, file: &PathBuf, out: &mut Vec<Key>) {
    for entry in node.entries() {
        let path = if prefix.is_empty() { entry.key.clone() } else { format!("{prefix}.{}", entry.key) };
        out.push(Key {
            path: path.clone(),
            value: entry.value.as_str().map(str::to_owned),
            file: file.clone(),
            range: entry.range,
        });
        flatten(&entry.value, &path, file, out);
    }
}

impl ConfigKey {
    /// The configuration the string's receiver was made for.
    fn config(cx: &RefContext<'_>) -> Option<String> {
        match cx.site.as_ref()?.call.as_ref()? {
            (CallTarget::Method { tables, .. }, _) => tables.first().cloned(),
            _ => None,
        }
    }

    /// The keys of the configuration `name`.
    fn keys(cx: &RefContext<'_>, name: &str) -> Vec<Key> {
        let project = cx.engine.drupal();
        let mut out = Vec::new();
        for def in project.configs.iter().filter(|def| def.name == name) {
            let Ok(text) = std::fs::read_to_string(&def.file) else { continue };
            flatten(&yaml::parse(&text), "", &def.file, &mut out);
        }
        // Not what Drupal adds to every file.
        out.retain(|key| !key.path.starts_with("_core") && key.path != "langcode");
        // No file installs it: what its schema describes (`node.type.*` for `node.type.article`).
        if out.is_empty() {
            let described = |schema: &&crate::frameworks::drupal::project::Schema| {
                let pattern = &schema.name.name;
                pattern == name || (pattern.contains('*') && crate::summary::matches_pattern(pattern, name))
            };
            for schema in project.schemas.iter().filter(described) {
                out.extend(schema.keys.iter().map(|key| Key {
                    path: key.name.clone(),
                    value: key.detail.clone(),
                    file: key.file.clone(),
                    range: key.range,
                }));
            }
        }
        out
    }

    fn named(cx: &RefContext<'_>, value: &str) -> Vec<Key> {
        let Some(config) = Self::config(cx) else { return Vec::new() };
        Self::keys(cx, &config).into_iter().filter(|key| key.path == value).collect()
    }
}

impl ReferenceKind for ConfigKey {
    fn id(&self) -> &'static str {
        "drupalConfigKey"
    }

    fn patterns(&self) -> &'static [PlacePattern] {
        &PATTERNS
    }

    /// The key with its configuration before it: `shop.settings:currency`.
    fn canonical(&self, cx: &RefContext<'_>, value: &str) -> String {
        match Self::config(cx) {
            Some(config) => format!("{config}:{value}"),
            None => value.to_owned(),
        }
    }

    fn usage_text<'v>(&self, value: &'v str) -> &'v str {
        value.rsplit(':').next().unwrap_or(value)
    }

    fn usage_words(&self, value: &str) -> Vec<String> {
        crate::index::words(self.usage_text(value))
    }

    fn complete(&self, cx: &RefContext<'_>, value: &str, _at: usize) -> Vec<RefItem> {
        let Some(config) = Self::config(cx) else { return Vec::new() };
        Self::keys(cx, &config)
            .into_iter()
            .map(|key| RefItem {
                label: key.path.clone(),
                insert: key.path,
                detail: key.value.or_else(|| Some(config.clone())),
                from: 0,
                to: value.len(),
            })
            .collect()
    }

    fn resolve(&self, cx: &RefContext<'_>, value: &str, _at: usize) -> Vec<Target> {
        Self::named(cx, value).into_iter().map(|key| Target { path: key.file, range: key.range }).collect()
    }

    fn is_known(&self, cx: &RefContext<'_>, value: &str) -> bool {
        let Some(config) = Self::config(cx) else { return true };
        let keys = Self::keys(cx, &config);
        keys.is_empty() || keys.iter().any(|key| key.path == value)
    }

    fn hover(&self, cx: &RefContext<'_>, value: &str) -> Option<String> {
        let config = Self::config(cx)?;
        let key = Self::named(cx, value).into_iter().next()?;
        let root = cx.engine.root();
        let file = key.file.strip_prefix(&root).unwrap_or(&key.file).to_string_lossy().replace('\\', "/");
        let mut out = format!("**configuration key** `{value}` of `{config}`");
        if let Some(value) = &key.value {
            out.push_str(&format!("\n\n`{value}`"));
        }
        out.push_str(&format!("\n\n{file}:{}", key.range.start.line + 1));
        Some(out)
    }
}
