//! Symfony's reference kinds ([`kinds`]), on the machinery every framework
//! shares ([`crate::frameworks::refs`]): `symfonyRoute` ([`route_name`]).

pub mod route_name;

pub use route_name::SymfonyRoute;

use crate::frameworks::refs::ReferenceKind;

/// Symfony's kinds.
pub fn kinds() -> &'static [&'static dyn ReferenceKind] {
    &[&SymfonyRoute]
}
