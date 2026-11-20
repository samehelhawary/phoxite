//! Framework support built on the engine's indexes: which frameworks a
//! project uses ([`detect`]), the string references every framework shares
//! ([`refs`]), and each framework's project model and reference kinds
//! ([`laravel`], [`symfony`]).
//!
//! The type providers stay in [`crate::types::provider`]; what a framework
//! knows about the whole project lives here. `docs/engine/frameworks.md`
//! says how a framework is added.

pub mod detect;
pub mod laravel;
pub mod refs;
pub mod symfony;

pub use detect::{ComposerPackages, Framework, Frameworks};
