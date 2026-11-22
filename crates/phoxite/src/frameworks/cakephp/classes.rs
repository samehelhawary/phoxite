//! The classes CakePHP's names stand for, by its conventions: the table
//! `Articles` is `App\Model\Table\ArticlesTable`, the component `Flash` is
//! `App\Controller\Component\FlashComponent` or CakePHP's own, `Blog.Tags`
//! is the plugin's. A name is a class only when the project has the class.
//!
//! What a class loads comes from its code ([`ClassFacts`]), its own and that
//! of the classes of the project it extends (an `AppController`'s
//! components are every controller's).

use std::sync::Arc;

use super::facts::{Association, ClassFacts, Loaded};
use super::inflect::{pluralize, singularize};
use super::project::{CakeProject, plugin_namespace, plugin_split};
use crate::types::{Resolver, Type};

pub const TABLE: &str = "Cake\\ORM\\Table";
pub const ENTITY: &str = "Cake\\ORM\\Entity";
pub const ENTITY_INTERFACE: &str = "Cake\\Datasource\\EntityInterface";
pub const CONTROLLER: &str = "Cake\\Controller\\Controller";
pub const COMPONENT: &str = "Cake\\Controller\\Component";
pub const VIEW: &str = "Cake\\View\\View";
pub const HELPER: &str = "Cake\\View\\Helper";
pub const BEHAVIOR: &str = "Cake\\ORM\\Behavior";

/// A kind of class a name stands for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Table,
    Entity,
    Behavior,
    Component,
    Helper,
    Cell,
    Controller,
}

impl Kind {
    /// The namespace below the application's or a plugin's.
    pub fn folder(self) -> &'static str {
        match self {
            Self::Table => "Model\\Table",
            Self::Entity => "Model\\Entity",
            Self::Behavior => "Model\\Behavior",
            Self::Component => "Controller\\Component",
            Self::Helper => "View\\Helper",
            Self::Cell => "View\\Cell",
            Self::Controller => "Controller",
        }
    }

    /// What follows the name in the class's.
    pub fn suffix(self) -> &'static str {
        match self {
            Self::Table => "Table",
            Self::Entity => "",
            Self::Behavior => "Behavior",
            Self::Component => "Component",
            Self::Helper => "Helper",
            Self::Cell => "Cell",
            Self::Controller => "Controller",
        }
    }

    /// The namespace of CakePHP's own.
    pub fn core(self) -> Option<&'static str> {
        match self {
            Self::Behavior => Some("Cake\\ORM\\Behavior"),
            Self::Component => Some("Cake\\Controller\\Component"),
            Self::Helper => Some("Cake\\View\\Helper"),
            _ => None,
        }
    }

    /// The class every one of the kind extends.
    pub fn base(self) -> &'static str {
        match self {
            Self::Table => TABLE,
            Self::Entity => ENTITY,
            Self::Behavior => BEHAVIOR,
            Self::Component => COMPONENT,
            Self::Helper => HELPER,
            Self::Cell => "Cake\\View\\Cell",
            Self::Controller => CONTROLLER,
        }
    }
}

/// The namespaces the classes of `kind` named without a plugin are looked
/// for in (the application's, then CakePHP's), or the plugin's.
pub fn namespaces(project: &CakeProject, kind: Kind, plugin: Option<&str>) -> Vec<String> {
    match plugin {
        Some(plugin) => vec![format!("{}\\{}", plugin_namespace(plugin), kind.folder())],
        None => {
            let mut out = vec![format!("{}\\{}", project.namespace, kind.folder())];
            out.extend(kind.core().map(str::to_owned));
            out
        }
    }
}

/// The class the name `name` of a `kind` stands for: a class name as it is,
/// `Blog.Tags` in the plugin, any other in the application and then in
/// CakePHP. `None` when the project has no such class.
pub fn class_of(resolver: &Resolver<'_>, project: &CakeProject, kind: Kind, name: &str) -> Option<String> {
    let known = |class: &str| resolver.class_model(class).map(|model| model.fqn().to_owned());
    if name.contains('\\') {
        return known(name.trim_start_matches('\\'));
    }
    let (plugin, short) = plugin_split(name);
    if short.is_empty() || !short.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '/')) {
        return None;
    }
    let short = short.replace('/', "\\");
    let found = namespaces(project, kind, plugin);
    found.iter().find_map(|namespace| known(&format!("{namespace}\\{short}{}", kind.suffix())))
}

/// The type of the table `alias`: its class, else CakePHP's `Table` (a
/// table without a class of its own is one).
pub fn table_type(resolver: &Resolver<'_>, project: &CakeProject, alias: &str) -> Option<Type> {
    let class = class_of(resolver, project, Kind::Table, alias);
    let class = class.or_else(|| resolver.class_model(TABLE).map(|model| model.fqn().to_owned()))?;
    Some(Type::named(&class))
}

/// What `class` and the classes of the project it extends say they have,
/// the class's own first.
pub fn facts(resolver: &Resolver<'_>, class: &str) -> Vec<Arc<ClassFacts>> {
    let Some(model) = resolver.class_model(class) else { return Vec::new() };
    let own = !model.fqn().to_ascii_lowercase().starts_with("cake\\");
    let mut classes: Vec<&str> = if own { vec![model.fqn()] } else { Vec::new() };
    classes.extend(
        model
            .parents
            .iter()
            .map(String::as_str)
            .filter(|parent| !parent.trim_start_matches('\\').to_ascii_lowercase().starts_with("cake\\")),
    );
    classes.into_iter().filter_map(|class| resolver.cake_facts(class.trim_start_matches('\\'))).collect()
}

/// The first of what `class` and its ancestors load with `list` that is
/// under the name `alias`.
pub fn loaded(
    resolver: &Resolver<'_>,
    class: &str,
    alias: &str,
    list: impl Fn(&ClassFacts) -> &Vec<Loaded>,
) -> Option<Loaded> {
    facts(resolver, class).iter().find_map(|facts| list(facts).iter().find(|loaded| loaded.alias() == alias).cloned())
}

/// Everything `class` and its ancestors load with `list`, a name once.
pub fn all_loaded(resolver: &Resolver<'_>, class: &str, list: impl Fn(&ClassFacts) -> &Vec<Loaded>) -> Vec<Loaded> {
    let mut out: Vec<Loaded> = Vec::new();
    for facts in facts(resolver, class) {
        for loaded in list(&facts) {
            if !out.iter().any(|known| known.alias() == loaded.alias()) {
                out.push(loaded.clone());
            }
        }
    }
    out
}

/// The associations of the table `class`, a name once.
pub fn associations(resolver: &Resolver<'_>, class: &str) -> Vec<Association> {
    let mut out: Vec<Association> = Vec::new();
    for facts in facts(resolver, class) {
        for association in &facts.associations {
            if !out.iter().any(|known| known.loaded.alias() == association.loaded.alias()) {
                out.push(association.clone());
            }
        }
    }
    out
}

/// `Articles` of `App\Model\Table\ArticlesTable`.
pub fn alias_of(table: &str) -> &str {
    let short = table.rsplit('\\').next().unwrap_or(table);
    short.strip_suffix("Table").filter(|alias| !alias.is_empty()).unwrap_or(short)
}

/// The namespace above `\Model\Table\…` or `\Controller\…` of a class:
/// the application's, or a plugin's.
fn root_namespace<'c>(class: &'c str, folder: &str) -> Option<&'c str> {
    let at = class.rfind(&format!("\\{folder}\\"))?;
    Some(&class[..at])
}

/// The entity class of the table class `table`: what `setEntityClass()`
/// names, else the singular of the table's name among the entities beside
/// it (`ArticlesTable` has `Article`), else CakePHP's `Entity`.
pub fn entity_of(resolver: &Resolver<'_>, project: &CakeProject, table: &str) -> Option<String> {
    let known = |class: &str| resolver.class_model(class).map(|model| model.fqn().to_owned());
    let model = resolver.class_model(table)?;
    let set = facts(resolver, model.fqn()).iter().find_map(|facts| facts.entity_class.clone());
    if let Some(found) = set.and_then(|name| class_of(resolver, project, Kind::Entity, &name)) {
        return Some(found);
    }
    let by_name = root_namespace(model.fqn(), "Model\\Table").and_then(|root| {
        let alias = alias_of(model.fqn());
        known(&format!("{root}\\Model\\Entity\\{}", singularize(alias)))
    });
    by_name.or_else(|| known(ENTITY))
}

/// The table class of the entity class `entity`: the plural of its name
/// among the tables beside it (`Article` is of `ArticlesTable`).
pub fn table_of(resolver: &Resolver<'_>, entity: &str) -> Option<String> {
    let model = resolver.class_model(entity)?;
    let root = root_namespace(model.fqn(), "Model\\Entity")?;
    let short = model.fqn().rsplit('\\').next()?;
    let table = format!("{root}\\Model\\Table\\{}Table", pluralize(short));
    resolver.class_model(&table).map(|model| model.fqn().to_owned())
}

/// The table a controller has as a property and paginates by default: its
/// `$defaultTable`, else the one of its name (`ArticlesController` has
/// `Articles`).
pub fn default_table(resolver: &Resolver<'_>, controller: &str) -> Option<String> {
    let model = resolver.class_model(controller)?;
    if let Some(set) = facts(resolver, model.fqn()).iter().find_map(|facts| facts.default_table.clone()) {
        return Some(if set.contains('\\') { alias_of(&set).to_owned() } else { set });
    }
    let short = model.fqn().rsplit('\\').next()?;
    let name = short.strip_suffix("Controller").filter(|name| !name.is_empty() && *name != "App")?;
    // A plugin's controller has the plugin's table.
    let root = root_namespace(model.fqn(), "Controller");
    let plugin = root.filter(|root| resolver.cake().is_some_and(|project| project.namespace != **root));
    Some(match plugin {
        Some(plugin) => format!("{}.{name}", plugin.replace('\\', "/")),
        None => name.to_owned(),
    })
}
