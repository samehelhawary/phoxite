//! A Magento project's XML files as documents: what the text at the caret
//! names, for goto, hover, Find References, rename and completion there,
//! and what the file names that isn't there.
//!
//! The open text is read like the file on disk
//! ([`project::read`](super::project::read)); the caret is on one of the
//! names it writes ([`Mention`]):
//!
//! | name | where | answered by |
//! | --- | --- | --- |
//! | a class | `di.xml` (`preference`, `type`, `virtualType`, `plugin`, an object argument), `events.xml` (`observer instance`), layout (`block class`), `system.xml` (`source_model` and its kin), `webapi.xml`, `crontab.xml`, the queue files, UI components, a GraphQL schema's resolvers | the class: its declaration, its usages in PHP and in the other files, its rename |
//! | a method beside a class | `webapi.xml` (`service method`), `crontab.xml`, `communication.xml` | the method |
//! | a virtual type | where a `di.xml` declares or names one | its declaration, the places naming it |
//! | an event, an ACL resource, a configuration path, a template, a layout handle, a block, a module, a table, a route | `events.xml`, `acl.xml` (`system.xml`'s `<resource>`, `menu.xml`, `webapi.xml` too), `system.xml` and `config.xml`, layout (`template`, `<update handle>`, `referenceBlock`, `before`/`after`, `<move>`), `module.xml`, `db_schema.xml`, `routes.xml` | its reference kind ([`super::refs`]), as for a string of PHP |
//! | a UI component | layout (`<uiComponent name>`) | its file |
//!
//! Completion offers what the place takes: the namespaces and classes
//! below what is written, or the kind's names.
//!
//! Two things are reported ([`PhpEngine::magento_document_problems`]), in
//! the project's own files: a class that no file declares, Magento wouldn't
//! generate and no virtual type names (`MagentoXmlUndefinedClass`), where
//! its namespace is a module the project has; and a template whose file
//! isn't there (`MagentoXmlUndefinedTemplate`), where the project has its
//! module.

use std::path::Path;

use super::project::{self, FileFacts, Mention, What, XmlKind, area_of};
use super::refs::{MagentoName, kind_of};
use crate::base::{
    CancellationToken, Diagnostic, FileEdit, IdeError, IdeResult, Pos, Range, Severity, TextEdit, WorkspaceEdit,
};
use crate::engine::PhpEngine;
use crate::frameworks::Framework;
use crate::frameworks::drupal::documents::NameCompletion;
use crate::frameworks::refs::{RefContext, ReferenceKind};
use crate::navigate::{HoverInfo, Target, Usage, UsageKind};

/// The source of an XML file's diagnostics.
pub const DIAGNOSTIC_SOURCE: &str = "magento";
pub const UNDEFINED_CLASS: &str = "MagentoXmlUndefinedClass";
pub const UNDEFINED_TEMPLATE: &str = "MagentoXmlUndefinedTemplate";

/// What the text at the caret names.
enum Named {
    Name(&'static MagentoName),
    /// A class (FQN), and the method of it the caret is on.
    Class(String, Option<String>),
    VirtualType(String),
    Component(String),
}

struct Subject<'e> {
    cx: RefContext<'e>,
    named: Named,
    /// The name; a template's with its module.
    value: String,
    /// Where the name is written.
    range: Range,
    /// Whether what is written is the whole of `value` (a template written
    /// without its module isn't).
    whole: bool,
}

fn within(range: Range, pos: Pos) -> bool {
    range.start <= pos && pos <= range.end
}

/// The range of the last `length` columns of `range`.
fn tail(range: Range, length: usize) -> Range {
    let width = range.end.col.saturating_sub(range.start.col);
    Range::new(Pos::new(range.end.line, range.end.col - (length as u32).min(width)), range.end)
}

/// `name` spelled as `written` is: with its leading backslash, with its
/// backslashes doubled.
pub(crate) fn respell(written: &str, name: &str) -> String {
    let doubled = written.contains("\\\\");
    let separator = if doubled { "\\\\" } else { "\\" };
    let mut out = name.trim_start_matches('\\').replace('\\', separator);
    if written.starts_with('\\') {
        out.insert_str(0, separator);
    }
    out
}

impl PhpEngine {
    fn magento_context(&self) -> Option<RefContext<'_>> {
        self.detected_frameworks().contains(Framework::Magento).then(|| self.ref_context()).flatten()
    }

    /// Whether `path` is a file of a Magento project that is served as a
    /// document.
    pub fn is_magento_document(&self, path: &Path) -> bool {
        XmlKind::of(path).is_some() && self.detected_frameworks().contains(Framework::Magento)
    }

    /// What the open text of the file at `path` declares and names.
    fn magento_document_facts(&self, path: &Path, text: &str) -> FileFacts {
        let module = self.magento().module_name_of(path);
        project::read(path, text, module.as_deref())
    }

    fn magento_named(&self, path: &Path, mention: &Mention, pos: Pos) -> (Named, String, bool) {
        let project = self.magento();
        let on_method = mention.method.as_ref().filter(|(_, range)| within(*range, pos));
        match mention.what {
            What::Class => {
                let known = self.resolver().class_model(&mention.name).is_some();
                if !known && project.virtual_type(&mention.name).is_some() {
                    (Named::VirtualType(mention.name.clone()), mention.name.clone(), true)
                } else {
                    let method = on_method.map(|(method, _)| method.clone());
                    (Named::Class(mention.name.clone(), method), mention.name.clone(), true)
                }
            }
            What::Component => (Named::Component(mention.name.clone()), mention.name.clone(), true),
            what => {
                let kind = kind_of(what).expect("a kind for every other name");
                // A template written without its module is of the file's.
                if what == What::Template && !mention.name.contains("::") {
                    let id = match project.module_name_of(path) {
                        Some(module) => format!("{module}::{}", mention.name),
                        None => mention.name.clone(),
                    };
                    return (Named::Name(kind), id, false);
                }
                (Named::Name(kind), mention.name.clone(), true)
            }
        }
    }

    fn magento_subject(&self, path: &Path, text: &str, pos: Pos) -> Option<Subject<'_>> {
        let cx = self.magento_context()?;
        XmlKind::of(path)?;
        let facts = self.magento_document_facts(path, text);
        let on = |mention: &&Mention| {
            within(mention.range, pos) || mention.method.as_ref().is_some_and(|(_, range)| within(*range, pos))
        };
        let mention = facts.mentions.iter().find(on)?;
        let (named, value, whole) = self.magento_named(path, mention, pos);
        let range = match (&named, &mention.method) {
            (Named::Class(_, Some(_)), Some((_, range))) => *range,
            _ => mention.range,
        };
        Some(Subject { cx, named, value, range, whole })
    }

    /// The method `method` of `class`, where it is declared.
    fn magento_method_target(&self, class: &str, method: &str) -> Option<Target> {
        let resolver = self.resolver();
        let model = resolver.class_model(class)?;
        let found = model.method(method)?;
        let declared = found.declared()?;
        let path = self.class_path(&found.declaring, Path::new(""), &crate::summary::FileSummary::default())?;
        Some(Target { path, range: declared.name_loc.range })
    }

    /// Where what the text at `pos` of the file at `path` names is
    /// declared. `None` outside a Magento project and where the text names
    /// nothing.
    pub fn magento_document_definition(&self, path: &Path, text: &str, pos: Pos) -> Option<Vec<Target>> {
        let subject = self.magento_subject(path, text, pos)?;
        let project = self.magento();
        Some(match &subject.named {
            Named::Name(kind) => kind.resolve(&subject.cx, &subject.value, subject.value.len()),
            Named::Class(class, method) => {
                let method = method.as_deref().and_then(|method| self.magento_method_target(class, method));
                method.map_or_else(|| self.class_targets(class), |target| vec![target])
            }
            Named::VirtualType(name) => project
                .virtual_types
                .iter()
                .filter(|found| found.name == *name)
                .map(|found| Target { path: found.file.clone(), range: found.range })
                .collect(),
            Named::Component(name) => project
                .files
                .iter()
                .filter(|file| {
                    XmlKind::of(&file.file) == Some(XmlKind::UiComponent)
                        && file.file.file_stem().is_some_and(|stem| stem.to_string_lossy() == name.as_str())
                })
                .map(|file| Target { path: file.file.clone(), range: Range::default() })
                .collect(),
        })
    }

    pub fn magento_document_hover(&self, path: &Path, text: &str, pos: Pos) -> Option<HoverInfo> {
        let subject = self.magento_subject(path, text, pos)?;
        let project = self.magento();
        let area = area_of(path);
        let markdown = match &subject.named {
            Named::Name(kind) => kind.hover(&subject.cx, &subject.value)?,
            Named::Class(class, method) => {
                self.class_targets(class).first()?;
                let mut out = match method {
                    Some(method) => format!("```php\n{class}::{method}()\n```"),
                    None => format!("```php\n{class}\n```"),
                };
                if method.is_none() {
                    // What the object manager makes for it here, and what intercepts it.
                    if let Some(preference) = project.preferred(class, area.as_deref()) {
                        out.push_str(&format!("\n\nPreference: `{}`", preference.class));
                    }
                    let mut plugins: Vec<&str> = project
                        .plugins
                        .iter()
                        .filter(|plugin| plugin.target.eq_ignore_ascii_case(class))
                        .map(|plugin| plugin.class.as_str())
                        .collect();
                    plugins.dedup();
                    if !plugins.is_empty() {
                        out.push_str(&format!("\n\nPlugins: `{}`", plugins.join("`, `")));
                    }
                }
                out
            }
            Named::VirtualType(name) => {
                let class = project.virtual_class(name)?;
                format!("**virtual type** `{name}`\n\n`{class}`")
            }
            Named::Component(name) => format!("**UI component** `{name}`"),
        };
        Some(HoverInfo { markdown, range: subject.range })
    }

    /// The usages of the class `fqn`: in PHP and in the files of a Magento
    /// project that name it.
    pub(crate) fn magento_class_references(
        &self,
        fqn: &str,
        include_declaration: bool,
        cancel: &CancellationToken,
    ) -> IdeResult<Vec<Usage>> {
        use crate::navigate::references::Sought;
        let sought = [Sought::Name { kind: crate::names::NameKind::ClassLike, fqn: fqn.to_owned() }];
        let here = crate::summary::FileSummary::default();
        let mut usages = self.find_usages(&sought, Path::new(""), &here, include_declaration, cancel)?;
        usages.extend(self.magento_class_usages(fqn));
        usages.sort_by_key(|usage| (crate::index::path_key(&usage.path), usage.range.start));
        usages.dedup_by(|b, a| a.path == b.path && a.range == b.range);
        Ok(usages)
    }

    /// The usages of what the text at `pos` names, in PHP and in the XML.
    pub fn magento_document_references(
        &self,
        path: &Path,
        text: &str,
        pos: Pos,
        include_declaration: bool,
        cancel: &CancellationToken,
    ) -> Option<IdeResult<Vec<Usage>>> {
        let subject = self.magento_subject(path, text, pos)?;
        Some(match &subject.named {
            Named::Name(kind) => {
                let kind: &'static dyn ReferenceKind = *kind;
                self.kind_usages(&subject.cx, kind, &subject.value, &subject.value, include_declaration, cancel)
            }
            Named::Class(class, None) => self.magento_class_references(class, include_declaration, cancel),
            Named::Class(class, Some(method)) => {
                use crate::navigate::references::Sought;
                use crate::navigate::symbol::MemberKind;
                let resolver = self.resolver();
                let declaring = resolver
                    .class_model(class)
                    .and_then(|model| model.method(method).map(|found| found.declaring.fqn.clone()));
                let Some(declaring) = declaring else { return Some(Ok(Vec::new())) };
                let sought = [Sought::Member {
                    kind: MemberKind::Method,
                    name: method.clone(),
                    declaring: declaring.clone(),
                    private: false,
                }];
                let here = crate::summary::FileSummary::default();
                self.find_usages(&sought, Path::new(""), &here, include_declaration, cancel).map(|mut usages| {
                    usages.extend(self.magento_method_usages(&declaring, method));
                    usages.sort_by_key(|usage| (crate::index::path_key(&usage.path), usage.range.start));
                    usages.dedup_by(|b, a| a.path == b.path && a.range == b.range);
                    usages
                })
            }
            Named::VirtualType(name) => {
                let project = self.magento();
                let found = project.mentions_of(What::Class, name);
                Ok(found
                    .filter(|(_, mention)| include_declaration || !mention.declares)
                    .map(|(file, mention)| Usage {
                        path: file.file.clone(),
                        range: mention.range,
                        kind: if mention.declares { UsageKind::Declaration } else { UsageKind::String },
                    })
                    .collect())
            }
            Named::Component(_) => return None,
        })
    }

    /// The range a rename at `pos` changes; `Some(None)` when what is there
    /// can't be renamed.
    pub fn magento_document_prepare_rename(&self, path: &Path, text: &str, pos: Pos) -> Option<Option<Range>> {
        let subject = self.magento_subject(path, text, pos)?;
        Some(match &subject.named {
            Named::Name(kind) => kind.rename_target(&subject.cx, &subject.value, 0).map(|target| {
                if subject.whole { tail(subject.range, target.range.1 - target.range.0) } else { subject.range }
            }),
            // A class's short name.
            Named::Class(class, None) => {
                let declared = self.class_targets(class);
                let own = declared.first().is_some_and(|target| !self.is_library(&target.path));
                own.then(|| tail(subject.range, class.rsplit('\\').next().unwrap_or(class).len()))
            }
            Named::VirtualType(_) => Some(subject.range),
            Named::Class(_, Some(_)) | Named::Component(_) => None,
        })
    }

    /// Renames what the text at `pos` names: in PHP and in every file
    /// naming it, as they are saved.
    pub fn magento_document_rename(
        &self,
        path: &Path,
        text: &str,
        pos: Pos,
        new_name: &str,
        cancel: &CancellationToken,
    ) -> Option<IdeResult<WorkspaceEdit>> {
        let subject = self.magento_subject(path, text, pos)?;
        Some(match &subject.named {
            Named::Name(kind) => {
                let Some(target) = kind.rename_target(&subject.cx, &subject.value, 0) else {
                    return Some(Err(IdeError::invalid("this name can't be renamed")));
                };
                let kind: &'static dyn ReferenceKind = *kind;
                self.kind_rename(&subject.cx, kind, target, new_name, cancel)
            }
            Named::Class(class, None) => match self.class_targets(class).into_iter().next() {
                Some(declared) => self.rename_class(class, &declared.path, new_name, cancel),
                None => Err(IdeError::invalid("the class isn't declared in the project")),
            },
            Named::VirtualType(name) => {
                let valid = !new_name.is_empty()
                    && new_name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '\\'));
                if !valid || self.resolver().class_model(new_name).is_some() {
                    return Some(Err(IdeError::invalid("not a name a virtual type can have")));
                }
                let project = self.magento();
                let mut edit = WorkspaceEdit::default();
                for file in &project.files {
                    if self.is_library(&file.file) {
                        continue;
                    }
                    let named =
                        file.mentions.iter().filter(|mention| mention.what == What::Class && mention.name == *name);
                    let edits: Vec<TextEdit> =
                        named.map(|mention| TextEdit { range: mention.range, new_text: new_name.to_owned() }).collect();
                    if !edits.is_empty() {
                        edit.files.push(FileEdit::Edit { path: file.file.clone(), edits, version: None });
                    }
                }
                Ok(edit)
            }
            Named::Class(_, Some(_)) | Named::Component(_) => Err(IdeError::invalid("this can't be renamed here")),
        })
    }

    /// The names that can be written at `pos`: classes where a class is
    /// named, else the names of the kind the place takes.
    pub fn magento_document_completion(&self, path: &Path, text: &str, pos: Pos) -> Option<Vec<NameCompletion>> {
        let cx = self.magento_context()?;
        XmlKind::of(path)?;
        let at = |facts: &FileFacts| facts.mentions.iter().find(|mention| within(mention.range, pos)).cloned();
        // What is written so far names something, or would with a letter more.
        let (mention, range) = match at(&self.magento_document_facts(path, text)) {
            Some(mention) => {
                let range = mention.range;
                (mention, range)
            }
            None => {
                let offset = crate::syntax::LineIndex::new(text).byte(pos) as usize;
                let mut probe = String::with_capacity(text.len() + 1);
                probe.push_str(text.get(..offset)?);
                probe.push('X');
                probe.push_str(&text[offset..]);
                let mention = at(&self.magento_document_facts(path, &probe))?;
                let mut range = mention.range;
                range.end.col = range.end.col.saturating_sub(1).max(range.start.col);
                (mention, range)
            }
        };
        if mention.what == What::Class {
            // The namespaces and classes below what is written before the caret.
            let before = (pos.col.saturating_sub(mention.range.start.col)) as usize;
            let written: String = mention.name.chars().take(before).collect();
            let mut found = self.class_names(&written, range);
            let project = self.magento();
            let mut virtual_types: Vec<&str> = project.virtual_types.iter().map(|found| found.name.as_str()).collect();
            virtual_types.sort_unstable();
            virtual_types.dedup();
            found.extend(virtual_types.into_iter().map(|name| NameCompletion {
                label: name.to_owned(),
                detail: Some("virtual type".to_owned()),
                range,
            }));
            return Some(found);
        }
        let kind = kind_of(mention.what)?;
        let items = kind.complete(&cx, "", 0);
        Some(items.into_iter().map(|item| NameCompletion { label: item.label, detail: item.detail, range }).collect())
    }

    /// What the file at `path` names that isn't there; see the module
    /// documentation. `None` when the file isn't a Magento document.
    pub fn magento_document_problems(&self, path: &Path, text: &str) -> Option<Vec<Diagnostic>> {
        if !self.is_magento_document(path) {
            return None;
        }
        let mut out = Vec::new();
        if self.is_library(path) || self.status().state != crate::engine::EngineState::Ready {
            return Some(out);
        }
        let project = self.magento();
        let resolver = self.resolver();
        let facts = self.magento_document_facts(path, text);
        let own = project.module_name_of(path);
        // A file read only for what looks like a class: nothing is said of what it lacks.
        if XmlKind::of(path) == Some(XmlKind::Other) {
            return Some(out);
        }
        let mut report = |range: Range, code: &str, message: String| {
            out.push(Diagnostic {
                range,
                severity: Severity::Warning,
                message,
                source: Some(DIAGNOSTIC_SOURCE.into()),
                code: Some(code.into()),
                unnecessary: false,
                deprecated: false,
                server: None,
            });
        };
        for mention in &facts.mentions {
            match mention.what {
                What::Class if !mention.declares => {
                    let name = &mention.name;
                    // Only in the namespace of a module the project has: what a package
                    // that isn't installed declares is not known to be missing.
                    let mut parts = name.split('\\');
                    let module = match (parts.next(), parts.next(), parts.next()) {
                        (Some(vendor), Some(module), Some(_)) => format!("{vendor}_{module}"),
                        _ => continue,
                    };
                    let known = resolver.class_model(name).is_some()
                        || project.virtual_type(name).is_some()
                        || facts.virtual_types.iter().any(|found| found.name == *name)
                        || self.magento_generated(name).is_some();
                    if !known && project.module(&module).is_some() {
                        report(mention.range, UNDEFINED_CLASS, format!("Undefined class '{name}'"));
                    }
                }
                What::Template => {
                    let id = match (mention.name.contains("::"), &own) {
                        (false, Some(module)) => format!("{module}::{}", mention.name),
                        _ => mention.name.clone(),
                    };
                    let Some((module, file)) = id.split_once("::") else { continue };
                    let files = project.template_files(&id, None, None);
                    if file.ends_with(".phtml")
                        && project.module(module).is_some()
                        && !files.iter().any(|file| file.is_file())
                    {
                        report(mention.range, UNDEFINED_TEMPLATE, format!("Unknown template '{}'", mention.name));
                    }
                }
                _ => {}
            }
        }
        Some(out)
    }

    /// The places the files of a Magento project name the class `fqn`.
    pub(crate) fn magento_class_usages(&self, fqn: &str) -> Vec<Usage> {
        if !self.detected_frameworks().contains(Framework::Magento) {
            return Vec::new();
        }
        let project = self.magento();
        let found = project.mentions_of(What::Class, fqn).filter(|(_, mention)| !mention.declares);
        found
            .map(|(file, mention)| Usage { path: file.file.clone(), range: mention.range, kind: UsageKind::String })
            .collect()
    }

    /// The edits that give the class `fqn` the short name `new` in the
    /// files of a Magento project naming it.
    pub(crate) fn magento_class_rename_edits(&self, fqn: &str, new: &str) -> Vec<(std::path::PathBuf, TextEdit)> {
        if !self.detected_frameworks().contains(Framework::Magento) {
            return Vec::new();
        }
        let short = fqn.rsplit('\\').next().unwrap_or(fqn);
        let project = self.magento();
        let found = project.mentions_of(What::Class, fqn).filter(|(_, mention)| !mention.declares);
        found
            .filter(|(file, _)| !self.is_library(&file.file))
            .map(|(file, mention)| {
                (file.file.clone(), TextEdit { range: tail(mention.range, short.len()), new_text: new.to_owned() })
            })
            .collect()
    }

    /// The edits that make the files of a Magento project name the class
    /// `old` as `new` (both fully qualified): a class that moved.
    pub(crate) fn magento_class_move_edits(&self, old: &str, new: &str) -> Vec<(std::path::PathBuf, TextEdit)> {
        if !self.detected_frameworks().contains(Framework::Magento) {
            return Vec::new();
        }
        let project = self.magento();
        let mut out = Vec::new();
        for (file, mention) in project.mentions_of(What::Class, old).filter(|(_, mention)| !mention.declares) {
            if self.is_library(&file.file) {
                continue;
            }
            // As it is written: with its leading backslash, with its backslashes doubled.
            let name = if mention.escaped { mention.name.replace('\\', "\\\\") } else { mention.name.clone() };
            let width = (mention.range.end.col - mention.range.start.col) as usize;
            let leading = if width > name.len() { if mention.escaped { "\\\\" } else { "\\" } } else { "" };
            let written = format!("{leading}{name}");
            out.push((file.file.clone(), TextEdit { range: mention.range, new_text: respell(&written, new) }));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::respell;

    #[test]
    fn a_class_is_spelled_as_it_was_written() {
        assert_eq!(respell("Shop\\Model\\Cart", "Shop\\Model\\Basket"), "Shop\\Model\\Basket");
        assert_eq!(respell("\\Shop\\Model\\Cart", "Shop\\Model\\Basket"), "\\Shop\\Model\\Basket");
        assert_eq!(respell("\\\\Shop\\\\Model\\\\Cart", "Shop\\Model\\Basket"), "\\\\Shop\\\\Model\\\\Basket");
    }
}
