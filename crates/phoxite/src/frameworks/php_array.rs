//! A reader for PHP files that return an array: the configuration of Yii
//! and Laminas, Symfony's `services.php` where it is a plain array, PHP
//! translation catalogues. It reads the source and runs nothing.
//!
//! What it follows, since configuration files are written that way:
//!
//! - `return [...]` and `return array(...)` at the top level of the file;
//! - `return $config;` where the file assigns `$config` an array, with the
//!   later `$config['components']['log'] = …` and `$config['bootstrap'][] =
//!   …` of the file put into it (those inside an `if` too: what a
//!   development environment adds is still a name of the project);
//! - a value that is a variable the file assigns (`'db' => $db`);
//! - `require` and `include` of a path written with strings, `__DIR__` and
//!   `dirname()`: the array that file returns, when the caller gives a way
//!   to read files ([`read`]);
//! - `array_merge()`, `array_replace()`, their `_recursive` forms and a
//!   static `merge()` (Yii's `ArrayHelper::merge()`) of arrays it can read:
//!   a later string key replaces an earlier one, arrays under one key are
//!   merged;
//! - a static `config()` given one array (`return App::config([...])`,
//!   `Routes::config([...])`: Symfony's array configuration): that array.
//!
//! A key or a value is a string, a number, `true`, `false`, `null`,
//! `Foo::class` (the class as a fully qualified name, through the file's
//! namespace and imports), `Foo::BAR`, or strings joined with `.`. Anything
//! else is [`Node::Other`] with its source text: an `env()` call, a closure,
//! a `new`. Every entry keeps its file and the place of its key, for goto.
//!
//! It never fails: a file that returns no array it can read gives `None`.

use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use mago_span::HasSpan;
use mago_syntax::cst::{
    Access, Argument, ArrayElement, Assignment, AssignmentOperator, BinaryOperator, Call, ClassLikeConstantSelector,
    ClassLikeMemberSelector, Construct, Expression, Literal, MagicConstant, Statement, UnaryPrefixOperator, Variable,
};
use mago_syntax::walker::Walker;

use crate::base::Range;
use crate::names::{FileNames, NameKind, Resolution};
use crate::syntax::ParsedFile;

/// How deep `require`s and variables are followed.
const MAX_DEPTH: usize = 8;

/// A value of the array.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Node {
    Null,
    Bool(bool),
    Int(i64),
    /// A float, as written.
    Float(String),
    String(String),
    /// `Foo::class`: the class's fully qualified name, without a leading
    /// backslash.
    Class(String),
    /// `Foo::BAR`: the class's fully qualified name and the constant.
    Constant {
        class: String,
        name: String,
    },
    Array(Vec<Entry>),
    /// Anything else, as written.
    Other(String),
}

/// A key of the array.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Key {
    Str(String),
    Int(i64),
    /// `Foo::class => …`
    Class(String),
    /// `Foo::BAR => …`
    Constant {
        class: String,
        name: String,
    },
}

impl Key {
    /// The key as PHP has it when the source says so: a string, or the name
    /// of the class of `Foo::class`.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Str(text) | Self::Class(text) => Some(text),
            _ => None,
        }
    }
}

/// An element of an array.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// `None` for an element without a key (a list's item).
    pub key: Option<Key>,
    /// The file it is written in.
    pub file: Arc<Path>,
    /// Where the key is written (a string's content, without its quotes);
    /// the value's place for an element without a key.
    pub range: Range,
    pub value: Node,
    /// Where the value is written (a string's content, without its quotes).
    pub value_range: Range,
}

impl Entry {
    /// The key when it is a string or a `::class`.
    pub fn name(&self) -> Option<&str> {
        self.key.as_ref().and_then(Key::as_str)
    }
}

impl Node {
    /// The entry of the string key `key`.
    pub fn entry(&self, key: &str) -> Option<&Entry> {
        self.entries().iter().rev().find(|entry| entry.name() == Some(key))
    }

    /// The value of the string key `key`.
    pub fn get(&self, key: &str) -> Option<&Node> {
        self.entry(key).map(|entry| &entry.value)
    }

    /// The value below the string keys `path`, one array inside the other.
    pub fn at(&self, path: &[&str]) -> Option<&Node> {
        path.iter().try_fold(self, |node, key| node.get(key))
    }

    /// The elements of an array (none for anything else).
    pub fn entries(&self) -> &[Entry] {
        match self {
            Self::Array(entries) => entries,
            _ => &[],
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(text) => Some(text),
            _ => None,
        }
    }

    /// The class a value names: `Foo::class`, or a string that is a class
    /// name (`'app\models\User'`), without a leading backslash.
    pub fn class(&self) -> Option<&str> {
        match self {
            Self::Class(class) => Some(class),
            Self::String(text) => {
                let name = text.trim_start_matches('\\');
                let named = !name.is_empty()
                    && !name.ends_with('\\')
                    && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '\\')
                    && !name.starts_with(|c: char| c.is_ascii_digit());
                named.then_some(name)
            }
            _ => None,
        }
    }

    pub fn is_array(&self) -> bool {
        matches!(self, Self::Array(_))
    }
}

/// The array the file `path` returns, the files it requires read through
/// `text_of` (an open buffer first, where the caller has one).
pub fn read(path: &Path, text_of: &dyn Fn(&Path) -> Option<String>) -> Option<Node> {
    let mut files = Files { text_of: Some(text_of), open: Vec::new() };
    files.read(path)
}

/// The array `text`, the content of the file `path`, returns. A `require`
/// isn't followed: its value is [`Node::Other`].
pub fn parse(path: &Path, text: &str) -> Option<Node> {
    let mut files = Files { text_of: None, open: Vec::new() };
    files.parse(path, text)
}

/// The array literal a file's top-level `return` returns, for a reader that
/// walks the tree itself.
pub fn returned_array<'a>(parsed: &'a ParsedFile) -> Option<&'a Expression<'a>> {
    returned(parsed).filter(|value| is_array(value))
}

/// What the file's `return` returns: a statement of the file itself, or of
/// its `namespace`.
fn returned<'a>(parsed: &'a ParsedFile) -> Option<&'a Expression<'a>> {
    fn of<'a>(statements: impl Iterator<Item = &'a Statement<'a>>) -> Option<&'a Expression<'a>> {
        statements.into_iter().find_map(|statement| match statement {
            Statement::Return(r#return) => r#return.value,
            Statement::Namespace(namespace) => of(namespace.statements().iter()),
            _ => None,
        })
    }
    of(parsed.program().statements.iter())
}

pub fn is_array(expr: &Expression<'_>) -> bool {
    matches!(expr, Expression::Array(_) | Expression::LegacyArray(_))
}

/// The elements of an array literal (none for anything else).
pub fn elements<'a>(expr: &'a Expression<'a>) -> Vec<&'a ArrayElement<'a>> {
    match expr {
        Expression::Array(array) => array.elements.iter().collect(),
        Expression::LegacyArray(array) => array.elements.iter().collect(),
        _ => Vec::new(),
    }
}

/// A string literal's value and the byte range of its content.
pub fn string_literal(expr: &Expression<'_>) -> Option<(String, u32, u32)> {
    let Expression::Literal(Literal::String(string)) = expr else { return None };
    let value = String::from_utf8_lossy(string.value?).into_owned();
    let span = string.span();
    Some((value, span.start.offset + 1, span.end.offset.saturating_sub(1)))
}

/// Reads a file's text.
type TextOf<'f> = &'f dyn Fn(&Path) -> Option<String>;

/// The files being read, one requiring the other.
struct Files<'f> {
    text_of: Option<TextOf<'f>>,
    open: Vec<PathBuf>,
}

impl Files<'_> {
    fn read(&mut self, path: &Path) -> Option<Node> {
        let text = (self.text_of?)(path)?;
        self.parse(path, &text)
    }

    fn parse(&mut self, path: &Path, text: &str) -> Option<Node> {
        let path = tidy(path);
        if self.open.len() >= MAX_DEPTH || self.open.contains(&path) {
            return None;
        }
        let parsed = ParsedFile::parse("array.php", text.to_owned());
        let mut assignments = Assignments::default();
        AssignmentWalker.walk_program(parsed.program(), &mut assignments);
        let returned = returned(&parsed)?;
        self.open.push(path.clone());
        let mut reader = Reader {
            files: self,
            parsed: &parsed,
            names: FileNames::resolve(&parsed),
            file: Arc::from(path.as_path()),
            assignments: assignments.found,
        };
        let node = reader.node(returned, 0);
        self.open.pop();
        node.is_array().then_some(node)
    }
}

/// Every `=` of a file, in source order.
#[derive(Default)]
struct Assignments<'a> {
    found: Vec<&'a Assignment<'a>>,
}

struct AssignmentWalker;

impl<'a> Walker<'a, 'a, Assignments<'a>> for AssignmentWalker {
    fn walk_in_assignment(&self, assignment: &'a Assignment<'a>, cx: &mut Assignments<'a>) {
        if matches!(assignment.operator, AssignmentOperator::Assign(_)) {
            cx.found.push(assignment);
        }
    }
}

struct Reader<'r, 'f, 'a> {
    files: &'r mut Files<'f>,
    parsed: &'a ParsedFile,
    names: FileNames,
    file: Arc<Path>,
    assignments: Vec<&'a Assignment<'a>>,
}

fn lossy(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn direct<'a>(expr: &Expression<'a>) -> Option<&'a [u8]> {
    match expr {
        Expression::Variable(Variable::Direct(variable)) => Some(variable.name),
        _ => None,
    }
}

impl<'a> Reader<'_, '_, 'a> {
    fn source(&self, expr: &Expression<'_>) -> String {
        let span = expr.span();
        let text = self.parsed.text().get(span.start.offset as usize..span.end.offset as usize).unwrap_or_default();
        text.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    /// Where `expr` is written: a string's content, else all of it.
    fn range(&self, expr: &Expression<'_>) -> Range {
        let span = expr.span();
        let (start, end) = match string_literal(expr) {
            Some((_, start, end)) => (start, end),
            None => (span.start.offset, span.end.offset),
        };
        self.parsed.line_index().range(start, end)
    }

    /// The class `expr` names in `expr::…`, fully qualified.
    fn class_name(&self, expr: &Expression<'_>) -> Option<String> {
        let Expression::Identifier(identifier) = expr else { return None };
        let scope = self.names.scope_at(identifier.span().start.offset);
        match scope.resolve(NameKind::ClassLike, &lossy(identifier.value())).resolution {
            Resolution::Fqn(fqn) => Some(fqn),
            _ => None,
        }
    }

    /// `Foo::class` and `Foo::BAR`.
    fn class_constant(&self, expr: &Expression<'_>) -> Option<Node> {
        let Expression::Access(Access::ClassConstant(access)) = expr else { return None };
        let ClassLikeConstantSelector::Identifier(constant) = &access.constant else { return None };
        let class = self.class_name(access.class)?;
        Some(if constant.value.eq_ignore_ascii_case(b"class") {
            Node::Class(class)
        } else {
            Node::Constant { class, name: lossy(constant.value) }
        })
    }

    fn node(&mut self, expr: &'a Expression<'a>, depth: usize) -> Node {
        if depth > MAX_DEPTH {
            return Node::Other(self.source(expr));
        }
        match expr {
            Expression::Parenthesized(inner) => return self.node(inner.expression, depth),
            Expression::Literal(Literal::String(string)) => {
                if let Some(value) = string.value {
                    return Node::String(lossy(value));
                }
            }
            Expression::Literal(Literal::Integer(integer)) => {
                if let Some(value) = integer.value.and_then(|value| i64::try_from(value).ok()) {
                    return Node::Int(value);
                }
            }
            Expression::Literal(Literal::Float(float)) => return Node::Float(lossy(float.raw)),
            Expression::Literal(Literal::True(_)) => return Node::Bool(true),
            Expression::Literal(Literal::False(_)) => return Node::Bool(false),
            Expression::Literal(Literal::Null(_)) => return Node::Null,
            Expression::UnaryPrefix(unary) if matches!(unary.operator, UnaryPrefixOperator::Negation(_)) => {
                match self.node(unary.operand, depth + 1) {
                    Node::Int(value) => return Node::Int(-value),
                    Node::Float(value) => return Node::Float(format!("-{value}")),
                    _ => {}
                }
            }
            Expression::Array(_) | Expression::LegacyArray(_) => return Node::Array(self.entries(expr, depth)),
            Expression::Access(_) => {
                if let Some(constant) = self.class_constant(expr) {
                    return constant;
                }
            }
            Expression::Binary(binary) if matches!(binary.operator, BinaryOperator::StringConcat(_)) => {
                let (lhs, rhs) = (self.node(binary.lhs, depth + 1), self.node(binary.rhs, depth + 1));
                let text = |node: &Node| match node {
                    Node::String(text) | Node::Class(text) => Some(text.clone()),
                    Node::Int(value) => Some(value.to_string()),
                    _ => None,
                };
                if let (Some(lhs), Some(rhs)) = (text(&lhs), text(&rhs)) {
                    return Node::String(lhs + &rhs);
                }
            }
            Expression::Variable(_) => {
                if let Some(value) = self.variable(expr, depth) {
                    return value;
                }
            }
            Expression::Construct(construct) => {
                let path = match construct {
                    Construct::Require(require) => Some(require.value),
                    Construct::RequireOnce(require) => Some(require.value),
                    Construct::Include(include) => Some(include.value),
                    Construct::IncludeOnce(include) => Some(include.value),
                    _ => None,
                };
                let file = path.and_then(|path| path_of(&self.file, path)).map(|path| tidy(Path::new(&path)));
                if let Some(node) = file.and_then(|file| self.files.read(&file)) {
                    return node;
                }
            }
            Expression::Call(call) => {
                if let Some(merged) = self.merged(call, depth) {
                    return merged;
                }
            }
            _ => {}
        }
        Node::Other(self.source(expr))
    }

    /// What the file assigns the variable `expr`: its first plain
    /// assignment, with the later `$name[…] = …` put into it.
    fn variable(&mut self, expr: &'a Expression<'a>, depth: usize) -> Option<Node> {
        let name = direct(expr)?;
        let assignments = self.assignments.clone();
        let first = assignments.iter().find(|assignment| direct(assignment.lhs) == Some(name))?;
        // A variable assigned itself (`$config = merge($config, …)`) ends here.
        if first.rhs.span().start.offset <= expr.span().start.offset
            && expr.span().end.offset <= first.rhs.span().end.offset
        {
            return None;
        }
        let mut node = self.node(first.rhs, depth + 1);
        let Node::Array(entries) = &mut node else { return Some(node) };
        for assignment in &assignments {
            let Some(keys) = self.patch_keys(assignment.lhs, name) else { continue };
            if keys.is_empty() {
                continue;
            }
            let value = self.node(assignment.rhs, depth + 1);
            let value_range = self.range(assignment.rhs);
            put(entries, &keys, value, value_range, &self.file);
        }
        Some(node)
    }

    /// The keys of `$name['a']['b']` and `$name['a'][]` (`None` for `[]`),
    /// outermost first; each with where it is written.
    fn patch_keys(&self, target: &Expression<'_>, name: &[u8]) -> Option<Vec<(Option<Key>, Range)>> {
        match target {
            Expression::ArrayAccess(access) => {
                let mut keys = self.patch_keys(access.array, name)?;
                keys.push((Some(self.key(access.index)?), self.range(access.index)));
                Some(keys)
            }
            Expression::ArrayAppend(append) => {
                let mut keys = self.patch_keys(append.array, name)?;
                let at = append.left_bracket.start.offset;
                keys.push((None, self.parsed.line_index().range(at, at)));
                Some(keys)
            }
            _ => (direct(target) == Some(name)).then(Vec::new),
        }
    }

    fn key(&self, expr: &Expression<'_>) -> Option<Key> {
        match expr {
            Expression::Parenthesized(inner) => self.key(inner.expression),
            Expression::Literal(Literal::String(string)) => Some(Key::Str(lossy(string.value?))),
            Expression::Literal(Literal::Integer(integer)) => {
                Some(Key::Int(integer.value.and_then(|value| i64::try_from(value).ok())?))
            }
            Expression::Access(_) => match self.class_constant(expr)? {
                Node::Class(class) => Some(Key::Class(class)),
                Node::Constant { class, name } => Some(Key::Constant { class, name }),
                _ => None,
            },
            _ => None,
        }
    }

    fn entries(&mut self, array: &'a Expression<'a>, depth: usize) -> Vec<Entry> {
        let mut out = Vec::new();
        for element in elements(array) {
            let (key, value) = match element {
                ArrayElement::KeyValue(element) => (Some(element.key), element.value),
                ArrayElement::Value(element) => (None, element.value),
                // `...require 'more.php'`: its elements are this array's.
                ArrayElement::Variadic(element) => {
                    if let Node::Array(spread) = self.node(element.value, depth + 1) {
                        merge(&mut out, spread, false);
                    }
                    continue;
                }
                ArrayElement::Missing(_) => continue,
            };
            let read = match key {
                Some(key) => match self.key(key) {
                    Some(read) => Some(read),
                    // A key that can't be read names nothing.
                    None => continue,
                },
                None => None,
            };
            let value_range = self.range(value);
            let range = key.map_or(value_range, |key| self.range(key));
            out.push(Entry {
                key: read,
                file: self.file.clone(),
                range,
                value: self.node(value, depth + 1),
                value_range,
            });
        }
        out
    }

    /// `array_merge($a, $b)`, `ArrayHelper::merge($a, $b)`: one array of
    /// those of its arguments that can be read.
    fn merged(&mut self, call: &'a Call<'a>, depth: usize) -> Option<Node> {
        const FUNCTIONS: [&str; 4] =
            ["array_merge", "array_replace", "array_merge_recursive", "array_replace_recursive"];
        let (arguments, deep) = match call {
            Call::Function(call) => {
                let Expression::Identifier(function) = call.function else { return None };
                let name = lossy(function.value());
                let name = name.trim_start_matches('\\');
                let known = FUNCTIONS.iter().find(|known| known.eq_ignore_ascii_case(name))?;
                (&call.argument_list, known.ends_with("_recursive"))
            }
            Call::StaticMethod(call) => {
                let ClassLikeMemberSelector::Identifier(method) = &call.method else { return None };
                if method.value.eq_ignore_ascii_case(b"config") {
                    let mut arguments = call.argument_list.arguments.iter();
                    let (Some(Argument::Positional(only)), None) = (arguments.next(), arguments.next()) else {
                        return None;
                    };
                    let node = self.node(only.value, depth + 1);
                    return node.is_array().then_some(node);
                }
                if !method.value.eq_ignore_ascii_case(b"merge") {
                    return None;
                }
                (&call.argument_list, true)
            }
            _ => return None,
        };
        let mut out = Vec::new();
        let mut any = false;
        for argument in arguments.arguments.iter() {
            let Argument::Positional(argument) = argument else { continue };
            if let Node::Array(entries) = self.node(argument.value, depth + 1) {
                merge(&mut out, entries, deep);
                any = true;
            }
        }
        any.then_some(Node::Array(out))
    }
}

/// The path `expr`, an expression of the file `file`, is: strings,
/// `__DIR__`, `__FILE__` and `dirname()` joined with `.`.
pub fn path_of(file: &Path, expr: &Expression<'_>) -> Option<String> {
    match expr {
        Expression::Parenthesized(inner) => path_of(file, inner.expression),
        Expression::Literal(Literal::String(string)) => Some(lossy(string.value?)),
        Expression::MagicConstant(MagicConstant::Directory(_)) => Some(file.parent()?.to_string_lossy().into_owned()),
        Expression::MagicConstant(MagicConstant::File(_)) => Some(file.to_string_lossy().into_owned()),
        Expression::Binary(binary) if matches!(binary.operator, BinaryOperator::StringConcat(_)) => {
            Some(path_of(file, binary.lhs)? + &path_of(file, binary.rhs)?)
        }
        Expression::Call(Call::Function(call)) => {
            let Expression::Identifier(function) = call.function else { return None };
            if !function.value().trim_ascii_start().eq_ignore_ascii_case(b"dirname")
                && !function.value().eq_ignore_ascii_case(b"\\dirname")
            {
                return None;
            }
            let mut arguments = call.argument_list.arguments.iter().filter_map(|argument| match argument {
                Argument::Positional(argument) => Some(argument.value),
                _ => None,
            });
            let mut path = PathBuf::from(path_of(file, arguments.next()?)?);
            let levels = match arguments.next() {
                Some(Expression::Literal(Literal::Integer(levels))) => levels.value.unwrap_or(1),
                Some(_) => return None,
                None => 1,
            };
            for _ in 0..levels.min(16) {
                path.pop();
            }
            Some(path.to_string_lossy().into_owned())
        }
        _ => None,
    }
}

/// Puts `value` below `keys` of `entries`, making the arrays on the way.
fn put(entries: &mut Vec<Entry>, keys: &[(Option<Key>, Range)], value: Node, value_range: Range, file: &Arc<Path>) {
    let Some(((key, range), rest)) = keys.split_first() else { return };
    let existing = key.as_ref().and_then(|key| entries.iter().rposition(|entry| entry.key.as_ref() == Some(key)));
    if rest.is_empty() {
        let entry = Entry { key: key.clone(), file: file.clone(), range: *range, value, value_range };
        match existing {
            Some(at) => entries[at] = entry,
            None => entries.push(entry),
        }
        return;
    }
    let at = match existing {
        Some(at) => at,
        None => {
            entries.push(Entry {
                key: key.clone(),
                file: file.clone(),
                range: *range,
                value: Node::Array(Vec::new()),
                value_range: *range,
            });
            entries.len() - 1
        }
    };
    if !entries[at].value.is_array() {
        entries[at].value = Node::Array(Vec::new());
    }
    if let Node::Array(inner) = &mut entries[at].value {
        put(inner, rest, value, value_range, file);
    }
}

/// `from` into `into`, as PHP's `array_merge()` does: an element without a
/// string key is added, one with a string key replaces the element of that
/// key. `deep`: arrays under one key are merged, not replaced.
fn merge(into: &mut Vec<Entry>, from: Vec<Entry>, deep: bool) {
    for entry in from {
        let named = matches!(entry.key, Some(Key::Str(_) | Key::Class(_) | Key::Constant { .. }));
        let existing = if named { into.iter().position(|known| known.key == entry.key) } else { None };
        match existing {
            Some(at) => match (&mut into[at].value, entry.value) {
                (Node::Array(known), Node::Array(new)) if deep => merge(known, new, deep),
                (_, value) => {
                    into[at] = Entry { value, ..entry };
                }
            },
            None => into.push(entry),
        }
    }
}

/// `path` without its `.` and `..` parts.
pub fn tidy(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn array(text: &str) -> Node {
        parse(Path::new("/app/config/web.php"), text).expect("an array")
    }

    #[test]
    fn scalars_classes_and_constants_are_read() {
        let node = array(
            "<?php\nnamespace App;\nuse yii\\caching\\FileCache;\nuse App\\Kind as K;\nreturn [\n    'id' => 'basic',\n    'port' => 8080,\n    'offset' => -3,\n    'ratio' => 1.5,\n    'debug' => true,\n    'none' => null,\n    'cache' => FileCache::class,\n    'local' => Mailer::class,\n    'full' => \\Psr\\Log\\LoggerInterface::class,\n    'kind' => K::DEFAULT,\n    'name' => 'a' . '-' . 'b',\n    'env' => getenv('X'),\n];\n",
        );
        assert_eq!(node.get("id"), Some(&Node::String("basic".into())));
        assert_eq!(node.get("port"), Some(&Node::Int(8080)));
        assert_eq!(node.get("offset"), Some(&Node::Int(-3)));
        assert_eq!(node.get("ratio"), Some(&Node::Float("1.5".into())));
        assert_eq!(node.get("debug"), Some(&Node::Bool(true)));
        assert_eq!(node.get("none"), Some(&Node::Null));
        assert_eq!(node.get("cache"), Some(&Node::Class("yii\\caching\\FileCache".into())));
        assert_eq!(node.get("local"), Some(&Node::Class("App\\Mailer".into())));
        assert_eq!(node.get("full"), Some(&Node::Class("Psr\\Log\\LoggerInterface".into())));
        assert_eq!(node.get("kind"), Some(&Node::Constant { class: "App\\Kind".into(), name: "DEFAULT".into() }));
        assert_eq!(node.get("name"), Some(&Node::String("a-b".into())));
        assert_eq!(node.get("env"), Some(&Node::Other("getenv('X')".into())));
    }

    #[test]
    fn keys_are_strings_numbers_and_classes_and_keep_their_place() {
        let node = array(
            "<?php\nuse App\\Mailer;\nreturn array(\n    'factories' => [\n        Mailer::class => MailerFactory::class,\n        3 => 'three',\n        'plain',\n    ],\n);\n",
        );
        let factories = node.get("factories").expect("factories");
        let entries = factories.entries();
        assert_eq!(entries[0].key, Some(Key::Class("App\\Mailer".into())));
        assert_eq!(entries[0].name(), Some("App\\Mailer"));
        assert_eq!(entries[0].value.class(), Some("MailerFactory"));
        assert_eq!(entries[1].key, Some(Key::Int(3)));
        assert_eq!(entries[2].key, None);
        assert_eq!(factories.get("App\\Mailer"), Some(&Node::Class("MailerFactory".into())));
        // `'factories'` is on the fourth line, its content after the quote.
        let entry = node.entry("factories").expect("entry");
        assert_eq!((entry.range.start.line, entry.range.start.col), (3, 5));
        assert_eq!(entry.range.end.col, 14);
        assert_eq!(&*entry.file, Path::new("/app/config/web.php"));
        // A value's place is its content too.
        assert_eq!((entries[1].value_range.start.line, entries[1].value_range.start.col), (5, 14));
    }

    #[test]
    fn a_returned_variable_has_what_the_file_puts_into_it() {
        let node = array(
            "<?php\n$db = ['class' => 'yii\\db\\Connection'];\n$config = [\n    'id' => 'basic',\n    'bootstrap' => ['log'],\n    'components' => ['db' => $db],\n];\nif (YII_ENV_DEV) {\n    $config['bootstrap'][] = 'debug';\n    $config['modules']['debug'] = ['class' => 'yii\\debug\\Module'];\n    $config['id'] = 'dev';\n}\nreturn $config;\n",
        );
        assert_eq!(node.at(&["components", "db", "class"]).and_then(Node::class), Some("yii\\db\\Connection"));
        let bootstrap: Vec<_> = node.get("bootstrap").unwrap().entries().iter().map(|e| e.value.as_str()).collect();
        assert_eq!(bootstrap, [Some("log"), Some("debug")]);
        assert_eq!(node.at(&["modules", "debug", "class"]).and_then(Node::as_str), Some("yii\\debug\\Module"));
        assert_eq!(node.get("id").and_then(Node::as_str), Some("dev"));
        // The key written in the `if` is where the module is declared.
        assert_eq!(node.get("modules").unwrap().entry("debug").unwrap().range.start.line, 9);
    }

    #[test]
    fn required_files_and_merges_are_followed() {
        let files: HashMap<PathBuf, &str> = [
            (
                PathBuf::from("/app/config/web.php"),
                "<?php\n$params = require __DIR__ . '/params.php';\nreturn array_merge(\n    require(__DIR__ . '/common.php'),\n    ['params' => $params, 'components' => ['db' => require dirname(__DIR__) . '/db.php']],\n    require __DIR__ . '/missing.php'\n);\n",
            ),
            (PathBuf::from("/app/config/params.php"), "<?php\nreturn ['adminEmail' => 'admin@example.com'];\n"),
            (
                PathBuf::from("/app/config/common.php"),
                "<?php\nreturn ['id' => 'app', 'components' => ['cache' => ['class' => 'yii\\caching\\FileCache']]];\n",
            ),
            (PathBuf::from("/app/db.php"), "<?php\nreturn ['dsn' => 'sqlite::memory:'];\n"),
        ]
        .into_iter()
        .collect();
        let text_of = |path: &Path| files.get(&tidy(path)).map(|text| (*text).to_owned());
        let node = read(Path::new("/app/config/web.php"), &text_of).expect("an array");
        assert_eq!(node.get("id").and_then(Node::as_str), Some("app"));
        assert_eq!(node.at(&["params", "adminEmail"]).and_then(Node::as_str), Some("admin@example.com"));
        // `array_merge` replaces `components`; the entry says which file it is in.
        assert_eq!(node.at(&["components", "cache"]), None);
        let db = node.get("components").unwrap().entry("db").unwrap();
        assert_eq!(db.value.get("dsn").and_then(Node::as_str), Some("sqlite::memory:"));
        assert_eq!(&*db.value.entry("dsn").unwrap().file, Path::new("/app/db.php"));
    }

    #[test]
    fn a_deep_merge_keeps_both_sides() {
        let node = array(
            "<?php\nreturn ArrayHelper::merge(\n    ['components' => ['cache' => ['class' => 'A'], 'log' => []], 'list' => ['a']],\n    ['components' => ['cache' => ['class' => 'B']], 'list' => ['b']]\n);\n",
        );
        assert_eq!(node.at(&["components", "cache", "class"]).and_then(Node::as_str), Some("B"));
        assert!(node.at(&["components", "log"]).is_some());
        assert_eq!(node.get("list").unwrap().entries().len(), 2);
    }

    #[test]
    fn a_configuration_call_is_its_array() {
        let node = array(
            "<?php\nnamespace Symfony\\Config;\nreturn App::config([\n    'parameters' => ['locale' => 'en'],\n]);\n",
        );
        assert_eq!(node.at(&["parameters", "locale"]).and_then(Node::as_str), Some("en"));
    }

    #[test]
    fn a_file_that_returns_no_array_is_nothing() {
        let path = Path::new("/app/x.php");
        assert_eq!(parse(path, "<?php\nreturn new Config();\n"), None);
        assert_eq!(parse(path, "<?php\necho 1;\n"), None);
        assert_eq!(parse(path, "<?php\nreturn $unknown;\n"), None);
        // A file requiring itself ends.
        let text_of = |_: &Path| Some("<?php\nreturn ['again' => require __FILE__];\n".to_owned());
        let node = read(path, &text_of).expect("an array");
        assert!(matches!(node.get("again"), Some(Node::Other(_))));
    }
}
