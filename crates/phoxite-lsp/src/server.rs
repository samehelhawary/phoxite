//! The server loop: initialization, the client's notifications (handled in
//! order on this thread), requests (each on its own thread, cancellable),
//! and the engine's events (diagnostics, indexing progress).

use std::collections::HashMap;
use std::sync::Arc;

use crossbeam_channel::{Receiver, select};
use lsp_server::{Connection, ErrorCode, Message, Notification, Request, RequestId, Response};
use lsp_types::notification::{self as n, Notification as _};
use lsp_types::request::{self as r, Request as _};
use lsp_types::{self as lsp, Url};
use parking_lot::Mutex;
use phoxite::base::{self as e, CancellationToken};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::analyzers;
use crate::commands;
use crate::config::{InitOptions, Settings, default_cache_dir};
use crate::convert::{self, EditSupport};
use crate::handlers::{self as h, Goto};
use crate::laravel;
use crate::refactor;
use crate::state::{ClientSupport, State};

const INDEXING_TOKEN: &str = "phoxite/indexing";

pub fn capabilities() -> Value {
    let php_files = lsp::FileOperationRegistrationOptions {
        filters: vec![
            lsp::FileOperationFilter {
                scheme: Some("file".into()),
                pattern: lsp::FileOperationPattern {
                    glob: "**/*.php".into(),
                    matches: Some(lsp::FileOperationPatternKind::File),
                    options: None,
                },
            },
            lsp::FileOperationFilter {
                scheme: Some("file".into()),
                pattern: lsp::FileOperationPattern {
                    glob: "**".into(),
                    matches: Some(lsp::FileOperationPatternKind::Folder),
                    options: None,
                },
            },
        ],
    };
    let caps = lsp::ServerCapabilities {
        position_encoding: Some(lsp::PositionEncodingKind::UTF16),
        text_document_sync: Some(lsp::TextDocumentSyncCapability::Options(lsp::TextDocumentSyncOptions {
            open_close: Some(true),
            change: Some(lsp::TextDocumentSyncKind::INCREMENTAL),
            // PHPStan and Psalm run on the saved file.
            save: Some(lsp::TextDocumentSyncSaveOptions::Supported(true)),
            ..lsp::TextDocumentSyncOptions::default()
        })),
        completion_provider: Some(lsp::CompletionOptions {
            resolve_provider: Some(true),
            trigger_characters: Some(
                ["$", ">", ":", "\\", "'", "\"", "@", "<", "-", ".", "/"].iter().map(|s| s.to_string()).collect(),
            ),
            ..lsp::CompletionOptions::default()
        }),
        hover_provider: Some(lsp::HoverProviderCapability::Simple(true)),
        signature_help_provider: Some(lsp::SignatureHelpOptions {
            trigger_characters: Some(vec!["(".into(), ",".into()]),
            retrigger_characters: Some(vec![",".into()]),
            work_done_progress_options: Default::default(),
        }),
        definition_provider: Some(lsp::OneOf::Left(true)),
        declaration_provider: Some(lsp::DeclarationCapability::Simple(true)),
        type_definition_provider: Some(lsp::TypeDefinitionProviderCapability::Simple(true)),
        implementation_provider: Some(lsp::ImplementationProviderCapability::Simple(true)),
        references_provider: Some(lsp::OneOf::Left(true)),
        document_highlight_provider: Some(lsp::OneOf::Left(true)),
        document_symbol_provider: Some(lsp::OneOf::Left(true)),
        workspace_symbol_provider: Some(lsp::OneOf::Left(true)),
        code_action_provider: Some(lsp::CodeActionProviderCapability::Options(lsp::CodeActionOptions {
            code_action_kinds: Some(
                phoxite::imports::ACTION_KINDS.iter().map(|k| lsp::CodeActionKind::from(k.to_string())).collect(),
            ),
            resolve_provider: Some(false),
            work_done_progress_options: Default::default(),
        })),
        code_lens_provider: Some(lsp::CodeLensOptions { resolve_provider: Some(false) }),
        document_formatting_provider: Some(lsp::OneOf::Left(true)),
        document_range_formatting_provider: Some(lsp::OneOf::Left(true)),
        rename_provider: Some(lsp::OneOf::Right(lsp::RenameOptions {
            prepare_provider: Some(true),
            work_done_progress_options: Default::default(),
        })),
        semantic_tokens_provider: Some(lsp::SemanticTokensServerCapabilities::SemanticTokensOptions(
            lsp::SemanticTokensOptions {
                legend: h::semantic_tokens_legend(),
                full: Some(lsp::SemanticTokensFullOptions::Bool(true)),
                range: None,
                work_done_progress_options: Default::default(),
            },
        )),
        inlay_hint_provider: Some(lsp::OneOf::Left(true)),
        call_hierarchy_provider: Some(lsp::CallHierarchyServerCapability::Simple(true)),
        execute_command_provider: Some(lsp::ExecuteCommandOptions {
            commands: commands::SERVER_COMMANDS.iter().map(|c| c.to_string()).collect(),
            work_done_progress_options: Default::default(),
        }),
        workspace: Some(lsp::WorkspaceServerCapabilities {
            workspace_folders: Some(lsp::WorkspaceFoldersServerCapabilities {
                supported: Some(true),
                change_notifications: Some(lsp::OneOf::Left(true)),
            }),
            file_operations: Some(lsp::WorkspaceFileOperationsServerCapabilities {
                will_rename: Some(php_files),
                ..Default::default()
            }),
        }),
        ..lsp::ServerCapabilities::default()
    };
    let mut caps = serde_json::to_value(caps).expect("capabilities serialize");
    // lsp-types 0.95 has no field for it.
    caps["typeHierarchyProvider"] = Value::Bool(true);
    caps
}

fn client_support(caps: &lsp::ClientCapabilities) -> ClientSupport {
    let edit = caps.workspace.as_ref().and_then(|w| w.workspace_edit.as_ref());
    let completion =
        caps.text_document.as_ref().and_then(|t| t.completion.as_ref()).and_then(|c| c.completion_item.as_ref());
    let hover_markdown = caps
        .text_document
        .as_ref()
        .and_then(|t| t.hover.as_ref())
        .and_then(|h| h.content_format.as_ref())
        .is_none_or(|formats| formats.contains(&lsp::MarkupKind::Markdown));
    ClientSupport {
        edits: EditSupport {
            document_changes: edit.and_then(|e| e.document_changes).unwrap_or(false),
            resource_operations: edit.and_then(|e| e.resource_operations.as_ref()).is_some_and(|ops| {
                [lsp::ResourceOperationKind::Create, lsp::ResourceOperationKind::Rename]
                    .iter()
                    .all(|op| ops.contains(op))
            }),
        },
        insert_replace: completion.and_then(|c| c.insert_replace_support).unwrap_or(false),
        snippets: completion.and_then(|c| c.snippet_support).unwrap_or(false),
        work_done_progress: caps.window.as_ref().and_then(|w| w.work_done_progress).unwrap_or(false),
        markdown: hover_markdown,
        dialogs: caps
            .experimental
            .as_ref()
            .and_then(|x| x.pointer("/phoxite/dialogs"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
    }
}

/// Runs the server on `connection` until the client shuts it down.
pub fn run(connection: Connection) -> Result<(), String> {
    let (id, params) = connection.initialize_start().map_err(|e| e.to_string())?;
    let params: lsp::InitializeParams = serde_json::from_value(params).map_err(|e| e.to_string())?;
    let result = serde_json::json!({
        "capabilities": capabilities(),
        "serverInfo": { "name": "phoxite-lsp", "version": env!("CARGO_PKG_VERSION") },
    });
    connection.initialize_finish(id, result).map_err(|e| e.to_string())?;

    let options: InitOptions =
        params.initialization_options.clone().and_then(|o| serde_json::from_value(o).ok()).unwrap_or_default();
    let settings = options.settings.as_ref().map(Settings::from_json).unwrap_or_default();
    let (events_tx, events_rx) = crossbeam_channel::unbounded();
    let state = Arc::new(State::new(
        connection.sender.clone(),
        client_support(&params.capabilities),
        options.cache_dir.clone().unwrap_or_else(default_cache_dir),
        settings,
        events_tx,
    ));
    state.set_trusted(options.trusted.unwrap_or(true));
    #[allow(deprecated)]
    let folders: Vec<Url> = match (&params.workspace_folders, &params.root_uri) {
        (Some(folders), _) if !folders.is_empty() => folders.iter().map(|f| f.uri.clone()).collect(),
        (_, Some(root)) => vec![root.clone()],
        _ => Vec::new(),
    };
    for folder in folders {
        if let Some(path) = convert::path_of(&folder) {
            state.add_workspace(&path);
        }
    }
    let mut server =
        Server { state, pending: Arc::default(), initialized: false, progress_open: false, shutting_down: false };
    server.main_loop(&connection, &events_rx)?;
    server.state.shutdown();
    Ok(())
}

struct Server {
    state: Arc<State>,
    /// The cancellation tokens of running requests.
    pending: Arc<Mutex<HashMap<RequestId, CancellationToken>>>,
    /// The client sent `initialized`: the server may ask it things.
    initialized: bool,
    /// An indexing progress began and hasn't ended.
    progress_open: bool,
    /// `shutdown` came; only `exit` is left.
    shutting_down: bool,
}

impl Server {
    fn main_loop(&mut self, connection: &Connection, events: &Receiver<e::IdeEvent>) -> Result<(), String> {
        loop {
            select! {
                recv(connection.receiver) -> message => {
                    let Ok(message) = message else { return Ok(()) };
                    match message {
                        Message::Request(request) if request.method == r::Shutdown::METHOD => {
                            self.shutting_down = true;
                            let _ = connection.sender.send(Message::Response(Response::new_ok(request.id, ())));
                        }
                        Message::Request(request) if self.shutting_down => {
                            let response = Response::new_err(
                                request.id,
                                ErrorCode::InvalidRequest as i32,
                                "the server is shutting down".into(),
                            );
                            let _ = connection.sender.send(Message::Response(response));
                        }
                        Message::Request(request) => self.request(request),
                        Message::Notification(notification) if notification.method == n::Exit::METHOD => {
                            return Ok(());
                        }
                        Message::Notification(notification) => self.notification(notification),
                        // Answers to the server's own requests (progress tokens).
                        Message::Response(_) => {}
                    }
                }
                recv(events) -> event => {
                    if let Ok(event) = event {
                        self.event(event);
                    }
                }
            }
        }
    }

    fn event(&mut self, event: e::IdeEvent) {
        match event {
            e::IdeEvent::Diagnostics { path, source, items } => self.state.problems_changed(&path, source, items),
            e::IdeEvent::IndexState { indexing, files, symbols } => self.progress(indexing, files, symbols),
            e::IdeEvent::LanguageServer { status, message, .. } => {
                tracing::info!("engine {status:?}{}", message.map(|m| format!(": {m}")).unwrap_or_default());
            }
            e::IdeEvent::LanguageServerRemoved { .. } => {}
        }
    }

    /// Indexing as a work-done progress, when the client shows them.
    fn progress(&mut self, indexing: bool, files: u32, symbols: u32) {
        // Nothing before `initialized`: the client may not take requests yet.
        if !self.state.support.work_done_progress || !self.initialized || indexing == self.progress_open {
            return;
        }
        self.progress_open = indexing;
        let token = lsp::NumberOrString::String(INDEXING_TOKEN.into());
        let value = if indexing {
            let create = Request::new(
                RequestId::from(format!("{INDEXING_TOKEN}/{files}")),
                r::WorkDoneProgressCreate::METHOD.to_owned(),
                lsp::WorkDoneProgressCreateParams { token: token.clone() },
            );
            let _ = self.state.sender.send(Message::Request(create));
            lsp::WorkDoneProgress::Begin(lsp::WorkDoneProgressBegin {
                title: "Indexing PHP".into(),
                cancellable: Some(false),
                message: None,
                percentage: None,
            })
        } else {
            lsp::WorkDoneProgress::End(lsp::WorkDoneProgressEnd {
                message: Some(format!("{files} files, {symbols} declarations")),
            })
        };
        self.state
            .notify::<n::Progress>(lsp::ProgressParams { token, value: lsp::ProgressParamsValue::WorkDone(value) });
    }

    fn notification(&mut self, notification: Notification) {
        let state = &self.state;
        match notification.method.as_str() {
            n::Initialized::METHOD => self.initialized = true,
            n::DidOpenTextDocument::METHOD => {
                if let Some(p) = params::<lsp::DidOpenTextDocumentParams>(notification.params) {
                    let doc = p.text_document;
                    state.open(doc.uri, &doc.language_id, doc.version, &doc.text);
                }
            }
            n::DidChangeTextDocument::METHOD => {
                if let Some(p) = params::<lsp::DidChangeTextDocumentParams>(notification.params) {
                    state.change(&p.text_document.uri, p.text_document.version, p.content_changes);
                }
            }
            n::DidSaveTextDocument::METHOD => {
                if let Some(p) = params::<lsp::DidSaveTextDocumentParams>(notification.params) {
                    analyzers::saved(state, &p.text_document.uri);
                }
            }
            analyzers::WORKSPACE_TRUST => {
                if let Some(p) = params::<analyzers::TrustParams>(notification.params) {
                    state.set_trusted(p.trusted);
                }
            }
            n::DidCloseTextDocument::METHOD => {
                if let Some(p) = params::<lsp::DidCloseTextDocumentParams>(notification.params) {
                    state.close(&p.text_document.uri);
                }
            }
            n::DidChangeWatchedFiles::METHOD => {
                if let Some(p) = params::<lsp::DidChangeWatchedFilesParams>(notification.params) {
                    self.files_changed(p.changes);
                }
            }
            n::DidChangeConfiguration::METHOD => {
                if let Some(p) = params::<lsp::DidChangeConfigurationParams>(notification.params) {
                    state.apply_settings(Settings::from_json(&p.settings));
                }
            }
            n::DidChangeWorkspaceFolders::METHOD => {
                if let Some(p) = params::<lsp::DidChangeWorkspaceFoldersParams>(notification.params) {
                    for folder in p.event.removed {
                        if let Some(path) = convert::path_of(&folder.uri) {
                            state.remove_workspace(&path);
                        }
                    }
                    for folder in p.event.added {
                        if let Some(path) = convert::path_of(&folder.uri) {
                            state.add_workspace(&path);
                        }
                    }
                }
            }
            n::Cancel::METHOD => {
                if let Some(p) = params::<lsp::CancelParams>(notification.params) {
                    let id = match p.id {
                        lsp::NumberOrString::Number(n) => RequestId::from(n),
                        lsp::NumberOrString::String(s) => RequestId::from(s),
                    };
                    if let Some(token) = self.pending.lock().get(&id) {
                        token.cancel();
                    }
                }
            }
            _ => {}
        }
    }

    /// Watcher events: each engine takes those below its folder.
    fn files_changed(&self, changes: Vec<lsp::FileEvent>) {
        let mut changed = Vec::new();
        let mut removed = Vec::new();
        for change in changes {
            let Some(path) = convert::path_of(&change.uri) else { continue };
            if change.typ == lsp::FileChangeType::DELETED { removed.push(path) } else { changed.push(path) }
        }
        for w in self.state.workspaces.read().iter() {
            let mine = |paths: &[std::path::PathBuf]| -> Vec<_> {
                paths.iter().filter(|p| p.starts_with(&w.root)).cloned().collect()
            };
            let (c, r) = (mine(&changed), mine(&removed));
            if !c.is_empty() || !r.is_empty() {
                w.engine.files_changed(&c, &r);
            }
            // The database file or the `.env` naming it: the live schema.
            if changed.iter().chain(&removed).any(|path| crate::schema::concerns(&self.state, &w.root, path)) {
                crate::schema::refresh_quietly(&self.state, &w.root, &w.engine);
            }
        }
    }

    fn request(&self, request: Request) {
        let token = CancellationToken::new();
        self.pending.lock().insert(request.id.clone(), token.clone());
        let state = self.state.clone();
        let pending = self.pending.clone();
        std::thread::Builder::new()
            .name(format!("request {}", request.method))
            .spawn(move || {
                let id = request.id.clone();
                let response = dispatch(&state, request, &token);
                pending.lock().remove(&id);
                let response = if token.is_cancelled() {
                    Response::new_err(id, ErrorCode::RequestCanceled as i32, "cancelled".into())
                } else {
                    response
                };
                let _ = state.sender.send(Message::Response(response));
            })
            .expect("spawn a request thread");
    }
}

fn params<P: DeserializeOwned>(value: Value) -> Option<P> {
    match serde_json::from_value(value) {
        Ok(p) => Some(p),
        Err(e) => {
            tracing::warn!("malformed params: {e}");
            None
        }
    }
}

fn respond<T: Serialize>(id: RequestId, result: h::Result<T>) -> Response {
    match result {
        Ok(value) => Response::new_ok(id, value),
        Err(message) => Response::new_err(id, ErrorCode::RequestFailed as i32, message),
    }
}

/// Answers one request.
fn dispatch(state: &State, request: Request, cancel: &CancellationToken) -> Response {
    let Request { id, method, params } = request;
    let request_id = id.clone();
    macro_rules! on {
        ($req:ty, |$p:ident| $body:expr) => {
            match serde_json::from_value::<<$req as r::Request>::Params>(params) {
                Ok($p) => respond(id, $body),
                Err(e) => Response::new_err(id, ErrorCode::InvalidParams as i32, e.to_string()),
            }
        };
    }
    let run = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match method.as_str() {
        r::Completion::METHOD => on!(r::Completion, |p| h::completion(state, p, cancel)),
        r::ResolveCompletionItem::METHOD => on!(r::ResolveCompletionItem, |p| h::resolve_completion(state, p)),
        r::HoverRequest::METHOD => on!(r::HoverRequest, |p| h::hover(state, p)),
        r::SignatureHelpRequest::METHOD => on!(r::SignatureHelpRequest, |p| h::signature_help(state, p)),
        r::GotoDefinition::METHOD => {
            on!(r::GotoDefinition, |p| h::goto(state, p.text_document_position_params, Goto::Definition))
        }
        r::GotoDeclaration::METHOD => {
            on!(r::GotoDeclaration, |p| h::goto(state, p.text_document_position_params, Goto::Definition))
        }
        r::GotoTypeDefinition::METHOD => {
            on!(r::GotoTypeDefinition, |p| h::goto(state, p.text_document_position_params, Goto::TypeDefinition))
        }
        r::GotoImplementation::METHOD => {
            on!(r::GotoImplementation, |p| h::goto(state, p.text_document_position_params, Goto::Implementation))
        }
        r::References::METHOD => on!(r::References, |p| h::references(state, p, cancel)),
        r::DocumentHighlightRequest::METHOD => on!(r::DocumentHighlightRequest, |p| h::document_highlight(state, p)),
        r::DocumentSymbolRequest::METHOD => on!(r::DocumentSymbolRequest, |p| h::document_symbols(state, p)),
        r::WorkspaceSymbolRequest::METHOD => on!(r::WorkspaceSymbolRequest, |p| h::workspace_symbols(state, p, cancel)),
        r::PrepareRenameRequest::METHOD => on!(r::PrepareRenameRequest, |p| h::prepare_rename(state, p)),
        r::Rename::METHOD => on!(r::Rename, |p| h::rename(state, p, cancel)),
        r::WillRenameFiles::METHOD => on!(r::WillRenameFiles, |p| h::will_rename_files(state, p, cancel)),
        r::CodeActionRequest::METHOD => on!(r::CodeActionRequest, |p| h::code_actions(state, p)),
        r::Formatting::METHOD => on!(r::Formatting, |p| h::format(state, &p.text_document.uri, None, &p.options)),
        r::RangeFormatting::METHOD => {
            on!(r::RangeFormatting, |p| h::format(state, &p.text_document.uri, Some(p.range), &p.options))
        }
        r::InlayHintRequest::METHOD => on!(r::InlayHintRequest, |p| h::inlay_hints(state, p)),
        r::CodeLensRequest::METHOD => on!(r::CodeLensRequest, |p| h::code_lens(state, p)),
        r::SemanticTokensFullRequest::METHOD => on!(r::SemanticTokensFullRequest, |p| h::semantic_tokens(state, p)),
        r::CallHierarchyPrepare::METHOD => on!(r::CallHierarchyPrepare, |p| h::prepare_call_hierarchy(state, p)),
        r::CallHierarchyIncomingCalls::METHOD => {
            on!(r::CallHierarchyIncomingCalls, |p| h::incoming_calls(state, p, cancel))
        }
        r::CallHierarchyOutgoingCalls::METHOD => {
            on!(r::CallHierarchyOutgoingCalls, |p| h::outgoing_calls(state, p, cancel))
        }
        r::TypeHierarchyPrepare::METHOD => on!(r::TypeHierarchyPrepare, |p| h::prepare_type_hierarchy(state, p)),
        r::TypeHierarchySupertypes::METHOD => on!(r::TypeHierarchySupertypes, |p| h::type_relatives(
            state,
            p.item.data,
            phoxite::navigate::Direction::Up,
            cancel
        )),
        r::TypeHierarchySubtypes::METHOD => on!(r::TypeHierarchySubtypes, |p| h::type_relatives(
            state,
            p.item.data,
            phoxite::navigate::Direction::Down,
            cancel
        )),
        r::ExecuteCommand::METHOD => on!(r::ExecuteCommand, |p| execute_command(state, p)),
        custom if laravel::METHODS.contains(&custom) => respond(id, laravel::request(state, custom, params)),
        custom if refactor::METHODS.contains(&custom) => respond(id, refactor::request(state, custom, params, cancel)),
        _ => Response::new_err(id, ErrorCode::MethodNotFound as i32, format!("unknown request {method}")),
    }));
    run.unwrap_or_else(|panic| {
        let message = panic
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string()))
            .unwrap_or_else(|| "the request panicked".into());
        tracing::error!("{method}: {message}");
        Response::new_err(request_id, ErrorCode::InternalError as i32, message)
    })
}

fn execute_command(state: &State, p: lsp::ExecuteCommandParams) -> h::Result<Option<Value>> {
    match p.command.as_str() {
        commands::RESCAN => {
            for engine in state.engines() {
                engine.rescan();
            }
            Ok(None)
        }
        other => Err(format!("unknown command {other}")),
    }
}
