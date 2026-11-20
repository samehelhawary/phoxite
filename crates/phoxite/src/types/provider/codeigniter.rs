//! CodeIgniter 4: what its helper functions make.
//!
//! - `service('name')` and `single_service('name')` are what the static
//!   method `name()` of `Config\Services` returns (the application's, else
//!   the framework's `CodeIgniter\Config\Services`).
//! - `model('BookModel')` is `App\Models\BookModel`; `model(BookModel::class)`
//!   and `model('App\Models\BookModel')` are the class named.
//! - `config('App')` is `Config\App`, `config(App::class)` the class named.
//! - On a model whose `$returnType` names a class, `find()` and `first()`
//!   are that class or null, and `findAll()` a list of it. (CodeIgniter
//!   declares `array|object|null`.)
//! - `$db->table('books')` is a builder of that table (`BaseBuilder<'books'>`),
//!   which the names of its columns are asked from.
//! - The closure of a route group is given the route collection.
//! - CodeIgniter 3: the properties of a controller and of a model that the
//!   loader makes (`$this->load`, `$this->db`, `$this->session`,
//!   `$this->book_model`).
//!
//! Only where the function is CodeIgniter's (its `CodeIgniter\CodeIgniter`
//! class is known): other frameworks have helpers of the same names. The
//! behaviour follows CodeIgniter's user guide.

use super::{CallKind, CallSite, TypeProvider, provided};
use crate::frameworks::codeigniter::refs::class_of;
use crate::types::{Context, LazyType, Resolver, Type};

const CODEIGNITER: &str = "CodeIgniter\\CodeIgniter";
const SERVICES: [&str; 2] = ["Config\\Services", "CodeIgniter\\Config\\Services"];

const CONNECTIONS: [&str; 2] = ["CodeIgniter\\Database\\ConnectionInterface", "CodeIgniter\\Database\\BaseConnection"];
const ROUTES: [&str; 2] = ["CodeIgniter\\Router\\RouteCollectionInterface", "CodeIgniter\\Router\\RouteCollection"];
/// CodeIgniter 3: the classes the loader gives properties.
const LEGACY_OWNERS: [&str; 2] = ["CI_Controller", "CI_Model"];
const MODELS: [&str; 2] = ["CodeIgniter\\BaseModel", "CodeIgniter\\Model"];

/// Whether the model method `name` finds rows, and whether several.
fn finder(name: &str) -> Option<bool> {
    const ONE: [&str; 2] = ["find", "first"];
    const MANY: [&str; 1] = ["findAll"];
    if ONE.iter().any(|known| known.eq_ignore_ascii_case(name)) {
        Some(false)
    } else if MANY.iter().any(|known| known.eq_ignore_ascii_case(name)) {
        Some(true)
    } else {
        None
    }
}

/// See the module documentation.
pub struct CodeIgniterProvider;

impl TypeProvider for CodeIgniterProvider {
    fn key(&self) -> &'static str {
        "codeigniter"
    }

    fn call(&self, site: &CallSite<'_>) -> Option<LazyType> {
        // What a model's finders return: rows of its `$returnType`.
        if site.kind == CallKind::Method && !site.stored && finder(site.name).is_some() {
            let receiver = site.receiver?.clone();
            let next = (site.next)();
            return Some(provided(self.key(), format!("found:{}", site.name), vec![receiver, next.clone()], next));
        }
        // `$db->table('books')`: a builder of that table, whose columns are the migration's.
        if site.kind == CallKind::Method && site.name.eq_ignore_ascii_case("table") {
            let first = site.args.first().filter(|arg| arg.name.is_none())?;
            let table = first.string.as_deref().filter(|table| !table.is_empty())?;
            let receiver = site.receiver?.clone();
            let next = (site.next)();
            return Some(provided(self.key(), format!("table:{table}"), vec![receiver, next.clone()], next));
        }
        if site.kind != CallKind::Function {
            return None;
        }
        let function = site.name.rsplit('\\').next().unwrap_or(site.name).to_ascii_lowercase();
        if !matches!(function.as_str(), "service" | "single_service" | "model" | "config") {
            return None;
        }
        let first = site.args.first().filter(|arg| arg.name.is_none())?;
        let named = first.class.as_deref().or(first.string.as_deref()).filter(|name| !name.is_empty())?;
        let next = (site.next)();
        Some(provided(self.key(), format!("{function}:{named}"), vec![next.clone()], next))
    }

    /// `$routes->group('admin', static function ($routes) { … })`: the closure is
    /// given the collection its group is of.
    fn closure_parameters(
        &self,
        receiver: &Type,
        method: &str,
        _key: &str,
        resolver: &Resolver<'_>,
        context: &Context,
    ) -> Option<Vec<Type>> {
        if !method.eq_ignore_ascii_case("group") && !method.eq_ignore_ascii_case("environment") {
            return None;
        }
        let models = resolver.class_models(receiver, context);
        let routes = models
            .iter()
            .find(|model| ROUTES.iter().any(|class| model.fqn() == *class || model.is_subclass_of(class)))?;
        Some(vec![Type::named(routes.fqn())])
    }

    /// CodeIgniter 3: what a controller or a model has once the loader made
    /// it, by its name. `$this->load` is the loader, `$this->db` the query
    /// builder, `$this->session` the class `CI_Session`, and
    /// `$this->book_model` the model `Book_model`.
    fn magic_property(&self, class: &str, name: &str, resolver: &Resolver<'_>) -> Option<Type> {
        let owner = resolver.class_model(class)?;
        if !LEGACY_OWNERS.iter().any(|base| owner.fqn() == *base || owner.is_subclass_of(base)) {
            return None;
        }
        let known = |class: &str| resolver.class_model(class).map(|model| Type::named(model.fqn()));
        match name {
            "load" => return known("CI_Loader"),
            "db" => return ["CI_DB_query_builder", "CI_DB_driver", "CI_DB"].iter().find_map(|class| known(class)),
            _ => {}
        }
        if let Some(core) = known(&format!("CI_{name}")) {
            return Some(core);
        }
        // What the project says the loader made under that name: a model under another
        // name, a library of the application.
        if let Some(loaded) = resolver.loaded_class(name).and_then(|class| known(&class)) {
            return Some(loaded);
        }
        // A model is named as it was loaded; another class of that name isn't taken for one.
        let model = resolver.class_model(name)?;
        model.is_subclass_of("CI_Model").then(|| Type::named(model.fqn()))
    }

    fn declared_property(&self, class: &str, name: &str, declared: &Type, resolver: &Resolver<'_>) -> Option<Type> {
        (*declared == Type::Mixed).then(|| self.magic_property(class, name, resolver))?
    }

    fn resolve(&self, payload: &str, args: &[Type], resolver: &Resolver<'_>, context: &Context) -> Option<Type> {
        resolver.class_model(CODEIGNITER)?;
        let (function, name) = payload.split_once(':')?;
        if function == "table" {
            let receiver = args.first()?;
            let models = resolver.class_models(receiver, context);
            if !models
                .iter()
                .any(|model| CONNECTIONS.iter().any(|class| model.fqn() == *class || model.is_subclass_of(class)))
            {
                return None;
            }
            let builder = args.get(1)?.members().iter().find_map(|member| match member {
                Type::Named { fqn, .. } => Some(fqn.clone()),
                _ => None,
            })?;
            let table = name.split_whitespace().next().unwrap_or(name);
            return Some(Type::generic(&builder, vec![Type::LiteralString(table.to_owned())]));
        }
        if function == "found" {
            let receiver = args.first()?;
            let models = resolver.class_models(receiver, context);
            if !models.iter().any(|model| MODELS.iter().any(|class| model.is_subclass_of(class))) {
                return None;
            }
            // `protected $returnType = Book::class;`, or the class's name in a string (which
            // the property's type doesn't keep: its default as written).
            let declared = resolver.property_type(receiver, "returnType", context);
            let constant = declared.members().iter().find_map(|member| match member {
                Type::ClassString(Some(class)) => Some((**class).clone()),
                _ => None,
            });
            let written = || {
                let property = models.iter().find_map(|model| model.properties.get("returnType"))?;
                let default = property.declared()?.default.as_deref()?;
                let class = default.trim().trim_matches(['\'', '"']).replace("\\\\", "\\");
                let model = resolver.class_model(class.trim_start_matches('\\')).filter(|_| class.contains('\\'))?;
                Some(Type::named(model.fqn()))
            };
            let entity = constant.or_else(written)?;
            return Some(if finder(name)? { Type::list_of(entity) } else { entity.nullable() });
        }
        match function {
            "service" | "single_service" => SERVICES.iter().find_map(|class| {
                resolver.class_model(class)?.methods.get(name)?;
                let ty = resolver.method_return(&Type::named(class), name, context);
                (ty != Type::Mixed).then_some(ty)
            }),
            "model" | "config" => {
                let namespace = if function == "model" { "App\\Models" } else { "Config" };
                let model = resolver.class_model(&class_of(namespace, name))?;
                Some(Type::named(model.fqn()))
            }
            _ => None,
        }
    }
}
