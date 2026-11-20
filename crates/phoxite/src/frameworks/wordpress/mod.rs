//! WordPress: the string references of a WordPress project ([`refs`]): its
//! hooks and the other names it registers.
//!
//! A project is a WordPress one when it has WordPress's own files
//! (`wp-config.php`, `wp-load.php`, `wp-includes/version.php`), when a file
//! at its root has a plugin or theme header, or when Composer installs
//! WordPress or its stubs ([`crate::frameworks::detect`]).
//!
//! Nothing of WordPress is bundled: its functions and hooks are known from
//! the project's own copy of it, from a stubs package in `vendor/`, or from
//! a folder named in `includePaths`. A plugin developed on its own, without
//! any of those, still has the hooks it fires itself.

pub mod assets;
pub mod refs;
pub mod scripts;
