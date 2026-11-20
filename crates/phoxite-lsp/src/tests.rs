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
    /// What the server asked the client (progress tokens, registrations).
    requests: Vec<Request>,
    _cache: tempfile::TempDir,
}

fn fixture(name: &str) -> PathBuf {
    dunce::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures").join(name)).unwrap()
}

impl Client {
    fn start(root: &Path) -> Self {
        Self::start_with(root, Value::Null)
    }

    /// Starts the server for a client declaring `experimental` capabilities.
    fn start_with(root: &Path, experimental: Value) -> Self {
        Self::start_as(root, experimental, true)
    }

    /// The same for a client that does (or doesn't) watch files for the server.
    fn start_as(root: &Path, experimental: Value, watches_files: bool) -> Self {
        let (client, server) = Connection::memory();
        let handle = std::thread::spawn(move || crate::server::run(server).unwrap());
        let cache = tempfile::tempdir().unwrap();
        let mut c = Self {
            conn: client,
            server: Some(handle),
            next: 0,
            notifications: Vec::new(),
            requests: Vec::new(),
            _cache: cache,
        };
        let caps = json!({
            "workspace": {
                "workspaceEdit": { "documentChanges": true, "resourceOperations": ["create", "rename", "delete"] },
                "didChangeWatchedFiles": { "dynamicRegistration": watches_files }
            },
            "textDocument": {
                "synchronization": { "dynamicRegistration": watches_files },
                "completion": { "completionItem": { "snippetSupport": true, "insertReplaceSupport": true } },
                "hover": { "contentFormat": ["markdown"] }
            },
            "experimental": experimental
        });
        let init = c.request::<r::Initialize>(json!({
            "processId": null,
            "rootUri": null,
            "capabilities": caps,
            "workspaceFolders": [{ "uri": Url::from_file_path(root).unwrap(), "name": "fixture" }],
            "initializationOptions": { "cacheDir": c._cache.path(), "settings": { "phoxite": {} } }
        }));
        assert_eq!(init["serverInfo"]["name"], "phoxite-lsp");
        assert_eq!(init["capabilities"]["experimental"]["phoxite"]["refactor"], true);
        c.notify::<n::Initialized>(json!({}));
        c
    }

    fn request<R: r::Request>(&mut self, params: Value) -> Value {
        self.call(R::METHOD, params).unwrap_or_else(|message| panic!("{} failed: {message}", R::METHOD))
    }

    /// Sends request `method`; `Err` is the server's error message.
    fn call(&mut self, method: &str, params: Value) -> Result<Value, String> {
        self.next += 1;
        let id = RequestId::from(self.next);
        self.conn.sender.send(Message::Request(Request::new(id.clone(), method.to_owned(), params))).unwrap();
        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.conn.receiver.recv_timeout(left).unwrap_or_else(|_| panic!("no answer to {method}")) {
                Message::Response(response) if response.id == id => {
                    return match response.error {
                        Some(error) => Err(error.message),
                        None => Ok(response.result.unwrap_or(Value::Null)),
                    };
                }
                Message::Notification(notification) => self.notifications.push(notification),
                // The server asking us something (progress tokens): answer it.
                Message::Request(request) => {
                    let _ =
                        self.conn.sender.send(Message::Response(lsp_server::Response::new_ok(request.id.clone(), ())));
                    self.requests.push(request);
                }
                Message::Response(_) => {}
            }
        }
    }

    fn notify<N: n::Notification>(&self, params: Value) {
        self.conn.sender.send(Message::Notification(Notification::new(N::METHOD.to_owned(), params))).unwrap();
    }

    fn notify_custom(&self, method: &str, params: Value) {
        self.conn.sender.send(Message::Notification(Notification::new(method.to_owned(), params))).unwrap();
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

    /// Asks custom request `method` until `done` accepts the answer. A refusal is retried
    /// too: until the index has caught up the server may not know the classes asked about.
    fn call_until(&mut self, method: &str, params: Value, done: impl Fn(&Value) -> bool) -> Value {
        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            let late = Instant::now() > deadline;
            match self.call(method, params.clone()) {
                Ok(found) if late || done(&found) => return found,
                Err(message) if late => panic!("{method}: {message}"),
                _ => std::thread::sleep(Duration::from_millis(200)),
            }
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
fn a_setting_that_does_not_fit_is_named_and_the_others_apply() {
    let root = fixture("php-library");
    let dir = tempfile::tempdir().unwrap();
    copy_dir(&root, dir.path());
    let root = dunce::canonicalize(dir.path()).unwrap();
    let mut c = Client::start(&root);
    let file = root.join("src/Broken.php");
    std::fs::write(&file, "<?php\nnamespace App;\n\nfunction f(): void {\n    new UnknownThing();\n}\n").unwrap();
    let uri = c.open(&file, "php");
    c.diagnostics(&uri, |d| !d.is_empty());

    let settings = json!({ "phpVersion": 8, "inspections": { "PhpUndefinedClass": { "enabled": false } } });
    c.notify::<n::DidChangeConfiguration>(json!({ "settings": { "phoxite": settings } }));
    // The inspection is off although the PHP version next to it is no text.
    c.diagnostics(&uri, |d| d.is_empty());
    let warnings = |c: &Client| -> Vec<String> {
        c.notifications
            .iter()
            .filter(|n| n.method == n::ShowMessage::METHOD)
            .map(|n| n.params["message"].as_str().unwrap().to_owned())
            .collect()
    };
    assert_eq!(warnings(&c), ["Phoxite left `phpVersion` at the default: the value isn't one it understands."]);

    // The same settings again: the user isn't told twice.
    c.notify::<n::DidChangeConfiguration>(json!({ "settings": { "phoxite": settings } }));
    c.request::<r::DocumentSymbolRequest>(json!({ "textDocument": { "uri": uri } }));
    assert_eq!(warnings(&c).len(), 1);
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
    // A client without a Blade language sends the template as PHP: the file name decides.
    let blade = c.open(&root.join("resources/views/about.blade.php"), "php");
    let outline = c.request::<r::DocumentSymbolRequest>(json!({ "textDocument": { "uri": blade } }));
    assert!(outline.is_array(), "{outline}");
    c.shutdown();
}

/// A Symfony project written here: the two framework classes route names
/// are about, a YAML route and a controller naming it.
fn symfony_project() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = dunce::canonicalize(dir.path()).unwrap();
    let files = [
        ("composer.json", r#"{"require": {"symfony/framework-bundle": "^7.0"}}"#),
        (
            "lib/symfony.php",
            "<?php\nnamespace Symfony\\Bundle\\FrameworkBundle;\nclass FrameworkBundle {}\n\nnamespace Symfony\\Bundle\\FrameworkBundle\\Controller;\nabstract class AbstractController {\n    protected function generateUrl(string $route, array $parameters = []): string { return ''; }\n}\n",
        ),
        ("config/routes.yaml", "home:\n    path: /\nabout:\n    path: /about\n"),
        (
            "src/Controller/HomeController.php",
            "<?php\nnamespace App\\Controller;\n\nuse Symfony\\Bundle\\FrameworkBundle\\Controller\\AbstractController;\n\nclass HomeController extends AbstractController\n{\n    public function index(): string\n    {\n        return $this->generateUrl('about');\n    }\n}\n",
        ),
    ];
    for (rel, text) in files {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    (dir, root)
}

#[test]
fn symfony_route_names_go_to_their_route_and_rename_with_it() {
    let (_dir, root) = symfony_project();
    let mut c = Client::start(&root);
    let controller = root.join("src/Controller/HomeController.php");
    let uri = c.open(&controller, "php");
    let (line, col) = find(&controller, "'about')", 2);
    let found =
        c.request_until::<r::GotoDefinition>(at(&uri, line, col), |v| v.as_array().is_some_and(|a| !a.is_empty()));
    assert!(found.to_string().contains("config/routes.yaml"), "{found}");
    assert_eq!(found[0]["range"]["start"]["line"], 2, "{found}");

    let mut rename = at(&uri, line, col);
    rename["newName"] = json!("about_us");
    let edit = c.request::<r::Rename>(rename).to_string();
    assert!(edit.contains("config/routes.yaml") && edit.contains("HomeController.php"), "{edit}");
    assert!(edit.contains("about_us"), "{edit}");
    c.shutdown();
}

#[test]
fn wordpress_hooks_in_a_plugin_opened_on_its_own() {
    let dir = tempfile::tempdir().unwrap();
    let root = dunce::canonicalize(dir.path()).unwrap();
    // A plugin's folder is the workspace: its header makes it a WordPress project.
    let plugin = root.join("shop.php");
    let files = [
        (
            "stubs/wordpress.php",
            "<?php\n/** @param callable $callback */\nfunction add_action($hook_name, $callback, $priority = 10, $accepted_args = 1) {}\nfunction do_action($hook_name, ...$arg) {}\n",
        ),
        (
            "shop.php",
            "<?php\n/*\n * Plugin Name: Shop\n */\n\nfunction shop_boot() {}\n\ndo_action('shop_loaded', 1);\nadd_action('shop_loaded', 'shop_boot');\nadd_action('', 'shop_boot');\n",
        ),
    ];
    for (rel, text) in files {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    let mut c = Client::start(&root);
    let uri = c.open(&plugin, "php");

    // The hook goes to where it is fired, the callback to its function.
    let (line, col) = find(&plugin, "add_action('shop_loaded'", 14);
    let found =
        c.request_until::<r::GotoDefinition>(at(&uri, line, col), |v| v.as_array().is_some_and(|a| !a.is_empty()));
    assert_eq!(found[0]["range"]["start"]["line"], 7, "{found}");
    let (line, col) = find(&plugin, "'shop_loaded', 'shop_boot'", 18);
    // (Once the index has the file declaring `add_action`.)
    let found =
        c.request_until::<r::GotoDefinition>(at(&uri, line, col), |v| v.as_array().is_some_and(|a| !a.is_empty()));
    assert_eq!(found[0]["range"]["start"]["line"], 5, "{found}");

    // Completion offers the hooks fired, and a rename changes every call.
    let (line, col) = find(&plugin, "add_action('', ", 12);
    let items = c.request::<r::Completion>(at(&uri, line, col)).to_string();
    assert!(items.contains("\"shop_loaded\""), "{items}");
    let (line, col) = find(&plugin, "do_action('shop_loaded'", 12);
    let mut rename = at(&uri, line, col);
    rename["newName"] = json!("shop_ready");
    let edit = c.request::<r::Rename>(rename).to_string();
    assert_eq!(edit.matches("shop_ready").count(), 2, "{edit}");

    // The client is told what the project is, for its status item.
    let status = c.call(crate::laravel::STATUS, json!({})).expect("answered");
    assert_eq!(status["frameworks"], json!(["WordPress"]), "{status}");
    assert_eq!(status["laravel"], false);
    c.shutdown();
}

#[test]
fn a_script_of_a_wordpress_plugin_names_what_php_declares() {
    let dir = tempfile::tempdir().unwrap();
    let root = dunce::canonicalize(dir.path()).unwrap();
    let script = root.join("assets/shop.js");
    let files = [
        (
            "stubs/wordpress.php",
            "<?php\n/** @param callable $callback */\nfunction add_action($hook_name, $callback, $priority = 10, $accepted_args = 1) {}\nfunction register_block_type($block_type, $args = []) {}\nfunction register_rest_route($route_namespace, $route, $args = []) {}\n",
        ),
        (
            "shop.php",
            "<?php\n/*\n * Plugin Name: Shop\n */\n\nregister_block_type('shop/price');\nregister_rest_route('shop/v1', '/books', []);\nadd_action('wp_ajax_shop_add', 'shop_add');\nfunction shop_add() {}\n",
        ),
        (
            "assets/shop.js",
            "registerBlockType('shop/price', {});\napiFetch({ path: '/shop/v1/books' });\njQuery.post(ajaxurl, { action: 'shop_add' });\njQuery.post(ajaxurl, { action: '' });\n",
        ),
    ];
    for (rel, text) in files {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    let mut c = Client::start(&root);
    let uri = c.open(&script, "javascript");
    let filled = |v: &serde_json::Value| v.as_array().is_some_and(|a| !a.is_empty());

    // A block, a REST route and an AJAX action go to the PHP declaring or handling them.
    for (needle, line) in [("'shop/price'", 5), ("'/shop/v1/books'", 6), ("'shop_add'", 7)] {
        let (at_line, col) = find(&script, needle, 4);
        let found = c.request_until::<r::GotoDefinition>(at(&uri, at_line, col), filled);
        assert!(found[0]["uri"].as_str().is_some_and(|uri| uri.ends_with("shop.php")), "{found}");
        assert_eq!(found[0]["range"]["start"]["line"], line, "{needle}: {found}");
    }
    let (line, col) = find(&script, "'shop/price'", 4);
    let hover = c.request::<r::HoverRequest>(at(&uri, line, col)).to_string();
    assert!(hover.contains("shop/price"), "{hover}");
    let mut references = at(&uri, line, col);
    references["context"] = json!({ "includeDeclaration": true });
    let found = c.request::<r::References>(references);
    assert_eq!(found.as_array().map(Vec::len), Some(2), "{found}");

    // Completion of an action, and a rename of one from the script.
    let (line, col) = find(&script, "action: ''", 9);
    let items = c.request::<r::Completion>(at(&uri, line, col)).to_string();
    assert!(items.contains("\"shop_add\""), "{items}");
    let (line, col) = find(&script, "'shop_add'", 4);
    let mut rename = at(&uri, line, col);
    rename["newName"] = json!("shop_put");
    let edit = c.request::<r::Rename>(rename).to_string();
    assert_eq!(edit.matches("shop_put").count(), 2, "{edit}");

    // The client was asked for the scripts, as this is a WordPress project.
    let asked = |c: &Client| {
        c.requests
            .iter()
            .filter(|r| r.method == "client/registerCapability")
            .flat_map(|r| r.params["registrations"].as_array().cloned().unwrap_or_default())
            .filter(|registration| registration["id"].as_str().is_some_and(|id| id.starts_with("phoxite/scripts/")))
            .collect::<Vec<_>>()
    };
    // (The server asks after it has answered initialize: read on until its requests are in.)
    for _ in 0..50 {
        if asked(&c).len() >= 8 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
        let _ = c.call(crate::laravel::STATUS, json!({}));
    }
    let registrations = asked(&c);
    assert_eq!(registrations.len(), 8, "{registrations:?}");
    assert_eq!(registrations[0]["method"], "textDocument/didOpen");
    assert_eq!(registrations[0]["registerOptions"]["documentSelector"][0]["language"], "javascript");
    c.shutdown();

    // Not in another project.
    let (_dir, root) = laravel_project();
    let mut c = Client::start(&root);
    c.call_until(crate::laravel::ROUTES, json!({}), |v| v.as_array().is_some_and(|a| !a.is_empty()));
    assert!(asked(&c).is_empty());
    c.shutdown();
}

#[test]
fn a_drupal_module_names_what_its_yaml_files_declare() {
    let dir = tempfile::tempdir().unwrap();
    let root = dunce::canonicalize(dir.path()).unwrap();
    copy_dir(&fixture("drupal-app"), &root);
    let module = root.join("modules/shop/shop.module");
    let routing = root.join("modules/shop/shop.routing.yml");
    let mut c = Client::start(&root);
    // `shop.module` is PHP whatever the client calls it.
    let uri = c.open(&module, "plaintext");
    let filled = |v: &Value| v.as_array().is_some_and(|a| !a.is_empty());

    // A route goes to its key in the routing file; a hook's parameter is typed by the hook.
    let (line, col) = find(&module, "setRedirect('shop.books')", 15);
    let found = c.request_until::<r::GotoDefinition>(at(&uri, line, col), filled);
    assert!(found[0]["uri"].as_str().is_some_and(|uri| uri.ends_with("shop.routing.yml")), "{found}");
    assert_eq!(found[0]["range"]["start"]["line"], 0, "{found}");
    let (line, col) = find(&module, "$form_state->setRedirect", 15);
    let found = c.request_until::<r::GotoDefinition>(at(&uri, line, col), filled);
    assert!(found[0]["uri"].as_str().is_some_and(|uri| uri.ends_with("core/lib/Drupal.php")), "{found}");

    // A route written outside the editor is offered once the client reports the file.
    let (line, col) = find(&module, "setRedirect('shop.books')", 13);
    let routes = c.request::<r::Completion>(at(&uri, line, col)).to_string();
    assert!(routes.contains("\"shop.book\"") && !routes.contains("\"shop.authors\""), "{routes}");
    let mut text = std::fs::read_to_string(&routing).unwrap();
    text.push_str("shop.authors:\n  path: '/authors'\n");
    std::fs::write(&routing, text).unwrap();
    c.notify::<n::DidChangeWatchedFiles>(
        json!({ "changes": [{ "uri": Url::from_file_path(&routing).unwrap(), "type": 2 }] }),
    );
    c.request_until::<r::Completion>(at(&uri, line, col), |v| v.to_string().contains("\"shop.authors\""));

    // A theme's template includes a module's by its namespace.
    let page = root.join("themes/shelf/templates/page.html.twig");
    let template = c.open(&page, "twig");
    let (line, col) = find(&page, "@shop/shop-book.html.twig", 8);
    let found = c.request_until::<r::GotoDefinition>(at(&template, line, col), filled);
    assert!(found[0]["uri"].as_str().is_some_and(|uri| uri.ends_with("shop/templates/shop-book.html.twig")), "{found}");

    // And attaches a library, which goes to the libraries file.
    let book = root.join("modules/shop/templates/shop-book.html.twig");
    let template = c.open(&book, "twig");
    let (line, col) = find(&book, "\"shop/cart\"", 4);
    let found = c.request_until::<r::GotoDefinition>(at(&template, line, col), filled);
    assert!(found[0]["uri"].as_str().is_some_and(|uri| uri.ends_with("shop.libraries.yml")), "{found}");

    // The routing file itself: a controller's method, a permission, and completion of permissions.
    let yaml = c.open(&routing, "yaml");
    let (line, col) = find(&routing, "BookController::list", 17);
    let found = c.request_until::<r::GotoDefinition>(at(&yaml, line, col), filled);
    assert!(found[0]["uri"].as_str().is_some_and(|uri| uri.ends_with("BookController.php")), "{found}");
    assert_eq!(found[0]["range"]["start"]["line"], 18, "{found}");
    let (line, col) = find(&routing, "administer shop+", 3);
    let found = c.request::<r::GotoDefinition>(at(&yaml, line, col));
    assert!(found[0]["uri"].as_str().is_some_and(|uri| uri.ends_with("shop.permissions.yml")), "{found}");
    let items = c.request::<r::Completion>(at(&yaml, line, col)).to_string();
    assert!(items.contains("\"access shop\""), "{items}");
    let mut references = at(&yaml, 0, 3);
    references["context"] = json!({ "includeDeclaration": false });
    let found = c.request::<r::References>(references);
    assert!(found.as_array().is_some_and(|found| found.len() >= 3), "the usages of the route `shop.books`: {found}");

    // The client was asked for the YAML files, as this is a Drupal project (and not for scripts).
    let asked: Vec<String> = c
        .requests
        .iter()
        .filter(|r| r.method == "client/registerCapability")
        .flat_map(|r| r.params["registrations"].as_array().cloned().unwrap_or_default())
        .filter_map(|registration| registration["id"].as_str().map(str::to_owned))
        .collect();
    assert_eq!(asked.iter().filter(|id| id.starts_with("phoxite/yaml/")).count(), 8, "{asked:?}");
    assert!(!asked.iter().any(|id| id.starts_with("phoxite/scripts/")), "{asked:?}");

    let status = c.call(crate::laravel::STATUS, json!({})).expect("answered");
    assert_eq!(status["frameworks"], json!(["Drupal"]), "{status}");
    c.shutdown();
}

#[test]
fn a_codeigniter_application_names_its_views_and_types_them() {
    let dir = tempfile::tempdir().unwrap();
    let root = dunce::canonicalize(dir.path()).unwrap();
    copy_dir(&fixture("codeigniter-app"), &root);
    let controller = root.join("app/Controllers/Books.php");
    let view = root.join("app/Views/books/list.php");
    let mut c = Client::start(&root);
    let uri = c.open(&controller, "php");
    let filled = |v: &Value| v.as_array().is_some_and(|a| !a.is_empty());

    // A view's name goes to its file, a language line to its file.
    let (line, col) = find(&controller, "view('books/list'", 8);
    let found = c.request_until::<r::GotoDefinition>(at(&uri, line, col), filled);
    assert!(found[0]["uri"].as_str().is_some_and(|uri| uri.ends_with("Views/books/list.php")), "{found}");
    let (line, col) = find(&controller, "lang('Shop.cart.empty')", 8);
    let found = c.request::<r::GotoDefinition>(at(&uri, line, col));
    assert!(found[0]["uri"].as_str().is_some_and(|uri| uri.ends_with("Language/en/Shop.php")), "{found}");
    let items = c.request::<r::Completion>(at(&uri, line, col)).to_string();
    assert!(items.contains("\"Shop.title\""), "{items}");

    // The view has what the controller passes it, with its type, and reports nothing.
    let page = c.open(&view, "php");
    let (line, col) = find(&view, "$catalog->count()", 12);
    let found = c.request_until::<r::GotoDefinition>(at(&page, line, col), filled);
    assert!(found[0]["uri"].as_str().is_some_and(|uri| uri.ends_with("Libraries/Catalog.php")), "{found}");
    let (line, col) = find(&view, "$catalog->count()", 3);
    let hover = c.request::<r::HoverRequest>(at(&page, line, col)).to_string();
    assert!(hover.contains("Catalog"), "{hover}");

    let status = c.call(crate::laravel::STATUS, json!({})).expect("answered");
    assert_eq!(status["frameworks"], json!(["CodeIgniter"]), "{status}");
    c.shutdown();
}

#[test]
fn a_twig_template_names_routes_and_templates_of_a_symfony_project() {
    let (_dir, root) = symfony_project();
    let layout = root.join("templates/base.html.twig");
    let page = root.join("templates/page.html.twig");
    std::fs::create_dir_all(root.join("templates")).unwrap();
    std::fs::write(&layout, "{% block body %}{% endblock %}\n").unwrap();
    std::fs::write(
        &page,
        "{% extends 'base.html.twig' %}\n<a href=\"{{ path('about') }}\">x</a>\n<a href=\"{{ path('') }}\">y</a>\n",
    )
    .unwrap();
    let mut c = Client::start(&root);
    let uri = c.open(&page, "twig");
    // Until the index is there the project isn't known to be a Symfony one.
    let (line, col) = find(&page, "path('')", 6);
    let routes = c.request_until::<r::Completion>(at(&uri, line, col), |v| v.to_string().contains("\"home\""));
    assert!(routes.to_string().contains("\"about\""), "{routes}");

    // Goto from the route name: its key in the YAML file.
    let (line, col) = find(&page, "'about'", 2);
    let found = c.request::<r::GotoDefinition>(at(&uri, line, col));
    assert!(found.to_string().contains("config/routes.yaml"), "{found}");

    // Its usages: the controller, this template, and the route itself.
    let mut params = at(&uri, line, col);
    params["context"] = json!({ "includeDeclaration": true });
    // The controller's comes from the index, once it has the file.
    let usages = c.request_until::<r::References>(params, |v| v.to_string().contains("HomeController.php")).to_string();
    for file in ["HomeController.php", "page.html.twig", "routes.yaml"] {
        assert!(usages.contains(file), "{file}: {usages}");
    }

    // Renamed from the template: the YAML key, the controller's string and the template's.
    let mut params = at(&uri, line, col);
    params["newName"] = json!("about_us");
    let edit = c.request::<r::Rename>(params).to_string();
    for file in ["HomeController.php", "page.html.twig", "routes.yaml"] {
        assert!(edit.contains(file), "{file}: {edit}");
    }

    // A template's name is renamed with its file.
    let (line, col) = find(&page, "base.html.twig", 2);
    let range = c.request::<r::PrepareRenameRequest>(at(&uri, line, col));
    assert_eq!(range["start"]["character"], 12, "{range}");
    let mut params = at(&uri, line, col);
    params["newName"] = json!("layout.html.twig");
    let edit = c.request::<r::Rename>(params).to_string();
    assert!(edit.contains("page.html.twig") && edit.contains("layout.html.twig"), "{edit}");
    assert!(edit.contains("\"kind\":\"rename\"") || edit.contains("oldUri"), "the file moves: {edit}");
    c.shutdown();
}

#[test]
fn a_twig_template_completes_what_the_controller_passes_it() {
    let (_dir, root) = symfony_project();
    std::fs::write(
        root.join("lib/symfony.php"),
        "<?php\nnamespace Symfony\\Bundle\\FrameworkBundle;\nclass FrameworkBundle {}\n\nnamespace Symfony\\Bundle\\FrameworkBundle\\Controller;\nabstract class AbstractController {\n    protected function render(string $view, array $parameters = []): string { return ''; }\n}\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src/Controller/HomeController.php"),
        "<?php\nnamespace App\\Controller;\n\nuse Symfony\\Bundle\\FrameworkBundle\\Controller\\AbstractController;\n\nclass Post\n{\n    public string $title = '';\n    public function getAuthor(): Author { return new Author(); }\n}\nclass Author { public string $name = ''; }\n\nclass HomeController extends AbstractController\n{\n    public function index(Post $post): string\n    {\n        return $this->render('home.html.twig', ['post' => $post, 'posts' => [$post]]);\n    }\n}\n",
    )
    .unwrap();
    let page = root.join("templates/home.html.twig");
    std::fs::create_dir_all(root.join("templates")).unwrap();
    std::fs::write(
        &page,
        "{{ p }}\n{{ post. }}\n{{ post.author. }}\n{% for item in posts %}{{ item. }}{% endfor %}\n{{ post.author.name }}\n",
    )
    .unwrap();
    let mut c = Client::start(&root);
    let uri = c.open(&page, "twig");

    // The variables, once the index has the controller.
    let variables = c.request_until::<r::Completion>(at(&uri, 0, 4), |v| v.to_string().contains("\"posts\""));
    assert!(variables.to_string().contains("\"post\""), "{variables}");
    // After a dot: a property, and a getter under the name Twig reads it by.
    let members = c.request::<r::Completion>(at(&uri, 1, 8)).to_string();
    assert!(members.contains("\"title\"") && members.contains("\"author\""), "{members}");
    assert!(!members.contains("\"getAuthor\"") && !members.contains("\"upper\""), "{members}");
    let author = c.request::<r::Completion>(at(&uri, 2, 15)).to_string();
    assert!(author.contains("\"name\""), "{author}");
    // A loop's variable is one of what the loop goes through.
    let (line, col) = find(&page, "item. ", 5);
    let item = c.request::<r::Completion>(at(&uri, line, col)).to_string();
    assert!(item.contains("\"title\""), "{item}");

    // A member that is written: its type on hover, and goto to the property or the getter.
    let (line, col) = find(&page, "post.author.name", 13);
    let hover = c.request::<r::HoverRequest>(at(&uri, line, col)).to_string();
    assert!(hover.contains("name: string"), "{hover}");
    let found = c.request::<r::GotoDefinition>(at(&uri, line, col));
    assert!(found.to_string().contains("HomeController.php"), "{found}");
    assert_eq!(found[0]["range"]["start"]["line"], 10, "the property of Author: {found}");
    let (line, col) = find(&page, "post.author.name", 6);
    let found = c.request::<r::GotoDefinition>(at(&uri, line, col));
    assert_eq!(found[0]["range"]["start"]["line"], 8, "the getter Twig calls: {found}");

    // What the editor has, unsaved, is what the project's usages see.
    c.notify::<n::DidChangeTextDocument>(json!({
        "textDocument": { "uri": uri, "version": 2 },
        "contentChanges": [{ "text": "<a href=\"{{ path('about') }}\">x</a>\n" }]
    }));
    std::fs::write(
        root.join("src/Controller/Links.php"),
        "<?php\nnamespace App\\Controller;\n\nuse Symfony\\Bundle\\FrameworkBundle\\Controller\\AbstractController;\n\nclass Links extends AbstractController\n{\n    public function go(): string\n    {\n        return $this->generateUrl('about');\n    }\n}\n",
    )
    .unwrap();
    let links = root.join("src/Controller/Links.php");
    let links_uri = c.open(&links, "php");
    let (line, col) = find(&links, "'about')", 2);
    let mut params = at(&links_uri, line, col);
    params["context"] = json!({ "includeDeclaration": false });
    let usages = c.request_until::<r::References>(params, |v| v.to_string().contains("home.html.twig"));
    assert!(usages.to_string().contains("home.html.twig"), "the unsaved template names the route: {usages}");
    c.shutdown();
}

#[test]
fn a_framework_turned_off_in_the_settings_has_no_string_references() {
    let (_dir, root) = symfony_project();
    let mut c = Client::start(&root);
    let controller = root.join("src/Controller/HomeController.php");
    let uri = c.open(&controller, "php");
    let (line, col) = find(&controller, "'about')", 2);
    // On: the route's key in the YAML file.
    c.request_until::<r::GotoDefinition>(at(&uri, line, col), |v| v.as_array().is_some_and(|a| !a.is_empty()));
    c.notify::<n::DidChangeConfiguration>(
        json!({ "settings": { "phoxite": { "frameworks": { "symfony": "disabled" } } } }),
    );
    let found = c.request_until::<r::GotoDefinition>(at(&uri, line, col), |v| !v.to_string().contains("routes.yaml"));
    assert!(!found.to_string().contains("routes.yaml"), "{found}");
    c.shutdown();
}

/// A project with a layout, a template of macros and a page using both.
fn twig_project() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = dunce::canonicalize(dir.path()).unwrap();
    let files = [
        ("composer.json", r#"{"require": {"symfony/twig-bundle": "^7.0"}}"#),
        ("templates/base.html.twig", "<html>\n{% block body %}{% endblock %}\n</html>\n"),
        ("templates/forms.html.twig", "{% macro input(name) %}<input name=\"{{ name }}\">{% endmacro %}\n"),
        (
            "templates/page.html.twig",
            "{% extends 'base.html.twig' %}\n{% import 'forms.html.twig' as forms %}\n{% block body %}\n  {{ forms.input('q') }}\n  {{ title|upper }}\n{% endblock %}\n",
        ),
        ("templates/other.html.twig", "{% extends 'base.html.twig' %}\n"),
    ];
    for (rel, text) in files {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    (dir, root)
}

#[test]
fn twig_templates_are_served_without_an_index() {
    let (_dir, root) = twig_project();
    let mut c = Client::start(&root);
    let page = root.join("templates/page.html.twig");
    let uri = c.open(&page, "twig");

    // The outline: the parent, the import and the block.
    let outline = c.request::<r::DocumentSymbolRequest>(json!({ "textDocument": { "uri": uri } })).to_string();
    assert!(outline.contains("base.html.twig") && outline.contains("forms") && outline.contains("body"), "{outline}");

    // Filters after `|`, with what Symfony's bridge adds elsewhere.
    let (line, col) = find(&page, "upper", 0);
    let filters = c.request::<r::Completion>(at(&uri, line, col)).to_string();
    assert!(filters.contains("\"upper\"") && filters.contains("\"trans\""), "{filters}");

    // Hover on a filter.
    let hover = c.request::<r::HoverRequest>(at(&uri, line, col + 2)).to_string();
    assert!(hover.contains("upper") && hover.contains("filter"), "{hover}");

    // Goto: the parent template, the macro of the imported template, the block it overrides.
    let (line, col) = find(&page, "base.html.twig", 2);
    let parent = c.request::<r::GotoDefinition>(at(&uri, line, col)).to_string();
    assert!(parent.contains("templates/base.html.twig"), "{parent}");
    let (line, col) = find(&page, "input('q')", 2);
    let found = c.request::<r::GotoDefinition>(at(&uri, line, col));
    assert!(found.to_string().contains("templates/forms.html.twig"), "{found}");
    assert_eq!(found[0]["range"]["start"]["character"], 9, "{found}");
    let (line, col) = find(&page, "block body", 7);
    let block = c.request::<r::GotoDefinition>(at(&uri, line, col)).to_string();
    assert!(block.contains("templates/base.html.twig"), "{block}");

    // Usages of a template name: every template naming it, and the file itself.
    let (line, col) = find(&page, "base.html.twig", 2);
    let mut params = at(&uri, line, col);
    params["context"] = json!({ "includeDeclaration": true });
    let usages = c.request::<r::References>(params);
    let files: Vec<String> = usages.as_array().unwrap().iter().map(|usage| usage["uri"].to_string()).collect();
    assert_eq!(files.len(), 3, "{usages}");
    assert!(files.iter().any(|file| file.contains("other.html.twig")), "{usages}");
    c.shutdown();
}

#[test]
fn a_twig_templates_syntax_errors_are_published_and_cleared() {
    let (_dir, root) = twig_project();
    let mut c = Client::start(&root);
    let page = root.join("templates/broken.html.twig");
    std::fs::write(&page, "{% if a %}\n{{ a| }}\n").unwrap();
    // A client without a Twig language: the file name decides.
    let uri = c.open(&page, "plaintext");
    let problems = c.diagnostics(&uri, |d| d.len() == 2);
    assert!(problems.iter().all(|d| d["source"] == "twig" && d["code"] == "TwigSyntax"), "{problems:?}");
    assert!(problems.iter().any(|d| d["message"] == "Unclosed 'if' block"), "{problems:?}");

    c.notify::<n::DidChangeTextDocument>(json!({
        "textDocument": { "uri": uri, "version": 2 },
        "contentChanges": [{ "text": "{% if a %}\n{{ a|upper }}\n{% endif %}\n" }]
    }));
    assert!(c.diagnostics(&uri, |d| d.is_empty()).is_empty());
    c.shutdown();
}

#[test]
fn twig_rename_formatting_and_semantic_tokens() {
    let (_dir, root) = twig_project();
    let mut c = Client::start(&root);
    let page = root.join("templates/page.html.twig");
    let uri = c.open(&page, "twig");

    // A block: here and in the parent it overrides.
    let (line, col) = find(&page, "block body", 7);
    let range = c.request::<r::PrepareRenameRequest>(at(&uri, line, col));
    assert_eq!(range["start"]["character"], 9, "{range}");
    let mut params = at(&uri, line, col);
    params["newName"] = json!("main");
    let edit = c.request::<r::Rename>(params).to_string();
    assert!(edit.contains("page.html.twig") && edit.contains("base.html.twig") && edit.contains("main"), "{edit}");
    assert!(!edit.contains("other.html.twig"), "it has no such block: {edit}");

    // A macro, from its call: the call and the declaration.
    let (line, col) = find(&page, "input('q')", 2);
    let mut params = at(&uri, line, col);
    params["newName"] = json!("field");
    let edit = c.request::<r::Rename>(params).to_string();
    assert!(edit.contains("page.html.twig") && edit.contains("forms.html.twig"), "{edit}");
    // A template's name isn't renamed.
    let (line, col) = find(&page, "base.html.twig", 2);
    assert!(c.call("textDocument/prepareRename", at(&uri, line, col)).is_err());

    // Formatting: the spacing inside the tags.
    c.notify::<n::DidChangeTextDocument>(json!({
        "textDocument": { "uri": uri, "version": 2 },
        "contentChanges": [{ "text": "<p>\n{{title | upper}}\n</p>\n{%if a%}x{% endif %}\n" }]
    }));
    let edits = c.request::<r::Formatting>(json!({
        "textDocument": { "uri": uri }, "options": { "tabSize": 4, "insertSpaces": true }
    }));
    let texts: Vec<&str> = edits.as_array().unwrap().iter().map(|edit| edit["newText"].as_str().unwrap()).collect();
    // A line per change: the indentation, and the spacing inside the tags.
    assert_eq!(texts, ["    {{ title|upper }}", "{% if a %}x{% endif %}"]);

    // Semantic tokens: `title` and `upper`, then `a`.
    let tokens = c.request::<r::SemanticTokensFullRequest>(json!({ "textDocument": { "uri": uri } }));
    assert_eq!(tokens["data"].as_array().unwrap().len(), 3 * 5, "{tokens}");
    c.shutdown();
}

#[test]
fn twig_syntax_errors_are_off_when_the_inspection_is() {
    let (_dir, root) = twig_project();
    let broken = root.join("templates/broken.html.twig");
    std::fs::write(&broken, "{% if a %}\n").unwrap();
    let other = root.join("templates/other.html.twig");

    // The inspection turned off in the settings.
    let mut c = Client::start(&root);
    c.notify::<n::DidChangeConfiguration>(
        json!({ "settings": { "phoxite": { "inspections": { "TwigSyntax": { "enabled": false } } } } }),
    );
    let uri = c.open(&broken, "twig");
    // A second document's diagnostics come after the first one's.
    let later = c.open(&other, "twig");
    c.diagnostics(&later, |_| true);
    assert!(c.diagnostics(&uri, |_| true).is_empty());
    c.shutdown();
}

#[test]
fn a_twig_template_knows_the_filters_the_projects_php_adds() {
    let (_dir, root) = twig_project();
    std::fs::create_dir_all(root.join("src/Twig")).unwrap();
    std::fs::write(
        root.join("src/Twig/AppExtension.php"),
        "<?php\nnamespace App\\Twig;\n\nuse Twig\\TwigFilter;\n\nclass AppExtension\n{\n    public function getFilters(): array\n    {\n        return [new TwigFilter('price', [$this, 'price'])];\n    }\n}\n",
    )
    .unwrap();
    let page = root.join("templates/priced.html.twig");
    std::fs::write(&page, "{{ total|price }}\n{{ total|upper }}\n").unwrap();
    let mut c = Client::start(&root);
    let uri = c.open(&page, "twig");

    // After `|`: the project's filter beside Twig's own, once the index has the extension.
    let (line, col) = find(&page, "upper", 0);
    let filters = c.request_until::<r::Completion>(at(&uri, line, col), |v| v.to_string().contains("\"price\""));
    assert!(filters.to_string().contains("\"price\"") && filters.to_string().contains("\"upper\""), "{filters}");
    // Not where a function is written.
    let functions = c.request::<r::Completion>(at(&uri, 0, 3)).to_string();
    assert!(!functions.contains("\"price\""), "{functions}");

    // Goto: where PHP names it.
    let (line, col) = find(&page, "price", 2);
    let found = c.request::<r::GotoDefinition>(at(&uri, line, col));
    assert!(found.to_string().contains("AppExtension.php"), "{found}");
    assert_eq!(found[0]["range"]["start"]["line"], 9, "{found}");
    c.shutdown();
}

#[test]
fn a_new_template_is_a_template_name_once_the_client_reports_it() {
    let (_dir, root) = twig_project();
    let mut c = Client::start(&root);
    let page = root.join("templates/page.html.twig");
    let uri = c.open(&page, "twig");
    let (line, col) = find(&page, "base.html.twig", 0);
    let names = |c: &mut Client| c.request::<r::Completion>(at(&uri, line, col)).to_string();
    assert!(names(&mut c).contains("other.html.twig"));
    assert!(!names(&mut c).contains("fresh.html.twig"));

    let fresh = root.join("templates/fresh.html.twig");
    std::fs::write(&fresh, "x").unwrap();
    c.notify::<n::DidChangeWatchedFiles>(json!({
        "changes": [{ "uri": Url::from_file_path(&fresh).unwrap(), "type": 1 }]
    }));
    let found = c.request_until::<r::Completion>(at(&uri, line, col), |v| v.to_string().contains("fresh.html.twig"));
    assert!(found.to_string().contains("fresh.html.twig"), "{found}");
    c.shutdown();
}

#[test]
fn a_pipe_opens_completion_in_twig_only() {
    let root = fixture("php-library");
    let mut c = Client::start(&root);
    let file =
        std::fs::read_dir(root.join("src")).unwrap().flatten().map(|entry| entry.path()).find(|path| path.is_file());
    let uri = c.open(&file.unwrap(), "php");
    let mut params = at(&uri, 1, 0);
    params["context"] = json!({ "triggerKind": 2, "triggerCharacter": "|" });
    assert!(c.request::<r::Completion>(params).is_null());
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

/// A copy of the Laravel fixture with the framework stubs as its `vendor`.
fn laravel_project() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = dunce::canonicalize(dir.path()).unwrap();
    copy_dir(&fixture("laravel-app"), &root);
    std::fs::create_dir_all(root.join("vendor")).unwrap();
    copy_dir(&fixture("laravel-stubs"), &root.join("vendor"));
    (dir, root)
}

/// The file names an edit's `created` lists.
fn created(applied: &Value) -> Vec<String> {
    let created = applied["created"].as_array().unwrap_or_else(|| panic!("no `created` in {applied}"));
    created.iter().map(|uri| uri.as_str().unwrap().rsplit('/').next().unwrap().to_owned()).collect()
}

/// The new text of the edit of the file whose URI ends with `name`.
fn new_text(applied: &Value, name: &str) -> String {
    let ops = applied["edit"]["documentChanges"].as_array().unwrap_or_else(|| panic!("no operations in {applied}"));
    ops.iter()
        .find(|op| op["textDocument"]["uri"].as_str().is_some_and(|uri| uri.ends_with(name)))
        .map(|op| op["edits"][0]["newText"].as_str().unwrap().to_owned())
        .unwrap_or_else(|| panic!("no edit of {name} in {applied}"))
}

#[test]
fn laravel_requests_for_the_client_ui() {
    use crate::laravel as l;
    let (_dir, root) = laravel_project();
    let mut c = Client::start(&root);
    assert_eq!(c.call_until(l::STATUS, Value::Null, |v| v["laravel"] == true)["laravel"], true);

    // Routes, all and searched.
    let routes = c.call_until(l::ROUTES, json!({}), |v| v.as_array().is_some_and(|a| !a.is_empty()));
    let show = routes.as_array().unwrap().iter().find(|r| r["name"] == "posts.show").expect("posts.show");
    assert!(show["path"].as_str().unwrap().ends_with("web.php"), "{show}");
    assert!(show["range"]["start"]["line"].is_u64(), "{show}");
    let searched = c.call(l::ROUTES, json!({ "query": "posts.show" })).unwrap();
    assert!(searched.as_array().unwrap().len() < routes.as_array().unwrap().len(), "{searched}");

    // The generators and what their dialogs offer (the models need the index).
    let context = c.call_until(l::GENERATION_CONTEXT, json!({ "folder": Url::from_file_path(&root).unwrap() }), |v| {
        v["models"].as_array().is_some_and(|m| m.iter().any(|m| m == "App\\Models\\Post"))
    });
    let ids: Vec<&str> = context["generators"].as_array().unwrap().iter().filter_map(|g| g["id"].as_str()).collect();
    assert!(ids.contains(&"model") && ids.contains(&"controller"), "{ids:?}");
    assert!(ids.iter().any(|id| id.starts_with("ide-json:")), "{ids:?}");
    assert_eq!(context["namespace"], "App\\");

    // The model view.
    // The migrations and the model are indexed after the routes: ask until both are there.
    let view = c.call_until(l::MODEL_VIEW, json!({ "class": "App\\Models\\Post" }), |v| {
        let has_title = v["columns"].as_array().is_some_and(|c| c.iter().any(|c| c["name"] == "title"));
        has_title && v["relations"].as_array().is_some_and(|r| !r.is_empty())
    });
    assert_eq!(view["table"], "posts");
    let columns: Vec<&str> = view["columns"].as_array().unwrap().iter().filter_map(|c| c["name"].as_str()).collect();
    assert!(columns.contains(&"title"), "{columns:?}");
    assert!(view["relations"].as_array().is_some_and(|r| !r.is_empty()), "{view}");
    assert!(view["location"]["path"].as_str().unwrap().ends_with("Post.php"), "{view}");
    let error = c.call(l::MODEL_VIEW, json!({ "class": "App\\Nope" })).unwrap_err();
    assert!(error.contains("not an Eloquent model"), "{error}");

    // Add columns: a migration and `$fillable`.
    let added = c
        .call(
            l::ADD_COLUMNS,
            json!({
                "class": "App\\Models\\Post",
                "columns": [{ "name": "subtitle", "ty": "string", "nullable": true }],
                "fillable": true,
                "timestamp": "2030_01_02_030405"
            }),
        )
        .unwrap();
    let files = created(&added);
    assert!(files[0].starts_with("2030_01_02_030405_add_subtitle_to_posts_table"), "{files:?}");
    assert!(new_text(&added, &files[0]).contains("->nullable()"), "{added}");

    // Artisan: the project's commands and Laravel's own.
    let commands = c.call(l::ARTISAN_COMMANDS, json!({})).unwrap();
    let publish = commands.as_array().unwrap().iter().find(|c| c["name"] == "posts:publish").expect("posts:publish");
    assert_eq!(publish["source"], "class");
    assert_eq!(publish["synopsis"], "posts:publish [--dry-run]");
    assert!(publish["location"]["path"].as_str().unwrap().ends_with("PublishScheduledPosts.php"), "{publish}");
    assert!(commands.as_array().unwrap().iter().any(|c| c["name"] == "migrate" && c["source"] == "framework"));

    // A class generator.
    let generated = c
        .call(l::GENERATE_CLASS, json!({ "spec": { "generator": "enum", "name": "Visibility", "backing": "string" } }))
        .unwrap();
    assert_eq!(created(&generated), ["Visibility.php"]);
    assert!(new_text(&generated, "Visibility.php").contains("enum Visibility: string"), "{generated}");
    let error =
        c.call(l::GENERATE_CLASS, json!({ "spec": { "generator": "enum", "name": "PostStatus" } })).unwrap_err();
    assert!(error.contains("PostStatus"), "{error}");

    // The user's template replaces the default.
    let templates = c.call(l::TEMPLATES, json!({})).unwrap();
    let template = templates.as_array().unwrap().iter().find(|t| t["name"] == "Laravel Enum").expect("the template");
    assert_eq!(template["edited"], false);
    let path = PathBuf::from(template["path"].as_str().unwrap());
    assert!(path.ends_with(".phoxite/templates/Laravel Enum.ft"), "{path:?}");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, format!("{}// from my template\n", template["body"].as_str().unwrap())).unwrap();
    let generated =
        c.call(l::GENERATE_CLASS, json!({ "spec": { "generator": "enum", "name": "Visibility" } })).unwrap();
    assert!(new_text(&generated, "Visibility.php").contains("// from my template"), "{generated}");
    let templates = c.call(l::TEMPLATES, json!({})).unwrap();
    assert!(templates.as_array().unwrap().iter().any(|t| t["name"] == "Laravel Enum" && t["edited"] == true));

    // New Model: the model and what it ticks.
    let model = c
        .call(
            l::NEW_MODEL,
            json!({
                "spec": {
                    "name": "Widget",
                    "fields": [{ "name": "label", "ty": "string" }],
                    "relations": [{ "kind": "belongsTo", "model": "App\\Models\\User" }],
                    "migration": true, "factory": true, "controller": "resource", "policy": true
                },
                "timestamp": "2030_01_02_030405"
            }),
        )
        .unwrap();
    let files = created(&model);
    for file in ["Widget.php", "2030_01_02_030405_create_widgets_table.php", "WidgetFactory.php", "WidgetPolicy.php"] {
        assert!(files.iter().any(|f| f == file), "{file} not in {files:?}");
    }
    assert!(files.iter().any(|f| f == "WidgetController.php"), "{files:?}");
    assert!(new_text(&model, "Widget.php").contains("function user()"), "{model}");

    // A translation key, in the locales asked for.
    let translated = c
        .call(
            l::CREATE_TRANSLATION,
            json!({ "key": "messages.brand_new", "texts": [{ "locale": "fr", "text": "Tout neuf" }] }),
        )
        .unwrap();
    assert!(translated["created"].as_array().unwrap().is_empty(), "{translated}");
    assert!(translated["edit"].to_string().contains("Tout neuf"), "{translated}");
    assert!(!translated["edit"].to_string().contains("lang/en/"), "{translated}");

    // Its action, on a key no locale has.
    let probe = root.join("app/Probe.php");
    std::fs::write(
        &probe,
        "<?php\nnamespace App;\n\nfunction probe(): string {\n    return __('messages.nowhere_yet');\n}\n",
    )
    .unwrap();
    let uri = c.open(&probe, "php");
    let (line, col) = find(&probe, "messages.nowhere_yet", 3);
    let actions = c.request_until::<r::CodeActionRequest>(
        json!({
            "textDocument": { "uri": uri },
            "range": { "start": { "line": line, "character": col }, "end": { "line": line, "character": col } },
            "context": { "diagnostics": [] }
        }),
        |v| v.to_string().contains("laravel.createTranslation"),
    );
    let action = actions
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["command"]["command"] == "laravel.createTranslation")
        .unwrap_or_else(|| panic!("no action in {actions}"));
    let request: Value = serde_json::from_str(action["command"]["arguments"][0].as_str().unwrap()).unwrap();
    assert_eq!(request["key"], "messages.nowhere_yet");
    let locales: Vec<&str> =
        request["locales"].as_array().unwrap().iter().filter_map(|l| l["locale"].as_str()).collect();
    assert_eq!(locales, ["en", "fr"]);

    // The lens running a console command, on its class.
    let command = c.open(&root.join("app/Console/Commands/PublishScheduledPosts.php"), "php");
    let lenses = c.request_until::<r::CodeLensRequest>(json!({ "textDocument": { "uri": command } }), |v| {
        v.to_string().contains("laravel.runArtisan")
    });
    let lens = lenses.as_array().unwrap().iter().find(|l| l["command"]["command"] == "laravel.runArtisan").unwrap();
    assert_eq!(lens["command"]["arguments"], json!(["posts:publish"]));
    assert_eq!(lens["command"]["title"], "Run artisan posts:publish");
    c.shutdown();
}

#[test]
fn laravel_requests_outside_laravel_projects() {
    use crate::laravel as l;
    let root = fixture("php-library");
    let mut c = Client::start(&root);
    assert_eq!(c.call(l::STATUS, Value::Null).unwrap()["laravel"], false);
    assert_eq!(c.call(l::ROUTES, json!({})).unwrap(), json!([]));
    assert!(c.call(l::ARTISAN_COMMANDS, json!({})).unwrap().is_null());
    assert!(c.call(l::GENERATION_CONTEXT, json!({})).unwrap().is_null());
    let error = c.call(l::GENERATE_CLASS, json!({ "spec": { "generator": "enum", "name": "A" } })).unwrap_err();
    assert!(error.contains("Not a Laravel project"), "{error}");
    c.shutdown();
}

/// A project with a small class hierarchy, for the refactorings.
fn shop_project() -> (tempfile::TempDir, PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let root = dunce::canonicalize(tmp.path()).unwrap();
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join("composer.json"), r#"{ "autoload": { "psr-4": { "App\\": "src/" } } }"#).unwrap();
    std::fs::write(root.join("src/Base.php"), "<?php\n\nnamespace App;\n\nclass Base\n{\n}\n").unwrap();
    std::fs::write(
        root.join("src/Shop.php"),
        "<?php\n\nnamespace App;\n\nclass Shop extends Base\n{\n    private int $count = 0;\n    private string $label = '';\n\n    public function total(int $a, int $b): int\n    {\n        $sum = $a + $b;\n        return $sum * 2;\n    }\n\n    public function run(): int\n    {\n        return $this->total(1, 2);\n    }\n}\n",
    )
    .unwrap();
    (tmp, root)
}

fn actions_at(c: &mut Client, uri: &Url, start: (u32, u32), end: (u32, u32), title: &str) -> Vec<Value> {
    let params = json!({
        "textDocument": { "uri": uri },
        "range": { "start": { "line": start.0, "character": start.1 }, "end": { "line": end.0, "character": end.1 } },
        "context": { "diagnostics": [] }
    });
    let found = c.request_until::<r::CodeActionRequest>(params, |v| {
        v.as_array().is_some_and(|a| a.iter().any(|x| x["title"].as_str().is_some_and(|t| t.contains(title))))
    });
    found.as_array().cloned().unwrap_or_default()
}

fn titled<'a>(actions: &'a [Value], title: &str) -> &'a Value {
    actions.iter().find(|a| a["title"].as_str().is_some_and(|t| t.contains(title))).unwrap_or_else(|| {
        panic!("no action {title:?} in {:?}", actions.iter().map(|a| &a["title"]).collect::<Vec<_>>())
    })
}

#[test]
fn refactorings_with_a_dialog() {
    let (_tmp, root) = shop_project();
    let shop = root.join("src/Shop.php");
    let mut c = Client::start_with(&root, json!({ "phoxite": { "dialogs": true } }));
    let uri = c.open(&shop, "php");

    // Change Signature: the action carries the signature; the dialog's answer comes back.
    let name = find(&shop, "total(int", 0);
    let actions = actions_at(&mut c, &uri, name, name, "Change signature");
    let action = titled(&actions, "Change signature of 'Shop::total'…");
    assert_eq!(action["command"]["command"], "phoxite.changeSignature");
    assert_eq!(action["command"]["arguments"][0], uri.as_str());
    let mut data = action["command"]["arguments"][1].clone();
    let mut change = data["signature"].clone();
    assert_eq!(change["parameters"].as_array().unwrap().len(), 2);
    change["functionName"] = json!("sum");
    change["parameters"].as_array_mut().unwrap().reverse();
    data["change"] = change;
    let changed = c.call("phoxite/refactor/changeSignature", json!({ "uri": uri, "data": data })).unwrap();
    let text = changed["edit"].to_string();
    for part in [r#""newText":"sum""#, r#""newText":"int $b, int $a""#, r#""newText":"2, 1""#] {
        assert!(text.contains(part), "{part} not in {text}");
    }
    let unanswered = action["command"]["arguments"][1].clone();
    let refused = c.call("phoxite/refactor/changeSignature", json!({ "uri": uri, "data": unanswered }));
    assert_eq!(refused.unwrap_err(), "Choose the new signature first");

    // A member chooser replaces the action generating every member.
    let property = find(&shop, "$count", 1);
    let actions = actions_at(&mut c, &uri, property, property, "Generate getters…");
    titled(&actions, "Generate constructor…");
    assert!(!actions.iter().any(|a| a["title"] == "Generate constructor"), "the plain action stays");
    let action = titled(&actions, "Generate getters…");
    assert_eq!(action["command"]["command"], "phoxite.generateMembers");
    let mut data = action["command"]["arguments"][1].clone();
    let members: Vec<&str> = data["members"].as_array().unwrap().iter().filter_map(|m| m["id"].as_str()).collect();
    assert_eq!(members, ["count", "label"]);
    data["chosen"] = json!(["count"]);
    let generated = c.call("phoxite/refactor/generate", json!({ "uri": uri, "data": data })).unwrap();
    let text = generated["edit"].to_string();
    assert!(text.contains("getCount") && !text.contains("getLabel"), "{text}");

    // Extract Method: the defaults, then the chosen name.
    let start = find(&shop, "$sum = $a + $b;", 0);
    let end = (start.0, start.1 + "$sum = $a + $b;".len() as u32);
    let actions = actions_at(&mut c, &uri, start, end, "Extract method…");
    let action = titled(&actions, "Extract method…");
    assert_eq!(action["command"]["command"], "phoxite.extractMethod");
    let mut data = action["command"]["arguments"][1].clone();
    assert_eq!(data["className"], "Shop");
    let mut change = data["defaults"].clone();
    change["name"] = json!("add");
    data["change"] = change;
    let extracted = c.call("phoxite/refactor/extractMethod", json!({ "uri": uri, "data": data })).unwrap();
    let text = extracted["edit"].to_string();
    assert!(text.contains("function add(int $a, int $b)"), "{text}");

    // Pull Members Up: the class, where its members can go, and the checked move.
    let position = json!({ "line": name.0, "character": name.1 });
    let ask = json!({ "uri": uri, "position": position, "kind": "pullUp" });
    let info = c.call_until("phoxite/refactor/hierarchyMembers", ask, |v| !v["targets"].as_array().unwrap().is_empty());
    assert_eq!(info["className"], "Shop");
    assert_eq!(info["targets"][0]["qualifiedName"], "App\\Base");
    let total = info["members"].as_array().unwrap().iter().find(|m| m["id"] == "method:total").unwrap();
    assert_eq!(total["selected"], true);
    let moved = c
        .call(
            "phoxite/refactor/hierarchy",
            json!({
                "uri": uri, "position": position, "kind": "pullUp",
                "target": info["targets"][0]["id"],
                "choices": [{ "id": "method:total", "makeAbstract": false }]
            }),
        )
        .unwrap();
    assert!(moved["problems"].as_array().unwrap().iter().all(|p| p["severity"] != "fatal"), "{moved}");
    assert!(moved["edit"].to_string().contains("Base.php"), "{moved}");
    // Nothing to push down: Shop has no subclasses.
    let down = json!({ "uri": uri, "position": position, "kind": "pushDown" });
    assert!(c.call("phoxite/refactor/hierarchyMembers", down).is_err());
    c.shutdown();

    // A client without the dialogs gets the plain actions only.
    let mut c = Client::start(&root);
    let uri = c.open(&shop, "php");
    let actions = actions_at(&mut c, &uri, property, property, "Generate constructor");
    let titles: Vec<&Value> = actions.iter().map(|a| &a["title"]).collect();
    assert!(actions.iter().any(|a| a["title"] == "Generate constructor"), "{titles:?}");
    assert!(!actions.iter().any(|a| a["command"]["command"].as_str().is_some_and(|c| c.starts_with("phoxite."))));
    c.shutdown();
}

/// A stand-in for the PHP interpreter: whatever it is asked to run, it prints
/// the report next to it. Answers its path.
fn fake_php(dir: &Path, report: &str) -> PathBuf {
    std::fs::write(dir.join("report.json"), report).unwrap();
    if cfg!(windows) {
        let php = dir.join("php.cmd");
        std::fs::write(&php, "@echo off\r\ntype \"%~dp0report.json\"\r\n").unwrap();
        php
    } else {
        let php = dir.join("php");
        std::fs::write(&php, "#!/bin/sh\ncat \"$(dirname \"$0\")/report.json\"\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&php, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        php
    }
}

#[test]
fn analysers_run_on_save() {
    let (_tmp, root) = shop_project();
    let shop = root.join("src/Shop.php");
    std::fs::create_dir_all(root.join("vendor/bin")).unwrap();
    std::fs::write(root.join("vendor/bin/phpstan"), "<?php\n").unwrap();
    let tools = tempfile::tempdir().unwrap();
    let report = json!({
        "totals": { "errors": 0, "file_errors": 1 },
        "files": { shop.to_string_lossy(): { "errors": 1, "messages": [
            { "message": "Method App\\Shop::run() is too clever.", "line": 16, "ignorable": true, "identifier": "shop.clever" }
        ] } },
        "errors": []
    });
    let php = fake_php(tools.path(), &report.to_string());
    let settings = |on: bool| json!({ "settings": { "phoxite": { "analyzers": { "onSave": on, "phpPath": php } } } });
    let from_phpstan = |items: &[Value]| items.iter().filter(|d| d["source"] == "phpstan").cloned().collect::<Vec<_>>();
    let save = |c: &Client, uri: &Url| c.notify::<n::DidSaveTextDocument>(json!({ "textDocument": { "uri": uri } }));

    let mut c = Client::start(&root);
    let uri = c.open(&shop, "php");
    // Off by default: a save runs nothing.
    save(&c, &uri);
    c.notify::<n::DidChangeConfiguration>(settings(true));
    // An untrusted workspace runs nothing either.
    c.notify_custom("phoxite/workspaceTrust", json!({ "trusted": false }));
    save(&c, &uri);
    c.notify_custom("phoxite/workspaceTrust", json!({ "trusted": true }));
    assert!(from_phpstan(&c.diagnostics(&uri, |_| true)).is_empty());

    save(&c, &uri);
    let found = from_phpstan(&c.diagnostics(&uri, |items| items.iter().any(|d| d["source"] == "phpstan")));
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0]["message"], "Method App\\Shop::run() is too clever.");
    assert_eq!(found[0]["range"]["start"]["line"], 15);

    // Turning the setting off clears what was published.
    c.notify::<n::DidChangeConfiguration>(settings(false));
    let left = c.diagnostics(&uri, |items| !items.iter().any(|d| d["source"] == "phpstan"));
    assert!(from_phpstan(&left).is_empty(), "{left:?}");
    c.shutdown();
}

/// A stand-in for the PHP interpreter running Pint: whatever script it is given, it
/// writes `formatted` over the file that follows it. Answers its path.
fn fake_pint(dir: &Path, formatted: &str) -> PathBuf {
    std::fs::write(dir.join("formatted.php"), formatted).unwrap();
    if cfg!(windows) {
        let php = dir.join("php.cmd");
        std::fs::write(&php, "@echo off\r\ncopy /y \"%~dp0formatted.php\" %2 >nul\r\n").unwrap();
        php
    } else {
        let php = dir.join("php");
        std::fs::write(&php, "#!/bin/sh\ncp \"$(dirname \"$0\")/formatted.php\" \"$2\"\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&php, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        php
    }
}

/// A stand-in for the Node.js running Prettier: whatever it is given, it prints
/// `formatted`, or fails when the template's path has `fail` in it. Answers its path.
fn fake_node(dir: &Path, formatted: &str) -> PathBuf {
    std::fs::write(dir.join("formatted.twig"), formatted).unwrap();
    if cfg!(windows) {
        let node = dir.join("node.cmd");
        let script = "@echo off\r\necho %3 | findstr fail >nul && (echo No parser could be inferred 1>&2 & exit /b 2)\r\ntype \"%~dp0formatted.twig\"\r\n";
        std::fs::write(&node, script).unwrap();
        node
    } else {
        let node = dir.join("node");
        let script = "#!/bin/sh\ncase \"$3\" in *fail*) echo 'No parser could be inferred' >&2; exit 2;; esac\ncat \"$(dirname \"$0\")/formatted.twig\"\n";
        std::fs::write(&node, script).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&node, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        node
    }
}

#[test]
fn prettier_formats_the_whole_template_of_a_trusted_project() {
    let (_tmp, root) = twig_project();
    let page = root.join("templates/page.html.twig");
    let before = std::fs::read_to_string(&page).unwrap();
    let tools = tempfile::tempdir().unwrap();
    let by_prettier = "{# formatted by the stand-in for Prettier #}\n";
    let node = fake_node(tools.path(), by_prettier);
    let settings = |tool: &str| json!({ "settings": { "phoxite": { "format": { "twig": tool }, "analyzers": { "nodePath": node } } } });
    let options = json!({ "tabSize": 4, "insertSpaces": true });

    let mut c = Client::start(&root);
    let uri = c.open(&page, "twig");
    let format = |c: &mut Client, uri: &Url| {
        c.request::<r::Formatting>(json!({ "textDocument": { "uri": uri }, "options": options }))
    };
    let from_prettier =
        |edits: &Value| edits.as_array().is_some_and(|edits| edits.iter().any(|e| e["newText"] == by_prettier));
    c.notify_custom("phoxite/workspaceTrust", json!({ "trusted": true }));

    // The project doesn't install Prettier: the built-in formatter answers.
    c.notify::<n::DidChangeConfiguration>(settings("prettier"));
    assert!(!from_prettier(&format(&mut c, &uri)));

    std::fs::create_dir_all(root.join("node_modules/prettier/bin")).unwrap();
    std::fs::write(root.join("node_modules/prettier/bin/prettier.cjs"), "// the project's Prettier\n").unwrap();
    // One edit replaces the whole text with what Prettier printed; the project's file is untouched.
    let edits = format(&mut c, &uri);
    assert_eq!(edits.as_array().map(Vec::len), Some(1), "{edits}");
    assert_eq!(edits[0]["newText"], by_prettier);
    assert_eq!(edits[0]["range"]["start"], json!({ "line": 0, "character": 0 }));
    assert_eq!(edits[0]["range"]["end"], json!({ "line": before.matches('\n').count(), "character": 0 }));
    assert_eq!(std::fs::read_to_string(&page).unwrap(), before);

    // A range is the built-in formatter's.
    let range = json!({ "start": { "line": 2, "character": 0 }, "end": { "line": 4, "character": 0 } });
    let ranged =
        c.request::<r::RangeFormatting>(json!({ "textDocument": { "uri": uri }, "range": range, "options": options }));
    assert!(!from_prettier(&ranged), "{ranged}");

    // Prettier fails (no plugin reads the file): the built-in formatter answers.
    let failing = root.join("templates/fail.html.twig");
    std::fs::write(&failing, "<p>\n{{x}}\n</p>\n").unwrap();
    let failing_uri = c.open(&failing, "twig");
    let edits = format(&mut c, &failing_uri);
    assert!(!from_prettier(&edits), "{edits}");
    assert!(edits.to_string().contains("{{ x }}"), "the built-in formatter's edit: {edits}");

    // Prettier is code of the project: nothing of it runs in a workspace that isn't trusted.
    c.notify_custom("phoxite/workspaceTrust", json!({ "trusted": false }));
    assert!(!from_prettier(&format(&mut c, &uri)));
    c.notify_custom("phoxite/workspaceTrust", json!({ "trusted": true }));
    assert!(from_prettier(&format(&mut c, &uri)));

    // And the setting's default is the built-in formatter.
    c.notify::<n::DidChangeConfiguration>(settings("phoxite"));
    assert!(!from_prettier(&format(&mut c, &uri)));
    c.shutdown();
}

#[test]
fn pint_formats_the_whole_file_of_a_trusted_project() {
    let (_tmp, root) = shop_project();
    let shop = root.join("src/Shop.php");
    let before = std::fs::read_to_string(&shop).unwrap();
    let tools = tempfile::tempdir().unwrap();
    let by_pint = "<?php\n\n// formatted by the stand-in for Pint\n\nnamespace App;\n";
    let php = fake_pint(tools.path(), by_pint);
    let settings = |tool: &str| json!({ "settings": { "phoxite": { "format": { "tool": tool }, "analyzers": { "phpPath": php } } } });
    let options = json!({ "tabSize": 4, "insertSpaces": true });

    let mut c = Client::start(&root);
    let uri = c.open(&shop, "php");
    let format =
        |c: &mut Client| c.request::<r::Formatting>(json!({ "textDocument": { "uri": uri }, "options": options }));
    let from_pint = |edits: &Value| edits.as_array().is_some_and(|edits| edits.iter().any(|e| e["newText"] == by_pint));
    c.notify_custom("phoxite/workspaceTrust", json!({ "trusted": true }));

    // The project doesn't install Pint: the built-in formatter answers.
    c.notify::<n::DidChangeConfiguration>(settings("pint"));
    assert!(!from_pint(&format(&mut c)));

    std::fs::create_dir_all(root.join("vendor/bin")).unwrap();
    std::fs::write(root.join("vendor/bin/pint"), "<?php\n").unwrap();
    // One edit replaces the whole text with what Pint wrote; the project's file is untouched.
    let edits = format(&mut c);
    assert_eq!(edits.as_array().map(Vec::len), Some(1), "{edits}");
    assert_eq!(edits[0]["newText"], by_pint);
    assert_eq!(edits[0]["range"]["start"], json!({ "line": 0, "character": 0 }));
    assert_eq!(edits[0]["range"]["end"], json!({ "line": before.matches('\n').count(), "character": 0 }));
    assert_eq!(std::fs::read_to_string(&shop).unwrap(), before);

    // A range is the built-in formatter's.
    let range = json!({ "start": { "line": 9, "character": 0 }, "end": { "line": 13, "character": 0 } });
    let ranged =
        c.request::<r::RangeFormatting>(json!({ "textDocument": { "uri": uri }, "range": range, "options": options }));
    assert!(!from_pint(&ranged), "{ranged}");

    // Pint is code of the project: nothing of it runs in a workspace that isn't trusted.
    c.notify_custom("phoxite/workspaceTrust", json!({ "trusted": false }));
    assert!(!from_pint(&format(&mut c)));
    c.notify_custom("phoxite/workspaceTrust", json!({ "trusted": true }));
    assert!(from_pint(&format(&mut c)));

    // And the setting's default is the built-in formatter.
    c.notify::<n::DidChangeConfiguration>(settings("phoxite"));
    assert!(!from_pint(&format(&mut c)));
    c.shutdown();
}

/// Format Document of a Twig template through a real Prettier, run by a real Node.js.
/// `PHOXITE_REAL_PRETTIER` names a project that installs it with a plugin that reads
/// Twig (`npm install --save-dev prettier @zackad/prettier-plugin-twig`, and the plugin
/// in its `.prettierrc`), and `node` is on `PATH`.
#[test]
#[ignore = "needs Node.js and Prettier: set PHOXITE_REAL_PRETTIER to a project that installs it"]
fn a_real_prettier_formats_the_template() {
    let project = std::env::var("PHOXITE_REAL_PRETTIER")
        .expect("PHOXITE_REAL_PRETTIER names a project with node_modules/prettier");
    let root = dunce::canonicalize(project).unwrap();
    assert!(root.join("node_modules/prettier").is_dir(), "{} doesn't install Prettier", root.display());
    std::fs::create_dir_all(root.join("templates")).unwrap();
    let messy = root.join("templates/messy.html.twig");
    let before = "{%if user%}\n<p   class=\"a\">{{user.name|upper}}</p>\n{%endif%}\n";
    std::fs::write(&messy, before).unwrap();
    let options = json!({ "tabSize": 4, "insertSpaces": true });

    let mut c = Client::start(&root);
    let uri = c.open(&messy, "twig");
    c.notify_custom("phoxite/workspaceTrust", json!({ "trusted": true }));
    c.notify::<n::DidChangeConfiguration>(json!({ "settings": { "phoxite": { "format": { "twig": "prettier" } } } }));
    let edits = c.request::<r::Formatting>(json!({ "textDocument": { "uri": uri }, "options": options }));

    // One edit over the whole text, as Prettier writes it.
    assert_eq!(edits.as_array().map(Vec::len), Some(1), "{edits}");
    assert_eq!(edits[0]["range"]["start"], json!({ "line": 0, "character": 0 }));
    let formatted = edits[0]["newText"].as_str().unwrap();
    assert!(formatted.contains("{% if user %}"), "{formatted}");
    assert!(formatted.contains("<p class=\"a\">"), "what the built-in formatter leaves alone: {formatted}");
    assert!(formatted.contains("{{ user.name|upper }}"), "{formatted}");
    // Prettier read the text from the editor: the project's file is as it was.
    assert_eq!(std::fs::read_to_string(&messy).unwrap(), before);
    c.shutdown();
}

/// Format Document through a real Laravel Pint, run by a real PHP. `PHOXITE_REAL_PINT`
/// names a project that installs it (`composer require --dev laravel/pint`), and `php`
/// is on `PATH`: CI's `pint` job sets that up.
#[test]
#[ignore = "needs PHP and Laravel Pint: set PHOXITE_REAL_PINT to a project that installs it"]
fn a_real_pint_formats_the_file() {
    let project = std::env::var("PHOXITE_REAL_PINT").expect("PHOXITE_REAL_PINT names a project with vendor/bin/pint");
    let root = dunce::canonicalize(project).unwrap();
    assert!(root.join("vendor/bin/pint").is_file(), "{} doesn't install Pint", root.display());
    std::fs::create_dir_all(root.join("src")).unwrap();
    let messy = root.join("src/Messy.php");
    let before = "<?php\nnamespace App;\nclass Messy {\n  function  total( $a,$b ){ return $a+$b; }\n}\n";
    std::fs::write(&messy, before).unwrap();
    let options = json!({ "tabSize": 4, "insertSpaces": true });

    let mut c = Client::start(&root);
    let uri = c.open(&messy, "php");
    c.notify_custom("phoxite/workspaceTrust", json!({ "trusted": true }));
    c.notify::<n::DidChangeConfiguration>(json!({ "settings": { "phoxite": { "format": { "tool": "pint" } } } }));
    let edits = c.request::<r::Formatting>(json!({ "textDocument": { "uri": uri }, "options": options }));

    // One edit over the whole text, as Pint (the Laravel preset) writes it.
    assert_eq!(edits.as_array().map(Vec::len), Some(1), "{edits}");
    assert_eq!(edits[0]["range"]["start"], json!({ "line": 0, "character": 0 }));
    let formatted = edits[0]["newText"].as_str().unwrap();
    assert!(formatted.contains("function total($a, $b)"), "{formatted}");
    assert!(formatted.contains("return $a + $b;"), "{formatted}");
    assert!(formatted.contains("class Messy\n{"), "{formatted}");
    // Pint worked on a copy: the project's file is as it was.
    assert_eq!(std::fs::read_to_string(&messy).unwrap(), before);

    // The project's `pint.json` applies: the PSR-12 preset keeps what Laravel's removes.
    std::fs::write(root.join("pint.json"), r#"{ "preset": "psr12" }"#).unwrap();
    let text = "<?php\n\nnamespace App;\n\nclass Messy\n{\n    function total($a, $b)\n    {\n        return $a.$b;\n    }\n}\n";
    c.notify::<n::DidChangeTextDocument>(json!({
        "textDocument": { "uri": uri, "version": 2 },
        "contentChanges": [{ "text": text }],
    }));
    let edits = c.request::<r::Formatting>(json!({ "textDocument": { "uri": uri }, "options": options }));
    let formatted = edits[0]["newText"].as_str().unwrap_or(text);
    // PSR-12 asks for the visibility; Laravel's preset would also space the concatenation.
    assert!(formatted.contains("public function total($a, $b)"), "{formatted}");
    assert!(formatted.contains("return $a.$b;") || formatted.contains("return $a . $b;"), "{formatted}");

    // A file Pint can't parse: it changes nothing, and the answer is no worse than without it.
    c.notify::<n::DidChangeTextDocument>(json!({
        "textDocument": { "uri": uri, "version": 3 },
        "contentChanges": [{ "text": "<?php\nclass {{{\n" }],
    }));
    let broken = c.request::<r::Formatting>(json!({ "textDocument": { "uri": uri }, "options": options }));
    assert!(broken.is_null() || broken.as_array().is_some(), "{broken}");
    c.shutdown();
    let _ = std::fs::remove_file(root.join("pint.json"));
    let _ = std::fs::remove_dir_all(root.join("src"));
}

#[test]
fn the_database_schema_joins_the_migrations() {
    let (_tmp, root) = laravel_project();
    std::fs::write(root.join(".env"), "APP_NAME=Fixture\nDB_CONNECTION=sqlite\n").unwrap();
    let file = root.join("database/database.sqlite");
    {
        let db = rusqlite::Connection::open(&file).unwrap();
        db.execute_batch(
            "CREATE TABLE posts (id INTEGER PRIMARY KEY, title VARCHAR(120) NOT NULL, rating REAL, views INTEGER NOT NULL);
             CREATE TABLE audits (id INTEGER PRIMARY KEY, note TEXT);",
        )
        .unwrap();
    }
    // The file itself: tables in order, types as SQLite names them, a key is never null.
    let schema = crate::schema::read(&file).unwrap();
    assert_eq!(schema.source, "database.sqlite");
    assert_eq!(schema.tables.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(), ["audits", "posts"]);
    let posts: Vec<_> =
        schema.tables[1].columns.iter().map(|c| (c.name.as_str(), c.data_type.as_str(), c.nullable)).collect();
    assert_eq!(
        posts,
        [
            ("id", "INTEGER", false),
            ("title", "VARCHAR(120)", false),
            ("rating", "REAL", true),
            ("views", "INTEGER", false)
        ]
    );
    assert!(crate::schema::read(&root.join("database/none.sqlite")).is_err());
    // Which file the setting names.
    assert_eq!(crate::schema::database_path(&root, ""), None);
    assert_eq!(crate::schema::database_path(&root, "auto"), Some(file.clone()));
    assert_eq!(crate::schema::database_path(&root, "storage/app.db"), Some(root.join("storage/app.db")));
    std::fs::write(root.join(".env"), "DB_CONNECTION=mysql\nDB_DATABASE=shop\n").unwrap();
    assert_eq!(crate::schema::database_path(&root, "auto"), None);
    std::fs::write(root.join(".env"), "DB_CONNECTION=sqlite\nDB_DATABASE=:memory:\n").unwrap();
    assert_eq!(crate::schema::database_path(&root, "auto"), Some(file.clone()));
    std::fs::write(root.join(".env"), "DB_CONNECTION=sqlite\n").unwrap();

    let mut c = Client::start(&root);
    let model = json!({ "class": "App\\Models\\Post" });
    let column = |view: &Value, name: &str| {
        view["columns"].as_array().and_then(|columns| columns.iter().find(|c| c["name"] == name).cloned())
    };
    // The models need the index.
    c.call_until("phoxite/laravel/generationContext", json!({}), |v| {
        v["models"].as_array().is_some_and(|m| m.iter().any(|m| m == "App\\Models\\Post"))
    });
    let before = c.call_until("phoxite/laravel/modelView", model.clone(), |v| column(v, "title").is_some());
    assert!(column(&before, "rating").is_none(), "{before}");
    assert_eq!(c.call("phoxite/laravel/refreshSchema", json!({})).unwrap(), Value::Null);

    // The setting on: the database's columns join the migrations'.
    let settings = |database: &str| json!({ "settings": { "phoxite": { "laravel": { "database": database } } } });
    c.notify::<n::DidChangeConfiguration>(settings("auto"));
    let with = c.call_until("phoxite/laravel/modelView", model.clone(), |v| column(v, "rating").is_some());
    let rating = column(&with, "rating").unwrap();
    assert_eq!(rating["source"], "database", "{rating}");
    assert_eq!(rating["database"], "database.sqlite");
    assert_eq!(rating["nullable"], true);
    assert!(column(&with, "body").is_some(), "the migrations' columns stay: {with}");

    // The database changes: the client reports the file, or the user asks.
    rusqlite::Connection::open(&file).unwrap().execute_batch("ALTER TABLE posts ADD COLUMN stars INTEGER").unwrap();
    c.notify::<n::DidChangeWatchedFiles>(
        json!({ "changes": [{ "uri": Url::from_file_path(&file).unwrap(), "type": 2 }] }),
    );
    c.call_until("phoxite/laravel/modelView", model.clone(), |v| column(v, "stars").is_some());
    let status = c.call("phoxite/laravel/refreshSchema", json!({})).unwrap();
    assert_eq!(status["tables"], 2);
    assert_eq!(status["columns"], 7);

    // A database that can't be read says so, and the migrations are all there is.
    c.notify::<n::DidChangeConfiguration>(settings("database/none.sqlite"));
    let refused = c.call("phoxite/laravel/refreshSchema", json!({})).unwrap_err();
    assert!(refused.contains("doesn't exist"), "{refused}");
    let without = c.call_until("phoxite/laravel/modelView", model, |v| column(v, "rating").is_none());
    assert!(column(&without, "title").is_some());
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

/// The index is built from files on disk, and a client reports their changes only for the
/// patterns a server registers: without the registration, what a generator writes (or a
/// `git checkout` brings) is never seen.
#[test]
fn the_server_asks_the_client_to_watch_the_files_it_indexes() {
    use crate::laravel as l;
    let (_dir, root) = laravel_project();
    let mut c = Client::start(&root);
    let model = json!({ "class": "App\\Models\\Post" });
    let column =
        |view: &Value, name: &str| view["columns"].as_array().is_some_and(|c| c.iter().any(|c| c["name"] == name));
    c.call_until(l::MODEL_VIEW, model.clone(), |v| column(v, "title"));

    let registration =
        c.requests.iter().find(|r| r.method == "client/registerCapability").expect("a registration").params.clone();
    let registration = &registration["registrations"][0];
    assert_eq!(registration["method"], "workspace/didChangeWatchedFiles");
    let globs: Vec<&str> = registration["registerOptions"]["watchers"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|w| w["globPattern"].as_str())
        .collect();
    assert_eq!(globs, crate::server::WATCHED_FILES);

    // A migration written outside the editor: the client reports it, the model has the column.
    let migration = root.join("database/migrations/2030_01_01_000000_add_votes_to_posts_table.php");
    let text = "<?php\nuse Illuminate\\Database\\Migrations\\Migration;\nuse Illuminate\\Database\\Schema\\Blueprint;\n\
                use Illuminate\\Support\\Facades\\Schema;\nreturn new class extends Migration {\n\
                public function up(): void { Schema::table('posts', function (Blueprint $table) { $table->integer('votes'); }); }\n};\n";
    std::fs::write(&migration, text).unwrap();
    c.notify::<n::DidChangeWatchedFiles>(
        json!({ "changes": [{ "uri": Url::from_file_path(&migration).unwrap(), "type": 1 }] }),
    );
    c.call_until(l::MODEL_VIEW, model.clone(), |v| column(v, "votes"));
    assert!(column(&c.call(l::MODEL_VIEW, model).unwrap(), "votes"));
    c.shutdown();

    // A client that doesn't take registrations is not asked.
    let mut c = Client::start_as(&root, Value::Null, false);
    c.call_until(l::ROUTES, json!({}), |v| v.as_array().is_some_and(|a| !a.is_empty()));
    assert!(c.requests.iter().all(|r| r.method != "client/registerCapability"));
    c.shutdown();
}

/// A client is told which folders are Laravel projects once the index knows, and again
/// when that changes: it doesn't have to keep asking.
#[test]
fn the_client_is_told_which_folders_are_laravel_projects() {
    use crate::laravel as l;
    let status = |c: &Client| -> Vec<Value> {
        c.notifications.iter().filter(|n| n.method == l::STATUS_CHANGED).map(|n| n.params.clone()).collect()
    };

    // The notification follows the index: a request can be answered before it is sent, so
    // the client keeps reading (each call reads what the server sent meanwhile) until
    // `arrived` holds.
    let wait = |c: &mut Client, arrived: &dyn Fn(&[Value]) -> bool| {
        for _ in 0..400 {
            if arrived(&status(c)) {
                return;
            }
            let _ = c.call(l::STATUS, json!({}));
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
        panic!("the status never arrived: {:?}", status(c));
    };

    let (_dir, root) = laravel_project();
    let mut c = Client::start(&root);
    c.call_until(l::ROUTES, json!({}), |v| v.as_array().is_some_and(|a| !a.is_empty()));
    wait(&mut c, &|told| !told.is_empty());
    let told = status(&c);
    assert_eq!(told.len(), 1, "once, not with every index pass: {told:?}");
    assert_eq!(told[0]["laravel"], true);
    assert_eq!(told[0]["folders"], json!([Url::from_file_path(&root).unwrap()]));
    assert_eq!(told[0]["frameworks"], json!(["Laravel"]));

    // The folder leaves the workspace: no Laravel project is left.
    c.notify::<n::DidChangeWorkspaceFolders>(json!({ "event": {
        "added": [],
        "removed": [{ "uri": Url::from_file_path(&root).unwrap(), "name": "fixture" }],
    } }));
    c.call_until(l::STATUS, json!({}), |v| v["laravel"] == false);
    let gone = json!({ "laravel": false, "folders": [], "frameworks": [] });
    wait(&mut c, &|told| told.last() == Some(&gone));
    c.shutdown();

    // A project that is not a Laravel one is told so too.
    let mut c = Client::start(&fixture("php-library"));
    c.call_until("workspace/symbol", json!({ "query": "Library" }), |v| v.as_array().is_some_and(|a| !a.is_empty()));
    wait(&mut c, &|told| !told.is_empty());
    assert_eq!(status(&c), [json!({ "laravel": false, "folders": [], "frameworks": [] })]);
    c.shutdown();
}

/// The PHP version and the extensions come from `composer.json`: a change to it is
/// followed without a restart.
#[test]
fn the_php_version_and_the_extensions_follow_composer_json() {
    let dir = tempfile::tempdir().unwrap();
    let root = dunce::canonicalize(dir.path()).unwrap();
    let manifest = root.join("composer.json");
    std::fs::write(&manifest, r#"{ "require": { "php": "^8.1" } }"#).unwrap();
    let file = root.join("index.php");
    let text =
        "<?php\nclass C {\n    public string $p { get => 'x'; }\n}\n\nfunction f(): void {\n    new \\Redis();\n}\n";
    std::fs::write(&file, text).unwrap();
    let mut c = Client::start(&root);
    let uri = c.open(&file, "php");
    let codes = |found: &[Value]| -> Vec<String> {
        let mut codes: Vec<String> = found.iter().filter_map(|d| d["code"].as_str().map(str::to_owned)).collect();
        codes.sort();
        codes
    };
    let found = c.diagnostics(&uri, |d| d.len() >= 2);
    assert_eq!(codes(&found), ["PhpLanguageLevel", "PhpUndefinedClass"], "{found:?}");

    std::fs::write(&manifest, r#"{ "require": { "php": "^8.4", "ext-redis": "*" } }"#).unwrap();
    c.notify::<n::DidChangeWatchedFiles>(json!({
        "changes": [{ "uri": Url::from_file_path(&manifest).unwrap(), "type": 2 }]
    }));
    let found = c.diagnostics(&uri, |d| d.is_empty());
    assert!(found.is_empty());
    c.shutdown();
}

/// `phoxite.json` at the root of a project is laid over the client's settings, and is
/// followed when it changes.
#[test]
fn a_project_settings_file_is_laid_over_the_clients_settings() {
    let dir = tempfile::tempdir().unwrap();
    let root = dunce::canonicalize(dir.path()).unwrap();
    let settings_file = root.join("phoxite.json");
    let severity =
        |name: &str| format!(r#"{{ "inspections": {{ "PhpUndefinedClass": {{ "severity": "{name}" }} }} }}"#);
    std::fs::write(&settings_file, severity("hint")).unwrap();
    let file = root.join("index.php");
    std::fs::write(&file, "<?php\nfunction f(): void {\n    new UnknownThing();\n}\n").unwrap();
    let mut c = Client::start(&root);
    let uri = c.open(&file, "php");
    let changed = |c: &Client, kind: u32| {
        c.notify::<n::DidChangeWatchedFiles>(json!({
            "changes": [{ "uri": Url::from_file_path(&settings_file).unwrap(), "type": kind }]
        }));
    };
    // The LSP severities: 1 error, 3 information, 4 hint.
    let has = |wanted: u64| move |found: &[Value]| found.len() == 1 && found[0]["severity"] == wanted;

    // The file's severity, not the default (an error).
    c.diagnostics(&uri, has(4));

    // The file changes: the problems are published again with the new severity.
    std::fs::write(&settings_file, severity("information")).unwrap();
    changed(&c, 2);
    c.diagnostics(&uri, has(3));

    // The client sets the same setting: the file wins. `stubs`, which the file doesn't
    // set, is the client's.
    let client = json!({ "inspections": { "PhpUndefinedClass": { "severity": "error" } }, "stubs": ["redis"] });
    c.notify::<n::DidChangeConfiguration>(json!({ "settings": { "phoxite": client } }));
    c.request::<r::DocumentSymbolRequest>(json!({ "textDocument": { "uri": uri } }));
    c.diagnostics(&uri, has(3));

    // A file that can't be read is named, and what it held stays.
    std::fs::write(&settings_file, "{ \"inspections\": ").unwrap();
    changed(&c, 2);
    c.request::<r::DocumentSymbolRequest>(json!({ "textDocument": { "uri": uri } }));
    let messages: Vec<&str> = c
        .notifications
        .iter()
        .filter(|n| n.method == n::ShowMessage::METHOD)
        .filter_map(|n| n.params["message"].as_str())
        .collect();
    assert_eq!(messages.len(), 1, "{messages:?}");
    assert!(messages[0].starts_with("Phoxite can't read ") && messages[0].contains("phoxite.json"), "{messages:?}");
    c.diagnostics(&uri, has(3));

    // The file goes: the client's settings are all there is.
    std::fs::remove_file(&settings_file).unwrap();
    changed(&c, 3);
    c.diagnostics(&uri, has(1));
    c.shutdown();
}
