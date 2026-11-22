//! The tables a CakePHP project's migrations make ([`Table`]), read without
//! running them: the files of `config/Migrations` (the application's and
//! each plugin's), in the order of their names. A migration builds a table
//! from `$this->table('articles')`: `addColumn('title', 'string', [...])`,
//! `removeColumn()`, `renameColumn()`, `changeColumn()`, `drop()` and
//! `rename()`, called on it directly or on the variable it is assigned to.
//! A table has an `id` unless its options say otherwise (`'id' => false`,
//! `'id' => 'article_id'`). What `down()` does is left out.
//!
//! An entity's fields are the columns of its table: the table class of its
//! name (`Article` is of `ArticlesTable`), whose database table is what
//! `setTable()` names, else the table's name in snake case (`articles`).
//!
//! The database itself is never asked: a table no migration of the project
//! makes has no columns here.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use mago_syntax::cst::{
    Argument, ArrayElement, Assignment, Call, ClassLikeMemberSelector, Expression, Literal, Method, MethodCall,
    Variable,
};
use mago_syntax::walker::Walker;

use super::classes;
use super::inflect::underscore;
use crate::base::Range;
use crate::frameworks::php_array::{elements, string_literal};
use crate::syntax::ParsedFile;
use crate::types::Resolver;

/// What PHP has for a column's value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColumnType {
    Int,
    Float,
    Bool,
    String,
    /// A date and a time: CakePHP's `DateTime`.
    DateTime,
    /// CakePHP's `Date`.
    Date,
    /// CakePHP's `Time`.
    Time,
    /// JSON: whatever was stored.
    Mixed,
}

impl ColumnType {
    /// The type of a column type's name (`string`, `biginteger`, `datetime`).
    fn of(name: &str) -> Self {
        match name.to_ascii_lowercase().as_str() {
            "integer" | "biginteger" | "smallinteger" | "tinyinteger" | "int" | "bigint" => Self::Int,
            "boolean" | "bool" => Self::Bool,
            "float" | "double" => Self::Float,
            "datetime" | "timestamp" | "datetimefractional" | "timestampfractional" | "timestamptimezone" => {
                Self::DateTime
            }
            "date" => Self::Date,
            "time" => Self::Time,
            "json" | "jsonb" => Self::Mixed,
            _ => Self::String,
        }
    }
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

/// A table and its columns, in the order the migrations add them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Table {
    pub name: String,
    pub columns: Vec<Column>,
}

/// One thing a migration does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Op {
    /// `$this->table('articles')`: the table, and its primary key when it
    /// has the one CakePHP adds.
    Table {
        table: String,
        id: Option<Column>,
    },
    Add {
        table: String,
        column: Column,
    },
    Remove {
        table: String,
        column: String,
    },
    Rename {
        table: String,
        from: String,
        to: String,
    },
    Change {
        table: String,
        column: String,
        ty: ColumnType,
        nullable: bool,
    },
    Drop(String),
    RenameTable {
        from: String,
        to: String,
    },
}

struct Reader<'a> {
    parsed: &'a ParsedFile,
    file: &'a Path,
    /// The variables a table is assigned to, by name.
    variables: HashMap<Vec<u8>, String>,
    /// In `down()`.
    undoing: bool,
    out: Vec<Op>,
}

fn lossy(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn member(call: &MethodCall<'_>) -> Option<String> {
    match &call.method {
        ClassLikeMemberSelector::Identifier(name) => Some(lossy(name.value).to_ascii_lowercase()),
        _ => None,
    }
}

fn arguments<'a>(call: &'a MethodCall<'a>) -> Vec<&'a Expression<'a>> {
    call.argument_list
        .arguments
        .iter()
        .filter_map(|argument| match argument {
            Argument::Positional(argument) => Some(argument.value),
            Argument::Named(_) => None,
        })
        .collect()
}

/// The value under the string key `key` of an options array.
fn option<'a>(options: Option<&&'a Expression<'a>>, key: &str) -> Option<&'a Expression<'a>> {
    elements(options?).into_iter().find_map(|element| match element {
        ArrayElement::KeyValue(element) if string_literal(element.key).is_some_and(|(found, _, _)| found == key) => {
            Some(element.value)
        }
        _ => None,
    })
}

impl Reader<'_> {
    /// The table the calls on `expr` are about: `$this->table('articles')`
    /// at the start of the chain, or a variable assigned one.
    fn table(&self, expr: &Expression<'_>) -> Option<String> {
        match expr {
            Expression::Parenthesized(inner) => self.table(inner.expression),
            Expression::Variable(Variable::Direct(variable)) => self.variables.get(variable.name).cloned(),
            Expression::Call(Call::Method(call)) => {
                if member(call).as_deref() == Some("table") {
                    return arguments(call).first().and_then(|name| string_literal(name)).map(|(name, _, _)| name);
                }
                self.table(call.object)
            }
            _ => None,
        }
    }

    fn column(&self, name: (String, u32, u32), ty: ColumnType, options: Option<&&Expression<'_>>) -> Column {
        let nullable = matches!(option(options, "null"), Some(Expression::Literal(Literal::True(_))));
        let (name, start, end) = name;
        Column { name, ty, nullable, file: self.file.to_path_buf(), range: self.parsed.line_index().range(start, end) }
    }
}

struct MigrationWalker;

impl<'a> Walker<'a, 'a, Reader<'a>> for MigrationWalker {
    fn walk_in_method(&self, method: &'a Method<'a>, cx: &mut Reader<'a>) {
        cx.undoing = method.name.value.eq_ignore_ascii_case(b"down");
        cx.variables.clear();
    }

    fn walk_out_method(&self, _: &'a Method<'a>, cx: &mut Reader<'a>) {
        cx.undoing = false;
    }

    /// `$table = $this->table('articles');`: the variable is the table.
    fn walk_out_assignment(&self, assignment: &'a Assignment<'a>, cx: &mut Reader<'a>) {
        if let Expression::Variable(Variable::Direct(variable)) = assignment.lhs
            && let Some(table) = cx.table(assignment.rhs)
        {
            cx.variables.insert(variable.name.to_vec(), table);
        }
    }

    /// (On the way out: the calls of a chain in the order they are made.)
    fn walk_out_method_call(&self, call: &'a MethodCall<'a>, cx: &mut Reader<'a>) {
        if cx.undoing {
            return;
        }
        let Some(method) = member(call) else { return };
        let arguments = arguments(call);
        if method == "table" {
            let Some((table, start, end)) = arguments.first().and_then(|name| string_literal(name)) else { return };
            // The primary key CakePHP adds: `id`, or the name the options give; none for `false`.
            let id = match option(arguments.get(1), "id") {
                Some(Expression::Literal(Literal::False(_))) => None,
                Some(named) => string_literal(named).map(|(name, _, _)| name),
                None => Some("id".to_owned()),
            };
            let id = id.map(|name| cx.column((name, start, end), ColumnType::Int, None));
            cx.out.push(Op::Table { table, id });
            return;
        }
        let Some(table) = cx.table(call.object) else { return };
        let named = |index: usize| arguments.get(index).and_then(|argument| string_literal(argument));
        match method.as_str() {
            "addcolumn" => {
                let Some(name) = named(0) else { return };
                let ty = named(1).map_or(ColumnType::String, |(ty, _, _)| ColumnType::of(&ty));
                let column = cx.column(name, ty, arguments.get(2));
                cx.out.push(Op::Add { table, column });
            }
            "removecolumn" => {
                if let Some((column, _, _)) = named(0) {
                    cx.out.push(Op::Remove { table, column });
                }
            }
            "renamecolumn" => {
                if let (Some((from, _, _)), Some((to, _, _))) = (named(0), named(1)) {
                    cx.out.push(Op::Rename { table, from, to });
                }
            }
            "changecolumn" => {
                let (Some(name), Some((ty, _, _))) = (named(0), named(1)) else { return };
                let column = cx.column(name, ColumnType::of(&ty), arguments.get(2));
                cx.out.push(Op::Change { table, column: column.name, ty: column.ty, nullable: column.nullable });
            }
            "drop" => cx.out.push(Op::Drop(table)),
            "rename" => {
                if let Some((to, _, _)) = named(0) {
                    cx.out.push(Op::RenameTable { from: table, to });
                }
            }
            _ => {}
        }
    }
}

/// What the migration `text`, the file at `path`, does.
pub fn read(path: &Path, text: String) -> Vec<Op> {
    if !text.contains("table(") {
        return Vec::new();
    }
    let parsed = ParsedFile::parse(&path.to_string_lossy(), text);
    let mut reader = Reader { parsed: &parsed, file: path, variables: HashMap::new(), undoing: false, out: Vec::new() };
    MigrationWalker.walk_program(parsed.program(), &mut reader);
    reader.out
}

/// The tables after `ops`, in order.
pub fn tables<'o>(ops: impl IntoIterator<Item = &'o Op>) -> Vec<Table> {
    let mut out: Vec<Table> = Vec::new();
    let at = |out: &mut Vec<Table>, name: &str| match out.iter().position(|table| table.name == name) {
        Some(at) => at,
        None => {
            out.push(Table { name: name.to_owned(), columns: Vec::new() });
            out.len() - 1
        }
    };
    for op in ops {
        match op {
            Op::Table { table, id } => {
                let fresh = !out.iter().any(|known| known.name == *table);
                let at = at(&mut out, table);
                // The key comes with the table, not with a later change of it.
                if fresh && let Some(id) = id {
                    out[at].columns.push(id.clone());
                }
            }
            Op::Add { table, column } => {
                let at = at(&mut out, table);
                out[at].columns.retain(|known| known.name != column.name);
                out[at].columns.push(column.clone());
            }
            Op::Remove { table, column } => {
                let at = at(&mut out, table);
                out[at].columns.retain(|known| known.name != *column);
            }
            Op::Rename { table, from, to } => {
                let at = at(&mut out, table);
                for column in out[at].columns.iter_mut().filter(|column| column.name == *from) {
                    column.name = to.clone();
                }
            }
            Op::Change { table, column, ty, nullable } => {
                let at = at(&mut out, table);
                for known in out[at].columns.iter_mut().filter(|known| known.name == *column) {
                    known.ty = *ty;
                    known.nullable = *nullable;
                }
            }
            Op::Drop(table) => out.retain(|known| known.name != *table),
            Op::RenameTable { from, to } => {
                let at = at(&mut out, from);
                out[at].name = to.clone();
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
}

impl TableCache {
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
        // Migrations run in the order of their names (`20240101120000_CreateArticles`).
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

/// The database table of the table class `table`: what `setTable()` names,
/// else the table's name in snake case (`PostCategoriesTable` is
/// `post_categories`).
pub fn database_table(resolver: &Resolver<'_>, table: &str) -> String {
    let set = classes::facts(resolver, table).iter().find_map(|facts| facts.table.clone());
    set.unwrap_or_else(|| underscore(classes::alias_of(table)))
}

/// The columns of the table of the entity class `entity`, with the
/// database table's name.
pub fn columns_of(resolver: &Resolver<'_>, entity: &str) -> Option<(String, Vec<Column>)> {
    let tables = resolver.cake_tables()?;
    if tables.is_empty() {
        return None;
    }
    let table = database_table(resolver, &classes::table_of(resolver, entity)?);
    let found = tables.iter().find(|known| known.name == table)?;
    Some((table, found.columns.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const CREATE: &str = "<?php\nuse Migrations\\AbstractMigration;\n\nclass CreateArticles extends AbstractMigration\n{\n    public function change(): void\n    {\n        $table = $this->table('articles');\n        $table->addColumn('title', 'string', ['limit' => 255, 'null' => false]);\n        $table->addColumn('body', 'text', ['null' => true])\n            ->addColumn('published', 'boolean', ['default' => false])\n            ->addColumn('created', 'datetime')\n            ->addColumn('old', 'string');\n        $table->create();\n        $this->table('tags', ['id' => false])->addColumn('label', 'string')->create();\n    }\n}\n";
    const CHANGE: &str = "<?php\nclass AlterArticles extends \\Migrations\\AbstractMigration\n{\n    public function up(): void\n    {\n        $this->table('articles')\n            ->addColumn('rating', 'float', ['null' => true])\n            ->removeColumn('old')\n            ->renameColumn('body', 'content')\n            ->changeColumn('published', 'integer')\n            ->update();\n        $this->table('gone')->create();\n        $this->table('gone')->drop()->save();\n        $this->table('tags')->rename('labels')->update();\n    }\n\n    public function down(): void\n    {\n        $this->table('articles')->removeColumn('rating')->update();\n    }\n}\n";

    #[test]
    fn migrations_make_the_tables_and_their_columns() {
        let mut ops = read(Path::new("1_CreateArticles.php"), CREATE.to_owned());
        ops.extend(read(Path::new("2_AlterArticles.php"), CHANGE.to_owned()));
        let tables = tables(&ops);
        let names: Vec<&str> = tables.iter().map(|table| table.name.as_str()).collect();
        assert_eq!(names, ["articles", "labels"]);
        let columns: Vec<(&str, ColumnType, bool)> =
            tables[0].columns.iter().map(|column| (column.name.as_str(), column.ty, column.nullable)).collect();
        assert_eq!(
            columns,
            [
                ("id", ColumnType::Int, false),
                ("title", ColumnType::String, false),
                ("content", ColumnType::String, true),
                ("published", ColumnType::Int, false),
                ("created", ColumnType::DateTime, false),
                ("rating", ColumnType::Float, true),
            ]
        );
        // A table without the key CakePHP adds; a column is where its migration names it.
        let labels: Vec<&str> = tables[1].columns.iter().map(|column| column.name.as_str()).collect();
        assert_eq!(labels, ["label"]);
        assert_eq!(tables[0].columns[1].range.start.line, 8);
        assert_eq!(tables[0].columns[5].file, Path::new("2_AlterArticles.php"));
    }
}
