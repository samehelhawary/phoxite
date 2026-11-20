//! Drupal: the service container and entity storages.
//!
//! - **Services:** `\Drupal::service('id')` and `$container->get('id')` are
//!   the class the id's definition in a `*.services.yml` makes, through
//!   aliases and parents ([`crate::frameworks::drupal::DrupalProject`]; the
//!   engine puts those services into the configuration
//!   [`Resolver::symfony`] answers with).
//! - **Entity storages:** `getStorage('node')` on the entity type manager is
//!   the storage handler the entity type's attribute or annotation names
//!   (else `EntityStorageInterface`), of the entity's class: on it `load()`,
//!   `loadUnchanged()` and `loadRevision()` are `?Node`, `create()` is
//!   `Node`, and `loadMultiple()` and `loadByProperties()` are lists of it.
//!   Drupal's own declarations say `EntityInterface` there.
//!
//! - **Configuration:** `Drupal::config('shop.settings')`, `$this->config()`
//!   and the config factory's `get()` and `getEditable()` are the
//!   configuration class they declare, of that name: the `drupalConfigKey`
//!   kind reads it for the keys of `->get('…')`. `->get('key')` on it has the
//!   type the configuration's schema gives the key (`config/schema`).
//!
//! - **Queries:** `db_select('node', 'n')` and the connection's `select()`,
//!   `insert()`, `update()`, `delete()` and `merge()` are the query class
//!   they declare, of that table: the `drupalColumn` kind reads it for the
//!   columns of `->condition('nid', …)`. In Drupal 7 too.
//!
//! Only where Drupal's `Drupal` class is known (but for queries). The behaviour follows
//! Drupal's API documentation; nothing of its code is here.

use super::{CallKind, CallSite, TypeProvider, provided};
use crate::types::{Context, LazyType, Resolver, Type};

const DRUPAL: &str = "Drupal";
const CONTAINERS: [&str; 2] =
    ["Psr\\Container\\ContainerInterface", "Symfony\\Component\\DependencyInjection\\ContainerInterface"];
const ENTITY_TYPE_MANAGER: &str = "Drupal\\Core\\Entity\\EntityTypeManagerInterface";
const ENTITY_STORAGE: &str = "Drupal\\Core\\Entity\\EntityStorageInterface";
const CONNECTIONS: [&str; 2] = ["Drupal\\Core\\Database\\Connection", "DatabaseConnection"];
const QUERY_FUNCTIONS: [&str; 5] = ["db_select", "db_insert", "db_update", "db_delete", "db_merge"];
const QUERY_METHODS: [&str; 5] = ["select", "insert", "update", "delete", "merge"];
/// The query classes: Drupal 7's, and the interfaces of the later ones.
const QUERIES: [&str; 13] = [
    "SelectQueryInterface",
    "SelectQuery",
    "QueryConditionInterface",
    "InsertQuery",
    "UpdateQuery",
    "DeleteQuery",
    "MergeQuery",
    "Drupal\\Core\\Database\\Query\\SelectInterface",
    "Drupal\\Core\\Database\\Query\\ConditionInterface",
    "Drupal\\Core\\Database\\Query\\Insert",
    "Drupal\\Core\\Database\\Query\\Update",
    "Drupal\\Core\\Database\\Query\\Delete",
    "Drupal\\Core\\Database\\Query\\Merge",
];
/// The methods of a query that return the query.
const FLUENT: [&str; 17] = [
    "fields",
    "condition",
    "isNull",
    "isNotNull",
    "orderBy",
    "groupBy",
    "range",
    "distinct",
    "extend",
    "addTag",
    "addMetaData",
    "having",
    "exists",
    "notExists",
    "orderRandom",
    "forUpdate",
    "expression",
];
const CONFIG_BASE: &str = "Drupal\\Core\\Config\\ConfigBase";

/// What a storage's method returns of its entity: one that may be missing,
/// one, or several.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Loaded {
    Maybe,
    One,
    Many,
}

fn loaded(method: &str) -> Option<Loaded> {
    const METHODS: [(&str, Loaded); 7] = [
        ("load", Loaded::Maybe),
        ("loadUnchanged", Loaded::Maybe),
        ("loadRevision", Loaded::Maybe),
        ("create", Loaded::One),
        ("loadMultiple", Loaded::Many),
        ("loadByProperties", Loaded::Many),
        ("loadMultipleRevisions", Loaded::Many),
    ];
    METHODS.iter().find(|(name, _)| name.eq_ignore_ascii_case(method)).map(|(_, loaded)| *loaded)
}

/// See the module documentation.
pub struct DrupalProvider;

impl DrupalProvider {
    fn is_a(resolver: &Resolver<'_>, ty: &Type, context: &Context, classes: &[&str]) -> bool {
        resolver.class_models(ty, context).iter().any(|model| classes.iter().any(|class| model.is_subclass_of(class)))
    }

    fn named(resolver: &Resolver<'_>, class: &str) -> Type {
        let class = class.trim_start_matches('\\');
        resolver.class_model(class).map_or_else(|| Type::named(class), |model| Type::named(model.fqn()))
    }

    /// The name of the configuration `receiver` is, when it is one of a
    /// known name (`Config<'shop.settings'>`).
    fn config_name(resolver: &Resolver<'_>, receiver: &Type, context: &Context) -> Option<String> {
        receiver.members().iter().find_map(|member| match member {
            Type::Named { args, .. } if Self::is_a(resolver, member, context, &[CONFIG_BASE]) => {
                args.iter().find_map(|arg| match arg {
                    Type::LiteralString(name) => Some(name.clone()),
                    _ => None,
                })
            }
            _ => None,
        })
    }

    fn service(resolver: &Resolver<'_>, id: &str) -> Option<Type> {
        let class = resolver.symfony().config.service_class(id)?;
        Some(Self::named(resolver, &class))
    }
}

impl TypeProvider for DrupalProvider {
    fn key(&self) -> &'static str {
        "drupal"
    }

    fn call(&self, site: &CallSite<'_>) -> Option<LazyType> {
        let string = match site.args.first() {
            Some(arg) if arg.name.is_none() => arg.string.as_deref().unwrap_or_default(),
            _ => "",
        };
        // A query on a table: Drupal 7's functions, the connection's methods.
        let function = site.name.rsplit('\\').next().unwrap_or(site.name);
        let queried = match site.kind {
            CallKind::Function => QUERY_FUNCTIONS.iter().any(|known| known.eq_ignore_ascii_case(function)),
            CallKind::Method => QUERY_METHODS.iter().any(|known| known.eq_ignore_ascii_case(site.name)),
            CallKind::StaticMethod => false,
        };
        if queried && !string.is_empty() {
            let receiver = site.receiver.cloned().unwrap_or_else(LazyType::mixed);
            let next = (site.next)();
            return Some(provided(self.key(), format!("query:{string}"), vec![receiver, next.clone()], next));
        }
        // A method a query answers with itself: the table goes on along the chain.
        if site.kind == CallKind::Method
            && !site.stored
            && FLUENT.iter().any(|known| known.eq_ignore_ascii_case(site.name))
        {
            let receiver = site.receiver?.clone();
            let next = (site.next)();
            return Some(provided(self.key(), "fluent:", vec![receiver, next.clone()], next));
        }
        let payload = match site.kind {
            CallKind::StaticMethod if site.name.eq_ignore_ascii_case("service") && !string.is_empty() => {
                format!("service:{string}")
            }
            CallKind::Method if site.name.eq_ignore_ascii_case("get") && !string.is_empty() => format!("get:{string}"),
            // `Drupal::config('name')`, `$this->config('name')`, the factory's `getEditable('name')`.
            _ if !string.is_empty()
                && (site.name.eq_ignore_ascii_case("config") || site.name.eq_ignore_ascii_case("getEditable")) =>
            {
                format!("config:{string}")
            }
            CallKind::Method if site.name.eq_ignore_ascii_case("getStorage") && !string.is_empty() => {
                format!("storage:{string}")
            }
            // `load()` and `create()` are common names: not for what a summary stores.
            CallKind::Method if !site.stored && loaded(site.name).is_some() => format!("entity:{}", site.name),
            _ => return None,
        };
        let receiver = site.receiver?.clone();
        let next = (site.next)();
        Some(provided(self.key(), payload, vec![receiver, next.clone()], next))
    }

    fn resolve(&self, payload: &str, args: &[Type], resolver: &Resolver<'_>, context: &Context) -> Option<Type> {
        let receiver = args.first()?;
        let (what, value) = payload.split_once(':')?;
        // A query of that table: its columns are those `hook_schema()` gives it. (Drupal 7
        // has no `Drupal` class: before the check below.)
        if what == "query" {
            let connection = *receiver == Type::Mixed || Self::is_a(resolver, receiver, context, &CONNECTIONS);
            let class = args.get(1)?.members().iter().find_map(|member| match member {
                Type::Named { fqn, .. } => Some(fqn.clone()),
                _ => None,
            })?;
            return connection.then(|| Type::generic(&class, vec![Type::LiteralString(value.to_owned())]));
        }
        // `db_select('node')->fields(…)`: Drupal documents the query's class as what its
        // methods return, which says nothing of the table; it is the receiver's.
        if what == "fluent" {
            let table = receiver.members().iter().find_map(|member| match member {
                Type::Named { args, .. } if Self::is_a(resolver, member, context, &QUERIES) => {
                    args.iter().find(|arg| matches!(arg, Type::LiteralString(_))).cloned()
                }
                _ => None,
            })?;
            let returned = args.get(1)?.members().iter().find_map(|member| match member {
                Type::Named { fqn, args } if args.is_empty() && Self::is_a(resolver, member, context, &QUERIES) => {
                    Some(fqn.clone())
                }
                _ => None,
            })?;
            return Some(Type::generic(&returned, vec![table]));
        }
        resolver.class_model(DRUPAL)?;
        match what {
            "service" => Self::is_a(resolver, receiver, context, &[DRUPAL]).then(|| Self::service(resolver, value))?,
            "get" if Self::is_a(resolver, receiver, context, &CONTAINERS) => Self::service(resolver, value),
            // A key of a configuration of a known name: the type its schema gives it.
            "get" if Self::config_name(resolver, receiver, context).is_some() => {
                let config = Self::config_name(resolver, receiver, context)?;
                let symfony = resolver.symfony();
                Some(match symfony.config.config_type(&config, value)? {
                    "string" | "label" | "text" | "path" | "uri" | "email" | "machine_name" | "color_hex" | "uuid"
                    | "langcode" | "date_format" | "plural_label" | "required_label" => Type::string(),
                    "integer" | "timestamp" | "weight" => Type::Int,
                    "boolean" => Type::Bool,
                    "float" => Type::Float,
                    "mapping" | "sequence" => Type::array(),
                    _ => return None,
                })
            }
            // A configuration object of that name: its keys are the file's.
            "get" | "config" => {
                let made = args.get(1)?;
                let class = made.members().iter().find_map(|member| match member {
                    Type::Named { fqn, .. } if Self::is_a(resolver, member, context, &[CONFIG_BASE]) => {
                        Some(fqn.clone())
                    }
                    _ => None,
                })?;
                Some(Type::generic(&class, vec![Type::LiteralString(value.to_owned())]))
            }
            "storage" => {
                if !Self::is_a(resolver, receiver, context, &[ENTITY_TYPE_MANAGER]) {
                    return None;
                }
                let (entity, storage) = resolver.drupal_entity_type(value)?;
                let storage = if storage.is_empty() { ENTITY_STORAGE } else { storage.as_str() };
                let storage = match Self::named(resolver, storage) {
                    Type::Named { fqn, .. } => fqn,
                    _ => return None,
                };
                Some(Type::generic(&storage, vec![Self::named(resolver, &entity)]))
            }
            "entity" => {
                let entity = receiver.members().iter().find_map(|member| match member {
                    Type::Named { args, .. } if Self::is_a(resolver, member, context, &[ENTITY_STORAGE]) => {
                        args.first().cloned()
                    }
                    _ => None,
                })?;
                Some(match loaded(value)? {
                    Loaded::Maybe => entity.nullable(),
                    Loaded::One => entity,
                    Loaded::Many => Type::list_of(entity),
                })
            }
            _ => None,
        }
    }
}
