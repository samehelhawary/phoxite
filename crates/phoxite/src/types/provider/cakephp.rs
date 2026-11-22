//! CakePHP 4 and 5: what its conventions make of a name.
//!
//! - **Tables:** `$this->fetchTable('Articles')`, a table locator's
//!   `get('Articles')` and `loadModel('Articles')` are the application's
//!   `ArticlesTable` (`Blog.Tags` a plugin's), else CakePHP's `Table`. A
//!   controller has the table of its name as a property (`$this->Articles`
//!   in `ArticlesController`, or what `$defaultTable` names), and
//!   `fetchTable()` without a name is that table.
//! - **Entities:** a table's `get()`, `newEntity()`, `newEmptyEntity()`,
//!   `patchEntity()`, `save()` and `findOrCreate()` give its entity class
//!   (`Article` for `ArticlesTable`, or what `setEntityClass()` names), and
//!   `find()` a query of it: `first()` is the entity or null, `toArray()`
//!   and `all()` hold it. Where CakePHP's own declarations already say the
//!   entity (a table documented with `@extends Table<…, Article>`), they
//!   are kept.
//! - **Associations:** `$this->Articles->Authors` is the association and the
//!   table it leads to; `$article->author` and `$article->comments` are the
//!   associated entity, or a list of them.
//! - **Components and helpers:** `$this->Flash` in a controller is the
//!   component `loadComponent('Flash')` loads (the controller's own or one
//!   of the classes it extends); `$this->Html` in a view or a template is
//!   the helper of that name.
//! - **Behaviours:** `getBehavior('Timestamp')` is the behaviour's class.
//!
//! Only in a CakePHP project. The behaviour follows CakePHP's book
//! (Controllers, Views, Database Access & ORM).

use super::{AttributeOrigin, CallKind, CallSite, MagicMember, MagicTarget, Source, TypeProvider, provided};
use crate::frameworks::cakephp::CakeProject;
use crate::frameworks::cakephp::classes::{
    self, CONTROLLER, ENTITY, ENTITY_INTERFACE, HELPER, Kind, TABLE, VIEW, all_loaded, associations, class_of,
    default_table, entity_of, loaded, table_of, table_type,
};
use crate::frameworks::cakephp::facts::{Association, Loaded};
use crate::frameworks::cakephp::migrations;
use crate::summary::Loc;
use crate::types::{Context, LazyType, Resolver, Type};

const LOCATORS: [&str; 3] =
    ["Cake\\ORM\\Locator\\LocatorInterface", "Cake\\Datasource\\Locator\\LocatorInterface", "Cake\\ORM\\TableRegistry"];
const QUERIES: [&str; 3] = ["Cake\\ORM\\Query\\SelectQuery", "Cake\\ORM\\Query", "Cake\\Datasource\\QueryInterface"];
const RESULTS: [&str; 2] = ["Cake\\Datasource\\ResultSetInterface", "Cake\\Collection\\CollectionInterface"];
const PAGINATED: [&str; 2] = ["Cake\\Datasource\\Paging\\PaginatedInterface", "Cake\\Datasource\\ResultSetInterface"];

/// The table's methods that give its entity.
const ENTITY_METHODS: [&str; 8] =
    ["get", "newEntity", "newEmptyEntity", "patchEntity", "save", "saveOrFail", "findOrCreate", "loadInto"];
/// Those that give several.
const ENTITIES_METHODS: [&str; 2] = ["newEntities", "patchEntities"];
/// Those that give a query.
const QUERY_METHODS: [&str; 3] = ["find", "query", "selectQuery"];
/// A query's and a result set's methods that give what was found.
const RESULT_METHODS: [&str; 6] = ["first", "firstOrFail", "last", "all", "toArray", "toList"];

/// See the module documentation.
pub struct CakePhpProvider;

fn is_one_of(resolver: &Resolver<'_>, class: &str, bases: &[&str]) -> bool {
    resolver.class_model(class).is_some_and(|model| bases.iter().any(|base| model.is_subclass_of(base)))
}

/// The class among the members of `ty` that is one of `bases`.
fn class_among(ty: &Type, bases: &[&str], resolver: &Resolver<'_>, context: &Context) -> Option<String> {
    let models = resolver.class_models(ty, context);
    models.iter().find(|model| bases.iter().any(|base| model.is_subclass_of(base))).map(|model| model.fqn().to_owned())
}

/// Whether `ty` names a class of the project: an answer that already says
/// which entity or table it is.
fn is_precise(ty: &Type) -> bool {
    fn named(ty: &Type, out: &mut Vec<String>) {
        match ty {
            Type::Named { fqn, args } => {
                out.push(fqn.trim_start_matches('\\').to_ascii_lowercase());
                args.iter().for_each(|arg| named(arg, out));
            }
            Type::Union(members) | Type::Intersection(members) => members.iter().for_each(|member| named(member, out)),
            Type::List { value, .. } => named(value, out),
            Type::Array { key, value, .. } | Type::Iterable { key, value } => {
                named(key, out);
                named(value, out);
            }
            _ => {}
        }
    }
    let mut classes = Vec::new();
    named(ty, &mut classes);
    classes.iter().any(|class| !class.starts_with("cake\\") && !class.starts_with("psr\\"))
}

/// The first class among `classes` the project has.
fn first_known(resolver: &Resolver<'_>, classes: &[&str]) -> Option<String> {
    classes.iter().find_map(|class| resolver.class_model(class).map(|model| model.fqn().to_owned()))
}

impl CakePhpProvider {
    /// The entity of the table class `table`, as a type.
    fn entity(resolver: &Resolver<'_>, project: &CakeProject, table: &str) -> Option<Type> {
        entity_of(resolver, project, table).map(|class| Type::named(&class))
    }

    /// What a query or a result set of `ty` holds: its last generic
    /// argument.
    fn held(ty: &Type, bases: &[&str], resolver: &Resolver<'_>) -> Option<Type> {
        ty.members().iter().find_map(|member| {
            let Type::Named { fqn, args } = member else { return None };
            (is_one_of(resolver, fqn, bases)).then(|| args.last().cloned())?
        })
    }

    /// The type of an association of a table: the association and the table
    /// it leads to, whose methods it answers.
    fn association(resolver: &Resolver<'_>, project: &CakeProject, association: &Association) -> Option<Type> {
        let target = table_type(resolver, project, association.loaded.target())?;
        Some(match resolver.class_model(association.kind.class()) {
            Some(class) => Type::intersection([Type::named(class.fqn()), target]),
            None => target,
        })
    }

    /// The entity an association of a table leads to.
    fn associated(resolver: &Resolver<'_>, project: &CakeProject, association: &Association) -> Option<Type> {
        let table = class_of(resolver, project, Kind::Table, association.loaded.target());
        let entity = match table {
            Some(table) => Self::entity(resolver, project, &table)?,
            None => Type::named(&first_known(resolver, &[ENTITY])?),
        };
        Some(if association.kind.many() { Type::list_of(entity) } else { entity.nullable() })
    }

    /// The type of what `class` loaded as `found`, of `kind`.
    fn loaded_type(resolver: &Resolver<'_>, project: &CakeProject, kind: Kind, found: &Loaded) -> Option<Type> {
        class_of(resolver, project, kind, found.target()).map(|class| Type::named(&class))
    }

    /// What the property `name` of `class` is by CakePHP's conventions.
    fn convention(class: &str, name: &str, resolver: &Resolver<'_>) -> Option<Type> {
        let project = resolver.cake()?;
        let model = resolver.class_model(class)?;
        let fqn = model.fqn();
        if model.is_subclass_of(CONTROLLER) {
            if let Some(found) = loaded(resolver, fqn, name, |facts| &facts.components) {
                return Self::loaded_type(resolver, &project, Kind::Component, &found);
            }
            if let Some(found) = loaded(resolver, fqn, name, |facts| &facts.tables) {
                return table_type(resolver, &project, found.target());
            }
            let default = default_table(resolver, fqn)?;
            let alias = crate::frameworks::cakephp::project::plugin_split(&default).1;
            return (alias == name).then(|| table_type(resolver, &project, &default))?;
        }
        if model.is_subclass_of(classes::COMPONENT) {
            let found = loaded(resolver, fqn, name, |facts| &facts.components)?;
            return Self::loaded_type(resolver, &project, Kind::Component, &found);
        }
        if model.is_subclass_of(TABLE) {
            let found = associations(resolver, fqn).into_iter().find(|found| found.loaded.alias() == name)?;
            return Self::association(resolver, &project, &found);
        }
        if model.is_subclass_of(VIEW) || model.is_subclass_of(HELPER) {
            if let Some(found) = loaded(resolver, fqn, name, |facts| &facts.helpers) {
                return Self::loaded_type(resolver, &project, Kind::Helper, &found);
            }
            // A view loads the helper of a name when it is asked for.
            let named = name.starts_with(|c: char| c.is_ascii_uppercase());
            return (named && model.is_subclass_of(VIEW))
                .then(|| class_of(resolver, &project, Kind::Helper, name))?
                .map(|class| Type::named(&class));
        }
        if model.is_subclass_of(ENTITY_INTERFACE) {
            let table = table_of(resolver, fqn)?;
            if let Some(found) = associations(resolver, &table).into_iter().find(|found| found.property() == name) {
                return Self::associated(resolver, &project, &found);
            }
            // A column of the entity's table, as the migrations make it.
            let (_, columns) = migrations::columns_of(resolver, fqn)?;
            return columns.iter().find(|column| column.name == name).map(|column| Self::column(resolver, column));
        }
        None
    }

    /// The type of an entity's field that is the column `column`: CakePHP
    /// gives dates and times as its own objects.
    fn column(resolver: &Resolver<'_>, column: &migrations::Column) -> Type {
        use migrations::ColumnType;
        let class = |classes: &[&str]| first_known(resolver, classes).map_or(Type::Mixed, |class| Type::named(&class));
        let modern = resolver.class_model("Cake\\I18n\\DateTime").is_some();
        let ty = match column.ty {
            ColumnType::Int => Type::Int,
            ColumnType::Float => Type::Float,
            ColumnType::Bool => Type::Bool,
            ColumnType::String => Type::string(),
            ColumnType::Mixed => return Type::Mixed,
            ColumnType::DateTime => class(&["Cake\\I18n\\DateTime", "Cake\\I18n\\FrozenTime", "DateTimeInterface"]),
            ColumnType::Date => class(&["Cake\\I18n\\Date", "Cake\\I18n\\FrozenDate", "DateTimeInterface"]),
            // CakePHP 5 has a class for a time of day; before it, a time is a date and a time.
            ColumnType::Time if modern => class(&["Cake\\I18n\\Time"]),
            ColumnType::Time => class(&["Cake\\I18n\\FrozenTime", "DateTimeInterface"]),
        };
        if ty == Type::Mixed || !column.nullable { ty } else { ty.nullable() }
    }
}

impl TypeProvider for CakePhpProvider {
    fn key(&self) -> &'static str {
        "cakephp"
    }

    fn applies(&self, resolver: &Resolver<'_>) -> bool {
        resolver.is_cake()
    }

    fn call(&self, site: &CallSite<'_>) -> Option<LazyType> {
        // Kept in a summary: only the few names that are CakePHP's own.
        if site.stored
            && !["fetchTable", "loadModel", "getBehavior", "loadComponent", "loadHelper"].contains(&site.name)
        {
            return None;
        }
        let named = |name: &str| site.name.eq_ignore_ascii_case(name);
        let among = |names: &[&str]| names.iter().any(|name| named(name));
        let first = site.args.first().filter(|arg| arg.name.is_none());
        let string =
            first.and_then(|arg| arg.string.as_deref().or(arg.class.as_deref())).filter(|name| !name.is_empty());
        let answer = |payload: String| {
            let receiver = site.receiver?.clone();
            let next = (site.next)();
            Some(provided(self.key(), payload, vec![receiver, next.clone()], next))
        };
        match site.kind {
            // `$this->fetchTable('Articles')`, `fetchTable()`.
            CallKind::Method if named("fetchTable") || named("loadModel") => {
                answer(format!("table:{}", string.unwrap_or_default()))
            }
            // `$this->paginate($this->Articles)`, `paginate('Articles')`, `paginate()`.
            CallKind::Method if named("paginate") && !site.stored => {
                let receiver = site.receiver?.clone();
                let next = (site.next)();
                let mut args = vec![receiver, next.clone()];
                args.extend((site.arg_types)().into_iter().next());
                Some(provided(self.key(), format!("paginate:{}", string.unwrap_or_default()), args, next))
            }
            CallKind::Method if named("getBehavior") => answer(format!("behavior:{}", string?)),
            CallKind::Method if named("loadComponent") => answer(format!("component:{}", string?)),
            CallKind::Method if named("loadHelper") => answer(format!("helper:{}", string?)),
            // The rest is asked of many classes: only where an expression is typed.
            _ if site.stored => None,
            // A locator's `get('Articles')`.
            CallKind::Method | CallKind::StaticMethod if named("get") && string.is_some() => {
                answer(format!("locate:{}", string?))
            }
            CallKind::Method if among(&ENTITY_METHODS) => answer("entity".to_owned()),
            CallKind::Method if among(&ENTITIES_METHODS) => answer("entities".to_owned()),
            CallKind::Method if among(&QUERY_METHODS) => answer("query".to_owned()),
            CallKind::Method if among(&RESULT_METHODS) => answer(format!("result:{}", site.name.to_ascii_lowercase())),
            _ => None,
        }
    }

    fn magic_property(&self, class: &str, name: &str, resolver: &Resolver<'_>) -> Option<Type> {
        resolver.is_cake().then(|| Self::convention(class, name, resolver))?
    }

    /// A property documented or declared without a type is what the
    /// conventions say.
    fn declared_property(&self, class: &str, name: &str, declared: &Type, resolver: &Resolver<'_>) -> Option<Type> {
        (*declared == Type::Mixed && resolver.is_cake()).then(|| Self::convention(class, name, resolver))?
    }

    /// A controller's components and tables, a table's associations, a
    /// view's helpers and an entity's associated entities.
    fn magic_members(&self, receiver: &Type, resolver: &Resolver<'_>, context: &Context) -> Vec<MagicMember> {
        let Some(project) = resolver.cake() else { return Vec::new() };
        let mut out: Vec<MagicMember> = Vec::new();
        let add = |out: &mut Vec<MagicMember>,
                   class: &str,
                   name: String,
                   ty: Option<Type>,
                   what: &str,
                   at: Option<&Loaded>| {
            let Some(ty) = ty else { return };
            if out.iter().any(|member| member.name == name) {
                return;
            }
            let source =
                at.map(|found| Source { file: found.file.clone(), loc: Loc { range: found.range, ..Loc::default() } });
            out.push(MagicMember {
                name,
                is_static: false,
                target: MagicTarget::Attribute {
                    class: class.to_owned(),
                    ty,
                    origin: AttributeOrigin::Configured { what: what.to_owned(), source },
                },
            });
        };
        for model in resolver.class_models(receiver, context) {
            let fqn = model.fqn();
            let declared = |name: &str| model.properties.get(name).is_some();
            if model.is_subclass_of(CONTROLLER) || model.is_subclass_of(classes::COMPONENT) {
                for found in all_loaded(resolver, fqn, |facts| &facts.components) {
                    if !declared(found.alias()) {
                        let ty = Self::loaded_type(resolver, &project, Kind::Component, &found);
                        add(&mut out, fqn, found.alias().to_owned(), ty, "CakePHP component", Some(&found));
                    }
                }
            }
            if model.is_subclass_of(CONTROLLER) {
                for found in all_loaded(resolver, fqn, |facts| &facts.tables) {
                    if !declared(found.alias()) {
                        let ty = table_type(resolver, &project, found.target());
                        add(&mut out, fqn, found.alias().to_owned(), ty, "CakePHP table", Some(&found));
                    }
                }
                if let Some(default) = default_table(resolver, fqn) {
                    let alias = crate::frameworks::cakephp::project::plugin_split(&default).1.to_owned();
                    // Only a table that has a class: every controller has a name.
                    let ty = class_of(resolver, &project, Kind::Table, &default).map(|class| Type::named(&class));
                    if !declared(&alias) {
                        add(&mut out, fqn, alias, ty, "CakePHP table", None);
                    }
                }
            }
            if model.is_subclass_of(TABLE) {
                for found in associations(resolver, fqn) {
                    if !declared(found.loaded.alias()) {
                        let ty = Self::association(resolver, &project, &found);
                        add(
                            &mut out,
                            fqn,
                            found.loaded.alias().to_owned(),
                            ty,
                            "CakePHP association",
                            Some(&found.loaded),
                        );
                    }
                }
            }
            if model.is_subclass_of(VIEW) || model.is_subclass_of(HELPER) {
                for found in all_loaded(resolver, fqn, |facts| &facts.helpers) {
                    if !declared(found.alias()) {
                        let ty = Self::loaded_type(resolver, &project, Kind::Helper, &found);
                        add(&mut out, fqn, found.alias().to_owned(), ty, "CakePHP helper", Some(&found));
                    }
                }
            }
            if model.is_subclass_of(VIEW) {
                // The helpers a view loads when asked: the application's and CakePHP's.
                for namespace in classes::namespaces(&project, Kind::Helper, None) {
                    for class in resolver.namespace_classes(&namespace) {
                        let short = class.rsplit('\\').next().unwrap_or(&class);
                        let Some(name) = short.strip_suffix("Helper").filter(|name| !name.is_empty()) else { continue };
                        let helper = resolver.class_model(&class).filter(|helper| helper.is_subclass_of(HELPER));
                        if !declared(name) {
                            let ty = helper.map(|helper| Type::named(helper.fqn()));
                            add(&mut out, fqn, name.to_owned(), ty, "CakePHP helper", None);
                        }
                    }
                }
            }
            if model.is_subclass_of(ENTITY_INTERFACE)
                && let Some(table) = table_of(resolver, fqn)
            {
                for found in associations(resolver, &table) {
                    let name = found.property();
                    if !declared(&name) {
                        let ty = Self::associated(resolver, &project, &found);
                        add(&mut out, fqn, name, ty, "CakePHP association", Some(&found.loaded));
                    }
                }
            }
            if model.is_subclass_of(ENTITY_INTERFACE)
                && let Some((table, columns)) = migrations::columns_of(resolver, fqn)
            {
                for column in columns {
                    if declared(&column.name) {
                        continue;
                    }
                    let at = Loaded {
                        name: column.name.clone(),
                        class_name: None,
                        property: None,
                        file: column.file.clone(),
                        range: column.range,
                    };
                    let ty = Some(Self::column(resolver, &column));
                    add(&mut out, fqn, column.name.clone(), ty, &format!("column of `{table}`"), Some(&at));
                }
            }
        }
        out
    }

    fn resolve(&self, payload: &str, args: &[Type], resolver: &Resolver<'_>, context: &Context) -> Option<Type> {
        let project = resolver.cake()?;
        let receiver = args.first()?;
        let next = args.get(1).cloned().unwrap_or(Type::Mixed);
        // What CakePHP's own declarations say is kept where it names the project's classes.
        let unless_precise = |ty: Option<Type>| if is_precise(&next) { None } else { ty };
        let table = || class_among(receiver, &[TABLE], resolver, context);
        let query = |entity: Type| {
            let class = first_known(resolver, &QUERIES[..2])?;
            Some(Type::generic(&class, vec![entity]))
        };
        match payload {
            "entity" => unless_precise(Self::entity(resolver, &project, &table()?)),
            "entities" => unless_precise(Self::entity(resolver, &project, &table()?).map(Type::list_of)),
            "query" => unless_precise(query(Self::entity(resolver, &project, &table()?)?)),
            _ => {
                let (what, name) = payload.split_once(':')?;
                match what {
                    "table" => {
                        if !name.is_empty() {
                            return table_type(resolver, &project, name);
                        }
                        // Without a name: the table of the controller asking.
                        let controller = class_among(receiver, &[CONTROLLER], resolver, context)?;
                        table_type(resolver, &project, &default_table(resolver, &controller)?)
                    }
                    "locate" => {
                        class_among(receiver, &LOCATORS, resolver, context)?;
                        table_type(resolver, &project, name)
                    }
                    "behavior" => {
                        table()?;
                        class_of(resolver, &project, Kind::Behavior, name).map(|class| Type::named(&class))
                    }
                    "component" => {
                        class_among(receiver, &[CONTROLLER], resolver, context)?;
                        class_of(resolver, &project, Kind::Component, name).map(|class| Type::named(&class))
                    }
                    "helper" => {
                        class_among(receiver, &[VIEW], resolver, context)?;
                        class_of(resolver, &project, Kind::Helper, name).map(|class| Type::named(&class))
                    }
                    "paginate" => {
                        let controller = class_among(receiver, &[CONTROLLER], resolver, context)?;
                        let given = args.get(2);
                        let of_table = |table: &str| Self::entity(resolver, &project, table);
                        let entity = if !name.is_empty() {
                            of_table(&class_of(resolver, &project, Kind::Table, name)?)
                        } else if let Some(given) = given {
                            match class_among(given, &[TABLE], resolver, context) {
                                Some(table) => of_table(&table),
                                None => Self::held(given, &QUERIES, resolver),
                            }
                        } else {
                            let default = default_table(resolver, &controller)?;
                            of_table(&class_of(resolver, &project, Kind::Table, &default)?)
                        }?;
                        if is_precise(&next) {
                            return None;
                        }
                        let class = first_known(resolver, &PAGINATED)?;
                        Some(Type::generic(&class, vec![Type::Int, entity]))
                    }
                    "result" => {
                        let held = Self::held(receiver, &QUERIES, resolver)
                            .or_else(|| Self::held(receiver, &RESULTS, resolver))
                            .or_else(|| Self::held(receiver, &PAGINATED, resolver))?;
                        if is_precise(&next) || !is_precise(&held) {
                            return None;
                        }
                        match name {
                            "first" | "last" => Some(held.nullable()),
                            "firstorfail" => Some(held),
                            "toarray" | "tolist" => Some(Type::list_of(held)),
                            "all" => {
                                class_among(receiver, &QUERIES, resolver, context)?;
                                let class = first_known(resolver, &RESULTS[..1])?;
                                Some(Type::generic(&class, vec![Type::Int, held]))
                            }
                            _ => None,
                        }
                    }
                    _ => None,
                }
            }
        }
    }
}
