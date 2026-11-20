//! `PhpUndefinedRoute`: a route name no route defines, where a route name is
//! expected (the route-name string references of the project's framework).
//!
//! In a Laravel project, the route names of `route()`, `to_route()`,
//! `redirect()->route()`, `URL::route()` and `Route::has()` (the `routeName`
//! string references) are checked against the routes of its route files.
//!
//! In a Symfony project, the first argument of `path()`, `url()`,
//! `generateUrl()`, `redirectToRoute()` and a URL generator's `generate()`
//! (the `symfonyRoute` string references) is checked against
//! `config/routes*.yaml` and the `#[Route]` attributes.
//!
//! Only string literals are checked, and only while the indexes are ready
//! and the project names some route (a project whose routes come from
//! somewhere the engine doesn't read reports nothing).

use crate::base::Severity;

use super::{Inspection, InspectionContext, Problems};
use crate::engine::{EngineState, is_meta_path};
use crate::frameworks::laravel::RouteTable;
use crate::frameworks::laravel::refs::{RouteName, classify, sites_with};
use crate::frameworks::refs::{ReferenceKind, prefixed};
use crate::frameworks::symfony::refs::SymfonyRoute;

/// `PhpUndefinedRoute`.
pub struct UndefinedRoute;

impl Inspection for UndefinedRoute {
    fn id(&self) -> &'static str {
        "PhpUndefinedRoute"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, cx: &InspectionContext<'_>, out: &mut Problems) {
        if cx.engine.status().state != EngineState::Ready || is_meta_path(cx.path) {
            return;
        }
        if let Some(routes) = cx.engine.laravel_routes() {
            laravel(cx, &routes, out);
            return;
        }
        // The route names of the other frameworks: what their kinds say.
        for found in cx.engine.undefined_references(cx.parsed, cx.path, &[&SymfonyRoute], cx.cancel) {
            out.report(found.start, found.end, found.problem.message);
        }
    }
}

/// In a Laravel project: the `routeName` string references
/// ([`RouteName`]: `route('…')`, `to_route('…')`, `redirect()->route('…')`, …)
/// whose route no route file names. Nothing while the project has no named
/// route (its routes come from somewhere the engine doesn't read).
fn laravel(cx: &InspectionContext<'_>, routes: &RouteTable, out: &mut Problems) {
    for (start, end, message) in undefined_laravel_routes(cx.engine, cx.parsed, routes, cx.cancel) {
        out.report(start, end, message);
    }
}

/// The route names of `parsed` (a PHP file, or the PHP a Blade template
/// compiles to) that no route of `routes` has, as `(start, end, message)`.
pub(crate) fn undefined_laravel_routes(
    engine: &crate::engine::PhpEngine,
    parsed: &crate::syntax::ParsedFile,
    routes: &RouteTable,
    cancel: &crate::base::CancellationToken,
) -> Vec<(u32, u32, String)> {
    let mut out = Vec::new();
    if !routes.routes().iter().any(|route| route.name.is_some()) {
        return out;
    }
    let resolver = engine.resolver();
    // The places of the `ide.json` rules too.
    let project = engine.laravel();
    let rules = project.as_ref().map(|project| project.ide_json.completions.as_slice()).unwrap_or_default();
    for site in sites_with(parsed, &resolver, &[&RouteName], rules, None) {
        if cancel.is_cancelled() {
            break;
        }
        // Only plain literals (no interpolation or escapes).
        if site.value.is_empty() || site.value.contains(['$', '\\', '{']) {
            continue;
        }
        let Some((kind, rule)) = classify(&site, &resolver, rules) else { continue };
        let value = prefixed(&site, rule);
        if kind.id() == RouteName.id() && routes.named(&value).next().is_none() {
            out.push((site.start, site.end, format!("Route '{value}' is not defined")));
        }
    }
    out
}
