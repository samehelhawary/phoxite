//! The tables a Yii project's migrations make ([`Table`]), read without
//! running them: `createTable('{{%post}}', ['id' => $this->primaryKey(), …])`,
//! `addColumn()`, `dropColumn()`, `renameColumn()`, `alterColumn()`,
//! `dropTable()` and `renameTable()` of the files of the migration folders
//! ([`YiiProject::migration_paths`](super::YiiProject::migration_paths)), in
//! the order of their names. What `down()` and `safeDown()` do is left out.
//!
//! A column's type is what Yii's schema builder says (`$this->string()`,
//! `$this->integer()->notNull()`), or the first word of a type written as a
//! string or with `Schema::TYPE_…`. An Active Record class is of the table
//! its `tableName()` returns as a string, else of the table Yii names after
//! the class (`PostComment` is `post_comment`): its columns are its
//! attributes.
//!
//! The database itself is never asked: a table no migration of the project
//! makes has no columns here.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use mago_syntax::cst::{
    Access, Argument, ArrayElement, BinaryOperator, Call, ClassLikeConstantSelector, ClassLikeMemberSelector,
    Expression, Literal, Method, MethodCall,
};
use mago_syntax::walker::Walker;

use crate::base::Range;
use crate::frameworks::php_array::{elements, string_literal};
use crate::syntax::ParsedFile;
use crate::types::{Resolver, Type};

/// What PHP has for a column's value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColumnType {
    Int,
    Float,
    Bool,
    String,
    /// JSON: whatever was stored.
    Mixed,
}

/// A column of a table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Column {
    pub name: String,
    pub ty: ColumnType,
    pub nullable: bool,
    /// Where the migration names it.
    pub file: PathBuf,
    pub range: Range,
}

impl Column {
    /// The type of the attribute of a record.
    pub fn php_type(&self) -> Type {
        let ty = match self.ty {
            ColumnType::Int => Type::Int,
            ColumnType::Float => Type::Float,
            ColumnType::Bool => Type::Bool,
            ColumnType::String => Type::string(),
            ColumnType::Mixed => return Type::Mixed,
        };
        if self.nullable { ty.nullable() } else { ty }
    }
}

/// A table and its columns, in the order the migrations add them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Table {
    pub name: String,
    pub columns: Vec<Column>,
}

/// One thing a migration does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Op {
    Create { table: String, columns: Vec<Column> },
    Add { table: String, column: Column },
    Drop { table: String, column: String },
    Rename { table: String, from: String, to: String },
    Alter { table: String, column: String, ty: ColumnType, nullable: bool },
    DropTable(String),
    RenameTable { from: String, to: String },
}

/// A table's name as a migration or `tableName()` writes it, without the
/// prefix marks and quotes: `{{%post}}`, `` `post` `` and `public.post` are
/// `post`.
pub fn table_name(raw: &str) -> String {
    let name = raw.trim().trim_start_matches("{{").trim_end_matches("}}").trim_start_matches('%');
    let name = name.rsplit('.').next().unwrap_or(name);
    name.trim_matches(|c| matches!(c, '`' | '"' | '[' | ']' | '%')).to_owned()
}

/// The table Yii names after a class: `PostComment` is `post_comment`.
pub fn default_table(class: &str) -> String {
    let short = class.rsplit('\\').next().unwrap_or(class);
    let mut out = String::new();
    for (at, c) in short.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if at > 0 {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// The PHP type of a column type's name (`string`, `bigInteger`, `pk`), and
/// whether it is a primary key.
fn type_of(name: &str) -> (ColumnType, bool) {
    let name: String = name.chars().filter(|c| *c != '_').collect::<String>().to_ascii_lowercase();
    match name.as_str() {
        "primarykey" | "bigprimarykey" | "pk" | "upk" | "bigpk" | "ubigpk" => (ColumnType::Int, true),
        "tinyinteger" | "tinyint" | "smallinteger" | "smallint" | "integer" | "int" | "biginteger" | "bigint" => {
            (ColumnType::Int, false)
        }
        "boolean" | "bool" => (ColumnType::Bool, false),
        "float" | "double" => (ColumnType::Float, false),
        "json" | "jsonb" => (ColumnType::Mixed, false),
        _ => (ColumnType::String, false),
    }
}

fn lossy(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn member(selector: &ClassLikeMemberSelector<'_>) -> Option<String> {
    match selector {
        ClassLikeMemberSelector::Identifier(name) => Some(lossy(name.value)),
        _ => None,
    }
}

/// A type written as text: strings and `Schema::TYPE_STRING` joined with `.`.
fn written(expr: &Expression<'_>) -> Option<String> {
    match expr {
        Expression::Parenthesized(inner) => written(inner.expression),
        Expression::Literal(Literal::String(string)) => Some(lossy(string.value?)),
        Expression::Binary(binary) if matches!(binary.operator, BinaryOperator::StringConcat(_)) => {
            Some(written(binary.lhs)? + &written(binary.rhs).unwrap_or_default())
        }
        Expression::Access(Access::ClassConstant(access)) => {
            let ClassLikeConstantSelector::Identifier(constant) = &access.constant else { return None };
            let name = lossy(constant.value);
            Some(name.strip_prefix("TYPE_").unwrap_or(&name).to_ascii_lowercase())
        }
        _ => None,
    }
}

/// The type a column's definition says, and whether it may be null.
fn column_type(expr: &Expression<'_>) -> Option<(ColumnType, bool)> {
    // `$this->string(64)->notNull()->defaultValue('')`: the builder's first call names the type.
    let mut names: Vec<String> = Vec::new();
    let mut at = expr;
    loop {
        match at {
            Expression::Parenthesized(inner) => at = inner.expression,
            Expression::Call(Call::Method(call)) => {
                names.push(member(&call.method)?);
                at = call.object;
            }
            _ => break,
        }
    }
    if let Some(root) = names.last() {
        let (ty, key) = type_of(root);
        let not_null = key || names.iter().any(|name| name.eq_ignore_ascii_case("notNull"));
        return Some((ty, !not_null));
    }
    let text = written(expr)?;
    let word: String = text.trim().chars().take_while(|c| c.is_ascii_alphabetic() || *c == '_').collect();
    let (ty, key) = type_of(&word);
    let lower = text.to_ascii_lowercase();
    Some((ty, !(key || lower.contains("not null") || lower.contains("primary key"))))
}

struct Reader<'a> {
    parsed: &'a ParsedFile,
    file: &'a Path,
    /// The string constants of the file's classes.
    constants: Vec<(String, String)>,
    /// In `down()` or `safeDown()`.
    undoing: bool,
    out: Vec<Op>,
}

impl Reader<'_> {
    /// A name an argument is: a string, or a constant of the class holding one.
    fn name(&self, expr: &Expression<'_>) -> Option<String> {
        if let Some((text, _, _)) = string_literal(expr) {
            return Some(text);
        }
        let Expression::Access(Access::ClassConstant(access)) = expr else { return None };
        let ClassLikeConstantSelector::Identifier(constant) = &access.constant else { return None };
        let wanted = lossy(constant.value);
        self.constants.iter().find(|(name, _)| *name == wanted).map(|(_, value)| value.clone())
    }

    fn column(&self, name: &str, range: Range, definition: &Expression<'_>) -> Column {
        let (ty, nullable) = column_type(definition).unwrap_or((ColumnType::String, true));
        Column { name: name.to_owned(), ty, nullable, file: self.file.to_path_buf(), range }
    }
}

struct MigrationWalker;

impl<'a> Walker<'a, 'a, Reader<'a>> for MigrationWalker {
    fn walk_in_method(&self, method: &'a Method<'a>, cx: &mut Reader<'a>) {
        let name = lossy(method.name.value);
        cx.undoing = name.eq_ignore_ascii_case("down") || name.eq_ignore_ascii_case("safeDown");
    }

    fn walk_out_method(&self, _: &'a Method<'a>, cx: &mut Reader<'a>) {
        cx.undoing = false;
    }

    fn walk_in_method_call(&self, call: &'a MethodCall<'a>, cx: &mut Reader<'a>) {
        if cx.undoing {
            return;
        }
        let Some(method) = member(&call.method) else { return };
        let arguments: Vec<&Expression<'_>> = call
            .argument_list
            .arguments
            .iter()
            .filter_map(|argument| match argument {
                Argument::Positional(argument) => Some(argument.value),
                Argument::Named(_) => None,
            })
            .collect();
        let Some(table) = arguments.first().and_then(|first| cx.name(first)).map(|name| table_name(&name)) else {
            return;
        };
        if table.is_empty() {
            return;
        }
        let range = |start: u32, end: u32| cx.parsed.line_index().range(start, end);
        let named = |index: usize| arguments.get(index).and_then(|argument| string_literal(argument));
        match method.to_ascii_lowercase().as_str() {
            "createtable" => {
                let Some(fields) = arguments.get(1) else { return };
                let mut columns = Vec::new();
                for element in elements(fields) {
                    let ArrayElement::KeyValue(element) = element else { continue };
                    let Some((name, start, end)) = string_literal(element.key) else { continue };
                    columns.push(cx.column(&name, range(start, end), element.value));
                }
                cx.out.push(Op::Create { table, columns });
            }
            "addcolumn" => {
                let (Some((name, start, end)), Some(definition)) = (named(1), arguments.get(2)) else { return };
                let column = cx.column(&name, range(start, end), definition);
                cx.out.push(Op::Add { table, column });
            }
            "dropcolumn" => {
                if let Some((column, _, _)) = named(1) {
                    cx.out.push(Op::Drop { table, column });
                }
            }
            "renamecolumn" => {
                if let (Some((from, _, _)), Some((to, _, _))) = (named(1), named(2)) {
                    cx.out.push(Op::Rename { table, from, to });
                }
            }
            "altercolumn" => {
                let (Some((column, _, _)), Some(definition)) = (named(1), arguments.get(2)) else { return };
                if let Some((ty, nullable)) = column_type(definition) {
                    cx.out.push(Op::Alter { table, column, ty, nullable });
                }
            }
            "droptable" => cx.out.push(Op::DropTable(table)),
            "renametable" => {
                if let Some(to) = arguments.get(1).and_then(|to| cx.name(to)) {
                    cx.out.push(Op::RenameTable { from: table, to: table_name(&to) });
                }
            }
            _ => {}
        }
    }
}

/// The `const NAME = 'text';` of a file.
fn string_constants(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find("const ") {
        rest = &rest[at + 6..];
        let name: String = rest.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
        let after = rest[name.len()..].trim_start();
        let Some(value) = after.strip_prefix('=').map(str::trim_start) else { continue };
        let Some(quote) = value.chars().next().filter(|c| matches!(c, '\'' | '"')) else { continue };
        if let Some(end) = value[1..].find(quote)
            && !name.is_empty()
        {
            out.push((name, value[1..1 + end].to_owned()));
        }
    }
    out
}

/// What the migration `text`, the file at `path`, does.
pub fn read(path: &Path, text: String) -> Vec<Op> {
    if !["createTable", "Column", "dropTable", "renameTable"].iter().any(|word| text.contains(word)) {
        return Vec::new();
    }
    let constants = string_constants(&text);
    let parsed = ParsedFile::parse(&path.to_string_lossy(), text);
    let mut reader = Reader { parsed: &parsed, file: path, constants, undoing: false, out: Vec::new() };
    MigrationWalker.walk_program(parsed.program(), &mut reader);
    reader.out
}

/// The tables after `ops`, in order.
pub fn tables<'o>(ops: impl IntoIterator<Item = &'o Op>) -> Vec<Table> {
    let mut out: Vec<Table> = Vec::new();
    for op in ops {
        let index = |out: &Vec<Table>, name: &str| out.iter().position(|table| table.name == name);
        match op {
            Op::Create { table, columns } => {
                out.retain(|known| known.name != *table);
                out.push(Table { name: table.clone(), columns: columns.clone() });
            }
            Op::Add { table, column } => {
                let at = index(&out, table).unwrap_or_else(|| {
                    out.push(Table { name: table.clone(), columns: Vec::new() });
                    out.len() - 1
                });
                out[at].columns.retain(|known| known.name != column.name);
                out[at].columns.push(column.clone());
            }
            Op::Drop { table, column } => {
                if let Some(at) = index(&out, table) {
                    out[at].columns.retain(|known| known.name != *column);
                }
            }
            Op::Rename { table, from, to } => {
                if let Some(at) = index(&out, table) {
                    for column in out[at].columns.iter_mut().filter(|column| column.name == *from) {
                        column.name = to.clone();
                    }
                }
            }
            Op::Alter { table, column, ty, nullable } => {
                if let Some(at) = index(&out, table) {
                    for known in out[at].columns.iter_mut().filter(|known| known.name == *column) {
                        known.ty = *ty;
                        known.nullable = *nullable;
                    }
                }
            }
            Op::DropTable(table) => out.retain(|known| known.name != *table),
            Op::RenameTable { from, to } => {
                if let Some(at) = index(&out, from) {
                    out[at].name = to.clone();
                }
            }
        }
    }
    out
}

/// What each migration file does, kept until the file changes.
#[derive(Default)]
pub struct TableCache {
    /// The tables at an edit count.
    known: Option<(u64, Arc<Vec<Table>>)>,
    files: HashMap<PathBuf, (SystemTime, Arc<Vec<Op>>)>,
    /// The table each class's `tableName()` returns, at an edit count.
    names: HashMap<String, (u64, Option<String>)>,
}

impl TableCache {
    /// The table the `tableName()` of `class` returns, when known for `count`.
    pub fn name(&self, count: u64, class: &str) -> Option<Option<String>> {
        self.names.get(class).filter(|(at, _)| *at == count).map(|(_, name)| name.clone())
    }

    pub fn set_name(&mut self, count: u64, class: &str, name: Option<String>) {
        self.names.insert(class.to_owned(), (count, name));
    }

    /// The tables the migrations of `folders` make. `open` is the text of a
    /// file an editor has open.
    pub fn tables(
        &mut self,
        count: u64,
        folders: &[PathBuf],
        open: impl Fn(&Path) -> Option<String>,
    ) -> Arc<Vec<Table>> {
        if let Some((at, tables)) = &self.known
            && *at == count
        {
            return tables.clone();
        }
        let mut files: Vec<PathBuf> = Vec::new();
        for folder in folders {
            let found = std::fs::read_dir(folder).into_iter().flatten().flatten().map(|entry| entry.path());
            files.extend(found.filter(|path| path.extension().is_some_and(|ext| ext == "php")));
        }
        // Migrations run in the order of their names (`m230101_120000_create_post`).
        files.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
        files.truncate(2000);
        let mut ops: Vec<Arc<Vec<Op>>> = Vec::new();
        for file in &files {
            if let Some(text) = open(file) {
                ops.push(Arc::new(read(file, text)));
                continue;
            }
            let Ok(modified) = std::fs::metadata(file).and_then(|metadata| metadata.modified()) else { continue };
            if let Some((at, known)) = self.files.get(file)
                && *at == modified
            {
                ops.push(known.clone());
                continue;
            }
            let Ok(text) = std::fs::read_to_string(file) else { continue };
            let read = Arc::new(read(file, text));
            self.files.insert(file.clone(), (modified, read.clone()));
            ops.push(read);
        }
        let tables = Arc::new(tables(ops.iter().flat_map(|ops| ops.iter())));
        self.known = Some((count, tables.clone()));
        tables
    }
}

const RECORD: &str = "yii\\db\\BaseActiveRecord";

/// The table of the Active Record class `class`: what its `tableName()`
/// returns as a string, else the one Yii names after the class.
pub fn table_of(resolver: &Resolver<'_>, class: &str) -> Option<String> {
    let model = resolver.class_model(class)?;
    if !model.is_subclass_of(RECORD) {
        return None;
    }
    let own = model
        .method("tableName")
        .filter(|method| !method.declaring.fqn.trim_start_matches('\\').to_ascii_lowercase().starts_with("yii\\"));
    let Some(method) = own else { return Some(default_table(model.fqn())) };
    // The string the body returns, read from the file (a summary keeps a plain `string`).
    resolver.yii_table_name(method.declaring.fqn.trim_start_matches('\\'))
}

/// The table the `tableName()` of the class `short` of a file returns as
/// a string.
pub fn returned_table(text: &str, short: &str) -> Option<String> {
    let class = text.find(&format!("class {short}")).unwrap_or(0);
    let rest = &text[class..];
    let rest = &rest[rest.find("function tableName")?..];
    let after = rest[rest.find("return")? + 6..].trim_start();
    let quote = after.chars().next().filter(|c| matches!(*c, '\'' | '"'))?;
    let end = after[1..].find(quote)?;
    Some(table_name(&after[1..1 + end])).filter(|name| !name.is_empty())
}

/// The columns of the table of the Active Record class `class`, with the
/// table's name.
pub fn columns_of(resolver: &Resolver<'_>, class: &str) -> Option<(String, Vec<Column>)> {
    let table = table_of(resolver, class)?;
    let tables = resolver.yii_tables()?;
    let found = tables.iter().find(|known| known.name == table)?;
    Some((table, found.columns.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const CREATE: &str = "<?php\nuse yii\\db\\Migration;\nuse yii\\db\\Schema;\nclass m1 extends Migration\n{\n    const TABLE = '{{%post}}';\n    public function safeUp()\n    {\n        $this->createTable(self::TABLE, [\n            'id' => $this->primaryKey(),\n            'title' => $this->string(128)->notNull(),\n            'body' => $this->text(),\n            'views' => Schema::TYPE_INTEGER . ' NOT NULL DEFAULT 0',\n            'draft' => 'boolean',\n            'old' => $this->string(),\n        ], 'ENGINE=InnoDB');\n    }\n    public function safeDown()\n    {\n        $this->dropTable(self::TABLE);\n    }\n}\n";
    const CHANGE: &str = "<?php\nclass m2 extends \\yii\\db\\Migration\n{\n    public function up()\n    {\n        $this->addColumn('{{%post}}', 'rating', $this->float()->notNull());\n        $this->addColumn('post', 'meta', $this->json());\n        $this->dropColumn('{{%post}}', 'old');\n        $this->renameColumn('{{%post}}', 'body', 'content');\n        $this->alterColumn('{{%post}}', 'draft', $this->boolean()->notNull());\n        $this->createTable('{{%gone}}', ['id' => 'pk']);\n        $this->dropTable('{{%gone}}');\n        $this->renameTable('{{%post}}', '{{%article}}');\n    }\n}\n";

    #[test]
    fn migrations_make_the_tables_and_their_columns() {
        let mut ops = read(Path::new("m1.php"), CREATE.to_owned());
        assert_eq!(ops.len(), 1, "what `safeDown()` drops stays");
        ops.extend(read(Path::new("m2.php"), CHANGE.to_owned()));
        let tables = tables(&ops);
        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0].name, "article");
        let columns: Vec<(&str, ColumnType, bool)> =
            tables[0].columns.iter().map(|column| (column.name.as_str(), column.ty, column.nullable)).collect();
        assert_eq!(
            columns,
            [
                ("id", ColumnType::Int, false),
                ("title", ColumnType::String, false),
                ("content", ColumnType::String, true),
                ("views", ColumnType::Int, false),
                ("draft", ColumnType::Bool, false),
                ("rating", ColumnType::Float, false),
                ("meta", ColumnType::Mixed, true),
            ]
        );
        // A column is where its migration names it.
        assert_eq!(tables[0].columns[1].range.start.line, 10);
        assert_eq!(tables[0].columns[5].file, Path::new("m2.php"));
    }

    #[test]
    fn a_table_is_named_as_yii_names_it() {
        assert_eq!(table_name("{{%post}}"), "post");
        assert_eq!(table_name("{{post}}"), "post");
        assert_eq!(table_name("`public`.`post`"), "post");
        assert_eq!(default_table("app\\models\\PostComment"), "post_comment");
    }
}
