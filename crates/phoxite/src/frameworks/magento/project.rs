//! What the XML files of a Magento 2 project declare ([`MagentoProject`]),
//! read with [`crate::base::xml`]:
//!
//! - **Modules:** a folder with a `registration.php` under `app/code/*/*` or
//!   `vendor/*/*`, or the project itself (a module opened alone), named as
//!   its registration names it (`Vendor_Module`).
//! - **Themes:** the folders of `app/design/<area>/<Vendor>/<theme>` and the
//!   themes Composer installed (a registration naming
//!   `<area>/<Vendor>/<theme>`), each with the parent its `theme.xml` names.
//! - **Dependency injection** (`etc/di.xml` and `etc/<area>/di.xml` of each
//!   module, `app/etc/di.xml`): preferences, virtual types, plugins, each
//!   with the area of its file.
//! - **Events** (`events.xml`): each event's observers.
//! - **ACL resources** (`acl.xml`), **configuration paths** (the sections,
//!   groups and fields of `adminhtml/system.xml`, the defaults of
//!   `config.xml`), **extension attributes** (`extension_attributes.xml`).
//! - **Routes** (`routes.xml`): a route's id, front name and modules.
//!   **Tables** (`db_schema.xml`): a table's columns.
//! - **Layout** (`view/<area>/layout/*.xml`, a theme's too): a file is a
//!   handle; its blocks and containers, each block with its class, its
//!   template and its view model.
//! - **Every name a file writes** ([`Mention`]): the classes, events, ACL
//!   resources, templates, handles, blocks, modules, tables and routes of
//!   the files above and of `webapi.xml`, `crontab.xml`,
//!   `communication.xml`, `queue_consumer.xml`, `menu.xml`, `module.xml`, UI
//!   components (`view/<area>/ui_component/*.xml`), GraphQL schemas
//!   (`etc/schema.graphqls`) and the other files of `etc` (`widget.xml`,
//!   `indexer.xml`, `cache.xml`…, read for the classes they name), each
//!   with where it is written. They are what
//!   Find References and a rename reach in these files, and what the caret
//!   is on in one ([`super::documents`]).
//!
//! Magento is large: a file's declarations are kept until the file changes
//! ([`MagentoCache`]), and the project is put together again only when one
//! of these files or a registration is written, not on every edit. The
//! behaviour follows Magento's developer documentation.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use crate::base::Range;
use crate::base::xml::{self, Element};
use crate::syntax::LineIndex;

/// A name a file declares.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Def {
    pub name: String,
    /// A resource's title, a configuration value's default.
    pub detail: Option<String>,
    pub file: PathBuf,
    pub range: Range,
}

/// A module.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Module {
    /// `Vendor_Module`.
    pub name: String,
    pub path: PathBuf,
}

/// A theme.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Theme {
    pub area: String,
    /// `Vendor/name`.
    pub code: String,
    /// The theme its `theme.xml` names as its parent: `Vendor/name`.
    pub parent: Option<String>,
    pub path: PathBuf,
}

/// `<preference for="Interface" type="Class"/>`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Preference {
    pub of: String,
    pub class: String,
    /// The area of its file (`etc/<area>/di.xml`); none for a global one.
    pub area: Option<String>,
    pub file: PathBuf,
    pub range: Range,
}

/// `<virtualType name="Name" type="Class">`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VirtualType {
    pub name: String,
    pub class: Option<String>,
    pub area: Option<String>,
    pub file: PathBuf,
    pub range: Range,
}

/// `<type name="Target"><plugin name="x" type="Class"/></type>`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plugin {
    pub target: String,
    pub class: String,
    pub name: String,
    pub area: Option<String>,
    pub file: PathBuf,
    /// Where the plugin's class is written.
    pub range: Range,
}

/// `<event name="x"><observer name="y" instance="Class"/></event>`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Observer {
    pub event: String,
    pub class: Option<String>,
    pub area: Option<String>,
    pub file: PathBuf,
    /// Where the event's name is written.
    pub range: Range,
}

/// A block or a container of a layout file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Block {
    pub name: String,
    /// `as`: the name its parent has it under.
    pub alias: Option<String>,
    /// A block's class (none for a container).
    pub class: Option<String>,
    /// `Vendor_Module::path/file.phtml`.
    pub template: Option<String>,
    /// Where the template is written.
    pub template_range: Option<Range>,
    /// The class of its `view_model` argument.
    pub view_model: Option<String>,
    /// The module (or the theme's module folder) the layout file is of.
    pub module: Option<String>,
    pub area: String,
    pub file: PathBuf,
    pub range: Range,
}

/// `<attribute code="x" type="T"/>` of `<extension_attributes for="I">`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExtensionAttribute {
    pub of: String,
    pub code: String,
    pub ty: String,
    pub file: PathBuf,
    pub range: Range,
}

/// `<route id="wishlist" frontName="wishlist"><module name="Magento_Wishlist"/></route>`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Route {
    pub id: String,
    pub front_name: String,
    /// `frontend` or `adminhtml`.
    pub area: Option<String>,
    pub modules: Vec<String>,
    pub file: PathBuf,
    /// Where the front name is written.
    pub range: Range,
}

/// `<table name="wishlist">` of a `db_schema.xml`, with its columns.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Table {
    pub name: String,
    pub comment: Option<String>,
    /// Each column's name, its type and where it is written.
    pub columns: Vec<(String, String, Range)>,
    pub file: PathBuf,
    pub range: Range,
}

/// What a [`Mention`] names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum What {
    Class,
    Event,
    Acl,
    Config,
    Template,
    Handle,
    Block,
    Module,
    Table,
    Route,
    /// A UI component: a file of `ui_component`.
    Component,
}

/// A name written in a file, and where.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mention {
    pub what: What,
    /// The name; a class without its leading backslash.
    pub name: String,
    /// The method named with a class (`<service class="…" method="getById"/>`),
    /// and where it is written.
    pub method: Option<(String, Range)>,
    /// Whether the name is declared here (an event's observers, a block, a
    /// table), rather than referred to.
    pub declares: bool,
    /// Where the name is written, as written (a class with its leading
    /// backslash when it has one).
    pub range: Range,
    /// Whether backslashes are written twice (a GraphQL schema's strings).
    pub escaped: bool,
}

/// The files that are read, by where they are.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum XmlKind {
    Di,
    Events,
    Acl,
    System,
    Config,
    ExtensionAttributes,
    Routes,
    Schema,
    Webapi,
    Crontab,
    Communication,
    QueueConsumer,
    ModuleXml,
    Menu,
    Layout,
    UiComponent,
    /// `schema.graphqls`: not XML, scanned for the classes it names.
    GraphQl,
    /// Another file of `etc` (`widget.xml`, `indexer.xml`, `cache.xml`,
    /// `sales.xml`…): the classes and the modules it names.
    Other,
}

impl XmlKind {
    /// The kind of the file at `path`, when it is one the model reads.
    pub fn of(path: &Path) -> Option<Self> {
        let name = path.file_name()?.to_str()?;
        let parent = path.parent().and_then(|parent| parent.file_name()).and_then(|name| name.to_str());
        if name.ends_with(".xml") {
            match parent {
                Some("layout") => return Some(Self::Layout),
                Some("ui_component") => return Some(Self::UiComponent),
                _ => {}
            }
        }
        if !path.components().any(|part| part.as_os_str() == "etc") {
            return None;
        }
        Some(match name {
            "di.xml" => Self::Di,
            "events.xml" => Self::Events,
            "acl.xml" => Self::Acl,
            "system.xml" => Self::System,
            "config.xml" => Self::Config,
            "extension_attributes.xml" => Self::ExtensionAttributes,
            "routes.xml" => Self::Routes,
            "db_schema.xml" => Self::Schema,
            "webapi.xml" => Self::Webapi,
            "crontab.xml" => Self::Crontab,
            "communication.xml" => Self::Communication,
            "queue_consumer.xml" => Self::QueueConsumer,
            "module.xml" => Self::ModuleXml,
            "menu.xml" => Self::Menu,
            "schema.graphqls" => Self::GraphQl,
            _ if name.ends_with(".xml") => Self::Other,
            _ => return None,
        })
    }
}

/// The area a file is of, by its folders: `etc/<area>/`, `view/<area>/`,
/// `app/design/<area>/`, a class below `Adminhtml`. None for a global file.
pub fn area_of(path: &Path) -> Option<String> {
    let parts: Vec<String> = path.components().map(|part| part.as_os_str().to_string_lossy().into_owned()).collect();
    let folders = &parts[..parts.len().saturating_sub(1)];
    for (at, part) in folders.iter().enumerate().rev() {
        let next = folders.get(at + 1);
        match part.as_str() {
            "etc" | "view" | "design" => {
                if let Some(next) = next {
                    return Some(next.clone());
                }
            }
            "Adminhtml" => return Some("adminhtml".to_owned()),
            _ => {}
        }
    }
    None
}

/// What one file declares and names.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FileFacts {
    pub file: PathBuf,
    pub preferences: Vec<Preference>,
    pub virtual_types: Vec<VirtualType>,
    pub plugins: Vec<Plugin>,
    pub observers: Vec<Observer>,
    pub acl: Vec<Def>,
    pub config: Vec<Def>,
    pub blocks: Vec<Block>,
    pub handles: Vec<Def>,
    pub extension_attributes: Vec<ExtensionAttribute>,
    pub routes: Vec<Route>,
    pub tables: Vec<Table>,
    pub mentions: Vec<Mention>,
}

/// See the module documentation.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MagentoProject {
    pub root: PathBuf,
    pub modules: Vec<Module>,
    pub themes: Vec<Theme>,
    pub preferences: Vec<Preference>,
    pub virtual_types: Vec<VirtualType>,
    pub plugins: Vec<Plugin>,
    pub observers: Vec<Observer>,
    pub acl: Vec<Def>,
    /// Configuration paths: `section/group/field`.
    pub config: Vec<Def>,
    pub blocks: Vec<Block>,
    /// Layout handles: a layout file's name.
    pub handles: Vec<Def>,
    pub extension_attributes: Vec<ExtensionAttribute>,
    pub routes: Vec<Route>,
    pub tables: Vec<Table>,
    /// Every file read, with the names it writes.
    pub files: Vec<Arc<FileFacts>>,
}

fn class(name: &str) -> String {
    name.trim().trim_start_matches('\\').to_owned()
}

/// Whether `name` can be a class's name (or a virtual type's).
fn is_class_name(name: &str) -> bool {
    let name = name.trim_start_matches('\\');
    !name.is_empty()
        && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '\\'))
        && name.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
}

struct Reader<'a> {
    file: &'a Path,
    area: Option<String>,
    module: Option<&'a str>,
    out: FileFacts,
}

impl Reader<'_> {
    fn mention(&mut self, what: What, name: &str, range: Range, declares: bool) {
        let name = if what == What::Class { class(name) } else { name.trim().to_owned() };
        if name.is_empty() || (what == What::Class && !is_class_name(&name)) {
            return;
        }
        self.out.mentions.push(Mention { what, name, method: None, declares, range, escaped: false });
    }

    /// The attribute `name` of `element`, as a mention.
    fn attribute(&mut self, element: &Element, name: &str, what: What, declares: bool) {
        if let Some(attribute) = element.attribute(name) {
            self.mention(what, &attribute.value, attribute.range, declares);
        }
    }

    /// A class and the method named beside it.
    fn class_method(&mut self, element: &Element, class_attribute: &str, method_attribute: &str) {
        let Some(named) = element.attribute(class_attribute) else { return };
        self.mention(What::Class, &named.value, named.range, false);
        if let (Some(method), Some(last)) = (element.attribute(method_attribute), self.out.mentions.last_mut())
            && last.range == named.range
        {
            last.method = Some((method.value.clone(), method.range));
        }
    }

    /// The objects among the arguments below `element`:
    /// `<argument xsi:type="object">Class</argument>`, an `<item>` too.
    fn objects(&mut self, element: &Element) {
        for found in element.descendants() {
            if matches!(found.name.as_str(), "argument" | "item")
                && found.attr("xsi:type") == Some("object")
                && !found.text.is_empty()
            {
                self.mention(What::Class, &found.text, found.text_range, false);
            }
        }
    }

    fn di(&mut self, root: &Element) {
        let file = self.file.to_path_buf();
        for element in &root.children {
            match element.name.as_str() {
                "preference" => {
                    self.attribute(element, "for", What::Class, false);
                    self.attribute(element, "type", What::Class, false);
                    if let (Some(of), Some(to)) = (element.attribute("for"), element.attr("type")) {
                        self.out.preferences.push(Preference {
                            of: class(&of.value),
                            class: class(to),
                            area: self.area.clone(),
                            file: file.clone(),
                            range: of.range,
                        });
                    }
                }
                "virtualType" => {
                    self.attribute(element, "name", What::Class, true);
                    self.attribute(element, "type", What::Class, false);
                    if let Some(name) = element.attribute("name") {
                        self.out.virtual_types.push(VirtualType {
                            name: class(&name.value),
                            class: element.attr("type").map(class),
                            area: self.area.clone(),
                            file: file.clone(),
                            range: name.range,
                        });
                    }
                }
                "type" => self.attribute(element, "name", What::Class, false),
                _ => {}
            }
            // A type's and a virtual type's plugins.
            if matches!(element.name.as_str(), "type" | "virtualType")
                && let Some(target) = element.attr("name")
            {
                for plugin in element.named("plugin") {
                    self.attribute(plugin, "type", What::Class, false);
                    if plugin.attr("disabled") == Some("true") {
                        continue;
                    }
                    if let Some(of) = plugin.attribute("type") {
                        self.out.plugins.push(Plugin {
                            target: class(target),
                            class: class(&of.value),
                            name: plugin.attr("name").unwrap_or_default().to_owned(),
                            area: self.area.clone(),
                            file: file.clone(),
                            range: of.range,
                        });
                    }
                }
            }
        }
        self.objects(root);
    }

    fn events(&mut self, root: &Element) {
        for event in root.named("event") {
            let Some(name) = event.attribute("name") else { continue };
            self.mention(What::Event, &name.value, name.range, true);
            let observers: Vec<&Element> = event.named("observer").collect();
            let mut classes: Vec<Option<String>> = observers
                .iter()
                .filter(|observer| observer.attr("disabled") != Some("true"))
                .map(|observer| observer.attr("instance").map(class))
                .collect();
            if classes.is_empty() {
                classes.push(None);
            }
            for instance in classes {
                self.out.observers.push(Observer {
                    event: name.value.clone(),
                    class: instance,
                    area: self.area.clone(),
                    file: self.file.to_path_buf(),
                    range: name.range,
                });
            }
            for observer in observers {
                self.attribute(observer, "instance", What::Class, false);
            }
        }
    }

    fn acl(&mut self, root: &Element) {
        for resource in root.descendants().into_iter().filter(|element| element.name == "resource") {
            // A resource a module adds to has no title there: where it has one, it is declared.
            if let Some(id) = resource.attribute("id") {
                let title = resource.attr("title").map(str::to_owned);
                self.mention(What::Acl, &id.value, id.range, title.is_some());
                self.out.acl.push(Def {
                    name: id.value.clone(),
                    detail: title,
                    file: self.file.to_path_buf(),
                    range: id.range,
                });
            }
        }
    }

    /// The fields of `system.xml`: `section/group/field`, groups inside groups too.
    fn system(&mut self, root: &Element) {
        fn below(element: &Element, above: &str, reader: &mut Reader<'_>) {
            for child in &element.children {
                match child.name.as_str() {
                    "resource" if !child.text.is_empty() => {
                        reader.mention(What::Acl, &child.text, child.text_range, false);
                    }
                    "source_model" | "backend_model" | "frontend_model" if !child.text.is_empty() => {
                        reader.mention(What::Class, &child.text, child.text_range, false);
                    }
                    _ => {}
                }
                let Some(id) = child.attribute("id") else { continue };
                let path = if above.is_empty() { id.value.clone() } else { format!("{above}/{}", id.value) };
                match child.name.as_str() {
                    "section" | "group" => below(child, &path, reader),
                    "field" => {
                        below(child, &path, reader);
                        reader.mention(What::Config, &path, id.range, true);
                        reader.out.config.push(Def {
                            name: path,
                            detail: child
                                .child("label")
                                .map(|label| label.text.clone())
                                .filter(|label| !label.is_empty()),
                            file: reader.file.to_path_buf(),
                            range: id.range,
                        });
                    }
                    _ => {}
                }
            }
        }
        for system in root.named("system") {
            below(system, "", self);
        }
    }

    /// The defaults of `config.xml`: `<default><a><b><c>1</c></b></a></default>` is `a/b/c`.
    fn defaults(&mut self, root: &Element) {
        fn below(element: &Element, above: &str, depth: usize, reader: &mut Reader<'_>) {
            for child in &element.children {
                let path = if above.is_empty() { child.name.clone() } else { format!("{above}/{}", child.name) };
                if child.children.is_empty() {
                    if depth >= 2 {
                        let value = Some(child.text.clone()).filter(|text| !text.is_empty() && text.len() < 120);
                        reader.mention(What::Config, &path, child.range, true);
                        reader.out.config.push(Def {
                            name: path,
                            detail: value,
                            file: reader.file.to_path_buf(),
                            range: child.range,
                        });
                    }
                } else if depth < 6 {
                    below(child, &path, depth + 1, reader);
                }
            }
        }
        for default in root.named("default") {
            below(default, "", 0, self);
        }
    }

    fn extension_attributes(&mut self, root: &Element) {
        for of in root.named("extension_attributes") {
            self.attribute(of, "for", What::Class, false);
            let Some(interface) = of.attr("for") else { continue };
            for attribute in of.named("attribute") {
                if let (Some(code), Some(ty)) = (attribute.attribute("code"), attribute.attribute("type")) {
                    // A class, without the `[]` of a list of them.
                    if ty.value.contains('\\') {
                        let name = ty.value.trim_end_matches("[]");
                        let mut range = ty.range;
                        range.end.col = range.start.col + name.encode_utf16().count() as u32;
                        self.mention(What::Class, name, range, false);
                    }
                    self.out.extension_attributes.push(ExtensionAttribute {
                        of: class(interface),
                        code: code.value.clone(),
                        ty: ty.value.trim().to_owned(),
                        file: self.file.to_path_buf(),
                        range: code.range,
                    });
                }
            }
        }
    }

    /// The blocks and containers of a layout file, which is a handle.
    fn layout(&mut self, root: &Element) {
        let file = self.file.to_path_buf();
        if let Some(stem) = file.file_stem().and_then(|stem| stem.to_str()) {
            self.out.handles.push(Def { name: stem.to_owned(), detail: None, file: file.clone(), range: root.range });
        }
        let area = self.area.clone().unwrap_or_default();
        for element in root.descendants() {
            match element.name.as_str() {
                "update" => self.attribute(element, "handle", What::Handle, false),
                "move" => {
                    self.attribute(element, "element", What::Block, false);
                    self.attribute(element, "destination", What::Block, false);
                }
                "uiComponent" => self.attribute(element, "name", What::Component, false),
                _ => {}
            }
            for sibling in ["before", "after"] {
                if element.attr(sibling).is_some_and(|name| name != "-" && !name.is_empty()) {
                    self.attribute(element, sibling, What::Block, false);
                }
            }
            let declares = matches!(element.name.as_str(), "block" | "container");
            // A reference gives a block of another file a template or a view model.
            let refers = matches!(element.name.as_str(), "referenceBlock" | "referenceContainer");
            if !declares && !refers {
                continue;
            }
            self.attribute(element, "name", What::Block, declares);
            self.attribute(element, "class", What::Class, false);
            self.attribute(element, "template", What::Template, false);
            let Some(name) = element.attribute("name") else { continue };
            let view_model = element.child("arguments").and_then(|arguments| {
                let found = arguments.named("argument").find(|argument| {
                    matches!(argument.attr("name"), Some("view_model" | "viewModel"))
                        && argument.attr("xsi:type") == Some("object")
                });
                found.map(|argument| class(&argument.text)).filter(|class| !class.is_empty())
            });
            let template = element.attr("template").map(str::to_owned);
            let is_block = element.name == "block" || element.name == "referenceBlock";
            if refers && (!is_block || (view_model.is_none() && template.is_none() && element.attr("class").is_none()))
            {
                continue;
            }
            self.out.blocks.push(Block {
                name: name.value.clone(),
                alias: element.attr("as").map(str::to_owned),
                class: element.attr("class").map(class),
                template,
                template_range: element.attribute("template").map(|template| template.range),
                view_model,
                module: self.module.map(str::to_owned),
                area: area.clone(),
                file: file.clone(),
                range: name.range,
            });
        }
        self.objects(root);
    }

    fn routes(&mut self, root: &Element) {
        for route in root.descendants().into_iter().filter(|element| element.name == "route") {
            let Some(id) = route.attribute("id") else { continue };
            let front = route.attribute("frontName").unwrap_or(id);
            self.mention(What::Route, &front.value, front.range, true);
            let mut modules = Vec::new();
            for module in route.named("module") {
                self.attribute(module, "name", What::Module, false);
                modules.extend(module.attr("name").map(str::to_owned));
            }
            self.out.routes.push(Route {
                id: id.value.clone(),
                front_name: front.value.clone(),
                area: self.area.clone(),
                modules,
                file: self.file.to_path_buf(),
                range: front.range,
            });
        }
    }

    fn schema(&mut self, root: &Element) {
        for table in root.named("table") {
            let Some(name) = table.attribute("name") else { continue };
            self.mention(What::Table, &name.value, name.range, true);
            let mut columns = Vec::new();
            for column in table.named("column") {
                if let Some(of) = column.attribute("name") {
                    let ty = column.attr("xsi:type").unwrap_or_default().to_owned();
                    columns.push((of.value.clone(), ty, of.range));
                }
            }
            for constraint in table.named("constraint") {
                self.attribute(constraint, "table", What::Table, false);
                self.attribute(constraint, "referenceTable", What::Table, false);
            }
            self.out.tables.push(Table {
                name: name.value.clone(),
                comment: table.attr("comment").map(str::to_owned),
                columns,
                file: self.file.to_path_buf(),
                range: name.range,
            });
        }
    }

    /// The files that only name things.
    fn naming(&mut self, kind: XmlKind, root: &Element) {
        for element in root.descendants() {
            match (kind, element.name.as_str()) {
                (XmlKind::Webapi, "service") => self.class_method(element, "class", "method"),
                (XmlKind::Webapi, "resource") => self.attribute(element, "ref", What::Acl, false),
                (XmlKind::Crontab, "job") => self.class_method(element, "instance", "method"),
                (XmlKind::Communication, "handler") => self.class_method(element, "type", "method"),
                (XmlKind::QueueConsumer, "consumer") => {
                    // `Class::method`.
                    if let Some(handler) = element.attribute("handler") {
                        let (name, method) = handler.value.split_once("::").unwrap_or((&handler.value, ""));
                        let mut range = handler.range;
                        range.end.col = range.start.col + name.encode_utf16().count() as u32;
                        self.mention(What::Class, name, range, false);
                        if !method.is_empty()
                            && let Some(last) = self.out.mentions.last_mut()
                        {
                            let mut at = handler.range;
                            at.start.col = range.end.col + 2;
                            last.method = Some((method.to_owned(), at));
                        }
                    }
                    self.attribute(element, "consumerInstance", What::Class, false);
                }
                (XmlKind::ModuleXml, "module") => {
                    // The module itself, and those it is loaded after.
                    let declares = element.child("sequence").is_some() || element.attr("setup_version").is_some();
                    let top = self.out.mentions.iter().all(|mention| mention.what != What::Module);
                    self.attribute(element, "name", What::Module, declares || top);
                }
                (XmlKind::Menu, "add" | "update") => {
                    self.attribute(element, "resource", What::Acl, false);
                    self.attribute(element, "module", What::Module, false);
                }
                (XmlKind::UiComponent, _) => {
                    if element.attr("class").is_some_and(|name| name.contains('\\')) {
                        self.attribute(element, "class", What::Class, false);
                    }
                    if element.attr("provider").is_some_and(|name| name.contains('\\')) {
                        self.attribute(element, "provider", What::Class, false);
                    }
                    if element.name == "aclResource" && !element.text.is_empty() {
                        self.mention(What::Acl, &element.text, element.text_range, false);
                    }
                }
                _ => {}
            }
        }
        if kind == XmlKind::UiComponent {
            self.objects(root);
        }
    }

    /// A file the model has no reading of: every attribute and every text
    /// that is a class's name with its namespace, and the modules named by
    /// a `module` attribute.
    fn generic(&mut self, root: &Element) {
        let qualified = |name: &str| {
            let name = name.trim().trim_start_matches('\\');
            name.matches('\\').count() >= 2 && is_class_name(name) && !name.ends_with('\\')
        };
        for element in root.descendants() {
            for attribute in &element.attributes {
                if qualified(&attribute.value) {
                    self.mention(What::Class, &attribute.value, attribute.range, false);
                } else if attribute.name == "module" && attribute.value.contains('_') {
                    self.mention(What::Module, &attribute.value, attribute.range, false);
                }
            }
            if element.children.is_empty() && qualified(&element.text) {
                self.mention(What::Class, &element.text, element.text_range, false);
            }
        }
    }

    /// A GraphQL schema: the classes of `@resolver(class: "…")` and
    /// `@typeResolver(class: "…")`, written with their backslashes doubled.
    fn graphql(&mut self, text: &str) {
        let lines = LineIndex::new(text);
        let mut from = 0;
        while let Some(found) = text[from..].find("class:") {
            let at = from + found + "class:".len();
            from = at;
            let rest = &text[at..];
            let quote = rest.len() - rest.trim_start().len();
            if !rest[quote..].starts_with('"') {
                continue;
            }
            let start = at + quote + 1;
            let Some(length) = text[start..].find('"') else { break };
            let written = &text[start..start + length];
            let name = written.replace("\\\\", "\\");
            from = start + length;
            if !name.contains('\\') || !is_class_name(&name) {
                continue;
            }
            self.out.mentions.push(Mention {
                what: What::Class,
                name: class(&name),
                method: None,
                declares: false,
                range: lines.range(start as u32, (start + length) as u32),
                escaped: true,
            });
        }
    }
}

/// What the file `path` declares and names, by its name and where it is:
/// `module` is the module it is of.
pub fn read(path: &Path, text: &str, module: Option<&str>) -> FileFacts {
    let mut reader = Reader {
        file: path,
        area: area_of(path),
        module,
        out: FileFacts { file: path.to_path_buf(), ..Default::default() },
    };
    let Some(kind) = XmlKind::of(path) else { return reader.out };
    if kind == XmlKind::GraphQl {
        reader.graphql(text);
        return reader.out;
    }
    let Some(root) = xml::root(text) else { return reader.out };
    match kind {
        XmlKind::Di => reader.di(&root),
        XmlKind::Events => reader.events(&root),
        XmlKind::Acl => reader.acl(&root),
        XmlKind::System => reader.system(&root),
        XmlKind::Config => reader.defaults(&root),
        XmlKind::ExtensionAttributes => reader.extension_attributes(&root),
        XmlKind::Layout => reader.layout(&root),
        XmlKind::Routes => reader.routes(&root),
        XmlKind::Schema => reader.schema(&root),
        XmlKind::Other => reader.generic(&root),
        _ => reader.naming(kind, &root),
    }
    reader.out
}

/// What a `registration.php` registers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Component {
    /// `'Vendor_Module'`.
    Module(String),
    /// `'frontend/Vendor/name'`: the area and the theme's code.
    Theme(String, String),
}

/// What a `registration.php` registers: a module or a theme.
pub fn registered(text: &str) -> Option<Component> {
    let (at, theme) = match text.find("ComponentRegistrar::MODULE") {
        Some(at) => (at, false),
        None => (text.find("ComponentRegistrar::THEME")?, true),
    };
    let rest = &text[at..];
    let quote = rest.find(['\'', '"'])?;
    let mark = rest[quote..].chars().next()?;
    let end = rest[quote + 1..].find(mark)?;
    let name = &rest[quote + 1..quote + 1 + end];
    if theme {
        let (area, code) = name.split_once('/')?;
        return code.contains('/').then(|| Component::Theme(area.to_owned(), code.to_owned()));
    }
    (name.contains('_') && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
        .then(|| Component::Module(name.to_owned()))
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

/// The files of `folder` the model reads.
fn read_files(folder: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(folder)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| XmlKind::of(path).is_some())
        .collect();
    found.sort();
    found
}

/// What each file declares, kept until the file changes, and the project
/// put together from them, kept until [`MagentoCache::forget`].
#[derive(Default)]
pub struct MagentoCache {
    project: Option<Arc<MagentoProject>>,
    files: HashMap<PathBuf, (SystemTime, Arc<FileFacts>)>,
    /// What each registration file registers.
    registrations: HashMap<PathBuf, (SystemTime, Option<Component>)>,
}

impl MagentoCache {
    /// A file the model reads or a registration was written: the project is
    /// put together again when next asked for.
    pub fn forget(&mut self) {
        self.project = None;
    }

    /// The project at `root`.
    pub fn project(&mut self, root: &Path) -> Arc<MagentoProject> {
        if let Some(project) = &self.project {
            return project.clone();
        }
        let project = Arc::new(self.load(root));
        self.project = Some(project.clone());
        project
    }

    fn facts(&mut self, path: &Path, module: Option<&str>) -> Option<Arc<FileFacts>> {
        let modified = std::fs::metadata(path).and_then(|metadata| metadata.modified()).ok()?;
        if let Some((at, known)) = self.files.get(path)
            && *at == modified
        {
            return Some(known.clone());
        }
        let text = std::fs::read_to_string(path).ok()?;
        let facts = Arc::new(read(path, &text, module));
        self.files.insert(path.to_path_buf(), (modified, facts.clone()));
        Some(facts)
    }

    fn component(&mut self, folder: &Path) -> Option<Component> {
        let file = folder.join("registration.php");
        let modified = std::fs::metadata(&file).and_then(|metadata| metadata.modified()).ok()?;
        match self.registrations.get(&file) {
            Some((at, known)) if *at == modified => known.clone(),
            _ => {
                let found = std::fs::read_to_string(&file).ok().and_then(|text| registered(&text));
                self.registrations.insert(file, (modified, found.clone()));
                found
            }
        }
    }

    fn load(&mut self, root: &Path) -> MagentoProject {
        let mut out = MagentoProject { root: root.to_path_buf(), ..MagentoProject::default() };
        // The modules and the themes: the project's, Composer's, or the project itself.
        let mut candidates: Vec<PathBuf> = vec![root.to_path_buf()];
        for vendor in folders(&root.join("app").join("code")).into_iter().chain(folders(&root.join("vendor"))) {
            candidates.extend(folders(&vendor));
        }
        let parent = |theme: &Path| {
            let text = std::fs::read_to_string(theme.join("theme.xml")).ok()?;
            let parent = xml::root(&text)?.child("parent")?.text.clone();
            (!parent.is_empty()).then_some(parent)
        };
        for area in folders(&root.join("app").join("design")) {
            let name = area.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
            for vendor in folders(&area) {
                let of = vendor.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
                for theme in folders(&vendor) {
                    let code = theme.file_name().map(|name| format!("{of}/{}", name.to_string_lossy()));
                    out.themes.push(Theme {
                        area: name.clone(),
                        code: code.unwrap_or_default(),
                        parent: parent(&theme),
                        path: theme,
                    });
                }
            }
        }
        for folder in candidates {
            match self.component(&folder) {
                Some(Component::Module(name)) if !out.modules.iter().any(|known| known.name == name) => {
                    out.modules.push(Module { name, path: folder });
                }
                Some(Component::Theme(area, code))
                    if !out.themes.iter().any(|known| known.area == area && known.code == code) =>
                {
                    out.themes.push(Theme { area, code, parent: parent(&folder), path: folder });
                }
                _ => {}
            }
        }

        let mut files: Vec<(PathBuf, Option<String>)> = vec![(root.join("app").join("etc").join("di.xml"), None)];
        for module in &out.modules {
            let etc = module.path.join("etc");
            for folder in std::iter::once(etc.clone()).chain(folders(&etc)) {
                files.extend(read_files(&folder).into_iter().map(|file| (file, Some(module.name.clone()))));
            }
            for area in folders(&module.path.join("view")) {
                for folder in ["layout", "ui_component"] {
                    let found = read_files(&area.join(folder));
                    files.extend(found.into_iter().map(|file| (file, Some(module.name.clone()))));
                }
            }
        }
        // A theme's layout files: `<theme>/Vendor_Module/layout/*.xml`.
        for theme in &out.themes {
            for module in folders(&theme.path) {
                let name = module.file_name().map(|name| name.to_string_lossy().into_owned());
                files.extend(read_files(&module.join("layout")).into_iter().map(|file| (file, name.clone())));
            }
        }
        for (file, module) in files {
            let Some(facts) = self.facts(&file, module.as_deref()) else { continue };
            out.preferences.extend(facts.preferences.iter().cloned());
            out.virtual_types.extend(facts.virtual_types.iter().cloned());
            out.plugins.extend(facts.plugins.iter().cloned());
            out.observers.extend(facts.observers.iter().cloned());
            out.acl.extend(facts.acl.iter().cloned());
            out.config.extend(facts.config.iter().cloned());
            out.blocks.extend(facts.blocks.iter().cloned());
            out.handles.extend(facts.handles.iter().cloned());
            out.extension_attributes.extend(facts.extension_attributes.iter().cloned());
            out.routes.extend(facts.routes.iter().cloned());
            out.tables.extend(facts.tables.iter().cloned());
            out.files.push(facts);
        }
        out
    }
}

impl MagentoProject {
    pub fn module(&self, name: &str) -> Option<&Module> {
        self.modules.iter().find(|module| module.name == name)
    }

    /// The module the file `path` is in.
    pub fn module_of(&self, path: &Path) -> Option<&Module> {
        let inside = self.modules.iter().filter(|module| path.starts_with(&module.path));
        inside.max_by_key(|module| module.path.as_os_str().len())
    }

    /// The module a file is of: the one it is in, or for a theme's file the
    /// module folder it is below (`<theme>/Vendor_Module/…`).
    pub fn module_name_of(&self, path: &Path) -> Option<String> {
        if let Some(module) = self.module_of(path) {
            return Some(module.name.clone());
        }
        let theme = self.themes.iter().find(|theme| path.starts_with(&theme.path))?;
        let below = path.strip_prefix(&theme.path).ok()?.components().next()?;
        let name = below.as_os_str().to_string_lossy().into_owned();
        name.contains('_').then_some(name)
    }

    /// Every name the files write, with the file it is in.
    pub fn mentions(&self) -> impl Iterator<Item = (&FileFacts, &Mention)> {
        self.files.iter().flat_map(|file| file.mentions.iter().map(move |mention| (file.as_ref(), mention)))
    }

    /// The mentions of `name` as a `what`.
    pub fn mentions_of<'p>(&'p self, what: What, name: &'p str) -> impl Iterator<Item = (&'p FileFacts, &'p Mention)> {
        let name = if what == What::Class { name.trim_start_matches('\\') } else { name };
        self.mentions().filter(move |(_, mention)| {
            mention.what == what
                && if what == What::Class { mention.name.eq_ignore_ascii_case(name) } else { mention.name == name }
        })
    }

    /// The one of `found` that holds in `area`: the last declared for the
    /// area, else the last global one, else the last of any area (the area
    /// isn't known, or only another declares it).
    fn in_area<'p, T>(found: Vec<&'p T>, area: Option<&str>, of: impl Fn(&T) -> Option<&str>) -> Option<&'p T> {
        let own = area.and_then(|area| found.iter().rev().find(|item| of(item) == Some(area)).copied());
        own.or_else(|| found.iter().rev().find(|item| of(item).is_none()).copied()).or_else(|| found.last().copied())
    }

    /// The class a preference gives the interface `of` in `area`: the
    /// area's `di.xml` wins over the global ones.
    pub fn preferred(&self, of: &str, area: Option<&str>) -> Option<&Preference> {
        let of = of.trim_start_matches('\\');
        let found: Vec<&Preference> =
            self.preferences.iter().filter(|preference| preference.of.eq_ignore_ascii_case(of)).collect();
        Self::in_area(found, area, |preference| preference.area.as_deref())
    }

    pub fn virtual_type(&self, name: &str) -> Option<&VirtualType> {
        self.virtual_type_in(name, None)
    }

    /// The virtual type `name` as `area` declares it.
    pub fn virtual_type_in(&self, name: &str, area: Option<&str>) -> Option<&VirtualType> {
        let name = name.trim_start_matches('\\');
        let found: Vec<&VirtualType> = self.virtual_types.iter().filter(|found| found.name == name).collect();
        Self::in_area(found, area, |found| found.area.as_deref())
    }

    /// The class a virtual type is in the end: its `type`, a virtual type's
    /// in turn.
    pub fn virtual_class(&self, name: &str) -> Option<String> {
        let mut name = name.trim_start_matches('\\').to_owned();
        for _ in 0..8 {
            match self.virtual_type(&name).and_then(|found| found.class.clone()) {
                Some(class) if self.virtual_type(&class).is_some() && class != name => name = class,
                Some(class) => return Some(class),
                None => return None,
            }
        }
        None
    }

    /// The themes of `area` (every area's for `None`), a theme before the
    /// themes it inherits from: where a template is looked for, in order.
    pub fn theme_chain(&self, area: Option<&str>) -> Vec<&Theme> {
        let themes: Vec<&Theme> =
            self.themes.iter().filter(|theme| area.is_none_or(|area| theme.area == area)).collect();
        // How many themes a theme inherits from: the deepest first.
        let depth = |theme: &Theme| {
            let (mut depth, mut current) = (0, theme);
            while let Some(parent) = current.parent.as_deref()
                && let Some(found) = self.themes.iter().find(|other| other.area == current.area && other.code == parent)
                && depth < 16
            {
                depth += 1;
                current = found;
            }
            depth
        };
        let mut ordered: Vec<(usize, &Theme)> = themes.into_iter().map(|theme| (depth(theme), theme)).collect();
        ordered.sort_by_key(|entry| std::cmp::Reverse(entry.0));
        ordered.into_iter().map(|(_, theme)| theme).collect()
    }

    /// The theme `path` is in, then the themes it inherits from.
    pub fn theme_parents(&self, path: &Path) -> Vec<&Theme> {
        let mut out: Vec<&Theme> = Vec::new();
        let mut current = self.themes.iter().find(|theme| path.starts_with(&theme.path));
        while let Some(theme) = current
            && out.len() < 16
        {
            out.push(theme);
            current = theme
                .parent
                .as_deref()
                .and_then(|parent| self.themes.iter().find(|other| other.area == theme.area && other.code == parent));
        }
        out
    }

    /// The files the template `id` may be, the likeliest first: a theme's
    /// version (a theme before its parents), then the module's for the
    /// area and for every area (`base`). `Vendor_Module::path/file.phtml`;
    /// an id without a module is of `module`. `area` is the area asking,
    /// when it is known.
    pub fn template_files(&self, id: &str, module: Option<&str>, area: Option<&str>) -> Vec<PathBuf> {
        if id.is_empty() || id.contains(['$', '{']) {
            return Vec::new();
        }
        let (module, path) = match id.split_once("::") {
            Some((module, path)) => (Some(module), path),
            None => (module, id),
        };
        let Some(module) = module else { return Vec::new() };
        let mut out: Vec<PathBuf> = Vec::new();
        for theme in self.theme_chain(area) {
            out.push(theme.path.join(module).join("templates").join(path));
        }
        if let Some(found) = self.module(module) {
            let areas: Vec<&str> = match area {
                Some(area) if area != "base" => vec![area, "base"],
                _ => vec!["frontend", "adminhtml", "base"],
            };
            for area in areas {
                out.push(found.path.join("view").join(area).join("templates").join(path));
            }
        }
        out
    }

    /// The id of the template file `path` (`Vendor_Module::path/file.phtml`)
    /// and its area: a module's `view/<area>/templates`, a theme's
    /// `Vendor_Module/templates`.
    pub fn template_id(&self, path: &Path) -> Option<(String, String)> {
        let parts: Vec<String> =
            path.components().map(|part| part.as_os_str().to_string_lossy().into_owned()).collect();
        let at = parts.iter().rposition(|part| part == "templates")?;
        let below = parts[at + 1..].join("/");
        if let Some(module) = self.module_of(path)
            && at >= 2
            && parts[at - 2] == "view"
        {
            return Some((format!("{}::{below}", module.name), parts[at - 1].clone()));
        }
        let theme = self.themes.iter().find(|theme| path.starts_with(&theme.path))?;
        let module = parts.get(at.checked_sub(1)?)?;
        Some((format!("{module}::{below}"), theme.area.clone()))
    }

    /// The blocks the layout gives the template `id` (as
    /// [`template_id`](Self::template_id) names it).
    pub fn blocks_of(&self, id: &str) -> Vec<&Block> {
        let (module, path) = id.split_once("::").unwrap_or(("", id));
        self.blocks
            .iter()
            .filter(|block| match block.template.as_deref() {
                Some(template) if template == id => true,
                // A template named without its module is of the layout file's.
                Some(template) => {
                    !template.contains("::") && template == path && block.module.as_deref() == Some(module)
                }
                None => false,
            })
            .collect()
    }

    /// Whether `path` is a template: a `.phtml` file of a `templates` folder.
    pub fn is_template(&self, path: &Path) -> bool {
        path.extension().is_some_and(|ext| ext == "phtml")
            && path.components().any(|part| part.as_os_str() == "templates")
    }

    /// The table `name` of a `db_schema.xml`: where a module declares it
    /// with columns first.
    pub fn table(&self, name: &str) -> Option<&Table> {
        let found = self.tables.iter().filter(|table| table.name == name);
        found.max_by_key(|table| table.columns.len())
    }

    /// The columns of the table `name`, of every module adding to it.
    pub fn columns(&self, name: &str) -> Vec<(&Table, &(String, String, Range))> {
        let tables = self.tables.iter().filter(|table| table.name == name);
        tables.flat_map(|table| table.columns.iter().map(move |column| (table, column))).collect()
    }

    /// The modules of the route whose front name is `front`, in `area` when
    /// it is known.
    pub fn route_modules(&self, front: &str, area: Option<&str>) -> Vec<&str> {
        let mut out: Vec<&str> = Vec::new();
        for route in &self.routes {
            let here = area.is_none() || route.area.is_none() || route.area.as_deref() == area;
            if route.front_name == front && here {
                for module in &route.modules {
                    if !out.contains(&module.as_str()) {
                        out.push(module);
                    }
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(files: &[(&str, &str)]) -> (tempfile::TempDir, Arc<MagentoProject>) {
        let tmp = tempfile::tempdir().unwrap();
        for (rel, text) in files {
            let path = tmp.path().join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
        let project = MagentoCache::default().project(tmp.path());
        (tmp, project)
    }

    fn shown(root: &Path, path: &Path) -> String {
        path.strip_prefix(root).unwrap_or(path).to_string_lossy().replace('\\', "/")
    }

    const REGISTRATION: &str = "<?php\nuse Magento\\Framework\\Component\\ComponentRegistrar;\n\nComponentRegistrar::register(ComponentRegistrar::MODULE, 'Shop_Catalog', __DIR__);\n";
    const DI: &str = "<?xml version=\"1.0\"?>\n<config>\n    <preference for=\"Shop\\Catalog\\Api\\CartInterface\" type=\"Shop\\Catalog\\Model\\Cart\"/>\n    <virtualType name=\"ShopLogger\" type=\"Shop\\Catalog\\Model\\Logger\">\n        <arguments><argument name=\"name\" xsi:type=\"string\">shop</argument></arguments>\n    </virtualType>\n    <virtualType name=\"ShopAudit\" type=\"ShopLogger\"/>\n    <type name=\"Shop\\Catalog\\Model\\Cart\">\n        <plugin name=\"limit\" type=\"Shop\\Catalog\\Plugin\\CartLimit\"/>\n        <plugin name=\"off\" type=\"Shop\\Catalog\\Plugin\\Off\" disabled=\"true\"/>\n    </type>\n</config>\n";
    const LAYOUT: &str = "<?xml version=\"1.0\"?>\n<page>\n    <body>\n        <referenceContainer name=\"content\">\n            <block class=\"Shop\\Catalog\\Block\\Cart\" name=\"shop.cart\" as=\"cart\" template=\"Shop_Catalog::cart.phtml\">\n                <arguments>\n                    <argument name=\"view_model\" xsi:type=\"object\">Shop\\Catalog\\ViewModel\\Totals</argument>\n                </arguments>\n                <block name=\"shop.cart.items\" template=\"items.phtml\"/>\n            </block>\n            <container name=\"shop.extra\"/>\n        </referenceContainer>\n    </body>\n</page>\n";

    #[test]
    fn the_xml_files_of_a_module_declare_its_names() {
        let (tmp, project) = project(&[
            (
                "app/etc/di.xml",
                "<config><preference for=\"Psr\\Log\\LoggerInterface\" type=\"Magento\\Framework\\Logger\\Monolog\"/></config>\n",
            ),
            ("app/code/Shop/Catalog/registration.php", REGISTRATION),
            ("app/code/Shop/Catalog/etc/di.xml", DI),
            (
                "app/code/Shop/Catalog/etc/frontend/events.xml",
                "<config>\n    <event name=\"shop_cart_saved\">\n        <observer name=\"notify\" instance=\"Shop\\Catalog\\Observer\\Notify\"/>\n    </event>\n</config>\n",
            ),
            (
                "app/code/Shop/Catalog/etc/acl.xml",
                "<config><acl><resources>\n    <resource id=\"Magento_Backend::admin\">\n        <resource id=\"Shop_Catalog::cart\" title=\"Carts\"/>\n    </resource>\n</resources></acl></config>\n",
            ),
            (
                "app/code/Shop/Catalog/etc/adminhtml/system.xml",
                "<config><system>\n    <section id=\"shop\">\n        <group id=\"cart\">\n            <field id=\"limit\"><label>Limit</label></field>\n            <group id=\"mail\"><field id=\"sender\"/></group>\n        </group>\n    </section>\n</system></config>\n",
            ),
            (
                "app/code/Shop/Catalog/etc/config.xml",
                "<config><default><shop><cart><limit>5</limit></cart></shop></default></config>\n",
            ),
            (
                "app/code/Shop/Catalog/etc/extension_attributes.xml",
                "<config><extension_attributes for=\"Shop\\Catalog\\Api\\Data\\ItemInterface\">\n    <attribute code=\"gift_note\" type=\"string\"/>\n</extension_attributes></config>\n",
            ),
            ("app/code/Shop/Catalog/view/frontend/layout/shop_cart_index.xml", LAYOUT),
            ("app/code/Shop/Catalog/view/frontend/templates/cart.phtml", "<p></p>\n"),
            ("app/design/frontend/Acme/blue/Shop_Catalog/templates/cart.phtml", "<p></p>\n"),
            ("vendor/acme/module-extra/registration.php", &REGISTRATION.replace("Shop_Catalog", "Acme_Extra")),
            ("vendor/acme/not-a-module/composer.json", "{}"),
        ]);
        let root = tmp.path();
        let modules: Vec<(&str, String)> =
            project.modules.iter().map(|module| (module.name.as_str(), shown(root, &module.path))).collect();
        assert_eq!(
            modules,
            [
                ("Shop_Catalog", "app/code/Shop/Catalog".to_owned()),
                ("Acme_Extra", "vendor/acme/module-extra".to_owned())
            ]
        );
        // Dependency injection: a preference, a virtual type of a virtual type, the plugins that are on.
        assert_eq!(
            project.preferred("\\Shop\\Catalog\\Api\\CartInterface", None).unwrap().class,
            "Shop\\Catalog\\Model\\Cart"
        );
        assert_eq!(
            project.preferred("Psr\\Log\\LoggerInterface", None).map(|found| shown(root, &found.file)).as_deref(),
            Some("app/etc/di.xml")
        );
        assert_eq!(project.virtual_class("ShopAudit").as_deref(), Some("Shop\\Catalog\\Model\\Logger"));
        let plugins: Vec<(&str, &str)> =
            project.plugins.iter().map(|plugin| (plugin.target.as_str(), plugin.class.as_str())).collect();
        assert_eq!(plugins, [("Shop\\Catalog\\Model\\Cart", "Shop\\Catalog\\Plugin\\CartLimit")]);
        // Events, ACL, configuration paths, extension attributes.
        assert_eq!(project.observers[0].event, "shop_cart_saved");
        assert_eq!(project.observers[0].class.as_deref(), Some("Shop\\Catalog\\Observer\\Notify"));
        let acl: Vec<(&str, Option<&str>)> =
            project.acl.iter().map(|def| (def.name.as_str(), def.detail.as_deref())).collect();
        assert_eq!(acl, [("Magento_Backend::admin", None), ("Shop_Catalog::cart", Some("Carts"))]);
        let config: Vec<(&str, Option<&str>)> =
            project.config.iter().map(|def| (def.name.as_str(), def.detail.as_deref())).collect();
        assert_eq!(
            config,
            [("shop/cart/limit", Some("5")), ("shop/cart/limit", Some("Limit")), ("shop/cart/mail/sender", None)]
        );
        assert_eq!(project.extension_attributes[0].code, "gift_note");
        // Layout: the handle, its blocks, a block's class, template and view model.
        assert_eq!(project.handles[0].name, "shop_cart_index");
        let blocks: Vec<(&str, Option<&str>, Option<&str>)> = project
            .blocks
            .iter()
            .map(|block| (block.name.as_str(), block.alias.as_deref(), block.class.as_deref()))
            .collect();
        assert_eq!(
            blocks,
            [
                ("shop.cart", Some("cart"), Some("Shop\\Catalog\\Block\\Cart")),
                ("shop.cart.items", None, None),
                ("shop.extra", None, None)
            ]
        );
        assert_eq!(project.blocks[0].view_model.as_deref(), Some("Shop\\Catalog\\ViewModel\\Totals"));
        // A template: a theme's version first; its id from its file; the blocks rendering it.
        let files: Vec<String> = project
            .template_files("Shop_Catalog::cart.phtml", None, None)
            .iter()
            .map(|file| shown(root, file))
            .collect();
        assert_eq!(files[0], "app/design/frontend/Acme/blue/Shop_Catalog/templates/cart.phtml");
        assert_eq!(files[1], "app/code/Shop/Catalog/view/frontend/templates/cart.phtml");
        let id = project.template_id(&root.join("app/code/Shop/Catalog/view/frontend/templates/cart.phtml"));
        assert_eq!(id, Some(("Shop_Catalog::cart.phtml".to_owned(), "frontend".to_owned())));
        let id = project.template_id(&root.join("app/design/frontend/Acme/blue/Shop_Catalog/templates/cart.phtml"));
        assert_eq!(id, Some(("Shop_Catalog::cart.phtml".to_owned(), "frontend".to_owned())));
        assert_eq!(project.blocks_of("Shop_Catalog::cart.phtml")[0].name, "shop.cart");
        assert_eq!(project.blocks_of("Shop_Catalog::items.phtml")[0].name, "shop.cart.items");
    }

    #[test]
    fn a_file_is_read_again_only_when_it_changes() {
        let tmp = tempfile::tempdir().unwrap();
        let write = |rel: &str, text: &str| {
            let path = tmp.path().join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        };
        write("registration.php", REGISTRATION);
        write("etc/module.xml", "<config><module name=\"Shop_Catalog\"/></config>\n");
        write("etc/events.xml", "<config><event name=\"first\"/></config>\n");
        let mut cache = MagentoCache::default();
        // A module opened alone is the project.
        let project = cache.project(tmp.path());
        assert_eq!(project.modules[0].name, "Shop_Catalog");
        assert_eq!(project.observers[0].event, "first");
        // Kept until it is told an XML file was written.
        write("etc/frontend/events.xml", "<config><event name=\"second\"/></config>\n");
        assert_eq!(cache.project(tmp.path()).observers.len(), 1);
        cache.forget();
        let events: Vec<String> = cache.project(tmp.path()).observers.iter().map(|found| found.event.clone()).collect();
        assert_eq!(events, ["first", "second"]);
    }

    #[test]
    fn an_areas_declarations_win_there_and_a_theme_comes_before_its_parents() {
        let theme = |parent: &str| format!("<theme><title>T</title>{parent}</theme>\n");
        let (tmp, project) = project(&[
            ("app/code/Shop/Catalog/registration.php", REGISTRATION),
            (
                "app/code/Shop/Catalog/etc/di.xml",
                "<config><preference for=\"Shop\\Api\\CartInterface\" type=\"Shop\\Model\\Cart\"/></config>\n",
            ),
            (
                "app/code/Shop/Catalog/etc/adminhtml/di.xml",
                "<config><preference for=\"Shop\\Api\\CartInterface\" type=\"Shop\\Model\\AdminCart\"/></config>\n",
            ),
            (
                "app/code/Shop/Catalog/etc/frontend/routes.xml",
                "<config><router id=\"standard\"><route id=\"shop\" frontName=\"store\"><module name=\"Shop_Catalog\"/></route></router></config>\n",
            ),
            (
                "app/code/Shop/Catalog/etc/db_schema.xml",
                "<schema><table name=\"shop_cart\"><column xsi:type=\"int\" name=\"cart_id\"/></table></schema>\n",
            ),
            (
                "app/code/Shop/Catalog/etc/crontab.xml",
                "<config><group id=\"default\"><job name=\"sweep\" instance=\"Shop\\Model\\Sweeper\" method=\"run\"/></group></config>\n",
            ),
            (
                "app/code/Shop/Catalog/etc/queue_consumer.xml",
                "<config><consumer name=\"c\" queue=\"q\" handler=\"Shop\\Model\\Consumer::process\"/></config>\n",
            ),
            ("app/code/Shop/Catalog/view/frontend/templates/cart.phtml", "<p></p>\n"),
            ("app/code/Shop/Catalog/view/adminhtml/templates/cart.phtml", "<p></p>\n"),
            ("app/design/frontend/Acme/a_base/theme.xml", &theme("")),
            ("app/design/frontend/Acme/b_child/theme.xml", &theme("<parent>Acme/a_base</parent>")),
            ("app/design/frontend/Acme/c_grandchild/theme.xml", &theme("<parent>Acme/b_child</parent>")),
            ("app/design/adminhtml/Acme/admin/theme.xml", &theme("")),
            (
                "vendor/acme/theme-extra/registration.php",
                "<?php\n\\Magento\\Framework\\Component\\ComponentRegistrar::register(\\Magento\\Framework\\Component\\ComponentRegistrar::THEME, 'frontend/Acme/extra', __DIR__);\n",
            ),
            ("vendor/acme/theme-extra/theme.xml", &theme("<parent>Acme/c_grandchild</parent>")),
        ]);
        let root = tmp.path();
        // The area's preference there, the global one elsewhere.
        let preferred = |area: Option<&str>| project.preferred("Shop\\Api\\CartInterface", area).unwrap().class.clone();
        assert_eq!(preferred(Some("adminhtml")), "Shop\\Model\\AdminCart");
        assert_eq!(preferred(Some("frontend")), "Shop\\Model\\Cart");
        assert_eq!(preferred(None), "Shop\\Model\\Cart");
        assert_eq!(
            area_of(&root.join("app/code/Shop/Catalog/Controller/Adminhtml/Cart/Index.php")).as_deref(),
            Some("adminhtml")
        );
        assert_eq!(area_of(&root.join("app/code/Shop/Catalog/Model/Cart.php")), None);
        // Themes: one Composer installed too; a theme before the ones it inherits from.
        let chain: Vec<&str> = project.theme_chain(Some("frontend")).iter().map(|theme| theme.code.as_str()).collect();
        assert_eq!(chain, ["Acme/extra", "Acme/c_grandchild", "Acme/b_child", "Acme/a_base"]);
        let parents =
            project.theme_parents(&root.join("app/design/frontend/Acme/b_child/Shop_Catalog/templates/cart.phtml"));
        let parents: Vec<&str> = parents.iter().map(|theme| theme.code.as_str()).collect();
        assert_eq!(parents, ["Acme/b_child", "Acme/a_base"]);
        // A template for an area: its themes, then the module's folder for it and for every area.
        let files: Vec<String> = project
            .template_files("Shop_Catalog::cart.phtml", None, Some("adminhtml"))
            .iter()
            .map(|file| shown(root, file))
            .collect();
        assert_eq!(
            files,
            [
                "app/design/adminhtml/Acme/admin/Shop_Catalog/templates/cart.phtml",
                "app/code/Shop/Catalog/view/adminhtml/templates/cart.phtml",
                "app/code/Shop/Catalog/view/base/templates/cart.phtml",
            ]
        );
        // Routes, tables, and the classes and methods the other files name.
        assert_eq!(project.route_modules("store", Some("frontend")), ["Shop_Catalog"]);
        assert_eq!(project.table("shop_cart").unwrap().columns[0].0, "cart_id");
        let named: Vec<(&str, Option<&str>)> = project
            .mentions()
            .filter(|(file, mention)| mention.what == What::Class && !file.file.ends_with("di.xml"))
            .map(|(_, mention)| (mention.name.as_str(), mention.method.as_ref().map(|(method, _)| method.as_str())))
            .collect();
        assert_eq!(named, [("Shop\\Model\\Sweeper", Some("run")), ("Shop\\Model\\Consumer", Some("process"))]);
    }
}
