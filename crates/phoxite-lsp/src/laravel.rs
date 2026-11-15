//! The Laravel requests beyond standard LSP: what a client needs for a routes
//! list, the Eloquent model view, the generators, the Artisan command list and
//! "Create translation key…". Each is a `phoxite/laravel/*` request.
//!
//! | Request | Params | Result |
//! | --- | --- | --- |
//! | `status` | | `{ laravel }` |
//! | `routes` | `query` | routes (as the route lenses carry them) |
//! | `modelView` | `class` | the model's table, columns, relations… |
//! | `addColumns` | `class`, `columns`, `fillable` | an edit |
//! | `artisanCommands` | | commands, or `null` |
//! | `generationContext` | | modules, models and generators, or `null` |
//! | `newModel` | `spec` | an edit |
//! | `generateClass` | `spec` | an edit |
//! | `createTranslation` | `key`, `texts` | an edit |
//! | `templates` | | the generators' templates |
//! | `refreshSchema` | | the database read again: `{ path, tables, columns }`, or `null` |
//!
//! Every request takes an optional `folder`: the URI of a workspace folder (or
//! of a file in one) choosing the project when several are open; without it
//! the first Laravel project answers. Places in files are `{ path, range }`
//! with `range.start.line` / `.col`, as in the routes of `laravel.routeActions`.
//! An edit is `{ edit, created }`: an LSP `WorkspaceEdit` for the client to
//! apply and the URIs of the files it creates. The requests making a migration
//! take an optional `timestamp` (`2024_05_01_120000`, the client's local time);
//! without it the server uses UTC.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use lsp_types::{self as lsp, Url};
use phoxite::base as e;
use phoxite::engine::PhpEngine;
use phoxite::frameworks::laravel::generate::{BUILTIN_TEMPLATES, GenerationContext, NewClass, NewModel, TemplateSet};
use phoxite::frameworks::laravel::model_view::{ModelLocation, ModelView, NewColumn};
use phoxite::frameworks::laravel::{ArtisanCommand, CommandSource, RouteInfo};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::convert;
use crate::handlers::Result;
use crate::state::State;

pub const STATUS: &str = "phoxite/laravel/status";
pub const ROUTES: &str = "phoxite/laravel/routes";
pub const MODEL_VIEW: &str = "phoxite/laravel/modelView";
pub const ADD_COLUMNS: &str = "phoxite/laravel/addColumns";
pub const ARTISAN_COMMANDS: &str = "phoxite/laravel/artisanCommands";
pub const GENERATION_CONTEXT: &str = "phoxite/laravel/generationContext";
pub const NEW_MODEL: &str = "phoxite/laravel/newModel";
pub const GENERATE_CLASS: &str = "phoxite/laravel/generateClass";
pub const CREATE_TRANSLATION: &str = "phoxite/laravel/createTranslation";
pub const TEMPLATES: &str = "phoxite/laravel/templates";
pub const REFRESH_SCHEMA: &str = "phoxite/laravel/refreshSchema";

/// Every request of this module.
pub const METHODS: &[&str] = &[
    STATUS,
    ROUTES,
    MODEL_VIEW,
    ADD_COLUMNS,
    ARTISAN_COMMANDS,
    GENERATION_CONTEXT,
    NEW_MODEL,
    GENERATE_CLASS,
    CREATE_TRANSLATION,
    TEMPLATES,
    REFRESH_SCHEMA,
];

/// The extension of an edited template's file: `Laravel Model.ft`.
pub const TEMPLATE_EXTENSION: &str = "ft";

const NOT_LARAVEL: &str = "Not a Laravel project, or its index isn't ready yet";

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct Params {
    folder: Option<Url>,
    query: String,
    class: String,
    columns: Vec<NewColumn>,
    fillable: bool,
    timestamp: Option<String>,
    key: String,
    texts: Vec<TranslationText>,
    spec: Value,
}

#[derive(Debug, Deserialize)]
struct TranslationText {
    locale: String,
    text: String,
}

/// An edit for the client to apply.
#[derive(Debug, Serialize)]
struct Applied {
    edit: lsp::WorkspaceEdit,
    /// The files the edit creates, in order.
    created: Vec<Url>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ArtisanArgumentInfo {
    name: String,
    description: String,
    required: bool,
    array: bool,
    default: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ArtisanOptionInfo {
    name: String,
    shortcut: Option<String>,
    description: String,
    accepts_value: bool,
    array: bool,
    default: Option<String>,
}

/// An Artisan command.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ArtisanCommandInfo {
    name: String,
    description: String,
    /// `posts:publish [--dry-run] <post>`.
    synopsis: String,
    arguments: Vec<ArtisanArgumentInfo>,
    options: Vec<ArtisanOptionInfo>,
    /// `class` (the project's), `closure` (`routes/console.php`), `package` or `framework`.
    source: &'static str,
    class: Option<String>,
    location: Option<ModelLocation>,
}

impl From<&ArtisanCommand> for ArtisanCommandInfo {
    fn from(command: &ArtisanCommand) -> Self {
        Self {
            name: command.name.clone(),
            description: command.description.clone(),
            synopsis: command.synopsis(),
            arguments: command
                .arguments
                .iter()
                .map(|a| ArtisanArgumentInfo {
                    name: a.name.clone(),
                    description: a.description.clone(),
                    required: a.required,
                    array: a.array,
                    default: a.default.clone(),
                })
                .collect(),
            options: command
                .options
                .iter()
                .map(|o| ArtisanOptionInfo {
                    name: o.name.clone(),
                    shortcut: o.shortcut.clone(),
                    description: o.description.clone(),
                    accepts_value: o.accepts_value,
                    array: o.array,
                    default: o.default.clone(),
                })
                .collect(),
            source: match command.source {
                CommandSource::Class => "class",
                CommandSource::Closure => "closure",
                CommandSource::Package => "package",
                CommandSource::Framework => "framework",
            },
            class: command.class.clone(),
            location: match (&command.path, command.range) {
                (Some(path), Some(range)) => Some(ModelLocation { path: path.clone(), range }),
                _ => None,
            },
        }
    }
}

/// A generator template: its default body, and the user's when edited.
#[derive(Debug, Serialize)]
struct TemplateInfo {
    name: &'static str,
    /// Where the user's version is (or would be).
    path: PathBuf,
    body: String,
    edited: bool,
}

/// The engine of the Laravel project `folder` is in, else of the first
/// Laravel project.
fn project(state: &State, folder: &Option<Url>) -> Option<(PathBuf, Arc<PhpEngine>)> {
    if let Some(path) = folder.as_ref().and_then(convert::path_of)
        && let Some((root, engine)) = state.workspace_of(&path)
        && engine.laravel().is_some()
    {
        return Some((root, engine));
    }
    let workspaces = state.workspaces.read();
    workspaces.iter().find(|w| w.engine.laravel().is_some()).map(|w| (w.root.clone(), w.engine.clone()))
}

/// The folder of the edited templates of the project at `root`.
pub fn templates_dir(state: &State, root: &Path) -> PathBuf {
    let configured = state.settings.read().templates.path.trim().to_owned();
    let relative = if configured.is_empty() { crate::config::DEFAULT_TEMPLATES_PATH } else { configured.as_str() };
    root.join(relative)
}

/// The templates the user edited (`<folder>/<name>.ft`) over the defaults.
pub fn edited_templates(dir: &Path) -> TemplateSet {
    let edited = BUILTIN_TEMPLATES.iter().filter_map(|template| {
        let body = std::fs::read_to_string(template_file(dir, template.name)).ok()?;
        Some((template.name.to_owned(), body.replace("\r\n", "\n")))
    });
    TemplateSet::new(edited)
}

fn template_file(dir: &Path, name: &str) -> PathBuf {
    dir.join(format!("{name}.{TEMPLATE_EXTENSION}"))
}

/// Reads the edited templates again and gives them to the engine (the
/// Generate actions inside a model class render them without a request).
pub fn load_templates(state: &State, root: &Path, engine: &PhpEngine) -> TemplateSet {
    let templates = edited_templates(&templates_dir(state, root));
    engine.set_laravel_templates(templates.clone());
    templates
}

/// `2024_05_01_120000` for now, in UTC.
fn utc_timestamp() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    timestamp_of(secs)
}

/// The migration prefix of `secs` after the Unix epoch.
fn timestamp_of(secs: u64) -> String {
    let (days, rest) = (secs / 86_400, secs % 86_400);
    // Civil date from days (Howard Hinnant's algorithm).
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}_{month:02}_{day:02}_{:02}{:02}{:02}", rest / 3_600, rest % 3_600 / 60, rest % 60)
}

/// A client's timestamp when it is one, else now.
fn timestamp(given: Option<String>) -> String {
    given.filter(|t| t.len() == 17 && t.bytes().all(|b| b.is_ascii_digit() || b == b'_')).unwrap_or_else(utc_timestamp)
}

fn applied(state: &State, edit: e::WorkspaceEdit) -> Applied {
    let created = edit
        .files
        .iter()
        .filter_map(|file| match file {
            e::FileEdit::Create { path, .. } => convert::uri_of(path),
            _ => None,
        })
        .collect();
    Applied { edit: convert::workspace_edit(edit, state.support.edits), created }
}

fn json<T: Serialize>(value: T) -> Result<Value> {
    serde_json::to_value(value).map_err(|e| e.to_string())
}

fn spec<T: serde::de::DeserializeOwned>(value: Value) -> Result<T> {
    serde_json::from_value(value).map_err(|e| format!("invalid spec: {e}"))
}

/// Answers request `method` of this module.
pub fn request(state: &State, method: &str, params: Value) -> Result<Value> {
    let p: Params = if params.is_null() {
        Params::default()
    } else {
        serde_json::from_value(params).map_err(|e| format!("invalid params: {e}"))?
    };
    let found = project(state, &p.folder);
    match method {
        STATUS => json(serde_json::json!({ "laravel": found.is_some() })),
        ROUTES => {
            let routes: Vec<RouteInfo> =
                found.and_then(|(_, engine)| engine.laravel_route_list(&p.query)).unwrap_or_default();
            json(routes)
        }
        ARTISAN_COMMANDS => {
            let commands: Option<Vec<ArtisanCommandInfo>> = found
                .and_then(|(_, engine)| engine.laravel_artisan())
                .map(|commands| commands.commands.iter().map(ArtisanCommandInfo::from).collect());
            json(commands)
        }
        GENERATION_CONTEXT => {
            let context: Option<GenerationContext> = found.and_then(|(_, engine)| engine.laravel_generation_context());
            json(context)
        }
        _ => {
            let (root, engine) = found.ok_or(NOT_LARAVEL)?;
            match method {
                REFRESH_SCHEMA => json(crate::schema::refresh(state, &root, &engine)?),
                MODEL_VIEW => {
                    let view: ModelView = engine
                        .laravel_model_view(&p.class)
                        .ok_or_else(|| format!("'{}' is not an Eloquent model", p.class))?;
                    json(view)
                }
                ADD_COLUMNS => {
                    let edit = engine.laravel_add_columns(&p.class, &p.columns, p.fillable, &timestamp(p.timestamp))?;
                    json(applied(state, edit))
                }
                NEW_MODEL => {
                    let spec: NewModel = spec(p.spec)?;
                    let templates = load_templates(state, &root, &engine);
                    let edit = engine.laravel_new_model(&spec, &templates, &timestamp(p.timestamp))?;
                    json(applied(state, edit))
                }
                GENERATE_CLASS => {
                    let spec: NewClass = spec(p.spec)?;
                    let templates = load_templates(state, &root, &engine);
                    let edit = engine.laravel_generate_class(&spec, &templates, &timestamp(p.timestamp))?;
                    json(applied(state, edit))
                }
                CREATE_TRANSLATION => {
                    let texts: Vec<(String, String)> = p.texts.into_iter().map(|t| (t.locale, t.text)).collect();
                    let edit = engine
                        .laravel_create_translation(&p.key, &texts)
                        .filter(|edit| !edit.files.is_empty())
                        .ok_or_else(|| format!("The translation key '{}' can't be added", p.key))?;
                    json(applied(state, edit))
                }
                TEMPLATES => {
                    let dir = templates_dir(state, &root);
                    let templates: Vec<TemplateInfo> = BUILTIN_TEMPLATES
                        .iter()
                        .map(|template| {
                            let path = template_file(&dir, template.name);
                            let edited = std::fs::read_to_string(&path).ok();
                            TemplateInfo {
                                name: template.name,
                                path,
                                edited: edited.is_some(),
                                body: edited.unwrap_or_else(|| template.body.to_owned()),
                            }
                        })
                        .collect();
                    json(templates)
                }
                other => Err(format!("unknown request {other}")),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamps_are_migration_prefixes() {
        assert_eq!(timestamp_of(0), "1970_01_01_000000");
        // 2024-05-01 12:00:00 UTC.
        assert_eq!(timestamp_of(1_714_564_800), "2024_05_01_120000");
        // The last second of a leap day.
        assert_eq!(timestamp_of(1_709_251_199), "2024_02_29_235959");
        assert_eq!(timestamp(Some("2024_05_01_120000".into())), "2024_05_01_120000");
        assert_ne!(timestamp(Some("../../x".into())), "../../x");
    }

    #[test]
    fn edited_templates_replace_the_defaults() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Laravel Enum.ft"), "<?php // mine\r\n").unwrap();
        let templates = edited_templates(dir.path());
        assert_eq!(templates.body("Laravel Enum"), Some("<?php // mine\n"));
        let default = BUILTIN_TEMPLATES.iter().find(|t| t.name == "Laravel Model").unwrap().body;
        assert_eq!(templates.body("Laravel Model"), Some(default));
    }
}
