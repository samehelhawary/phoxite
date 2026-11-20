//! Symfony's reference kinds ([`kinds`]), on the machinery every framework
//! shares ([`crate::frameworks::refs`]): `symfonyRoute` ([`route_name`]),
//! `symfonyTemplate` ([`template_name`]), `symfonyService` and
//! `symfonyParameter` ([`container`]), `symfonyTranslation`
//! ([`translation`]), and `doctrineField` ([`doctrine`]).
//!
//! A Twig template names both too (`path('home')`, `{% extends 'base.html.twig' %}`).
//! Templates aren't PHP, so the editor's requests about them start in
//! [`crate::twig`]; what the project knows about the name at the caret is
//! asked here by the kind's id ([`PhpEngine::symfony_name_usages`] and the
//! methods beside it).

pub mod container;
pub mod doctrine;
pub mod route_name;
pub mod template_name;
pub mod translation;

pub use container::{SymfonyParameter, SymfonyService};
pub use doctrine::DoctrineField;
pub use route_name::SymfonyRoute;
pub use template_name::SymfonyTemplate;
pub use translation::SymfonyTranslation;

use crate::base::{CancellationToken, IdeError, IdeResult, WorkspaceEdit};
use crate::engine::PhpEngine;
use crate::frameworks::refs::{RefItem, ReferenceKind, kind_named};
use crate::navigate::{Target, Usage};

/// Symfony's kinds.
pub fn kinds() -> &'static [&'static dyn ReferenceKind] {
    // A parameter bag is a container too: its `get('name')` is a parameter's, so that kind is asked first.
    &[&SymfonyRoute, &SymfonyTemplate, &SymfonyParameter, &SymfonyService, &SymfonyTranslation, &DoctrineField]
}

impl PhpEngine {
    /// The filters, functions and tests the project's PHP adds to Twig
    /// (`new TwigFilter('price', …)`, `#[AsTwigFilter('price')]`), with where
    /// each name is written. Whatever the project's frameworks: Twig is
    /// extended the same way everywhere.
    pub fn twig_extension_names(&self) -> Vec<crate::symfony::TwigNameDef> {
        self.resolver().symfony().twig_names.clone()
    }

    /// The Symfony kind with id `kind`, in a Symfony project.
    fn symfony_kind(&self, kind: &str) -> Option<&'static dyn ReferenceKind> {
        self.frameworks().contains(crate::frameworks::Framework::Symfony).then(|| kind_named(kinds(), kind)).flatten()
    }

    /// What completion offers for a name of the Symfony kind `kind`
    /// (`symfonyRoute`, `symfonyTemplate`) written outside PHP. Empty
    /// outside Symfony projects.
    pub fn symfony_name_items(&self, kind: &str) -> Vec<RefItem> {
        let (Some(kind), Some(cx)) = (self.symfony_kind(kind), self.ref_context()) else { return Vec::new() };
        kind.complete(&cx, "", 0)
    }

    /// Where the name `value` of the Symfony kind `kind` is declared;
    /// `None` outside Symfony projects.
    pub fn symfony_name_targets(&self, kind: &str, value: &str) -> Option<Vec<Target>> {
        let kind = self.symfony_kind(kind)?;
        Some(kind.resolve(&self.ref_context()?, value, value.len()))
    }

    /// The usages of the name `value` of the Symfony kind `kind`, in PHP and
    /// in the templates on disk; `None` outside Symfony projects.
    pub fn symfony_name_usages(
        &self,
        kind: &str,
        value: &str,
        include_declaration: bool,
        cancel: &CancellationToken,
    ) -> Option<IdeResult<Vec<Usage>>> {
        let kind = self.symfony_kind(kind)?;
        let cx = self.ref_context()?;
        Some(self.kind_usages(&cx, kind, value, value, include_declaration, cancel))
    }

    /// Whether the name `value` of the Symfony kind `kind` can be renamed.
    pub fn symfony_name_renames(&self, kind: &str, value: &str) -> bool {
        let Some(kind) = self.symfony_kind(kind) else { return false };
        self.ref_context().is_some_and(|cx| kind.rename_target(&cx, value, value.len()).is_some())
    }

    /// Renames the name `value` of the Symfony kind `kind` to `new_name`:
    /// its declaration and every usage, in PHP and in the templates on
    /// disk. `None` outside Symfony projects.
    pub fn symfony_name_rename(
        &self,
        kind: &str,
        value: &str,
        new_name: &str,
        cancel: &CancellationToken,
    ) -> Option<IdeResult<WorkspaceEdit>> {
        let kind = self.symfony_kind(kind)?;
        let cx = self.ref_context()?;
        let Some(target) = kind.rename_target(&cx, value, value.len()) else {
            return Some(Err(IdeError::invalid("this name can't be renamed")));
        };
        Some(self.kind_rename(&cx, kind, target, new_name, cancel))
    }
}
