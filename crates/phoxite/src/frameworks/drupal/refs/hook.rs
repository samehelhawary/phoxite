//! The `drupalHook` reference kind: the name of a hook, which the function
//! `hook_<name>()` of an `*.api.php` file documents.
//!
//! A string names one as the first argument of the module handler's
//! `invokeAll()`, `invokeAllWith()` and `hasImplementations()`, the second
//! of its `invoke()`, and of the `#[Hook('…')]` attribute. `alter('form')`
//! on the module handler or the theme manager names `hook_form_alter()`.
//! Drupal 7's `module_invoke_all()`, `module_implements()`,
//! `module_invoke()` and `drupal_alter()` are the same.
//!
//! A hook documented with a placeholder (`hook_ENTITY_TYPE_presave`,
//! `hook_form_FORM_ID_alter`) is found from a name that fits it
//! (`node_presave`).
//!
//! Goto goes to the hook's function, completion offers the hooks, and Find
//! References lists the strings naming the hook and the functions
//! implementing it: `<module>_<name>()` for each module, theme and profile
//! of the project. A hook isn't renamed here.

use std::path::{Path, PathBuf};

use crate::base::Range;
use crate::frameworks::refs::{CallTarget, CalleePattern, PlacePattern, Position, RefContext, RefItem, ReferenceKind};
use crate::names::NameKind;
use crate::navigate::Target;
use crate::summary::FileSummary;

const MODULE_HANDLER: &[&str] = &["Drupal\\Core\\Extension\\ModuleHandlerInterface"];
const ALTERING: &[&str] =
    &["Drupal\\Core\\Extension\\ModuleHandlerInterface", "Drupal\\Core\\Theme\\ThemeManagerInterface"];
const ALTER: &[&str] = &["alter", "alterDeprecated"];

const fn method(
    classes: &'static [&'static str],
    names: &'static [&'static str],
    index: usize,
    name: &'static str,
) -> PlacePattern {
    PlacePattern::argument(CalleePattern::Method { classes, names }, index, name)
}

const fn function(name: &'static str, index: usize, parameter: &'static str) -> PlacePattern {
    PlacePattern::argument(CalleePattern::Function(name), index, parameter)
}

static PATTERNS: [PlacePattern; 11] = [
    // Drupal 7's functions.
    function("module_invoke_all", 0, "hook"),
    function("module_implements", 0, "hook"),
    function("module_invoke", 1, "hook"),
    function("module_hook", 1, "hook"),
    function("drupal_alter", 0, "type"),
    function("drupal_alter", 0, "type").at(Position::ListValue),
    method(MODULE_HANDLER, &["invokeAll", "invokeAllWith", "hasImplementations", "invokeAllDeprecated"], 0, "hook"),
    method(MODULE_HANDLER, &["invoke", "invokeDeprecated"], 1, "hook"),
    method(ALTERING, ALTER, 0, "type"),
    method(ALTERING, ALTER, 0, "type").at(Position::ListValue),
    method(&["Drupal\\Core\\Hook\\Attribute\\Hook"], &["__construct"], 0, "hook"),
];

/// See the module documentation.
pub struct DrupalHook;

impl DrupalHook {
    /// Whether the string's call adds `_alter` to what it is given.
    fn alters(cx: &RefContext<'_>) -> bool {
        let call = cx.site.as_ref().and_then(|site| site.call.as_ref());
        match call {
            Some((CallTarget::Method { name, .. }, _)) => ALTER.iter().any(|known| known.eq_ignore_ascii_case(name)),
            Some((CallTarget::Function(names), _)) => names.iter().any(|name| name.ends_with("drupal_alter")),
            _ => false,
        }
    }

    fn function(cx: &RefContext<'_>, fqn: &str) -> Vec<Target> {
        cx.engine.declarations_of(NameKind::Function, fqn, Path::new(""), &FileSummary::default())
    }
}

impl ReferenceKind for DrupalHook {
    fn id(&self) -> &'static str {
        "drupalHook"
    }

    fn patterns(&self) -> &'static [PlacePattern] {
        &PATTERNS
    }

    /// The hook's name: with the `_alter` its call adds.
    fn canonical(&self, cx: &RefContext<'_>, value: &str) -> String {
        if Self::alters(cx) { format!("{value}_alter") } else { value.to_owned() }
    }

    /// `alter('form')` doesn't write `form_alter`.
    fn usage_text<'v>(&self, value: &'v str) -> &'v str {
        value.strip_suffix("_alter").unwrap_or(value)
    }

    fn usage_words(&self, value: &str) -> Vec<String> {
        crate::index::words(self.usage_text(value))
    }

    fn complete(&self, cx: &RefContext<'_>, value: &str, _at: usize) -> Vec<RefItem> {
        let alters = Self::alters(cx);
        let hooks = cx.engine.resolver().drupal_hooks();
        hooks
            .into_iter()
            .filter_map(|(name, fqn)| {
                // A hook with a placeholder is written out by whoever names it.
                if crate::types::resolve::hook_pattern(&name).is_some() {
                    return None;
                }
                let label = if alters { name.strip_suffix("_alter")?.to_owned() } else { name };
                Some(RefItem {
                    label: label.clone(),
                    insert: label,
                    detail: Some(format!("{fqn}()")),
                    from: 0,
                    to: value.len(),
                })
            })
            .collect()
    }

    fn resolve(&self, cx: &RefContext<'_>, value: &str, _at: usize) -> Vec<Target> {
        let name = self.canonical(cx, value);
        let Some(fqn) = cx.engine.resolver().drupal_hook(&name) else { return Vec::new() };
        Self::function(cx, &fqn)
    }

    /// The functions implementing the hook `key`.
    fn other_usages(&self, cx: &RefContext<'_>, key: &str) -> Vec<(PathBuf, Range)> {
        let project = cx.engine.drupal();
        let mut out: Vec<(PathBuf, Range)> = Vec::new();
        for extension in &project.extensions {
            for target in Self::function(cx, &format!("{}_{key}", extension.name)) {
                if !out.contains(&(target.path.clone(), target.range)) {
                    out.push((target.path, target.range));
                }
            }
        }
        out
    }

    fn is_known(&self, cx: &RefContext<'_>, value: &str) -> bool {
        let resolver = cx.engine.resolver();
        resolver.drupal_hooks().is_empty() || resolver.drupal_hook(&self.canonical(cx, value)).is_some()
    }

    fn hover(&self, cx: &RefContext<'_>, value: &str) -> Option<String> {
        let name = self.canonical(cx, value);
        let fqn = cx.engine.resolver().drupal_hook(&name)?;
        let target = Self::function(cx, &fqn).into_iter().next()?;
        let root = cx.engine.root();
        let file = target.path.strip_prefix(&root).unwrap_or(&target.path).to_string_lossy().replace('\\', "/");
        Some(format!("**hook** `{name}`\n\n`{fqn}()` in {file}:{}", target.range.start.line + 1))
    }
}
