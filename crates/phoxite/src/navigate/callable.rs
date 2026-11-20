//! String callables: the function or method a string names where a
//! callable is expected (`array_map('trim', …)`, `usort($a, 'App\\by_name')`,
//! `call_user_func('Foo::make')`, `[$this, 'handle']`, `[Foo::class, 'make']`).
//!
//! A string is one when what it is given to takes a callable and nothing
//! else a string (or, for the array form, an array) could be. That is the
//! parameter it is passed to, the return type of the function or method
//! returning it, or the property it is assigned to. For
//! `callable|string` the string may as well be a column's name. Those stay
//! possible usages ([`PhpEngine::possible_usages`](crate::engine::PhpEngine::possible_usages)).
//! What it names must exist: a function by its full name (a string isn't
//! resolved against the file's namespace or imports), or a method of the
//! class before `::`, of `Foo::class`, or of the object's type.
//!
//! A callable can also be the value of a key of an array argument, which
//! no parameter type says: WordPress registers callbacks that way
//! (`register_rest_route($ns, $route, ['callback' => [$this, 'run']])`).
//! [`KEYED`] lists those the WordPress developer reference documents, and
//! a key named as a callback (`callback`, `…_callback`, `…_cb`) is one
//! in any call.
//!
//! Such a string is then a name like any other: goto, hover, usages and
//! rename see through it ([`Lookup::callable_at`], and the usage scan), and
//! function names complete in it.

use mago_span::HasSpan;
use mago_syntax::cst::{
    Access, Argument, ArgumentList, ArrayElement, Call, ClassLikeMemberSelector, Expression, Literal, LiteralString,
    Return,
};
use mago_syntax::walker::Walker;

use super::symbol::{AtCaret, Decl, Lookup, MemberKind, Subject};
use crate::names::NameKind;
use crate::types::{Context, Frame, Relative, StringKind, Type};

/// What a string that can be a callable is given to.
#[derive(Clone, Copy)]
pub(super) enum Place<'a> {
    /// A call: the string is its argument [`Candidate::index`].
    Argument(&'a Expression<'a>),
    /// The function or method around returns it.
    Returned,
    /// It is assigned to this property.
    Assigned(&'a Expression<'a>),
}

/// A string that can be a callable.
pub(super) struct Candidate<'a> {
    /// The argument it is, or is in (0 when it is no argument).
    pub index: usize,
    pub string: &'a LiteralString<'a>,
    /// What it says.
    pub text: String,
    /// For the method of a callable array, the element before it.
    pub receiver: Option<&'a Expression<'a>>,
    /// It sits where only a callable goes ([`KEYED`]), whatever the
    /// parameter's type says.
    pub keyed: bool,
}

impl Candidate<'_> {
    /// The class before `::` (without a leading `\`) and the name: the
    /// function's, or the method's.
    pub fn parts(&self) -> (Option<&str>, &str) {
        match self.text.split_once("::") {
            Some((class, method)) if self.receiver.is_none() => (Some(class.trim_start_matches('\\')), method),
            _ => (None, self.text.trim_start_matches('\\')),
        }
    }
}

/// What a string callable names.
pub(super) struct Named {
    /// The bytes of the function's or the method's name (a function's with
    /// its namespace).
    pub name: (u32, u32),
    pub decls: Vec<Decl>,
    /// The class before `::`: its bytes and its name.
    pub class: Option<((u32, u32), String)>,
}

fn literal<'a>(expr: &'a Expression<'a>) -> Option<(&'a LiteralString<'a>, String)> {
    match expr {
        Expression::Parenthesized(inner) => literal(inner.expression),
        Expression::Literal(Literal::String(string)) if string.raw.len() >= 2 => {
            Some((string, String::from_utf8_lossy(string.value?).into_owned()))
        }
        _ => None,
    }
}

/// The string `value` is when it can be a callable: a string, or the
/// second element of a two-element array.
pub(super) fn candidate<'a>(value: &'a Expression<'a>, index: usize) -> Option<Candidate<'a>> {
    if let Some((string, text)) = literal(value) {
        return Some(Candidate { index, string, text, receiver: None, keyed: false });
    }
    let elements = match value {
        Expression::Array(array) => &array.elements,
        Expression::LegacyArray(array) => &array.elements,
        _ => return None,
    };
    let mut values = elements.iter().map(|element| match element {
        ArrayElement::Value(element) => Some(element.value),
        _ => None,
    });
    let (Some(Some(object)), Some(Some(method)), None) = (values.next(), values.next(), values.next()) else {
        return None;
    };
    let (string, text) = literal(method)?;
    Some(Candidate { index, string, text, receiver: Some(object), keyed: false })
}

/// A function, the position of its array argument, and the keys of that
/// array whose values are callables. An array of such arrays (the
/// endpoints of a REST route) is read the same.
const KEYED: &[(&str, usize, &[&str])] = &[
    ("register_rest_route", 2, &["callback", "permission_callback"]),
    ("register_rest_field", 2, &["get_callback", "update_callback"]),
    ("register_setting", 2, &["sanitize_callback"]),
    ("register_meta", 2, &["sanitize_callback", "auth_callback"]),
    ("register_post_meta", 2, &["sanitize_callback", "auth_callback"]),
    ("register_term_meta", 2, &["sanitize_callback", "auth_callback"]),
    ("register_post_type", 1, &["register_meta_box_cb"]),
    ("register_taxonomy", 2, &["meta_box_cb", "update_count_callback"]),
    ("register_block_type", 1, &["render_callback"]),
];

/// Whether a key says by its name that its value is a callback:
/// `callback`, `sanitize_callback`, `meta_box_cb`. No documentation lists
/// every array that has one (`$wp_customize->add_setting($id,
/// ['sanitize_callback' => 'absint'])`, a plugin's own registry).
fn callback_key(key: &str) -> bool {
    key == "callback" || key.ends_with("_callback") || key.ends_with("_cb")
}

/// The strings of `call` that are callables under a key of an array
/// argument: one of [`KEYED`], or one [named as a callback](callback_key).
pub(super) fn keyed_candidates<'a>(call: &'a Expression<'a>) -> Vec<Candidate<'a>> {
    let (list, function) = match call {
        Expression::Call(Call::Function(call)) => (&call.argument_list, Some(call.function)),
        Expression::Call(Call::Method(call)) => (&call.argument_list, None),
        Expression::Call(Call::NullSafeMethod(call)) => (&call.argument_list, None),
        Expression::Call(Call::StaticMethod(call)) => (&call.argument_list, None),
        Expression::Instantiation(new) => match &new.argument_list {
            Some(list) => (list, None),
            None => return Vec::new(),
        },
        _ => return Vec::new(),
    };
    let listed = function.and_then(|function| {
        let Expression::Identifier(name) = function else { return None };
        let name = name.value().rsplit(|byte| *byte == b'\\').next()?;
        KEYED.iter().find(|(known, ..)| name.eq_ignore_ascii_case(known.as_bytes()))
    });
    let mut out = Vec::new();
    for (index, argument) in list.arguments.iter().enumerate() {
        let Argument::Positional(argument) = argument else { continue };
        if !matches!(argument.value, Expression::Array(_) | Expression::LegacyArray(_)) {
            continue;
        }
        let keys: &[&str] = match listed {
            Some((_, at, keys)) if *at == index => keys,
            _ => &[],
        };
        keyed_values(argument.value, index, keys, 0, &mut out);
    }
    out
}

fn keyed_values<'a>(
    value: &'a Expression<'a>,
    index: usize,
    keys: &[&str],
    depth: usize,
    out: &mut Vec<Candidate<'a>>,
) {
    let elements = match value {
        Expression::Array(array) => &array.elements,
        Expression::LegacyArray(array) => &array.elements,
        _ => return,
    };
    for element in elements.iter() {
        match element {
            ArrayElement::KeyValue(element) => {
                let named =
                    literal(element.key).is_some_and(|(_, key)| keys.contains(&key.as_str()) || callback_key(&key));
                if named && let Some(found) = candidate(element.value, index) {
                    out.push(Candidate { keyed: true, ..found });
                }
            }
            // One endpoint among several.
            ArrayElement::Value(element) if depth == 0 => keyed_values(element.value, index, keys, 1, out),
            _ => {}
        }
    }
}

/// The strings of `list` that can be callables.
pub(super) fn candidates<'a>(list: &'a ArgumentList<'a>) -> Vec<Candidate<'a>> {
    let value = |argument: &'a Argument<'a>| match argument {
        Argument::Positional(argument) => argument.value,
        Argument::Named(argument) => argument.value,
    };
    list.arguments.iter().enumerate().filter_map(|(index, argument)| candidate(value(argument), index)).collect()
}

/// The class a callable array's first element stands for (`$o`,
/// `Foo::class`, `'Foo'`), and whether it is a class's name, not an object.
pub(crate) fn receiver(ty: &Type) -> (Type, bool) {
    let mut named = true;
    let members = ty.members().iter().map(|member| match member {
        Type::ClassString(Some(class)) => (**class).clone(),
        Type::LiteralString(class) => Type::named(class.trim_start_matches('\\')),
        other => {
            named = false;
            other.clone()
        }
    });
    let ty = Type::union(members.collect::<Vec<_>>());
    (ty, named)
}

/// How sure a string passed to a parameter is a callable.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Takes {
    /// The parameter takes callables and no other string (or array).
    Callable,
    /// It takes callables among other things.
    Maybe,
    No,
}

/// What a parameter of type `param` makes of a string (of a two-element
/// array when `array`) passed to it.
pub(crate) fn takes(param: &Type, array: bool) -> Takes {
    let members = param.members();
    if !members.iter().any(|member| matches!(member, Type::Callable(_) | Type::String(StringKind::Callable))) {
        return Takes::No;
    }
    let other = members.iter().any(|member| match member {
        Type::Mixed | Type::Template(_) => true,
        Type::Array { .. } | Type::List { .. } | Type::Shape(_) | Type::Iterable { .. } => array,
        Type::String(kind) => !array && *kind != StringKind::Callable,
        Type::LiteralString(_) | Type::ClassString(_) | Type::ArrayKey | Type::Scalar | Type::Numeric => !array,
        _ => false,
    });
    if other { Takes::Maybe } else { Takes::Callable }
}

impl<'a> Lookup<'a, '_> {
    /// Whether what `candidate` is given to at `place` takes a callable.
    pub(super) fn takes_callable(&self, place: Place<'a>, candidate: &Candidate<'_>, frames: &[Frame<'a>]) -> Takes {
        if candidate.keyed {
            return Takes::Callable;
        }
        let context = self.types.context(frames);
        let expected = match place {
            Place::Argument(call) => {
                let Some(list) = crate::signature::argument_list(call) else { return Takes::No };
                let Some(callee) = crate::signature::callee(self.types, self.names, self.resolver, call, frames) else {
                    return Takes::No;
                };
                let mut params = crate::signature::callee_params(self.resolver, self.here, &callee, &context);
                match crate::signature::parameter_index(&params, list, candidate.index) {
                    Some(param) => params.swap_remove(param).ty,
                    None => return Takes::No,
                }
            }
            // What the function or method declares it returns (a closure's
            // own return type isn't looked at).
            Place::Returned => match frames.iter().rev().find(|frame| !matches!(frame, Frame::Class(_))) {
                Some(Frame::Method(method)) => {
                    let name = String::from_utf8_lossy(method.name.value);
                    self.resolver.method_return(&Type::Relative(Relative::This), &name, &context)
                }
                Some(Frame::Function(function)) => {
                    let start = function.name.span.start.offset;
                    let declared = self.here.functions.iter().find(|decl| decl.name_loc.start == start);
                    match declared.and_then(|decl| self.resolver.function_return(&decl.fqn)) {
                        Some(ty) => ty,
                        None => return Takes::No,
                    }
                }
                _ => return Takes::No,
            },
            Place::Assigned(Expression::Access(Access::Property(access))) => match &access.property {
                ClassLikeMemberSelector::Identifier(name) => {
                    let object = self.types.type_of(access.object, frames);
                    self.resolver.property_type(&object, &String::from_utf8_lossy(name.value), &context)
                }
                _ => return Takes::No,
            },
            Place::Assigned(_) => return Takes::No,
        };
        takes(&expected, candidate.receiver.is_some())
    }

    /// What `candidate` names, when that exists.
    pub(super) fn callable_named(&self, candidate: &Candidate<'a>, frames: &[Frame<'a>]) -> Option<Named> {
        let span = candidate.string.span;
        let (start, end) = (span.start.offset + 1, span.end.offset - 1);
        let written = self.parsed.text().get(start as usize..end as usize)?;
        let context = self.types.context(frames);
        let methods = |receiver: &Type, name: &str| {
            let decls = self.members(MemberKind::Method, receiver, &context, name, None);
            // A method some class has, not one a `__call` may answer.
            decls.into_iter().filter(|decl| matches!(decl, Decl::Member(_))).collect::<Vec<_>>()
        };
        let identifier = |name: &str| super::rename::is_identifier(name);
        if let Some(object) = candidate.receiver {
            if !identifier(&candidate.text) {
                return None;
            }
            let (receiver, _) = receiver(&self.types.type_of(object, frames));
            let decls = methods(&receiver, &candidate.text);
            return (!decls.is_empty()).then_some(Named { name: (start, end), decls, class: None });
        }
        match candidate.parts() {
            (Some(class), method) => {
                let colons = written.rfind("::")? as u32;
                if !identifier(method) || class.is_empty() || !class.split('\\').all(identifier) {
                    return None;
                }
                let decls = methods(&Type::named(class), method);
                let class_start = start + written.len() as u32 - written.trim_start_matches('\\').len() as u32;
                let class = Some(((class_start, start + colons), class.to_owned()));
                (!decls.is_empty()).then_some(Named { name: (start + colons + 2, end), decls, class })
            }
            (None, function) => {
                if function.is_empty() || !function.split('\\').all(identifier) {
                    return None;
                }
                let declared = self.exists(NameKind::Function, function);
                declared.then(|| Named {
                    name: (start, end),
                    decls: vec![Decl::Function(function.to_owned())],
                    class: None,
                })
            }
        }
    }

    /// The function, method or class a caret at `offset` is on when it is
    /// in a string callable.
    pub(super) fn callable_at(&self, offset: u32) -> Option<AtCaret> {
        // Only a caret inside a string literal can be on one.
        let text = self.parsed.text().as_bytes();
        let caret = (offset as usize).min(text.len());
        let line_start = text[..caret].iter().rposition(|byte| *byte == b'\n').map_or(0, |at| at + 1);
        if !text[line_start..caret].iter().any(|byte| matches!(byte, b'\'' | b'"')) {
            return None;
        }
        let mut find = Find { offset, found: None };
        FindWalker.walk_program(self.parsed.program(), &mut find);
        let (place, value, index) = find.found?;
        let around = |candidate: &Candidate<'_>| {
            let inside = candidate.string.span;
            inside.start.offset < offset && offset < inside.end.offset
        };
        let keyed = || match place {
            Place::Argument(call) => keyed_candidates(call).into_iter().find(|candidate| around(candidate)),
            _ => None,
        };
        let candidate = candidate(value, index).filter(|candidate| around(candidate)).or_else(keyed)?;
        let frames = self.types.locate(offset).expression.map(|(_, frames)| frames).unwrap_or_default();
        if self.takes_callable(place, &candidate, &frames) != Takes::Callable {
            return None;
        }
        let named = self.callable_named(&candidate, &frames)?;
        let context: Context = self.types.context(&frames);
        let index = self.parsed.line_index();
        if let Some(((start, end), class)) = named.class
            && offset <= end
        {
            let subject = Subject::Decls(vec![Decl::ClassLike(class)]);
            return Some(AtCaret { range: index.range(start, end), subject, context });
        }
        let (start, end) = named.name;
        Some(AtCaret { range: index.range(start, end), subject: Subject::Decls(named.decls), context })
    }
}

/// The innermost place a value around the offset is given to: the place,
/// the value, and which argument it is.
struct Find<'a> {
    offset: u32,
    found: Option<(Place<'a>, &'a Expression<'a>, usize)>,
}

impl Find<'_> {
    fn around(&self, expr: &Expression<'_>) -> bool {
        let span = expr.span();
        span.start.offset < self.offset && self.offset < span.end.offset
    }
}

struct FindWalker;

impl<'a> Walker<'a, 'a, Find<'a>> for FindWalker {
    fn walk_in_expression(&self, expr: &'a Expression<'a>, cx: &mut Find<'a>) {
        if let Expression::Assignment(assignment) = expr
            && cx.around(assignment.rhs)
        {
            cx.found = Some((Place::Assigned(assignment.lhs), assignment.rhs, 0));
        }
        let Some(list) = crate::signature::argument_list(expr) else { return };
        let value = |argument: &'a Argument<'a>| match argument {
            Argument::Positional(argument) => argument.value,
            Argument::Named(argument) => argument.value,
        };
        if let Some(index) = list.arguments.iter().position(|argument| cx.around(value(argument))) {
            cx.found = Some((Place::Argument(expr), value(&list.arguments.as_slice()[index]), index));
        }
    }

    fn walk_in_return(&self, node: &'a Return<'a>, cx: &mut Find<'a>) {
        if let Some(value) = node.value
            && cx.around(value)
        {
            cx.found = Some((Place::Returned, value, 0));
        }
    }
}
