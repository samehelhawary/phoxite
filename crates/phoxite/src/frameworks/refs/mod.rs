//! String references: completion, goto, find usages, rename and document
//! highlights for string literals in known places (`config('app.name')`,
//! `route('home')`, `view('posts.index')`, …), for every framework.
//!
//! The design follows the public description of IntelliJ's reference
//! contributors (IntelliJ Community, Apache-2.0): a **place pattern**
//! ([`PlacePattern`]) says where a string sits (an argument of a function,
//! of a method of a class or its subclasses, of `new`; by position or by
//! name; the string itself or a key or value of an array argument, nested
//! or not; an attribute's arguments are its class's `__construct`'s), and a
//! **reference kind** ([`ReferenceKind`]) says what such a
//! string names and supplies completion items, targets, usages, rename and
//! validation.
//!
//! Each framework lists its kinds ([`crate::frameworks::laravel::refs::kinds`]),
//! and [`kinds_for`] gives the kinds of the frameworks a project uses
//! ([`PhpEngine::frameworks`]): a kind is only asked in a project of its
//! framework, so two frameworks may give the same call (`view('…')`) a
//! meaning each. Everything is off in a project none of whose frameworks
//! has kinds.
//!
//! A place may also be the value a method returns
//! ([`CalleePattern::Return`]: what a Form Request's `rules()` returns), or
//! the default value of a property ([`CalleePattern::Property`]: a model's
//! `$fillable`).
//!
//! A kind may describe a usage for hover ([`ReferenceKind::hover`]) and say
//! what is wrong with one that names nothing ([`ReferenceKind::undefined`]),
//! which the "undefined …" inspections
//! ([`crate::inspect::laravel::UndefinedReference`]) report.
//!
//! A kind sees the string's site ([`SiteInfo`]): its file, and the class it
//! is about (the `Foo::class` before it in its array, or the controller of
//! the route registering it), so `[PostController::class, 'index']` names a
//! method of `PostController`. Usages are compared by their canonical value
//! ([`ReferenceKind::canonical`]: `App\Http\Controllers\PostController@index`).
//!
//! The string at the caret is found in the document's parse, or, while it
//! is being typed and isn't closed yet, in a copy of the file with a closed
//! probe string in its place (as completion does), so a partial string
//! (`config('app.na`) works too. Find usages goes through the word index
//! (the words of the value and the kind's words) and checks each candidate
//! string against the patterns again. Blade templates are compiled first
//! ([`crate::blade::compile`]), so the strings of their directives and echoes
//! count too, at their places in the template.
//!
//! Usages that aren't string literals (`<livewire:counter />` tags) come
//! from [`ReferenceKind::markup_usages`]. A rename edits every declaration
//! of the value ([`RenameTarget::declarations`]: each locale's key, both
//! `.env` files).
//!
//! What is still Laravel's here: the completion rules of `ide.json`
//! ([`crate::frameworks::laravel::ide_json`], read in a Laravel project
//! only), the helper instances `sites` knows, and the handling of a few
//! Laravel kinds in [`PhpEngine::undefined_references`].

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::base::{
    CancellationToken, DocId, FileEdit, IdeError, IdeResult, Location, Pos, Range, Severity, TextEdit, WorkspaceEdit,
};
use mago_span::{HasSpan, Span};
use mago_syntax::cst::{
    Access, Argument, ArgumentList, ArrayElement, Call, ClassLikeConstantSelector, ClassLikeMemberSelector, Expression,
    Literal,
};
use mago_syntax::walker::Walker;

use super::Frameworks;
use super::laravel::LaravelProject;
use super::laravel::ide_json::CompletionRule;
use super::laravel::refs::{AuthGuard, ConfigKey, Gate, Middleware, middleware, rules};
use crate::complete::{Completion, CompletionKind, Completions};
use crate::engine::PhpEngine;
use crate::names::NameKind;
use crate::names::{FileNames, Resolution, fqn_eq};
use crate::navigate::{Target, Usage, UsageKind};
use crate::signature::Callee;
use crate::summary::FileSummary;
use crate::syntax::ParsedFile;
use crate::types::{ExprTypes, Resolver, Type};

// --------------------------------------------------------------- patterns

/// The call a [`PlacePattern`] is about.
#[derive(Clone, Copy, Debug)]
pub enum CalleePattern {
    /// A function, by FQN without the leading `\` (`config`).
    Function(&'static str),
    /// A function of this name that no PHP file declares: a template
    /// engine's function written in PHP (Twig's `path()`). The name may be
    /// read as one of the current namespace.
    UndeclaredFunction(&'static str),
    /// One of `names` (any case) called on one of `classes`, a subclass or
    /// an implementation of one (FQNs; `new` is `__construct`).
    Method { classes: &'static [&'static str], names: &'static [&'static str] },
    /// What one of `names` (any case), a method of one of `classes` or of a
    /// subclass, returns (`rules()` of a Form Request). The returned value
    /// is the argument at index 0.
    Return { classes: &'static [&'static str], names: &'static [&'static str] },
    /// What one of `names` (any case) returns in a Form Request (or a
    /// subclass of `form_request`), or in a class that validates with it
    /// (`Validator::make($data, $this->rules())` in the class or an
    /// ancestor, [`LaravelFacts::rule_methods`](crate::summary::LaravelFacts::rule_methods)).
    /// The returned value is the argument at index 0.
    Rules { form_request: &'static str, names: &'static [&'static str] },
    /// The default value of one of the properties `names` (without `$`) of
    /// one of `classes` or of a subclass (a model's `$fillable`). The value
    /// is the argument at index 0.
    Property { classes: &'static [&'static str], names: &'static [&'static str] },
    /// The value under this key of any array, wherever the array is
    /// written: `'#theme' => 'item_list'` of a Drupal render array, and
    /// `$build['#theme'] = 'item_list'`. The value is the argument at
    /// index 0; an array below it is reached with a [`Position`].
    Entry(&'static str),
    /// The key read from one of the properties `names` of an object of one
    /// of `classes` or of a subclass: `'adminEmail'` in
    /// `Yii::$app->params['adminEmail']`, and `'from'` in
    /// `Yii::$app->params['mail']['from']` (the keys before it are the
    /// target's, [`CallTarget::Read`]). The key is the argument at index 0.
    Read { classes: &'static [&'static str], names: &'static [&'static str] },
}

/// Where in the argument the string sits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Position {
    /// The argument is the string.
    Argument,
    /// A key of the array argument.
    ArrayKey,
    /// A value of the array argument.
    ArrayValue,
    /// A value without a key of the array argument (a list item).
    ListValue,
    /// The value under this key of the array argument.
    ValueOfKey(&'static str),
    /// A list item of the array under this key of the array argument:
    /// `'book'` in `['post_type' => ['book', 'page']]`.
    ListOfKey(&'static str),
}

/// A place a string literal can sit in; see the module documentation.
#[derive(Clone, Copy, Debug)]
pub struct PlacePattern {
    pub callee: CalleePattern,
    /// The parameter: its position and its name (for named arguments).
    pub index: usize,
    pub name: &'static str,
    pub position: Position,
    /// The array position also holds in arrays nested in the argument at
    /// any depth (as values).
    pub nested: bool,
}

impl PlacePattern {
    /// The string itself is argument `index` (named `name`) of `callee`.
    pub const fn argument(callee: CalleePattern, index: usize, name: &'static str) -> Self {
        Self { callee, index, name, position: Position::Argument, nested: false }
    }

    pub const fn at(self, position: Position) -> Self {
        Self { position, ..self }
    }

    pub const fn nested(self) -> Self {
        Self { nested: true, ..self }
    }

    /// The function or method names the pattern is about (for a quick
    /// look at a call before its types are worked out).
    fn names(&self) -> Vec<&'static str> {
        match self.callee {
            CalleePattern::Function(fqn) | CalleePattern::UndeclaredFunction(fqn) => {
                vec![fqn.rsplit('\\').next().unwrap_or(fqn)]
            }
            CalleePattern::Entry(key) => vec![key],
            CalleePattern::Method { names, .. }
            | CalleePattern::Read { names, .. }
            | CalleePattern::Return { names, .. }
            | CalleePattern::Rules { names, .. }
            | CalleePattern::Property { names, .. } => names.to_vec(),
        }
    }

    /// Whether `site` is in this place.
    pub fn matches(&self, site: &StringSite, resolver: &Resolver<'_>) -> bool {
        let argument = match &site.argument_name {
            Some(name) => name == self.name,
            None => site.index == Some(self.index),
        };
        argument && self.position_matches(&site.steps) && self.callee_matches(&site.target, resolver)
    }

    fn position_matches(&self, steps: &[ArrayStep]) -> bool {
        if let Position::ListOfKey(wanted) = self.position {
            return matches!(steps, [ArrayStep::Value(Some(key)), ArrayStep::Value(None)] if key == wanted);
        }
        let Some((last, outer)) = steps.split_last() else { return self.position == Position::Argument };
        let last_matches = match (self.position, last) {
            (Position::Argument, _) => false,
            (Position::ArrayKey, ArrayStep::Key) => true,
            (Position::ArrayValue, ArrayStep::Value(_)) => true,
            (Position::ListValue, ArrayStep::Value(None)) => true,
            (Position::ValueOfKey(wanted), ArrayStep::Value(Some(key))) => key == wanted,
            _ => false,
        };
        last_matches
            && (outer.is_empty() || (self.nested && outer.iter().all(|step| matches!(step, ArrayStep::Value(_)))))
    }

    fn callee_matches(&self, target: &CallTarget, resolver: &Resolver<'_>) -> bool {
        match (self.callee, target) {
            (CalleePattern::Function(wanted), CallTarget::Function(candidates)) => {
                candidates.iter().any(|fqn| fqn_eq(NameKind::Function, fqn.trim_start_matches('\\'), wanted))
            }
            (CalleePattern::UndeclaredFunction(wanted), CallTarget::Function(candidates)) => {
                let named = |fqn: &str| fqn.rsplit('\\').next().is_some_and(|name| name.eq_ignore_ascii_case(wanted));
                candidates.iter().any(|fqn| named(fqn))
                    && candidates.iter().all(|fqn| resolver.declarations().function(fqn).is_none())
            }
            (CalleePattern::Method { classes, names }, CallTarget::Method { classes: receivers, name, .. }) => {
                names.iter().any(|wanted| wanted.eq_ignore_ascii_case(name))
                    && receivers.iter().any(|receiver| {
                        classes.iter().any(|class| fqn_eq(NameKind::ClassLike, receiver, class))
                            || resolver.class_model(receiver).is_some_and(|model| {
                                classes.iter().any(|class| class.contains('\\') && model.is_subclass_of(class))
                                    // A method of a listed trait the class uses
                                    // (`AuthorizesRequests::authorize`).
                                    || model.method(name).is_some_and(|method| {
                                        classes.iter().any(|class| {
                                            fqn_eq(NameKind::ClassLike, method.declaring.fqn.trim_start_matches('\\'), class)
                                        })
                                    })
                            })
                    })
            }
            (CalleePattern::Return { classes, names }, CallTarget::Return { class, method })
            | (CalleePattern::Property { classes, names }, CallTarget::Property { class, name: method }) => {
                names.iter().any(|wanted| *wanted == "*" || wanted.eq_ignore_ascii_case(method))
                    && (classes.iter().any(|wanted| fqn_eq(NameKind::ClassLike, class, wanted))
                        || resolver.class_model(class).is_some_and(|model| {
                            classes.iter().any(|wanted| wanted.contains('\\') && model.is_subclass_of(wanted))
                        }))
            }
            (CalleePattern::Entry(wanted), CallTarget::Entry { key }) => key == wanted,
            (CalleePattern::Read { classes, names }, CallTarget::Read { classes: receivers, name, .. }) => {
                names.contains(&name.as_str())
                    && receivers.iter().any(|receiver| {
                        classes.iter().any(|class| fqn_eq(NameKind::ClassLike, receiver, class))
                            || resolver
                                .class_model(receiver)
                                .is_some_and(|model| classes.iter().any(|class| model.is_subclass_of(class)))
                    })
            }
            (CalleePattern::Rules { form_request, names }, CallTarget::Return { class, method }) => {
                names.iter().any(|wanted| wanted.eq_ignore_ascii_case(method))
                    && resolver.class_model(class).is_some_and(|model| {
                        model.is_subclass_of(form_request)
                            || resolver.laravel().returns_rules(model.fqn(), &model.parents, method)
                    })
            }
            _ => false,
        }
    }
}

// ------------------------------------------------------------------ sites

/// The receiver of a method called on `$this` outside any class, in a
/// [`CallTarget::Method`]: a template's `$this` is the object rendering it,
/// which the file doesn't say. A pattern lists it among its classes to
/// take such calls (CodeIgniter's `$this->extend('layout')` in a view).
pub const TEMPLATE_THIS: &str = "$this";

/// The call a string literal is an argument of.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CallTarget {
    /// A function: the FQNs the name may mean (the namespaced one, then
    /// the global one for an unqualified name).
    Function(Vec<String>),
    /// A method of the receiver's classes (FQNs); `models` are the models
    /// the receiver's generic arguments name (`Builder<Post>`), `tables`
    /// the tables they name (`DB::table('posts')` is a
    /// `Query\Builder<'posts'>`).
    Method { classes: Vec<String>, name: String, models: Vec<String>, tables: Vec<String> },
    /// The value a method returns: the class declaring it (FQN) and its
    /// name.
    Return { class: String, method: String },
    /// The default value of a property: the class declaring it (FQN) and
    /// its name without `$`.
    Property { class: String, name: String },
    /// The value under a key of an array ([`CalleePattern::Entry`]).
    Entry { key: String },
    /// A key read from the property `name` of an object of `classes`
    /// ([`CalleePattern::Read`]); `keys` are the keys read before it
    /// (`mail` for `'from'` in `->params['mail']['from']`).
    Read { classes: Vec<String>, name: String, keys: Vec<String> },
}

/// One step from an argument into the arrays around the string.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ArrayStep {
    Key,
    /// A value, with its string key when it has one.
    Value(Option<String>),
}

/// A string literal argument of a call.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StringSite {
    /// The byte range of the content, inside the quotes.
    pub start: u32,
    pub end: u32,
    /// The content as written.
    pub value: String,
    pub target: CallTarget,
    /// The position among the positional arguments; `None` for a named
    /// argument.
    pub index: Option<usize>,
    pub argument_name: Option<String>,
    /// From the argument down to the string.
    pub steps: Vec<ArrayStep>,
    /// The class a `Foo::class` element of the string's array names.
    pub class: Option<String>,
    /// The byte the call (or the `return`) starts at: strings of one call
    /// share it.
    pub call: u32,
    /// What the positional argument after the string's argument is, for
    /// kinds that ask ([`ReferenceKind::uses_next_argument`]): the classes
    /// of its type (`$post` in `Gate::allows('update', $post)`, the first
    /// element of an array), the class of `Post::class`, or the content of
    /// a string literal.
    pub next_argument: Vec<String>,
    /// For a string of a later argument, the first positional argument when
    /// it is a string literal (its content) or `Foo::class` (the class):
    /// `counter` for the keys of `@livewire('counter', ['start' => 1])`.
    pub first_argument: Option<String>,
}

/// Helpers that return a service when called without arguments, as
/// Laravel's `helpers.php` documents them (`config()` is the repository).
const HELPER_INSTANCES: &[(&str, &str)] = &[
    ("config", "Illuminate\\Config\\Repository"),
    ("redirect", "Illuminate\\Routing\\Redirector"),
    ("url", "Illuminate\\Routing\\UrlGenerator"),
    ("response", "Illuminate\\Routing\\ResponseFactory"),
    ("view", "Illuminate\\View\\Factory"),
    ("trans", "Illuminate\\Translation\\Translator"),
    ("auth", "Illuminate\\Auth\\AuthManager"),
];

/// What a string literal sits in: a call, or the value a method returns.
#[derive(Clone)]
enum SiteCall<'a> {
    Call(&'a Expression<'a>),
    /// The class (as written, with the offset of its name), the method and
    /// the offset of the `return`.
    Return {
        class: (String, u32),
        method: String,
        at: u32,
    },
    /// The class (as above), the property (without `$`) and the offset of
    /// its variable.
    Property {
        class: (String, u32),
        name: String,
        at: u32,
    },
    /// An attribute (`#[Template('page.html.twig')]`): its class (as above)
    /// and the offset it starts at. Its arguments are its constructor's.
    Attribute {
        class: (String, u32),
        at: u32,
    },
    /// The value under `key` of an array, the key at `at`.
    Entry {
        key: String,
        at: u32,
    },
    /// A key read from the property `name` of `object`, after `keys`.
    Read {
        object: &'a Expression<'a>,
        name: String,
        keys: Vec<String>,
        at: u32,
    },
}

impl SiteCall<'_> {
    fn start(&self) -> u32 {
        match self {
            SiteCall::Call(call) => call.span().start.offset,
            SiteCall::Return { at, .. }
            | SiteCall::Property { at, .. }
            | SiteCall::Attribute { at, .. }
            | SiteCall::Entry { at, .. }
            | SiteCall::Read { at, .. } => *at,
        }
    }
}

/// A class-like, function or method the walker is in.
enum Frame {
    /// A named class (its name and offset), or another class-like.
    Class(Option<(String, u32)>),
    Method(String),
    Function,
}

/// A string literal argument before its call is resolved.
struct RawSite<'a> {
    call: SiteCall<'a>,
    span: Span,
    index: Option<usize>,
    argument_name: Option<String>,
    steps: Vec<ArrayStep>,
    /// The `Foo::class` of the string's array: the name and its offset.
    class: Option<(String, u32)>,
    /// The positional argument after the string's argument, when its call
    /// is named like a pattern of a kind using it.
    next: Option<&'a Expression<'a>>,
    /// The first positional argument, for strings of later arguments.
    first: Option<&'a Expression<'a>>,
}

struct Collect<'a, 'n> {
    /// Only the literal starting at this offset.
    want: Option<u32>,
    /// The function and method names asked about, lower case.
    names: &'n HashSet<String>,
    /// Those of kinds using the argument after the string's.
    next_names: &'n HashSet<String>,
    /// The argument after the one being walked, for those calls.
    next: Option<&'a Expression<'a>>,
    /// The first positional argument, while a later one is walked.
    first: Option<&'a Expression<'a>>,
    out: Vec<RawSite<'a>>,
    /// The class-likes, functions and methods around the walker.
    frames: Vec<Frame>,
}

struct SiteWalker;

fn lossy(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn member_name(selector: &ClassLikeMemberSelector<'_>) -> Option<String> {
    match selector {
        ClassLikeMemberSelector::Identifier(name) => Some(lossy(name.value)),
        _ => None,
    }
}

/// The name of what `expr` calls and its arguments.
fn call_parts<'a>(expr: &'a Expression<'a>) -> Option<(String, &'a ArgumentList<'a>)> {
    match expr {
        Expression::Call(Call::Function(call)) => match call.function {
            Expression::Identifier(identifier) => {
                let name = lossy(identifier.value());
                let short = name.rsplit('\\').next().unwrap_or(&name).to_owned();
                Some((short, &call.argument_list))
            }
            _ => None,
        },
        Expression::Call(Call::Method(call)) => Some((member_name(&call.method)?, &call.argument_list)),
        Expression::Call(Call::NullSafeMethod(call)) => Some((member_name(&call.method)?, &call.argument_list)),
        Expression::Call(Call::StaticMethod(call)) => Some((member_name(&call.method)?, &call.argument_list)),
        Expression::Instantiation(instantiation) => {
            Some(("__construct".to_owned(), instantiation.argument_list.as_ref()?))
        }
        _ => None,
    }
}

impl<'a> Collect<'a, '_> {
    fn strings(
        &mut self,
        call: &SiteCall<'a>,
        value: &'a Expression<'a>,
        at: (Option<usize>, Option<&str>),
        steps: &mut Vec<ArrayStep>,
        class: Option<&(String, u32)>,
    ) {
        match value {
            Expression::Literal(Literal::String(string)) => {
                let span = string.span();
                if self.want.is_none_or(|want| want == span.start.offset) {
                    self.out.push(RawSite {
                        call: call.clone(),
                        span,
                        index: at.0,
                        argument_name: at.1.map(str::to_owned),
                        steps: steps.clone(),
                        class: class.cloned(),
                        next: self.next,
                        first: self.first,
                    });
                }
            }
            Expression::Parenthesized(inner) => self.strings(call, inner.expression, at, steps, class),
            Expression::Array(_) | Expression::LegacyArray(_) => {
                let elements: Vec<&ArrayElement<'_>> = match value {
                    Expression::Array(array) => array.elements.iter().collect(),
                    Expression::LegacyArray(array) => array.elements.iter().collect(),
                    _ => Vec::new(),
                };
                let class = elements.iter().find_map(|element| match element {
                    ArrayElement::Value(element) => class_constant(element.value),
                    _ => None,
                });
                let class = class.as_ref();
                for element in elements {
                    match element {
                        ArrayElement::KeyValue(element) => {
                            steps.push(ArrayStep::Key);
                            self.strings(call, element.key, at, steps, class);
                            steps.pop();
                            let key = match element.key {
                                Expression::Literal(Literal::String(string)) => string.value.map(lossy),
                                _ => None,
                            };
                            steps.push(ArrayStep::Value(key));
                            self.strings(call, element.value, at, steps, class);
                            steps.pop();
                        }
                        ArrayElement::Value(element) => {
                            steps.push(ArrayStep::Value(None));
                            self.strings(call, element.value, at, steps, class);
                            steps.pop();
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
}

impl<'a> Collect<'a, '_> {
    /// The strings under a key asked about ([`CalleePattern::Entry`]): of
    /// the array `expr` is, or of the array element it assigns.
    fn entries(&mut self, expr: &'a Expression<'a>) {
        let elements = match expr {
            Expression::Array(array) => Some(&array.elements),
            Expression::LegacyArray(array) => Some(&array.elements),
            _ => None,
        };
        for element in elements.into_iter().flat_map(|elements| elements.iter()) {
            let ArrayElement::KeyValue(element) = element else { continue };
            let Expression::Literal(Literal::String(key)) = element.key else { continue };
            let Some(key) = key.value.map(lossy).filter(|key| self.names.contains(&key.to_ascii_lowercase())) else {
                continue;
            };
            let call = SiteCall::Entry { key, at: element.key.span().start.offset };
            self.strings(&call, element.value, (Some(0), None), &mut Vec::new(), None);
        }
        // `Yii::$app->params['mail']['from']`: a key read from the property asked about
        // ([`CalleePattern::Read`]), after the keys read before it.
        if let Expression::ArrayAccess(access) = expr {
            let mut keys: Vec<String> = Vec::new();
            let mut below = access.array;
            while let Expression::ArrayAccess(inner) = below
                && let Expression::Literal(Literal::String(key)) = inner.index
                && let Some(key) = key.value
            {
                keys.push(lossy(key));
                below = inner.array;
            }
            if let Expression::Access(Access::Property(property)) = below
                && let ClassLikeMemberSelector::Identifier(name) = &property.property
                && self.names.contains(&lossy(name.value).to_ascii_lowercase())
            {
                keys.reverse();
                let call = SiteCall::Read {
                    object: property.object,
                    name: lossy(name.value),
                    keys,
                    at: below.span().start.offset,
                };
                self.strings(&call, access.index, (Some(0), None), &mut Vec::new(), None);
            }
        }
        // `$build['#attached']['library'][] = 'core/drupal.ajax'`: the keys the target is reached by.
        let Expression::Assignment(assignment) = expr else { return };
        let mut keys: Vec<Option<String>> = Vec::new();
        let mut target = assignment.lhs;
        loop {
            match target {
                Expression::ArrayAppend(append) => {
                    keys.push(None);
                    target = append.array;
                }
                Expression::ArrayAccess(access) => {
                    keys.push(match access.index {
                        Expression::Literal(Literal::String(key)) => key.value.map(lossy),
                        _ => None,
                    });
                    target = access.array;
                }
                _ => break,
            }
        }
        keys.reverse();
        // `$route['books/(:num)'] = 'books/show/$1'`: an entry of the variable asked about
        // (`CalleePattern::Entry("$route")`).
        if let Expression::Variable(mago_syntax::cst::Variable::Direct(variable)) = target
            && keys.len() == 1
        {
            let name = lossy(variable.name);
            if self.names.contains(&name.to_ascii_lowercase()) {
                let call = SiteCall::Entry { key: name, at: assignment.lhs.span().start.offset };
                self.strings(&call, assignment.rhs, (Some(0), None), &mut Vec::new(), None);
                return;
            }
        }
        let Some(at) =
            keys.iter().position(|key| key.as_ref().is_some_and(|key| self.names.contains(&key.to_ascii_lowercase())))
        else {
            return;
        };
        let Some(key) = keys[at].clone() else { return };
        let mut steps: Vec<ArrayStep> = keys[at + 1..].iter().cloned().map(ArrayStep::Value).collect();
        let call = SiteCall::Entry { key, at: assignment.lhs.span().start.offset };
        self.strings(&call, assignment.rhs, (Some(0), None), &mut steps, None);
    }
}

/// `Foo::class`: the name as written and its offset.
fn class_constant(expr: &Expression<'_>) -> Option<(String, u32)> {
    let Expression::Access(Access::ClassConstant(access)) = expr else { return None };
    let ClassLikeConstantSelector::Identifier(constant) = &access.constant else { return None };
    let Expression::Identifier(identifier) = access.class else { return None };
    constant.value.eq_ignore_ascii_case(b"class").then(|| (lossy(identifier.value()), identifier.span().start.offset))
}

impl<'a> Walker<'a, 'a, Collect<'a, '_>> for SiteWalker {
    fn walk_in_class(&self, class: &'a mago_syntax::cst::Class<'a>, cx: &mut Collect<'a, '_>) {
        cx.frames.push(Frame::Class(Some((lossy(class.name.value), class.name.span.start.offset))));
    }

    fn walk_out_class(&self, _: &'a mago_syntax::cst::Class<'a>, cx: &mut Collect<'a, '_>) {
        cx.frames.pop();
    }

    fn walk_in_anonymous_class(&self, _: &'a mago_syntax::cst::AnonymousClass<'a>, cx: &mut Collect<'a, '_>) {
        cx.frames.push(Frame::Class(None));
    }

    fn walk_out_anonymous_class(&self, _: &'a mago_syntax::cst::AnonymousClass<'a>, cx: &mut Collect<'a, '_>) {
        cx.frames.pop();
    }

    fn walk_in_trait(&self, _: &'a mago_syntax::cst::Trait<'a>, cx: &mut Collect<'a, '_>) {
        cx.frames.push(Frame::Class(None));
    }

    fn walk_out_trait(&self, _: &'a mago_syntax::cst::Trait<'a>, cx: &mut Collect<'a, '_>) {
        cx.frames.pop();
    }

    fn walk_in_method(&self, method: &'a mago_syntax::cst::Method<'a>, cx: &mut Collect<'a, '_>) {
        cx.frames.push(Frame::Method(lossy(method.name.value)));
    }

    fn walk_out_method(&self, _: &'a mago_syntax::cst::Method<'a>, cx: &mut Collect<'a, '_>) {
        cx.frames.pop();
    }

    fn walk_in_function(&self, _: &'a mago_syntax::cst::Function<'a>, cx: &mut Collect<'a, '_>) {
        cx.frames.push(Frame::Function);
    }

    fn walk_out_function(&self, _: &'a mago_syntax::cst::Function<'a>, cx: &mut Collect<'a, '_>) {
        cx.frames.pop();
    }

    fn walk_in_closure(&self, _: &'a mago_syntax::cst::Closure<'a>, cx: &mut Collect<'a, '_>) {
        cx.frames.push(Frame::Function);
    }

    fn walk_out_closure(&self, _: &'a mago_syntax::cst::Closure<'a>, cx: &mut Collect<'a, '_>) {
        cx.frames.pop();
    }

    fn walk_in_arrow_function(&self, _: &'a mago_syntax::cst::ArrowFunction<'a>, cx: &mut Collect<'a, '_>) {
        cx.frames.push(Frame::Function);
    }

    fn walk_out_arrow_function(&self, _: &'a mago_syntax::cst::ArrowFunction<'a>, cx: &mut Collect<'a, '_>) {
        cx.frames.pop();
    }

    fn walk_in_return(&self, r#return: &'a mago_syntax::cst::Return<'a>, cx: &mut Collect<'a, '_>) {
        let Some(value) = r#return.value else { return };
        let [.., Frame::Class(Some(class)), Frame::Method(method)] = cx.frames.as_slice() else { return };
        if !cx.names.contains(&method.to_ascii_lowercase()) {
            return;
        }
        let call = SiteCall::Return { class: class.clone(), method: method.clone(), at: r#return.span().start.offset };
        cx.strings(&call, value, (Some(0), None), &mut Vec::new(), None);
    }

    fn walk_in_property_concrete_item(
        &self,
        item: &'a mago_syntax::cst::PropertyConcreteItem<'a>,
        cx: &mut Collect<'a, '_>,
    ) {
        let [.., Frame::Class(Some(class))] = cx.frames.as_slice() else { return };
        let name = lossy(item.variable.name.strip_prefix(b"$").unwrap_or(item.variable.name));
        // (`*`: a pattern about every property of its classes.)
        if !cx.names.contains(&name.to_ascii_lowercase()) && !cx.names.contains("*") {
            return;
        }
        let call = SiteCall::Property { class: class.clone(), name, at: item.variable.span.start.offset };
        cx.strings(&call, item.value, (Some(0), None), &mut Vec::new(), None);
    }

    /// An attribute's arguments are those of its class's constructor.
    fn walk_in_attribute(&self, attribute: &'a mago_syntax::cst::Attribute<'a>, cx: &mut Collect<'a, '_>) {
        let Some(list) = &attribute.argument_list else { return };
        if !cx.names.contains("__construct") {
            return;
        }
        let at = attribute.name.span().start.offset;
        let call = SiteCall::Attribute { class: (lossy(attribute.name.value()), at), at };
        let mut positional = 0;
        for argument in list.arguments.iter() {
            let mut steps = Vec::new();
            match argument {
                mago_syntax::cst::PartialArgument::Positional(argument) => {
                    cx.strings(&call, argument.value, (Some(positional), None), &mut steps, None);
                    positional += 1;
                }
                mago_syntax::cst::PartialArgument::Named(argument) => {
                    let name = lossy(argument.name.value);
                    cx.strings(&call, argument.value, (None, Some(&name)), &mut steps, None);
                }
                _ => {}
            }
        }
    }

    fn walk_in_expression(&self, expr: &'a Expression<'a>, cx: &mut Collect<'a, '_>) {
        cx.entries(expr);
        let Some((name, list)) = call_parts(expr) else { return };
        if !cx.names.contains(&name.to_ascii_lowercase()) {
            return;
        }
        let call = SiteCall::Call(expr);
        let wants_next = cx.next_names.contains(&name.to_ascii_lowercase());
        let positionals: Vec<&'a Expression<'a>> = list
            .arguments
            .iter()
            .filter_map(|argument| match argument {
                Argument::Positional(argument) => Some(argument.value),
                Argument::Named(_) => None,
            })
            .collect();
        let mut positional = 0;
        for argument in list.arguments.iter() {
            let mut steps = Vec::new();
            match argument {
                Argument::Positional(argument) => {
                    cx.next = if wants_next { positionals.get(positional + 1).copied() } else { None };
                    cx.first = if positional > 0 { positionals.first().copied() } else { None };
                    cx.strings(&call, argument.value, (Some(positional), None), &mut steps, None);
                    cx.next = None;
                    cx.first = None;
                    positional += 1;
                }
                Argument::Named(argument) => {
                    let name = lossy(argument.name.value);
                    cx.strings(&call, argument.value, (None, Some(&name)), &mut steps, None);
                }
            }
        }
    }
}

/// The string literal arguments of `parsed` (only the one starting at
/// `want` when given) in calls named like a pattern of `kinds`, with their
/// calls resolved.
pub fn sites(
    parsed: &ParsedFile,
    resolver: &Resolver<'_>,
    kinds: &[&dyn ReferenceKind],
    want: Option<u32>,
) -> Vec<StringSite> {
    sites_with(parsed, resolver, kinds, &[], want)
}

/// [`sites`], with the calls `rules` of `kinds` name too.
pub fn sites_with(
    parsed: &ParsedFile,
    resolver: &Resolver<'_>,
    kinds: &[&dyn ReferenceKind],
    rules: &[CompletionRule],
    want: Option<u32>,
) -> Vec<StringSite> {
    sites_at(None, parsed, resolver, kinds, rules, want)
}

/// [`sites_with`], `parsed` being the file at `path`: a receiver that is a
/// variable a view is passed (`$form->getAttributeLabel('email')`) is typed
/// by what the view is passed.
pub fn sites_at(
    path: Option<&Path>,
    parsed: &ParsedFile,
    resolver: &Resolver<'_>,
    kinds: &[&dyn ReferenceKind],
    rules: &[CompletionRule],
    want: Option<u32>,
) -> Vec<StringSite> {
    let rule_names = |next: bool| {
        rules
            .iter()
            .filter(|rule| kinds.iter().any(|kind| kind.id() == rule.kind && (!next || kind.uses_next_argument())))
            .flat_map(|rule| rule.conditions.iter().flat_map(|condition| condition.call_names()))
            .collect::<Vec<String>>()
    };
    let mut names: HashSet<String> = kinds
        .iter()
        .flat_map(|kind| kind.patterns().iter().flat_map(PlacePattern::names))
        .map(str::to_ascii_lowercase)
        .collect();
    names.extend(rule_names(false));
    let mut next_names: HashSet<String> = kinds
        .iter()
        .filter(|kind| kind.uses_next_argument())
        .flat_map(|kind| kind.patterns().iter().flat_map(PlacePattern::names))
        .map(str::to_ascii_lowercase)
        .collect();
    next_names.extend(rule_names(true));
    let mut collect = Collect {
        want,
        names: &names,
        next_names: &next_names,
        next: None,
        first: None,
        out: Vec::new(),
        frames: Vec::new(),
    };
    SiteWalker.walk_program(parsed.program(), &mut collect);
    if collect.out.is_empty() {
        return Vec::new();
    }
    let file_names = FileNames::resolve(parsed);
    let summary = FileSummary::extract(parsed);
    let types = ExprTypes::new(parsed, &file_names, &summary, resolver).at_path(path);
    let text = parsed.text();
    collect
        .out
        .into_iter()
        .filter_map(|raw| {
            let call = raw.call.start();
            let target = match raw.call {
                SiteCall::Call(expr) => call_target(&types, &file_names, resolver, expr)?,
                SiteCall::Return { class: (name, offset), method, .. } => {
                    let class = match file_names.scope_at(offset).resolve(NameKind::ClassLike, &name).resolution {
                        Resolution::Fqn(fqn) | Resolution::Fallback { namespaced: fqn, .. } => fqn,
                        Resolution::Relative(_) => return None,
                    };
                    CallTarget::Return { class: class.trim_start_matches('\\').to_owned(), method }
                }
                SiteCall::Property { class: (class, offset), name, .. } => {
                    let class = match file_names.scope_at(offset).resolve(NameKind::ClassLike, &class).resolution {
                        Resolution::Fqn(fqn) | Resolution::Fallback { namespaced: fqn, .. } => fqn,
                        Resolution::Relative(_) => return None,
                    };
                    CallTarget::Property { class: class.trim_start_matches('\\').to_owned(), name }
                }
                SiteCall::Entry { key, .. } => CallTarget::Entry { key },
                SiteCall::Read { object, name, keys, .. } => {
                    let frames = types
                        .locate(object.span().start.offset)
                        .expression
                        .map(|(_, frames)| frames)
                        .unwrap_or_default();
                    let classes: Vec<String> = types
                        .type_of(object, &frames)
                        .members()
                        .iter()
                        .filter_map(|member| match member {
                            Type::Named { fqn, .. } => Some(fqn.trim_start_matches('\\').to_owned()),
                            Type::Relative(_) | Type::GenericRelative { .. } => types.context(&frames).self_class,
                            _ => None,
                        })
                        .collect();
                    CallTarget::Read { classes, name, keys }
                }
                SiteCall::Attribute { class: (class, offset), .. } => {
                    let class = match file_names.scope_at(offset).resolve(NameKind::ClassLike, &class).resolution {
                        Resolution::Fqn(fqn) | Resolution::Fallback { namespaced: fqn, .. } => fqn,
                        Resolution::Relative(_) => return None,
                    };
                    CallTarget::Method {
                        classes: vec![class.trim_start_matches('\\').to_owned()],
                        name: "__construct".to_owned(),
                        models: Vec::new(),
                        tables: Vec::new(),
                    }
                }
            };
            let class = raw.class.and_then(|(name, offset)| {
                match file_names.scope_at(offset).resolve(NameKind::ClassLike, &name).resolution {
                    Resolution::Fqn(fqn) | Resolution::Fallback { namespaced: fqn, .. } => {
                        Some(fqn.trim_start_matches('\\').to_owned())
                    }
                    Resolution::Relative(_) => None,
                }
            });
            let next_argument = raw.next.map(|next| next_argument(&types, &file_names, next)).unwrap_or_default();
            let first_argument = raw.first.and_then(|first| first_argument(&file_names, first));
            let start = raw.span.start.offset + 1;
            let end = raw.span.end.offset.saturating_sub(1).max(start);
            Some(StringSite {
                start,
                end,
                value: text.get(start as usize..end as usize).unwrap_or_default().to_owned(),
                target,
                index: raw.index,
                argument_name: raw.argument_name,
                steps: raw.steps,
                class,
                call,
                next_argument,
                first_argument,
            })
        })
        .collect()
}

/// See [`StringSite::first_argument`].
fn first_argument(names: &FileNames, expr: &Expression<'_>) -> Option<String> {
    if let Some((name, offset)) = class_constant(expr) {
        return match names.scope_at(offset).resolve(NameKind::ClassLike, &name).resolution {
            Resolution::Fqn(fqn) | Resolution::Fallback { namespaced: fqn, .. } => {
                Some(fqn.trim_start_matches('\\').to_owned())
            }
            Resolution::Relative(_) => None,
        };
    }
    match expr {
        Expression::Literal(Literal::String(string)) => string.value.map(lossy),
        _ => None,
    }
}

/// See [`StringSite::next_argument`].
fn next_argument<'a>(types: &ExprTypes<'a, '_>, names: &FileNames, expr: &'a Expression<'a>) -> Vec<String> {
    let first = |elements: Vec<&'a ArrayElement<'a>>| match elements.first() {
        Some(ArrayElement::Value(element)) => Some(element.value),
        _ => None,
    };
    let expr = match expr {
        Expression::Array(array) => first(array.elements.iter().collect()),
        Expression::LegacyArray(array) => first(array.elements.iter().collect()),
        _ => Some(expr),
    };
    let Some(expr) = expr else { return Vec::new() };
    if let Some((name, offset)) = class_constant(expr) {
        return match names.scope_at(offset).resolve(NameKind::ClassLike, &name).resolution {
            Resolution::Fqn(fqn) | Resolution::Fallback { namespaced: fqn, .. } => {
                vec![fqn.trim_start_matches('\\').to_owned()]
            }
            Resolution::Relative(_) => Vec::new(),
        };
    }
    if let Expression::Literal(Literal::String(string)) = expr {
        return string.value.map(lossy).into_iter().collect();
    }
    let frames = types.locate(expr.span().start.offset).expression.map(|(_, frames)| frames).unwrap_or_default();
    let mut out: Vec<String> = Vec::new();
    for member in types.type_of(expr, &frames).members() {
        if let Type::Named { fqn, .. } = member {
            let fqn = fqn.trim_start_matches('\\').to_owned();
            if !out.contains(&fqn) {
                out.push(fqn);
            }
        }
    }
    out
}

fn call_target<'a>(
    types: &ExprTypes<'a, '_>,
    names: &FileNames,
    resolver: &Resolver<'_>,
    call: &'a Expression<'a>,
) -> Option<CallTarget> {
    if let Expression::Call(Call::Function(function)) = call {
        let Expression::Identifier(identifier) = function.function else { return None };
        let name = lossy(identifier.value());
        let resolved = names.scope_at(identifier.span().start.offset).resolve(NameKind::Function, &name);
        return Some(CallTarget::Function(resolved.resolution.candidates().map(str::to_owned).collect()));
    }
    let frames = types.locate(call.span().start.offset).expression.map(|(_, frames)| frames).unwrap_or_default();
    // `$this->extend('layout')` outside any class: a template, whose `$this` is the object
    // rendering it.
    if let Expression::Call(Call::Method(method)) = call
        && matches!(method.object, Expression::Variable(mago_syntax::cst::Variable::Direct(this)) if this.name == b"$this")
        && types.context(&frames).self_class.is_none()
        && let Some(name) = member_name(&method.method)
    {
        return Some(CallTarget::Method {
            classes: vec![TEMPLATE_THIS.to_owned()],
            name,
            models: Vec::new(),
            tables: Vec::new(),
        });
    }
    let Callee::Method { receiver, name } = crate::signature::callee(types, names, resolver, call, &frames)? else {
        return None;
    };
    let mut classes: Vec<String> = receiver
        .members()
        .iter()
        .filter_map(|member| match member {
            Type::Named { fqn, .. } => Some(fqn.trim_start_matches('\\').to_owned()),
            // `$this`, `static`, `self`: the class the call is in.
            Type::Relative(_) | Type::GenericRelative { .. } => types.context(&frames).self_class,
            _ => None,
        })
        .collect();
    let object = match call {
        Expression::Call(Call::Method(method)) => Some(method.object),
        Expression::Call(Call::NullSafeMethod(method)) => Some(method.object),
        _ => None,
    };
    // (Laravel's helpers: the classes pushed are only in the patterns of Laravel's kinds,
    // which are asked in a Laravel project only.)
    if let Some(class) = object.and_then(helper_instance) {
        classes.push(class.to_owned());
    }
    let mut models: Vec<String> = Vec::new();
    let mut tables: Vec<String> = Vec::new();
    for member in receiver.members() {
        let Type::Named { args, .. } = member else { continue };
        for arg in args {
            if let Type::LiteralString(table) = arg
                && !tables.contains(table)
            {
                tables.push(table.clone());
            }
            if let Type::Named { fqn, .. } = arg
                && let Some(model) = resolver.class_model(fqn)
                // (An Eloquent model, or the entity of a CakePHP query.)
                && (model.is_subclass_of(super::laravel::refs::eloquent::MODEL)
                    || model.is_subclass_of(super::cakephp::classes::ENTITY_INTERFACE))
                && !models.iter().any(|known| known == model.fqn())
            {
                models.push(model.fqn().to_owned());
            }
        }
    }
    Some(CallTarget::Method { classes, name, models, tables })
}

/// `config()`: the service a helper returns without arguments.
fn helper_instance(object: &Expression<'_>) -> Option<&'static str> {
    let Expression::Call(Call::Function(call)) = object else { return None };
    let Expression::Identifier(identifier) = call.function else { return None };
    if !call.argument_list.arguments.is_empty() {
        return None;
    }
    let name = identifier.value();
    let name = name.strip_prefix(b"\\").unwrap_or(name);
    HELPER_INSTANCES.iter().find(|(helper, _)| helper.as_bytes().eq_ignore_ascii_case(name)).map(|(_, class)| *class)
}

// ------------------------------------------------------------------ kinds

/// What [`ReferenceKind`] methods get.
#[derive(Clone)]
pub struct RefContext<'e> {
    pub engine: &'e PhpEngine,
    /// The frameworks of the project: the ones whose kinds are asked.
    pub frameworks: Frameworks,
    /// The Laravel project model, in a Laravel project.
    pub laravel: Option<Arc<LaravelProject>>,
    /// The string's site, when the question is about one string.
    pub site: Option<SiteInfo>,
}

/// Where a string sits; see the module documentation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SiteInfo {
    pub path: PathBuf,
    /// The byte its content starts at.
    pub start: u32,
    /// The class the string is about.
    pub class: Option<String>,
    /// The call the string is an argument of (or the method returning it)
    /// and the byte it starts at ([`StringSite::call`]).
    pub call: Option<(CallTarget, u32)>,
    /// The key of the string's entry in the argument's array, when it sits
    /// in one (`'title'` for `['title' => 'required']`).
    pub key: Option<String>,
    /// The string keys of the arrays the string sits in, from the argument
    /// down: `Comments` for `'Users'` in `['Comments' => ['Users']]`.
    pub keys: Vec<String>,
    /// [`StringSite::next_argument`].
    pub next_argument: Vec<String>,
    /// [`StringSite::first_argument`].
    pub first_argument: Option<String>,
    /// The `ide.json` rule the string was matched by, when no built-in
    /// pattern matched it.
    pub rule: Option<CompletionRule>,
}

impl RefContext<'_> {
    /// The Laravel project model, for Laravel's kinds.
    ///
    /// # Panics
    ///
    /// Outside a Laravel project. A kind is only asked in a project of its
    /// framework ([`kinds_for`]), so a Laravel kind always has it.
    pub fn project(&self) -> &Arc<LaravelProject> {
        self.laravel.as_ref().expect("a Laravel reference kind is asked in a Laravel project only")
    }

    /// The completion rules of the project's `ide.json` files (Laravel
    /// projects have them).
    pub fn rules(&self) -> &[CompletionRule] {
        self.laravel.as_ref().map(|project| project.ide_json.completions.as_slice()).unwrap_or_default()
    }

    /// The class of the string's site.
    pub fn class(&self) -> Option<&str> {
        self.site.as_ref()?.class.as_deref()
    }
}

/// A completion item of a kind.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RefItem {
    pub label: String,
    /// The text inserted (usually the label).
    pub insert: String,
    pub detail: Option<String>,
    /// The byte range of the value the item replaces: from `from` to the
    /// caret, and to `to` on Tab.
    pub from: usize,
    pub to: usize,
}

/// A part of a value that can be renamed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenameTarget {
    /// The value usages must have (or start with, see
    /// [`ReferenceKind::renames_with`]).
    pub key: String,
    /// The byte range of `key` that the new name replaces.
    pub range: (usize, usize),
    /// The declarations renamed with it (every locale's key, both `.env`
    /// files): their files and the ranges replaced.
    pub declarations: Vec<(PathBuf, Range)>,
}

/// A kind of string reference; see the module documentation.
pub trait ReferenceKind: Send + Sync {
    /// The kind's id (`configKey`).
    fn id(&self) -> &'static str;

    /// The places strings of this kind sit in.
    fn patterns(&self) -> &'static [PlacePattern];

    /// Completion items for the string `value` with the caret at its byte
    /// `at` (the text before it is what was typed).
    fn complete(&self, cx: &RefContext<'_>, value: &str, at: usize) -> Vec<RefItem>;

    /// What `value` names, the caret at byte `at` of it (a kind with
    /// segments may resolve the segment at the caret).
    fn resolve(&self, cx: &RefContext<'_>, value: &str, at: usize) -> Vec<Target>;

    /// Words (lower case) every file holding a usage of `value` (a
    /// canonical value) has, for the word index.
    fn usage_words(&self, value: &str) -> Vec<String> {
        crate::index::words(value)
    }

    /// Text every file holding a usage of `value` (a canonical value)
    /// contains.
    fn usage_text<'v>(&self, value: &'v str) -> &'v str {
        value
    }

    /// The value a string written `value` at the context's site stands
    /// for, the same for every usage of one thing (the class and method of
    /// a controller action).
    fn canonical(&self, _cx: &RefContext<'_>, value: &str) -> String {
        value.to_owned()
    }

    /// Whether a usage written `value` is one of `key`.
    fn refers_to(&self, value: &str, key: &str) -> bool {
        value == key
    }

    /// The value a declaration at byte `offset` of `path` declares (the
    /// key of a configuration array), with the byte range of its literal's
    /// content and the caret's byte in the value.
    fn declared_at(&self, _cx: &RefContext<'_>, _path: &Path, _parsed: &ParsedFile, _offset: u32) -> Option<Declared> {
        None
    }

    /// What renaming at byte `at` of `value` changes; `None` when it can't
    /// be renamed there.
    fn rename_target(&self, _cx: &RefContext<'_>, _value: &str, _at: usize) -> Option<RenameTarget> {
        None
    }

    /// Whether a usage written `value` changes with a rename of `key`
    /// (`app` when `app.name` is renamed: keys below it change too).
    fn renames_with(&self, value: &str, key: &str) -> bool {
        value == key
    }

    /// The files a rename of `key` moves, `new_value` being `key` with its
    /// rename range replaced (a view's file).
    fn rename_files(&self, _cx: &RefContext<'_>, _key: &str, _new_value: &str) -> Vec<FileEdit> {
        Vec::new()
    }

    /// Whether `name` can be the new name of a rename.
    fn valid_name(&self, name: &str) -> bool {
        !name.is_empty() && !name.contains(['\'', '"', '\\'])
    }

    /// Whether `name` can be the new name of a rename of `key` (by default
    /// [`valid_name`](Self::valid_name)).
    fn valid_name_for(&self, _key: &str, name: &str) -> bool {
        self.valid_name(name)
    }

    /// Whether `name` can replace the part `target` renames, knowing what
    /// that part is (the last part of a nested translation key can't get a
    /// dot). Asked after [`valid_name_for`](Self::valid_name_for).
    fn valid_rename(&self, _target: &RenameTarget, _name: &str) -> bool {
        true
    }

    /// The bytes of a usage written `value` that a rename of `target`
    /// replaces: by default the same as in the key. A kind whose usages
    /// write the key with something around it (a slash before a REST
    /// route's path) says where it is.
    fn site_rename_range(&self, _value: &str, target: &RenameTarget) -> (usize, usize) {
        target.range
    }

    /// The ranges a rename of `target` replaces in the files of
    /// [`other_usages`](Self::other_usages): by default the renamed part of
    /// each (on one line).
    fn other_renames(&self, cx: &RefContext<'_>, target: &RenameTarget) -> Vec<(PathBuf, Range)> {
        let part = |range: Range| {
            let column = |offset: usize| Pos::new(range.start.line, range.start.col + offset as u32);
            Range::new(column(target.range.0), column(target.range.1))
        };
        self.other_usages(cx, &target.key).into_iter().map(|(path, range)| (path, part(range))).collect()
    }

    /// Usages of `key` (a canonical value) that aren't string literals in
    /// PHP (`<livewire:counter />` tags), as byte ranges of the value in
    /// `text`, the text of the file at `path`.
    fn markup_usages(&self, _cx: &RefContext<'_>, _path: &Path, _text: &str, _key: &str) -> Vec<(u32, u32)> {
        Vec::new()
    }

    /// Usages of `key` (a canonical value) in files the word index doesn't
    /// have (a Twig template naming a route): the file and the range of
    /// the value there. A rename replaces the part of each that it renames.
    fn other_usages(&self, _cx: &RefContext<'_>, _key: &str) -> Vec<(PathBuf, Range)> {
        Vec::new()
    }

    /// Words (lower case) a file holding a [`markup_usages`](Self::markup_usages)
    /// usage of `key` has; empty when the kind has none.
    fn markup_words(&self, _key: &str) -> Vec<String> {
        Vec::new()
    }

    /// Whether `value` names something that exists (the "unknown …"
    /// inspection).
    fn is_known(&self, cx: &RefContext<'_>, value: &str) -> bool;

    /// Whether the kind looks at the argument after the string's
    /// ([`SiteInfo::next_argument`]).
    fn uses_next_argument(&self) -> bool {
        false
    }

    /// Whether completion offers the kind's values where the string would
    /// be written too, before a quote is typed (`path(|)` inserts
    /// `'home'`).
    fn completes_unquoted(&self) -> bool {
        false
    }

    /// Markdown describing what a usage written `value` names (hover).
    fn hover(&self, _cx: &RefContext<'_>, _value: &str) -> Option<String> {
        None
    }

    /// Whether [`undefined`](Self::undefined) is also asked of a value with
    /// backslashes (a class's name, a namespaced view).
    fn checks_backslashes(&self) -> bool {
        false
    }

    /// What is wrong with a usage written `value` (a plain literal) that
    /// names nothing; `None` when it is fine, or when that can't be told
    /// (a package's configuration, say).
    fn undefined(&self, _cx: &RefContext<'_>, _value: &str) -> Option<Undefined> {
        None
    }
}

/// A usage naming nothing; see [`ReferenceKind::undefined`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Undefined {
    pub message: String,
    pub severity: Severity,
    /// The quick-fixes, the preferred one first.
    pub fixes: Vec<UndefinedFix>,
}

/// A quick-fix of an [`Undefined`] usage.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UndefinedFix {
    pub title: String,
    pub edit: WorkspaceEdit,
    /// Where the editor opens after applying it (a created class at its
    /// new method).
    pub open: Option<Location>,
}

impl UndefinedFix {
    pub fn new(title: impl Into<String>, edit: WorkspaceEdit) -> Self {
        Self { title: title.into(), edit, open: None }
    }

    pub fn opening(mut self, open: Option<Location>) -> Self {
        self.open = open;
        self
    }
}

impl Undefined {
    pub fn warning(message: impl Into<String>) -> Self {
        Self { message: message.into(), severity: Severity::Warning, fixes: Vec::new() }
    }

    pub fn with_fix(self, title: impl Into<String>, edit: WorkspaceEdit) -> Self {
        self.with(UndefinedFix::new(title, edit))
    }

    pub fn with(mut self, fix: UndefinedFix) -> Self {
        self.fixes.push(fix);
        self
    }
}

/// An undefined usage found in a file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UndefinedSite {
    /// The kind's id.
    pub kind: &'static str,
    /// The literal's content (bytes of the parse).
    pub start: u32,
    pub end: u32,
    pub problem: Undefined,
}

/// A declaration found by [`ReferenceKind::declared_at`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Declared {
    pub value: String,
    pub start: u32,
    pub end: u32,
    pub at: usize,
}

/// The kinds of the frameworks in `frameworks`, each framework's in its own
/// order.
pub fn kinds_for(frameworks: Frameworks) -> Vec<&'static dyn ReferenceKind> {
    let mut kinds: Vec<&'static dyn ReferenceKind> = Vec::new();
    for framework in frameworks.iter() {
        kinds.extend_from_slice(framework_kinds(framework));
    }
    kinds
}

/// The kinds of one framework.
pub fn framework_kinds(framework: super::Framework) -> &'static [&'static dyn ReferenceKind] {
    match framework {
        super::Framework::Laravel => super::laravel::refs::kinds(),
        super::Framework::Symfony => super::symfony::refs::kinds(),
        super::Framework::WordPress => super::wordpress::refs::kinds(),
        super::Framework::Drupal => super::drupal::refs::kinds(),
        super::Framework::CodeIgniter => super::codeigniter::refs::kinds(),
        super::Framework::Yii => super::yii::refs::kinds(),
        super::Framework::CakePhp => super::cakephp::refs::kinds(),
        _ => &[],
    }
}

/// The kind among `kinds` whose patterns `site` matches.
pub fn kind_in(
    kinds: &[&'static dyn ReferenceKind],
    site: &StringSite,
    resolver: &Resolver<'_>,
) -> Option<&'static dyn ReferenceKind> {
    kinds.iter().copied().find(|kind| kind.patterns().iter().any(|pattern| pattern.matches(site, resolver)))
}

/// The kind among `kinds` with id `id`.
pub fn kind_named(kinds: &[&'static dyn ReferenceKind], id: &str) -> Option<&'static dyn ReferenceKind> {
    kinds.iter().copied().find(|kind| kind.id() == id)
}

/// The kind of `site` among `kinds`: the one whose patterns match it, else
/// the one of the first of `rules` matching it, with that rule.
pub fn classify_in<'r>(
    kinds: &[&'static dyn ReferenceKind],
    site: &StringSite,
    resolver: &Resolver<'_>,
    rules: &'r [CompletionRule],
) -> Option<(&'static dyn ReferenceKind, Option<&'r CompletionRule>)> {
    if let Some(kind) = kind_in(kinds, site, resolver) {
        return Some((kind, None));
    }
    rules
        .iter()
        .find(|rule| rules::rule_matches(rule, site, resolver))
        .and_then(|rule| Some((kind_named(kinds, &rule.kind)?, Some(rule))))
}

/// The value `site` stands for: its content, after the `prefix` option of
/// the rule matching it.
pub(crate) fn prefixed(site: &StringSite, rule: Option<&CompletionRule>) -> String {
    match rule {
        Some(rule) => format!("{}{}", rule.options.prefix, site.value),
        None => site.value.clone(),
    }
}

// ------------------------------------------------------------------ caret

/// What the caret is on.
pub struct Subject {
    pub kind: &'static dyn ReferenceKind,
    /// The value (a declaration's full key).
    pub value: String,
    /// The caret's byte in `value`.
    pub at: usize,
    /// The byte range of the literal's content in the file.
    pub start: u32,
    pub end: u32,
    /// The caret is on a declaration, not a usage.
    pub declaration: bool,
    /// The length of the text put before the literal's content in `value`
    /// (an `ide.json` rule's `prefix`).
    pub prefix: usize,
}

/// The string literal around `offset` (inside its quotes): the offset of
/// its opening quote, and of the end of its content (the closing quote, or
/// the end of the line while it isn't closed) and whether it is closed.
fn literal_around(text: &str, offset: usize) -> Option<(usize, usize, bool)> {
    let bytes = text.as_bytes();
    let quote = crate::complete::string_start(bytes, offset)?;
    let mut end = offset;
    while end < bytes.len() && bytes[end] != bytes[quote] && bytes[end] != b'\n' {
        if bytes[end] == b'\\' {
            end += 1;
        }
        end += 1;
    }
    let end = end.min(bytes.len());
    let closed = bytes.get(end) == Some(&bytes[quote]);
    let content_end = if closed { end } else { text[..end].trim_end().len().max(offset) };
    Some((quote, content_end, closed))
}

/// Of the sites one string is (an argument of a call, and the value under a key
/// of an array in it), the first a pattern of `kinds` takes; else the first.
fn placed(sites: Vec<StringSite>, kinds: &[&'static dyn ReferenceKind], resolver: &Resolver<'_>) -> Option<StringSite> {
    let taken = |site: &StringSite| {
        kinds.iter().any(|kind| kind.patterns().iter().any(|pattern| pattern.matches(site, resolver)))
    };
    let at = sites.iter().position(taken).unwrap_or(0);
    sites.into_iter().nth(at)
}

/// The string site of one of `kinds` at `offset` of `parsed`, in its own
/// parse or in a probe with the literal closed; the calls of `rules` count
/// too.
pub fn site_at_in(
    kinds: &[&'static dyn ReferenceKind],
    parsed: &ParsedFile,
    resolver: &Resolver<'_>,
    rules: &[CompletionRule],
    offset: u32,
) -> Option<StringSite> {
    site_at_in_file(None, kinds, parsed, resolver, rules, offset)
}

/// [`site_at_in`], `parsed` being the file at `path` ([`sites_at`]).
pub fn site_at_in_file(
    path: Option<&Path>,
    kinds: &[&'static dyn ReferenceKind],
    parsed: &ParsedFile,
    resolver: &Resolver<'_>,
    rules: &[CompletionRule],
    offset: u32,
) -> Option<StringSite> {
    let text = parsed.text();
    let (quote, content_end, closed) = literal_around(text, offset as usize)?;
    if let Some(site) = placed(sites_at(path, parsed, resolver, kinds, rules, Some(quote as u32)), kinds, resolver) {
        return Some(site);
    }
    // Not a literal of the parse (being typed): a probe string in its place.
    let after = if closed { content_end + 1 } else { content_end };
    let rest = &text[after..];
    let mut tries = vec![String::new()];
    if !closed {
        tries.push(")".to_owned());
    }
    for close in tries {
        let terminator = if crate::complete::needs_terminator(rest) { ";" } else { "" };
        let probe_value = "x".repeat(content_end - quote - 1);
        let probe =
            ParsedFile::parse("reference.php", format!("{}'{probe_value}'{close}{terminator}{rest}", &text[..quote]));
        if let Some(mut site) =
            placed(sites_at(path, &probe, resolver, kinds, rules, Some(quote as u32)), kinds, resolver)
        {
            site.start = quote as u32 + 1;
            site.end = content_end as u32;
            site.value = text[quote + 1..content_end].to_owned();
            return Some(site);
        }
    }
    None
}

impl PhpEngine {
    /// The reference context, when one of the project's frameworks has
    /// reference kinds.
    pub(crate) fn ref_context(&self) -> Option<RefContext<'_>> {
        let frameworks = self.frameworks();
        if !frameworks.iter().any(|framework| !framework_kinds(framework).is_empty()) {
            return None;
        }
        Some(RefContext { engine: self, frameworks, laravel: self.laravel(), site: None })
    }

    /// Where class-like `fqn` is declared.
    pub(crate) fn class_targets(&self, fqn: &str) -> Vec<Target> {
        self.declarations_of(NameKind::ClassLike, fqn, Path::new(""), &FileSummary::default())
    }

    /// The reference context of `site` in the file at `path`.
    pub(crate) fn site_context(&self, path: &Path, site: &StringSite) -> Option<RefContext<'_>> {
        self.rule_site_context(path, site, None)
    }

    /// [`site_context`](Self::site_context) of a site `rule` matched.
    pub(crate) fn rule_site_context(
        &self,
        path: &Path,
        site: &StringSite,
        rule: Option<&CompletionRule>,
    ) -> Option<RefContext<'_>> {
        let mut cx = self.ref_context()?;
        let class = site.class.clone().or_else(|| {
            let routes = self.laravel_routes()?;
            routes.route_at(path, site.start).and_then(|route| route.action.class()).map(str::to_owned)
        });
        let key = match site.steps.first() {
            Some(ArrayStep::Value(key)) => key.clone(),
            _ => None,
        };
        cx.site = Some(SiteInfo {
            path: path.to_path_buf(),
            start: site.start,
            class,
            call: Some((site.target.clone(), site.call)),
            key,
            keys: site
                .steps
                .iter()
                .filter_map(|step| match step {
                    ArrayStep::Value(Some(key)) => Some(key.clone()),
                    _ => None,
                })
                .collect(),
            next_argument: site.next_argument.clone(),
            first_argument: site.first_argument.clone(),
            rule: rule.cloned(),
        });
        Some(cx)
    }

    /// What the caret at `offset` of `parsed` (the file at `path`) is on.
    pub(crate) fn ref_subject(
        &self,
        parsed: &ParsedFile,
        path: &Path,
        offset: u32,
    ) -> Option<(RefContext<'_>, Subject)> {
        // Cheap checks first: inside a string at all.
        literal_around(parsed.text(), offset as usize)?;
        let cx = self.ref_context()?;
        let resolver = self.resolver();
        let kinds = kinds_for(cx.frameworks);
        let rules = cx.rules();
        if let Some(site) = site_at_in_file(Some(path), &kinds, parsed, &resolver, rules, offset)
            && let Some((kind, rule)) = classify_in(&kinds, &site, &resolver, rules)
        {
            let site_cx = self.rule_site_context(path, &site, rule)?;
            let prefix = rule.map_or(0, |rule| rule.options.prefix.len());
            let at = prefix + (offset.clamp(site.start, site.end) - site.start) as usize;
            let value = prefixed(&site, rule);
            let subject = Subject { kind, value, at, start: site.start, end: site.end, declaration: false, prefix };
            return Some((site_cx, subject));
        }
        let declared = kinds.iter().find_map(|kind| Some((*kind, kind.declared_at(&cx, path, parsed, offset)?)));
        let (kind, declared) = declared?;
        let subject = Subject {
            kind,
            value: declared.value,
            at: declared.at,
            start: declared.start,
            end: declared.end,
            declaration: true,
            prefix: 0,
        };
        Some((cx, subject))
    }

    /// What completion offers for argument `index` of `callee` when no
    /// string is written there yet: the values of the kind whose string
    /// that argument is, for the kinds that complete without quotes
    /// ([`ReferenceKind::completes_unquoted`]). `self_class` is the class
    /// the call is in.
    pub(crate) fn unquoted_ref_items(
        &self,
        callee: &Callee,
        index: usize,
        self_class: Option<&str>,
        resolver: &Resolver<'_>,
    ) -> Vec<RefItem> {
        let Some(cx) = self.ref_context() else { return Vec::new() };
        let kinds: Vec<&'static dyn ReferenceKind> =
            kinds_for(cx.frameworks).into_iter().filter(|kind| kind.completes_unquoted()).collect();
        if kinds.is_empty() {
            return Vec::new();
        }
        let target = match callee {
            Callee::Function(fqn) => CallTarget::Function(vec![fqn.trim_start_matches('\\').to_owned()]),
            Callee::Method { receiver, name } => {
                let classes = receiver
                    .members()
                    .iter()
                    .filter_map(|member| match member {
                        Type::Named { fqn, .. } => Some(fqn.trim_start_matches('\\').to_owned()),
                        Type::Relative(_) | Type::GenericRelative { .. } => self_class.map(str::to_owned),
                        _ => None,
                    })
                    .collect();
                CallTarget::Method { classes, name: name.clone(), models: Vec::new(), tables: Vec::new() }
            }
        };
        // The string that would be written there.
        let site = StringSite {
            start: 0,
            end: 0,
            value: String::new(),
            target,
            index: Some(index),
            argument_name: None,
            steps: Vec::new(),
            class: None,
            call: 0,
            next_argument: Vec::new(),
            first_argument: None,
        };
        match kind_in(&kinds, &site, resolver) {
            Some(kind) => kind.complete(&cx, "", 0),
            None => Vec::new(),
        }
    }

    /// Completion inside a string reference; `None` when the caret isn't in
    /// one.
    pub(crate) fn string_ref_completion(&self, parsed: &ParsedFile, path: &Path, offset: u32) -> Option<Completions> {
        let (cx, subject) = self.ref_subject(parsed, path, offset)?;
        if subject.declaration {
            return None;
        }
        let lines = parsed.line_index();
        let typed = &subject.value[..subject.at];
        let mut items = Vec::new();
        let mut seen = HashSet::new();
        for item in subject.kind.complete(&cx, &subject.value, subject.at) {
            // An item replacing the rule's prefix can't be inserted.
            if item.from < subject.prefix {
                continue;
            }
            let prefix = typed.get(item.from..).unwrap_or_default();
            let Some(quality) = crate::complete::quality(&item.label, prefix) else { continue };
            if !seen.insert(item.label.clone()) {
                continue;
            }
            let from = subject.start + (item.from - subject.prefix) as u32;
            let to = subject.start + (item.to.max(subject.at) - subject.prefix) as u32;
            items.push(Completion {
                label: item.label.clone(),
                label_detail: None,
                detail: item.detail,
                kind: CompletionKind::Value,
                insert_text: item.insert,
                is_snippet: false,
                filter_text: None,
                sort_text: format!("0{quality}{}", item.label.to_lowercase()),
                replace: lines.range(from, offset),
                replace_tab: (to > offset).then(|| lines.range(from, to)),
                additional_edits: Vec::new(),
                deprecated: false,
                data: None,
            });
        }
        items.sort_by(|a, b| a.sort_text.cmp(&b.sort_text));
        // The segment being completed changes with every `.`: ask again.
        Some(Completions { items, incomplete: true })
    }

    /// Hover over a string reference; `None` when the caret isn't on one (or
    /// its kind has nothing to say).
    pub(crate) fn string_ref_hover(
        &self,
        parsed: &ParsedFile,
        path: &Path,
        offset: u32,
    ) -> Option<crate::navigate::HoverInfo> {
        let (cx, subject) = self.ref_subject(parsed, path, offset)?;
        if subject.declaration {
            return None;
        }
        let markdown = subject.kind.hover(&cx, &subject.value)?;
        Some(crate::navigate::HoverInfo { markdown, range: parsed.line_index().range(subject.start, subject.end) })
    }

    /// The usages of `kinds` in `parsed` (the file at `path`) that name
    /// nothing, plain literals only (no interpolation or escapes).
    pub(crate) fn undefined_references(
        &self,
        parsed: &ParsedFile,
        path: &Path,
        kinds: &[&'static dyn ReferenceKind],
        cancel: &CancellationToken,
    ) -> Vec<UndefinedSite> {
        let Some(cx) = self.ref_context() else { return Vec::new() };
        let resolver = self.resolver();
        let rules = cx.rules();
        // A site is of the first kind of the project matching it, asked for or not.
        let known = kinds_for(cx.frameworks);
        let classify = |site: &StringSite| classify_in(&known, site, &resolver, rules);
        let mut out = Vec::new();
        // The parameters of `can:` and `auth:` middleware are abilities and guards.
        let parameters = kinds.iter().any(|kind| kind.id() == Gate.id() || kind.id() == AuthGuard.id());
        let mut walked = kinds.to_vec();
        if parameters {
            walked.push(&Middleware);
        }
        let sites = sites_at(Some(path), parsed, &resolver, &walked, rules, None);
        // Configuration keys the file sets (`config(['themes.shop.narrow' =>
        // …])` in Bagisto's tests): setting a new key is fine, and so is
        // reading it, or a key below it, afterwards.
        let written: Vec<String> = if kinds.iter().any(|kind| kind.id() == ConfigKey.id()) {
            sites
                .iter()
                .filter(|site| writes_config_key(site))
                .filter(|site| classify(site).is_some_and(|(kind, _)| kind.id() == ConfigKey.id()))
                .map(|site| site.value.clone())
                .collect()
        } else {
            Vec::new()
        };
        let set_here = |value: &str| {
            written.iter().any(|key| {
                value.strip_prefix(key.as_str()).is_some_and(|rest| rest.is_empty() || rest.starts_with('.'))
            })
        };
        for site in sites {
            if cancel.is_cancelled() {
                break;
            }
            if site.value.is_empty() || site.value.contains(['$', '{']) {
                continue;
            }
            let Some((kind, rule)) = classify(&site) else { continue };
            // A backslash may be an escape: only a kind that names classes asks, of a
            // single-quoted string without one.
            if site.value.contains('\\') {
                let quote = parsed.text().as_bytes().get((site.start as usize).wrapping_sub(1)).copied();
                let literal = quote == Some(b'\'') && !site.value.contains("\\\\") && !site.value.contains("\\'");
                if !literal || !kind.checks_backslashes() {
                    continue;
                }
            }
            if kind.id() == ConfigKey.id() && set_here(&site.value) {
                continue;
            }
            if parameters && kind.id() == Middleware.id() {
                let Some(cx) = self.rule_site_context(path, &site, rule) else { continue };
                for (id, from, to, problem) in middleware::parameter_problems(&cx, &site.value) {
                    if kinds.iter().any(|wanted| wanted.id() == id) {
                        let (start, end) = (site.start + from as u32, site.start + to as u32);
                        out.push(UndefinedSite { kind: id, start, end, problem });
                    }
                }
                continue;
            }
            if !kinds.iter().any(|wanted| wanted.id() == kind.id()) {
                continue;
            }
            let Some(cx) = self.rule_site_context(path, &site, rule) else { continue };
            if let Some(problem) = kind.undefined(&cx, &prefixed(&site, rule)) {
                out.push(UndefinedSite { kind: kind.id(), start: site.start, end: site.end, problem });
            }
        }
        out
    }
}

/// True when `site` sets a configuration key rather than reading it: a key
/// of the array given to `config([...])` or `Config::set([...])`, or the
/// key of `Config::set('key', …)`.
fn writes_config_key(site: &StringSite) -> bool {
    match &site.target {
        CallTarget::Function(_) => matches!(site.steps.as_slice(), [ArrayStep::Key]),
        CallTarget::Method { name, .. } => name.eq_ignore_ascii_case("set"),
        _ => false,
    }
}

impl PhpEngine {
    /// Goto from a string reference; `None` when the caret isn't on one (or
    /// is on a declaration).
    pub(crate) fn string_ref_definition(&self, parsed: &ParsedFile, path: &Path, offset: u32) -> Option<Vec<Target>> {
        let (cx, subject) = self.ref_subject(parsed, path, offset)?;
        if subject.declaration {
            return None;
        }
        Some(subject.kind.resolve(&cx, &subject.value, subject.at))
    }

    /// The usages of the string reference at the caret.
    pub(crate) fn string_ref_references(
        &self,
        parsed: &ParsedFile,
        path: &Path,
        offset: u32,
        include_declaration: bool,
        cancel: &CancellationToken,
    ) -> Option<IdeResult<Vec<Usage>>> {
        let (cx, subject) = self.ref_subject(parsed, path, offset)?;
        let key = if subject.declaration { subject.value.clone() } else { subject.kind.canonical(&cx, &subject.value) };
        Some(self.kind_usages(&cx, subject.kind, &subject.value, &key, include_declaration, cancel))
    }

    /// The usages of `key` (a canonical value) of `kind`, sorted, with the
    /// declarations of `value` (what the caret's string says) first when
    /// `include_declaration`.
    pub(crate) fn kind_usages(
        &self,
        cx: &RefContext<'_>,
        kind: &'static dyn ReferenceKind,
        value: &str,
        key: &str,
        include_declaration: bool,
        cancel: &CancellationToken,
    ) -> IdeResult<Vec<Usage>> {
        let sites = self.string_usages(kind, key, &|found| kind.refers_to(found, key), cancel)?;
        let mut usages: Vec<Usage> = Vec::new();
        if include_declaration {
            for target in kind.resolve(cx, value, value.len()) {
                usages.push(Usage { path: target.path, range: target.range, kind: UsageKind::Declaration });
            }
        }
        for site in sites {
            usages.push(Usage { path: site.path, range: site.range, kind: UsageKind::String });
        }
        for (path, range) in kind.other_usages(cx, key) {
            usages.push(Usage { path, range, kind: UsageKind::String });
        }
        // A declaration that is a string site too (a hook's firing call) is listed once, as the declaration.
        usages.sort_by(|a, b| (&a.path, a.range.start).cmp(&(&b.path, b.range.start)));
        usages.dedup_by(|b, a| a.path == b.path && a.range == b.range);
        Ok(usages)
    }

    /// The range a rename at the caret changes, when the caret is on a
    /// string reference: `Some(None)` when it can't be renamed.
    pub(crate) fn string_ref_prepare_rename(
        &self,
        parsed: &ParsedFile,
        path: &Path,
        offset: u32,
    ) -> Option<Option<Range>> {
        let (cx, subject) = self.ref_subject(parsed, path, offset)?;
        if subject.prefix > 0 {
            return Some(None);
        }
        let lines = parsed.line_index();
        if let Some((_, (from, to))) = self.discovered_livewire(&cx, &subject) {
            return Some(Some(lines.range(subject.start + from as u32, subject.start + to as u32)));
        }
        Some(subject.kind.rename_target(&cx, &subject.value, subject.at).map(|target| {
            if subject.declaration {
                lines.range(subject.start, subject.end)
            } else {
                let (from, to) = subject.kind.site_rename_range(&subject.value, &target);
                lines.range(subject.start + from as u32, subject.start + to as u32)
            }
        }))
    }

    /// Renames the string reference at the caret: its declaration and every
    /// usage.
    pub(crate) fn string_ref_rename(
        &self,
        parsed: &ParsedFile,
        path: &Path,
        offset: u32,
        new_name: &str,
        cancel: &CancellationToken,
    ) -> Option<IdeResult<WorkspaceEdit>> {
        let (cx, subject) = self.ref_subject(parsed, path, offset)?;
        if let Some((component, _)) = self.discovered_livewire(&cx, &subject) {
            return Some(self.rename_discovered_livewire(&component, new_name, cancel));
        }
        let kind = subject.kind;
        let target = if subject.prefix > 0 { None } else { kind.rename_target(&cx, &subject.value, subject.at) };
        let Some(target) = target else {
            return Some(Err(IdeError::invalid("this string can't be renamed")));
        };
        Some(self.kind_rename(&cx, kind, target, new_name, cancel))
    }

    /// The edit renaming `target` of `kind` to `new_name`: its declarations,
    /// every usage, and the files the kind moves with it.
    pub(crate) fn kind_rename(
        &self,
        cx: &RefContext<'_>,
        kind: &'static dyn ReferenceKind,
        target: RenameTarget,
        new_name: &str,
        cancel: &CancellationToken,
    ) -> IdeResult<WorkspaceEdit> {
        let key = target.key.clone();
        if !kind.valid_name_for(&key, new_name) || !kind.valid_rename(&target, new_name) {
            return Err(IdeError::invalid(format!("'{new_name}' is not a valid name")));
        }
        let sites = self.string_usages(kind, &key, &|value| kind.renames_with(value, &key), cancel)?;
        let mut files: Vec<(PathBuf, Vec<TextEdit>)> = Vec::new();
        let mut add = |path: PathBuf, range: Range| match files.iter_mut().find(|(known, _)| *known == path) {
            Some((_, edits)) => {
                if !edits.iter().any(|edit| edit.range == range) {
                    edits.push(TextEdit { range, new_text: new_name.to_owned() });
                }
            }
            None => files.push((path, vec![TextEdit { range, new_text: new_name.to_owned() }])),
        };
        for (path, range) in target.declarations.iter().cloned() {
            add(path, range);
        }
        for site in sites {
            let (from, to) = kind.site_rename_range(&site.value, &target);
            let range = site.lines.range(site.start + from as u32, site.start + to as u32);
            add(site.path, range);
        }
        // The same part of the value where another kind of file writes it.
        for (path, range) in kind.other_renames(cx, &target) {
            add(path, range);
        }
        let mut edit = WorkspaceEdit::default();
        files.sort_by(|a, b| a.0.cmp(&b.0));
        for (path, edits) in files {
            edit.files.push(FileEdit::Edit { path, edits, version: None });
        }
        let new_value = format!("{}{new_name}{}", &key[..target.range.0], &key[target.range.1..]);
        edit.files.extend(kind.rename_files(cx, &key, &new_value));
        Ok(edit)
    }

    /// The other places in the document of the string reference at `pos`:
    /// its usages, and its declaration when it is in the document. `None`
    /// when the caret isn't on a string reference.
    pub fn string_highlights(&self, doc: DocId, pos: Pos) -> Option<Vec<StringHighlight>> {
        let parsed = self.parsed_document(doc)?;
        let path = self.document_path(doc)?;
        let offset = parsed.line_index().byte(pos);
        let (cx, subject) = self.ref_subject(&parsed, &path, offset)?;
        let resolver = self.resolver();
        let lines = parsed.line_index();
        let mut out: Vec<StringHighlight> = Vec::new();
        for target in subject.kind.resolve(&cx, &subject.value, subject.value.len()) {
            if target.path == path && target.range != Range::default() {
                out.push(StringHighlight { range: target.range, write: true });
            }
        }
        let key = if subject.declaration { subject.value.clone() } else { subject.kind.canonical(&cx, &subject.value) };
        let rules = cx.rules();
        let kinds = kinds_for(cx.frameworks);
        for site in sites_at(Some(&path), &parsed, &resolver, &[subject.kind], rules, None) {
            let Some((kind, rule)) = classify_in(&kinds, &site, &resolver, rules) else { continue };
            if kind.id() == subject.kind.id()
                && self.rule_site_context(&path, &site, rule).is_some_and(|cx| {
                    subject.kind.refers_to(&subject.kind.canonical(&cx, &prefixed(&site, rule)), &key)
                })
            {
                out.push(StringHighlight { range: lines.range(site.start, site.end), write: false });
            }
        }
        out.sort_by_key(|highlight| highlight.range.start);
        out.dedup();
        Some(out)
    }

    /// The string sites of `kind` in the project whose value `keep`
    /// accepts. The candidate files are the indexed files with every word
    /// of `key` and of the kind, and the open documents.
    pub(crate) fn string_usages(
        &self,
        kind: &'static dyn ReferenceKind,
        key: &str,
        keep: &dyn Fn(&str) -> bool,
        cancel: &CancellationToken,
    ) -> IdeResult<Vec<FoundSite>> {
        let mut candidates = self.usage_candidates(kind, key)?;
        let open: Vec<_> = self.open_documents().into_iter().filter(|document| !document.embedded).collect();
        let resolver = self.resolver();
        let project = self.laravel();
        let rules: Vec<CompletionRule> = project
            .iter()
            .flat_map(|project| project.ide_json.completions.iter())
            .filter(|rule| rule.kind == kind.id())
            .cloned()
            .collect();
        // A rule's prefix isn't written in the file.
        let mut text = kind.usage_text(key);
        for rule in &rules {
            if !rule.options.prefix.is_empty()
                && let Some(rest) = text.strip_prefix(rule.options.prefix.as_str())
            {
                text = rest;
            }
            // Its usages don't have the prefix's words either.
            if !rule.options.prefix.is_empty()
                && let Some(rest) = key.strip_prefix(rule.options.prefix.as_str())
            {
                for path in self.usage_candidates(kind, rest)? {
                    if !candidates.contains(&path) {
                        candidates.push(path);
                    }
                }
            }
        }
        let mut out = Vec::new();
        for path in candidates {
            if cancel.is_cancelled() {
                return Err(IdeError::cancelled());
            }
            let Some(file) = self.usage_file(&path, &open, text) else { continue };
            for site in sites_at(Some(&path), &file.parsed, &resolver, &[kind], &rules, None) {
                let rule = if kind.patterns().iter().any(|pattern| pattern.matches(&site, &resolver)) {
                    None
                } else {
                    match rules.iter().find(|rule| rules::rule_matches(rule, &site, &resolver)) {
                        Some(rule) => Some(rule),
                        None => continue,
                    }
                };
                let Some(cx) = self.rule_site_context(&path, &site, rule) else { continue };
                if !keep(&kind.canonical(&cx, &prefixed(&site, rule))) {
                    continue;
                }
                let Some((start, end)) = file.to_source(site.start, site.end) else { continue };
                let range = file.lines.range(start, end);
                let value = site.value.clone();
                out.push(FoundSite { path: path.clone(), range, start, lines: file.lines.clone(), value });
            }
        }
        out.extend(self.markup_sites(kind, key, &open, cancel)?);
        Ok(out)
    }

    /// The [`ReferenceKind::markup_usages`] of `key` in the project's files
    /// with every word of [`ReferenceKind::markup_words`] (and the open
    /// documents), closed or open.
    fn markup_sites(
        &self,
        kind: &'static dyn ReferenceKind,
        key: &str,
        open: &[crate::engine::OpenDocument],
        cancel: &CancellationToken,
    ) -> IdeResult<Vec<FoundSite>> {
        let words = kind.markup_words(key);
        if words.is_empty() {
            return Ok(Vec::new());
        }
        let Some(cx) = self.ref_context() else { return Ok(Vec::new()) };
        let mut candidates = self.files_with_words(words)?;
        for document in open {
            if !candidates.contains(&document.path) {
                candidates.push(document.path.clone());
            }
        }
        candidates.sort();
        let mut out = Vec::new();
        for path in candidates {
            if cancel.is_cancelled() {
                return Err(IdeError::cancelled());
            }
            if self.is_library(&path) {
                continue;
            }
            let text: String = match open.iter().find(|document| document.path == path) {
                Some(document) => document.text.to_string(),
                None => match std::fs::read_to_string(&path) {
                    Ok(text) => text.replace("\r\n", "\n"),
                    Err(_) => continue,
                },
            };
            let found = kind.markup_usages(&cx, &path, &text, key);
            if found.is_empty() {
                continue;
            }
            let lines = Arc::new(crate::syntax::LineIndex::new(&text));
            for (start, end) in found {
                out.push(FoundSite {
                    path: path.clone(),
                    range: lines.range(start, end),
                    start,
                    lines: lines.clone(),
                    value: key.to_owned(),
                });
            }
        }
        Ok(out)
    }

    /// The indexed files with every one of `words` (lower case).
    fn files_with_words(&self, mut words: Vec<String>) -> IdeResult<Vec<PathBuf>> {
        let Some(index) = self.index() else { return Ok(Vec::new()) };
        words.sort();
        words.dedup();
        let mut found: Option<HashSet<PathBuf>> = None;
        for word in words {
            let files: HashSet<PathBuf> = index.files_with_word(&word)?.into_iter().collect();
            found = Some(match found {
                Some(known) => known.intersection(&files).cloned().collect(),
                None => files,
            });
        }
        Ok(found.unwrap_or_default().into_iter().collect())
    }

    /// The candidate file `path` of a usage search for text `text`: the
    /// parse of its open buffer or of the file on disk (a Blade template
    /// compiled first). `None` for a library file or one without `text`.
    pub(crate) fn usage_file(
        &self,
        path: &Path,
        open: &[crate::engine::OpenDocument],
        text: &str,
    ) -> Option<UsageFile> {
        if self.is_library(path) {
            return None;
        }
        if crate::frameworks::laravel::views::is_blade(path) {
            let source = std::fs::read_to_string(path).ok()?.replace("\r\n", "\n");
            if !source.contains(text) {
                return None;
            }
            let component = crate::blade::views::ViewPaths::new(&self.root()).is_component_view(path);
            let compiled = crate::blade::compile(&source, component);
            let parsed = Arc::new(ParsedFile::parse("usages.php", compiled.php.as_str()));
            let lines = Arc::new(crate::syntax::LineIndex::new(&source));
            return Some(UsageFile { parsed, lines, compiled: Some(compiled) });
        }
        let parsed = match open.iter().find(|document| document.path == path) {
            Some(document) => self.parse_open(document),
            None => {
                let source = std::fs::read_to_string(path).ok()?;
                if !source.contains(text) {
                    return None;
                }
                Arc::new(ParsedFile::parse("usages.php", source))
            }
        };
        if !parsed.text().contains(text) {
            return None;
        }
        let lines = Arc::new(parsed.line_index().clone());
        Some(UsageFile { parsed, lines, compiled: None })
    }

    /// The files [`string_usages`](Self::string_usages) looks at for `key`
    /// of `kind`: the indexed files with every word, and the open documents.
    pub(crate) fn usage_candidates(&self, kind: &'static dyn ReferenceKind, key: &str) -> IdeResult<Vec<PathBuf>> {
        let mut candidates: Vec<PathBuf> = self.files_with_words(kind.usage_words(key))?;
        for document in self.open_documents().into_iter().filter(|document| !document.embedded) {
            if !candidates.contains(&document.path) {
                candidates.push(document.path.clone());
            }
        }
        candidates.sort();
        Ok(candidates)
    }
}

/// A file a usage search parsed.
pub(crate) struct UsageFile {
    pub(crate) parsed: Arc<ParsedFile>,
    /// The lines of the file's own text (the template's for a Blade one).
    pub(crate) lines: Arc<crate::syntax::LineIndex>,
    /// A Blade template's compiled PHP, which `parsed` is the parse of.
    pub(crate) compiled: Option<crate::blade::Compiled>,
}

impl UsageFile {
    /// The byte range in the file's own text of `start..end` of the parse.
    fn to_source(&self, start: u32, end: u32) -> Option<(u32, u32)> {
        match &self.compiled {
            Some(compiled) => Some((compiled.blade_offset(start)?, compiled.blade_offset(end)?)),
            None => Some((start, end)),
        }
    }
}

/// A string site found in the project.
pub(crate) struct FoundSite {
    pub(crate) path: PathBuf,
    /// The content of the literal.
    pub(crate) range: Range,
    /// The byte the content starts at, and the file's lines (for edits
    /// inside it).
    start: u32,
    lines: Arc<crate::syntax::LineIndex>,
    /// The content as written.
    value: String,
}

/// One place [`PhpEngine::string_highlights`] marks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StringHighlight {
    pub range: Range,
    /// The declaration (as opposed to a usage).
    pub write: bool,
}
