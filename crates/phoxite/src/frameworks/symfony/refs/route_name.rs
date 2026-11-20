//! The `symfonyRoute` reference kind: the names of the project's routes
//! ([`SymfonyProject::routes`]: `config/routes*.yaml` and the `#[Route]`
//! attributes) where Symfony's documentation on routing and URL generation
//! takes one: the first argument of `generateUrl()` and `redirectToRoute()`
//! on a controller, of `generate()` on a URL generator, and of Twig's
//! `path()` and `url()` when they are called from PHP. The templates naming a
//! route in `path()` and `url()` are usages too, and are renamed with it.
//!
//! The declaration is the route's key in a YAML routes file, or the `name`
//! of its `#[Route]` attribute, whose class-level `#[Route]` name prefix
//! makes the full name. Goto goes there; renaming changes the part of the
//! name written there, in the declaration and in every usage.
//!
//! While the project names no route at all (its routes come from somewhere
//! the engine doesn't read), every name is taken as known.
//!
//! [`SymfonyProject::routes`]: crate::symfony::SymfonyProject

use std::path::Path;
use std::sync::Arc;

use crate::frameworks::refs::{
    CalleePattern, Declared, PlacePattern, RefContext, RefItem, ReferenceKind, RenameTarget, Undefined,
};
use crate::navigate::Target;
use crate::symfony::{RouteDef, SymfonyProject};
use crate::syntax::ParsedFile;

const ABSTRACT_CONTROLLER: &str = "Symfony\\Bundle\\FrameworkBundle\\Controller\\AbstractController";
const URL_GENERATOR: &str = "Symfony\\Component\\Routing\\Generator\\UrlGeneratorInterface";

const CONTROLLER_ROUTE: CalleePattern =
    CalleePattern::Method { classes: &[ABSTRACT_CONTROLLER], names: &["generateUrl", "redirectToRoute"] };
const GENERATE: CalleePattern = CalleePattern::Method { classes: &[URL_GENERATOR], names: &["generate"] };

static PATTERNS: [PlacePattern; 4] = [
    PlacePattern::argument(CONTROLLER_ROUTE, 0, "route"),
    PlacePattern::argument(GENERATE, 0, "name"),
    PlacePattern::argument(CalleePattern::UndeclaredFunction("path"), 0, "name"),
    PlacePattern::argument(CalleePattern::UndeclaredFunction("url"), 0, "name"),
];

/// See the module documentation.
pub struct SymfonyRoute;

impl SymfonyRoute {
    fn project(cx: &RefContext<'_>) -> Arc<SymfonyProject> {
        cx.engine.resolver().symfony()
    }

    /// The routes named `name`.
    fn named<'p>(project: &'p SymfonyProject, name: &'p str) -> impl Iterator<Item = &'p RouteDef> {
        project.routes.iter().filter(move |route| route.name == name)
    }

    /// How many bytes of a route's name are written at its declaration (a
    /// `#[Route]` on the class may put a prefix before it); `None` when
    /// that can't be told (the range isn't on one line, or the name isn't
    /// ASCII).
    fn written(route: &RouteDef) -> Option<usize> {
        if route.range.start.line != route.range.end.line || !route.name.is_ascii() {
            return None;
        }
        let width = route.range.end.col.checked_sub(route.range.start.col)? as usize;
        (width > 0 && width <= route.name.len()).then_some(width)
    }
}

impl ReferenceKind for SymfonyRoute {
    fn id(&self) -> &'static str {
        "symfonyRoute"
    }

    fn patterns(&self) -> &'static [PlacePattern] {
        &PATTERNS
    }

    fn complete(&self, cx: &RefContext<'_>, value: &str, _at: usize) -> Vec<RefItem> {
        let project = Self::project(cx);
        let mut items: Vec<RefItem> = Vec::new();
        for route in &project.routes {
            if !items.iter().any(|known| known.label == route.name) {
                items.push(RefItem {
                    label: route.name.clone(),
                    insert: route.name.clone(),
                    detail: route.path.clone(),
                    from: 0,
                    to: value.len(),
                });
            }
        }
        items
    }

    fn completes_unquoted(&self) -> bool {
        true
    }

    fn resolve(&self, cx: &RefContext<'_>, value: &str, _at: usize) -> Vec<Target> {
        let project = Self::project(cx);
        let mut out: Vec<Target> = Vec::new();
        for route in Self::named(&project, value) {
            let Some(file) = &route.file else { continue };
            let target = Target { path: file.clone(), range: route.range };
            if !out.contains(&target) {
                out.push(target);
            }
        }
        out
    }

    /// The templates writing the name in `path()` and `url()`.
    fn other_usages(&self, cx: &RefContext<'_>, key: &str) -> Vec<(std::path::PathBuf, crate::base::Range)> {
        let paths = cx.engine.twig_paths();
        let mut out = Vec::new();
        for template in paths.names() {
            let Some(file) = paths.resolve(&template) else { continue };
            let Some(text) = cx.engine.template_text(&file).filter(|text| text.contains(key)) else { continue };
            for found in crate::twig::route_refs(&text) {
                if found.name == key {
                    out.push((file.clone(), found.range));
                }
            }
        }
        out
    }

    fn declared_at(&self, cx: &RefContext<'_>, path: &Path, parsed: &ParsedFile, offset: u32) -> Option<Declared> {
        let project = Self::project(cx);
        let lines = parsed.line_index();
        project.routes.iter().find_map(|route| {
            if route.file.as_deref() != Some(path) {
                return None;
            }
            let (start, end) = lines.byte_range(route.range);
            if offset < start || offset > end {
                return None;
            }
            let written = Self::written(route)?;
            let at = route.name.len() - written + (offset - start) as usize;
            Some(Declared { value: route.name.clone(), start, end, at })
        })
    }

    fn rename_target(&self, cx: &RefContext<'_>, value: &str, _at: usize) -> Option<RenameTarget> {
        let project = Self::project(cx);
        let mut written: Option<usize> = None;
        let mut declarations = Vec::new();
        for route in Self::named(&project, value) {
            let file = route.file.clone()?;
            let width = Self::written(route)?;
            // Declarations of one name that write different parts of it
            // can't be renamed together.
            if written.is_some_and(|known| known != width) {
                return None;
            }
            written = Some(width);
            if !declarations.contains(&(file.clone(), route.range)) {
                declarations.push((file, route.range));
            }
        }
        let written = written?;
        Some(RenameTarget { key: value.to_owned(), range: (value.len() - written, value.len()), declarations })
    }

    fn valid_name(&self, name: &str) -> bool {
        !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-' | ':'))
    }

    fn is_known(&self, cx: &RefContext<'_>, value: &str) -> bool {
        let project = Self::project(cx);
        project.routes.is_empty() || project.route(value).is_some()
    }

    fn hover(&self, cx: &RefContext<'_>, value: &str) -> Option<String> {
        let project = Self::project(cx);
        let root = cx.engine.root();
        let sections: Vec<String> = Self::named(&project, value)
            .map(|route| {
                let mut out = match &route.path {
                    Some(path) => format!("`{path}`"),
                    None => String::new(),
                };
                if let Some(file) = &route.file {
                    let file = file.strip_prefix(&root).unwrap_or(file).to_string_lossy().replace('\\', "/");
                    if !out.is_empty() {
                        out.push_str(" · ");
                    }
                    out.push_str(&file);
                }
                out
            })
            .filter(|section| !section.is_empty())
            .collect();
        Self::named(&project, value).next()?;
        Some(format!("**route** `{value}`\n\n{}", sections.join("\n\n")))
    }

    fn undefined(&self, cx: &RefContext<'_>, value: &str) -> Option<Undefined> {
        (!self.is_known(cx, value)).then(|| Undefined::warning(format!("Route '{value}' is not defined")))
    }
}
