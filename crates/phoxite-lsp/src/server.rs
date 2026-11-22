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
use crate::config::{InitOptions, PROJECT_FILE, default_cache_dir, lenient};
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
                ["$", ">", ":", "\\", "'", "\"", "@", "<", "-", ".", "/", "|"].iter().map(|s| s.to_string()).collect(),
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
    // What a client can look for instead of the server's name: the `phoxite/laravel/*`
    // and `phoxite/refactor/*` requests.
    caps["experimental"] =
        serde_json::json!({ "phoxite": { "laravel": true, "refactor": true, "laravelStatus": true } });
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
        watched_files: caps
            .workspace
            .as_ref()
            .and_then(|w| w.did_change_watched_files.as_ref())
            .and_then(|w| w.dynamic_registration)
            .unwrap_or(false),
        document_registration: caps
            .text_document
            .as_ref()
            .and_then(|t| t.synchronization.as_ref())
            .and_then(|s| s.dynamic_registration)
            .unwrap_or(false),
        markdown: hover_markdown,
        inlay_hint_refresh: caps
            .workspace
            .as_ref()
            .and_then(|w| w.inlay_hint.as_ref())
            .and_then(|hints| hints.refresh_support)
            .unwrap_or(false),
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

    let (options, mut rejected): (InitOptions, _) =
        lenient(params.initialization_options.as_ref().unwrap_or(&Value::Null));
    let (events_tx, events_rx) = crossbeam_channel::unbounded();
    let state = Arc::new(State::new(
        connection.sender.clone(),
        client_support(&params.capabilities),
        options.cache_dir.clone().unwrap_or_else(default_cache_dir),
        events_tx,
    ));
    state.set_trusted(options.trusted.unwrap_or(true));
    state.apply_settings(options.settings.as_ref().unwrap_or(&Value::Null), std::mem::take(&mut rejected));
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
    // `initialize_finish` returned, so the client's `initialized` has come (it reads the
    // notification itself): the server may ask the client things from here on.
    let mut server = Server {
        state,
        pending: Arc::default(),
        progress_open: false,
        shutting_down: false,
        laravel: None,
        scripts: false,
        yaml: false,
        xml: false,
        joomla: false,
        failure: None,
    };
    server.watch_files();
    server.main_loop(&connection, &events_rx)?;
    server.state.shutdown();
    Ok(())
}

struct Server {
    state: Arc<State>,
    /// The cancellation tokens of running requests.
    pending: Arc<Mutex<HashMap<RequestId, CancellationToken>>>,
    /// An indexing progress began and hasn't ended.
    progress_open: bool,
    /// `shutdown` came; only `exit` is left.
    shutting_down: bool,
    /// The Laravel folders the client was last told about; none before the first time.
    laravel: Option<(Vec<Url>, Vec<&'static str>)>,
    /// The client was asked to send the scripts of a WordPress project.
    scripts: bool,
    /// The client was asked to send the YAML files of a Drupal project.
    yaml: bool,
    /// The client was asked to send the XML files of a Magento project.
    xml: bool,
    /// The client was asked to send the XML, language and asset files of a
    /// Joomla project.
    joomla: bool,
    /// Why indexing last failed, once the client was told; none after it worked again.
    failure: Option<String>,
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
            e::IdeEvent::IndexState { indexing, files, symbols } => {
                self.progress(indexing, files, symbols);
                // What makes a folder a Laravel project is read with the index.
                if !indexing {
                    self.laravel_status();
                }
            }
            e::IdeEvent::LanguageServer { status, message, .. } => {
                tracing::info!("engine {status:?}{}", message.as_ref().map(|m| format!(": {m}")).unwrap_or_default());
                match status {
                    e::event::ServerStatus::Crashed => self.indexing_failed(message.unwrap_or_default()),
                    e::event::ServerStatus::Ready => self.failure = None,
                    _ => {}
                }
            }
            e::IdeEvent::LanguageServerRemoved { .. } => {}
        }
    }

    /// Tells the user indexing failed, once for each reason until it works again.
    fn indexing_failed(&mut self, reason: String) {
        if self.failure.as_ref() == Some(&reason) {
            return;
        }
        self.state.notify::<n::ShowMessage>(lsp::ShowMessageParams {
            typ: lsp::MessageType::ERROR,
            message: format!(
                "Phoxite could not index the project: {reason}. Index the project again to retry (`{}`).",
                crate::commands::RESCAN
            ),
        });
        self.failure = Some(reason);
    }

    /// Tells the client which workspace folders are Laravel projects, when that changed
    /// since it was last told: a client shows its Laravel tools on it without asking again.
    fn laravel_status(&mut self) {
        // What an open XML file of a Magento project names is known once the index is.
        self.state.magento_documents_changed();
        let folders = crate::laravel::folders(&self.state);
        let frameworks = crate::laravel::frameworks(&self.state);
        if self.laravel.as_ref() == Some(&(folders.clone(), frameworks.clone())) {
            return;
        }
        let scripts = serde_json::json!([
            { "scheme": "file", "language": "javascript" },
            { "scheme": "file", "language": "javascriptreact" },
            { "scheme": "file", "language": "typescript" },
            { "scheme": "file", "language": "typescriptreact" },
            { "scheme": "file", "pattern": "**/block.json" },
        ]);
        let wanted = frameworks.contains(&"WordPress");
        if wanted != self.scripts && self.register_documents("scripts", wanted, scripts) {
            self.scripts = wanted;
        }
        // Its services, routing, permissions, libraries, info and links files.
        let yaml = serde_json::json!([
            { "scheme": "file", "pattern": "**/*.yml" },
            { "scheme": "file", "pattern": "**/*.info" },
        ]);
        let wanted = frameworks.contains(&"Drupal");
        if wanted != self.yaml && self.register_documents("yaml", wanted, yaml) {
            self.yaml = wanted;
        }
        // The declarations of its modules and themes.
        let xml = serde_json::json!([
            { "scheme": "file", "pattern": "**/etc/**/*.xml" },
            { "scheme": "file", "pattern": "**/layout/*.xml" },
            { "scheme": "file", "pattern": "**/ui_component/*.xml" },
            { "scheme": "file", "pattern": "**/etc/**/schema.graphqls" },
        ]);
        let wanted = frameworks.contains(&"Magento");
        if wanted != self.xml && self.register_documents("xml", wanted, xml) {
            self.xml = wanted;
        }
        // Its manifests, forms and access files, its language files, its asset lists.
        let joomla = serde_json::json!([
            { "scheme": "file", "pattern": "**/*.xml" },
            { "scheme": "file", "pattern": "**/language/**/*.ini" },
            { "scheme": "file", "pattern": "**/joomla.asset.json" },
        ]);
        let wanted = frameworks.contains(&"Joomla");
        if wanted != self.joomla && self.register_documents("joomla", wanted, joomla) {
            self.joomla = wanted;
        }
        let params =
            serde_json::json!({ "laravel": !folders.is_empty(), "folders": folders, "frameworks": frameworks });
        self.laravel = Some((folders, frameworks));
        let notification = Notification::new(crate::laravel::STATUS_CHANGED.to_owned(), params);
        let _ = self.state.sender.send(Message::Notification(notification));
    }

    /// Asks the client for documents no selector of its own lists (`wanted`), or to stop
    /// sending them: the scripts and `block.json` files while a WordPress project is open,
    /// the YAML files while a Drupal one is. Their documents, and the requests the server
    /// answers in them. A client that doesn't take registrations sends them only if it is
    /// set up to. Returns whether the client was asked.
    fn register_documents(&mut self, set: &str, wanted: bool, selector: Value) -> bool {
        if !self.state.support.document_registration {
            return false;
        }
        let id = |method: &str| format!("phoxite/{set}/{method}");
        let request = if wanted {
            let registrations = SCRIPT_METHODS
                .iter()
                .map(|method| {
                    let mut options = serde_json::json!({ "documentSelector": selector });
                    match *method {
                        "textDocument/didChange" => options["syncKind"] = serde_json::json!(2),
                        "textDocument/completion" => options["triggerCharacters"] = serde_json::json!(["'", "\"", "/"]),
                        "textDocument/rename" => options["prepareProvider"] = serde_json::json!(true),
                        _ => {}
                    }
                    lsp::Registration { id: id(method), method: (*method).to_owned(), register_options: Some(options) }
                })
                .collect();
            Request::new(
                RequestId::from(format!("phoxite/{set}")),
                r::RegisterCapability::METHOD.to_owned(),
                lsp::RegistrationParams { registrations },
            )
        } else {
            let unregisterations = SCRIPT_METHODS
                .iter()
                .map(|method| lsp::Unregistration { id: id(method), method: (*method).to_owned() })
                .collect();
            Request::new(
                RequestId::from(format!("phoxite/{set}/stop")),
                r::UnregisterCapability::METHOD.to_owned(),
                lsp::UnregistrationParams { unregisterations },
            )
        };
        let _ = self.state.sender.send(Message::Request(request));
        true
    }

    /// Indexing as a work-done progress, when the client shows them.
    fn progress(&mut self, indexing: bool, files: u32, symbols: u32) {
        // Nothing before `initialized`: the client may not take requests yet.
        if !self.state.support.work_done_progress || indexing == self.progress_open {
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
        // The settings choose how Laravel is detected, and a folder can leave.
        let laravel_may_change = [n::DidChangeConfiguration::METHOD, n::DidChangeWorkspaceFolders::METHOD]
            .contains(&notification.method.as_str());
        match notification.method.as_str() {
            // Read by `initialize_finish`; a client that repeats it changes nothing.
            n::Initialized::METHOD => {}
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
                    state.apply_settings(&p.settings, Vec::new());
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
        if laravel_may_change {
            self.laravel_status();
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
        // Settings files first: what changed with them is indexed by the new settings.
        let roots: Vec<_> = self.state.workspaces.read().iter().map(|w| w.root.clone()).collect();
        for root in roots {
            let file = root.join(PROJECT_FILE);
            if changed.iter().chain(&removed).any(|path| *path == file) {
                self.state.project_file_changed(&root);
            }
        }
        for w in self.state.workspaces.read().iter() {
            let mine = |paths: &[std::path::PathBuf]| -> Vec<_> {
                paths.iter().filter(|p| p.starts_with(&w.root)).cloned().collect()
            };
            let (c, r) = (mine(&changed), mine(&removed));
            // The PHP version and the extensions the project requires.
            let manifest = w.root.join("composer.json");
            if c.iter().chain(&r).any(|path| *path == manifest) {
                self.state.refresh_stub_filter(w);
            }
            if !c.is_empty() || !r.is_empty() {
                w.engine.files_changed(&c, &r);
            }
            // Templates created or removed, and what says where they are.
            if c.iter().chain(&r).any(|path| concerns_twig(path)) {
                self.state.forget_twig(&w.root);
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

/// The files on disk the index is built from, as glob patterns: what a client watches
/// for the server.
pub const WATCHED_FILES: [&str; 27] = [
    "**/*.php",
    // Drupal's PHP files, and the YAML files that declare its names.
    "**/*.{module,install,theme,profile,inc,engine}",
    "**/*.{services,routing,permissions,libraries,info}.yml",
    "**/*.links.*.yml",
    "**/*.{layouts,breakpoints}.yml",
    "**/*.info",
    // Twig templates, and the YAML configuration that names template
    // folders, services and routes.
    "**/*.twig",
    "**/config/**/*.{yaml,yml}",
    // The settings file of a project.
    "**/phoxite.json",
    "**/composer.{json,lock}",
    "**/vendor/composer/installed.json",
    "**/.env*",
    // The database of `laravel.database`.
    "**/*.{sqlite,sqlite3,db}",
    "**/lang/**/*.json",
    "**/ide.json",
    "**/.phpstorm.meta.php",
    // The blocks of a WordPress plugin or theme.
    "**/block.json",
    // Magento's declarations: a module's `etc` and its layout files.
    "**/etc/**/*.xml",
    "**/layout/*.xml",
    "**/ui_component/*.xml",
    "**/etc/**/schema.graphqls",
    "**/theme.xml",
    // Joomla's declarations: manifests and forms, language files, asset lists, SQL scripts.
    "**/{components,modules,plugins,templates}/**/*.xml",
    "**/forms/*.xml",
    "**/language/**/*.ini",
    "**/joomla.asset.json",
    "**/sql/**/*.sql",
];

/// What the server is sent and asked in the documents it registers for.
const SCRIPT_METHODS: [&str; 8] = [
    "textDocument/didOpen",
    "textDocument/didChange",
    "textDocument/didClose",
    "textDocument/completion",
    "textDocument/hover",
    "textDocument/definition",
    "textDocument/references",
    "textDocument/rename",
];

impl Server {
    /// Asks the client to report changes of the files the index is built from. Without
    /// it the index goes stale for whatever is written outside the editor: a generator's
    /// files, a `git checkout`, `composer install`.
    fn watch_files(&self) {
        if !self.state.support.watched_files {
            return;
        }
        let watchers = WATCHED_FILES
            .iter()
            .map(|glob| lsp::FileSystemWatcher { glob_pattern: lsp::GlobPattern::String((*glob).into()), kind: None })
            .collect();
        let registration = lsp::Registration {
            id: "phoxite/watched-files".into(),
            method: n::DidChangeWatchedFiles::METHOD.into(),
            register_options: serde_json::to_value(lsp::DidChangeWatchedFilesRegistrationOptions { watchers }).ok(),
        };
        let request = Request::new(
            RequestId::from("phoxite/watched-files".to_owned()),
            r::RegisterCapability::METHOD.to_owned(),
            lsp::RegistrationParams { registrations: vec![registration] },
        );
        let _ = self.state.sender.send(Message::Request(request));
    }
}

/// Whether a change of `path` changes what the Twig support read about its
/// project: a template, `twig.yaml`, or a Composer file.
fn concerns_twig(path: &std::path::Path) -> bool {
    let name = path.file_name().and_then(|name| name.to_str()).unwrap_or_default().to_ascii_lowercase();
    name.ends_with(".twig")
        || matches!(name.as_str(), "twig.yaml" | "twig.yml" | "composer.json" | "composer.lock" | "installed.json")
        || path.extension().is_none()
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
            // The settings files too, for a client that doesn't watch them.
            let roots: Vec<_> = state.workspaces.read().iter().map(|w| w.root.clone()).collect();
            for root in roots {
                state.project_file_changed(&root);
            }
            for w in state.workspaces.read().iter() {
                state.refresh_stub_filter(w);
                w.engine.rescan();
            }
            Ok(None)
        }
        other => Err(format!("unknown command {other}")),
    }
}
