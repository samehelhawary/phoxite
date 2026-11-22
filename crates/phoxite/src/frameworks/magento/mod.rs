//! Magento 2: what a project's XML files declare
//! ([`project::MagentoProject`]), the classes its code generator would write
//! ([`generated`]), the names its strings write ([`refs`]) and what its
//! templates are given ([`views`]). The types are in
//! [`crate::types::provider`] (`magento.rs`); `docs/engine/magento.md` lists
//! what is known and what isn't.

pub mod documents;
pub mod generated;
pub mod plugins;
pub mod project;
pub mod refs;
pub mod views;

pub use project::MagentoProject;
