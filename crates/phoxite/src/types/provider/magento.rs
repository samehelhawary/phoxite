//! Magento 2: what the object manager makes, and a data object's magic
//! accessors.
//!
//! - **The object manager:** `$objectManager->get(Foo::class)` and
//!   `create(Foo::class)` are `Foo`; a virtual type's name
//!   (`get('ShopLogger')`) is the class its `di.xml` gives it. An interface
//!   stays the interface: the preference is what runs, not what the code may
//!   rely on.
//! - **Data objects:** on `Magento\Framework\DataObject` and its subclasses,
//!   for a method the class neither declares nor documents: `setFooBar()`
//!   and `unsFooBar()` return the object, `hasFooBar()` a `bool`;
//!   `getFooBar()` stays `mixed`.
//! - **View models:** `$block->getViewModel()` and
//!   `$block->getData('view_model')` are the class of the `view_model`
//!   argument the layout gives blocks of that class, where they agree on a
//!   few (a class every template shares, like `Template`, has too many to
//!   say).
//!
//! Only in a Magento project ([`TypeProvider::applies`]). The behaviour
//! follows Magento's developer documentation.

use super::{CallKind, CallSite, TypeProvider, provided};
use crate::types::{Context, LazyType, Resolver, Type};

const OBJECT_MANAGER: &str = "Magento\\Framework\\ObjectManagerInterface";
const DATA_OBJECT: &str = "Magento\\Framework\\DataObject";
/// How many view models the blocks of one class may have for the class to
/// say which.
const VIEW_MODELS: usize = 3;

/// See the module documentation.
pub struct MagentoProvider;

/// The class among the members of `ty` that is a `base`.
fn class_among(ty: &Type, base: &str, resolver: &Resolver<'_>, context: &Context) -> Option<String> {
    let models = resolver.class_models(ty, context);
    models.iter().find(|model| model.is_subclass_of(base)).map(|model| model.fqn().to_owned())
}

/// `setFooBar`: the accessor's prefix, when the rest is a name.
fn accessor(name: &str) -> Option<&'static str> {
    ["get", "set", "has", "uns"].into_iter().find(|prefix| {
        name.len() > 3
            && name[..3].eq_ignore_ascii_case(prefix)
            && name[3..].chars().next().is_some_and(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
    })
}

impl TypeProvider for MagentoProvider {
    fn key(&self) -> &'static str {
        "magento"
    }

    fn applies(&self, resolver: &Resolver<'_>) -> bool {
        resolver.is_magento()
    }

    fn call(&self, site: &CallSite<'_>) -> Option<LazyType> {
        // Asked of every method: only where an expression is typed, in a Magento project.
        if site.stored || site.kind != CallKind::Method {
            return None;
        }
        let named = |name: &str| site.name.eq_ignore_ascii_case(name);
        let first = site.args.first().filter(|arg| arg.name.is_none());
        let string =
            first.and_then(|arg| arg.string.as_deref().or(arg.class.as_deref())).filter(|name| !name.is_empty());
        let payload = match string {
            Some(name) if named("get") || named("create") => format!("make:{name}"),
            Some("view_model") if named("getData") => "view-model:".to_owned(),
            None if named("getViewModel") => "view-model:".to_owned(),
            _ => format!("{}:{}", accessor(site.name)?, site.name),
        };
        let receiver = site.receiver?.clone();
        let next = (site.next)();
        Some(provided(self.key(), payload, vec![receiver, next.clone()], next))
    }

    fn resolve(&self, payload: &str, args: &[Type], resolver: &Resolver<'_>, context: &Context) -> Option<Type> {
        let project = resolver.magento()?;
        let receiver = args.first()?;
        let (what, name) = payload.split_once(':')?;
        match what {
            "make" => {
                class_among(receiver, OBJECT_MANAGER, resolver, context)?;
                let name = name.trim_start_matches('\\');
                if let Some(model) = resolver.class_model(name) {
                    return Some(Type::named(model.fqn()));
                }
                let class = project.virtual_class(name)?;
                resolver.class_model(&class).map(|model| Type::named(model.fqn()))
            }
            "view-model" => {
                let class = class_among(receiver, DATA_OBJECT, resolver, context)?;
                let mut found: Vec<&str> = Vec::new();
                for block in &project.blocks {
                    let Some(view_model) = block.view_model.as_deref() else { continue };
                    // A reference gives the block of that name its view model.
                    let of = block.class.as_deref().or_else(|| {
                        let declared =
                            project.blocks.iter().find(|other| other.name == block.name && other.class.is_some());
                        declared.and_then(|other| other.class.as_deref())
                    });
                    if of.is_some_and(|of| of.eq_ignore_ascii_case(&class)) && !found.contains(&view_model) {
                        found.push(view_model);
                    }
                }
                if found.is_empty() || found.len() > VIEW_MODELS {
                    return None;
                }
                let known: Vec<Type> = found
                    .into_iter()
                    .filter_map(|class| resolver.class_model(class).map(|model| Type::named(model.fqn())))
                    .collect();
                (!known.is_empty()).then(|| Type::union(known))
            }
            "set" | "uns" | "has" => {
                let class = class_among(receiver, DATA_OBJECT, resolver, context)?;
                // A method the class has, or documents, is that method.
                if resolver.class_model(&class)?.method(name).is_some() {
                    return None;
                }
                Some(if what == "has" { Type::Bool } else { Type::named(&class) })
            }
            _ => None,
        }
    }
}
