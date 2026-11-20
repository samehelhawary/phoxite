//! The names a file declares for a framework
//! ([`NameFacts`](crate::summary::NameFacts)), read from the calls of
//! [`DECLARING`].
//!
//! The functions are the ones WordPress's developer handbooks document for
//! firing hooks and for registering shortcodes, post types, taxonomies,
//! scripts, styles, options, image sizes, menu locations and blocks. They are
//! matched by name only: a summary doesn't resolve calls, and whether the
//! project is a WordPress one is asked when the facts are read.

use mago_span::{HasSpan, Span};
use mago_syntax::cst::{Argument, ArrayElement, CompositeString, Expression, FunctionCall, Literal, StringPart};

use super::Extractor;
use crate::summary::{DeclaredKind, DeclaredName, DocBlock, GlobalVariable};

/// Which part of the argument is the name.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Form {
    /// The argument.
    Argument,
    /// The argument, when another follows it: `wp_enqueue_script('app',
    /// $src)` registers the script, `wp_enqueue_script('app')` only uses it.
    WithNext,
    /// Each key of the array the argument is.
    Keys,
}

/// A function, the position of the argument that names, how, and what the
/// name is of.
const DECLARING: &[(&str, usize, Form, DeclaredKind)] = &[
    ("do_action", 0, Form::Argument, DeclaredKind::Action),
    ("do_action_ref_array", 0, Form::Argument, DeclaredKind::Action),
    ("do_action_deprecated", 0, Form::Argument, DeclaredKind::Action),
    ("apply_filters", 0, Form::Argument, DeclaredKind::Filter),
    ("apply_filters_ref_array", 0, Form::Argument, DeclaredKind::Filter),
    ("apply_filters_deprecated", 0, Form::Argument, DeclaredKind::Filter),
    ("add_shortcode", 0, Form::Argument, DeclaredKind::Shortcode),
    ("register_post_type", 0, Form::Argument, DeclaredKind::PostType),
    ("register_taxonomy", 0, Form::Argument, DeclaredKind::Taxonomy),
    ("wp_register_script", 0, Form::Argument, DeclaredKind::Script),
    ("wp_enqueue_script", 0, Form::WithNext, DeclaredKind::Script),
    ("wp_register_style", 0, Form::Argument, DeclaredKind::Style),
    ("wp_enqueue_style", 0, Form::WithNext, DeclaredKind::Style),
    ("add_option", 0, Form::Argument, DeclaredKind::Option),
    ("update_option", 0, Form::Argument, DeclaredKind::Option),
    ("register_setting", 1, Form::Argument, DeclaredKind::Option),
    ("add_image_size", 0, Form::Argument, DeclaredKind::ImageSize),
    ("register_nav_menu", 0, Form::Argument, DeclaredKind::NavMenu),
    ("register_nav_menus", 0, Form::Keys, DeclaredKind::NavMenu),
    ("register_block_type", 0, Form::Argument, DeclaredKind::Block),
];

/// The part of a dynamic name that isn't written.
const UNKNOWN: char = '*';

impl Extractor<'_> {
    /// The names `node` declares, if it is a call of [`DECLARING`].
    pub(super) fn declared_name(&mut self, node: &FunctionCall<'_>) {
        let Expression::Identifier(function) = node.function else { return };
        let Some(called) = function.value().rsplit(|byte| *byte == b'\\').next() else { return };
        // Every one starts with one of these letters: most calls end here.
        if !matches!(called.first().map(u8::to_ascii_lowercase), Some(b'a' | b'd' | b'r' | b'u' | b'w')) {
            return;
        }
        if called.eq_ignore_ascii_case(b"add_action") || called.eq_ignore_ascii_case(b"add_filter") {
            self.hook_callback(node);
            return;
        }
        if called.eq_ignore_ascii_case(b"register_rest_route") {
            self.rest_route(node);
            return;
        }
        let Some((_, index, form, kind)) =
            DECLARING.iter().find(|(known, ..)| called.eq_ignore_ascii_case(known.as_bytes()))
        else {
            return;
        };
        let arguments = node.argument_list.arguments.as_slice();
        let Some(Argument::Positional(argument)) = arguments.get(*index) else { return };
        match form {
            Form::WithNext if arguments.len() <= index + 1 => {}
            Form::Argument | Form::WithNext => {
                self.declare(*kind, argument.value);
                if matches!(kind, DeclaredKind::Action | DeclaredKind::Filter) {
                    self.hook_parameters(node);
                }
            }
            Form::Keys => {
                let elements = match argument.value {
                    Expression::Array(array) => &array.elements,
                    Expression::LegacyArray(array) => &array.elements,
                    _ => return,
                };
                for element in elements.iter() {
                    if let ArrayElement::KeyValue(element) = element {
                        self.declare(*kind, element.key);
                    }
                }
            }
        }
    }

    /// The function or method `add_action('hook', …)` or `add_filter()`
    /// registers by name: `'function'`, `'Class::method'`, `[$this,
    /// 'method']`, `[Foo::class, 'method']`, `['Foo', 'method']`.
    fn hook_callback(&mut self, node: &FunctionCall<'_>) {
        let arguments = node.argument_list.arguments.as_slice();
        let (Some(Argument::Positional(hook)), Some(Argument::Positional(callback))) =
            (arguments.first(), arguments.get(1))
        else {
            return;
        };
        let text = |expr: &Expression<'_>| match expr {
            Expression::Literal(Literal::String(string)) => {
                string.value.map(|v| String::from_utf8_lossy(v).into_owned())
            }
            _ => None,
        };
        let Some(hook) = text(hook.value).filter(|hook| !hook.is_empty()) else { return };
        let callback = match callback.value {
            Expression::Array(_) | Expression::LegacyArray(_) => {
                let elements = match callback.value {
                    Expression::Array(array) => &array.elements,
                    Expression::LegacyArray(array) => &array.elements,
                    _ => return,
                };
                let mut values = elements.iter().map(|element| match element {
                    ArrayElement::Value(element) => Some(element.value),
                    _ => None,
                });
                let (Some(Some(object)), Some(Some(method)), None) = (values.next(), values.next(), values.next())
                else {
                    return;
                };
                let Some(method) = text(method) else { return };
                let own = || self.classes.last().cloned();
                let class = match object {
                    Expression::Variable(mago_syntax::cst::Variable::Direct(variable)) if variable.name == b"$this" => {
                        own()
                    }
                    Expression::Access(mago_syntax::cst::Access::ClassConstant(access))
                        if matches!(&access.constant, mago_syntax::cst::ClassLikeConstantSelector::Identifier(name)
                            if name.value.eq_ignore_ascii_case(b"class")) =>
                    {
                        match access.class {
                            Expression::Identifier(class) => Some(self.class_name(class)),
                            Expression::Self_(_) | Expression::Static(_) => own(),
                            _ => None,
                        }
                    }
                    other => text(other),
                };
                let Some(class) = class else { return };
                crate::summary::callback_key(Some(&class), &method)
            }
            other => {
                let Some(written) = text(other) else { return };
                match written.split_once("::") {
                    Some((class, method)) => crate::summary::callback_key(Some(class), method),
                    None => crate::summary::callback_key(None, &written),
                }
            }
        };
        self.summary.names.callbacks.push(crate::summary::HookCallback { hook, callback });
    }

    /// The route `register_rest_route('shop/v1', '/books/(?P<id>\d+)', …)`
    /// declares: `shop/v1/books/*`, at the route's string.
    fn rest_route(&mut self, node: &FunctionCall<'_>) {
        let arguments = node.argument_list.arguments.as_slice();
        let (Some(Argument::Positional(namespace)), Some(Argument::Positional(route))) =
            (arguments.first(), arguments.get(1))
        else {
            return;
        };
        let source = self.file.text();
        let namespace = rest_text(namespace.value, source);
        let namespace = namespace.trim_matches('/');
        let pattern = route_pattern(&rest_text(route.value, source));
        // A namespace that isn't known would make the route anything's.
        if namespace.is_empty() || namespace.contains(UNKNOWN) || pattern.is_empty() {
            return;
        }
        let name = format!("{namespace}/{pattern}");
        let dynamic = name.contains(UNKNOWN);
        let loc = self.loc(inside(route.value));
        self.summary.names.declared.push(DeclaredName {
            kind: DeclaredKind::RestRoute,
            name,
            dynamic,
            loc,
            params: Vec::new(),
        });
    }

    /// The global variables the `@global` tags of the file's functions and
    /// methods document.
    pub(super) fn documented_globals(&mut self) {
        let mut found: Vec<GlobalVariable> = Vec::new();
        let mut add = |doc: Option<&DocBlock>| {
            for global in doc.iter().flat_map(|doc| &doc.globals) {
                let Some(name) = &global.name else { continue };
                if !found.iter().any(|known| known.name == *name && known.ty == global.ty) {
                    found.push(GlobalVariable { name: name.clone(), ty: global.ty.clone() });
                }
            }
        };
        for function in &self.summary.functions {
            add(function.doc.as_deref());
        }
        for class in self.summary.class_likes.iter().chain(&self.summary.anonymous) {
            for method in &class.methods {
                add(method.doc.as_deref());
            }
        }
        self.summary.names.globals = found;
    }

    fn declare(&mut self, kind: DeclaredKind, value: &Expression<'_>) {
        let mut name = String::new();
        let mut dynamic = false;
        pattern(value, &mut name, &mut dynamic);
        // Only a hook is listened to by a name its firing call puts together.
        let hook = matches!(kind, DeclaredKind::Action | DeclaredKind::Filter);
        if name.is_empty() || name.chars().all(|c| c == UNKNOWN) || (dynamic && !hook) {
            return;
        }
        // `register_block_type()` also takes the folder of a `block.json`.
        if kind == DeclaredKind::Block && !block_name(&name) {
            return;
        }
        let loc = self.loc(inside(value));
        self.summary.names.declared.push(DeclaredName { kind, name, dynamic, loc, params: Vec::new() });
    }

    /// Gives the hook just declared by `node` the `@param` types of the
    /// docblock in front of its statement: where WordPress's documentation
    /// standard puts what a hook passes.
    fn hook_parameters(&mut self, node: &FunctionCall<'_>) {
        let start = node.span().start.offset;
        let Some(declared) = self.summary.names.declared.last().filter(|declared| declared.params.is_empty()) else {
            return;
        };
        if declared.loc.start < start {
            return;
        }
        // The statement starts the line: `$title = apply_filters(…)`, `return apply_filters(…)`.
        let text = self.file.text().as_bytes();
        let from = (start as usize).min(text.len());
        let line = text[..from].iter().rposition(|byte| *byte == b'\n').map_or(0, |at| at + 1);
        let statement = line + text[line..from].iter().take_while(|byte| byte.is_ascii_whitespace()).count();
        let Some(doc) = self.doc_at(statement as u32, &[]) else { return };
        let params: Vec<_> = doc.params.iter().map(|param| param.ty.clone()).collect();
        if let Some(declared) = self.summary.names.declared.last_mut() {
            declared.params = params;
        }
    }
}

/// What `expr` says, with [`UNKNOWN`] for what isn't written: literals,
/// their concatenation, and a property of `$this` that `source` sets to
/// one string ([`property_string`]).
fn rest_text(expr: &Expression<'_>, source: &str) -> String {
    use mago_syntax::cst::{Access, ClassLikeMemberSelector, Variable};
    match expr {
        Expression::Parenthesized(inner) => rest_text(inner.expression, source),
        Expression::Literal(Literal::String(string)) => match string.value {
            Some(value) => String::from_utf8_lossy(value).into_owned(),
            None => UNKNOWN.to_string(),
        },
        Expression::Binary(binary) if binary.operator.is_concatenation() => {
            format!("{}{}", rest_text(binary.lhs, source), rest_text(binary.rhs, source))
        }
        Expression::Access(Access::Property(access)) => {
            let name = match (&access.property, access.object) {
                (ClassLikeMemberSelector::Identifier(name), Expression::Variable(Variable::Direct(this)))
                    if this.name == b"$this" =>
                {
                    Some(String::from_utf8_lossy(name.value).into_owned())
                }
                _ => None,
            };
            name.and_then(|name| property_string(source, &name)).unwrap_or_else(|| UNKNOWN.to_string())
        }
        _ => UNKNOWN.to_string(),
    }
}

/// The string the file's text sets property `name` to, when every
/// `$this->name = '…';` and `$name = '…';` of it says the same one.
fn property_string(source: &str, name: &str) -> Option<String> {
    let mut found: Option<&str> = None;
    for written in [format!("$this->{name}"), format!("${name}")] {
        let mut from = 0;
        while let Some(at) = source[from..].find(&written) {
            from += at + written.len();
            let rest = &source[from..];
            if rest.starts_with(|c: char| c.is_alphanumeric() || c == '_') {
                continue;
            }
            let Some(value) = rest.trim_start().strip_prefix('=') else { continue };
            if value.starts_with(['=', '>']) {
                continue;
            }
            let value = value.trim_start();
            let quote = value.chars().next().filter(|c| matches!(c, '\'' | '"'))?;
            let end = value[1..].find(quote)?;
            let string = &value[1..1 + end];
            let plain = !string.contains(['$', '\\']) && value[2 + end..].trim_start().starts_with(';');
            if !plain || found.is_some_and(|known| known != string) {
                return None;
            }
            found = Some(string);
        }
    }
    found.map(str::to_owned)
}

/// A REST route as a name: without the slashes around it, and with
/// [`UNKNOWN`] for each part a pattern stands for (a group, a character
/// class, and what repeats them).
fn route_pattern(route: &str) -> String {
    let mut out = String::new();
    let unknown = |out: &mut String| {
        if !out.ends_with(UNKNOWN) {
            out.push(UNKNOWN);
        }
    };
    let mut chars = route.trim_matches('/').chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '(' | '[' => {
                let (open, close) = if c == '(' { ('(', ')') } else { ('[', ']') };
                let mut depth = 1;
                while depth > 0
                    && let Some(inner) = chars.next()
                {
                    match inner {
                        '\\' => {
                            chars.next();
                        }
                        inner if inner == open => depth += 1,
                        inner if inner == close => depth -= 1,
                        _ => {}
                    }
                }
                unknown(&mut out);
            }
            '\\' => {
                // An escaped character is itself; a class (`\d`) is something.
                match chars.next() {
                    Some(escaped) if escaped.is_ascii_alphanumeric() => unknown(&mut out),
                    Some(escaped) => out.push(escaped),
                    None => {}
                }
            }
            '?' | '+' | '*' | '^' | '$' => {
                if matches!(c, '+' | '*') {
                    unknown(&mut out);
                }
            }
            '{' => {
                for inner in chars.by_ref() {
                    if inner == '}' {
                        break;
                    }
                }
            }
            '.' => unknown(&mut out),
            other => out.push(other),
        }
    }
    out
}

/// Whether `name` is a block's: `namespace/name`, in lower case letters,
/// digits and dashes.
fn block_name(name: &str) -> bool {
    let part = |part: &str| {
        part.starts_with(|c: char| c.is_ascii_lowercase())
            && part.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    };
    name.split_once('/').is_some_and(|(namespace, block)| part(namespace) && part(block))
}

/// Appends what `expr` says to `name`: the text of literals, and
/// [`UNKNOWN`] once for whatever else is there.
fn pattern(expr: &Expression<'_>, name: &mut String, dynamic: &mut bool) {
    let mut unknown = |name: &mut String| {
        *dynamic = true;
        if !name.ends_with(UNKNOWN) {
            name.push(UNKNOWN);
        }
    };
    match expr {
        Expression::Parenthesized(inner) => pattern(inner.expression, name, dynamic),
        Expression::Literal(Literal::String(string)) => match string.value {
            Some(value) => name.push_str(&String::from_utf8_lossy(value)),
            None => unknown(name),
        },
        Expression::CompositeString(CompositeString::Interpolated(string)) => {
            for part in string.parts.iter() {
                match part {
                    StringPart::Literal(literal) => match literal.value {
                        Some(value) => name.push_str(&String::from_utf8_lossy(value)),
                        None => unknown(name),
                    },
                    _ => unknown(name),
                }
            }
        }
        Expression::Binary(binary) if binary.operator.is_concatenation() => {
            pattern(binary.lhs, name, dynamic);
            pattern(binary.rhs, name, dynamic);
        }
        _ => unknown(name),
    }
}

/// The span of a name's text: inside the quotes of a string, else the
/// whole expression.
fn inside(expr: &Expression<'_>) -> Span {
    let mut span = expr.span();
    let quoted = match expr {
        Expression::Literal(Literal::String(string)) => string.raw.len() >= 2,
        Expression::CompositeString(CompositeString::Interpolated(_)) => true,
        _ => false,
    };
    if quoted && span.end.offset >= span.start.offset + 2 {
        span.start.offset += 1;
        span.end.offset -= 1;
    }
    span
}
