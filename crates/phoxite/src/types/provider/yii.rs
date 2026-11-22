//! Yii 2: what its service locator, its object factory and Active Record
//! make.
//!
//! - **Components:** `Yii::$app->mailer` and `Yii::$app->get('mailer')` are
//!   the class the configuration gives the component `mailer`
//!   ([`crate::frameworks::yii::YiiProject`]); a component Yii documents
//!   (`$db`, `$user`) is the configured class when the configuration names
//!   another. On a module too.
//! - **The identity:** `Yii::$app->user->identity` is the `identityClass` of
//!   the `user` component, or null.
//! - **Objects:** `Yii::createObject(Foo::class)`, `Yii::createObject('app\Foo')`,
//!   `Yii::createObject(['class' => Foo::class, …])` and
//!   `Yii::$container->get(Foo::class)` are a `Foo`.
//! - **Properties read through a getter:** on a `yii\base\BaseObject`,
//!   `$object->name` is what `getName()` returns.
//! - **Active Record:** `Post::find()` is an `ActiveQuery<Post>` (the query
//!   class `find()` declares, when it declares one), which the methods
//!   returning `$this` keep; `one()` is then a `Post` or null and `all()` a
//!   list of them. `hasOne(Author::class, …)` is an `ActiveQuery<Author>`
//!   and `hasMany(Tag::class, …)` an `ActiveQuery<list<Tag>>`, so the
//!   relation's property (`$post->author`, `$post->tags`, through
//!   `getAuthor()` and `getTags()`) is an `Author` or null, a list of `Tag`.
//!
//! - **A record's columns:** `$post->title` is the type of the column
//!   `title` of the record's table, as the project's migrations make it
//!   ([`crate::frameworks::yii::migrations`]). A property the class declares
//!   or documents says more.
//!
//! Only where `yii\BaseYii` is known. The behaviour follows Yii's guide
//! (Service Locator, Dependency Injection Container, Properties, Active
//! Record).

use super::{AttributeOrigin, CallKind, CallSite, MagicMember, MagicTarget, Source, TypeProvider, provided};
use crate::frameworks::yii::migrations;
use crate::summary::Loc;
use crate::types::{Context, LazyType, Resolver, ShapeKey, Type};

const YII: &str = "yii\\BaseYii";
const LOCATOR: &str = "yii\\di\\ServiceLocator";
const CONTAINER: &str = "yii\\di\\Container";
const OBJECT: &str = "yii\\base\\BaseObject";
const USER: &str = "yii\\web\\User";
const RECORDS: [&str; 2] = ["yii\\db\\BaseActiveRecord", "yii\\db\\ActiveRecordInterface"];
const QUERIES: [&str; 2] = ["yii\\db\\ActiveQueryInterface", "yii\\db\\ActiveQuery"];
const QUERY: &str = "yii\\db\\ActiveQuery";

/// See the module documentation.
pub struct YiiProvider;

fn is_one_of(resolver: &Resolver<'_>, class: &str, bases: &[&str]) -> bool {
    resolver.class_model(class).is_some_and(|model| bases.iter().any(|base| model.is_subclass_of(base)))
}

/// The class a known class name stands for.
fn known(resolver: &Resolver<'_>, class: &str) -> Option<Type> {
    resolver.class_model(class.trim_start_matches('\\')).map(|model| Type::named(model.fqn()))
}

/// The model of a query (`ActiveQuery<Post>`, `ActiveQuery<list<Post>>`),
/// and whether the query is of several (a `hasMany`).
fn queried(ty: &Type, resolver: &Resolver<'_>, context: &Context) -> Option<(Type, bool)> {
    ty.members().iter().find_map(|member| {
        let Type::Named { fqn, args, .. } = member else { return None };
        if !is_one_of(resolver, fqn, &QUERIES) {
            return None;
        }
        let _ = context;
        match args.first()? {
            Type::List { value, .. } => Some(((**value).clone(), true)),
            model => Some((model.clone(), false)),
        }
    })
}

impl YiiProvider {
    /// What `$object->name` is through `getName()`; a relation's query is
    /// the related model, or a list of them.
    fn getter(class: &str, name: &str, resolver: &Resolver<'_>) -> Option<Type> {
        let owner = resolver.class_model(class)?;
        if !owner.is_subclass_of(OBJECT) {
            return None;
        }
        let getter = format!("get{name}");
        owner.method(&getter)?;
        let this = Type::named(owner.fqn());
        let context = Context { self_class: Some(owner.fqn().to_owned()), static_type: Some(this.clone()) };
        let returned = resolver.method_return(&this, &getter, &context);
        if let Some((model, many)) = queried(&returned, resolver, &context) {
            return Some(if many { Type::list_of(model) } else { model.nullable() });
        }
        (returned != Type::Mixed && returned != Type::Void).then_some(returned)
    }

    /// The type of the column `name` of the table of the record `class`.
    fn column(class: &str, name: &str, resolver: &Resolver<'_>) -> Option<Type> {
        let (_, columns) = migrations::columns_of(resolver, class)?;
        columns.iter().find(|column| column.name == name).map(migrations::Column::php_type)
    }

    /// The class the configuration gives the component `name` of `class`
    /// (an application, a module); the identity of the `user` component.
    fn configured(class: &str, name: &str, resolver: &Resolver<'_>) -> Option<Type> {
        let owner = resolver.class_model(class)?;
        if owner.is_subclass_of(LOCATOR) {
            return known(resolver, &resolver.yii()?.component(name)?.detail.clone()?);
        }
        if name == "identity" && owner.is_subclass_of(USER) {
            return known(resolver, &resolver.yii()?.identity.clone()?).map(Type::nullable);
        }
        None
    }
}

impl TypeProvider for YiiProvider {
    fn key(&self) -> &'static str {
        "yii"
    }

    fn call(&self, site: &CallSite<'_>) -> Option<LazyType> {
        let named = |name: &str| site.name.eq_ignore_ascii_case(name);
        let first = site.args.first().filter(|arg| arg.name.is_none());
        match site.kind {
            // `Post::find()`: a query of posts.
            CallKind::StaticMethod if named("find") && site.args.is_empty() => {
                let receiver = site.receiver?.clone();
                let next = (site.next)();
                Some(provided(self.key(), "find", vec![receiver, next.clone()], next))
            }
            // `Yii::createObject(Foo::class)`.
            CallKind::StaticMethod if named("createObject") => {
                let next = (site.next)();
                match first?.class.as_deref().filter(|class| !class.is_empty()) {
                    Some(class) => Some(provided(self.key(), format!("create:{class}"), Vec::new(), next)),
                    // A configuration array: its `class` (or `__class`) element.
                    None => {
                        let configuration = (site.arg_types)().into_iter().next()?;
                        Some(provided(self.key(), "configured", vec![configuration], next))
                    }
                }
            }
            // A relation: `$this->hasOne(Author::class, ['id' => 'author_id'])`.
            CallKind::Method if named("hasOne") || named("hasMany") => {
                let class = first?.class.as_deref().filter(|class| !class.is_empty())?;
                let receiver = site.receiver?.clone();
                let next = (site.next)();
                let many = if named("hasMany") { "many" } else { "one" };
                Some(provided(self.key(), format!("has:{many}:{class}"), vec![receiver, next.clone()], next))
            }
            // What a query finds.
            CallKind::Method if !site.stored && (named("one") || named("all")) => {
                let receiver = site.receiver?.clone();
                let next = (site.next)();
                let payload = if named("one") { "one" } else { "all" };
                Some(provided(self.key(), payload, vec![receiver, next.clone()], next))
            }
            // `Yii::$app->get('mailer')`, `Yii::$container->get(Foo::class)`.
            // (Not kept in a summary: many classes have a `get()`.)
            CallKind::Method if !site.stored && named("get") => {
                let name = first?.string.as_deref().or(first?.class.as_deref()).filter(|name| !name.is_empty())?;
                let receiver = site.receiver?.clone();
                let next = (site.next)();
                Some(provided(self.key(), format!("get:{name}"), vec![receiver, next.clone()], next))
            }
            _ => None,
        }
    }

    fn magic_property(&self, class: &str, name: &str, resolver: &Resolver<'_>) -> Option<Type> {
        resolver.class_model(YII)?;
        Self::configured(class, name, resolver)
            .or_else(|| Self::column(class, name, resolver))
            .or_else(|| Self::getter(class, name, resolver))
    }

    /// A component Yii documents (`@property \yii\web\User $user`) is the
    /// class the configuration names; a relation documented as its query's
    /// result stays as documented.
    fn declared_property(&self, class: &str, name: &str, declared: &Type, resolver: &Resolver<'_>) -> Option<Type> {
        resolver.class_model(YII)?;
        if let Some(configured) = Self::configured(class, name, resolver) {
            return Some(configured);
        }
        (*declared == Type::Mixed).then(|| Self::getter(class, name, resolver))?
    }

    /// The components of an application or a module, and the properties a
    /// class of the project reads through a getter.
    fn magic_members(&self, receiver: &Type, resolver: &Resolver<'_>, context: &Context) -> Vec<MagicMember> {
        if resolver.class_model(YII).is_none() {
            return Vec::new();
        }
        let mut out: Vec<MagicMember> = Vec::new();
        for model in resolver.class_models(receiver, context) {
            if model.is_subclass_of(LOCATOR)
                && let Some(project) = resolver.yii()
            {
                for def in &project.components {
                    let Some(ty) = def.detail.as_deref().and_then(|class| known(resolver, class)) else { continue };
                    if out.iter().any(|member| member.name == def.name) {
                        continue;
                    }
                    let source = Source { file: def.file.clone(), loc: Loc { range: def.range, ..Loc::default() } };
                    out.push(MagicMember {
                        name: def.name.clone(),
                        is_static: false,
                        target: MagicTarget::Attribute {
                            class: model.fqn().to_owned(),
                            ty,
                            origin: AttributeOrigin::Configured {
                                what: "Yii component".to_owned(),
                                source: Some(source),
                            },
                        },
                    });
                }
            }
            if !model.is_subclass_of(OBJECT) {
                continue;
            }
            // A record's columns, where the class says nothing about the attribute.
            if let Some((table, columns)) = migrations::columns_of(resolver, model.fqn()) {
                for column in columns {
                    if model.properties.get(&column.name).is_some()
                        || out.iter().any(|member| member.name == column.name)
                    {
                        continue;
                    }
                    let source =
                        Source { file: column.file.clone(), loc: Loc { range: column.range, ..Loc::default() } };
                    out.push(MagicMember {
                        name: column.name.clone(),
                        is_static: false,
                        target: MagicTarget::Attribute {
                            class: model.fqn().to_owned(),
                            ty: column.php_type(),
                            origin: AttributeOrigin::Configured {
                                what: format!("column of `{table}`"),
                                source: Some(source),
                            },
                        },
                    });
                }
            }
            for method in model.methods.iter() {
                // The getters of the project's classes: the framework documents its own.
                let own = !method.declaring.fqn.trim_start_matches('\\').to_ascii_lowercase().starts_with("yii\\");
                let Some(name) = method.name.strip_prefix("get").filter(|_| own && !method.is_static) else {
                    continue;
                };
                let mut chars = name.chars();
                let Some(first) = chars.next().filter(char::is_ascii_uppercase) else { continue };
                let name = first.to_ascii_lowercase().to_string() + chars.as_str();
                if model.properties.get(&name).is_some() || out.iter().any(|member| member.name == name) {
                    continue;
                }
                let Some(ty) = Self::getter(model.fqn(), &name, resolver) else { continue };
                out.push(MagicMember {
                    name,
                    is_static: false,
                    target: MagicTarget::Attribute {
                        class: model.fqn().to_owned(),
                        ty,
                        origin: AttributeOrigin::Getter { method: method.name.clone() },
                    },
                });
            }
        }
        out
    }

    fn resolve(&self, payload: &str, args: &[Type], resolver: &Resolver<'_>, context: &Context) -> Option<Type> {
        resolver.class_model(YII)?;
        let receiver_is = |bases: &[&str]| {
            let models = resolver.class_models(args.first()?, context);
            models
                .iter()
                .find(|model| bases.iter().any(|base| model.is_subclass_of(base)))
                .map(|model| model.fqn().to_owned())
        };
        // The query class a call declares, when it declares one.
        let declared_query = || {
            args.get(1)?.members().iter().find_map(|member| match member {
                Type::Named { fqn, .. } if is_one_of(resolver, fqn, &QUERIES) => Some(fqn.clone()),
                _ => None,
            })
        };
        let query = || declared_query().unwrap_or_else(|| QUERY.to_owned());
        match payload {
            "find" => {
                let model = receiver_is(&RECORDS)?;
                Some(Type::generic(&query(), vec![Type::named(&model)]))
            }
            // `['class' => Foo::class, 'size' => 3]`: what the array's `class` names.
            "configured" => {
                let named = |ty: &Type| match ty {
                    Type::LiteralString(class) => known(resolver, class),
                    Type::ClassString(Some(class)) => Some((**class).clone()),
                    _ => None,
                };
                args.first()?.members().iter().find_map(|member| {
                    let Type::Shape(shape) = member else { return None };
                    let is_class =
                        |key: &ShapeKey| matches!(key, ShapeKey::String(key) if key == "class" || key == "__class");
                    let field = shape.fields.iter().find(|field| is_class(&field.key))?;
                    field.ty.members().iter().find_map(named)
                })
            }
            "one" | "all" => {
                let (model, _) = queried(args.first()?, resolver, context)?;
                Some(if payload == "one" { model.nullable() } else { Type::list_of(model) })
            }
            _ => {
                let (what, name) = payload.split_once(':')?;
                match what {
                    "create" => known(resolver, name),
                    "has" => {
                        let (many, class) = name.split_once(':')?;
                        receiver_is(&RECORDS)?;
                        let model = known(resolver, class)?;
                        let model = if many == "many" { Type::list_of(model) } else { model };
                        Some(Type::generic(&query(), vec![model]))
                    }
                    "get" => {
                        if receiver_is(&[CONTAINER]).is_some() {
                            return known(resolver, name);
                        }
                        receiver_is(&[LOCATOR])?;
                        known(resolver, &resolver.yii()?.component(name)?.detail.clone()?)
                    }
                    _ => None,
                }
            }
        }
    }
}
