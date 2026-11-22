//! Joomla: what the application, a component's MVC factory and a container
//! hand out.
//!
//! - **The application:** `Factory::getApplication()` is the CMS
//!   application (`Joomla\CMS\Application\CMSApplication`), so
//!   `getDocument()`, `getIdentity()`, `getInput()` and the rest are there;
//!   the interface it is documented to return has only part of them.
//! - **A component:** `bootComponent('com_content')` is the component's
//!   extension class (the one of its administrator namespace's `Extension`
//!   folder whose name ends in `Component`).
//! - **The MVC factory:** `createModel('Article', 'Administrator')` and
//!   `createTable('Article')` are `<namespace>\<Client>\Model\ArticleModel`
//!   and `…\Table\ArticleTable`, in the component the calling class is of;
//!   without a client, the calling class's, then the site's, then the
//!   administrator's. Outside a component's classes, the one class of that
//!   name the project has.
//! - **A controller and a view:** `$this->getModel('Article')` the same;
//!   in a view, `$this->getModel()` is the model named after the view, and
//!   `$this->get('Items')` is what that model's `getItems()` returns.
//! - **A container:** `$container->get(Foo::class)` is `Foo`.
//! - **An event handler's parameter** that nothing types (or that is
//!   declared as the base event): the event class of the name its class
//!   subscribes the method to ([`Resolver::joomla_handler_event`]).
//!
//! Only in a Joomla project ([`TypeProvider::applies`]). The behaviour
//! follows Joomla's manual.

use super::{CallKind, CallSite, ParameterSite, TypeProvider, provided};
use crate::frameworks::joomla::JoomlaProject;
use crate::frameworks::joomla::project::ExtensionKind;
use crate::types::{Context, LazyType, Resolver, Type};

const FACTORY: &str = "Joomla\\CMS\\Factory";
const APPLICATION: &str = "Joomla\\CMS\\Application\\CMSApplication";
const EXTENSION_MANAGER: &str = "Joomla\\CMS\\Extension\\ExtensionManagerInterface";
const MVC_FACTORY: &str = "Joomla\\CMS\\MVC\\Factory\\MVCFactoryInterface";
const CONTROLLER: &str = "Joomla\\CMS\\MVC\\Controller\\BaseController";
const VIEW: &str = "Joomla\\CMS\\MVC\\View\\AbstractView";
const CONTAINER: &str = "Psr\\Container\\ContainerInterface";
const SUBSCRIBER: &str = "Joomla\\Event\\SubscriberInterface";
const PLUGIN: &str = "Joomla\\CMS\\Plugin\\CMSPlugin";
/// What a handler is declared to take when it says no more than "an event".
const BASE_EVENTS: [&str; 4] = [
    "Joomla\\Event\\Event",
    "Joomla\\Event\\EventInterface",
    "Joomla\\CMS\\Event\\AbstractEvent",
    "Joomla\\CMS\\Event\\AbstractImmutableEvent",
];
const CLIENTS: [&str; 3] = ["Site", "Administrator", "Api"];

/// See the module documentation.
pub struct JoomlaProvider;

fn class_among(ty: &Type, base: &str, resolver: &Resolver<'_>, context: &Context) -> Option<String> {
    let models = resolver.class_models(ty, context);
    models.iter().find(|model| model.is_subclass_of(base)).map(|model| model.fqn().to_owned())
}

fn known(resolver: &Resolver<'_>, class: &str) -> Option<Type> {
    resolver.class_model(class.trim_start_matches('\\')).map(|model| Type::named(model.fqn()))
}

/// A component's namespace without its client, and the client:
/// `Joomla\Component\Content\Administrator` is `Joomla\Component\Content`
/// and `Administrator`.
fn split_client(namespace: &str) -> (&str, Option<&'static str>) {
    for client in CLIENTS {
        if let Some(base) = namespace.strip_suffix(client).and_then(|base| base.strip_suffix('\\')) {
            return (base, Some(client));
        }
    }
    (namespace, None)
}

impl JoomlaProvider {
    /// The class `<base>\<Client>\<Kind>\<Name><Kind>` the MVC factory
    /// makes: in the component `class` is of, else the only one of that
    /// name.
    fn mvc_class(
        project: &JoomlaProject,
        resolver: &Resolver<'_>,
        class: Option<&str>,
        kind: &str,
        name: &str,
        prefix: &str,
    ) -> Option<Type> {
        let mut chars = name.chars();
        let name = chars.next()?.to_ascii_uppercase().to_string() + chars.as_str();
        let own = class.and_then(|class| project.extension_of_class(class));
        let own = own.and_then(|extension| extension.namespace.as_deref()).map(split_client);
        let wanted = CLIENTS.iter().find(|client| client.eq_ignore_ascii_case(prefix)).copied();
        let made = |base: &str, client: &str| known(resolver, &format!("{base}\\{client}\\{kind}\\{name}{kind}"));
        let in_component = |base: &str, client: Option<&'static str>| -> Option<Type> {
            let order: Vec<&str> = match wanted {
                Some(wanted) => vec![wanted],
                None => client.into_iter().chain(CLIENTS).collect(),
            };
            order.into_iter().find_map(|client| made(base, client))
        };
        if let Some((base, client)) = own {
            return in_component(base, client);
        }
        // Not in a component: the one component that has it.
        let mut bases: Vec<&str> = project
            .extensions
            .iter()
            .filter(|extension| extension.kind == ExtensionKind::Component)
            .filter_map(|extension| extension.namespace.as_deref())
            .map(|namespace| split_client(namespace).0)
            .collect();
        bases.sort_unstable();
        bases.dedup();
        let mut found = bases.into_iter().filter_map(|base| in_component(base, None));
        let first = found.next()?;
        found.next().is_none().then_some(first)
    }
}

impl TypeProvider for JoomlaProvider {
    fn key(&self) -> &'static str {
        "joomla"
    }

    fn applies(&self, resolver: &Resolver<'_>) -> bool {
        resolver.is_joomla()
    }

    fn call(&self, site: &CallSite<'_>) -> Option<LazyType> {
        if site.stored || site.kind == CallKind::Function {
            return None;
        }
        let named = |name: &str| site.name.eq_ignore_ascii_case(name);
        let string = |index: usize| {
            let arg = site.args.get(index).filter(|arg| arg.name.is_none())?;
            arg.string.as_deref().or(arg.class.as_deref()).filter(|name| !name.is_empty())
        };
        let first = string(0);
        let payload = match (site.kind, first) {
            (CallKind::StaticMethod, _) if named("getApplication") => "app:".to_owned(),
            (CallKind::StaticMethod, _) => return None,
            (_, Some(name)) if named("bootComponent") => format!("boot:{name}"),
            (_, Some(name)) if named("createModel") || named("getModel") => {
                format!("mvc:Model:{name}:{}", string(1).unwrap_or_default())
            }
            (_, Some(name)) if named("createTable") => format!("mvc:Table:{name}:{}", string(1).unwrap_or_default()),
            (_, None) if named("getModel") && site.args.is_empty() => "model:".to_owned(),
            (_, Some(name)) if named("get") => format!("get:{name}"),
            _ => return None,
        };
        let receiver = site.receiver.cloned().unwrap_or_else(LazyType::mixed);
        let next = (site.next)();
        Some(provided(self.key(), payload, vec![receiver, next.clone()], next))
    }

    fn resolve(&self, payload: &str, args: &[Type], resolver: &Resolver<'_>, context: &Context) -> Option<Type> {
        let project = resolver.joomla()?;
        let receiver = args.first()?;
        let (what, rest) = payload.split_once(':')?;
        let own = context.self_class.as_deref();
        match what {
            "app" => {
                class_among(receiver, FACTORY, resolver, context)?;
                known(resolver, APPLICATION)
            }
            "boot" => {
                class_among(receiver, EXTENSION_MANAGER, resolver, context)?;
                let folder = project.extension(rest).into_iter().find(|extension| extension.namespace.is_some())?;
                let (base, _) = split_client(folder.namespace.as_deref()?);
                let classes = resolver.namespace_classes(&format!("{base}\\Administrator\\Extension"));
                let found = classes.into_iter().find(|class| class.ends_with("Component"))?;
                known(resolver, &found)
            }
            "mvc" => {
                let mut parts = rest.splitn(3, ':');
                let (kind, name, prefix) = (parts.next()?, parts.next()?, parts.next().unwrap_or_default());
                let factory = class_among(receiver, MVC_FACTORY, resolver, context).is_some();
                let holder = class_among(receiver, CONTROLLER, resolver, context)
                    .or_else(|| class_among(receiver, VIEW, resolver, context));
                if !factory && holder.is_none() {
                    return None;
                }
                // (A model named with its component, `Content.Article`, is the part after it.)
                let name = name.rsplit('.').next().unwrap_or(name);
                Self::mvc_class(&project, resolver, holder.as_deref().or(own), kind, name, prefix)
            }
            // A view's own model: the one named after the view.
            "model" | "get" => {
                let view = class_among(receiver, VIEW, resolver, context);
                if what == "get" && view.is_none() {
                    // A container makes the class it is asked for.
                    class_among(receiver, CONTAINER, resolver, context)?;
                    return rest.contains('\\').then(|| known(resolver, rest)).flatten();
                }
                let view = view?;
                let name = view.rsplit('\\').nth(1)?;
                let model = Self::mvc_class(&project, resolver, Some(&view), "Model", name, "")?;
                if what == "model" {
                    return Some(model);
                }
                let method = format!("get{rest}");
                let Type::Named { fqn, .. } = &model else { return None };
                resolver.class_model(fqn)?.method(&method)?;
                let inner = Context { self_class: Some(fqn.clone()), static_type: Some(model.clone()) };
                let returned = resolver.method_return(&model, &method, &inner);
                (returned != Type::Mixed).then_some(returned)
            }
            _ => None,
        }
    }

    fn parameter(&self, site: &ParameterSite<'_>, resolver: &Resolver<'_>) -> Option<Type> {
        if site.index != 0 || !resolver.is_joomla() {
            return None;
        }
        let vague = match site.declared {
            Type::Mixed => true,
            Type::Named { fqn, .. } => BASE_EVENTS.iter().any(|base| base.eq_ignore_ascii_case(fqn)),
            _ => false,
        };
        if !vague {
            return None;
        }
        let model = resolver.class_model(site.class)?;
        if !model.is_subclass_of(SUBSCRIBER) && !model.is_subclass_of(PLUGIN) {
            return None;
        }
        let event = resolver.joomla_handler_event(site.class, &site.method.name)?;
        known(resolver, &event)
    }
}
