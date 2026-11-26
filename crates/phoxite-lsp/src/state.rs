//! What the server holds: one engine per workspace folder, the open
//! documents, the settings, and the problems published per file.
//!
//! The settings are the client's, and for each workspace folder what its
//! `phoxite.json` sets over them ([`Settings::layered`]).

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::{Duration, Instant};

use crossbeam_channel::Sender;
use lsp_server::{Message, Notification, Request, RequestId};
use lsp_types::{self as lsp, Url};
use parking_lot::{Condvar, Mutex, RwLock};
use phoxite::base::settings::PhpConfig;
use phoxite::base::{self as e, DocId, Document};
use phoxite::engine::{CACHE_SUBDIR, EngineConfig, PhpEngine};
use phoxite::inspect::InspectionConfig;
use phoxite::project::LayoutConfig;
use phoxite::stubs::StubFilter;

use serde_json::Value;

use crate::config::{PROJECT_FILE, Settings, read_project_file};
use crate::convert::{self, EditSupport};

/// What a document is, which decides who answers for it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Php,
    Blade,
    /// `*.twig`: answered from the template's text and the project's
    /// template folders, without the engine.
    Twig,
    /// `.env`, `.env.example`…: environment keys.
    Env,
    /// `lang/*.json`: translation keys.
    TranslationJson,
    /// A script or a `block.json`: in a WordPress project, the blocks,
    /// REST routes and AJAX actions its strings name.
    WordPressScript,
    /// A YAML file of a Drupal module or theme (`*.services.yml`,
    /// `*.routing.yml`…), or Drupal 7's `<name>.info`: the names it
    /// declares and writes.
    DrupalYaml,
    /// A file of a Magento module the engine reads (`di.xml`, `events.xml`,
    /// a layout file, a GraphQL schema…): the names it declares and writes.
    MagentoXml,
    /// An XML file, a language file or an asset list of a Joomla site or
    /// extension: the names it declares and writes.
    JoomlaFile,
    Other,
}

impl Kind {
    /// The server itself looks for the problems of such a document (the engine
    /// publishes a PHP file's).
    fn has_template_problems(self) -> bool {
        matches!(self, Kind::Blade | Kind::Twig | Kind::MagentoXml)
    }

    pub fn of(language: &str, path: &Path) -> Self {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_ascii_lowercase();
        if language == "blade" || name.ends_with(".blade.php") {
            Kind::Blade
        } else if language == "twig" || name.ends_with(".twig") {
            Kind::Twig
        } else if language == "php" || phoxite::project::is_php_path(path) {
            Kind::Php
        } else if name == ".env" || name.starts_with(".env.") {
            Kind::Env
        } else if name.ends_with(".json")
            && path.parent().is_some_and(|dir| dir.components().any(|c| c.as_os_str() == "lang"))
        {
            Kind::TranslationJson
        } else if (name.ends_with(".yml") || name.ends_with(".info"))
            && phoxite::frameworks::drupal::FileKind::of(path).is_some()
        {
            Kind::DrupalYaml
        } else if phoxite::frameworks::magento::project::XmlKind::of(path).is_some() {
            Kind::MagentoXml
        } else if name == "block.json"
            || matches!(language, "javascript" | "javascriptreact" | "typescript" | "typescriptreact")
            || [".js", ".jsx", ".mjs", ".ts", ".tsx"].iter().any(|ext| name.ends_with(ext))
        {
            Kind::WordPressScript
        } else if name.ends_with(".xml") || name.ends_with(".ini") || name == "joomla.asset.json" {
            // (Whether the project is a Joomla one is the engine's to say.)
            Kind::JoomlaFile
        } else {
            Kind::Other
        }
    }
}

/// An open document.
pub struct OpenDoc {
    pub id: DocId,
    pub uri: Url,
    pub path: PathBuf,
    pub kind: Kind,
    pub version: i32,
    pub text: Document,
}

/// A snapshot of an open document for a request.
#[derive(Clone)]
pub struct DocSnapshot {
    pub id: DocId,
    pub path: PathBuf,
    pub kind: Kind,
    pub version: i32,
    pub text: Arc<str>,
    pub engine: Option<Arc<PhpEngine>>,
    pub root: PathBuf,
}

impl DocSnapshot {
    pub fn blade(&self) -> phoxite::blade::BladeDocument<'_> {
        phoxite::blade::BladeDocument {
            doc: self.id,
            path: &self.path,
            version: self.version.max(0) as u32,
            text: &self.text,
        }
    }
}

/// What the engine of the project at `root` is opened with: its index is
/// kept in a folder of `cache_dir` named after the project's path.
pub fn engine_config(cache_dir: &Path, root: &Path, php: &PhpConfig) -> EngineConfig {
    let key = blake3::hash(root.to_string_lossy().as_bytes()).to_hex();
    EngineConfig {
        cache_dir: cache_dir.join(&key[..16]).join(CACHE_SUBDIR),
        layout: LayoutConfig::from_config(root, php),
        stubs: StubFilter::from_config(root, php),
    }
}

/// The file in a project's cache folder whose time says when the project was last opened.
const LAST_USED: &str = "last-used";

/// How long a project's cache is kept after the project was last opened.
pub const CACHE_KEPT: Duration = Duration::from_secs(90 * 24 * 60 * 60);

/// Notes that the project whose engine keeps its index in `engine_cache` (an
/// [`EngineConfig::cache_dir`]) is open now.
fn mark_cache_used(engine_cache: &Path) {
    let Some(folder) = engine_cache.parent() else { return };
    if std::fs::create_dir_all(folder).is_ok() {
        let _ = std::fs::write(folder.join(LAST_USED), b"");
    }
}

/// Removes the cache folders of projects not opened for [`CACHE_KEPT`]: a project that
/// was moved or deleted would leave its index behind for good. Only folders this server
/// named and marked are looked at; one without a mark (an older version made it) gets
/// one now, so its time starts here.
pub fn prune_caches(cache_dir: &Path, kept: Duration) {
    let Ok(entries) = std::fs::read_dir(cache_dir) else { return };
    for folder in entries.flatten().map(|entry| entry.path()) {
        let name = folder.file_name().and_then(|name| name.to_str()).unwrap_or_default();
        let ours =
            name.len() == 16 && name.bytes().all(|b| b.is_ascii_hexdigit()) && folder.join(CACHE_SUBDIR).is_dir();
        if !ours {
            continue;
        }
        let mark = folder.join(LAST_USED);
        let Ok(used) = std::fs::metadata(&mark).and_then(|mark| mark.modified()) else {
            let _ = std::fs::write(&mark, b"");
            continue;
        };
        if used.elapsed().is_ok_and(|unused| unused > kept) {
            match std::fs::remove_dir_all(&folder) {
                Ok(()) => tracing::info!("removed the unused cache {}", folder.display()),
                Err(e) => tracing::warn!("can't remove the unused cache {}: {e}", folder.display()),
            }
        }
    }
}

pub struct Workspace {
    pub root: PathBuf,
    pub engine: Arc<PhpEngine>,
    /// The client's settings with the project's settings file over them.
    pub settings: Arc<Settings>,
    /// The project's settings file as it was last read without an error.
    file: Option<Value>,
    /// The settings left at their defaults: their value doesn't fit.
    rejected: Vec<String>,
}

/// What the client supports.
#[derive(Clone, Copy, Debug, Default)]
pub struct ClientSupport {
    pub edits: EditSupport,
    pub insert_replace: bool,
    pub snippets: bool,
    pub work_done_progress: bool,
    /// The client watches files for a server that registers for it
    /// (`workspace.didChangeWatchedFiles.dynamicRegistration`).
    pub watched_files: bool,
    /// The client syncs more documents when the server registers for them
    /// (`textDocument.synchronization.dynamicRegistration`).
    pub document_registration: bool,
    pub markdown: bool,
    /// The client asks for the inlay hints again when told to
    /// (`workspace.inlayHint.refreshSupport`).
    pub inlay_hint_refresh: bool,
    /// The client has the dialogs of the refactorings that ask something first
    /// (`experimental.phoxite.dialogs`).
    pub dialogs: bool,
}

/// How long a template is left alone after its last change before its problems are
/// looked for: what a PHP file waits too (`phoxite::inspect`).
const TEMPLATE_DEBOUNCE: Duration = Duration::from_millis(120);

/// The templates waiting for their problems to be looked for, each with the time it is
/// due: a change while one waits moves that time on.
#[derive(Default)]
struct TemplateQueue {
    waiting: Mutex<(HashMap<Url, Instant>, bool)>,
    wake: Condvar,
}

impl TemplateQueue {
    fn push(&self, uri: Url) {
        self.waiting.lock().0.insert(uri, Instant::now() + TEMPLATE_DEBOUNCE);
        self.wake.notify_all();
    }

    fn stop(&self) {
        self.waiting.lock().1 = true;
        self.wake.notify_all();
    }

    /// Waits for the template due first; none once the server shuts down.
    fn next(&self) -> Option<Url> {
        let mut waiting = self.waiting.lock();
        loop {
            if waiting.1 {
                return None;
            }
            let first = waiting.0.iter().min_by_key(|(_, due)| **due).map(|(uri, due)| (uri.clone(), *due));
            match first {
                Some((uri, due)) if due <= Instant::now() => {
                    waiting.0.remove(&uri);
                    return Some(uri);
                }
                Some((_, due)) => {
                    self.wake.wait_until(&mut waiting, due);
                }
                None => self.wake.wait(&mut waiting),
            }
        }
    }
}

pub struct State {
    pub sender: Sender<Message>,
    pub support: ClientSupport,
    pub cache_dir: PathBuf,
    /// The client's settings: those of a file outside every workspace
    /// folder, and the ones a project's settings file can't set.
    pub settings: RwLock<Settings>,
    /// The same as the client sent them, and those of them left at their
    /// defaults.
    client_settings: RwLock<(Value, Vec<String>)>,
    pub workspaces: RwLock<Vec<Workspace>>,
    pub docs: RwLock<HashMap<Url, OpenDoc>>,
    /// The engine's events go here, and the main loop takes them.
    pub events: Sender<e::IdeEvent>,
    /// Problems by file and source (`php`, `blade`), published together.
    pub problems: Mutex<HashMap<PathBuf, BTreeMap<String, Vec<e::Diagnostic>>>>,
    /// The PHPStan and Psalm runs (`analyzers.rs`).
    pub analyzers: Mutex<crate::analyzers::Analyzers>,
    /// What the Twig support read about each project folder, until a
    /// template or a file it reads changes there ([`State::forget_twig`]).
    twig: Mutex<HashMap<PathBuf, Arc<phoxite::twig::TwigProject>>>,
    /// The user trusts the workspace: programs of the project may run.
    trusted: AtomicBool,
    /// The settings last left at their defaults for a value that didn't fit,
    /// as the user was told.
    rejected_settings: Mutex<Vec<String>>,
    next_doc: AtomicU32,
    /// The templates whose problems are to be looked for ([`State::watch_templates`]).
    templates: Arc<TemplateQueue>,
}

impl State {
    pub fn new(
        sender: Sender<Message>,
        support: ClientSupport,
        cache_dir: PathBuf,
        events: Sender<e::IdeEvent>,
    ) -> Self {
        Self {
            sender,
            support,
            cache_dir,
            settings: RwLock::default(),
            client_settings: RwLock::new((Value::Null, Vec::new())),
            workspaces: RwLock::new(Vec::new()),
            docs: RwLock::new(HashMap::new()),
            events,
            problems: Mutex::new(HashMap::new()),
            analyzers: Mutex::default(),
            twig: Mutex::default(),
            trusted: AtomicBool::new(true),
            rejected_settings: Mutex::default(),
            next_doc: AtomicU32::new(1),
            templates: Arc::default(),
        }
    }

    pub fn notify<N: lsp::notification::Notification>(&self, params: N::Params) {
        let _ = self.sender.send(Message::Notification(Notification::new(N::METHOD.to_owned(), params)));
    }

    /// Starts an engine for `root` unless one runs for it.
    pub fn add_workspace(&self, root: &Path) {
        let root = dunce::simplified(root).to_path_buf();
        if self.workspaces.read().iter().any(|w| w.root == root) {
            return;
        }
        let file = self.project_file(&root, None);
        let (settings, rejected) = Settings::layered(&self.client_settings.read().0, file.as_ref(), self.is_trusted());
        let php = settings.engine_config();
        let config = engine_config(&self.cache_dir, &root, &php);
        mark_cache_used(&config.cache_dir);
        let events = self.events.clone();
        let sink: e::SharedSink = Arc::new(move |event: e::IdeEvent| {
            let _ = events.send(event);
        });
        match PhpEngine::open(config, sink) {
            Ok(engine) => {
                engine.configure_inspections(InspectionConfig::from_profile(&php.inspections));
                engine.set_framework_settings(php.frameworks);
                engine.set_laravel_settings(php.laravel.clone());
                tracing::info!("indexing {}", root.display());
                let workspace = Workspace {
                    root: root.clone(),
                    engine: engine.clone(),
                    settings: Arc::new(settings),
                    file,
                    rejected,
                };
                self.workspaces.write().push(workspace);
                // These read the workspace's settings.
                crate::laravel::load_templates(self, &root, &engine);
                crate::schema::refresh_quietly(self, &root, &engine);
                self.settings_rejected();
            }
            Err(e) => tracing::error!("can't start the engine for {}: {e}", root.display()),
        }
    }

    /// Reads the PHP version and the `ext-*` requirements of the workspace's
    /// `composer.json` again. Nothing happens when they are the same.
    pub fn refresh_stub_filter(&self, workspace: &Workspace) {
        let php = workspace.settings.engine_config();
        workspace.engine.set_stub_filter(StubFilter::from_config(&workspace.root, &php));
    }

    /// The settings file of the project at `root`. One that can't be read is
    /// told to the user, and `before` (what it held until now) stays.
    fn project_file(&self, root: &Path, before: Option<Value>) -> Option<Value> {
        read_project_file(root).unwrap_or_else(|error| {
            self.notify::<lsp::notification::ShowMessage>(lsp::ShowMessageParams {
                typ: lsp::MessageType::WARNING,
                message: format!("Phoxite can't read {}: {error}", root.join(PROJECT_FILE).display()),
            });
            before
        })
    }

    /// The settings file of the project at `root` changed (or appeared, or
    /// went): its settings are worked out again and passed to its engine.
    pub fn project_file_changed(&self, root: &Path) {
        if self.configure(Some(root), true) {
            self.refresh_inlay_hints();
        }
        self.settings_rejected();
    }

    /// Asks the client for the inlay hints again (their settings changed),
    /// when it takes the request.
    fn refresh_inlay_hints(&self) {
        if !self.support.inlay_hint_refresh {
            return;
        }
        use lsp::request::{InlayHintRefreshRequest, Request as _};
        let request = Request::new(
            RequestId::from("phoxite/inlayHint/refresh".to_owned()),
            InlayHintRefreshRequest::METHOD.to_owned(),
            (),
        );
        let _ = self.sender.send(Message::Request(request));
    }

    /// Works out the settings of the workspace at `only` (of every one
    /// without it) and passes them to its engine; `reread` reads its settings
    /// file again first. True when the inlay hints of one of them show
    /// differently now.
    fn configure(&self, only: Option<&Path>, reread: bool) -> bool {
        let trusted = self.is_trusted();
        let mut changed = Vec::new();
        let mut hints = false;
        {
            let client = self.client_settings.read();
            let mut workspaces = self.workspaces.write();
            for w in workspaces.iter_mut().filter(|w| only.is_none_or(|root| w.root == root)) {
                if reread {
                    w.file = self.project_file(&w.root, w.file.take());
                }
                let (settings, rejected) = Settings::layered(&client.0, w.file.as_ref(), trusted);
                w.rejected = rejected;
                if *w.settings != settings {
                    hints |= w.settings.inlay_hints != settings.inlay_hints;
                    w.settings = Arc::new(settings);
                    changed.push((w.root.clone(), w.engine.clone(), w.settings.clone()));
                }
            }
        }
        for (root, engine, settings) in changed {
            let php = settings.engine_config();
            engine.set_layout_config(LayoutConfig::from_config(&root, &php));
            engine.set_stub_filter(StubFilter::from_config(&root, &php));
            engine.configure_inspections(InspectionConfig::from_profile(&php.inspections));
            engine.set_framework_settings(php.frameworks);
            engine.set_laravel_settings(php.laravel.clone());
            crate::laravel::load_templates(self, &root, &engine);
            crate::schema::refresh_quietly(self, &root, &engine);
            // The severities the user chose are applied when problems are published.
            self.republish_below(&root);
        }
        hints
    }

    /// Publishes the problems of the files below `root` again.
    fn republish_below(&self, root: &Path) {
        let paths: Vec<PathBuf> = self.problems.lock().keys().filter(|path| path.starts_with(root)).cloned().collect();
        for path in paths {
            self.publish(&path);
        }
    }

    /// The settings for `path`: those of its workspace folder, else the
    /// client's.
    pub fn settings_for(&self, path: &Path) -> Arc<Settings> {
        let workspaces = self.workspaces.read();
        workspaces
            .iter()
            .filter(|w| path.starts_with(&w.root))
            .max_by_key(|w| w.root.components().count())
            .or_else(|| workspaces.first())
            .map_or_else(|| Arc::new(self.settings.read().clone()), |w| w.settings.clone())
    }

    pub fn remove_workspace(&self, root: &Path) {
        let root = dunce::simplified(root).to_path_buf();
        let mut workspaces = self.workspaces.write();
        if let Some(i) = workspaces.iter().position(|w| w.root == root) {
            workspaces.remove(i).engine.shutdown();
        }
    }

    /// The client's settings, as it sent them: every workspace's settings
    /// are worked out again and passed to its engine. `rejected` names what
    /// didn't fit in what the settings came with (the initialization options).
    pub fn apply_settings(&self, client: &Value, mut rejected: Vec<String>) {
        let (settings, more) = Settings::from_json(client);
        rejected.extend(more);
        let analyzers = settings.analyzers.on_save;
        let database = settings.laravel.database != self.settings.read().laravel.database;
        // A file outside every workspace folder is shown as the client's settings say.
        let hints = settings.inlay_hints != self.settings.read().inlay_hints;
        *self.settings.write() = settings;
        *self.client_settings.write() = (client.clone(), rejected);
        if !analyzers {
            crate::analyzers::clear_all(self);
        }
        if self.configure(None, false) || hints {
            self.refresh_inlay_hints();
        }
        if database {
            // Read from the client's settings alone, so no workspace's changed with it.
            for w in self.workspaces.read().iter() {
                crate::schema::refresh_quietly(self, &w.root, &w.engine);
            }
        }
        self.settings_rejected();
    }

    /// Tells the user which settings were left at their defaults because
    /// their value doesn't fit, unless they are the ones named last time.
    pub fn settings_rejected(&self) {
        let mut rejected = self.client_settings.read().1.clone();
        for w in self.workspaces.read().iter() {
            for path in &w.rejected {
                if !rejected.contains(path) {
                    rejected.push(path.clone());
                }
            }
        }
        let mut last = self.rejected_settings.lock();
        if *last == rejected {
            return;
        }
        if !rejected.is_empty() {
            let names: Vec<String> = rejected
                .iter()
                .map(|path| if path.is_empty() { "the settings".into() } else { format!("`{path}`") })
                .collect();
            self.notify::<lsp::notification::ShowMessage>(lsp::ShowMessageParams {
                typ: lsp::MessageType::WARNING,
                message: format!(
                    "Phoxite left {} at the default: the value isn't one it understands.",
                    names.join(", ")
                ),
            });
        }
        *last = rejected;
    }

    pub fn is_trusted(&self) -> bool {
        self.trusted.load(Ordering::Acquire)
    }

    /// The client said whether the workspace is trusted.
    pub fn set_trusted(&self, trusted: bool) {
        self.trusted.store(trusted, Ordering::Release);
        if !trusted {
            crate::analyzers::clear_all(self);
        }
        // What a settings file may set depends on it.
        if self.configure(None, false) {
            self.refresh_inlay_hints();
        }
    }

    /// The workspace `path` belongs to (the innermost folder holding it).
    pub fn workspace_of(&self, path: &Path) -> Option<(PathBuf, Arc<PhpEngine>)> {
        let workspaces = self.workspaces.read();
        workspaces
            .iter()
            .filter(|w| path.starts_with(&w.root))
            .max_by_key(|w| w.root.components().count())
            .or_else(|| workspaces.first())
            .map(|w| (w.root.clone(), w.engine.clone()))
    }

    pub fn engines(&self) -> Vec<Arc<PhpEngine>> {
        self.workspaces.read().iter().map(|w| w.engine.clone()).collect()
    }

    pub fn snapshot(&self, uri: &Url) -> Option<DocSnapshot> {
        let docs = self.docs.read();
        let doc = docs.get(uri)?;
        let (root, engine) = match self.workspace_of(&doc.path) {
            Some((root, engine)) => (root, Some(engine)),
            None => (doc.path.parent().map(Path::to_path_buf).unwrap_or_default(), None),
        };
        Some(DocSnapshot {
            id: doc.id,
            path: doc.path.clone(),
            kind: doc.kind,
            version: doc.version,
            text: Arc::from(doc.text.as_str()),
            engine,
            root,
        })
    }

    pub fn open(&self, uri: Url, language: &str, version: i32, text: &str) {
        let Some(path) = convert::path_of(&uri) else { return };
        if self.workspaces.read().is_empty() {
            // A file opened without a folder: its Composer project, else its folder.
            let root = path
                .ancestors()
                .skip(1)
                .find(|dir| dir.join("composer.json").is_file())
                .or_else(|| path.parent())
                .map(Path::to_path_buf);
            if let Some(root) = root {
                self.add_workspace(&root);
            }
        }
        let kind = Kind::of(language, &path);
        let id = DocId(self.next_doc.fetch_add(1, Ordering::Relaxed));
        let doc = OpenDoc { id, uri: uri.clone(), path: path.clone(), kind, version, text: Document::new(text) };
        if kind == Kind::Php
            && let Some((_, engine)) = self.workspace_of(&path)
        {
            engine.document_opened(id, &path, version.max(0) as u32, doc.text.as_str());
        }
        // The usages of a route or a template read a Twig template as the editor has it.
        if kind == Kind::Twig
            && let Some((_, engine)) = self.workspace_of(&path)
        {
            engine.set_template_text(&path, Some(doc.text.as_str()));
        }
        self.docs.write().insert(uri.clone(), doc);
        self.template_changed(&uri);
    }

    pub fn change(&self, uri: &Url, version: i32, changes: Vec<lsp::TextDocumentContentChangeEvent>) {
        let (id, path, kind, text) = {
            let mut docs = self.docs.write();
            let Some(doc) = docs.get_mut(uri) else { return };
            for change in changes {
                match change.range {
                    Some(range) => doc.text.replace(convert::range(range), &change.text),
                    None => doc.text.set_text(&change.text),
                }
            }
            doc.version = version;
            (doc.id, doc.path.clone(), doc.kind, doc.text.text())
        };
        if kind == Kind::Php
            && let Some((_, engine)) = self.workspace_of(&path)
        {
            engine.document_changed(id, version.max(0) as u32, &text);
        }
        if kind == Kind::Twig
            && let Some((_, engine)) = self.workspace_of(&path)
        {
            engine.set_template_text(&path, Some(&text));
        }
        self.template_changed(uri);
    }

    pub fn close(&self, uri: &Url) {
        let Some(doc) = self.docs.write().remove(uri) else { return };
        if let Some((_, engine)) = self.workspace_of(&doc.path) {
            // Also drops a template's embedded PHP.
            engine.document_closed(doc.id);
            if doc.kind == Kind::Twig {
                engine.set_template_text(&doc.path, None);
            }
        }
        self.analyzers.lock().closed(&doc.path);
        self.problems.lock().remove(&doc.path);
        self.notify::<lsp::notification::PublishDiagnostics>(lsp::PublishDiagnosticsParams {
            uri: doc.uri,
            diagnostics: Vec::new(),
            version: None,
        });
    }

    /// The template folders of the project at `root`, and whether it has
    /// Symfony's Twig extensions. Read once, then kept.
    pub fn twig_project(&self, root: &Path) -> Arc<phoxite::twig::TwigProject> {
        if let Some(known) = self.twig.lock().get(root) {
            return known.clone();
        }
        let project = Arc::new(phoxite::twig::TwigProject::discover(root));
        self.twig.lock().insert(root.to_path_buf(), project.clone());
        project
    }

    /// Reads the Twig project at `root` again on next use: a template was
    /// created or removed, or `twig.yaml` or a Composer file changed.
    pub fn forget_twig(&self, root: &Path) {
        self.twig.lock().remove(root);
    }

    /// Starts the thread that looks for the problems of templates. They are
    /// found off the main loop, which a slow template would hold up for every
    /// request and cancellation behind it, and once the typing pauses.
    pub fn watch_templates(self: &Arc<Self>) {
        let queue = self.templates.clone();
        let state = Arc::downgrade(self);
        let run = move || {
            while let Some(uri) = queue.next() {
                let Some(state) = state.upgrade() else { return };
                let looked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| state.template_problems(&uri)));
                if looked.is_err() {
                    tracing::error!("looking for the problems of {uri} panicked");
                }
            }
        };
        if let Err(e) = std::thread::Builder::new().name("template problems".into()).spawn(run) {
            tracing::error!("can't start the template problems thread: {e}");
        }
    }

    /// The template at `uri` was opened or changed, or what it names was: its
    /// problems are looked for again once it has been left alone for
    /// [`TEMPLATE_DEBOUNCE`].
    fn template_changed(&self, uri: &Url) {
        let known = self.docs.read().get(uri).is_some_and(|doc| doc.kind.has_template_problems());
        if known {
            self.templates.push(uri.clone());
        }
    }

    /// A template's own problems: a Blade template's unclosed sections and
    /// pushes, a Twig template's syntax errors.
    fn template_problems(&self, uri: &Url) {
        self.template_problems_of(uri);
        // Closed meanwhile: `close` cleared its problems before these were published.
        if !self.docs.read().contains_key(uri)
            && let Some(path) = convert::path_of(uri)
            && self.problems.lock().remove(dunce::simplified(&path)).is_some()
        {
            self.publish(dunce::simplified(&path));
        }
    }

    fn template_problems_of(&self, uri: &Url) {
        let Some(doc) = self.snapshot(uri) else { return };
        if doc.kind == Kind::Twig {
            let reported = self.settings_for(&doc.path).inspection_enabled(phoxite::twig::SYNTAX_CODE);
            let items = if reported { phoxite::twig::problems(&doc.text) } else { Vec::new() };
            self.problems_changed(&doc.path, phoxite::twig::DIAGNOSTIC_SOURCE.to_owned(), items);
            return;
        }
        if doc.kind == Kind::MagentoXml {
            // The classes and templates the file names that aren't there.
            let settings = self.settings_for(&doc.path);
            let found = doc.engine.as_ref().and_then(|engine| engine.magento_document_problems(&doc.path, &doc.text));
            let mut items = found.unwrap_or_default();
            items.retain(|item| item.code.as_deref().is_none_or(|code| settings.inspection_enabled(code)));
            let source = phoxite::frameworks::magento::documents::DIAGNOSTIC_SOURCE.to_owned();
            self.problems_changed(&doc.path, source, items);
            return;
        }
        if doc.kind != Kind::Blade {
            return;
        }
        let items = match &doc.engine {
            Some(engine) => engine.blade_file_problems(&doc.path, &doc.text),
            None => phoxite::blade::structure::inspect(&doc.text, &[]),
        };
        let source = phoxite::blade::structure::DIAGNOSTIC_SOURCE.to_owned();
        self.problems_changed(&doc.path, source, items);
    }

    /// The open files of a Magento project are looked at again: what they
    /// name is known once the index is there, and changes with the other
    /// files.
    pub fn magento_documents_changed(&self) {
        let open: Vec<Url> =
            self.docs.read().values().filter(|doc| doc.kind == Kind::MagentoXml).map(|doc| doc.uri.clone()).collect();
        for uri in open {
            self.template_changed(&uri);
        }
    }

    /// Records the problems of `path` from `source` and publishes the file's
    /// problems from every source.
    pub fn problems_changed(&self, path: &Path, source: String, items: Vec<e::Diagnostic>) {
        let path = dunce::simplified(path).to_path_buf();
        self.problems.lock().entry(path.clone()).or_default().insert(source, items);
        self.publish(&path);
    }

    /// Publishes the problems of `path` from every source.
    fn publish(&self, path: &Path) {
        let all: Vec<e::Diagnostic> = self
            .problems
            .lock()
            .get(path)
            .map(|by_source| by_source.values().flatten().cloned().collect())
            .unwrap_or_default();
        let Some(uri) = convert::uri_of(path) else { return };
        let version = self.docs.read().get(&uri).map(|doc| doc.version);
        let settings = self.settings_for(path);
        let diagnostics = all
            .into_iter()
            .map(|mut d| {
                settings.apply_severity(&mut d);
                convert::diagnostic(d)
            })
            .collect();
        self.notify::<lsp::notification::PublishDiagnostics>(lsp::PublishDiagnosticsParams {
            uri,
            diagnostics,
            version,
        });
    }

    pub fn shutdown(&self) {
        self.templates.stop();
        for w in self.workspaces.write().drain(..) {
            w.engine.shutdown();
        }
    }
}
