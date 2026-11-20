//! String references: completion, goto, find usages, rename and document
//! highlights for string literals in known places (`config('app.name')`,
//! later `route('home')`, `view('posts.index')`, …).
//!
//! The design follows the public description of IntelliJ's reference
//! contributors (IntelliJ Community, Apache-2.0): a **place pattern**
//! ([`PlacePattern`]) says where a string sits (an argument of a function,
//! of a method of a class or its subclasses, of `new`; by position or by
//! name; the string itself or a key or value of an array argument, nested
//! or not), and a **reference kind** ([`ReferenceKind`]) says what such a
//! string names and supplies completion items, targets, usages, rename and
//! validation. Kinds are listed in [`kinds`]: `configKey` ([`config_key`]),
//! `envKey` ([`env_key`]), `translationKey` ([`translation_key`]),
//! `viteAsset` ([`vite_asset`]), `routeName` ([`route_name`]),
//! `controllerAction` and `resourceAction` ([`controller_action`]),
//! `middleware` ([`middleware`]), `viewName` ([`view_name`]), and
//! `validationRule` and `requestField` ([`validation_rule`]), and
//! `eloquentField` and `eloquentRelation` ([`eloquent`]), and `gate`,
//! `authGuard` and `passwordBroker` ([`gate`]), and `artisanCommand` and `artisanOption`
//! ([`artisan_command`]), and `livewireComponent` and `livewireParameter`
//! ([`livewire`]), and
//! `inertiaPage` ([`inertia_page`]), and `staticStrings` and
//! `directoryFiles` ([`rules`]).
//!
//! Besides the built-in patterns, the completion rules of the project's
//! `ide.json` files ([`crate::frameworks::laravel::ide_json`]) say where
//! strings of a kind sit ([`rules::rule_matches`]); the rule a string was
//! matched by is part of its site ([`SiteInfo::rule`]), and its `prefix`
//! option is put before the string's value ([`Subject::prefix`]).
//!
//! A place may also be the value a method returns
//! ([`CalleePattern::Return`]: what a Form Request's `rules()` returns), or
//! the default value of a property ([`CalleePattern::Property`]: a model's
//! `$fillable`).
//!
//! A kind may describe a usage for hover ([`ReferenceKind::hover`]) and say
//! what is wrong with one that names nothing ([`ReferenceKind::undefined`]),
//! which the "undefined …" inspections
//! ([`crate::inspect::laravel::UndefinedReference`]) and the problems of
//! Blade templates ([`PhpEngine::blade_problems`]) report.
//!
//! A kind sees the string's site ([`SiteInfo`]): its file, and the class it
//! is about (the `Foo::class` before it in its array, or the controller of
//! the route registering it), so `[PostController::class, 'index']` names a
//! method of `PostController`. Usages are compared by their canonical value
//! ([`ReferenceKind::canonical`]: `App\Http\Controllers\PostController@index`).
//!
//! The string at the caret is found in the document's parse, or, while it
//! is being typed and isn't closed yet, in a copy of the file with a closed
//! probe string in its place (as completion does), so a partial string
//! (`config('app.na`) works too. Find usages goes through the word index
//! (the words of the value and the kind's words) and checks each candidate
//! string against the patterns again. Blade templates are compiled first
//! ([`crate::blade::compile`]), so the strings of their directives and echoes
//! count too, at their places in the template.
//!
//! Usages that aren't string literals (`<livewire:counter />` tags) come
//! from [`ReferenceKind::markup_usages`]; find usages and rename from files
//! that aren't PHP (`.env`, Inertia pages) are in [`files`]. A rename edits
//! every declaration of the value ([`RenameTarget::declarations`]: each
//! locale's key, both `.env` files).
//!
//!
//! The machinery (place patterns, the kind trait, the engine's requests) is
//! [`crate::frameworks::refs`], shared by every framework; what is here is
//! Laravel's: its kinds, and what Blade and views ask of them. Laravel's
//! kinds are asked only in a Laravel project ([`PhpEngine::laravel`]).

pub mod artisan_command;
pub mod config_key;
pub mod controller_action;
mod controller_fixes;
pub mod db_table;
pub mod eloquent;
pub mod env_key;
pub mod files;
pub mod gate;
pub mod inertia_page;
pub mod intentions;
pub mod livewire;
pub mod middleware;
pub mod route_name;
pub mod rules;
#[cfg(test)]
mod tests;
pub mod translation_key;
pub mod validation_actions;
pub mod validation_rule;
pub mod view_name;
pub mod vite_asset;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::base::{CancellationToken, Diagnostic, IdeResult, Severity, WorkspaceEdit};

use super::ide_json::CompletionRule;
use super::routes::RouteAction;
use crate::engine::{EngineState, PhpEngine};
use crate::names::{FileNames, NameKind, fqn_eq};
use crate::navigate::{Usage, UsageKind};
use crate::summary::FileSummary;
use crate::syntax::ParsedFile;
use crate::types::{ExprTypes, Resolver, Type};

pub use crate::frameworks::refs::*;

pub use artisan_command::{ArtisanCommandKind, ArtisanOptionKind};
pub use config_key::ConfigKey;
pub use controller_action::{ControllerAction, ResourceAction};
pub use db_table::DbTable;
pub use eloquent::{EloquentField, EloquentRelation};
pub use env_key::EnvKey;
pub use gate::{AuthGuard, Gate, PasswordBroker};
pub use inertia_page::InertiaPageKind;
pub use livewire::{LivewireComponentKind, LivewireParameterKind};
pub use middleware::Middleware;
pub use route_name::RouteName;
pub use rules::{DirectoryFiles, StaticStrings};
pub use translation_key::TranslationKey;
pub use validation_rule::{RequestField, ValidationRule};
pub use view_name::ViewName;
pub use vite_asset::ViteAsset;

/// The built-in kinds.
pub fn kinds() -> &'static [&'static dyn ReferenceKind] {
    &[
        &ConfigKey,
        &EnvKey,
        &TranslationKey,
        &ViteAsset,
        &RouteName,
        &ControllerAction,
        &ResourceAction,
        &Middleware,
        &ViewName,
        &ValidationRule,
        &RequestField,
        &EloquentField,
        &EloquentRelation,
        &DbTable,
        &Gate,
        &AuthGuard,
        &PasswordBroker,
        &ArtisanCommandKind,
        &ArtisanOptionKind,
        &LivewireComponentKind,
        &LivewireParameterKind,
        &InertiaPageKind,
        &StaticStrings,
        &DirectoryFiles,
    ]
}

/// The Laravel kind whose patterns `site` matches.
pub fn kind_of(site: &StringSite, resolver: &Resolver<'_>) -> Option<&'static dyn ReferenceKind> {
    kind_in(kinds(), site, resolver)
}

/// The Laravel kind with id `id`.
pub fn kind_by_id(id: &str) -> Option<&'static dyn ReferenceKind> {
    kind_named(kinds(), id)
}

/// The Laravel kind of `site`: the one whose patterns match it, else the one
/// of the first of `rules` matching it, with that rule.
pub fn classify<'r>(
    site: &StringSite,
    resolver: &Resolver<'_>,
    rules: &'r [CompletionRule],
) -> Option<(&'static dyn ReferenceKind, Option<&'r CompletionRule>)> {
    classify_in(kinds(), site, resolver, rules)
}

/// The string site of a Laravel kind at `offset` of `parsed`, in its own
/// parse or in a probe with the literal closed.
pub fn site_at(parsed: &ParsedFile, resolver: &Resolver<'_>, offset: u32) -> Option<StringSite> {
    site_at_in(kinds(), parsed, resolver, &[], offset)
}

/// [`site_at`], with the calls of `rules` too.
pub fn site_at_with(
    parsed: &ParsedFile,
    resolver: &Resolver<'_>,
    rules: &[CompletionRule],
    offset: u32,
) -> Option<StringSite> {
    site_at_in(kinds(), parsed, resolver, rules, offset)
}

impl PhpEngine {
    /// The places the project's routes name method `method` of `class` (or
    /// of a class it extends, or that extends it): the method's string in
    /// `[Foo::class, 'bar']` and `'Foo@bar'`, the class of an invokable
    /// controller or of a resource. Empty outside Laravel projects.
    pub(crate) fn route_action_usages(&self, class: &str, method: &str) -> Vec<Usage> {
        let Some(routes) = self.laravel_routes() else { return Vec::new() };
        let resolver = self.resolver();
        let related = |found: &str| {
            fqn_eq(NameKind::ClassLike, found, class)
                || resolver.class_model(found).is_some_and(|model| model.is_subclass_of(class))
                || resolver.class_model(class).is_some_and(|model| model.is_subclass_of(found))
        };
        let mut out: Vec<Usage> = Vec::new();
        for route in routes.routes() {
            let RouteAction::Controller { class: found, method: named, at: Some((_, _, range)) } = &route.action else {
                continue;
            };
            if named.eq_ignore_ascii_case(method) && related(found) {
                let usage = Usage { path: route.path.clone(), range: *range, kind: UsageKind::String };
                if !out.contains(&usage) {
                    out.push(usage);
                }
            }
        }
        out
    }

    /// The undefined configuration keys, environment keys, translations, Vite
    /// assets and abilities of a Blade template, as diagnostics of the template.
    pub(crate) fn blade_reference_problems(&self, text: &str) -> Vec<Diagnostic> {
        self.blade_reference_fixes(text).into_iter().map(|(diagnostic, _)| diagnostic).collect()
    }

    /// [`blade_reference_problems`](Self::blade_reference_problems), each
    /// with its quick-fix ("Create view").
    pub(crate) fn blade_reference_fixes(&self, text: &str) -> Vec<(Diagnostic, Option<(String, WorkspaceEdit)>)> {
        let compiled = crate::blade::compile(text, false);
        let parsed = ParsedFile::parse("template.php", compiled.php.as_str());
        // Only the kinds whose inspection the profile leaves on, as in PHP files.
        let enabled = self.inspections().ids();
        let kinds: Vec<&'static dyn ReferenceKind> = [
            &ConfigKey as &'static dyn ReferenceKind,
            &EnvKey,
            &TranslationKey,
            &ViteAsset,
            &Gate,
            &AuthGuard,
            &ViewName,
        ]
        .into_iter()
        .filter(|kind| crate::inspect::laravel::inspection_id(kind.id()).is_some_and(|id| enabled.contains(&id)))
        .collect();
        // Route names have their own inspection (`crate::inspect::routes`).
        let routes = if enabled.contains(&"PhpUndefinedRoute") && self.status().state == EngineState::Ready {
            self.laravel_routes()
        } else {
            None
        };
        if kinds.is_empty() && routes.is_none() {
            return Vec::new();
        }
        let lines = crate::syntax::LineIndex::new(text);
        let mut out = Vec::new();
        let cancel = CancellationToken::new();
        let undefined_routes =
            routes.map(|routes| crate::inspect::routes::undefined_laravel_routes(self, &parsed, &routes, &cancel));
        for (start, end, message) in undefined_routes.unwrap_or_default() {
            let (Some(start), Some(end)) = (compiled.blade_offset(start), compiled.blade_offset(end)) else {
                continue;
            };
            let diagnostic = Diagnostic {
                range: lines.range(start, end),
                severity: Severity::Warning,
                message,
                source: Some(crate::blade::structure::DIAGNOSTIC_SOURCE.into()),
                code: Some("PhpUndefinedRoute".into()),
                unnecessary: false,
                deprecated: false,
                server: None,
            };
            out.push((diagnostic, None));
        }
        if kinds.is_empty() {
            return out;
        }
        for found in self.undefined_references(&parsed, Path::new(""), &kinds, &cancel) {
            let (Some(start), Some(end)) = (compiled.blade_offset(found.start), compiled.blade_offset(found.end))
            else {
                continue;
            };
            let diagnostic = Diagnostic {
                range: lines.range(start, end),
                severity: found.problem.severity,
                message: found.problem.message,
                source: Some(crate::blade::structure::DIAGNOSTIC_SOURCE.into()),
                code: Some(crate::inspect::laravel::inspection_id(found.kind).unwrap_or(found.kind).into()),
                unnecessary: false,
                deprecated: false,
                server: None,
            };
            out.push((diagnostic, found.problem.fixes.into_iter().next().map(|fix| (fix.title, fix.edit))));
        }
        out
    }

    /// The views of the Laravel project with the name of the view file
    /// `path` (`posts.index`), when it is one.
    pub fn laravel_view_name(&self, path: &Path) -> Option<String> {
        self.laravel_views()?.name_of(path)
    }

    /// The usages of view `name` (`view('posts.index')`, `@include(…)`, …)
    /// in the project. `None` outside Laravel projects.
    pub fn laravel_view_usages(&self, name: &str, cancel: &CancellationToken) -> Option<IdeResult<Vec<Usage>>> {
        self.ref_context()?;
        let key = crate::frameworks::laravel::views::normalize_name(name);
        let found = self.string_usages(&ViewName, &key, &|value| value == key, cancel);
        Some(found.map(|sites| {
            let mut usages: Vec<Usage> = sites
                .into_iter()
                .map(|site| Usage { path: site.path, range: site.range, kind: UsageKind::String })
                .collect();
            usages.sort_by(|a, b| (&a.path, a.range.start).cmp(&(&b.path, b.range.start)));
            usages.dedup();
            usages
        }))
    }

    /// The variables the callers of the view file `path` pass it (`(name,
    /// type)`, sorted by name; the types of one name joined), for the Blade
    /// compiler. Empty outside Laravel projects and for other files.
    pub fn laravel_view_variables(&self, path: &Path) -> Arc<Vec<(String, String)>> {
        // Read from the bodies of the controllers that render the view: every edit counts.
        let modification = self.edit_count();
        let path = crate::project::normalize(path);
        if let Some(found) = self.view_variables_cache().lock().get(&path).filter(|(at, _)| *at == modification) {
            return found.1.clone();
        }
        // A template including itself, directly or through others, gets
        // nothing more on the way round.
        thread_local! {
            static IN_PROGRESS: std::cell::RefCell<Vec<PathBuf>> = const { std::cell::RefCell::new(Vec::new()) };
        }
        if IN_PROGRESS.with(|views| views.borrow().contains(&path)) {
            return Arc::new(Vec::new());
        }
        IN_PROGRESS.with(|views| views.borrow_mut().push(path.clone()));
        let found = Arc::new(self.view_variables_uncached(&path).unwrap_or_default());
        IN_PROGRESS.with(|views| views.borrow_mut().retain(|view| *view != path));
        self.view_variables_cache().lock().insert(path, (modification, found.clone()));
        found
    }

    fn view_variables_uncached(&self, path: &Path) -> Option<Vec<(String, String)>> {
        self.ref_context()?;
        let name = self.laravel_view_name(path)?;
        let kind: &'static dyn ReferenceKind = &ViewName;
        let candidates = self.usage_candidates(kind, &name).ok()?;
        let open: Vec<_> = self.open_documents().into_iter().filter(|document| !document.embedded).collect();
        let resolver = self.resolver();
        let mut passed: Vec<(String, Type)> = Vec::new();
        for candidate in candidates {
            // A calling template is compiled with the variables its own
            // callers pass it, which `@include` and `@extends` pass on.
            let file = if crate::frameworks::laravel::views::is_blade(&candidate) && !self.is_library(&candidate) {
                self.current_text(&candidate).filter(|text| text.contains(kind.usage_text(&name))).map(|text| {
                    let text = text.replace("\r\n", "\n");
                    let compiled = self.blade_compile(&candidate, &text);
                    let parsed = Arc::new(ParsedFile::parse("usages.php", compiled.php.as_str()));
                    let lines = Arc::new(crate::syntax::LineIndex::new(&text));
                    UsageFile { parsed, lines, compiled: Some(compiled) }
                })
            } else {
                self.usage_file(&candidate, &open, kind.usage_text(&name))
            };
            let Some(file) = file else { continue };
            let starts: Vec<u32> = sites(&file.parsed, &resolver, &[kind], None)
                .into_iter()
                .filter(|site| {
                    kind.patterns().iter().any(|pattern| pattern.matches(site, &resolver))
                        && crate::frameworks::laravel::views::normalize_name(&site.value) == name
                })
                .map(|site| site.start)
                .collect();
            if starts.is_empty() {
                continue;
            }
            let file_names = FileNames::resolve(&file.parsed);
            let summary = FileSummary::extract(&file.parsed);
            let types = ExprTypes::new(&file.parsed, &file_names, &summary, &resolver);
            passed.extend(crate::frameworks::laravel::views::passed_variables(&types, &file.parsed, &starts));
        }
        let mut names: Vec<String> = passed.iter().map(|(name, _)| name.clone()).collect();
        names.sort();
        names.dedup();
        Some(
            names
                .into_iter()
                .map(|name| {
                    let known: Vec<Type> = passed
                        .iter()
                        .filter(|(passed, ty)| *passed == name && !matches!(ty, Type::Mixed))
                        .map(|(_, ty)| ty.clone())
                        .collect();
                    let ty = if known.is_empty() { Type::Mixed } else { Type::union(known) };
                    (name, ty.to_string())
                })
                .collect(),
        )
    }
}
