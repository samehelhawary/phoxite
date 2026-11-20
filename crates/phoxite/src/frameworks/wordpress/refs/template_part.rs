//! The `wordpressTemplatePart` reference kind: the slug of
//! `get_template_part('template-parts/content', 'page')`, which names a
//! file of the theme: `template-parts/content-page.php`, or
//! `template-parts/content.php` when the more specific one isn't there or
//! no name is given (the order WordPress's theme handbook describes).
//!
//! The theme is the nearest folder above the calling file that has a
//! `style.css` or a `functions.php`; without one, the project. A child
//! theme's files come first and its parent's after: the folder beside it
//! that the `Template:` header of its `style.css` names.
//!
//! Goto opens the file, completion offers the theme's PHP files as slugs,
//! and Find References lists the calls writing the same slug. A template
//! part isn't renamed here: the slug and the name together make the file's
//! name.
//!
//! The `wordpressThemeFile` kind ([`ThemeFile`]) is the theme's other
//! files named by a string: the name of `get_header('shop')`,
//! `get_footer()` and `get_sidebar()` (`header-shop.php`, else
//! `header.php`), and the path of `locate_template()` (one or a list) and
//! of `get_theme_file_path()` and `get_theme_file_uri()`.

use std::path::{Path, PathBuf};

use crate::base::Range;
use crate::frameworks::refs::{CallTarget, CalleePattern, PlacePattern, Position, RefContext, RefItem, ReferenceKind};
use crate::navigate::Target;

static PATTERNS: [PlacePattern; 1] = [PlacePattern::argument(CalleePattern::Function("get_template_part"), 0, "slug")];

/// How deep below the theme's folder template parts are looked for.
const MAX_DEPTH: usize = 4;

/// The most slugs completion offers.
const MAX_ITEMS: usize = 500;

/// See the module documentation.
pub struct TemplatePart;

/// The folder of the theme the context's file is in.
fn theme(cx: &RefContext<'_>) -> PathBuf {
    let root = cx.engine.root();
    let Some(site) = &cx.site else { return root };
    let mut folder = site.path.parent();
    while let Some(current) = folder {
        if current.join("style.css").is_file() || current.join("functions.php").is_file() {
            return current.to_path_buf();
        }
        if current == root {
            break;
        }
        folder = current.parent();
    }
    root
}

/// The theme and, for a child theme, its parent after it: the folder beside
/// it that the `Template:` header of its `style.css` names. WordPress looks
/// for a file in that order.
fn themes(cx: &RefContext<'_>) -> Vec<PathBuf> {
    let theme = theme(cx);
    let parent = std::fs::read_to_string(theme.join("style.css")).ok().and_then(|style| {
        let header = style.lines().take(40).find_map(|line| {
            let line = line.trim_start_matches([' ', '\t', '*', '/']);
            line.strip_prefix("Template:").map(|name| name.trim().to_owned())
        })?;
        let named = !header.is_empty() && !header.contains(['/', '\\']) && header != "..";
        let folder = theme.parent()?.join(&header);
        (named && folder != theme && folder.is_dir()).then_some(folder)
    });
    std::iter::once(theme).chain(parent).collect()
}

/// The first of `files`, each looked for in every one of `themes` in turn.
fn first(themes: &[PathBuf], files: impl IntoIterator<Item = String>) -> Option<PathBuf> {
    files.into_iter().flat_map(|file| themes.iter().map(move |theme| theme.join(&file))).find(|file| file.is_file())
}

/// The files below `folder` as paths relative to `theme`: the PHP ones
/// without their extension when `slugs`, else all of them as they are.
fn files(theme: &Path, folder: &Path, slugs: bool, depth: usize, out: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(folder) else { return };
    let mut entries: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).collect();
    entries.sort();
    for path in entries {
        if out.len() >= MAX_ITEMS {
            return;
        }
        let name = path.file_name().and_then(|name| name.to_str()).unwrap_or_default();
        if path.is_dir() {
            if depth < MAX_DEPTH && !name.starts_with('.') && !matches!(name, "node_modules" | "vendor") {
                files(theme, &path, slugs, depth + 1, out);
            }
        } else if let Ok(relative) = path.strip_prefix(theme) {
            let php = path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("php"));
            if slugs && php {
                out.push(relative.with_extension("").to_string_lossy().replace('\\', "/"));
            } else if !slugs && !name.starts_with('.') {
                out.push(relative.to_string_lossy().replace('\\', "/"));
            }
        }
    }
}

/// Whether `written` stays inside the folder it is relative to.
fn inside(written: &str) -> bool {
    !written.is_empty() && !written.split(['/', '\\']).any(|part| part == "..")
}

impl TemplatePart {
    /// The file `slug` names, with the name the call's next argument gives.
    fn file(cx: &RefContext<'_>, slug: &str) -> Option<PathBuf> {
        if !inside(slug) {
            return None;
        }
        let names = cx.site.as_ref().map(|site| site.next_argument.as_slice()).unwrap_or_default();
        let specific = names.iter().filter(|name| !name.is_empty()).map(|name| format!("{slug}-{name}.php"));
        first(&themes(cx), specific.chain(std::iter::once(format!("{slug}.php"))))
    }
}

impl ReferenceKind for TemplatePart {
    fn id(&self) -> &'static str {
        "wordpressTemplatePart"
    }

    fn patterns(&self) -> &'static [PlacePattern] {
        &PATTERNS
    }

    fn uses_next_argument(&self) -> bool {
        true
    }

    fn complete(&self, cx: &RefContext<'_>, value: &str, _at: usize) -> Vec<RefItem> {
        let mut slugs = Vec::new();
        for theme in themes(cx) {
            files(&theme, &theme, true, 0, &mut slugs);
        }
        slugs.sort();
        slugs.dedup();
        slugs
            .into_iter()
            .map(|slug| RefItem { label: slug.clone(), insert: slug, detail: None, from: 0, to: value.len() })
            .collect()
    }

    fn resolve(&self, cx: &RefContext<'_>, value: &str, _at: usize) -> Vec<Target> {
        Self::file(cx, value).map(|path| Target { path, range: Range::default() }).into_iter().collect()
    }

    fn is_known(&self, cx: &RefContext<'_>, value: &str) -> bool {
        Self::file(cx, value).is_some()
    }

    fn hover(&self, cx: &RefContext<'_>, value: &str) -> Option<String> {
        let file = Self::file(cx, value)?;
        let root = cx.engine.root();
        let file = file.strip_prefix(&root).unwrap_or(&file).to_string_lossy().replace('\\', "/");
        Some(format!("**template part** `{value}`\n\n{file}"))
    }
}

/// The functions that take the name of one of the theme's standard
/// templates, and the file's name without it.
const NAMED: &[(&str, &str)] = &[("get_header", "header"), ("get_footer", "footer"), ("get_sidebar", "sidebar")];

const fn file(function: &'static str, name: &'static str) -> PlacePattern {
    PlacePattern::argument(CalleePattern::Function(function), 0, name)
}

static FILE_PATTERNS: [PlacePattern; 9] = [
    file("get_header", "name"),
    file("get_footer", "name"),
    file("get_sidebar", "name"),
    file("locate_template", "template_names"),
    file("locate_template", "template_names").at(Position::ListValue),
    file("get_theme_file_path", "file"),
    file("get_theme_file_uri", "file"),
    file("get_parent_theme_file_path", "file"),
    file("get_parent_theme_file_uri", "file"),
];

/// See the module documentation.
pub struct ThemeFile;

impl ThemeFile {
    /// The standard template the string's call names a variant of
    /// (`header` for `get_header()`), if it is one of [`NAMED`].
    fn named(cx: &RefContext<'_>) -> Option<&'static str> {
        let (CallTarget::Function(candidates), _) = cx.site.as_ref()?.call.as_ref()? else { return None };
        NAMED.iter().find_map(|(function, base)| {
            let called = |fqn: &String| fqn.rsplit('\\').next().is_some_and(|name| name.eq_ignore_ascii_case(function));
            candidates.iter().any(called).then_some(*base)
        })
    }

    fn path(cx: &RefContext<'_>, value: &str) -> Option<PathBuf> {
        let value = value.trim_start_matches('/');
        if !inside(value) {
            return None;
        }
        let themes = themes(cx);
        match Self::named(cx) {
            // WordPress loads the plain one when the named one isn't there.
            Some(base) => first(&themes, [format!("{base}-{value}.php"), format!("{base}.php")]),
            None => first(&themes, [value.to_owned()]),
        }
    }
}

impl ReferenceKind for ThemeFile {
    fn id(&self) -> &'static str {
        "wordpressThemeFile"
    }

    fn patterns(&self) -> &'static [PlacePattern] {
        &FILE_PATTERNS
    }

    fn complete(&self, cx: &RefContext<'_>, value: &str, _at: usize) -> Vec<RefItem> {
        let mut found = Vec::new();
        let named = Self::named(cx);
        for theme in themes(cx) {
            match named {
                Some(base) => {
                    let Ok(entries) = std::fs::read_dir(&theme) else { continue };
                    let prefix = format!("{base}-");
                    for entry in entries.flatten() {
                        let name = entry.file_name().to_string_lossy().into_owned();
                        let variant = name.strip_prefix(&prefix).and_then(|rest| rest.strip_suffix(".php"));
                        found.extend(variant.filter(|variant| !variant.is_empty()).map(str::to_owned));
                    }
                }
                None => files(&theme, &theme, false, 0, &mut found),
            }
        }
        found.sort();
        found.dedup();
        found
            .into_iter()
            .map(|file| RefItem { label: file.clone(), insert: file, detail: None, from: 0, to: value.len() })
            .collect()
    }

    fn resolve(&self, cx: &RefContext<'_>, value: &str, _at: usize) -> Vec<Target> {
        Self::path(cx, value).map(|path| Target { path, range: Range::default() }).into_iter().collect()
    }

    fn is_known(&self, cx: &RefContext<'_>, value: &str) -> bool {
        Self::path(cx, value).is_some()
    }

    fn hover(&self, cx: &RefContext<'_>, value: &str) -> Option<String> {
        let file = Self::path(cx, value)?;
        let root = cx.engine.root();
        let file = file.strip_prefix(&root).unwrap_or(&file).to_string_lossy().replace('\\', "/");
        Some(format!("**theme file** `{value}`\n\n{file}"))
    }
}
