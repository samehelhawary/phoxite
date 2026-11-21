//! Deprecated syntax.
//!
//! [`DeprecatedSyntax`] (`PhpDeprecatedSyntax`): code that still runs but
//! that the project's PHP version ([`PhpEngine::language_level`]) deprecates.
//! [`super::level::Deprecation`] reports uses of declarations marked
//! deprecated; this one reports the patterns no declaration carries.
//!
//! Since PHP 8.6 (the deprecations RFC for 8.6, and
//! `deprecate-return-value-from-construct`):
//!
//! - `return` in a `finally` block: it discards the exception or the value
//!   on its way out of the `try`;
//! - a value returned from `__construct()` or `__destruct()`, and either as
//!   a generator;
//! - a function named `readonly`;
//! - `let` or `is` as the name of a class-like, a function or a constant, or
//!   as an alias in a `use`; `_` as the name of a constant;
//! - a class constant named `namespace`;
//! - a third argument to `define()`;
//! - `?array` as the return type of `__debugInfo()` (`array` is meant).
//!
//! Nothing is reported below 8.6, nor in `.phpstorm.meta.php` files.
//!
//! [`PhpEngine::language_level`]: crate::engine::PhpEngine::language_level

use crate::base::Severity;
use mago_span::{HasSpan, Span};
use mago_syntax::cst::{
    ArrowFunction, Call, Class, ClassLikeConstantItem, Closure, ConstantItem, Enum, Expression, Function, Hint,
    Interface, LocalIdentifier, Method, Return, Trait, TryFinallyClause, UseItemAlias, Yield,
};
use mago_syntax::walker::Walker;

use super::{Inspection, InspectionContext, Problems};
use crate::engine::is_meta_path;
use crate::stubs::PhpVersion;

/// `PhpDeprecatedSyntax`.
pub struct DeprecatedSyntax;

/// The version these deprecations start at.
const SINCE: PhpVersion = PhpVersion::new(8, 6);

impl Inspection for DeprecatedSyntax {
    fn id(&self) -> &'static str {
        "PhpDeprecatedSyntax"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, cx: &InspectionContext<'_>, out: &mut Problems) {
        if is_meta_path(cx.path) || cx.engine.language_level() < SINCE {
            return;
        }
        for (span, message) in deprecated(cx.parsed.program()) {
            out.report(span.start.offset, span.end.offset, format!("{message} is deprecated since PHP {SINCE}"));
        }
    }
}

/// What the program uses that PHP 8.6 deprecates, in source order: where,
/// and what it is (`'return' in a 'finally' block`).
pub(crate) fn deprecated(program: &mago_syntax::cst::Program<'_>) -> Vec<(Span, String)> {
    let mut scan = Scan { found: Vec::new(), scopes: vec![Scope::default()] };
    SyntaxWalker.walk_program(program, &mut scan);
    scan.found.sort_by_key(|(span, _)| (span.start.offset, span.end.offset));
    scan.found
}

/// A function-like around the walk (the file itself first).
#[derive(Default)]
struct Scope {
    /// `__construct` or `__destruct`, by its name as written.
    constructor: Option<String>,
    /// Nesting of `finally` blocks inside it.
    finally: u32,
}

struct Scan {
    found: Vec<(Span, String)>,
    scopes: Vec<Scope>,
}

impl Scan {
    fn add(&mut self, span: Span, message: impl Into<String>) {
        self.found.push((span, message.into()));
    }

    fn scope(&mut self) -> &mut Scope {
        if self.scopes.is_empty() {
            self.scopes.push(Scope::default());
        }
        let last = self.scopes.len() - 1;
        &mut self.scopes[last]
    }

    /// `let` and `is`, which PHP is keeping for itself.
    fn reserved(&mut self, name: &LocalIdentifier<'_>, what: &str) {
        if name.value.eq_ignore_ascii_case(b"let") || name.value.eq_ignore_ascii_case(b"is") {
            self.add(name.span, format!("'{}' as the name of {what}", String::from_utf8_lossy(name.value)));
        }
    }
}

struct SyntaxWalker;

impl<'a> Walker<'a, 'a, Scan> for SyntaxWalker {
    fn walk_in_function(&self, node: &'a Function<'a>, cx: &mut Scan) {
        if node.name.value.eq_ignore_ascii_case(b"readonly") {
            cx.add(node.name.span, "A function named 'readonly'");
        }
        cx.reserved(&node.name, "a function");
        cx.scopes.push(Scope::default());
    }
    fn walk_out_function(&self, _: &'a Function<'a>, cx: &mut Scan) {
        cx.scopes.pop();
    }

    fn walk_in_method(&self, node: &'a Method<'a>, cx: &mut Scan) {
        let name = node.name.value;
        let constructor = name.eq_ignore_ascii_case(b"__construct") || name.eq_ignore_ascii_case(b"__destruct");
        if name.eq_ignore_ascii_case(b"__debuginfo")
            && let Some(returns) = &node.return_type_hint
            && nullable_array(&returns.hint)
        {
            cx.add(returns.hint.span(), "A nullable return type of '__debugInfo()'");
        }
        cx.scopes
            .push(Scope { constructor: constructor.then(|| String::from_utf8_lossy(name).into_owned()), finally: 0 });
    }
    fn walk_out_method(&self, _: &'a Method<'a>, cx: &mut Scan) {
        cx.scopes.pop();
    }

    fn walk_in_closure(&self, _: &'a Closure<'a>, cx: &mut Scan) {
        cx.scopes.push(Scope::default());
    }
    fn walk_out_closure(&self, _: &'a Closure<'a>, cx: &mut Scan) {
        cx.scopes.pop();
    }
    fn walk_in_arrow_function(&self, _: &'a ArrowFunction<'a>, cx: &mut Scan) {
        cx.scopes.push(Scope::default());
    }
    fn walk_out_arrow_function(&self, _: &'a ArrowFunction<'a>, cx: &mut Scan) {
        cx.scopes.pop();
    }

    fn walk_in_try_finally_clause(&self, _: &'a TryFinallyClause<'a>, cx: &mut Scan) {
        cx.scope().finally += 1;
    }
    fn walk_out_try_finally_clause(&self, _: &'a TryFinallyClause<'a>, cx: &mut Scan) {
        let scope = cx.scope();
        scope.finally = scope.finally.saturating_sub(1);
    }

    fn walk_in_return(&self, node: &'a Return<'a>, cx: &mut Scan) {
        let scope = cx.scope();
        let (finally, constructor) = (scope.finally > 0, scope.constructor.clone());
        if finally {
            cx.add(node.r#return.span, "'return' in a 'finally' block");
        }
        if let (Some(name), Some(value)) = (constructor, node.value) {
            cx.add(value.span(), format!("Returning a value from '{name}()'"));
        }
    }

    fn walk_in_yield(&self, node: &'a Yield<'a>, cx: &mut Scan) {
        if let Some(name) = cx.scope().constructor.clone() {
            let keyword = match node {
                Yield::Value(node) => node.r#yield.span,
                Yield::Pair(node) => node.r#yield.span,
                Yield::From(node) => node.r#yield.span.join(node.from.span),
            };
            cx.add(keyword, format!("'{name}()' as a generator"));
        }
    }

    fn walk_in_class(&self, node: &'a Class<'a>, cx: &mut Scan) {
        cx.reserved(&node.name, "a class");
    }
    fn walk_in_interface(&self, node: &'a Interface<'a>, cx: &mut Scan) {
        cx.reserved(&node.name, "an interface");
    }
    fn walk_in_trait(&self, node: &'a Trait<'a>, cx: &mut Scan) {
        cx.reserved(&node.name, "a trait");
    }
    fn walk_in_enum(&self, node: &'a Enum<'a>, cx: &mut Scan) {
        cx.reserved(&node.name, "an enum");
    }

    fn walk_in_constant_item(&self, node: &'a ConstantItem<'a>, cx: &mut Scan) {
        cx.reserved(&node.name, "a constant");
        if node.name.value == b"_" {
            cx.add(node.name.span, "'_' as the name of a constant");
        }
    }

    fn walk_in_class_like_constant_item(&self, node: &'a ClassLikeConstantItem<'a>, cx: &mut Scan) {
        if node.name.value.eq_ignore_ascii_case(b"namespace") {
            cx.add(node.name.span, "'namespace' as the name of a class constant");
        }
    }

    fn walk_in_use_item_alias(&self, node: &'a UseItemAlias<'a>, cx: &mut Scan) {
        cx.reserved(&node.identifier, "an alias");
    }

    fn walk_in_expression(&self, expr: &'a Expression<'a>, cx: &mut Scan) {
        // `define('NAME', 1, true)`: constants are case-sensitive since 8.0.
        if let Expression::Call(Call::Function(call)) = expr
            && let Expression::Identifier(name) = call.function
            && name.value().strip_prefix(b"\\").unwrap_or(name.value()).eq_ignore_ascii_case(b"define")
            && let Some(third) = call.argument_list.arguments.get(2)
        {
            cx.add(third.span(), "The third argument of 'define()'");
        }
    }
}

/// `?array`, `array|null`, `null|array`.
fn nullable_array(hint: &Hint<'_>) -> bool {
    match hint {
        Hint::Nullable(nullable) => matches!(nullable.hint, Hint::Array(_)),
        Hint::Union(union) => {
            matches!((union.left, union.right), (Hint::Array(_), Hint::Null(_)) | (Hint::Null(_), Hint::Array(_)))
        }
        Hint::Parenthesized(inner) => nullable_array(inner.hint),
        _ => false,
    }
}
