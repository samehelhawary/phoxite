//! Symfony: the string references of a Symfony project ([`refs`]), and
//! what PHP knows about a Twig template ([`templates`]).
//!
//! What the project's configuration says is [`crate::symfony`], and the
//! types of its container and of Doctrine's repositories are the Symfony
//! type provider's ([`crate::types::provider::SymfonyProvider`]).
//!
//! A project is a Symfony one when Composer installs
//! `symfony/framework-bundle`, or when its `FrameworkBundle` class is known
//! (Symfony's own source tree, a bundle under development):
//! [`crate::engine::PhpEngine::frameworks`].

pub mod refs;
pub mod templates;

pub use templates::{LoopVariable, TwigMember};

/// The class every Symfony application has.
pub const FRAMEWORK_BUNDLE: &str = "Symfony\\Bundle\\FrameworkBundle\\FrameworkBundle";
