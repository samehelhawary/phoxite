//! Laminas MVC and Mezzio: what a service manager makes of a name.
//!
//! - **Services:** `$container->get('mailer')` on a PSR container or a
//!   Laminas service locator is what the configuration says the service is
//!   ([`crate::frameworks::laminas::LaminasProject`]): an alias is followed,
//!   an invokable is its class, a factory is what its `__invoke()` declares
//!   it returns (Laminas's own `InvokableFactory` and
//!   `ReflectionBasedAbstractFactory` make the class the service is named
//!   after). A name that is a class is that class. `get('config')` is an
//!   array.
//! - **Controller plugins:** `$this->greet()` in a controller is what the
//!   plugin `greet` of the configuration returns when called (its
//!   `__invoke()`), or the plugin itself; `$this->plugin('greet')` is the
//!   plugin.
//! - **View helpers:** the same in a template and on a renderer:
//!   `$this->price(…)` and `$this->plugin('price')`.
//!
//! - **Other plugin managers:** `get('name')` on the form element, validator,
//!   filter, hydrator and input filter managers is that manager's service.
//! - **Delegators and closures:** a service a delegator factory wraps is what
//!   that factory's `__invoke()` declares; a factory written as a closure
//!   makes what the closure is declared to return, or the `new` it returns.
//!
//! Zend Framework's names (`Zend\…`, `Zend\Expressive\…`) are Laminas's and
//! Mezzio's.
//!
//! Only in a Laminas or a Mezzio project ([`TypeProvider::applies`]). The
//! behaviour follows Laminas's documentation (laminas-servicemanager,
//! laminas-mvc, laminas-view).

use super::{CallKind, CallSite, MagicMember, MagicTarget, TypeProvider, provided};
use crate::frameworks::laminas::LaminasProject;
use crate::frameworks::laminas::project::{Manager, ServiceKind};
use crate::types::{Context, LazyType, Resolver, Type};

const CONTAINERS: [&str; 4] = [
    "Psr\\Container\\ContainerInterface",
    "Laminas\\ServiceManager\\ServiceLocatorInterface",
    "Zend\\ServiceManager\\ServiceLocatorInterface",
    "Interop\\Container\\ContainerInterface",
];
const CONTROLLERS: [&str; 2] =
    ["Laminas\\Mvc\\Controller\\AbstractController", "Zend\\Mvc\\Controller\\AbstractController"];
const RENDERERS: [&str; 4] = [
    "Laminas\\View\\Renderer\\PhpRenderer",
    "Zend\\View\\Renderer\\PhpRenderer",
    "Laminas\\View\\Renderer\\Template",
    "Zend\\View\\Renderer\\Template",
];
/// Factories that make the class a service is named after.
const NAMING_FACTORIES: [&str; 6] = [
    "Laminas\\ServiceManager\\Factory\\InvokableFactory",
    "Zend\\ServiceManager\\Factory\\InvokableFactory",
    "Laminas\\ServiceManager\\AbstractFactory\\ReflectionBasedAbstractFactory",
    "Zend\\ServiceManager\\AbstractFactory\\ReflectionBasedAbstractFactory",
    "Laminas\\ServiceManager\\AbstractFactory\\ConfigAbstractFactory",
    "Zend\\ServiceManager\\AbstractFactory\\ConfigAbstractFactory",
];

/// See the module documentation.
pub struct LaminasProvider;

/// The class among the members of `ty` that is one of `bases`.
fn class_among(ty: &Type, bases: &[&str], resolver: &Resolver<'_>, context: &Context) -> Option<String> {
    let models = resolver.class_models(ty, context);
    models.iter().find(|model| bases.iter().any(|base| model.is_subclass_of(base))).map(|model| model.fqn().to_owned())
}

fn known(resolver: &Resolver<'_>, class: &str) -> Option<Type> {
    resolver.class_model(class.trim_start_matches('\\')).map(|model| Type::named(model.fqn()))
}

/// What a class gives when called: what its `__invoke()` returns, when it
/// says.
fn invoked(resolver: &Resolver<'_>, class: &str) -> Option<Type> {
    let model = resolver.class_model(class)?;
    model.method("__invoke")?;
    let this = Type::named(model.fqn());
    let context = Context { self_class: Some(model.fqn().to_owned()), static_type: Some(this.clone()) };
    let returned = resolver.method_return(&this, "__invoke", &context);
    (returned != Type::Mixed && returned != Type::Void && returned != Type::Object).then_some(returned)
}

impl LaminasProvider {
    /// The type of the service `name` of a plugin manager.
    fn service(project: &LaminasProject, manager: Manager, name: &str, resolver: &Resolver<'_>) -> Option<Type> {
        let Some(service) = project.resolved(manager, name) else {
            // Not declared: a class's name is the class (a container makes it, or an
            // abstract factory does).
            return known(resolver, name).filter(|_| name.contains('\\') || manager != Manager::Services);
        };
        let named = || known(resolver, &service.name);
        // What wraps the service last is what is got: a delegator that declares a class.
        let delegated = project.delegator(manager, name).or_else(|| project.delegator(manager, &service.name));
        if let Some(wrapped) =
            delegated.and_then(|found| found.target.as_deref()).and_then(|factory| invoked(resolver, factory))
            && matches!(wrapped, Type::Named { .. })
        {
            return Some(wrapped);
        }
        // A factory written in place: the class it makes.
        if let Some(made) = service.made.as_deref().and_then(|class| known(resolver, class)) {
            return Some(made);
        }
        match (service.kind, service.target.as_deref()) {
            (ServiceKind::Invokable | ServiceKind::Alias, Some(target)) => known(resolver, target).or_else(named),
            (ServiceKind::Factory, Some(factory)) => {
                if NAMING_FACTORIES.iter().any(|naming| naming.eq_ignore_ascii_case(factory)) {
                    return named();
                }
                invoked(resolver, factory).or_else(named)
            }
            _ => named(),
        }
    }

    /// The plugin manager the methods of `receiver` are plugins of: a
    /// controller's plugins, a renderer's helpers.
    fn manager(receiver: &Type, resolver: &Resolver<'_>, context: &Context) -> Option<(Manager, String)> {
        if let Some(class) = class_among(receiver, &CONTROLLERS, resolver, context) {
            return Some((Manager::Plugins, class));
        }
        class_among(receiver, &RENDERERS, resolver, context).map(|class| (Manager::Helpers, class))
    }
}

impl TypeProvider for LaminasProvider {
    fn key(&self) -> &'static str {
        "laminas"
    }

    fn applies(&self, resolver: &Resolver<'_>) -> bool {
        resolver.is_laminas()
    }

    fn call(&self, site: &CallSite<'_>) -> Option<LazyType> {
        // Asked of every method: only where an expression is typed, in a Laminas project.
        if site.stored || site.kind != CallKind::Method {
            return None;
        }
        let named = |name: &str| site.name.eq_ignore_ascii_case(name);
        let first = site.args.first().filter(|arg| arg.name.is_none());
        let string =
            first.and_then(|arg| arg.string.as_deref().or(arg.class.as_deref())).filter(|name| !name.is_empty());
        let payload = match string {
            Some(name) if named("get") || named("build") => format!("get:{name}"),
            Some(name) if named("plugin") => format!("plugin:{name}"),
            _ => format!("call:{}", site.name),
        };
        let receiver = site.receiver?.clone();
        let next = (site.next)();
        Some(provided(self.key(), payload, vec![receiver, next.clone()], next))
    }

    /// A controller's plugins and a renderer's helpers, as the methods they
    /// are called by.
    fn magic_members(&self, receiver: &Type, resolver: &Resolver<'_>, context: &Context) -> Vec<MagicMember> {
        let Some(project) = resolver.laminas() else { return Vec::new() };
        let Some((manager, class)) = Self::manager(receiver, resolver, context) else { return Vec::new() };
        let Some(model) = resolver.class_model(&class) else { return Vec::new() };
        let mut out: Vec<MagicMember> = Vec::new();
        for service in project.callable_names(manager) {
            let name = &service.name;
            if model.method(name).is_some() {
                continue;
            }
            let Some(Type::Named { fqn, .. }) = Self::service(&project, manager, name, resolver) else { continue };
            out.push(MagicMember {
                name: name.clone(),
                is_static: false,
                target: MagicTarget::Method { class: fqn, method: "__invoke".to_owned() },
            });
        }
        out
    }

    fn resolve(&self, payload: &str, args: &[Type], resolver: &Resolver<'_>, context: &Context) -> Option<Type> {
        let project = resolver.laminas()?;
        let receiver = args.first()?;
        let (what, name) = payload.split_once(':')?;
        match what {
            "get" => {
                // A plugin manager's own services: a form element, a validator, a filter.
                let plugins = Manager::ALL.iter().find(|(manager, _, _, classes)| {
                    !matches!(manager, Manager::Services) && class_among(receiver, classes, resolver, context).is_some()
                });
                if let Some((manager, _, _, _)) = plugins {
                    return Self::service(&project, *manager, name, resolver);
                }
                class_among(receiver, &CONTAINERS, resolver, context)?;
                let declared = project.service(Manager::Services, name).is_some();
                if !declared && name.eq_ignore_ascii_case("config") {
                    return Some(Type::array());
                }
                Self::service(&project, Manager::Services, name, resolver)
            }
            "plugin" => {
                let (manager, _) = Self::manager(receiver, resolver, context)?;
                // The plugin itself, whatever calling it gives.
                let service = project.resolved(manager, name)?;
                service
                    .target
                    .as_deref()
                    .and_then(|class| known(resolver, class))
                    .or_else(|| known(resolver, &service.name))
            }
            "call" => {
                let (manager, class) = Self::manager(receiver, resolver, context)?;
                // A method the class has, or documents, is that method.
                if resolver.class_model(&class)?.method(name).is_some() {
                    return None;
                }
                let Type::Named { fqn, .. } = Self::service(&project, manager, name, resolver)? else { return None };
                Some(invoked(resolver, &fqn).unwrap_or_else(|| Type::named(&fqn)))
            }
            _ => None,
        }
    }
}
