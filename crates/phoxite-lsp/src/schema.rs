//! The live database schema: the tables and columns of the project's SQLite
//! database, given to the engine so Eloquent attributes, the query builder and
//! the validation rules know the columns the database really has (with the
//! migrations, the database winning).
//!
//! The setting `laravel.database` chooses the database:
//!
//! - empty: none, the migrations alone;
//! - `auto`: the project's own, when its `.env` says `DB_CONNECTION=sqlite`
//!   (`DB_DATABASE`, else `database/database.sqlite`);
//! - anything else: the path of a SQLite file, relative to the project.
//!
//! The file is opened read-only. It is read when the project opens, when the
//! setting changes, when the client reports the file or `.env` changed, and on
//! `phoxite/laravel/refreshSchema`.

use std::path::{Path, PathBuf};

use phoxite::engine::PhpEngine;
use phoxite::frameworks::laravel::route_hints::env_value;
use phoxite::frameworks::laravel::{LiveColumn, LiveSchema, LiveTable};
use rusqlite::{Connection, OpenFlags};
use serde::Serialize;

use crate::state::State;

/// The setting's value for the database of the project's `.env`.
pub const AUTO: &str = "auto";

/// Where Laravel keeps a SQLite database unless `DB_DATABASE` says.
const DEFAULT_DATABASE: &str = "database/database.sqlite";

/// What a refresh found.
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    /// The database file.
    pub path: PathBuf,
    pub tables: usize,
    pub columns: usize,
}

/// The SQLite file the setting `database` names for the project at `root`;
/// `None` when it names none (or the project uses another kind of database).
pub fn database_path(root: &Path, database: &str) -> Option<PathBuf> {
    let database = database.trim();
    let named = match database {
        "" => return None,
        AUTO => {
            let env = std::fs::read_to_string(root.join(".env")).ok()?;
            if env_value(&env, "DB_CONNECTION")?.to_lowercase() != "sqlite" {
                return None;
            }
            // `DB_DATABASE=:memory:` has no file to read.
            env_value(&env, "DB_DATABASE").filter(|path| path != ":memory:").unwrap_or_else(|| DEFAULT_DATABASE.into())
        }
        path => path.to_owned(),
    };
    let path = PathBuf::from(named);
    Some(if path.is_absolute() { path } else { root.join(path) })
}

/// The tables, views and columns of the SQLite database at `path`.
pub fn read(path: &Path) -> Result<LiveSchema, String> {
    if !path.is_file() {
        return Err(format!("{} doesn't exist", path.display()));
    }
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let db = Connection::open_with_flags(path, flags).map_err(|e| format!("{}: {e}", path.display()))?;
    let failed = |e: rusqlite::Error| format!("{}: {e}", path.display());
    let names: Vec<String> = db
        .prepare(
            "SELECT name FROM sqlite_master WHERE type IN ('table', 'view') AND name NOT LIKE 'sqlite_%' ORDER BY name",
        )
        .and_then(|mut found| found.query_map([], |row| row.get(0))?.collect())
        .map_err(failed)?;
    let mut tables = Vec::new();
    for name in names {
        // name, type, notnull, pk of each column (`table_info` takes no bound parameter as a pragma).
        let columns: Vec<LiveColumn> = db
            .prepare("SELECT name, type, \"notnull\", pk FROM pragma_table_info(?1)")
            .and_then(|mut found| {
                found
                    .query_map([&name], |row| {
                        let (not_null, key): (bool, i64) = (row.get(2)?, row.get(3)?);
                        Ok(LiveColumn { name: row.get(0)?, data_type: row.get(1)?, nullable: !not_null && key == 0 })
                    })?
                    .collect()
            })
            .map_err(failed)?;
        tables.push(LiveTable { name, columns });
    }
    let source = path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
    Ok(LiveSchema { source, tables })
}

/// Reads the database of the project at `root` again and gives it to its
/// engine. `Ok(None)` when the setting names no database (the engine goes
/// back to the migrations alone); an error leaves the engine without one too.
pub fn refresh(state: &State, root: &Path, engine: &PhpEngine) -> Result<Option<Status>, String> {
    let database = state.settings.read().laravel.database.clone();
    let Some(path) = database_path(root, &database) else {
        engine.set_live_schema(None);
        return Ok(None);
    };
    match read(&path) {
        Ok(schema) => {
            let status = Status {
                path,
                tables: schema.tables.len(),
                columns: schema.tables.iter().map(|t| t.columns.len()).sum(),
            };
            engine.set_live_schema(Some(schema));
            Ok(Some(status))
        }
        Err(e) => {
            engine.set_live_schema(None);
            Err(e)
        }
    }
}

/// [`refresh`], with a failure only logged (when nobody asked).
pub fn refresh_quietly(state: &State, root: &Path, engine: &PhpEngine) {
    match refresh(state, root, engine) {
        Ok(Some(status)) => tracing::info!(
            "database schema of {}: {} tables, {} columns",
            status.path.display(),
            status.tables,
            status.columns
        ),
        Ok(None) => {}
        Err(e) => tracing::warn!("database schema: {e}"),
    }
}

/// Whether a changed file means the schema of the project at `root` may have
/// changed: its `.env`, or the database file (or its journal).
pub fn concerns(state: &State, root: &Path, changed: &Path) -> bool {
    let database = state.settings.read().laravel.database.clone();
    if database.trim().is_empty() {
        return false;
    }
    // Clients and the file system may spell one path differently on Windows.
    let spelled = |path: &Path| {
        let text = dunce::simplified(path).to_string_lossy().replace('\\', "/");
        if cfg!(windows) { text.to_lowercase() } else { text }
    };
    let changed = spelled(changed);
    if database.trim() == AUTO && changed == spelled(&root.join(".env")) {
        return true;
    }
    database_path(root, &database).is_some_and(|path| {
        let path = spelled(&path);
        changed == path || changed.strip_prefix(path.as_str()).is_some_and(|rest| rest == "-wal" || rest == "-journal")
    })
}
