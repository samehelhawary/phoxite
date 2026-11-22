//! Yii 2: what an application's configuration says
//! ([`project::YiiProject`]) and the names its strings write
//! ([`refs`]), what its views are passed ([`views`]) and the tables its migrations make ([`migrations`]). The types are in [`crate::types::provider`] (`yii.rs`);
//! `docs/engine/yii.md` lists what is known and what isn't.

pub mod migrations;
pub mod project;
pub mod refs;
pub mod views;

pub use project::YiiProject;
