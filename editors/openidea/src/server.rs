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

    /// A key for item `n` of a tree. Several items can lead to one place (the routes of
    /// a `Route::resource`, the columns of `$table->timestamps()`), and a tree's keys must
    /// differ: the number after the `#` tells them apart and [`open`] ignores it.
    pub fn nth(path: &str, at: Pos, n: usize) -> String {
        format!("{}#{n}", Self::key(path, at))
    }
}

/// Opens the place a key names; a key that names none (a group's node) does nothing.
pub fn open(key: &str) -> Result<(), Error> {
    let place = key.rfind('}').map_or(key, |end| &key[..=end]);
    let Ok(target) = serde_json::from_str::<Target>(place) else { return Ok(()) };
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

/// `path` as a view shows it: relative to the project, with `/` between its parts.
/// The server and the IDE don't agree on the separator on Windows (`C:\app\a.php`
/// under the root `C:/app`), so both are compared with `/`.
pub fn relative(path: &str, root: Option<&str>) -> String {
    let slashes = |p: &str| p.replace('\\', "/");
    let path = slashes(path);
    let Some(root) = root.map(slashes) else { return path };
    let root = root.trim_end_matches('/');
    // Drive letters differ in case between the two (`c:` and `C:`).
    let inside = path.len() > root.len()
        && path.is_char_boundary(root.len())
        && path[..root.len()].eq_ignore_ascii_case(root)
        && path[root.len()..].starts_with('/');
    if inside { path[root.len() + 1..].to_owned() } else { path }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_are_shown_relative_to_the_project_whatever_the_separators() {
        assert_eq!(relative("/project/routes/web.php", Some("/project")), "routes/web.php");
        assert_eq!(relative("C:\\app\\routes\\web.php", Some("C:/app")), "routes/web.php");
        assert_eq!(relative("c:\\app\\routes\\web.php", Some("C:\\app\\")), "routes/web.php");
        // Another folder that only starts the same, and a file outside the project.
        assert_eq!(relative("/project-two/a.php", Some("/project")), "/project-two/a.php");
        assert_eq!(relative("/elsewhere/a.php", Some("/project")), "/elsewhere/a.php");
        assert_eq!(relative("C:\\x\\a.php", None), "C:/x/a.php");
    }

    #[test]
    fn a_numbered_key_opens_the_same_place() {
        let mock = crate::tests::mock();
        let at = Pos { line: 3, col: 4 };
        assert_ne!(Target::nth("/project/a#b.php", at, 0), Target::nth("/project/a#b.php", at, 1));
        open(&Target::nth("/project/a#b.php", at, 7)).unwrap();
        open(&Target::key("/project/a#b.php", at)).unwrap();
        open("group:columns").unwrap();
        assert_eq!(mock.opened_locations(), vec![("/project/a#b.php".to_owned(), 3, 4); 2]);
    }
}
