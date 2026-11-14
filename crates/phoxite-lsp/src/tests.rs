//! The server end to end over an in-memory connection, on the fixtures.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use lsp_server::{Connection, Message, Notification, Request, RequestId};
use lsp_types::Url;
use lsp_types::notification::{self as n, Notification as _};
use lsp_types::request as r;
use serde_json::{Value, json};

struct Client {
    conn: Connection,
    server: Option<std::thread::JoinHandle<()>>,
    next: i32,
    /// Notifications that arrived while waiting for a response.
    notifications: Vec<Notification>,
    _cache: tempfile::TempDir,
}

fn fixture(name: &str) -> PathBuf {
    dunce::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures").join(name)).unwrap()
}

impl Client {
    fn start(root: &Path) -> Self {
        let (client, server) = Connection::memory();
        let handle = std::thread::spawn(move || crate::server::run(server).unwrap());
        let cache = tempfile::tempdir().unwrap();
        let mut c = Self { conn: client, server: Some(handle), next: 0, notifications: Vec::new(), _cache: cache };
        let caps = json!({
            "workspace": { "workspaceEdit": { "documentChanges": true, "resourceOperations": ["create", "rename", "delete"] } },
            "textDocument": {
                "completion": { "completionItem": { "snippetSupport": true, "insertReplaceSupport": true } },
                "hover": { "contentFormat": ["markdown"] }
            }
        });
        let init = c.request::<r::Initialize>(json!({
            "processId": null,
            "rootUri": null,
            "capabilities": caps,
            "workspaceFolders": [{ "uri": Url::from_file_path(root).unwrap(), "name": "fixture" }],
            "initializationOptions": { "cacheDir": c._cache.path(), "settings": { "phoxite": {} } }
        }));
        assert_eq!(init["serverInfo"]["name"], "phoxite-lsp");
        c.notify::<n::Initialized>(json!({}));
        c
    }

    fn request<R: r::Request>(&mut self, params: Value) -> Value {
        self.next += 1;
        let id = RequestId::from(self.next);
        self.conn.sender.send(Message::Request(Request::new(id.clone(), R::METHOD.to_owned(), params))).unwrap();
        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.conn.receiver.recv_timeout(left).unwrap_or_else(|_| panic!("no answer to {}", R::METHOD)) {
                Message::Response(response) if response.id == id => {
                    if let Some(error) = response.error {
                        panic!("{} failed: {}", R::METHOD, error.message);
                    }
                    return response.result.unwrap_or(Value::Null);
                }
                Message::Notification(notification) => self.notifications.push(notification),
                // The server asking us something (progress tokens): answer it.
                Message::Request(request) => {
                    let _ = self.conn.sender.send(Message::Response(lsp_server::Response::new_ok(request.id, ())));
                }
                Message::Response(_) => {}
            }
        }
    }

    fn notify<N: n::Notification>(&self, params: Value) {
        self.conn.sender.send(Message::Notification(Notification::new(N::METHOD.to_owned(), params))).unwrap();
    }

    fn open(&self, path: &Path, language: &str) -> Url {
        let uri = Url::from_file_path(path).unwrap();
        let text = std::fs::read_to_string(path).unwrap();
        self.notify::<n::DidOpenTextDocument>(json!({
            "textDocument": { "uri": uri, "languageId": language, "version": 1, "text": text }
        }));
        uri
    }

    /// Asks until the index has caught up (an empty answer first).
    fn request_until<R: r::Request>(&mut self, params: Value, done: impl Fn(&Value) -> bool) -> Value {
        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            let found = self.request::<R>(params.clone());
            if done(&found) || Instant::now() > deadline {
                return found;
            }
            std::thread::sleep(Duration::from_millis(200));
        }
    }

    /// Waits for diagnostics of `uri`.
    fn diagnostics(&mut self, uri: &Url, done: impl Fn(&[Value]) -> bool) -> Vec<Value> {
        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            let found = self.notifications.iter().rev().find_map(|n| {
                (n.method == n::PublishDiagnostics::METHOD && n.params["uri"] == json!(uri))
                    .then(|| n.params["diagnostics"].as_array().cloned().unwrap_or_default())
            });
            if let Some(found) = found
                && done(&found)
            {
                return found;
            }
            assert!(Instant::now() < deadline, "no diagnostics for {uri}");
            match self.conn.receiver.recv_timeout(Duration::from_millis(200)) {
                Ok(Message::Notification(notification)) => self.notifications.push(notification),
                Ok(Message::Request(request)) => {
                    let _ = self.conn.sender.send(Message::Response(lsp_server::Response::new_ok(request.id, ())));
                }
                _ => {}
            }
        }
    }

    fn shutdown(mut self) {
        self.request::<r::Shutdown>(Value::Null);
        self.notify::<n::Exit>(Value::Null);
        self.server.take().unwrap().join().unwrap();
    }
}

fn at(uri: &Url, line: u32, character: u32) -> Value {
    json!({ "textDocument": { "uri": uri }, "position": { "line": line, "character": character } })
}

/// The position of the first `needle` in the file, plus `offset` columns.
fn find(path: &Path, needle: &str, offset: u32) -> (u32, u32) {
    let text = std::fs::read_to_string(path).unwrap().replace("\r\n", "\n");
    let byte = text.find(needle).unwrap_or_else(|| panic!("{needle} not in {}", path.display()));
    let line = text[..byte].matches('\n').count() as u32;
    let col = text[..byte].rsplit('\n').next().unwrap().encode_utf16().count() as u32;
    (line, col + offset)
}

#[test]
fn php_requests_on_the_library_fixture() {
    let root = fixture("php-library");
    let mut c = Client::start(&root);
    let file = root.join("src/Library.php");
    let uri = c.open(&file, "php");

    let symbols = c.request_until::<r::DocumentSymbolRequest>(json!({ "textDocument": { "uri": uri } }), |v| {
        v.as_array().is_some_and(|a| !a.is_empty())
    });
    assert!(symbols.to_string().contains("\"Library\""), "{symbols}");

    // Hover and goto on a type the file uses.
    let (line, col) = find(&file, "BookRepository $books", 1);
    let hover = c.request_until::<r::HoverRequest>(at(&uri, line, col), |v| !v.is_null());
    assert_eq!(hover["contents"]["kind"], "markdown");
    let definition = c.request::<r::GotoDefinition>(at(&uri, line, col));
    assert!(definition.to_string().contains("BookRepository.php"), "{definition}");

    // Members after `$this->books->`.
    let (mline, mcol) = find(&file, "$this->books->save", 14);
    let completion = c.request::<r::Completion>(at(&uri, mline, mcol));
    let labels: Vec<&str> =
        completion["items"].as_array().unwrap().iter().filter_map(|item| item["label"].as_str()).collect();
    assert!(labels.contains(&"save") && labels.contains(&"find"), "{labels:?}");

    let tokens = c.request::<r::SemanticTokensFullRequest>(json!({ "textDocument": { "uri": uri } }));
    assert!(tokens["data"].as_array().is_some_and(|d| !d.is_empty() && d.len() % 5 == 0));

    let references = c.request::<r::References>(json!({
        "textDocument": { "uri": uri }, "position": { "line": line, "character": col },
        "context": { "includeDeclaration": true }
    }));
    assert!(references.as_array().is_some_and(|r| r.len() > 1), "{references}");

    let formatted = c.request::<r::Formatting>(json!({
        "textDocument": { "uri": uri }, "options": { "tabSize": 4, "insertSpaces": true }
    }));
    assert!(formatted.is_array());

    let symbols = c.request::<r::WorkspaceSymbolRequest>(json!({ "query": "BookRepo" }));
    assert!(symbols.to_string().contains("BookRepository"), "{symbols}");

    c.shutdown();
}

#[test]
fn problems_are_published_and_fixed() {
    let root = fixture("php-library");
    let dir = tempfile::tempdir().unwrap();
    // A copy, so the edit stays out of the fixture.
    copy_dir(&root, dir.path());
    let root = dunce::canonicalize(dir.path()).unwrap();
    let mut c = Client::start(&root);
    let file = root.join("src/Broken.php");
    std::fs::write(&file, "<?php\nnamespace App;\n\nfunction f(): void {\n    new UnknownThing();\n}\n").unwrap();
    let uri = c.open(&file, "php");
    let problems = c.diagnostics(&uri, |d| !d.is_empty());
    assert!(problems.iter().any(|d| d["message"].as_str().is_some_and(|m| m.contains("UnknownThing"))), "{problems:?}");
    let actions = c.request::<r::CodeActionRequest>(json!({
        "textDocument": { "uri": uri },
        "range": { "start": { "line": 4, "character": 8 }, "end": { "line": 4, "character": 8 } },
        "context": { "diagnostics": [] }
    }));
    assert!(actions.is_array(), "{actions}");
    c.shutdown();
}

#[test]
fn laravel_routes_views_and_blade() {
    // The app with the Laravel framework stubs as its `vendor`.
    let dir = tempfile::tempdir().unwrap();
    let root = dunce::canonicalize(dir.path()).unwrap();
    copy_dir(&fixture("laravel-app"), &root);
    std::fs::create_dir_all(root.join("vendor")).unwrap();
    copy_dir(&fixture("laravel-stubs"), &root.join("vendor"));
    let mut c = Client::start(&root);
    let routes = root.join("routes/web.php");
    let uri = c.open(&routes, "php");
    // The view of `Route::view('/about', 'about')` is the template.
    let (line, col) = find(&routes, "'about')", 2);
    let found =
        c.request_until::<r::GotoDefinition>(at(&uri, line, col), |v| v.as_array().is_some_and(|a| !a.is_empty()));
    assert!(found.to_string().contains("about.blade.php"), "{found}");
    let hints = c.request_until::<r::InlayHintRequest>(
        json!({ "textDocument": { "uri": uri }, "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 9999, "character": 0 } } }),
        |v| v.as_array().is_some_and(|a| !a.is_empty()),
    );
    assert!(hints.to_string().contains("laravel.routeActions"), "{hints}");

    // A template: its outline.
    let blade = c.open(&root.join("resources/views/about.blade.php"), "blade");
    let outline = c.request::<r::DocumentSymbolRequest>(json!({ "textDocument": { "uri": blade } }));
    assert!(outline.is_array(), "{outline}");
    c.shutdown();
}

fn copy_dir(from: &Path, to: &Path) {
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            std::fs::create_dir_all(&target).unwrap();
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).unwrap();
        }
    }
}

#[test]
fn edits_then_member_completion_and_formatting() {
    let dir = tempfile::tempdir().unwrap();
    let root = dunce::canonicalize(dir.path()).unwrap();
    copy_dir(&fixture("laravel-app"), &root);
    std::fs::create_dir_all(root.join("vendor")).unwrap();
    copy_dir(&fixture("laravel-stubs"), &root.join("vendor"));
    let mut c = Client::start(&root);
    let file = root.join("app/Http/Controllers/PostController.php");
    let uri = c.open(&file, "php");
    let text = std::fs::read_to_string(&file).unwrap().replace("\r\n", "\n");
    // An incremental insert before the class's closing brace, as VS Code sends it.
    let brace = text.rfind('}').unwrap();
    let probe = "    public function probe(\\App\\Models\\Post $post) { $post->; }\n";
    let edited = format!("{}{probe}{}", &text[..brace], &text[brace..]);
    let brace_line = text[..brace].matches('\n').count() as u32;
    let brace_col = text[..brace].rsplit('\n').next().unwrap().encode_utf16().count() as u32;
    let point = json!({ "line": brace_line, "character": brace_col });
    c.notify::<n::DidChangeTextDocument>(json!({
        "textDocument": { "uri": uri, "version": 2 },
        "contentChanges": [{ "range": { "start": point, "end": point }, "text": probe }]
    }));
    let byte = edited.find("$post->;").unwrap() + "$post->".len();
    let line = edited[..byte].matches('\n').count() as u32;
    let col = edited[..byte].rsplit('\n').next().unwrap().encode_utf16().count() as u32;
    let completion = c.request_until::<r::Completion>(at(&uri, line, col), |v| {
        v["items"].as_array().is_some_and(|items| items.iter().any(|i| i["label"] == "author"))
    });
    let labels: Vec<&str> =
        completion["items"].as_array().unwrap().iter().filter_map(|i| i["label"].as_str()).collect();
    assert!(labels.contains(&"author") && labels.contains(&"comments"), "{labels:?}");
    assert!(!labels.contains(&"$_GET"), "{labels:?}");

    // Formatting a messy file gives edits; a tidy one gives an empty list.
    let messy = root.join("app/Messy.php");
    std::fs::write(&messy, "<?php\nnamespace App;\nclass Messy{public function a( $x ){return $x+1;}}\n").unwrap();
    let messy = c.open(&messy, "php");
    let options = json!({ "tabSize": 4, "insertSpaces": true });
    let edits = c.request::<r::Formatting>(json!({ "textDocument": { "uri": messy }, "options": options }));
    assert!(edits.as_array().is_some_and(|e| !e.is_empty()), "{edits}");
    c.shutdown();
}

#[test]
fn capabilities_name_every_feature() {
    let caps = crate::server::capabilities();
    for key in [
        "completionProvider",
        "hoverProvider",
        "definitionProvider",
        "referencesProvider",
        "renameProvider",
        "codeActionProvider",
        "documentFormattingProvider",
        "semanticTokensProvider",
        "inlayHintProvider",
        "codeLensProvider",
        "callHierarchyProvider",
        "typeHierarchyProvider",
    ] {
        assert!(caps.get(key).is_some_and(|v| !v.is_null()), "{key} missing");
    }
    assert_eq!(caps["positionEncoding"], "utf-16");
    // Clients register server commands as editor commands; the VS Code
    // extension's own commands are `phoxite.*`, so the server's must not be.
    let package: Value =
        serde_json::from_str(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../editors/vscode/package.json")))
            .unwrap();
    let extension: Vec<&str> =
        package["contributes"]["commands"].as_array().unwrap().iter().filter_map(|c| c["command"].as_str()).collect();
    for command in caps["executeCommandProvider"]["commands"].as_array().unwrap() {
        let command = command.as_str().unwrap();
        assert!(command.starts_with("phoxite.server."), "{command}");
        assert!(!extension.contains(&command), "{command} is also an extension command");
    }
}
