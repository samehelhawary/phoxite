//! The `wordpressRestRoute` reference kind: the path of a REST route,
//! `shop/v1/books`, which `register_rest_route('shop/v1', '/books', …)`
//! declares as a namespace and a route.
//!
//! PHP names one in `rest_url('shop/v1/books')`, `get_rest_url()`, `new
//! WP_REST_Request('GET', '/shop/v1/books')` and `rest_do_request()`; a
//! script in `apiFetch({ path: '/shop/v1/books' })` or the end of a URL
//! ([`assets`](crate::frameworks::wordpress::assets)). A slash before the
//! path and a query after it aren't part of it.
//!
//! A route with a pattern in it (`/books/(?P<id>\d+)`) is declared as
//! `shop/v1/books/*`: `/shop/v1/books/12` goes to it, and completion
//! doesn't offer it.
//!
//! Goto, completion, hover and Find References (from a usage or from the
//! route's string in `register_rest_route()`), scripts included. Rename
//! changes the last part of a path (`books`), in the route's string, in
//! the routes that go on from it (`/books/(?P<id>\d+)`) and wherever PHP
//! or a script writes them.
//!
//! The namespace and the route may be properties of the class set to a
//! string once (`$this->namespace`, `$this->rest_base`: WordPress's own
//! controllers); what isn't known is a `*`.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::base::Range;
use crate::frameworks::names::{DeclaredAt, NamesProject};
use crate::frameworks::refs::{
    CallTarget, CalleePattern, Declared, PlacePattern, RefContext, RefItem, ReferenceKind, RenameTarget,
};
use crate::frameworks::wordpress::assets::{Written, script_usages};
use crate::frameworks::wordpress::refs::names::in_core;
use crate::navigate::Target;
use crate::summary::DeclaredKind;
use crate::syntax::ParsedFile;

const KINDS: &[DeclaredKind] = &[DeclaredKind::RestRoute];

const REQUEST: CalleePattern = CalleePattern::Method { classes: &["WP_REST_Request"], names: &["__construct"] };

static PATTERNS: [PlacePattern; 4] = [
    PlacePattern::argument(CalleePattern::Function("rest_url"), 0, "path"),
    PlacePattern::argument(CalleePattern::Function("get_rest_url"), 1, "path"),
    PlacePattern::argument(CalleePattern::Function("rest_do_request"), 0, "request"),
    PlacePattern::argument(REQUEST, 1, "route"),
];

/// See the module documentation.
pub struct RestRoute;

/// The path `written` names: without the slashes around it and the query
/// after it.
fn path(written: &str) -> &str {
    written.split(['?', '#']).next().unwrap_or_default().trim_matches('/')
}

impl RestRoute {
    fn named<'p>(names: &'p Arc<NamesProject>, value: &str) -> Vec<&'p DeclaredAt> {
        let path = path(value);
        if path.is_empty() {
            return Vec::new();
        }
        let found = names.named(KINDS, path);
        // A pattern usually stands for one part of the path: those routes first.
        let parts = |path: &str| path.split('/').count();
        let alike: Vec<&DeclaredAt> =
            found.iter().copied().filter(|declared| parts(&declared.name) == parts(path)).collect();
        if alike.is_empty() { found } else { alike }
    }
}

impl ReferenceKind for RestRoute {
    fn id(&self) -> &'static str {
        "wordpressRestRoute"
    }

    fn patterns(&self) -> &'static [PlacePattern] {
        &PATTERNS
    }

    fn complete(&self, cx: &RefContext<'_>, value: &str, _at: usize) -> Vec<RefItem> {
        // A request's route starts with a slash; a URL's path needs none.
        let request = cx.site.as_ref().and_then(|site| site.call.as_ref()).is_some_and(|(target, _)| match target {
            CallTarget::Function(names) => names.iter().any(|name| name.ends_with("rest_do_request")),
            _ => true,
        });
        let slash = if value.starts_with('/') || (value.is_empty() && request) { "/" } else { "" };
        let names = cx.engine.names();
        let mut seen = std::collections::HashSet::new();
        names
            .of(KINDS)
            .filter(|declared| !declared.dynamic && seen.insert(declared.name.as_str()))
            .map(|declared| RefItem {
                label: declared.name.clone(),
                insert: format!("{slash}{}", declared.name),
                detail: Some("REST route".to_owned()),
                from: 0,
                to: value.len(),
            })
            .collect()
    }

    fn resolve(&self, cx: &RefContext<'_>, value: &str, _at: usize) -> Vec<Target> {
        let names = cx.engine.names();
        Self::named(&names, value)
            .into_iter()
            .map(|declared| Target { path: declared.path.clone(), range: declared.range })
            .collect()
    }

    fn canonical(&self, _cx: &RefContext<'_>, value: &str) -> String {
        path(value).to_owned()
    }

    fn refers_to(&self, value: &str, key: &str) -> bool {
        path(value) == key
    }

    fn declared_at(&self, cx: &RefContext<'_>, file: &Path, parsed: &ParsedFile, offset: u32) -> Option<Declared> {
        let names = cx.engine.names();
        let lines = parsed.line_index();
        names.of(KINDS).filter(|declared| !declared.dynamic && declared.path == file).find_map(|declared| {
            let (start, end) = lines.byte_range(declared.range);
            (start <= offset && offset <= end).then(|| Declared { value: declared.name.clone(), start, end, at: 0 })
        })
    }

    fn other_usages(&self, cx: &RefContext<'_>, key: &str) -> Vec<(PathBuf, Range)> {
        script_usages(&cx.engine.root(), key, Written::RestPath)
    }

    /// The last part of a path is renamed (`books` of `shop/v1/books`),
    /// in the routes that are it or go on from it, when the project
    /// declares them all.
    fn rename_target(&self, cx: &RefContext<'_>, value: &str, _at: usize) -> Option<RenameTarget> {
        let key = path(value);
        let (before, part) = key.rsplit_once('/')?;
        let names = cx.engine.names();
        let below = format!("{key}/");
        let routes: Vec<&DeclaredAt> =
            names.of(KINDS).filter(|declared| declared.name == key || declared.name.starts_with(&below)).collect();
        if !routes.iter().any(|declared| declared.name == key)
            || routes.iter().any(|declared| declared.library || in_core(&declared.path))
        {
            return None;
        }
        // Where each route's string says the part: after what of the key the namespace doesn't say.
        let mut declarations = Vec::new();
        for declared in routes {
            let text = std::fs::read_to_string(&declared.path).ok()?.replace("\r\n", "\n");
            let lines = crate::syntax::LineIndex::new(&text);
            let (start, end) = lines.byte_range(declared.range);
            let route = text.get(start as usize..end as usize)?;
            let lead = route.len() - route.trim_start_matches('/').len();
            let written = &route[lead..];
            let at = written.match_indices(part).map(|(at, _)| at).find(|at| {
                let upto = &written[..at + part.len()];
                let whole = !matches!(written[at + part.len()..].chars().next(), Some(c) if c != '/');
                whole
                    && (key == upto || key.ends_with(&format!("/{upto}")))
                    && before.ends_with(written[..*at].trim_end_matches('/'))
            })?;
            let from = start + (lead + at) as u32;
            declarations.push((declared.path.clone(), lines.range(from, from + part.len() as u32)));
        }
        Some(RenameTarget { key: key.to_owned(), range: (key.len() - part.len(), key.len()), declarations })
    }

    fn renames_with(&self, value: &str, key: &str) -> bool {
        value == key || value.strip_prefix(key).is_some_and(|rest| rest.starts_with('/'))
    }

    fn site_rename_range(&self, value: &str, target: &RenameTarget) -> (usize, usize) {
        let at = value.find(target.key.as_str()).unwrap_or(0);
        (at + target.range.0, at + target.range.1)
    }

    fn other_renames(&self, cx: &RefContext<'_>, target: &RenameTarget) -> Vec<(PathBuf, Range)> {
        let part = |range: Range| {
            let column = |offset: usize| crate::base::Pos::new(range.start.line, range.start.col + offset as u32);
            Range::new(column(target.range.0), column(target.range.1))
        };
        let found = script_usages(&cx.engine.root(), &target.key, Written::RestUnder);
        found.into_iter().map(|(path, range)| (path, part(range))).collect()
    }

    fn valid_name(&self, name: &str) -> bool {
        !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    }

    fn is_known(&self, cx: &RefContext<'_>, value: &str) -> bool {
        let names = cx.engine.names();
        names.of(KINDS).next().is_none() || !Self::named(&names, value).is_empty()
    }

    fn hover(&self, cx: &RefContext<'_>, value: &str) -> Option<String> {
        let names = cx.engine.names();
        let declared = Self::named(&names, value);
        let first = declared.first()?;
        let root = cx.engine.root();
        let file = first.path.strip_prefix(&root).unwrap_or(&first.path).to_string_lossy().replace('\\', "/");
        let pattern = if first.dynamic { format!(" (registered as `{}`)", first.name) } else { String::new() };
        Some(format!(
            "**REST route** `{}`{pattern}\n\nRegistered in {file}:{}",
            path(value),
            first.range.start.line + 1
        ))
    }
}
