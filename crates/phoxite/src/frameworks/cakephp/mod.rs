//! CakePHP 4 and 5: what an application's files say ([`project::CakeProject`]),
//! what the code of its classes says they load ([`facts`]), the classes its
//! names stand for ([`classes`], with the words of [`inflect`]), the names
//! its strings write ([`refs`]) and what its templates are given
//! ([`views`]) and the tables its migrations make ([`migrations`]). The types are in [`crate::types::provider`] (`cakephp.rs`);
//! `docs/engine/cakephp.md` lists what is known and what isn't.

pub mod classes;
pub mod facts;
pub mod inflect;
pub mod migrations;
pub mod project;
pub mod refs;
pub mod views;

pub use project::CakeProject;
