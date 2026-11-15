//! The refactorings that ask the user something first: Change Signature, the
//! Extract Method dialog, the Generate actions with a member chooser, and Pull
//! Members Up / Push Members Down.
//!
//! The first three are code actions carrying a client command (their arguments:
//! the document's URI and the action's data). The client shows its dialog, puts
//! the answer in the data and sends it back in a `phoxite/refactor/*` request,
//! which returns the edit to apply. They are offered only to a client declaring
//! `experimental.phoxite.dialogs` in its capabilities; other clients get the
//! plain actions (Generate getters for every property, Extract method with a
//! default name).
//!
//! | Request | Params | Result |
//! | --- | --- | --- |
//! | `changeSignature` | `uri`, `data` (with `change`) | `{ edit }` |
//! | `extractMethod` | `uri`, `data` (with `change`) | `{ edit }` |
//! | `generate` | `uri`, `data` (with `chosen`) | `{ edit }` |
//! | `hierarchyMembers` | `uri`, `position`, `kind` | the class, its targets and members |
//! | `hierarchy` | `uri`, `position`, `kind`, `target`, `choices` | `{ problems, edit }` |
//!
//! `kind` is `pullUp` or `pushDown`. The edit of `hierarchy` is absent when a
//! problem is fatal; the client confirms the others before applying it.

use lsp_types::{self as lsp, Url};
use phoxite::base::{self as e, CancellationToken};
use phoxite::engine::PhpEngine;
use phoxite::imports::ImportAction;
use phoxite::refactor::hierarchy::{HierarchyChoice, HierarchyInfo, HierarchyKind, HierarchyProblem};
use phoxite::refactor::{
    CHANGE_SIGNATURE, ChangeSignature, EXTRACT_METHOD_DIALOG, ExtractMethod, GENERATE_CHOOSER, Generator,
    MemberChooser, OVERRIDE_METHODS,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::convert;
use crate::handlers::Result;
use crate::state::{DocSnapshot, Kind, State};

pub const CHANGE_SIGNATURE_REQUEST: &str = "phoxite/refactor/changeSignature";
pub const EXTRACT_METHOD_REQUEST: &str = "phoxite/refactor/extractMethod";
pub const GENERATE_REQUEST: &str = "phoxite/refactor/generate";
pub const HIERARCHY_MEMBERS_REQUEST: &str = "phoxite/refactor/hierarchyMembers";
pub const HIERARCHY_REQUEST: &str = "phoxite/refactor/hierarchy";

/// Every request of this module.
pub const METHODS: &[&str] =
    &[CHANGE_SIGNATURE_REQUEST, EXTRACT_METHOD_REQUEST, GENERATE_REQUEST, HIERARCHY_MEMBERS_REQUEST, HIERARCHY_REQUEST];

/// Client command: the Change Signature dialog. Arguments: the document's URI
/// and a [`ChangeSignature`].
pub const CHANGE_SIGNATURE_COMMAND: &str = "phoxite.changeSignature";
/// Client command: the Extract Method dialog. Arguments: the URI and an
/// [`ExtractMethod`].
pub const EXTRACT_METHOD_COMMAND: &str = "phoxite.extractMethod";
/// Client command: a member chooser. Arguments: the URI and a [`MemberChooser`].
pub const GENERATE_COMMAND: &str = "phoxite.generateMembers";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DataParams {
    uri: Url,
    data: Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HierarchyParams {
    uri: Url,
    position: lsp::Position,
    kind: HierarchyKind,
    #[serde(default)]
    target: Option<String>,
    #[serde(default)]
    choices: Vec<HierarchyChoice>,
}

#[derive(Debug, Serialize)]
struct Edit {
    edit: lsp::WorkspaceEdit,
}

#[derive(Debug, Serialize)]
struct Moved {
    problems: Vec<HierarchyProblem>,
    edit: Option<lsp::WorkspaceEdit>,
}

fn wants(only: &[String], kind: &str) -> bool {
    only.is_empty()
        || only.iter().any(|o| kind == o || kind.strip_prefix(o.as_str()).is_some_and(|rest| rest.starts_with('.')))
}

fn action(title: String, kind: &str, command: &str, uri: &Url, data: Value) -> lsp::CodeActionOrCommand {
    lsp::CodeActionOrCommand::CodeAction(lsp::CodeAction {
        title: title.clone(),
        kind: Some(lsp::CodeActionKind::from(kind.to_owned())),
        command: Some(lsp::Command {
            title,
            command: command.to_owned(),
            arguments: Some(vec![Value::String(uri.to_string()), data]),
        }),
        ..lsp::CodeAction::default()
    })
}

/// The actions opening a dialog at `at` of a PHP document. A member chooser
/// replaces the plain action it refines in `plain` (`Generate getters` for
/// `Generate getters…`, the single overrides for `Override methods…`).
pub fn dialog_actions(
    engine: &PhpEngine,
    uri: &Url,
    doc: e::DocId,
    at: e::Range,
    only: &[String],
    plain: &mut Vec<ImportAction>,
) -> Vec<lsp::CodeActionOrCommand> {
    let mut actions = Vec::new();
    if wants(only, GENERATE_CHOOSER) {
        for chooser in engine.generate_choosers(doc, at).unwrap_or_default() {
            let Ok(data) = serde_json::to_value(&chooser) else { continue };
            let refined = chooser.title.trim_end_matches('…');
            let overrides = chooser.generator == Generator::Overrides;
            plain.retain(|a| a.title != refined && !(overrides && a.kind == OVERRIDE_METHODS));
            actions.push(action(chooser.title, GENERATE_CHOOSER, GENERATE_COMMAND, uri, data));
        }
    }
    if wants(only, CHANGE_SIGNATURE)
        && let Some(info) = engine.change_signature_info(doc, at)
        && let Ok(data) = serde_json::to_value(&info)
    {
        let title = format!("Change signature of '{}'…", info.signature.name);
        actions.push(action(title, CHANGE_SIGNATURE, CHANGE_SIGNATURE_COMMAND, uri, data));
    }
    if wants(only, EXTRACT_METHOD_DIALOG)
        && at.start != at.end
        && let Some(info) = engine.extract_method_info(doc, at)
        && let Ok(data) = serde_json::to_value(&info)
    {
        let title = if info.function { "Extract function…" } else { "Extract method…" };
        actions.push(action(title.to_owned(), EXTRACT_METHOD_DIALOG, EXTRACT_METHOD_COMMAND, uri, data));
    }
    actions
}

fn php(state: &State, uri: &Url) -> Result<(DocSnapshot, std::sync::Arc<PhpEngine>)> {
    let doc = state.snapshot(uri).ok_or_else(|| format!("{uri} is not open"))?;
    match (doc.kind, doc.engine.clone()) {
        (Kind::Php, Some(engine)) => Ok((doc, engine)),
        _ => Err("Not a PHP file of an indexed project".into()),
    }
}

fn parse<T: serde::de::DeserializeOwned>(value: Value) -> Result<T> {
    serde_json::from_value(value).map_err(|e| format!("The request is malformed: {e}"))
}

fn edit(state: &State, found: Option<e::IdeResult<e::WorkspaceEdit>>) -> Result<Value> {
    let edit = found.ok_or("The document is not open")?.map_err(|e| e.message)?;
    json(Edit { edit: convert::workspace_edit(edit, state.support.edits) })
}

fn json<T: Serialize>(value: T) -> Result<Value> {
    serde_json::to_value(value).map_err(|e| e.to_string())
}

/// Answers request `method` of [`METHODS`].
pub fn request(state: &State, method: &str, params: Value, cancel: &CancellationToken) -> Result<Value> {
    match method {
        CHANGE_SIGNATURE_REQUEST => {
            let p: DataParams = parse(params)?;
            let (doc, engine) = php(state, &p.uri)?;
            let data: ChangeSignature = parse(p.data)?;
            let change = data.change.as_ref().ok_or("Choose the new signature first")?;
            edit(state, engine.change_signature(doc.id, data.position, change, cancel))
        }
        EXTRACT_METHOD_REQUEST => {
            let p: DataParams = parse(params)?;
            let (doc, engine) = php(state, &p.uri)?;
            let data: ExtractMethod = parse(p.data)?;
            edit(state, engine.extract_method_edit(doc.id, &data))
        }
        GENERATE_REQUEST => {
            let p: DataParams = parse(params)?;
            let (doc, engine) = php(state, &p.uri)?;
            let data: MemberChooser = parse(p.data)?;
            edit(state, engine.generate_chosen(doc.id, &data))
        }
        HIERARCHY_MEMBERS_REQUEST => {
            let p: HierarchyParams = parse(params)?;
            let (doc, engine) = php(state, &p.uri)?;
            let info: HierarchyInfo = engine
                .hierarchy_members(doc.id, convert::pos(p.position), p.kind, cancel)
                .ok_or("The document is not open")?
                .map_err(|e| e.message)?;
            json(info)
        }
        HIERARCHY_REQUEST => {
            let p: HierarchyParams = parse(params)?;
            let (doc, engine) = php(state, &p.uri)?;
            let moved = engine
                .hierarchy_refactor(doc.id, convert::pos(p.position), p.kind, p.target.as_deref(), &p.choices, cancel)
                .ok_or("The document is not open")?
                .map_err(|e| e.message)?;
            json(Moved {
                problems: moved.problems,
                edit: moved.edit.map(|edit| convert::workspace_edit(edit, state.support.edits)),
            })
        }
        _ => Err(format!("unknown request {method}")),
    }
}
