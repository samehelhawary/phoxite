//! The reference kinds of the names WordPress keeps in its registries
//! ([`WordPressName`]): each is declared by a call that registers it and
//! named by a string wherever the functions of WordPress's API take one.
//!
//! | kind | declared by | named in |
//! | --- | --- | --- |
//! | `wordpressHook` | `do_action()`, `apply_filters()` and their `_ref_array` and `_deprecated` forms | `add_action()`, `add_filter()`, `remove_*()`, `has_*()`, `did_*()`, `doing_*()` |
//! | `wordpressShortcode` | `add_shortcode()` | `remove_shortcode()`, `shortcode_exists()`, `has_shortcode()` |
//! | `wordpressPostType` | `register_post_type()` | `get_post_type_object()`, `post_type_exists()`, `is_singular()`, `'post_type' => …` of a query's arguments, … |
//! | `wordpressTaxonomy` | `register_taxonomy()` | `get_terms(['taxonomy' => …])`, `get_the_terms()`, `has_term()`, `is_tax()`, … |
//! | `wordpressScript`, `wordpressStyle` | `wp_register_script()`, `wp_enqueue_script()` with a source (and `…_style()`) | `wp_enqueue_script()`, `wp_localize_script()`, the dependencies of another, … |
//! | `wordpressOption` | `add_option()`, `update_option()`, `register_setting()` | `get_option()`, `delete_option()` |
//! | `wordpressImageSize` | `add_image_size()` | `the_post_thumbnail()`, `wp_get_attachment_image()`, … |
//! | `wordpressNavMenu` | `register_nav_menu()`, the keys of `register_nav_menus()` | `wp_nav_menu(['theme_location' => …])`, `has_nav_menu()` |
//! | `wordpressBlock` | `register_block_type('namespace/name', …)` | `has_block()`, `unregister_block_type()`, `register_block_style()`, the block registry's `is_registered()` |
//!
//! Where a function takes one name or a list of them (`is_singular(['book',
//! 'page'])`, `'post_type' => ['book', 'page']`), each item of the list is
//! the name too.
//!
//! The declarations are facts of the indexed files
//! ([`NamesProject`]), WordPress's own among them when its source is in the
//! project. Goto goes to the declaring calls, completion offers the names
//! declared, and Find References lists the declaring calls and the others
//! alike.
//!
//! Actions and filters are one kind: WordPress keeps them in one table
//! (`add_action()` is `add_filter()`). A hook whose name is put together at
//! run time (`"save_post_{$post->post_type}"`) is known as a pattern,
//! `save_post_*`: `add_action('save_post_book', …)` goes to it. It isn't
//! offered by completion and can't be renamed.
//!
//! Renaming changes the name in every call that writes it, when the
//! project declares the name itself: not a name of WordPress (`wp-includes`,
//! `wp-admin` and the files beside them) or of a library.
//!
//! Only hooks have an "unknown" inspection (`PhpUndefinedHook`, off by
//! default): the other names are often registered where nothing can be
//! read (an option only the database has, the built-in image sizes).
//!
//! [`NamesProject`]: crate::frameworks::names::NamesProject

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::base::Range;
use crate::frameworks::wordpress::assets::{Written, script_usages};

use crate::frameworks::names::{DeclaredAt, NamesProject};
use crate::frameworks::refs::{
    CalleePattern, PlacePattern, Position, RefContext, RefItem, ReferenceKind, RenameTarget, Undefined,
};
use crate::navigate::Target;
use crate::summary::DeclaredKind;

/// One of the kinds of the module documentation.
pub struct WordPressName {
    id: &'static str,
    /// What the declarations are facts of.
    declared: &'static [DeclaredKind],
    /// What a name is called in a hover (`post type`).
    label: &'static str,
    /// What declaring it is called there (`Registered`).
    verb: &'static str,
    patterns: &'static [PlacePattern],
}

const fn arg(function: &'static str, index: usize, name: &'static str) -> PlacePattern {
    PlacePattern::argument(CalleePattern::Function(function), index, name)
}

const fn key(function: &'static str, index: usize, name: &'static str, key: &'static str) -> PlacePattern {
    arg(function, index, name).at(Position::ValueOfKey(key))
}

/// An item of the list the argument is: `is_singular(['book', 'page'])`.
const fn item(function: &'static str, index: usize, name: &'static str) -> PlacePattern {
    arg(function, index, name).at(Position::ListValue)
}

/// An item of the list under a key: `['post_type' => ['book', 'page']]`.
const fn items(function: &'static str, index: usize, name: &'static str, key: &'static str) -> PlacePattern {
    arg(function, index, name).at(Position::ListOfKey(key))
}

const QUERY: CalleePattern = CalleePattern::Method { classes: &["WP_Query"], names: &["__construct", "query"] };

pub static HOOK: WordPressName = WordPressName {
    id: "wordpressHook",
    declared: &[DeclaredKind::Action, DeclaredKind::Filter],
    label: "hook",
    verb: "Fired",
    patterns: &[
        arg("add_action", 0, "hook_name"),
        arg("add_filter", 0, "hook_name"),
        arg("remove_action", 0, "hook_name"),
        arg("remove_filter", 0, "hook_name"),
        arg("remove_all_actions", 0, "hook_name"),
        arg("remove_all_filters", 0, "hook_name"),
        arg("has_action", 0, "hook_name"),
        arg("has_filter", 0, "hook_name"),
        arg("did_action", 0, "hook_name"),
        arg("did_filter", 0, "hook_name"),
        arg("doing_action", 0, "hook_name"),
        arg("doing_filter", 0, "hook_name"),
        arg("do_action", 0, "hook_name"),
        arg("do_action_ref_array", 0, "hook_name"),
        arg("do_action_deprecated", 0, "hook_name"),
        arg("apply_filters", 0, "hook_name"),
        arg("apply_filters_ref_array", 0, "hook_name"),
        arg("apply_filters_deprecated", 0, "hook_name"),
    ],
};

pub static SHORTCODE: WordPressName = WordPressName {
    id: "wordpressShortcode",
    declared: &[DeclaredKind::Shortcode],
    label: "shortcode",
    verb: "Registered",
    patterns: &[
        arg("add_shortcode", 0, "tag"),
        arg("remove_shortcode", 0, "tag"),
        arg("shortcode_exists", 0, "tag"),
        arg("has_shortcode", 1, "tag"),
    ],
};

pub static POST_TYPE: WordPressName = WordPressName {
    id: "wordpressPostType",
    declared: &[DeclaredKind::PostType],
    label: "post type",
    verb: "Registered",
    patterns: &[
        arg("register_post_type", 0, "post_type"),
        arg("unregister_post_type", 0, "post_type"),
        arg("post_type_exists", 0, "post_type"),
        arg("get_post_type_object", 0, "post_type"),
        arg("get_post_type_archive_link", 0, "post_type"),
        arg("is_post_type_archive", 0, "post_types"),
        arg("is_singular", 0, "post_types"),
        arg("add_post_type_support", 0, "post_type"),
        arg("remove_post_type_support", 0, "post_type"),
        arg("post_type_supports", 0, "post_type"),
        arg("wp_count_posts", 0, "type"),
        arg("register_taxonomy", 1, "object_type"),
        arg("register_taxonomy_for_object_type", 1, "object_type"),
        key("get_posts", 0, "args", "post_type"),
        key("wp_insert_post", 0, "postarr", "post_type"),
        PlacePattern::argument(QUERY, 0, "query").at(Position::ValueOfKey("post_type")),
        item("is_post_type_archive", 0, "post_types"),
        item("is_singular", 0, "post_types"),
        item("register_taxonomy", 1, "object_type"),
        items("get_posts", 0, "args", "post_type"),
        PlacePattern::argument(QUERY, 0, "query").at(Position::ListOfKey("post_type")),
    ],
};

pub static TAXONOMY: WordPressName = WordPressName {
    id: "wordpressTaxonomy",
    declared: &[DeclaredKind::Taxonomy],
    label: "taxonomy",
    verb: "Registered",
    patterns: &[
        arg("register_taxonomy", 0, "taxonomy"),
        arg("unregister_taxonomy", 0, "taxonomy"),
        arg("taxonomy_exists", 0, "taxonomy"),
        arg("get_taxonomy", 0, "taxonomy"),
        arg("is_tax", 0, "taxonomy"),
        arg("register_taxonomy_for_object_type", 0, "taxonomy"),
        arg("get_the_terms", 1, "taxonomy"),
        arg("get_the_term_list", 1, "taxonomy"),
        arg("the_terms", 1, "taxonomy"),
        arg("wp_get_post_terms", 1, "taxonomy"),
        arg("wp_get_object_terms", 1, "taxonomies"),
        arg("wp_set_object_terms", 2, "taxonomy"),
        arg("wp_set_post_terms", 2, "taxonomy"),
        arg("has_term", 1, "taxonomy"),
        arg("get_term", 1, "taxonomy"),
        arg("get_term_by", 2, "taxonomy"),
        arg("get_term_link", 1, "taxonomy"),
        arg("term_exists", 1, "taxonomy"),
        arg("wp_insert_term", 1, "taxonomy"),
        key("get_terms", 0, "args", "taxonomy"),
        item("is_tax", 0, "taxonomy"),
        item("wp_get_object_terms", 1, "taxonomies"),
        items("get_terms", 0, "args", "taxonomy"),
        item("register_post_type", 1, "args").at(Position::ListOfKey("taxonomies")),
    ],
};

pub static SCRIPT: WordPressName = WordPressName {
    id: "wordpressScript",
    declared: &[DeclaredKind::Script],
    label: "script",
    verb: "Registered",
    patterns: &[
        arg("wp_register_script", 0, "handle"),
        arg("wp_enqueue_script", 0, "handle"),
        arg("wp_dequeue_script", 0, "handle"),
        arg("wp_deregister_script", 0, "handle"),
        arg("wp_localize_script", 0, "handle"),
        arg("wp_add_inline_script", 0, "handle"),
        arg("wp_script_is", 0, "handle"),
        arg("wp_set_script_translations", 0, "handle"),
        arg("wp_register_script", 2, "deps").at(Position::ListValue),
        arg("wp_enqueue_script", 2, "deps").at(Position::ListValue),
    ],
};

pub static STYLE: WordPressName = WordPressName {
    id: "wordpressStyle",
    declared: &[DeclaredKind::Style],
    label: "style",
    verb: "Registered",
    patterns: &[
        arg("wp_register_style", 0, "handle"),
        arg("wp_enqueue_style", 0, "handle"),
        arg("wp_dequeue_style", 0, "handle"),
        arg("wp_deregister_style", 0, "handle"),
        arg("wp_add_inline_style", 0, "handle"),
        arg("wp_style_is", 0, "handle"),
        arg("wp_register_style", 2, "deps").at(Position::ListValue),
        arg("wp_enqueue_style", 2, "deps").at(Position::ListValue),
    ],
};

pub static OPTION: WordPressName = WordPressName {
    id: "wordpressOption",
    declared: &[DeclaredKind::Option],
    label: "option",
    verb: "Set",
    patterns: &[
        arg("get_option", 0, "option"),
        arg("add_option", 0, "option"),
        arg("update_option", 0, "option"),
        arg("delete_option", 0, "option"),
        arg("register_setting", 1, "option_name"),
        arg("unregister_setting", 1, "option_name"),
    ],
};

pub static IMAGE_SIZE: WordPressName = WordPressName {
    id: "wordpressImageSize",
    declared: &[DeclaredKind::ImageSize],
    label: "image size",
    verb: "Registered",
    patterns: &[
        arg("add_image_size", 0, "name"),
        arg("remove_image_size", 0, "name"),
        arg("has_image_size", 0, "name"),
        arg("the_post_thumbnail", 0, "size"),
        arg("get_the_post_thumbnail", 1, "size"),
        arg("get_the_post_thumbnail_url", 1, "size"),
        arg("wp_get_attachment_image", 1, "size"),
        arg("wp_get_attachment_image_src", 1, "size"),
        arg("wp_get_attachment_image_url", 1, "size"),
    ],
};

pub static NAV_MENU: WordPressName = WordPressName {
    id: "wordpressNavMenu",
    declared: &[DeclaredKind::NavMenu],
    label: "menu location",
    verb: "Registered",
    patterns: &[
        arg("register_nav_menu", 0, "location"),
        arg("unregister_nav_menu", 0, "location"),
        arg("has_nav_menu", 0, "location"),
        arg("register_nav_menus", 0, "locations").at(Position::ArrayKey),
        key("wp_nav_menu", 0, "args", "theme_location"),
    ],
};

const BLOCKS: CalleePattern =
    CalleePattern::Method { classes: &["WP_Block_Type_Registry"], names: &["is_registered", "get_registered"] };

pub static BLOCK: WordPressName = WordPressName {
    id: "wordpressBlock",
    declared: &[DeclaredKind::Block],
    label: "block",
    verb: "Registered",
    patterns: &[
        arg("register_block_type", 0, "block_type"),
        arg("unregister_block_type", 0, "name"),
        arg("has_block", 0, "block_name"),
        arg("register_block_style", 0, "block_name"),
        arg("unregister_block_style", 0, "block_name"),
        PlacePattern::argument(BLOCKS, 0, "name"),
    ],
};

/// The action of the AJAX request the hook `name` is fired for:
/// `shop_add` of `wp_ajax_shop_add` and `wp_ajax_nopriv_shop_add`.
fn ajax_action(name: &str) -> Option<&str> {
    let action = name.strip_prefix("wp_ajax_")?;
    Some(action.strip_prefix("nopriv_").unwrap_or(action)).filter(|action| !action.is_empty())
}

/// Whether `path` is a file of WordPress itself: below one of the two
/// folders its core is in, or beside them (`wp-settings.php`, which fires
/// `init`).
pub(super) fn in_core(path: &Path) -> bool {
    path.components().any(|part| matches!(part.as_os_str().to_str(), Some("wp-includes" | "wp-admin")))
        || path.parent().is_some_and(|folder| folder.join("wp-includes").is_dir() && folder.join("wp-admin").is_dir())
}

impl WordPressName {
    fn names(cx: &RefContext<'_>) -> Arc<NamesProject> {
        cx.engine.names()
    }

    /// What declares a name outside PHP: for a block, the `block.json`
    /// files.
    fn elsewhere(&self, cx: &RefContext<'_>) -> Vec<DeclaredAt> {
        if self.id != BLOCK.id {
            return Vec::new();
        }
        let blocks = cx.engine.wordpress_blocks();
        blocks
            .iter()
            .map(|block| DeclaredAt {
                kind: DeclaredKind::Block,
                name: block.name.clone(),
                dynamic: false,
                path: block.path.clone(),
                range: block.range,
                library: cx.engine.is_library(&block.path),
                classes: Vec::new(),
            })
            .collect()
    }

    /// Where `value` is declared, in PHP and [elsewhere](Self::elsewhere).
    fn declared<'p>(&self, names: &'p NamesProject, elsewhere: &'p [DeclaredAt], value: &str) -> Vec<&'p DeclaredAt> {
        let mut found = names.named(self.declared, value);
        found.extend(elsewhere.iter().filter(|declared| declared.name == value));
        found
    }

    /// What a declaration is called: `action` and `filter` for a hook.
    fn label_of(&self, declared: &DeclaredAt) -> &'static str {
        match declared.kind {
            DeclaredKind::Action => "action",
            DeclaredKind::Filter => "filter",
            _ => self.label,
        }
    }
}

impl ReferenceKind for WordPressName {
    fn id(&self) -> &'static str {
        self.id
    }

    fn patterns(&self) -> &'static [PlacePattern] {
        self.patterns
    }

    fn complete(&self, cx: &RefContext<'_>, value: &str, _at: usize) -> Vec<RefItem> {
        let names = Self::names(cx);
        let elsewhere = self.elsewhere(cx);
        let mut seen = std::collections::HashSet::new();
        names
            .of(self.declared)
            .chain(&elsewhere)
            .filter(|declared| !declared.dynamic && seen.insert(declared.name.as_str()))
            .map(|declared| RefItem {
                label: declared.name.clone(),
                insert: declared.name.clone(),
                detail: Some(self.label_of(declared).to_owned()),
                from: 0,
                to: value.len(),
            })
            .collect()
    }

    fn resolve(&self, cx: &RefContext<'_>, value: &str, _at: usize) -> Vec<Target> {
        let names = Self::names(cx);
        let elsewhere = self.elsewhere(cx);
        let mut out: Vec<Target> = Vec::new();
        for declared in self.declared(&names, &elsewhere, value) {
            let target = Target { path: declared.path.clone(), range: declared.range };
            if !out.contains(&target) {
                out.push(target);
            }
        }
        out
    }

    fn rename_target(&self, cx: &RefContext<'_>, value: &str, _at: usize) -> Option<RenameTarget> {
        let names = Self::names(cx);
        // The hook of an AJAX request is the project's by its action, the part renamed:
        // WordPress fires it under a name it puts together.
        if self.id == HOOK.id
            && let Some(action) = ajax_action(value)
        {
            let range = (value.len() - action.len(), value.len());
            return Some(RenameTarget { key: value.to_owned(), range, declarations: Vec::new() });
        }
        let elsewhere = self.elsewhere(cx);
        let declared: Vec<&DeclaredAt> = names
            .of(self.declared)
            .filter(|declared| declared.names(value))
            .chain(elsewhere.iter().filter(|declared| declared.name == value))
            .collect();
        // The project's own, written out wherever it is declared.
        let own = !declared.is_empty()
            && declared.iter().all(|declared| !declared.library && !declared.dynamic && !in_core(&declared.path));
        // The declaring calls are usages like the others: only what isn't PHP is left to change.
        let declarations = elsewhere
            .iter()
            .filter(|declared| declared.name == value)
            .map(|declared| (declared.path.clone(), declared.range))
            .collect();
        own.then(|| RenameTarget { key: value.to_owned(), range: (0, value.len()), declarations })
    }

    fn usage_words(&self, value: &str) -> Vec<String> {
        crate::index::words(self.usage_text(value))
    }

    /// The action of an AJAX hook: `wp_ajax_nopriv_shop_add` doesn't say
    /// `wp_ajax_shop_add`, and both are renamed together.
    fn usage_text<'v>(&self, value: &'v str) -> &'v str {
        if self.id == HOOK.id { ajax_action(value).unwrap_or(value) } else { value }
    }

    fn renames_with(&self, value: &str, key: &str) -> bool {
        value == key || (self.id == HOOK.id && ajax_action(key).is_some() && ajax_action(value) == ajax_action(key))
    }

    fn site_rename_range(&self, value: &str, target: &RenameTarget) -> (usize, usize) {
        match ajax_action(value).filter(|_| self.id == HOOK.id) {
            Some(action) => (value.len() - action.len(), value.len()),
            None => target.range,
        }
    }

    fn other_renames(&self, cx: &RefContext<'_>, target: &RenameTarget) -> Vec<(PathBuf, Range)> {
        // A script writes the action alone: all of it is replaced.
        if self.id == HOOK.id && ajax_action(&target.key).is_some() {
            return self.other_usages(cx, &target.key);
        }
        let part = |range: Range| {
            let column = |offset: usize| crate::base::Pos::new(range.start.line, range.start.col + offset as u32);
            Range::new(column(target.range.0), column(target.range.1))
        };
        self.other_usages(cx, &target.key).into_iter().map(|(path, range)| (path, part(range))).collect()
    }

    fn other_usages(&self, cx: &RefContext<'_>, key: &str) -> Vec<(PathBuf, Range)> {
        let root = cx.engine.root();
        if self.id == BLOCK.id {
            script_usages(&root, key, Written::Name)
        } else if self.id == HOOK.id
            && let Some(action) = ajax_action(key)
        {
            script_usages(&root, action, Written::AjaxAction)
        } else {
            Vec::new()
        }
    }

    fn valid_name(&self, name: &str) -> bool {
        !name.is_empty() && !name.contains(['\'', '"', '\\', '$', '{', '}']) && !name.contains(char::is_whitespace)
    }

    fn is_known(&self, cx: &RefContext<'_>, value: &str) -> bool {
        let names = Self::names(cx);
        let elsewhere = self.elsewhere(cx);
        // A project that declares none has them somewhere unread.
        (names.of(self.declared).next().is_none() && elsewhere.is_empty())
            || !self.declared(&names, &elsewhere, value).is_empty()
    }

    fn hover(&self, cx: &RefContext<'_>, value: &str) -> Option<String> {
        let names = Self::names(cx);
        let elsewhere = self.elsewhere(cx);
        let declared = self.declared(&names, &elsewhere, value);
        let first = declared.first()?;
        let root = cx.engine.root();
        let mut files: Vec<String> = Vec::new();
        for declared in &declared {
            let file = declared.path.strip_prefix(&root).unwrap_or(&declared.path).to_string_lossy().replace('\\', "/");
            let line = format!("{file}:{}", declared.range.start.line + 1);
            if !files.contains(&line) {
                files.push(line);
            }
        }
        const SHOWN: usize = 5;
        let more = files.len().saturating_sub(SHOWN);
        files.truncate(SHOWN);
        let mut out = format!("**{}** `{value}`", self.label_of(first));
        if first.dynamic {
            out.push_str(&format!(" ({} as `{}`)", self.verb.to_lowercase(), first.name));
        }
        out.push_str(&format!("\n\n{} in {}", self.verb, files.join(", ")));
        if more > 0 {
            out.push_str(&format!(" and {more} more"));
        }
        Some(out)
    }

    fn undefined(&self, cx: &RefContext<'_>, value: &str) -> Option<Undefined> {
        let hook = self.id == HOOK.id;
        (hook && !self.is_known(cx, value)).then(|| Undefined::warning(format!("Hook '{value}' is not fired anywhere")))
    }
}
