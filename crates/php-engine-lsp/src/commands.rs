//! Commands: those the client runs (route actions, the model view, opening
//! a file after a quick-fix) and those the server runs
//! (`workspace/executeCommand`).

use lsp_types as lsp;
use php_engine::frameworks::laravel::RouteInfo;
use php_engine::frameworks::laravel::route_hints::ROUTE_ACTIONS_COMMAND;
use serde_json::Value;

/// Client command: opens a location (`lsp::Location`), as after the quick-fix
/// creating a view.
pub const OPEN_LOCATION: &str = "phpEngine.openLocation";

/// Server command: indexes the workspace folders again.
pub const RESCAN: &str = "phpEngine.rescan";

/// The commands the server runs.
pub const SERVER_COMMANDS: &[&str] = &[RESCAN];

/// The client command offering the actions on `routes` (go to the
/// declaration, copy or open the URL): its argument is their JSON.
pub fn route_actions(title: String, routes: &[RouteInfo]) -> Option<lsp::Command> {
    let json = serde_json::to_string(routes).ok()?;
    Some(lsp::Command { title, command: ROUTE_ACTIONS_COMMAND.to_owned(), arguments: Some(vec![Value::String(json)]) })
}
