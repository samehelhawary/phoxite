//! The `symfonyTemplate` reference kind: the names of the project's Twig
//! templates ([`TemplatePaths`]) where PHP names one, as Symfony's
//! documentation on templates lists them: the first argument of `render()`,
//! `renderView()`, `renderBlock()`, `renderBlockView()`, `renderForm()` and
//! `stream()` on a controller, of `render()`, `display()`, `load()` and
//! `resolveTemplate()` on Twig's environment, and of the `#[Template]`
//! attribute.
//!
//! The declaration is the template's file. Goto goes there; the usages are
//! the PHP strings and the templates naming it (`{% extends %}`,
//! `{% include %}`, …); renaming changes the name in all of them and moves
//! the file.
//!
//! Only a name without a namespace that ends in `.twig` is renamed: where a
//! bundle's `@Namespace/…` template is can't be changed from the project.
//!
//! [`TemplatePaths`]: crate::twig::TemplatePaths

use std::path::PathBuf;

use crate::base::{FileEdit, Range};
use crate::frameworks::refs::{
    CalleePattern, PlacePattern, RefContext, RefItem, ReferenceKind, RenameTarget, Undefined,
};
use crate::navigate::Target;

const ABSTRACT_CONTROLLER: &str = "Symfony\\Bundle\\FrameworkBundle\\Controller\\AbstractController";
const TWIG: &str = "Twig\\Environment";

const CONTROLLER_RENDER: CalleePattern = CalleePattern::Method {
    classes: &[ABSTRACT_CONTROLLER],
    names: &["render", "renderView", "renderBlock", "renderBlockView", "renderForm", "stream"],
};
const TWIG_RENDER: CalleePattern =
    CalleePattern::Method { classes: &[TWIG], names: &["render", "display", "load", "resolveTemplate"] };

/// `#[Template('page.html.twig')]` on a controller's method.
const TEMPLATE_ATTRIBUTE: CalleePattern =
    CalleePattern::Method { classes: &["Symfony\\Bridge\\Twig\\Attribute\\Template"], names: &["__construct"] };

static PATTERNS: [PlacePattern; 3] = [
    PlacePattern::argument(CONTROLLER_RENDER, 0, "view"),
    PlacePattern::argument(TWIG_RENDER, 0, "name"),
    PlacePattern::argument(TEMPLATE_ATTRIBUTE, 0, "template"),
];

/// See the module documentation.
pub struct SymfonyTemplate;

impl SymfonyTemplate {
    /// Whether `name` is a template of the project that can be renamed.
    fn renameable(name: &str) -> bool {
        !name.starts_with('@')
            && name.ends_with(".twig")
            && !name.split('/').any(|part| part.is_empty() || part == "..")
    }

    /// The file the template `name` is renamed to as `new_name`: in the
    /// template directory `name` is in.
    fn renamed(cx: &RefContext<'_>, name: &str, new_name: &str) -> Option<(PathBuf, PathBuf)> {
        let from = cx.engine.twig_paths().resolve(name)?;
        // The directory is what is left of the file without the name's own folders.
        let mut directory = from.clone();
        for _ in name.split('/') {
            directory.pop();
        }
        let mut to = directory;
        to.extend(new_name.split('/'));
        Some((from, to))
    }
}

impl ReferenceKind for SymfonyTemplate {
    fn id(&self) -> &'static str {
        "symfonyTemplate"
    }

    fn patterns(&self) -> &'static [PlacePattern] {
        &PATTERNS
    }

    fn complete(&self, cx: &RefContext<'_>, value: &str, _at: usize) -> Vec<RefItem> {
        let names = cx.engine.twig_paths().names();
        names
            .into_iter()
            .map(|name| RefItem { label: name.clone(), insert: name, detail: None, from: 0, to: value.len() })
            .collect()
    }

    fn resolve(&self, cx: &RefContext<'_>, value: &str, _at: usize) -> Vec<Target> {
        let file = cx.engine.twig_paths().resolve(value);
        file.map(|path| Target { path, range: Range::default() }).into_iter().collect()
    }

    fn other_usages(&self, cx: &RefContext<'_>, key: &str) -> Vec<(PathBuf, Range)> {
        crate::twig::template_usages_with(key, &cx.engine.twig_paths(), &|path| cx.engine.template_text(path))
    }

    fn rename_target(&self, cx: &RefContext<'_>, value: &str, _at: usize) -> Option<RenameTarget> {
        if !Self::renameable(value) {
            return None;
        }
        cx.engine.twig_paths().resolve(value)?;
        Some(RenameTarget { key: value.to_owned(), range: (0, value.len()), declarations: Vec::new() })
    }

    fn valid_name(&self, name: &str) -> bool {
        Self::renameable(name) && !name.contains(['\'', '"', '\\'])
    }

    fn rename_files(&self, cx: &RefContext<'_>, key: &str, new_value: &str) -> Vec<FileEdit> {
        match Self::renamed(cx, key, new_value) {
            Some((from, to)) if from != to => vec![FileEdit::Rename { from, to }],
            _ => Vec::new(),
        }
    }

    fn is_known(&self, cx: &RefContext<'_>, value: &str) -> bool {
        cx.engine.twig_paths().resolve(value).is_some()
    }

    /// A template of the project's own folders that isn't there. A
    /// `@Namespace/…` name can be a bundle's, whose folder the project
    /// doesn't say; a project without any template may keep them elsewhere.
    fn undefined(&self, cx: &RefContext<'_>, value: &str) -> Option<Undefined> {
        let paths = cx.engine.twig_paths();
        let certain = Self::renameable(value) && !paths.names().is_empty() && paths.resolve(value).is_none();
        certain.then(|| Undefined::warning(format!("Template '{value}' not found")))
    }

    fn hover(&self, cx: &RefContext<'_>, value: &str) -> Option<String> {
        let file = cx.engine.twig_paths().resolve(value)?;
        let root = cx.engine.root();
        let shown = file.strip_prefix(&root).unwrap_or(&file).to_string_lossy().replace('\\', "/");
        Some(format!("**template** `{value}`\n\n{shown}"))
    }
}
