//! Drupal (8 and later): what its YAML files declare ([`project`]), the
//! strings of PHP that name those things ([`refs`]), and the same names in
//! the YAML files and templates themselves ([`documents`]).

pub mod documents;
pub mod project;
pub mod refs;
pub mod templates;

pub use project::{DrupalProject, Extension, FileKind};
