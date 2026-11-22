//! Laminas MVC and Mezzio: what an application's configuration says
//! ([`project::LaminasProject`]), the names its strings write ([`refs`]) and
//! what its templates are given ([`views`]). The types are in
//! [`crate::types::provider`] (`laminas.rs`); `docs/engine/laminas.md` lists
//! what is known and what isn't.

pub mod project;
pub mod refs;
pub mod views;

pub use project::LaminasProject;
