//! The `doctrineField` reference kind: the fields of an entity where a
//! Doctrine repository takes them by name, as Doctrine's documentation on
//! repositories describes them: the keys of the criteria of `findBy()`,
//! `findOneBy()` and `count()`, and of the order of `findBy()` and
//! `findOneBy()`.
//!
//! The entity is the one of the repository the method is called on: the
//! one its mapping or its constructor names
//! ([`SymfonyProject::entity_of`](crate::symfony::SymfonyProject::entity_of)),
//! looked for up the repository's parents. A field is a property of the
//! entity. Goto goes to the property; the usages are the keys naming the
//! same field of the same entity.

use crate::frameworks::refs::{CallTarget, CalleePattern, PlacePattern, Position, RefContext, RefItem, ReferenceKind};
use crate::navigate::Target;
use crate::types::model::PropertySource;
use crate::types::{Context, Type};

const OBJECT_REPOSITORY: &str = "Doctrine\\Persistence\\ObjectRepository";
const ENTITY_REPOSITORY: &str = "Doctrine\\ORM\\EntityRepository";

const CRITERIA: CalleePattern = CalleePattern::Method {
    classes: &[OBJECT_REPOSITORY, ENTITY_REPOSITORY],
    names: &["findBy", "findOneBy", "count"],
};
const ORDER: CalleePattern =
    CalleePattern::Method { classes: &[OBJECT_REPOSITORY, ENTITY_REPOSITORY], names: &["findBy", "findOneBy"] };

static PATTERNS: [PlacePattern; 2] = [
    PlacePattern::argument(CRITERIA, 0, "criteria").at(Position::ArrayKey),
    PlacePattern::argument(ORDER, 1, "orderBy").at(Position::ArrayKey),
];

/// What separates the entity from the field in a canonical value.
const SEPARATOR: &str = "::";

/// See the module documentation.
pub struct DoctrineField;

impl DoctrineField {
    /// The entity of the repository the string's call is on.
    fn entity(cx: &RefContext<'_>) -> Option<String> {
        let (CallTarget::Method { classes, .. }, _) = cx.site.as_ref()?.call.as_ref()? else { return None };
        let resolver = cx.engine.resolver();
        let project = resolver.symfony();
        for class in classes {
            let mut model = resolver.class_model(class);
            for _ in 0..8 {
                let Some(current) = model else { break };
                if let Some(entity) = project.entity_of(current.fqn()) {
                    return Some(
                        resolver.class_model(entity).map_or_else(|| entity.to_owned(), |e| e.fqn().to_owned()),
                    );
                }
                model = current.class.extends.first().and_then(|parent| resolver.class_model(parent));
            }
        }
        None
    }
}

impl ReferenceKind for DoctrineField {
    fn id(&self) -> &'static str {
        "doctrineField"
    }

    fn patterns(&self) -> &'static [PlacePattern] {
        &PATTERNS
    }

    fn complete(&self, cx: &RefContext<'_>, value: &str, _at: usize) -> Vec<RefItem> {
        let Some(entity) = Self::entity(cx) else { return Vec::new() };
        let resolver = cx.engine.resolver();
        let Some(model) = resolver.class_model(&entity) else { return Vec::new() };
        let receiver = Type::named(model.fqn());
        model
            .properties
            .iter()
            .filter(|property| !property.is_static && matches!(property.source, PropertySource::Declared(_)))
            .map(|property| {
                let ty = resolver.property_type(&receiver, &property.name, &Context::default());
                RefItem {
                    label: property.name.clone(),
                    insert: property.name.clone(),
                    detail: Some(ty.to_string()),
                    from: 0,
                    to: value.len(),
                }
            })
            .collect()
    }

    fn resolve(&self, cx: &RefContext<'_>, value: &str, _at: usize) -> Vec<Target> {
        let Some(entity) = Self::entity(cx) else { return Vec::new() };
        let resolver = cx.engine.resolver();
        let Some(model) = resolver.class_model(&entity) else { return Vec::new() };
        let Some(property) = model.property(value) else { return Vec::new() };
        let PropertySource::Declared(index) = property.source else { return Vec::new() };
        let Some(declared) = property.declaring.properties.get(index) else { return Vec::new() };
        // In the file of the class declaring it.
        let files = cx.engine.class_targets(&property.declaring.fqn);
        files.into_iter().take(1).map(|class| Target { path: class.path, range: declared.name_loc.range }).collect()
    }

    /// `App\Entity\Product::name`: the same field name of another entity is another field.
    fn canonical(&self, cx: &RefContext<'_>, value: &str) -> String {
        match Self::entity(cx) {
            Some(entity) => format!("{entity}{SEPARATOR}{value}"),
            None => value.to_owned(),
        }
    }

    fn usage_words(&self, value: &str) -> Vec<String> {
        crate::index::words(self.usage_text(value))
    }

    fn usage_text<'v>(&self, value: &'v str) -> &'v str {
        value.rsplit_once(SEPARATOR).map_or(value, |(_, field)| field)
    }

    fn is_known(&self, cx: &RefContext<'_>, value: &str) -> bool {
        let Some(entity) = Self::entity(cx) else { return true };
        cx.engine.resolver().class_model(&entity).is_none_or(|model| model.property(value).is_some())
    }

    fn hover(&self, cx: &RefContext<'_>, value: &str) -> Option<String> {
        let entity = Self::entity(cx)?;
        let resolver = cx.engine.resolver();
        let model = resolver.class_model(&entity)?;
        model.property(value)?;
        let ty = resolver.property_type(&Type::named(model.fqn()), value, &Context::default());
        Some(format!("**field** `{value}` of `{}`\n\n`{ty}`", model.fqn()))
    }
}
