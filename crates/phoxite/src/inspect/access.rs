//! Members that exist but can't be used where they are used.
//!
//! - [`InaccessibleMember`] (`PhpInaccessibleMember`): a call of a private
//!   or protected method, a read of such a property or class constant, and
//!   a write of a property whose write visibility is narrower
//!   (`private(set)`, `protected(set)`, PHP 8.4) from code that isn't in a
//!   class allowed to.
//! - [`ReadonlyWrite`] (`PhpReadonlyPropertyWrittenOutsideDeclarationScope`):
//!   an assignment to a `readonly` property from outside the scope that may
//!   initialise it: its class, and from PHP 8.4 its subclasses too (a
//!   readonly property is `protected(set)` unless it says otherwise).
//!
//! The members are found by [`super::members`], which types each receiver
//! and knows its classes; this module holds the rules. PHP's:
//!
//! - private: only inside the class the member belongs to (for a trait's
//!   member, the class using the trait);
//! - protected: inside a class related by inheritance to the class that
//!   first declared the member, wherever a subclass overrode it since (so
//!   a class may call a sibling's override of a method of their parent).
//!
//! Lenient mode reports what fails for certain. Nothing is reported, in
//! either mode, when:
//!
//! - the receiver isn't only known classes with complete hierarchies (as
//!   for the undefined members), or one of them doesn't have the member;
//! - the member isn't declared in a class body: a `@method`/`@property`
//!   tag (also one named like a property that isn't accessible, which it
//!   stands in for), a `@mixin`'s, a macro, an enum built-in;
//! - magic would answer instead: `__call` for a method call (and
//!   `__callStatic` for a static one), `__get` for a read, `__set` for a
//!   write. PHP calls them for members that exist but aren't accessible;
//! - the code is in a trait (it runs in the classes using it), in an
//!   anonymous class, or in a closure or arrow function outside any class
//!   (`Closure::bind` and Pest give those a class);
//! - the receiver is Pest's `test()`: it is typed as the test case, and is
//!   a proxy that reaches the test case's protected methods;
//! - a subtype of the receiver's class has the member accessible (a
//!   subclass may widen a protected method to public, and the object may
//!   be one of it);
//! - the engine is indexing, or the file is a `.phpstorm.meta.php`.
//!
//! Strict mode adds what may fail: a union receiver of which only some
//! classes have the member inaccessible, and code in a closure inside a
//! class (reported as if it ran there; `Closure::bind` can give it another
//! class).
//!
//! A write is an assignment to the property (`=`, `.=`, `??=`, a
//! destructuring target) or into it (`$o->list[] = 1`). Not checked: `new`
//! on a class whose constructor isn't public, `++`/`--`, `&$o->p`,
//! `unset`, and a second assignment to a readonly property inside its
//! class.

use std::sync::Arc;

use super::calls::TypeCheckMode;
use super::members::Kind;
use super::{Inspection, InspectionContext, Problems};
use crate::base::Severity;
use crate::stubs::PhpVersion;
use crate::summary::Visibility;
use crate::types::model::Origin;
use crate::types::{ClassModel, Resolver};

/// `PhpInaccessibleMember`.
pub struct InaccessibleMember {
    pub mode: TypeCheckMode,
}

/// `PhpReadonlyPropertyWrittenOutsideDeclarationScope`.
pub struct ReadonlyWrite {
    pub mode: TypeCheckMode,
}

impl Inspection for InaccessibleMember {
    fn id(&self) -> &'static str {
        "PhpInaccessibleMember"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, cx: &InspectionContext<'_>, out: &mut Problems) {
        report(cx, Rule::Inaccessible, self.mode, out);
    }
}

impl Inspection for ReadonlyWrite {
    fn id(&self) -> &'static str {
        "PhpReadonlyPropertyWrittenOutsideDeclarationScope"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, cx: &InspectionContext<'_>, out: &mut Problems) {
        report(cx, Rule::Readonly, self.mode, out);
    }
}

fn report(cx: &InspectionContext<'_>, rule: Rule, mode: TypeCheckMode, out: &mut Problems) {
    let found = cx.inaccessible_members().iter();
    for found in found.filter(|f| f.rule == rule && (f.certain || mode == TypeCheckMode::Strict)) {
        out.report(found.start, found.end, found.message.clone());
    }
}

/// Which inspection a problem belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Rule {
    Inaccessible,
    Readonly,
}

/// A member used where it can't be.
#[derive(Clone, Debug)]
pub(crate) struct Found {
    pub(crate) rule: Rule,
    /// Fails at runtime for certain: reported in lenient mode too.
    pub(crate) certain: bool,
    pub(crate) start: u32,
    pub(crate) end: u32,
    pub(crate) message: String,
}

/// What the code does with the member.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Use {
    /// A call, a read, a constant: the member's visibility decides.
    Read,
    /// An assignment to a property: its write visibility decides.
    Write,
}

/// Why a member can't be used.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Blocked {
    pub(super) rule: Rule,
    pub(super) message: String,
}

/// The place a member is used from.
pub(super) struct Site<'s> {
    pub(super) resolver: &'s Resolver<'s>,
    /// The class the code is in; `None` outside any class.
    pub(super) scope: Option<&'s ClassModel>,
    pub(super) level: PhpVersion,
}

/// A member of one class as the rules need it.
struct Member<'m> {
    info: &'m crate::types::model::MemberInfo,
    /// The visibility of a write, for a property: its own unless
    /// `private(set)` / `protected(set)` or `readonly` narrow it.
    write: Visibility,
    readonly: bool,
    /// Whether the property says `(set)` itself.
    asymmetric: bool,
}

fn member<'m>(site: &Site<'_>, model: &'m ClassModel, kind: Kind, name: &str) -> Option<Member<'m>> {
    let level = site.level;
    let plain = |info: &'m crate::types::model::MemberInfo| Member {
        info,
        write: info.visibility,
        readonly: false,
        asymmetric: false,
    };
    match kind {
        Kind::Method => model.method(name).map(|method| plain(&method.info)),
        Kind::Constant => model.constant(name).map(|constant| plain(&constant.info)),
        Kind::Property => {
            // A `@property` tag of the same name stands in for a property
            // that isn't accessible (`__get` and `__set` serve it).
            let property = model.property_from(name, site.scope)?;
            let declared = property.declared();
            let set = declared.and_then(|declared| declared.set_visibility);
            let readonly = property.is_readonly;
            let write = match set {
                Some(set) => set,
                None if readonly => write_scope_of_readonly(property.info.visibility, level),
                None => property.info.visibility,
            };
            Some(Member { info: &property.info, write, readonly, asymmetric: set.is_some() })
        }
    }
}

/// Where a readonly property that doesn't say `(set)` may be initialised:
/// in its class alone before PHP 8.4; from 8.4 it is `protected(set)`, so
/// in its subclasses too (unless it is private altogether).
fn write_scope_of_readonly(visibility: Visibility, level: PhpVersion) -> Visibility {
    if level < PhpVersion::new(8, 4) || visibility == Visibility::Private {
        Visibility::Private
    } else {
        Visibility::Protected
    }
}

/// The class that first declared the member `info` of `model`: protected
/// access is checked against it, whichever subclass overrode it since.
fn first_declarer(site: &Site<'_>, kind: Kind, info: &crate::types::model::MemberInfo) -> Arc<str> {
    let mut class: Arc<str> = Arc::from(info.self_class.as_str());
    let mut overrides = info.overrides.clone();
    // A hierarchy that deep is a cycle.
    for _ in 0..64 {
        let Some(parent) = overrides else { break };
        let Some(model) = site.resolver.class_model(&parent) else { break };
        let found = match kind {
            Kind::Method => model.method(&info.name).map(|m| &m.info),
            Kind::Property => model.property(&info.name).map(|p| &p.info),
            Kind::Constant => model.constant(&info.name).map(|c| &c.info),
        };
        let Some(found) = found else { break };
        // A private member up there is another member, not this one's first declaration.
        if found.visibility == Visibility::Private {
            break;
        }
        class = Arc::from(found.self_class.as_str());
        overrides = found.overrides.clone();
    }
    class
}

/// Whether code at `site` may use a member with `visibility` of `model`.
fn allowed(
    site: &Site<'_>,
    model: &ClassModel,
    kind: Kind,
    info: &crate::types::model::MemberInfo,
    visibility: Visibility,
) -> bool {
    let same = |a: &str, b: &str| a.trim_start_matches('\\').eq_ignore_ascii_case(b.trim_start_matches('\\'));
    match visibility {
        Visibility::Public => true,
        Visibility::Private => site.scope.is_some_and(|scope| same(scope.fqn(), &info.self_class)),
        Visibility::Protected => site.scope.is_some_and(|scope| {
            let declarer = first_declarer(site, kind, info);
            scope.is_subclass_of(&declarer)
                || scope.is_subclass_of(&info.self_class)
                || site.resolver.class_model(&declarer).is_some_and(|declarer| declarer.is_subclass_of(scope.fqn()))
                || model.is_subclass_of(scope.fqn())
        }),
    }
}

/// Why `model`'s member `name` can't be used at `site`; `None` when it can,
/// or when the class has no such member declared in a class body.
pub(super) fn blocked(site: &Site<'_>, model: &ClassModel, kind: Kind, name: &str, usage: Use) -> Option<Blocked> {
    let member = member(site, model, kind, name)?;
    if !matches!(member.info.origin, Origin::Declared | Origin::Trait) {
        return None;
    }
    let owner = &member.info.self_class;
    let what = match kind {
        Kind::Method => format!("method '{}'", member.info.name),
        Kind::Property => format!("property '${}'", member.info.name.trim_start_matches('$')),
        Kind::Constant => format!("class constant '{}'", member.info.name),
    };
    let visibility = member.info.visibility;
    if !allowed(site, model, kind, member.info, visibility) {
        let message = format!("{} {what} of '{owner}' is not accessible here", capitalized(visibility.as_str()));
        return Some(Blocked { rule: Rule::Inaccessible, message });
    }
    if usage == Use::Read || allowed(site, model, kind, member.info, member.write) {
        return None;
    }
    if member.readonly && !member.asymmetric {
        let scope = match member.write {
            Visibility::Protected => "its class and subclasses",
            _ => "its class",
        };
        let message = format!("Readonly {what} of '{owner}' can only be written in {scope}");
        return Some(Blocked { rule: Rule::Readonly, message });
    }
    let message =
        format!("{} of '{owner}' is {}(set): it can't be written here", capitalized(&what), member.write.as_str());
    Some(Blocked { rule: Rule::Inaccessible, message })
}

/// Whether one of `models` has a method that answers for a member that
/// isn't accessible.
pub(super) fn magic_answers(models: &[Arc<ClassModel>], kind: Kind, is_static: bool, usage: Use) -> bool {
    let has = |name: &str| models.iter().any(|model| model.method(name).is_some());
    match (kind, usage) {
        (Kind::Method, _) if is_static => has("__callStatic") || has("__call"),
        (Kind::Method, _) => has("__call"),
        (Kind::Property, Use::Read) => !is_static && has("__get"),
        (Kind::Property, Use::Write) => !is_static && has("__set"),
        (Kind::Constant, _) => false,
    }
}

fn capitalized(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}
