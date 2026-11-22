//! Joomla: what a site's or an extension's files declare
//! ([`project::JoomlaProject`]), read with an INI reader for the language
//! files ([`ini`]) and the XML reader; the names its strings write
//! ([`refs`]), what a template is given ([`views`]) and its XML files as
//! documents ([`documents`]). The types are in [`crate::types::provider`]
//! (`joomla.rs`); `docs/engine/joomla.md` lists what is known and what
//! isn't.

pub mod documents;
pub mod ini;
pub mod project;
pub mod refs;
pub mod views;

pub use project::JoomlaProject;
