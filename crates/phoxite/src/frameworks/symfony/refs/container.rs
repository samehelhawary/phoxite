//! The `symfonyService` and `symfonyParameter` reference kinds: the service
//! ids and the parameters the project's services files define
//! ([`SymfonyConfig`]), where Symfony's documentation on the service
//! container takes one:
//!
//! - a service id as the first argument of `get()` and `has()` on a PSR or
//!   Symfony container;
//! - a parameter name as the first argument of `getParameter()` on a
//!   controller, of `getParameter()` and `hasParameter()` on a container, and
//!   of `get()` and `has()` on a parameter bag.
//!
//! The declaration is the key in the YAML file. Goto goes there. The usages
//! are the PHP strings and what the YAML files below `config/` write:
//! `@mailer` (and `@?mailer`) for a service, `%app.locale%` for a parameter.
//! A service is also what follows `alias:` and `parent:`. Renaming changes
//! the key and all of those. A service whose id is a class name isn't
//! renamed here (that is the class's rename).
//!
//! A service whose id is its class name needs no definition (autowiring),
//! so an id that isn't defined is not an error.
//!
//! [`SymfonyConfig`]: crate::symfony::SymfonyConfig

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::base::{Pos, Range};
use crate::frameworks::refs::{CalleePattern, PlacePattern, RefContext, RefItem, ReferenceKind, RenameTarget};
use crate::navigate::Target;
use crate::symfony::SymfonyProject;

const PSR_CONTAINER: &str = "Psr\\Container\\ContainerInterface";
const SYMFONY_CONTAINER: &str = "Symfony\\Component\\DependencyInjection\\ContainerInterface";
const ABSTRACT_CONTROLLER: &str = "Symfony\\Bundle\\FrameworkBundle\\Controller\\AbstractController";
const PARAMETER_BAG: &str = "Symfony\\Component\\DependencyInjection\\ParameterBag\\ParameterBagInterface";

const SERVICE: CalleePattern =
    CalleePattern::Method { classes: &[PSR_CONTAINER, SYMFONY_CONTAINER], names: &["get", "has"] };
const CONTROLLER_PARAMETER: CalleePattern =
    CalleePattern::Method { classes: &[ABSTRACT_CONTROLLER], names: &["getParameter"] };
const CONTAINER_PARAMETER: CalleePattern =
    CalleePattern::Method { classes: &[SYMFONY_CONTAINER], names: &["getParameter", "hasParameter"] };
const BAG_PARAMETER: CalleePattern = CalleePattern::Method { classes: &[PARAMETER_BAG], names: &["get", "has"] };

/// `#[Autowire(service: 'mailer')]`, `#[Autowire(param: 'app.locale')]`: the
/// second and the fifth argument of the attribute.
const AUTOWIRE: CalleePattern = CalleePattern::Method {
    classes: &["Symfony\\Component\\DependencyInjection\\Attribute\\Autowire"],
    names: &["__construct"],
};

static SERVICE_PATTERNS: [PlacePattern; 2] =
    [PlacePattern::argument(SERVICE, 0, "id"), PlacePattern::argument(AUTOWIRE, 1, "service")];
static PARAMETER_PATTERNS: [PlacePattern; 4] = [
    PlacePattern::argument(CONTROLLER_PARAMETER, 0, "name"),
    PlacePattern::argument(CONTAINER_PARAMETER, 0, "name"),
    PlacePattern::argument(BAG_PARAMETER, 0, "name"),
    PlacePattern::argument(AUTOWIRE, 4, "param"),
];

fn project(cx: &RefContext<'_>) -> Arc<SymfonyProject> {
    cx.engine.resolver().symfony()
}

/// The most YAML files looked into for usages.
const MAX_FILES: usize = 256;

/// The configuration files below `dir`, sorted: YAML, and PHP (a
/// `services.php` returning an array writes `'@mailer'` and `'%app.locale%'`
/// as YAML does).
fn yaml_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).collect();
    entries.sort();
    for path in entries {
        if out.len() >= MAX_FILES {
            return;
        }
        if path.is_dir() {
            yaml_files(&path, out);
        } else if matches!(path.extension().and_then(|e| e.to_str()), Some("yaml" | "yml" | "php")) {
            out.push(path);
        }
    }
}

/// Whether `c` can be part of a service id or a parameter name.
fn is_name(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '\\' | '-')
}

/// Where the YAML files below the project's `config/` write `name` between
/// `open` and `close` (`@mailer`, `%app.locale%`): the file and the range of
/// the name. With `close` empty the name ends where what follows it is no
/// part of a name.
fn yaml_usages(cx: &RefContext<'_>, open: &[&str], name: &str, close: &str) -> Vec<(PathBuf, Range)> {
    let mut files = Vec::new();
    yaml_files(&cx.engine.root().join("config"), &mut files);
    let mut out = Vec::new();
    for file in files {
        let Ok(text) = std::fs::read_to_string(&file) else { continue };
        if !text.contains(name) {
            continue;
        }
        for (number, line) in text.lines().enumerate() {
            let mut from = 0;
            while let Some(found) = line[from..].find(name) {
                let start = from + found;
                let end = start + name.len();
                from = end;
                // After `@` or `%`; a key (`alias:`) is followed by the bare name, quoted or not.
                let bare = line[..start].trim_end_matches(['\'', '"']).trim_end();
                // In PHP a key is `'alias' => 'name'`.
                let php_key = |key: &str| {
                    let key = key.trim_end_matches(':');
                    bare.strip_suffix("=>").map(str::trim_end).is_some_and(|before| {
                        before.strip_suffix(['\'', '"']).is_some_and(|before| before.ends_with(key))
                    })
                };
                let opened = open.iter().any(|open| {
                    if open.ends_with(':') {
                        bare.ends_with(open) || php_key(open)
                    } else {
                        line[..start].ends_with(open)
                    }
                });
                let closed =
                    if close.is_empty() { !line[end..].starts_with(is_name) } else { line[end..].starts_with(close) };
                // `@@` is an escaped `@`, not a service.
                if !opened || !closed || line[..start].ends_with("@@") {
                    continue;
                }
                let column = |byte: usize| line[..byte].encode_utf16().count() as u32;
                let range = Range::new(Pos::new(number as u32, column(start)), Pos::new(number as u32, column(end)));
                out.push((file.clone(), range));
            }
        }
    }
    out
}

/// Whether `name` can be a service id or a parameter name that isn't a class name.
fn valid(name: &str) -> bool {
    !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
}

/// See the module documentation.
pub struct SymfonyService;

impl ReferenceKind for SymfonyService {
    fn id(&self) -> &'static str {
        "symfonyService"
    }

    fn other_usages(&self, cx: &RefContext<'_>, key: &str) -> Vec<(PathBuf, Range)> {
        yaml_usages(cx, &["@", "@?", "alias:", "parent:"], key, "")
    }

    fn rename_target(&self, cx: &RefContext<'_>, value: &str, _at: usize) -> Option<RenameTarget> {
        let project = project(cx);
        if crate::symfony::is_class_id(value) {
            return None;
        }
        let declarations: Vec<(PathBuf, Range)> = project
            .config
            .services
            .iter()
            .filter(|service| service.id == value)
            .map(|service| (service.file.clone(), service.range))
            .collect();
        (!declarations.is_empty()).then(|| RenameTarget {
            key: value.to_owned(),
            range: (0, value.len()),
            declarations,
        })
    }

    fn valid_name(&self, name: &str) -> bool {
        valid(name)
    }

    fn patterns(&self) -> &'static [PlacePattern] {
        &SERVICE_PATTERNS
    }

    fn complete(&self, cx: &RefContext<'_>, value: &str, _at: usize) -> Vec<RefItem> {
        let project = project(cx);
        let mut items: Vec<RefItem> = Vec::new();
        // The last definition of an id is the one that counts.
        for service in project.config.services.iter().rev() {
            if !items.iter().any(|known| known.label == service.id) {
                items.push(RefItem {
                    label: service.id.clone(),
                    insert: service.id.clone(),
                    detail: project.config.service_class(&service.id),
                    from: 0,
                    to: value.len(),
                });
            }
        }
        items.reverse();
        items
    }

    fn resolve(&self, cx: &RefContext<'_>, value: &str, _at: usize) -> Vec<Target> {
        let project = project(cx);
        let service = project.config.service(value);
        service.map(|service| Target { path: service.file.clone(), range: service.range }).into_iter().collect()
    }

    fn is_known(&self, _cx: &RefContext<'_>, _value: &str) -> bool {
        // Autowiring defines the services whose id is their class.
        true
    }

    fn hover(&self, cx: &RefContext<'_>, value: &str) -> Option<String> {
        let project = project(cx);
        project.config.service(value)?;
        Some(match project.config.service_class(value) {
            Some(class) => format!("**service** `{value}`\n\n`{class}`"),
            None => format!("**service** `{value}`"),
        })
    }
}

/// See the module documentation.
pub struct SymfonyParameter;

impl ReferenceKind for SymfonyParameter {
    fn id(&self) -> &'static str {
        "symfonyParameter"
    }

    fn other_usages(&self, cx: &RefContext<'_>, key: &str) -> Vec<(PathBuf, Range)> {
        yaml_usages(cx, &["%"], key, "%")
    }

    fn rename_target(&self, cx: &RefContext<'_>, value: &str, _at: usize) -> Option<RenameTarget> {
        let project = project(cx);
        let declarations: Vec<(PathBuf, Range)> = project
            .config
            .parameters
            .iter()
            .filter(|parameter| parameter.name == value)
            .map(|parameter| (parameter.file.clone(), parameter.range))
            .collect();
        (!declarations.is_empty()).then(|| RenameTarget {
            key: value.to_owned(),
            range: (0, value.len()),
            declarations,
        })
    }

    fn valid_name(&self, name: &str) -> bool {
        valid(name)
    }

    fn patterns(&self) -> &'static [PlacePattern] {
        &PARAMETER_PATTERNS
    }

    fn complete(&self, cx: &RefContext<'_>, value: &str, _at: usize) -> Vec<RefItem> {
        let project = project(cx);
        let mut items: Vec<RefItem> = Vec::new();
        for parameter in project.config.parameters.iter().rev() {
            if !items.iter().any(|known| known.label == parameter.name) {
                items.push(RefItem {
                    label: parameter.name.clone(),
                    insert: parameter.name.clone(),
                    detail: parameter.value.clone(),
                    from: 0,
                    to: value.len(),
                });
            }
        }
        items.reverse();
        items
    }

    fn resolve(&self, cx: &RefContext<'_>, value: &str, _at: usize) -> Vec<Target> {
        let project = project(cx);
        let parameter = project.config.parameter(value);
        parameter.map(|found| Target { path: found.file.clone(), range: found.range }).into_iter().collect()
    }

    fn is_known(&self, _cx: &RefContext<'_>, _value: &str) -> bool {
        // The kernel and the bundles define parameters no file of the project has.
        true
    }

    fn hover(&self, cx: &RefContext<'_>, value: &str) -> Option<String> {
        let project = project(cx);
        let parameter = project.config.parameter(value)?;
        Some(match &parameter.value {
            Some(text) => format!("**parameter** `{value}`\n\n`{text}`"),
            None => format!("**parameter** `{value}`"),
        })
    }
}
