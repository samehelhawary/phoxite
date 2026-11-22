//! What the files of a Joomla site, or of an extension opened on its own,
//! declare ([`JoomlaProject`]):
//!
//! - **Extensions:** components (`components/com_*`,
//!   `administrator/components/com_*`, `api/components/com_*`), modules
//!   (`modules/mod_*`, `administrator/modules/mod_*`), plugins
//!   (`plugins/<group>/<name>`) and templates (`templates/*`,
//!   `administrator/templates/*`), each with the namespace its manifest
//!   gives it (`<namespace path="src">`): a component's and a module's get
//!   the client after it (`\Site`, `\Administrator`, `\Api`). A project
//!   whose own folder has a manifest is that extension, laid out as its
//!   package is (`site/`, `admin/`, or `src/` at once).
//! - **Language constants:** the `KEY="value"` lines of `language/<tag>/*.ini`
//!   (the site's, the administrator's, the API's, an extension's own).
//! - **Layouts:** the files of `layouts/` (`joomla.content.tags` is
//!   `layouts/joomla/content/tags.php`), a component's `layouts/`, and a
//!   template's overrides in `html/layouts/`.
//! - **Web assets:** the names of each `joomla.asset.json`, with their type
//!   and their dependencies.
//! - **ACL actions** (`access.xml`), **forms** (`forms/*.xml`, with their
//!   fields) and **parameters** (the fields of `config.xml` and of a
//!   manifest's `<config>`).
//! - **Events:** the event class of each name, from the map of
//!   `Joomla\CMS\Event\CoreEventAware`.
//! - **Tables:** the `CREATE TABLE` statements of the MySQL scripts
//!   (`installation/sql/mysql`, an extension's `sql/`), with their columns.
//! - **Every name an XML file writes** ([`Mention`]): classes, language
//!   constants, field types and layouts, for the file as a document
//!   ([`super::documents`]) and for usages.
//!
//! Each file's declarations are kept until the file changes
//! ([`JoomlaCache`]); the project is put together again only when one of
//! these files is written. The behaviour follows Joomla's manual.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use super::ini;
use crate::base::Range;
use crate::base::xml::{self, Element};
use crate::syntax::LineIndex;

/// A name a file declares.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Def {
    pub name: String,
    /// A constant's text, an action's title, a field's type.
    pub detail: Option<String>,
    pub file: PathBuf,
    pub range: Range,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExtensionKind {
    Component,
    Module,
    Plugin,
    Template,
}

/// A folder of an extension: a component has one per client.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Extension {
    pub kind: ExtensionKind,
    /// `com_content`, `mod_custom`, `pagebreak`, `cassiopeia`.
    pub name: String,
    /// A plugin's group (`content`).
    pub group: Option<String>,
    /// `Site`, `Administrator` or `Api`: the client the folder is of.
    pub client: Option<&'static str>,
    pub path: PathBuf,
    /// The namespace of its classes, the client's part included
    /// (`Joomla\Component\Content\Administrator`).
    pub namespace: Option<String>,
    /// Where its classes are, in the folder (`src`).
    pub source: String,
    pub manifest: Option<PathBuf>,
}

/// A web asset of a `joomla.asset.json`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Asset {
    pub name: String,
    /// `script`, `style` or `preset`.
    pub kind: String,
    pub dependencies: Vec<(String, Range)>,
    pub file: PathBuf,
    pub range: Range,
}

/// A form: an XML file of a `forms` folder, a `config.xml`, a manifest's
/// `<config>`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Form {
    /// The file's name without its extension (`article`).
    pub name: String,
    /// Whether its fields are parameters (`config.xml`, a manifest).
    pub params: bool,
    /// Each field: its name, its type, and where the name is written.
    pub fields: Vec<(String, String, Range)>,
    pub file: PathBuf,
}

/// A table of a MySQL script.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Table {
    /// With its prefix: `#__content`.
    pub name: String,
    /// Each column, its type, and where its name is written.
    pub columns: Vec<(String, String, Range)>,
    pub file: PathBuf,
    pub range: Range,
}

/// What a [`Mention`] names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum What {
    Class,
    Language,
    Layout,
    /// A form field's type: `text`, `modal_article`.
    FieldType,
    Asset,
    Action,
    Extension,
}

/// A name written in an XML or an asset file, and where.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mention {
    pub what: What,
    pub name: String,
    pub declares: bool,
    pub range: Range,
    /// For a field's type: the namespaces its `addfieldprefix` gives.
    pub prefixes: Vec<String>,
}

/// A manifest: its type, its plugin group, and its site and administrator folders.
pub type Manifest = (String, Option<String>, Option<String>, Option<String>);

/// What one file declares and names.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FileFacts {
    pub file: PathBuf,
    pub constants: Vec<Def>,
    pub assets: Vec<Asset>,
    pub actions: Vec<Def>,
    pub forms: Vec<Form>,
    pub tables: Vec<Table>,
    /// The namespace of a manifest and the folder its classes are in.
    pub namespace: Option<(String, String)>,
    /// A manifest's type, its plugin group, and its client folders
    /// (`<files folder="site">`, `<administration><files folder="admin">`).
    pub manifest: Option<Manifest>,
    pub mentions: Vec<Mention>,
}

/// See the module documentation.
#[derive(Clone, Debug, Default)]
pub struct JoomlaProject {
    pub root: PathBuf,
    pub extensions: Vec<Extension>,
    /// Language constants, with the files declaring each.
    pub constants: Vec<Def>,
    constant_index: HashMap<String, Vec<usize>>,
    /// Layouts: their id and their file, an override before what it
    /// overrides.
    pub layouts: Vec<(String, PathBuf)>,
    pub assets: Vec<Asset>,
    pub actions: Vec<Def>,
    pub forms: Vec<Form>,
    /// Events: their name and their class.
    pub events: Vec<(String, String)>,
    pub tables: Vec<Table>,
    pub files: Vec<Arc<FileFacts>>,
}

fn class(name: &str) -> String {
    name.trim().trim_start_matches('\\').to_owned()
}

fn is_class_name(name: &str) -> bool {
    let name = name.trim_start_matches('\\');
    name.contains('\\')
        && !name.ends_with('\\')
        && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '\\'))
}

/// The attributes whose value is a language constant when it is written
/// like one.
const TEXTS: [&str; 9] =
    ["label", "description", "title", "hint", "message", "translate_label", "translate_description", "text", "header"];

struct Reader<'a> {
    file: &'a Path,
    out: FileFacts,
}

impl Reader<'_> {
    fn mention(&mut self, what: What, name: &str, range: Range, declares: bool, prefixes: Vec<String>) {
        let name = if what == What::Class { class(name) } else { name.trim().to_owned() };
        if !name.is_empty() {
            self.out.mentions.push(Mention { what, name, declares, range, prefixes });
        }
    }

    /// The fields below `element`, with the field prefixes in force.
    fn fields(&mut self, element: &Element, prefixes: &[String], out: &mut Vec<(String, String, Range)>) {
        let mut prefixes = prefixes.to_vec();
        if let Some(prefix) = element.attribute("addfieldprefix") {
            self.mention(What::Class, &prefix.value, prefix.range, false, Vec::new());
            prefixes.insert(0, class(&prefix.value));
        }
        for attribute in &element.attributes {
            match attribute.name.as_str() {
                name if TEXTS.contains(&name) && ini::is_constant(&attribute.value) => {
                    self.mention(What::Language, &attribute.value, attribute.range, false, Vec::new());
                }
                "layout" if attribute.value.contains('.') => {
                    self.mention(What::Layout, &attribute.value, attribute.range, false, Vec::new());
                }
                "addruleprefix" | "addformprefix" => {
                    self.mention(What::Class, &attribute.value, attribute.range, false, Vec::new());
                }
                "validate" | "class" | "filter" if is_class_name(&attribute.value) => {
                    self.mention(What::Class, &attribute.value, attribute.range, false, Vec::new());
                }
                _ => {}
            }
        }
        if element.name == "field"
            && let Some(name) = element.attribute("name")
        {
            let ty = element.attribute("type");
            if let Some(ty) = ty {
                self.mention(What::FieldType, &ty.value, ty.range, false, prefixes.clone());
            }
            out.push((name.value.clone(), ty.map(|ty| ty.value.clone()).unwrap_or_default(), name.range));
        }
        if element.name == "option" && ini::is_constant(&element.text) {
            self.mention(What::Language, &element.text, element.text_range, false, Vec::new());
        }
        for child in &element.children {
            self.fields(child, &prefixes, out);
        }
    }

    fn form(&mut self, root: &Element, params: bool) {
        let mut fields = Vec::new();
        self.fields(root, &[], &mut fields);
        let name = self.file.file_stem().map(|stem| stem.to_string_lossy().into_owned()).unwrap_or_default();
        self.out.forms.push(Form { name, params, fields, file: self.file.to_path_buf() });
    }

    fn access(&mut self, root: &Element) {
        if let Some(component) = root.attribute("component") {
            self.mention(What::Extension, &component.value, component.range, false, Vec::new());
        }
        for action in root.descendants().into_iter().filter(|element| element.name == "action") {
            let Some(name) = action.attribute("name") else { continue };
            self.mention(What::Action, &name.value, name.range, true, Vec::new());
            if let Some(title) = action.attribute("title").filter(|title| ini::is_constant(&title.value)) {
                self.mention(What::Language, &title.value, title.range, false, Vec::new());
            }
            self.out.actions.push(Def {
                name: name.value.clone(),
                detail: action.attr("title").map(str::to_owned),
                file: self.file.to_path_buf(),
                range: name.range,
            });
        }
    }

    fn manifest(&mut self, root: &Element) {
        let kind = root.attr("type").unwrap_or_default().to_owned();
        let site = root.child("files").and_then(|files| files.attr("folder")).map(str::to_owned);
        let admin = root
            .child("administration")
            .and_then(|administration| administration.child("files"))
            .and_then(|files| files.attr("folder"))
            .map(str::to_owned);
        self.out.manifest = Some((kind, root.attr("group").map(str::to_owned), site, admin));
        if let Some(namespace) = root.child("namespace")
            && !namespace.text.is_empty()
        {
            self.mention(What::Class, &namespace.text, namespace.text_range, true, Vec::new());
            let path = namespace.attr("path").unwrap_or("src").to_owned();
            self.out.namespace = Some((class(&namespace.text), path));
        }
        // The texts that are language constants: the name, the description, a menu's.
        for element in root.descendants() {
            if element.children.is_empty() && ini::is_constant(&element.text) {
                self.mention(What::Language, &element.text, element.text_range, false, Vec::new());
            }
            for attribute in &element.attributes {
                if TEXTS.contains(&attribute.name.as_str())
                    && ini::is_constant(&attribute.value)
                    && element.name != "field"
                {
                    self.mention(What::Language, &attribute.value, attribute.range, false, Vec::new());
                }
            }
        }
        // Its parameters.
        for config in root.named("config") {
            let mut fields = Vec::new();
            for child in &config.children {
                self.fields(child, &[], &mut fields);
            }
            let name = self.file.file_stem().map(|stem| stem.to_string_lossy().into_owned()).unwrap_or_default();
            self.out.forms.push(Form { name, params: true, fields, file: self.file.to_path_buf() });
        }
    }

    /// A `joomla.asset.json`: the names of its assets and of what each
    /// depends on.
    fn assets(&mut self, text: &str) {
        let lines = LineIndex::new(text);
        let Some(start) = text.find("\"assets\"") else { return };
        // The string after the key at `at`, and where it is written.
        let string_after = |at: usize| -> Option<(usize, usize)> {
            let rest = &text[at..];
            let colon = rest.find(':')?;
            let open = rest[colon..].find('"')? + colon + 1;
            let close = rest[open..].find('"')? + open;
            rest[colon + 1..open - 1].trim().is_empty().then_some((at + open, at + close))
        };
        let mut from = start;
        while let Some(found) = text[from..].find("\"name\"") {
            let at = from + found;
            from = at + 6;
            let Some((open, close)) = string_after(at + 6) else { continue };
            let name = &text[open..close];
            let range = lines.range(open as u32, close as u32);
            // Up to the next asset: its type and what it depends on.
            let end = text[close..].find("\"name\"").map_or(text.len(), |next| close + next);
            let body = &text[close..end];
            let kind = body.find("\"type\"").and_then(|at| string_after(close + at + 6));
            let kind = kind.map(|(open, close)| text[open..close].to_owned()).unwrap_or_default();
            let mut dependencies = Vec::new();
            if let Some(list) = body.find("\"dependencies\"") {
                let list = close + list;
                let stop = text[list..end].find(']').map_or(end, |stop| list + stop);
                let mut at = text[list..stop].find('[').map_or(stop, |open| list + open);
                while let Some(open) = text[at..stop].find('"') {
                    let open = at + open + 1;
                    let Some(length) = text[open..stop].find('"') else { break };
                    let written = &text[open..open + length];
                    // (`core#script`: a dependency of one type.)
                    let dependency = written.split('#').next().unwrap_or(written);
                    let range = lines.range(open as u32, (open + dependency.len()) as u32);
                    dependencies.push((dependency.to_owned(), range));
                    self.mention(What::Asset, dependency, range, false, Vec::new());
                    at = open + length + 1;
                }
            }
            self.mention(What::Asset, name, range, true, Vec::new());
            self.out.assets.push(Asset {
                name: name.to_owned(),
                kind,
                dependencies,
                file: self.file.to_path_buf(),
                range,
            });
        }
    }

    /// The `CREATE TABLE` statements of a MySQL script.
    fn sql(&mut self, text: &str) {
        let lines = LineIndex::new(text);
        let mut from = 0;
        while let Some(found) = text[from..].find("CREATE TABLE") {
            let at = from + found;
            from = at + 12;
            let Some(open) = text[at..].find('`').map(|open| at + open + 1) else { break };
            let Some(length) = text[open..].find('`') else { break };
            let name = &text[open..open + length];
            if !name.starts_with("#__") {
                continue;
            }
            let end = text[open..].find(";\n").map_or(text.len(), |end| open + end);
            let mut columns = Vec::new();
            let mut offset = open + length;
            for line in text[offset..end].split_inclusive('\n') {
                let start = offset;
                offset += line.len();
                let indent = line.len() - line.trim_start().len();
                let Some(rest) = line.trim_start().strip_prefix('`') else { continue };
                let Some(length) = rest.find('`') else { continue };
                let ty = rest[length + 1..].split_whitespace().next().unwrap_or_default();
                let from = start + indent + 1;
                columns.push((
                    rest[..length].to_owned(),
                    ty.trim_end_matches(',').to_owned(),
                    lines.range(from as u32, (from + length) as u32),
                ));
            }
            self.out.tables.push(Table {
                name: name.to_owned(),
                columns,
                file: self.file.to_path_buf(),
                range: lines.range(open as u32, (open + length) as u32),
            });
        }
    }
}

/// Whether the file at `path` is one the model reads, by its name.
pub fn is_read(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else { return false };
    name.ends_with(".xml") || name.ends_with(".ini") || name.ends_with(".sql") || name == "joomla.asset.json"
}

/// What the file `path` declares and names.
pub fn read(path: &Path, text: &str) -> FileFacts {
    let mut reader = Reader { file: path, out: FileFacts { file: path.to_path_buf(), ..Default::default() } };
    let name = path.file_name().and_then(|name| name.to_str()).unwrap_or_default();
    if name.ends_with(".ini") {
        reader.out.constants = ini::read(text)
            .into_iter()
            .map(|entry| Def {
                name: entry.key,
                detail: Some(entry.value),
                file: path.to_path_buf(),
                range: entry.range,
            })
            .collect();
    } else if name == "joomla.asset.json" {
        reader.assets(text);
    } else if name.ends_with(".sql") {
        reader.sql(text);
    } else if name.ends_with(".xml")
        && let Some(root) = xml::root(text)
    {
        match root.name.as_str() {
            "extension" | "install" => reader.manifest(&root),
            "access" => reader.access(&root),
            "form" => reader.form(&root, false),
            "config" => reader.form(&root, true),
            // A menu item's options, a layout's.
            "metadata" | "layout" => {
                let mut fields = Vec::new();
                reader.fields(&root, &[], &mut fields);
            }
            _ => {}
        }
    }
    reader.out
}

/// The events of `Joomla\CMS\Event\CoreEventAware`: each
/// `'onName' => Namespace\Class::class` of its map.
pub fn events(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix('\'') else { continue };
        let Some((name, rest)) = rest.split_once('\'') else { continue };
        let Some(value) = rest.trim_start().strip_prefix("=>") else { continue };
        let Some(class) = value.trim().strip_suffix("::class,").or_else(|| value.trim().strip_suffix("::class")) else {
            continue;
        };
        if name.starts_with("on") {
            out.push((name.to_owned(), format!("Joomla\\CMS\\Event\\{}", class.trim_start_matches('\\'))));
        }
    }
    out
}

fn folders(folder: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(folder)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir() && path.file_name().is_some_and(|name| !name.to_string_lossy().starts_with('.')))
        .collect();
    found.sort();
    found
}

fn files(folder: &Path, extension: &str) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(folder)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_file() && path.extension().is_some_and(|ext| ext == extension))
        .collect();
    found.sort();
    found
}

/// The `.php` files below `folder`, as dotted ids (`joomla.content.tags`).
fn layouts_below(folder: &Path, above: &str, depth: usize, out: &mut Vec<(String, PathBuf)>) {
    if depth > 8 {
        return;
    }
    for file in files(folder, "php") {
        let Some(stem) = file.file_stem().map(|stem| stem.to_string_lossy().into_owned()) else { continue };
        out.push((if above.is_empty() { stem } else { format!("{above}.{stem}") }, file));
    }
    for folder in folders(folder) {
        let Some(name) = folder.file_name().map(|name| name.to_string_lossy().into_owned()) else { continue };
        let id = if above.is_empty() { name } else { format!("{above}.{name}") };
        layouts_below(&folder, &id, depth + 1, out);
    }
}

/// What each file declares, kept until the file changes, and the project
/// put together from them, kept until [`JoomlaCache::forget`].
#[derive(Default)]
pub struct JoomlaCache {
    project: Option<Arc<JoomlaProject>>,
    files: HashMap<PathBuf, (SystemTime, Arc<FileFacts>)>,
}

impl JoomlaCache {
    pub fn forget(&mut self) {
        self.project = None;
    }

    pub fn project(&mut self, root: &Path) -> Arc<JoomlaProject> {
        if let Some(project) = &self.project {
            return project.clone();
        }
        let project = Arc::new(self.load(root));
        self.project = Some(project.clone());
        project
    }

    fn facts(&mut self, path: &Path) -> Option<Arc<FileFacts>> {
        let modified = std::fs::metadata(path).and_then(|metadata| metadata.modified()).ok()?;
        if let Some((at, known)) = self.files.get(path)
            && *at == modified
        {
            return Some(known.clone());
        }
        let text = std::fs::read_to_string(path).ok()?;
        let facts = Arc::new(read(path, &text));
        self.files.insert(path.to_path_buf(), (modified, facts.clone()));
        Some(facts)
    }

    /// The manifest of the extension folder `folder`: the XML file at its
    /// top that is one.
    fn manifest(&mut self, folder: &Path) -> Option<Arc<FileFacts>> {
        files(folder, "xml").into_iter().find_map(|file| self.facts(&file).filter(|facts| facts.manifest.is_some()))
    }

    fn load(&mut self, root: &Path) -> JoomlaProject {
        let mut out = JoomlaProject { root: root.to_path_buf(), ..JoomlaProject::default() };
        // The extension folders of a site: each with its kind, its client and where its
        // manifest is when it isn't in the folder itself (a component's site folder).
        let mut found: Vec<(ExtensionKind, Option<&'static str>, Option<String>, PathBuf)> = Vec::new();
        for (base, client) in [("", "Site"), ("administrator", "Administrator"), ("api", "Api")] {
            let base = if base.is_empty() { root.to_path_buf() } else { root.join(base) };
            for folder in folders(&base.join("components")) {
                found.push((ExtensionKind::Component, Some(client), None, folder));
            }
            for folder in folders(&base.join("modules")) {
                found.push((ExtensionKind::Module, Some(client), None, folder));
            }
            if client != "Api" {
                for folder in folders(&base.join("templates")) {
                    found.push((ExtensionKind::Template, Some(client), None, folder));
                }
            }
        }
        for group in folders(&root.join("plugins")) {
            let name = group.file_name().map(|name| name.to_string_lossy().into_owned());
            for folder in folders(&group) {
                found.push((ExtensionKind::Plugin, None, name.clone(), folder));
            }
        }
        for (kind, client, group, path) in found {
            let name = path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
            // A component's manifest is in its administrator folder.
            let manifest = self.manifest(&path).or_else(|| {
                (kind == ExtensionKind::Component)
                    .then(|| self.manifest(&root.join("administrator").join("components").join(&name)))
                    .flatten()
            });
            let declared = manifest.as_ref().and_then(|manifest| manifest.namespace.clone());
            let namespace = declared.as_ref().map(|(namespace, _)| match (kind, client) {
                (ExtensionKind::Component | ExtensionKind::Module, Some(client)) => format!("{namespace}\\{client}"),
                _ => namespace.clone(),
            });
            out.extensions.push(Extension {
                kind,
                name,
                group,
                client,
                namespace,
                source: declared.map_or_else(|| "src".to_owned(), |(_, source)| source),
                manifest: manifest.map(|manifest| manifest.file.clone()),
                path,
            });
        }
        // The project itself: an extension as its package has it.
        if let Some(manifest) = self.manifest(root)
            && let Some((kind, group, site, admin)) = manifest.manifest.clone()
        {
            let kind = match kind.as_str() {
                "component" => Some(ExtensionKind::Component),
                "module" => Some(ExtensionKind::Module),
                "plugin" => Some(ExtensionKind::Plugin),
                "template" => Some(ExtensionKind::Template),
                _ => None,
            };
            let stem = manifest.file.file_stem().map(|stem| stem.to_string_lossy().into_owned()).unwrap_or_default();
            let (namespace, source) = manifest.namespace.clone().unzip();
            let source = source.unwrap_or_else(|| "src".to_owned());
            if let Some(kind) = kind {
                let mut add = |client: Option<&'static str>, folder: Option<String>, name: String| {
                    let path = folder.map_or_else(|| root.to_path_buf(), |folder| root.join(folder));
                    let namespace = namespace.as_ref().map(|namespace| match client {
                        Some(client) if kind != ExtensionKind::Plugin => format!("{namespace}\\{client}"),
                        _ => namespace.clone(),
                    });
                    out.extensions.push(Extension {
                        kind,
                        name,
                        group: group.clone(),
                        client,
                        path,
                        namespace,
                        source: source.clone(),
                        manifest: Some(manifest.file.clone()),
                    });
                };
                match kind {
                    ExtensionKind::Component => {
                        let name = if stem.starts_with("com_") { stem } else { format!("com_{stem}") };
                        add(Some("Site"), site.or_else(|| Some("site".to_owned())), name.clone());
                        add(Some("Administrator"), admin.or_else(|| Some("admin".to_owned())), name);
                    }
                    ExtensionKind::Module => add(Some("Site"), site, stem),
                    _ => add(None, site, stem),
                }
            }
        }

        // The files read: the language files, the layouts, and each extension's own.
        let mut read: Vec<PathBuf> = Vec::new();
        let mut language = vec![root.join("language"), root.join("administrator/language"), root.join("api/language")];
        let mut layouts: Vec<PathBuf> = Vec::new();
        for extension in &out.extensions {
            language.push(extension.path.join("language"));
            read.extend(files(&extension.path, "xml"));
            read.extend(files(&extension.path.join("forms"), "xml"));
            read.extend(files(&extension.path.join("sql"), "sql"));
            read.extend(files(&extension.path.join("sql/mysql"), "sql"));
            read.extend(files(&extension.path.join("sql/install/mysql"), "sql"));
            read.push(extension.path.join("joomla.asset.json"));
            for view in folders(&extension.path.join("tmpl")) {
                read.extend(files(&view, "xml"));
            }
            // A template's overrides come before what they override.
            if extension.kind == ExtensionKind::Template {
                layouts.insert(0, extension.path.join("html/layouts"));
            } else {
                layouts.push(extension.path.join("layouts"));
            }
        }
        language.push(root.join("site/language"));
        language.push(root.join("admin/language"));
        for folder in language {
            for tag in folders(&folder) {
                read.extend(files(&tag, "ini"));
            }
        }
        for media in folders(&root.join("media")).into_iter().chain(folders(&root.join("media_source"))) {
            read.push(media.join("joomla.asset.json"));
        }
        read.push(root.join("joomla.asset.json"));
        read.extend(files(&root.join("installation/sql/mysql"), "sql"));
        layouts.push(root.join("layouts"));
        layouts.push(root.join("administrator/layouts"));
        for folder in layouts {
            layouts_below(&folder, "", 0, &mut out.layouts);
        }

        let mut seen: std::collections::HashSet<PathBuf> = std::collections::HashSet::new();
        for file in read {
            if !seen.insert(file.clone()) {
                continue;
            }
            let Some(facts) = self.facts(&file) else { continue };
            out.constants.extend(facts.constants.iter().cloned());
            out.assets.extend(facts.assets.iter().cloned());
            out.actions.extend(facts.actions.iter().cloned());
            out.forms.extend(facts.forms.iter().cloned());
            out.tables.extend(facts.tables.iter().cloned());
            out.files.push(facts);
        }
        for (at, constant) in out.constants.iter().enumerate() {
            out.constant_index.entry(constant.name.clone()).or_default().push(at);
        }
        let map = root.join("libraries/src/Event/CoreEventAware.php");
        out.events = std::fs::read_to_string(map).map(|text| events(&text)).unwrap_or_default();
        out
    }
}

impl JoomlaProject {
    /// The declarations of the language constant `name`: the British
    /// English ones first.
    pub fn constant(&self, name: &str) -> Vec<&Def> {
        let found = self.constant_index.get(&name.to_ascii_uppercase());
        let mut found: Vec<&Def> = found.into_iter().flatten().map(|at| &self.constants[*at]).collect();
        found.sort_by_key(|def| !def.file.components().any(|part| part.as_os_str() == "en-GB"));
        found
    }

    /// The extension folder the file `path` is in.
    pub fn extension_of(&self, path: &Path) -> Option<&Extension> {
        let inside = self.extensions.iter().filter(|extension| path.starts_with(&extension.path));
        inside.max_by_key(|extension| extension.path.as_os_str().len())
    }

    /// The folders of the extension named `name` (`com_content`; a plugin
    /// as `plg_content_pagebreak` or by its name).
    pub fn extension(&self, name: &str) -> Vec<&Extension> {
        self.extensions.iter().filter(|extension| extension.is_named(name)).collect()
    }

    /// The extension whose classes are in the namespace of `class`.
    pub fn extension_of_class(&self, class: &str) -> Option<&Extension> {
        let class = class.trim_start_matches('\\');
        let has = |extension: &&Extension| {
            extension.namespace.as_deref().is_some_and(|namespace| {
                class.len() > namespace.len()
                    && class[..namespace.len()].eq_ignore_ascii_case(namespace)
                    && class.as_bytes()[namespace.len()] == b'\\'
            })
        };
        self.extensions.iter().filter(has).max_by_key(|extension| extension.namespace.as_ref().map_or(0, String::len))
    }

    /// The files the layout `id` may be: an override first.
    pub fn layout(&self, id: &str) -> Vec<&PathBuf> {
        self.layouts.iter().filter(|(known, _)| known == id).map(|(_, file)| file).collect()
    }

    pub fn event_class(&self, name: &str) -> Option<&str> {
        self.events.iter().find(|(known, _)| known.eq_ignore_ascii_case(name)).map(|(_, class)| class.as_str())
    }

    /// The table `name`, written with its prefix (`#__content`).
    pub fn table(&self, name: &str) -> Option<&Table> {
        self.tables.iter().find(|table| table.name == name)
    }

    /// The classes a form field of type `ty` may be, the likeliest first:
    /// in the namespaces its form adds, then Joomla's own. `modal_article`
    /// is `Modal\ArticleField`.
    pub fn field_classes(ty: &str, prefixes: &[String]) -> Vec<String> {
        let studly = |part: &str| {
            let mut chars = part.chars();
            chars.next().map_or_else(String::new, |first| first.to_ascii_uppercase().to_string() + chars.as_str())
        };
        let name = ty.split('_').map(studly).collect::<Vec<_>>().join("\\");
        let mut out: Vec<String> = prefixes.iter().map(|prefix| format!("{prefix}\\{name}Field")).collect();
        out.push(format!("Joomla\\CMS\\Form\\Field\\{name}Field"));
        out
    }

    /// Every name the XML and asset files write, with the file it is in.
    pub fn mentions(&self) -> impl Iterator<Item = (&FileFacts, &Mention)> {
        self.files.iter().flat_map(|file| file.mentions.iter().map(move |mention| (file.as_ref(), mention)))
    }

    /// The view template `path` is: its extension, the view's folder and
    /// the layout (`components/com_content/tmpl/article/default.php`).
    pub fn view_template<'p>(&'p self, path: &Path) -> Option<(&'p Extension, String, String)> {
        let extension = self.extension_of(path)?;
        let below = path.strip_prefix(extension.path.join("tmpl")).ok()?;
        let parts: Vec<String> =
            below.components().map(|part| part.as_os_str().to_string_lossy().into_owned()).collect();
        let [view, file] = parts.as_slice() else { return None };
        let layout = file.strip_suffix(".php")?;
        Some((extension, view.clone(), layout.to_owned()))
    }

    /// Whether `path` is a layout file: below a `layouts` folder.
    pub fn is_layout(&self, path: &Path) -> bool {
        path.extension().is_some_and(|ext| ext == "php") && self.layouts.iter().any(|(_, file)| file == path)
    }
}

impl Extension {
    /// Whether the extension is the one named `name`.
    pub fn is_named(&self, name: &str) -> bool {
        if self.name.eq_ignore_ascii_case(name) {
            return true;
        }
        match (&self.kind, &self.group) {
            (ExtensionKind::Plugin, Some(group)) => format!("plg_{group}_{}", self.name).eq_ignore_ascii_case(name),
            (ExtensionKind::Template, _) => format!("tpl_{}", self.name).eq_ignore_ascii_case(name),
            _ => false,
        }
    }

    /// The language prefix of its constants: `COM_CONTENT`,
    /// `PLG_CONTENT_PAGEBREAK`, `MOD_CUSTOM`, `TPL_CASSIOPEIA`.
    pub fn language_prefix(&self) -> String {
        match (&self.kind, &self.group) {
            (ExtensionKind::Plugin, Some(group)) => format!("PLG_{group}_{}", self.name).to_ascii_uppercase(),
            (ExtensionKind::Template, _) => format!("TPL_{}", self.name).to_ascii_uppercase(),
            _ => self.name.to_ascii_uppercase(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(files: &[(&str, &str)]) -> (tempfile::TempDir, Arc<JoomlaProject>) {
        let tmp = tempfile::tempdir().unwrap();
        for (rel, text) in files {
            let path = tmp.path().join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
        let project = JoomlaCache::default().project(tmp.path());
        (tmp, project)
    }

    fn shown(root: &Path, path: &Path) -> String {
        path.strip_prefix(root).unwrap_or(path).to_string_lossy().replace('\\', "/")
    }

    const MANIFEST: &str = "<?xml version=\"1.0\"?>\n<extension type=\"component\" method=\"upgrade\">\n    <name>COM_SHOP</name>\n    <description>COM_SHOP_XML_DESCRIPTION</description>\n    <namespace path=\"src\">Acme\\Component\\Shop</namespace>\n    <files folder=\"site\"><folder>src</folder></files>\n    <administration><files folder=\"admin\"><folder>src</folder></files></administration>\n</extension>\n";
    const FORM: &str = "<?xml version=\"1.0\"?>\n<form>\n    <fieldset addfieldprefix=\"Acme\\Component\\Shop\\Administrator\\Field\">\n        <field name=\"title\" type=\"text\" label=\"COM_SHOP_FIELD_TITLE_LABEL\"/>\n        <field name=\"category\" type=\"modal_category\" layout=\"joomla.form.field.list\"/>\n    </fieldset>\n</form>\n";
    const ASSETS: &str = "{\n  \"name\": \"com_shop\",\n  \"assets\": [\n    {\n      \"name\": \"com_shop.cart\",\n      \"type\": \"script\",\n      \"uri\": \"com_shop/cart.js\",\n      \"dependencies\": [\"core\", \"com_shop.base#script\"]\n    },\n    { \"name\": \"com_shop.base\", \"type\": \"style\", \"uri\": \"x.css\" }\n  ]\n}\n";
    const SQL: &str = "CREATE TABLE IF NOT EXISTS `#__shop_items` (\n  `id` int unsigned NOT NULL AUTO_INCREMENT,\n  `title` varchar(255) NOT NULL DEFAULT '',\n  PRIMARY KEY (`id`)\n) ENGINE=InnoDB;\n";

    #[test]
    fn a_site_has_its_extensions_and_what_they_declare() {
        let (tmp, project) = project(&[
            ("libraries/src/Version.php", "<?php\n"),
            (
                "libraries/src/Event/CoreEventAware.php",
                "<?php\ntrait CoreEventAware {\n    private static $eventNameToConcreteClass = [\n        // Content\n        'onContentPrepare' => Content\\ContentPrepareEvent::class,\n        'onError' => ErrorEvent::class,\n    ];\n}\n",
            ),
            ("administrator/components/com_shop/shop.xml", MANIFEST),
            (
                "administrator/components/com_shop/access.xml",
                "<access component=\"com_shop\">\n    <section name=\"component\">\n        <action name=\"core.edit\" title=\"JACTION_EDIT\"/>\n        <action name=\"shop.ship\" title=\"COM_SHOP_ACTION_SHIP\"/>\n    </section>\n</access>\n",
            ),
            (
                "administrator/components/com_shop/config.xml",
                "<config>\n    <fieldset name=\"shop\">\n        <field name=\"show_prices\" type=\"radio\"/>\n    </fieldset>\n</config>\n",
            ),
            ("administrator/components/com_shop/forms/item.xml", FORM),
            ("administrator/components/com_shop/sql/install.mysql.utf8.sql", SQL),
            ("administrator/components/com_shop/layouts/shop/price.php", "<?php\n"),
            ("components/com_shop/src/View/Items/HtmlView.php", "<?php\n"),
            ("components/com_shop/tmpl/items/default.php", "<?php\n"),
            ("media/com_shop/joomla.asset.json", ASSETS),
            ("language/en-GB/com_shop.ini", "COM_SHOP=\"Shop\"\nCOM_SHOP_HITS=\"Hits: %d\"\n"),
            ("language/de-DE/com_shop.ini", "COM_SHOP=\"Laden\"\n"),
            ("layouts/joomla/content/tags.php", "<?php\n"),
            (
                "templates/cassiopeia/templateDetails.xml",
                "<extension type=\"template\" client=\"site\"><name>cassiopeia</name></extension>\n",
            ),
            ("templates/cassiopeia/html/layouts/joomla/content/tags.php", "<?php\n"),
            (
                "plugins/content/stamp/stamp.xml",
                "<extension type=\"plugin\" group=\"content\">\n    <namespace path=\"src\">Acme\\Plugin\\Content\\Stamp</namespace>\n    <config><fields name=\"params\"><fieldset name=\"basic\"><field name=\"ink\" type=\"color\"/></fieldset></fields></config>\n</extension>\n",
            ),
        ]);
        let root = tmp.path();
        let extensions: Vec<(String, Option<&str>, Option<&str>)> = project
            .extensions
            .iter()
            .map(|extension| (shown(root, &extension.path), extension.client, extension.namespace.as_deref()))
            .collect();
        assert_eq!(
            extensions,
            [
                ("components/com_shop".to_owned(), Some("Site"), Some("Acme\\Component\\Shop\\Site")),
                ("templates/cassiopeia".to_owned(), Some("Site"), None),
                (
                    "administrator/components/com_shop".to_owned(),
                    Some("Administrator"),
                    Some("Acme\\Component\\Shop\\Administrator")
                ),
                ("plugins/content/stamp".to_owned(), None, Some("Acme\\Plugin\\Content\\Stamp")),
            ]
        );
        assert_eq!(project.extension("plg_content_stamp").len(), 1);
        assert_eq!(project.extension("com_shop").len(), 2);
        let found = project.extension_of_class("\\Acme\\Component\\Shop\\Site\\View\\Items\\HtmlView").unwrap();
        assert_eq!(shown(root, &found.path), "components/com_shop");
        assert_eq!(found.language_prefix(), "COM_SHOP");
        // Language constants: British English first.
        let constant: Vec<String> = project.constant("COM_SHOP").iter().map(|def| shown(root, &def.file)).collect();
        assert_eq!(constant, ["language/en-GB/com_shop.ini", "language/de-DE/com_shop.ini"]);
        assert_eq!(project.constant("com_shop_hits")[0].detail.as_deref(), Some("Hits: %d"));
        // Layouts: a template's override before the layout itself; a component's own.
        let layout: Vec<String> = project.layout("joomla.content.tags").iter().map(|file| shown(root, file)).collect();
        assert_eq!(
            layout,
            ["templates/cassiopeia/html/layouts/joomla/content/tags.php", "layouts/joomla/content/tags.php"]
        );
        assert_eq!(project.layout("shop.price").len(), 1);
        // Assets with their dependencies; actions; forms and parameters; events; tables.
        let assets: Vec<(&str, &str, usize)> = project
            .assets
            .iter()
            .map(|asset| (asset.name.as_str(), asset.kind.as_str(), asset.dependencies.len()))
            .collect();
        assert_eq!(assets, [("com_shop.cart", "script", 2), ("com_shop.base", "style", 0)]);
        assert_eq!(project.assets[0].dependencies[1].0, "com_shop.base");
        let actions: Vec<&str> = project.actions.iter().map(|action| action.name.as_str()).collect();
        assert_eq!(actions, ["core.edit", "shop.ship"]);
        let forms: Vec<(&str, bool, Vec<&str>)> = project
            .forms
            .iter()
            .map(|form| (form.name.as_str(), form.params, form.fields.iter().map(|field| field.0.as_str()).collect()))
            .collect();
        assert_eq!(
            forms,
            [
                ("config", true, vec!["show_prices"]),
                ("item", false, vec!["title", "category"]),
                ("stamp", true, vec!["ink"])
            ]
        );
        assert_eq!(project.event_class("onContentPrepare"), Some("Joomla\\CMS\\Event\\Content\\ContentPrepareEvent"));
        assert_eq!(project.event_class("onError"), Some("Joomla\\CMS\\Event\\ErrorEvent"));
        let table = project.table("#__shop_items").unwrap();
        let columns: Vec<(&str, &str)> =
            table.columns.iter().map(|column| (column.0.as_str(), column.1.as_str())).collect();
        assert_eq!(columns, [("id", "int"), ("title", "varchar(255)")]);
        // What the XML names: a field's type with the prefixes in force, a layout, constants.
        let named: Vec<(What, &str)> = project
            .mentions()
            .filter(|(file, _)| file.file.ends_with("item.xml"))
            .map(|(_, mention)| (mention.what, mention.name.as_str()))
            .collect();
        assert_eq!(
            named,
            [
                (What::Class, "Acme\\Component\\Shop\\Administrator\\Field"),
                (What::Language, "COM_SHOP_FIELD_TITLE_LABEL"),
                (What::FieldType, "text"),
                (What::Layout, "joomla.form.field.list"),
                (What::FieldType, "modal_category"),
            ]
        );
        assert_eq!(
            JoomlaProject::field_classes("modal_category", &["Acme\\Field".to_owned()]),
            ["Acme\\Field\\Modal\\CategoryField", "Joomla\\CMS\\Form\\Field\\Modal\\CategoryField"]
        );
        // A view's template.
        let (extension, view, layout) =
            project.view_template(&root.join("components/com_shop/tmpl/items/default.php")).unwrap();
        assert_eq!((extension.name.as_str(), view.as_str(), layout.as_str()), ("com_shop", "items", "default"));
    }

    #[test]
    fn an_extension_opened_alone_is_the_project() {
        let (tmp, project) = project(&[
            ("shop.xml", MANIFEST),
            ("site/src/View/Items/HtmlView.php", "<?php\n"),
            ("admin/src/Model/ItemModel.php", "<?php\n"),
            ("admin/forms/item.xml", FORM),
            ("admin/language/en-GB/com_shop.ini", "COM_SHOP=\"Shop\"\n"),
        ]);
        let root = tmp.path();
        let extensions: Vec<(&str, String, Option<&str>)> = project
            .extensions
            .iter()
            .map(|extension| (extension.name.as_str(), shown(root, &extension.path), extension.namespace.as_deref()))
            .collect();
        assert_eq!(
            extensions,
            [
                ("com_shop", "site".to_owned(), Some("Acme\\Component\\Shop\\Site")),
                ("com_shop", "admin".to_owned(), Some("Acme\\Component\\Shop\\Administrator")),
            ]
        );
        assert_eq!(project.forms.iter().filter(|form| form.name == "item").count(), 1);
        assert_eq!(project.constant("COM_SHOP").len(), 1);
    }
}
