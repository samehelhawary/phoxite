//! Commands: those the client runs (route actions, the model view, running
//! an Artisan command, the "Create translation key…" dialog, opening a file
//! after a quick-fix) and those the server runs (`workspace/executeCommand`).

use lsp_types as lsp;
use phoxite::frameworks::laravel::RouteInfo;
use phoxite::frameworks::laravel::refs::translation_key::{
    CREATE_TRANSLATION_COMMAND, TranslationRequest, create_translation_title,
};
use phoxite::frameworks::laravel::route_hints::ROUTE_ACTIONS_COMMAND;
use serde_json::Value;

/// Client command: opens a location (`lsp::Location`), as after the quick-fix
/// creating a view.
pub const OPEN_LOCATION: &str = "phoxite.openLocation";

/// Client command: runs the Artisan command its one argument names
/// (`posts:publish`), as the lens on a console command class offers.
pub const RUN_ARTISAN: &str = "laravel.runArtisan";

/// Server command: indexes the workspace folders again. Not `phoxite.rescan`:
/// LSP clients register server commands as editor commands, and the VS Code
/// extension has its own `phoxite.rescan`.
pub const RESCAN: &str = "phoxite.server.rescan";

/// The commands the server runs.
pub const SERVER_COMMANDS: &[&str] = &[RESCAN];

/// The client command offering the actions on `routes` (go to the
/// declaration, copy or open the URL): its argument is their JSON.
pub fn route_actions(title: String, routes: &[RouteInfo]) -> Option<lsp::Command> {
    let json = serde_json::to_string(routes).ok()?;
    Some(lsp::Command { title, command: ROUTE_ACTIONS_COMMAND.to_owned(), arguments: Some(vec![Value::String(json)]) })
}

/// The lens running Artisan command `name`.
pub fn run_artisan(name: &str) -> lsp::Command {
    lsp::Command {
        title: format!("Run artisan {name}"),
        command: RUN_ARTISAN.to_owned(),
        arguments: Some(vec![Value::String(name.to_owned())]),
    }
}

/// The action "Create translation key…": the client asks for the text of
/// each locale of `request` (its argument, as JSON), then sends
/// `phoxite/laravel/createTranslation`.
pub fn create_translation(request: &TranslationRequest) -> Option<lsp::CodeActionOrCommand> {
    let json = serde_json::to_string(request).ok()?;
    let title = create_translation_title(&request.key);
    Some(lsp::CodeActionOrCommand::CodeAction(lsp::CodeAction {
        title: title.clone(),
        kind: Some(lsp::CodeActionKind::from(phoxite::imports::REWRITE.to_owned())),
        command: Some(lsp::Command {
            title,
            command: CREATE_TRANSLATION_COMMAND.to_owned(),
            arguments: Some(vec![Value::String(json)]),
        }),
        ..lsp::CodeAction::default()
    }))
}
