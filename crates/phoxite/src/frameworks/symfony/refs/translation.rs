//! The `symfonyTranslation` reference kind: the keys of the project's
//! translation catalogues ([`crate::symfony::translations`]) where Symfony's
//! documentation on translations takes a message: the first argument of
//! `trans()` on a translator, of `new TranslatableMessage(…)` and of `t()`.
//! A template's `'key'|trans` is a usage too.
//!
//! The declarations are the key in each locale's catalogue. Goto goes to
//! them, hover shows the translations. A message that isn't a key is
//! nothing wrong (the text itself may be the message).
//!
//! Renaming changes the key in every catalogue and every usage. A nested
//! key (`form: { title: … }`) is written in parts, so only its last part is
//! renamed, to a name without a dot; a key that some catalogues nest and
//! others write whole isn't renamed.
//!
//! A template with unsaved changes is read as the editor has it
//! ([`crate::engine::PhpEngine::template_text`]).

use std::path::PathBuf;

use crate::base::Range;
use crate::frameworks::refs::{CalleePattern, PlacePattern, RefContext, RefItem, ReferenceKind, RenameTarget};
use crate::navigate::Target;

const TRANSLATOR: &str = "Symfony\\Contracts\\Translation\\TranslatorInterface";
const TRANSLATABLE: &str = "Symfony\\Component\\Translation\\TranslatableMessage";

static PATTERNS: [PlacePattern; 3] = [
    PlacePattern::argument(CalleePattern::Method { classes: &[TRANSLATOR], names: &["trans"] }, 0, "id"),
    PlacePattern::argument(CalleePattern::Method { classes: &[TRANSLATABLE], names: &["__construct"] }, 0, "message"),
    PlacePattern::argument(CalleePattern::Function("Symfony\\Component\\Translation\\t"), 0, "message"),
];

/// See the module documentation.
pub struct SymfonyTranslation;

impl ReferenceKind for SymfonyTranslation {
    fn id(&self) -> &'static str {
        "symfonyTranslation"
    }

    fn patterns(&self) -> &'static [PlacePattern] {
        &PATTERNS
    }

    fn complete(&self, cx: &RefContext<'_>, value: &str, _at: usize) -> Vec<RefItem> {
        let keys = cx.engine.symfony_translations();
        let mut items: Vec<RefItem> = Vec::new();
        for key in keys.iter() {
            if !items.iter().any(|known| known.label == key.key) {
                items.push(RefItem {
                    label: key.key.clone(),
                    insert: key.key.clone(),
                    detail: key.value.clone(),
                    from: 0,
                    to: value.len(),
                });
            }
        }
        items
    }

    fn resolve(&self, cx: &RefContext<'_>, value: &str, _at: usize) -> Vec<Target> {
        let keys = cx.engine.symfony_translations();
        keys.iter()
            .filter(|key| key.key == value)
            .map(|key| Target { path: key.file.clone(), range: key.range })
            .collect()
    }

    /// The templates writing the key before `|trans`.
    fn other_usages(&self, cx: &RefContext<'_>, key: &str) -> Vec<(PathBuf, Range)> {
        let paths = cx.engine.twig_paths();
        let mut out = Vec::new();
        for template in paths.names() {
            let Some(file) = paths.resolve(&template) else { continue };
            let Some(text) = cx.engine.template_text(&file).filter(|text| text.contains(key)) else { continue };
            out.extend(
                crate::twig::translation_refs(&text)
                    .into_iter()
                    .filter(|r| r.name == key)
                    .map(|r| (file.clone(), r.range)),
            );
        }
        out
    }

    fn is_known(&self, _cx: &RefContext<'_>, _value: &str) -> bool {
        // A message needn't be a key: the text itself can be translated.
        true
    }

    fn rename_target(&self, cx: &RefContext<'_>, value: &str, _at: usize) -> Option<RenameTarget> {
        let keys = cx.engine.symfony_translations();
        if !value.is_ascii() {
            return None;
        }
        let mut written: Option<usize> = None;
        let mut declarations = Vec::new();
        for key in keys.iter().filter(|key| key.key == value) {
            // How much of the key is written there: all of it, or its last part.
            if key.range.start.line != key.range.end.line {
                return None;
            }
            let width = key.range.end.col.checked_sub(key.range.start.col)? as usize;
            if width == 0 || width > value.len() || written.is_some_and(|known| known != width) {
                return None;
            }
            written = Some(width);
            declarations.push((key.file.clone(), key.range));
        }
        let written = written?;
        Some(RenameTarget { key: value.to_owned(), range: (value.len() - written, value.len()), declarations })
    }

    /// A part of a nested key can't have a dot: it would become two parts.
    fn valid_rename(&self, target: &RenameTarget, name: &str) -> bool {
        target.range.0 == 0 || !name.contains('.')
    }

    fn hover(&self, cx: &RefContext<'_>, value: &str) -> Option<String> {
        let keys = cx.engine.symfony_translations();
        let lines: Vec<String> = keys
            .iter()
            .filter(|key| key.key == value)
            .map(|key| {
                let text = key.value.as_deref().unwrap_or_default();
                format!("- `{}` ({}): {text}", key.locale, key.domain)
            })
            .collect();
        (!lines.is_empty()).then(|| format!("**translation** `{value}`\n\n{}", lines.join("\n")))
    }
}
