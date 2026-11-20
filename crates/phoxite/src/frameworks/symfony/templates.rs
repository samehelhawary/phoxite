//! What PHP knows about a Twig template: the variables the code rendering
//! it passes ([`PhpEngine::twig_template_variables`]), and what can follow
//! a `.` after one of them ([`PhpEngine::twig_members`]).
//!
//! The variables are the keys of the array given with the template's name
//! (`$this->render('blog/show.html.twig', ['post' => $post])`), with the
//! types of their values, from every place the project renders the
//! template.
//!
//! After a `.` Twig looks, on an object, for a public property, then a
//! method of that name, then `getName()`, `isName()` and `hasName()` (Twig's
//! documentation on variables); on an array, for the key. So the members of
//! `post.` are the public properties of its class and its public methods,
//! the getters under the name Twig reads them by (`getTitle()` as `title`).
//! A loop's variable (`{% for comment in post.comments %}`) is an element of
//! what the loop goes through.

use crate::engine::PhpEngine;
use crate::frameworks::refs::{ReferenceKind, sites};
use crate::names::FileNames;
use crate::summary::{FileSummary, Visibility};
use crate::types::{Context, ExprTypes, Resolver, Type};

use super::refs::SymfonyTemplate;

/// How deep a loop over a loop's variable is followed.
const MAX_LOOPS: usize = 8;

/// A variable of a template that a `{% for %}` around the caret defines: its
/// name, and the names of what it goes through (`post.comments` as `post`,
/// `comments`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoopVariable {
    pub name: String,
    pub source: Vec<String>,
}

/// A member Twig reads after a `.`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TwigMember {
    /// The name as the template writes it (`title` for `getTitle()`).
    pub name: String,
    /// Its type.
    pub detail: String,
    /// A method that isn't read as a property (it takes arguments, or is no getter).
    pub is_method: bool,
}

/// What a method is called in a template when Twig reads it as a property:
/// `getTitle` as `title`, `isActive` as `active`, `hasItems` as `items`.
fn read_as(method: &str) -> Option<String> {
    let rest = ["get", "is", "has"].iter().find_map(|prefix| method.strip_prefix(prefix))?;
    let mut chars = rest.chars();
    let first = chars.next().filter(|first| first.is_ascii_uppercase())?;
    Some(format!("{}{}", first.to_ascii_lowercase(), chars.as_str()))
}

/// The type of the elements a `{% for %}` over a value of type `ty` yields.
fn element_of(ty: &Type) -> Type {
    Type::union(ty.members().iter().map(|member| match member {
        Type::List { value, .. } | Type::Array { value, .. } => (**value).clone(),
        // An array written in place (`[$post]`): what its values are.
        Type::Shape(shape) => shape.value_type(),
        // A generic collection: its last argument (`Collection<int, Comment>`).
        Type::Named { args, .. } => args.last().cloned().unwrap_or(Type::Mixed),
        _ => Type::Mixed,
    }))
}

impl PhpEngine {
    /// The variables the project's PHP passes to the template named `name`:
    /// `(name, type)`, sorted by name, the types of one name joined. Empty
    /// outside Symfony projects.
    pub fn twig_template_variables(&self, name: &str) -> Vec<(String, Type)> {
        if self.ref_context().is_none() || !self.frameworks().contains(crate::frameworks::Framework::Symfony) {
            return Vec::new();
        }
        let kind: &'static dyn ReferenceKind = &SymfonyTemplate;
        let Ok(candidates) = self.usage_candidates(kind, name) else { return Vec::new() };
        let open: Vec<_> = self.open_documents().into_iter().filter(|document| !document.embedded).collect();
        let resolver = self.resolver();
        let mut passed: Vec<(String, Type)> = Vec::new();
        for candidate in candidates {
            let Some(file) = self.usage_file(&candidate, &open, name) else { continue };
            let starts: Vec<u32> = sites(&file.parsed, &resolver, &[kind], None)
                .into_iter()
                .filter(|site| {
                    site.value == name && kind.patterns().iter().any(|pattern| pattern.matches(site, &resolver))
                })
                .map(|site| site.start)
                .collect();
            if starts.is_empty() {
                continue;
            }
            let names = FileNames::resolve(&file.parsed);
            let summary = FileSummary::extract(&file.parsed);
            let types = ExprTypes::new(&file.parsed, &names, &summary, &resolver);
            passed.extend(crate::frameworks::laravel::views::passed_variables(&types, &file.parsed, &starts));
        }
        let mut names: Vec<String> = passed.iter().map(|(name, _)| name.clone()).collect();
        names.sort();
        names.dedup();
        names
            .into_iter()
            .map(|name| {
                let known: Vec<Type> = passed
                    .iter()
                    .filter(|(passed, ty)| *passed == name && !matches!(ty, Type::Mixed))
                    .map(|(_, ty)| ty.clone())
                    .collect();
                let ty = if known.is_empty() { Type::Mixed } else { Type::union(known) };
                (name, ty)
            })
            .collect()
    }

    /// The type of the chain `names` (`post`, `author`: `post.author`) in the
    /// template named `template`, whose loops around the caret are `loops`
    /// (innermost last).
    fn twig_chain_type(
        &self,
        resolver: &Resolver<'_>,
        variables: &[(String, Type)],
        loops: &[LoopVariable],
        names: &[String],
        depth: usize,
    ) -> Type {
        let Some((root, rest)) = names.split_first() else { return Type::Mixed };
        // A loop's variable hides a variable of the same name passed in.
        let looped = loops.iter().rposition(|variable| variable.name == *root);
        let mut ty = match looped {
            Some(at) if depth < MAX_LOOPS => {
                // What it goes through is read with the loops around that loop only.
                element_of(&self.twig_chain_type(resolver, variables, &loops[..at], &loops[at].source, depth + 1))
            }
            Some(_) => Type::Mixed,
            None => variables.iter().find(|(name, _)| name == root).map_or(Type::Mixed, |(_, ty)| ty.clone()),
        };
        for name in rest {
            ty = Self::twig_member_type(resolver, &ty, name);
        }
        ty
    }

    /// The type of `ty.name` as Twig reads it.
    fn twig_member_type(resolver: &Resolver<'_>, ty: &Type, name: &str) -> Type {
        let context = Context::default();
        let mut found: Vec<Type> = Vec::new();
        for model in resolver.class_models(ty, &context) {
            let receiver = Type::named(model.fqn());
            let public = |visibility: Visibility| visibility == Visibility::Public;
            if model.property(name).is_some_and(|property| public(property.visibility)) {
                found.push(resolver.property_type(&receiver, name, &context));
                continue;
            }
            let method = model.methods.iter().find(|method| {
                public(method.visibility)
                    && (method.name.eq_ignore_ascii_case(name)
                        || read_as(&method.name).is_some_and(|read| read.eq_ignore_ascii_case(name)))
            });
            if let Some(method) = method {
                found.push(resolver.method_return(&receiver, &method.name, &context));
            }
        }
        if found.is_empty() { Type::Mixed } else { Type::union(found) }
    }

    /// What can follow the `.` after the chain `names` in the template named
    /// `template`: the public properties and methods of its class, sorted
    /// by name. Empty when the chain's type isn't known.
    pub fn twig_members(&self, template: &str, loops: &[LoopVariable], names: &[String]) -> Vec<TwigMember> {
        let variables = self.twig_template_variables(template);
        let resolver = self.resolver();
        let ty = self.twig_chain_type(&resolver, &variables, loops, names, 0);
        let context = Context::default();
        let mut out: Vec<TwigMember> = Vec::new();
        let add = |out: &mut Vec<TwigMember>, name: String, detail: String, is_method: bool| {
            if !out.iter().any(|known| known.name == name) {
                out.push(TwigMember { name, detail, is_method });
            }
        };
        for model in resolver.class_models(&ty, &context) {
            let receiver = Type::named(model.fqn());
            for property in model.properties.iter().filter(|p| !p.is_static && p.visibility == Visibility::Public) {
                let detail = resolver.property_type(&receiver, &property.name, &context).to_string();
                add(&mut out, property.name.clone(), detail, false);
            }
            for method in model.methods.iter().filter(|m| !m.is_static && m.visibility == Visibility::Public) {
                if method.name.starts_with("__") {
                    continue;
                }
                let detail = resolver.method_return(&receiver, &method.name, &context).to_string();
                match read_as(&method.name) {
                    Some(read) => add(&mut out, read, detail, false),
                    None => add(&mut out, method.name.clone(), detail, true),
                }
            }
        }
        out.sort_by(|a, b| a.name.cmp(&b.name));
        out
    }

    /// The member `name` after the chain `names` in the template named
    /// `template`: where it is declared (the property, or the method Twig
    /// calls for it), and its type. `None` when the chain's class has none.
    pub fn twig_member(
        &self,
        template: &str,
        loops: &[LoopVariable],
        names: &[String],
        name: &str,
    ) -> Option<(Vec<crate::navigate::Target>, String)> {
        use crate::types::model::{MethodSource, PropertySource};

        let variables = self.twig_template_variables(template);
        let resolver = self.resolver();
        let ty = self.twig_chain_type(&resolver, &variables, loops, names, 0);
        let context = Context::default();
        let mut targets: Vec<crate::navigate::Target> = Vec::new();
        let mut found = false;
        for model in resolver.class_models(&ty, &context) {
            // In the file of the class-like declaring the member.
            let mut add = |declaring: &str, range: crate::base::Range| {
                for class in self.class_targets(declaring).into_iter().take(1) {
                    let target = crate::navigate::Target { path: class.path, range };
                    if !targets.contains(&target) {
                        targets.push(target);
                    }
                }
            };
            if let Some(property) = model.property(name).filter(|p| p.visibility == Visibility::Public) {
                found = true;
                if let PropertySource::Declared(index) = property.source
                    && let Some(declared) = property.declaring.properties.get(index)
                {
                    add(&property.declaring.fqn, declared.name_loc.range);
                }
                continue;
            }
            let method = model.methods.iter().find(|method| {
                method.visibility == Visibility::Public
                    && (method.name.eq_ignore_ascii_case(name)
                        || read_as(&method.name).is_some_and(|read| read.eq_ignore_ascii_case(name)))
            });
            if let Some(method) = method {
                found = true;
                if let MethodSource::Declared(index) = method.source
                    && let Some(declared) = method.declaring.methods.get(index)
                {
                    add(&method.declaring.fqn, declared.name_loc.range);
                }
            }
        }
        found.then(|| (targets, Self::twig_member_type(&resolver, &ty, name).to_string()))
    }

    /// The variables of the template named `template` as completion shows
    /// them: the ones PHP passes, with their types.
    pub fn twig_variables(&self, template: &str) -> Vec<(String, String)> {
        self.twig_template_variables(template).into_iter().map(|(name, ty)| (name, ty.to_string())).collect()
    }
}
