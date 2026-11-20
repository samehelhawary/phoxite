//! WordPress's reference kinds ([`kinds`]), on the machinery every
//! framework shares ([`crate::frameworks::refs`]): the names of
//! WordPress's registries ([`names`]: hooks, shortcodes, post types,
//! taxonomies, script and style handles, options, image sizes, menu
//! locations and blocks), REST routes ([`rest`]), and the files of a theme that
//! `get_template_part()`, `get_header()` and `locate_template()` name
//! ([`template_part`]).

pub mod names;
pub mod rest;
pub mod template_part;

pub use names::{
    BLOCK, HOOK, IMAGE_SIZE, NAV_MENU, OPTION, POST_TYPE, SCRIPT, SHORTCODE, STYLE, TAXONOMY, WordPressName,
};
pub use rest::RestRoute;
pub use template_part::{TemplatePart, ThemeFile};

use crate::frameworks::refs::ReferenceKind;

/// WordPress's kinds.
pub fn kinds() -> &'static [&'static dyn ReferenceKind] {
    static KINDS: [&dyn ReferenceKind; 13] = [
        &HOOK,
        &SHORTCODE,
        &POST_TYPE,
        &TAXONOMY,
        &SCRIPT,
        &STYLE,
        &OPTION,
        &IMAGE_SIZE,
        &NAV_MENU,
        &BLOCK,
        &RestRoute,
        &TemplatePart,
        &ThemeFile,
    ];
    &KINDS
}
