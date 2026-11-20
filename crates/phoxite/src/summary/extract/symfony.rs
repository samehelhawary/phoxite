//! Extraction of [`SymfonyFacts`](crate::summary::SymfonyFacts):
//!
//! - **Routes:** `#[Route]` attributes (`Symfony\Component\Routing\Attribute
//!   \Route`, or the older `…\Annotation\Route`) on methods with a `name`
//!   (named, or the second positional argument), prefixed with the name and
//!   path of the class's own `#[Route]`, as Symfony's attribute loader does.
//! - **Repositories:** an entity's `repositoryClass`, and in a class
//!   extending a `…Repository` class, `parent::__construct($registry,
//!   Product::class)` (Doctrine's `ServiceEntityRepository`).
//! - **Twig names:** `new TwigFilter('name', …)`, `new TwigFunction(…)` and
//!   `new TwigTest(…)` (what an extension's `getFilters()` returns), and
//!   methods with `#[AsTwigFilter('name')]`, `#[AsTwigFunction]` or
//!   `#[AsTwigTest]`, as Twig's documentation on extending it has them.
//! - **Associations:** properties with one of Doctrine's association
//!   attributes and a `targetEntity` (named, or the first argument).

use mago_span::Span;
use mago_syntax::cst::{
    ArgumentList, AttributeList, ClassLikeMember, ClassLikeMemberSelector, Expression, Instantiation, Literal,
    PartialArgument, Property, PropertyItem, Sequence,
};

use super::Extractor;
use crate::summary::{Association, Loc, RepositoryEntity, RouteAttribute, TwigName, TwigNameKind};
use crate::types::Type;
use crate::types::ops::literal_type;

/// The classes whose instances add a name to Twig, and the attributes that do.
const TWIG_CLASSES: [(&str, TwigNameKind); 3] = [
    ("Twig\\TwigFilter", TwigNameKind::Filter),
    ("Twig\\TwigFunction", TwigNameKind::Function),
    ("Twig\\TwigTest", TwigNameKind::Test),
];
const TWIG_ATTRIBUTES: [(&str, TwigNameKind); 3] = [
    ("Twig\\Attribute\\AsTwigFilter", TwigNameKind::Filter),
    ("Twig\\Attribute\\AsTwigFunction", TwigNameKind::Function),
    ("Twig\\Attribute\\AsTwigTest", TwigNameKind::Test),
];

/// Doctrine's association attributes, and whether they are to many.
const ASSOCIATIONS: [(&str, bool); 4] = [
    ("Doctrine\\ORM\\Mapping\\OneToMany", true),
    ("Doctrine\\ORM\\Mapping\\ManyToMany", true),
    ("Doctrine\\ORM\\Mapping\\ManyToOne", false),
    ("Doctrine\\ORM\\Mapping\\OneToOne", false),
];

const ROUTE_ATTRIBUTES: [&str; 2] =
    ["Symfony\\Component\\Routing\\Attribute\\Route", "Symfony\\Component\\Routing\\Annotation\\Route"];

/// The name and path a `#[Route]` gives, with where the name is written.
#[derive(Default)]
struct Route {
    name: Option<(String, Loc)>,
    path: Option<String>,
}

impl Extractor<'_> {
    /// A string literal's value and the location of its text inside the
    /// quotes.
    fn string_at(&self, expr: &Expression<'_>) -> Option<(String, Loc)> {
        let Expression::Literal(literal) = expr else { return None };
        let Type::LiteralString(value) = literal_type(literal) else { return None };
        let Literal::String(string) = literal else { return None };
        let mut span: Span = string.span;
        if string.raw.len() >= 2 {
            span.start.offset += 1;
            span.end.offset -= 1;
        }
        Some((value, self.loc(span)))
    }

    /// The first `#[Route]` of `lists`.
    fn route(&self, lists: &Sequence<'_, AttributeList<'_>>) -> Option<Route> {
        let attribute = lists.iter().flat_map(|list| list.attributes.iter()).find(|attribute| {
            let name = self.class_name(&attribute.name);
            ROUTE_ATTRIBUTES.iter().any(|route| route.eq_ignore_ascii_case(&name))
        })?;
        let mut route = Route::default();
        let Some(list) = &attribute.argument_list else { return Some(route) };
        for (index, argument) in list.arguments.iter().enumerate() {
            let (name, value) = match argument {
                PartialArgument::Named(argument) => (Some(argument.name.value), argument.value),
                PartialArgument::Positional(argument) => (None, argument.value),
                _ => continue,
            };
            match (name, index) {
                (Some(b"name"), _) | (None, 1) => route.name = self.string_at(value),
                (Some(b"path"), _) | (None, 0) => route.path = self.string_at(value).map(|(path, _)| path),
                _ => {}
            }
        }
        Some(route)
    }

    /// The named `#[Route]`s of a class's methods.
    pub(super) fn routes(
        &mut self,
        attributes: &Sequence<'_, AttributeList<'_>>,
        members: &Sequence<'_, ClassLikeMember<'_>>,
    ) {
        let mut prefix: Option<Route> = None;
        for member in members {
            let ClassLikeMember::Method(method) = member else { continue };
            let Some(route) = self.route(&method.attribute_lists) else { continue };
            let Some((name, loc)) = route.name else { continue };
            let prefix = prefix.get_or_insert_with(|| self.route(attributes).unwrap_or_default());
            let name_prefix = prefix.name.as_ref().map(|(name, _)| name.as_str()).unwrap_or_default();
            let path = match (&prefix.path, route.path) {
                (Some(base), Some(path)) => Some(format!("{}{path}", base.trim_end_matches('/'))),
                (None, path) => path,
                (Some(base), None) => Some(base.clone()),
            };
            self.summary.symfony.routes.push(RouteAttribute { name: format!("{name_prefix}{name}"), path, loc });
        }
    }

    /// `new TwigFilter('name', …)` and the like.
    pub(super) fn twig_instantiation(&mut self, node: &Instantiation<'_>) {
        let Expression::Identifier(class) = node.class else { return };
        let class = self.class_name(class);
        let Some((_, kind)) = TWIG_CLASSES.iter().find(|(known, _)| known.eq_ignore_ascii_case(&class)) else {
            return;
        };
        let first = node.argument_list.as_ref().and_then(|list| list.arguments.first());
        if let Some((name, loc)) = first.and_then(|argument| self.string_at(argument.value())) {
            self.summary.symfony.twig.push(TwigName { kind: *kind, name, loc });
        }
    }

    /// The methods of a class with `#[AsTwigFilter('name')]` and the like.
    pub(super) fn twig_attributes(&mut self, members: &Sequence<'_, ClassLikeMember<'_>>) {
        for member in members {
            let ClassLikeMember::Method(method) = member else { continue };
            for attribute in method.attribute_lists.iter().flat_map(|list| list.attributes.iter()) {
                let class = self.class_name(&attribute.name);
                let Some((_, kind)) = TWIG_ATTRIBUTES.iter().find(|(known, _)| known.eq_ignore_ascii_case(&class))
                else {
                    continue;
                };
                let Some(list) = &attribute.argument_list else { continue };
                let name = list.arguments.iter().enumerate().find_map(|(index, argument)| match argument {
                    PartialArgument::Named(argument) if argument.name.value == b"name" => Some(argument.value),
                    PartialArgument::Positional(argument) if index == 0 => Some(argument.value),
                    _ => None,
                });
                if let Some((name, loc)) = name.and_then(|value| self.string_at(value)) {
                    self.summary.symfony.twig.push(TwigName { kind: *kind, name, loc });
                }
            }
        }
    }

    /// The associations of the entity `entity`: its properties with one of
    /// Doctrine's association attributes naming a target.
    pub(super) fn associations(&mut self, entity: &str, members: &Sequence<'_, ClassLikeMember<'_>>) {
        for member in members {
            let ClassLikeMember::Property(Property::Plain(property)) = member else { continue };
            let found = property.attribute_lists.iter().flat_map(|list| list.attributes.iter()).find_map(|attribute| {
                let class = self.class_name(&attribute.name);
                let (_, many) = ASSOCIATIONS.iter().find(|(known, _)| known.eq_ignore_ascii_case(&class))?;
                let arguments = &attribute.argument_list.as_ref()?.arguments;
                let target = arguments
                    .iter()
                    .find_map(|argument| match argument {
                        PartialArgument::Named(argument) if argument.name.value == b"targetEntity" => {
                            Some(argument.value)
                        }
                        _ => None,
                    })
                    .or_else(|| match arguments.first() {
                        Some(PartialArgument::Positional(argument)) => Some(argument.value),
                        _ => None,
                    })?;
                Some((self.class_constant_name(target)?, *many))
            });
            let Some((target, many)) = found else { continue };
            for item in &property.items {
                let variable = match item {
                    PropertyItem::Concrete(item) => &item.variable,
                    PropertyItem::Abstract(item) => &item.variable,
                };
                let name = String::from_utf8_lossy(variable.name).trim_start_matches('$').to_owned();
                self.summary.symfony.associations.push(Association {
                    entity: entity.to_owned(),
                    property: name,
                    target: target.clone(),
                    many,
                });
            }
        }
    }

    /// An entity's `repositoryClass`.
    pub(super) fn entity_repository(&mut self, entity: &str, repository: Option<&str>) {
        if let Some(repository) = repository {
            self.summary.symfony.repositories.push(RepositoryEntity {
                repository: repository.trim_start_matches('\\').to_owned(),
                entity: entity.to_owned(),
            });
        }
    }

    /// `parent::__construct($registry, Product::class)` in a repository.
    pub(super) fn repository_constructor(
        &mut self,
        class: &Expression<'_>,
        selector: &ClassLikeMemberSelector<'_>,
        list: &ArgumentList<'_>,
    ) {
        let Some(repository) = self.repository_scope.clone() else { return };
        if !matches!(class, Expression::Parent(_)) {
            return;
        }
        let ClassLikeMemberSelector::Identifier(method) = selector else { return };
        if !method.value.eq_ignore_ascii_case(b"__construct") {
            return;
        }
        let Some(entity) = list.arguments.get(1).and_then(|argument| self.class_constant_name(argument.value())) else {
            return;
        };
        let known = self.summary.symfony.repositories.iter().any(|known| known.repository == repository);
        if !known {
            self.summary.symfony.repositories.push(RepositoryEntity { repository, entity });
        }
    }
}

/// Whether a class extending `parent` (FQN) can be a Doctrine repository.
pub(super) fn extends_repository(parent: &str) -> bool {
    parent.rsplit('\\').next().is_some_and(|short| short.ends_with("Repository"))
}
