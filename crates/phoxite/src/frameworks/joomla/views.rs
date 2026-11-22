//! What a Joomla template is given, and what a plugin subscribes to.
//!
//! | File | Variables |
//! | --- | --- |
//! | a view's template, `tmpl/<view>/<layout>.php` of a component | `$this` is the view: `<namespace>\View\<View>\HtmlView` (the feed, JSON or raw view when the component has no HTML one), else Joomla's `HtmlView` |
//! | its override in a template, `html/com_x/<view>/<layout>.php` | the same view |
//! | a module's template, `tmpl/*.php`, and its override `html/mod_x/*.php` | `$module` (an object), `$app` (the application), `$input`, `$params` (a registry), `$template` (a string) |
//! | a layout, a file of a `layouts` folder or of a template's `html/layouts` | `$this` is `Joomla\CMS\Layout\FileLayout` |
//! | a site template's `index.php`, `component.php`, `error.php`, `offline.php` | `$this` is `Joomla\CMS\Document\HtmlDocument` |
//!
//! The types come to the template's own inference through
//! [`Declarations::view_variable`](crate::types::Declarations::view_variable).
//!
//! **Subscriptions** ([`subscribed`]): the `'onEvent' => 'handler'` pairs a
//! class's `getSubscribedEvents()` returns, read from its text (a handler
//! written with its priority, `['handler', Priority::HIGH]`, too). A
//! handler's parameter that nothing types is the event class of the name it
//! subscribes to ([`PhpEngine::joomla_handler_event`]).

use std::path::Path;

use super::project::{ExtensionKind, JoomlaProject};
use crate::engine::PhpEngine;
use crate::frameworks::Framework;
use crate::types::Type;

const HTML_VIEW: &str = "Joomla\\CMS\\MVC\\View\\HtmlView";
const FILE_LAYOUT: &str = "Joomla\\CMS\\Layout\\FileLayout";
const HTML_DOCUMENT: &str = "Joomla\\CMS\\Document\\HtmlDocument";
const APPLICATION: &str = "Joomla\\CMS\\Application\\CMSApplicationInterface";
const INPUT: &str = "Joomla\\Input\\Input";
const REGISTRY: &str = "Joomla\\Registry\\Registry";

/// A subscription of a class: an event's name and the method handling it,
/// each with the bytes it is written at.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Subscription {
    pub event: String,
    pub event_at: (usize, usize),
    pub handler: String,
    pub handler_at: (usize, usize),
}

/// The quoted string starting at byte `at` of `text` (after spaces): its
/// content and the bytes of it.
fn quoted(text: &str, at: usize) -> Option<(&str, usize, usize)> {
    let rest = &text[at..];
    let skip = rest.len() - rest.trim_start().len();
    let quote = rest[skip..].chars().next().filter(|c| matches!(c, '\'' | '"'))?;
    let start = at + skip + 1;
    let length = text[start..].find(quote)?;
    Some((&text[start..start + length], start, start + length))
}

/// The subscriptions `getSubscribedEvents()` returns in the class file
/// `text`.
pub fn subscribed(text: &str) -> Vec<Subscription> {
    let mut out = Vec::new();
    let Some(start) = text.find("function getSubscribedEvents") else { return out };
    // To the end of the method: the next one, or the class's.
    let end = text[start + 10..].find("function ").map_or(text.len(), |next| start + 10 + next);
    let body = &text[start..end];
    let mut from = 0;
    while let Some(arrow) = body[from..].find("=>") {
        let arrow = from + arrow;
        from = arrow + 2;
        // The key: the quoted string ending before the arrow.
        let before = body[..arrow].trim_end();
        let Some(quote) = before.chars().last().filter(|c| matches!(c, '\'' | '"')) else { continue };
        let Some(open) = before[..before.len() - 1].rfind(quote) else { continue };
        let event = &before[open + 1..before.len() - 1];
        // The value: a method's name, or a list starting with one.
        let after = &body[arrow + 2..];
        let skip = after.len() - after.trim_start().len();
        let list = after[skip..].starts_with('[');
        let at = arrow + 2 + skip + usize::from(list);
        let Some((handler, handler_start, handler_end)) = quoted(body, at) else { continue };
        let valid = |name: &str| !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
        if valid(event) && valid(handler) {
            out.push(Subscription {
                event: event.to_owned(),
                event_at: (start + open + 1, start + before.len() - 1),
                handler: handler.to_owned(),
                handler_at: (start + handler_start, start + handler_end),
            });
        }
    }
    out
}

/// `featured` is `Featured`.
fn studly(name: &str) -> String {
    let mut chars = name.chars();
    chars.next().map_or_else(String::new, |first| first.to_ascii_uppercase().to_string() + chars.as_str())
}

/// What kind of template a file is.
enum Template {
    /// A view's: the namespace of its extension's folder and the view.
    View(String, String),
    Module,
    Layout,
    Document,
}

fn template(project: &JoomlaProject, path: &Path) -> Option<Template> {
    if path.extension().is_none_or(|ext| ext != "php") {
        return None;
    }
    if project.is_layout(path) {
        return Some(Template::Layout);
    }
    let extension = project.extension_of(path)?;
    let below = path.strip_prefix(&extension.path).ok()?;
    let parts: Vec<String> = below.components().map(|part| part.as_os_str().to_string_lossy().into_owned()).collect();
    let parts: Vec<&str> = parts.iter().map(String::as_str).collect();
    match (extension.kind, parts.as_slice()) {
        (ExtensionKind::Component, ["tmpl", view, _]) => {
            Some(Template::View(extension.namespace.clone().unwrap_or_default(), (*view).to_owned()))
        }
        (ExtensionKind::Module, ["tmpl", _]) => Some(Template::Module),
        (ExtensionKind::Template, [file])
            if matches!(*file, "index.php" | "component.php" | "error.php" | "offline.php") =>
        {
            Some(Template::Document)
        }
        // An override: of a component's view, of a module's template.
        (ExtensionKind::Template, ["html", of, view, _]) if of.starts_with("com_") => {
            let client = extension.client.unwrap_or("Site");
            let overridden = project.extension(of).into_iter().find(|found| found.client == Some(client))?;
            Some(Template::View(overridden.namespace.clone().unwrap_or_default(), (*view).to_owned()))
        }
        (ExtensionKind::Template, ["html", of, _]) if of.starts_with("mod_") => Some(Template::Module),
        _ => None,
    }
}

impl PhpEngine {
    /// The type of the variable `name` of the template at `path`: `None`
    /// when the file isn't a template of the Joomla project, `Some(None)`
    /// when it is and the variable isn't one it is given.
    pub(crate) fn joomla_view_variable(&self, path: &Path, name: &str) -> Option<Option<Type>> {
        let project = self.joomla();
        let template = template(&project, path)?;
        let resolver = self.resolver();
        let known = |class: &str| resolver.class_model(class).map(|model| Type::named(model.fqn()));
        Some(match (template, name) {
            (Template::View(namespace, view), "this") => {
                let base = format!("{namespace}\\View\\{}", studly(&view));
                let found = ["HtmlView", "FeedView", "JsonView", "RawView"]
                    .iter()
                    .find_map(|class| known(&format!("{base}\\{class}")));
                found.or_else(|| known(HTML_VIEW))
            }
            (Template::Layout, "this") => known(FILE_LAYOUT),
            (Template::Document, "this") => known(HTML_DOCUMENT),
            (Template::Module, "module") => Some(Type::Object),
            (Template::Module, "app") => known(APPLICATION),
            (Template::Module, "input") => known(INPUT),
            (Template::Module, "params") => known(REGISTRY),
            (Template::Module, "template") => Some(Type::string()),
            _ => None,
        })
    }

    /// The namespace of the classes of the folder `dir`, where it is below
    /// the source folder of an extension whose manifest names a namespace
    /// (`<namespace path="src">`): the manifest is the PSR-4 root.
    pub(crate) fn joomla_namespace_of_dir(&self, dir: &Path) -> Option<String> {
        if !self.detected_frameworks().contains(Framework::Joomla) {
            return None;
        }
        let project = self.joomla();
        let extension = project.extension_of(dir)?;
        let namespace = extension.namespace.as_deref()?;
        let below = dir.strip_prefix(extension.path.join(&extension.source)).ok()?;
        let mut out = namespace.to_owned();
        for part in below.components() {
            let part = part.as_os_str().to_str()?;
            if part.is_empty() || !part.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                return None;
            }
            out.push('\\');
            out.push_str(part);
        }
        Some(out)
    }

    /// The text of the file declaring the class `class`: the open buffer's,
    /// else the file's.
    pub(crate) fn joomla_class_text(&self, class: &str) -> Option<(std::path::PathBuf, String)> {
        let path = self.class_targets(class).into_iter().next()?.path;
        let text = match self.open_document(&path) {
            Some(document) => document.text.to_string(),
            None => std::fs::read_to_string(&path).ok()?,
        };
        Some((path, text))
    }

    /// The event class of what the method `method` of `class` handles: the
    /// event its `getSubscribedEvents()` gives it, else the event of its
    /// own name.
    pub(crate) fn joomla_handler_event(&self, class: &str, method: &str) -> Option<String> {
        if !self.detected_frameworks().contains(Framework::Joomla) {
            return None;
        }
        let project = self.joomla();
        let subscriptions = self.joomla_class_text(class).map(|(_, text)| subscribed(&text)).unwrap_or_default();
        let event = subscriptions
            .iter()
            .find(|subscription| subscription.handler.eq_ignore_ascii_case(method))
            .map_or(method, |subscription| subscription.event.as_str());
        project.event_class(event).map(str::to_owned)
    }
}

#[cfg(test)]
mod tests {
    use super::subscribed;

    #[test]
    fn a_subscriber_says_what_handles_each_event() {
        let text = "<?php\nclass Stamp {\n    public static function getSubscribedEvents(): array\n    {\n        return [\n            'onContentPrepare' => 'stamp',\n            'onContentAfterSave' => ['saved', Priority::HIGH],\n            \"onError\" => \"failed\",\n        ];\n    }\n\n    public function stamp($event) { $x = ['a' => 'b']; }\n}\n";
        let found: Vec<(String, String)> =
            subscribed(text).into_iter().map(|found| (found.event, found.handler)).collect();
        assert_eq!(
            found,
            [
                ("onContentPrepare".to_owned(), "stamp".to_owned()),
                ("onContentAfterSave".to_owned(), "saved".to_owned()),
                ("onError".to_owned(), "failed".to_owned()),
            ]
        );
        let first = &subscribed(text)[0];
        assert_eq!(&text[first.event_at.0..first.event_at.1], "onContentPrepare");
        assert_eq!(&text[first.handler_at.0..first.handler_at.1], "stamp");
        assert!(subscribed("<?php class A {}").is_empty());
    }
}
