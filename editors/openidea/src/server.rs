//! Asking phoxite-lsp: its `phoxite/laravel/*` requests (`docs/editors.md`).

use openidea_plugin::{Error, Invocation, Project};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The server's name in OpenIDEA's `languages/php.toml`.
pub const SERVER: &str = "phoxite";

/// A place in a file: 0-based line and UTF-16 column.
#[derive(Deserialize, Serialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pos {
    pub line: u32,
    pub col: u32,
}

#[derive(Deserialize, Clone, Copy, Debug)]
pub struct Range {
    pub start: Pos,
}

/// A place the server names: a file and a range in it.
#[derive(Deserialize, Clone, Debug)]
pub struct Place {
    pub path: String,
    pub range: Range,
}

/// Where a tree item or a pick leads: its key, as JSON.
#[derive(Deserialize, Serialize)]
pub struct Target {
    pub path: String,
    pub at: Pos,
}

impl Target {
    pub fn key(path: &str, at: Pos) -> String {
        serde_json::to_string(&Target { path: path.to_owned(), at }).unwrap_or_default()
    }

    pub fn of(place: &Place) -> String {
        Self::key(&place.path, place.range.start)
    }
}

/// Opens the place a key names; a key that names none (a group's node) does nothing.
pub fn open(key: &str) -> Result<(), Error> {
    let Ok(target) = serde_json::from_str::<Target>(key) else { return Ok(()) };
    openidea_plugin::host::open_location(&target.path, target.at.line, target.at.col)
}

pub fn project(ctx: &Invocation) -> Result<Project, Error> {
    ctx.project().ok_or_else(|| Error::Invalid("Open a Laravel project first".into()))
}

/// Sends `phoxite/laravel/<name>` for the invoking project and reads the answer.
pub fn request<T: DeserializeOwned>(ctx: &Invocation, name: &str, params: Value) -> Result<T, Error> {
    request_in(&project(ctx)?, name, params)
}

pub fn request_in<T: DeserializeOwned>(project: &Project, name: &str, params: Value) -> Result<T, Error> {
    let method = format!("phoxite/laravel/{name}");
    let answer = project.language_server_request(SERVER, &method, Some(&params.to_string())).map_err(|e| match e {
        Error::NotFound(_) => Error::NotFound(
            "Phoxite is not running for this project: choose it for PHP in Settings › Languages & Frameworks › \
             Language Servers"
                .into(),
        ),
        e => e,
    })?;
    serde_json::from_str(&answer).map_err(|e| Error::Failed(format!("{method}: {e}")))
}

/// The arguments of a command the server handed to the client, from their JSON.
pub fn command_arguments(arg: Option<&str>, command: &str) -> Result<Vec<Value>, Error> {
    serde_json::from_str(arg.unwrap_or("[]")).map_err(|e| Error::Invalid(format!("{command}: {e}")))
}

/// `App\Models\Post` → `Post`.
pub fn short_name(class: &str) -> &str {
    class.rsplit('\\').next().unwrap_or(class)
}

/// `path` as a view shows it: relative to the project.
pub fn relative<'a>(path: &'a str, root: Option<&str>) -> &'a str {
    root.and_then(|root| path.strip_prefix(root))
        .map(|rest| rest.trim_start_matches(['/', '\\']))
        .filter(|rest| !rest.is_empty())
        .unwrap_or(path)
}
