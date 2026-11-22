//! Plugins: a class a `di.xml` registers on a type
//! (`<type name="Target"><plugin type="Plugin"/></type>`) intercepts the
//! type's public methods with methods named after them: `beforeSave()`,
//! `afterSave()` and `aroundSave()` are about `save()`.
//!
//! - **Goto** on the name of such a method, where it is declared, goes to
//!   the method it intercepts ([`PhpEngine::magento_intercepted`]), in each
//!   type the plugin is registered on.
//! - **`PhpUndefinedMagentoPluginMethod`** reports one whose types have no
//!   such method ([`PhpEngine::magento_plugin_problems`]). A method that
//!   doesn't start with one of the three words is the plugin's own, and so
//!   is one when a type isn't a class the project has.
//!
//! A virtual type a plugin is registered on is the class it is of.

use std::path::Path;

use super::project::MagentoProject;
use crate::engine::PhpEngine;
use crate::frameworks::Framework;
use crate::navigate::{Target, Usage, UsageKind};
use crate::summary::FileSummary;

/// The method a plugin's method `name` is about: `afterGetName` is
/// `getName`.
pub fn intercepted(name: &str) -> Option<String> {
    let rest = ["before", "after", "around"].into_iter().find_map(|word| name.strip_prefix(word))?;
    let mut chars = rest.chars();
    let first = chars.next().filter(|first| first.is_ascii_uppercase())?;
    Some(first.to_ascii_lowercase().to_string() + chars.as_str())
}

impl MagentoProject {
    /// The classes the plugin `class` is registered on.
    pub fn plugin_targets(&self, class: &str) -> Vec<String> {
        let class = class.trim_start_matches('\\');
        let mut out: Vec<String> = Vec::new();
        for plugin in self.plugins.iter().filter(|plugin| plugin.class.eq_ignore_ascii_case(class)) {
            let target = self.virtual_class(&plugin.target).unwrap_or_else(|| plugin.target.clone());
            if !out.contains(&target) {
                out.push(target);
            }
        }
        out
    }
}

impl PhpEngine {
    /// Where the method the plugin method `method` of `class` intercepts is
    /// declared. `None` when `class` isn't a plugin or the method isn't one
    /// of a plugin's.
    pub(crate) fn magento_intercepted(&self, class: &str, method: &str) -> Option<Vec<Target>> {
        if !self.detected_frameworks().contains(Framework::Magento) {
            return None;
        }
        let name = intercepted(method)?;
        let targets = self.magento().plugin_targets(class);
        if targets.is_empty() {
            return None;
        }
        let resolver = self.resolver();
        let mut out: Vec<Target> = Vec::new();
        for target in targets {
            let Some(model) = resolver.class_model(&target) else { continue };
            let Some(found) = model.method(&name) else { continue };
            let Some(declared) = found.declared() else { continue };
            let Some(path) = self.class_path(&found.declaring, Path::new(""), &FileSummary::default()) else {
                continue;
            };
            let target = Target { path, range: declared.name_loc.range };
            if !out.contains(&target) {
                out.push(target);
            }
        }
        Some(out)
    }

    /// What intercepts or names the method `method` declared in `class`:
    /// the `before`, `after` and `around` methods of the plugins registered
    /// on the class, on a class below it or on one above it, and the XML
    /// naming the method (`webapi.xml`, `crontab.xml`, a queue's handler).
    pub(crate) fn magento_method_usages(&self, class: &str, method: &str) -> Vec<Usage> {
        let mut out: Vec<Usage> = Vec::new();
        if !self.detected_frameworks().contains(Framework::Magento) {
            return out;
        }
        let project = self.magento();
        let resolver = self.resolver();
        let related = |found: &str| {
            found.eq_ignore_ascii_case(class)
                || resolver.class_model(found).is_some_and(|model| model.is_subclass_of(class))
                || resolver.class_model(class).is_some_and(|model| model.is_subclass_of(found))
        };
        let mut studly = method.to_owned();
        if let Some(first) = studly.get_mut(..1) {
            first.make_ascii_uppercase();
        }
        let mut plugins: Vec<&str> = Vec::new();
        for plugin in &project.plugins {
            let target = project.virtual_class(&plugin.target).unwrap_or_else(|| plugin.target.clone());
            if related(&target) && !plugins.contains(&plugin.class.as_str()) {
                plugins.push(&plugin.class);
            }
        }
        for plugin in plugins {
            let Some(model) = resolver.class_model(plugin) else { continue };
            for word in ["before", "after", "around"] {
                let Some(found) = model.method(&format!("{word}{studly}")) else { continue };
                let Some(declared) = found.declared() else { continue };
                let Some(path) = self.class_path(&found.declaring, Path::new(""), &FileSummary::default()) else {
                    continue;
                };
                out.push(Usage { path, range: declared.name_loc.range, kind: UsageKind::Code });
            }
        }
        for (file, mention) in project.mentions() {
            if let Some((named, range)) = &mention.method
                && named.eq_ignore_ascii_case(method)
                && related(&mention.name)
            {
                out.push(Usage { path: file.file.clone(), range: *range, kind: UsageKind::String });
            }
        }
        out
    }

    /// The plugins on the method `method` of `class`, for its hover: each
    /// as `Plugin::beforeMethod()`.
    pub(crate) fn magento_plugins_of(&self, class: &str, method: &str) -> Vec<String> {
        let resolver = self.resolver();
        let usages = self.magento_method_usages(class, method);
        let mut out: Vec<String> = Vec::new();
        if usages.iter().all(|usage| usage.kind != UsageKind::Code) {
            return out;
        }
        let project = self.magento();
        let mut studly = method.to_owned();
        if let Some(first) = studly.get_mut(..1) {
            first.make_ascii_uppercase();
        }
        for plugin in &project.plugins {
            let Some(model) = resolver.class_model(&plugin.class) else { continue };
            let target = project.virtual_class(&plugin.target).unwrap_or_else(|| plugin.target.clone());
            let related = target.eq_ignore_ascii_case(class)
                || resolver.class_model(&target).is_some_and(|model| model.is_subclass_of(class))
                || resolver.class_model(class).is_some_and(|model| model.is_subclass_of(&target));
            if !related {
                continue;
            }
            for word in ["before", "after", "around"] {
                let name = format!("{word}{studly}");
                if model.method(&name).is_some() {
                    let short = plugin.class.rsplit('\\').next().unwrap_or(&plugin.class);
                    let shown = format!("{short}::{name}()");
                    if !out.contains(&shown) {
                        out.push(shown);
                    }
                }
            }
        }
        out
    }

    /// The plugin methods of `summary` that intercept nothing, as
    /// `(start, end, message)`.
    pub(crate) fn magento_plugin_problems(&self, summary: &FileSummary) -> Vec<(u32, u32, String)> {
        let mut out = Vec::new();
        if !self.detected_frameworks().contains(Framework::Magento) {
            return out;
        }
        let project = self.magento();
        if project.plugins.is_empty() {
            return out;
        }
        let resolver = self.resolver();
        for class in &summary.class_likes {
            let targets = project.plugin_targets(&class.fqn);
            // Every type must be a class the project has, to say what it lacks.
            let models: Vec<_> = targets.iter().filter_map(|target| resolver.class_model(target)).collect();
            if models.is_empty() || models.len() != targets.len() {
                continue;
            }
            for method in &class.methods {
                let Some(name) = intercepted(&method.name) else { continue };
                if models.iter().any(|model| model.method(&name).is_some()) {
                    continue;
                }
                let short = targets[0].rsplit('\\').next().unwrap_or(&targets[0]);
                out.push((
                    method.name_loc.start,
                    method.name_loc.end,
                    format!("'{short}' has no method '{name}' for this plugin to intercept"),
                ));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::intercepted;

    #[test]
    fn a_plugin_method_is_named_after_the_method_it_intercepts() {
        assert_eq!(intercepted("beforeSave").as_deref(), Some("save"));
        assert_eq!(intercepted("afterGetName").as_deref(), Some("getName"));
        assert_eq!(intercepted("aroundExecute").as_deref(), Some("execute"));
        assert_eq!(intercepted("before"), None);
        assert_eq!(intercepted("afterwards"), None);
        assert_eq!(intercepted("save"), None);
    }
}
