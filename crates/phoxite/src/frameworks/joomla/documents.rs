//! A Joomla project's XML files, language files and asset lists as
//! documents: what the text at the caret names, for goto, hover, Find
//! References, rename and completion there.
//!
//! | name | where | answered by |
//! | --- | --- | --- |
//! | a language constant | a label, a description, a title or a hint of a form, a `config.xml`, an `access.xml`, a menu item's options; a manifest's name, description and menu texts; the key of a line of a `.ini` file | `joomlaLanguage`: its declaration, its usages in PHP and in the other files, its rename |
//! | a field's type | `type="…"` of a form's field | the field's class: in the namespaces `addfieldprefix` gives (`modal_article` is `Modal\ArticleField`), then Joomla's own |
//! | a class | a rule's, a filter's or a field's `class` written with its namespace | the class |
//! | a layout | `layout="joomla.form.field.radio.switcher"` | `joomlaLayout` |
//! | an ACL action | `access.xml` | `joomlaAction` |
//! | a component | `access.xml`'s `component` | `joomlaExtension` |
//! | a web asset | a name or a dependency of a `joomla.asset.json` | `joomlaAsset` |
//!
//! Completion offers what the place takes: the constants starting with
//! what is written, the field types the form's namespaces and Joomla have,
//! the layouts, the assets.
//!
//! Each name is answered by its reference kind ([`super::refs`]), as for a
//! string of PHP.

use std::path::Path;

use super::ini;
use super::project::{self, JoomlaProject, Mention, What};
use super::refs::{JoomlaName, LANGUAGE_CONSTANT, kind_of};
use crate::base::{CancellationToken, IdeError, IdeResult, Pos, Range, WorkspaceEdit};
use crate::engine::PhpEngine;
use crate::frameworks::Framework;
use crate::frameworks::drupal::documents::NameCompletion;
use crate::frameworks::refs::{RefContext, ReferenceKind};
use crate::navigate::{HoverInfo, Target, Usage, UsageKind};

/// What the text at the caret names.
enum Named {
    Name(&'static JoomlaName),
    Class(String),
}

struct Subject<'e> {
    cx: RefContext<'e>,
    named: Named,
    value: String,
    range: Range,
}

fn within(range: Range, pos: Pos) -> bool {
    range.start <= pos && pos <= range.end
}

/// The names the file `path` writes, its declared constants among them.
fn mentions(path: &Path, text: &str) -> Vec<Mention> {
    let facts = project::read(path, text);
    let mut out = facts.mentions;
    out.extend(facts.constants.into_iter().map(|def| Mention {
        what: What::Language,
        name: def.name,
        declares: true,
        range: def.range,
        prefixes: Vec::new(),
    }));
    out
}

impl PhpEngine {
    fn joomla_context(&self) -> Option<RefContext<'_>> {
        self.detected_frameworks().contains(Framework::Joomla).then(|| self.ref_context()).flatten()
    }

    /// Whether `path` is a file of a Joomla project that is served as a
    /// document: an XML file, a language file, an asset list.
    pub fn is_joomla_document(&self, path: &Path) -> bool {
        let served = path.extension().is_some_and(|ext| ext == "xml" || ext == "ini")
            || path.file_name().is_some_and(|name| name == "joomla.asset.json");
        served && self.detected_frameworks().contains(Framework::Joomla)
    }

    /// The class a field's type is: the first of its candidates the project has.
    fn joomla_field_class(&self, mention: &Mention) -> Option<String> {
        let resolver = self.resolver();
        let candidates = JoomlaProject::field_classes(&mention.name, &mention.prefixes);
        candidates.into_iter().find(|class| resolver.class_model(class).is_some())
    }

    fn joomla_subject(&self, path: &Path, text: &str, pos: Pos) -> Option<Subject<'_>> {
        if !self.is_joomla_document(path) {
            return None;
        }
        let cx = self.joomla_context()?;
        let found = mentions(path, text);
        let mention = found.iter().find(|mention| within(mention.range, pos))?;
        let named = match mention.what {
            What::FieldType => Named::Class(self.joomla_field_class(mention)?),
            What::Class => Named::Class(mention.name.clone()),
            what => Named::Name(kind_of(what)?),
        };
        Some(Subject { cx, named, value: mention.name.clone(), range: mention.range })
    }

    /// Where what the text at `pos` of the file at `path` names is
    /// declared. `None` outside a Joomla project and where the text names
    /// nothing.
    pub fn joomla_document_definition(&self, path: &Path, text: &str, pos: Pos) -> Option<Vec<Target>> {
        let subject = self.joomla_subject(path, text, pos)?;
        Some(match &subject.named {
            Named::Name(kind) => kind.resolve(&subject.cx, &subject.value, subject.value.len()),
            Named::Class(class) => self.class_targets(class),
        })
    }

    pub fn joomla_document_hover(&self, path: &Path, text: &str, pos: Pos) -> Option<HoverInfo> {
        let subject = self.joomla_subject(path, text, pos)?;
        let markdown = match &subject.named {
            Named::Name(kind) => kind.hover(&subject.cx, &subject.value)?,
            Named::Class(class) => {
                self.class_targets(class).first()?;
                format!("```php\n{class}\n```")
            }
        };
        Some(HoverInfo { markdown, range: subject.range })
    }

    /// The form fields whose type is the class `fqn`: the usages of a
    /// field class in the XML.
    pub(crate) fn joomla_class_usages(&self, fqn: &str) -> Vec<Usage> {
        if !self.detected_frameworks().contains(Framework::Joomla) || !fqn.ends_with("Field") {
            return Vec::new();
        }
        let project = self.joomla();
        let fqn = fqn.trim_start_matches('\\');
        let mut out = Vec::new();
        for (file, mention) in project.mentions().filter(|(_, mention)| mention.what == What::FieldType) {
            // By its candidates' names first: only a field that may be it is looked up.
            let candidates = JoomlaProject::field_classes(&mention.name, &mention.prefixes);
            if candidates.iter().any(|class| class.eq_ignore_ascii_case(fqn))
                && self.joomla_field_class(mention).is_some_and(|class| class.eq_ignore_ascii_case(fqn))
            {
                out.push(Usage { path: file.file.clone(), range: mention.range, kind: UsageKind::String });
            }
        }
        out
    }

    /// The usages of what the text at `pos` names, in PHP and in the other
    /// files.
    pub fn joomla_document_references(
        &self,
        path: &Path,
        text: &str,
        pos: Pos,
        include_declaration: bool,
        cancel: &CancellationToken,
    ) -> Option<IdeResult<Vec<Usage>>> {
        let subject = self.joomla_subject(path, text, pos)?;
        Some(match &subject.named {
            Named::Name(kind) => {
                let kind: &'static dyn ReferenceKind = *kind;
                self.kind_usages(&subject.cx, kind, &subject.value, &subject.value, include_declaration, cancel)
            }
            Named::Class(class) => {
                use crate::navigate::references::Sought;
                let sought = [Sought::Name { kind: crate::names::NameKind::ClassLike, fqn: class.clone() }];
                let here = crate::summary::FileSummary::default();
                self.find_usages(&sought, Path::new(""), &here, include_declaration, cancel).map(|mut usages| {
                    usages.extend(self.joomla_class_usages(class));
                    usages.sort_by_key(|usage| (crate::index::path_key(&usage.path), usage.range.start));
                    usages.dedup_by(|b, a| a.path == b.path && a.range == b.range);
                    usages
                })
            }
        })
    }

    /// The range a rename at `pos` changes; `Some(None)` when what is there
    /// can't be renamed.
    pub fn joomla_document_prepare_rename(&self, path: &Path, text: &str, pos: Pos) -> Option<Option<Range>> {
        let subject = self.joomla_subject(path, text, pos)?;
        let Named::Name(kind) = subject.named else { return Some(None) };
        Some(kind.rename_target(&subject.cx, &subject.value, 0).map(|_| subject.range))
    }

    /// Renames what the text at `pos` names: a language constant, in PHP,
    /// in the language files and in the XML.
    pub fn joomla_document_rename(
        &self,
        path: &Path,
        text: &str,
        pos: Pos,
        new_name: &str,
        cancel: &CancellationToken,
    ) -> Option<IdeResult<WorkspaceEdit>> {
        let subject = self.joomla_subject(path, text, pos)?;
        let Named::Name(kind) = subject.named else {
            return Some(Err(IdeError::invalid("this can't be renamed here")));
        };
        let Some(target) = kind.rename_target(&subject.cx, &subject.value, 0) else {
            return Some(Err(IdeError::invalid("this name can't be renamed")));
        };
        let kind: &'static dyn ReferenceKind = kind;
        Some(self.kind_rename(&subject.cx, kind, target, new_name, cancel))
    }

    /// The names that can be written at `pos`.
    pub fn joomla_document_completion(&self, path: &Path, text: &str, pos: Pos) -> Option<Vec<NameCompletion>> {
        if !self.is_joomla_document(path) {
            return None;
        }
        let cx = self.joomla_context()?;
        let at = |found: &[Mention]| found.iter().find(|mention| within(mention.range, pos)).cloned();
        // What is written so far names something, or would with two letters more.
        let (mention, range) = match at(&mentions(path, text)) {
            Some(mention) => {
                let range = mention.range;
                (mention, range)
            }
            None => {
                let offset = crate::syntax::LineIndex::new(text).byte(pos) as usize;
                let mut probe = String::with_capacity(text.len() + 2);
                probe.push_str(text.get(..offset)?);
                probe.push_str("XX");
                probe.push_str(&text[offset..]);
                let mention = at(&mentions(path, &probe))?;
                let mut range = mention.range;
                range.end.col = range.end.col.saturating_sub(2).max(range.start.col);
                (mention, range)
            }
        };
        let item = |label: String, detail: Option<String>| NameCompletion { label, detail, range };
        match mention.what {
            What::FieldType => {
                // The fields of the form's namespaces and Joomla's own, as the types they are.
                let resolver = self.resolver();
                let mut namespaces = mention.prefixes.clone();
                namespaces.push("Joomla\\CMS\\Form\\Field".to_owned());
                let mut out: Vec<NameCompletion> = Vec::new();
                for namespace in namespaces {
                    for class in resolver.namespace_classes(&namespace) {
                        let short = class.rsplit('\\').next().unwrap_or(&class);
                        if let Some(name) = short.strip_suffix("Field").filter(|name| !name.is_empty())
                            && !out.iter().any(|known| known.label.eq_ignore_ascii_case(name))
                        {
                            out.push(item(name.to_ascii_lowercase(), Some(class.clone())));
                        }
                    }
                }
                out.sort_by(|a, b| a.label.cmp(&b.label));
                Some(out)
            }
            What::Class => Some(self.class_names(&mention.name, range)),
            what => {
                let kind = kind_of(what)?;
                // By what is written before the caret (a language constant has thousands).
                let before = pos.col.saturating_sub(mention.range.start.col) as usize;
                let typed: String = mention.name.chars().take(before).collect();
                let typed = if std::ptr::eq(kind, &LANGUAGE_CONSTANT) && !ini::is_constant(&typed) && typed.len() < 2 {
                    String::new()
                } else {
                    typed
                };
                let items = kind.complete(&cx, &typed, typed.len());
                Some(items.into_iter().map(|found| item(found.label, found.detail)).collect())
            }
        }
    }
}
