//! What the server holds: one engine per workspace folder, the open
//! documents, the settings, and the problems published per file.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use crossbeam_channel::Sender;
use lsp_server::{Message, Notification};
use lsp_types::{self as lsp, Url};
use parking_lot::{Mutex, RwLock};
use phoxite::base::{self as e, DocId, Document};
use phoxite::engine::{CACHE_SUBDIR, EngineConfig, PhpEngine};
use phoxite::inspect::InspectionConfig;
use phoxite::project::LayoutConfig;
use phoxite::stubs::StubFilter;

use crate::config::Settings;
use crate::convert::{self, EditSupport};

/// What a document is, which decides who answers for it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Php,
    Blade,
    /// `.env`, `.env.example`…: environment keys.
    Env,
    /// `lang/*.json`: translation keys.
    TranslationJson,
    Other,
}

impl Kind {
    pub fn of(language: &str, path: &Path) -> Self {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_ascii_lowercase();
        if language == "blade" || name.ends_with(".blade.php") {
            Kind::Blade
        } else if language == "php" || phoxite::project::is_php_path(path) {
            Kind::Php
        } else if name == ".env" || name.starts_with(".env.") {
            Kind::Env
        } else if name.ends_with(".json")
            && path.parent().is_some_and(|dir| dir.components().any(|c| c.as_os_str() == "lang"))
        {
            Kind::TranslationJson
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

pub struct Workspace {
    pub root: PathBuf,
    pub engine: Arc<PhpEngine>,
}

/// What the client supports.
#[derive(Clone, Copy, Debug, Default)]
pub struct ClientSupport {
    pub edits: EditSupport,
    pub insert_replace: bool,
    pub snippets: bool,
    pub work_done_progress: bool,
    pub markdown: bool,
    /// The client has the dialogs of the refactorings that ask something first
    /// (`experimental.phoxite.dialogs`).
    pub dialogs: bool,
}

pub struct State {
    pub sender: Sender<Message>,
    pub support: ClientSupport,
    pub cache_dir: PathBuf,
    pub settings: RwLock<Settings>,
    pub workspaces: RwLock<Vec<Workspace>>,
    pub docs: RwLock<HashMap<Url, OpenDoc>>,
    /// The engine's events go here, and the main loop takes them.
    pub events: Sender<e::IdeEvent>,
    /// Problems by file and source (`php`, `blade`), published together.
    pub problems: Mutex<HashMap<PathBuf, BTreeMap<String, Vec<e::Diagnostic>>>>,
    /// The PHPStan and Psalm runs (`analyzers.rs`).
    pub analyzers: Mutex<crate::analyzers::Analyzers>,
    /// The user trusts the workspace: programs of the project may run.
    trusted: AtomicBool,
    next_doc: AtomicU32,
}

impl State {
    pub fn new(
        sender: Sender<Message>,
        support: ClientSupport,
        cache_dir: PathBuf,
        settings: Settings,
        events: Sender<e::IdeEvent>,
    ) -> Self {
        Self {
            sender,
            support,
            cache_dir,
            settings: RwLock::new(settings),
            workspaces: RwLock::new(Vec::new()),
            docs: RwLock::new(HashMap::new()),
            events,
            problems: Mutex::new(HashMap::new()),
            analyzers: Mutex::default(),
            trusted: AtomicBool::new(true),
            next_doc: AtomicU32::new(1),
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
        let settings = self.settings.read().clone();
        let php = settings.engine_config();
        let key = blake3::hash(root.to_string_lossy().as_bytes()).to_hex();
        let config = EngineConfig {
            cache_dir: self.cache_dir.join(&key[..16]).join(CACHE_SUBDIR),
            layout: LayoutConfig::from_config(&root, &php),
            stubs: StubFilter::from_config(&root, &php),
        };
        let events = self.events.clone();
        let sink: e::SharedSink = Arc::new(move |event: e::IdeEvent| {
            let _ = events.send(event);
        });
        match PhpEngine::open(config, sink) {
            Ok(engine) => {
                engine.configure_inspections(InspectionConfig::from_profile(&php.inspections));
                engine.set_laravel_settings(php.laravel.clone());
                crate::laravel::load_templates(self, &root, &engine);
                crate::schema::refresh_quietly(self, &root, &engine);
                tracing::info!("indexing {}", root.display());
                self.workspaces.write().push(Workspace { root, engine });
            }
            Err(e) => tracing::error!("can't start the engine for {}: {e}", root.display()),
        }
    }

    pub fn remove_workspace(&self, root: &Path) {
        let root = dunce::simplified(root).to_path_buf();
        let mut workspaces = self.workspaces.write();
        if let Some(i) = workspaces.iter().position(|w| w.root == root) {
            workspaces.remove(i).engine.shutdown();
        }
    }

    /// Passes changed settings to every engine.
    pub fn apply_settings(&self, settings: Settings) {
        let php = settings.engine_config();
        let analyzers = settings.analyzers.on_save;
        *self.settings.write() = settings;
        if !analyzers {
            crate::analyzers::clear_all(self);
        }
        for w in self.workspaces.read().iter() {
            w.engine.set_layout_config(LayoutConfig::from_config(&w.root, &php));
            w.engine.set_stub_filter(StubFilter::from_config(&w.root, &php));
            w.engine.configure_inspections(InspectionConfig::from_profile(&php.inspections));
            w.engine.set_laravel_settings(php.laravel.clone());
            crate::laravel::load_templates(self, &w.root, &w.engine);
            crate::schema::refresh_quietly(self, &w.root, &w.engine);
        }
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
        self.docs.write().insert(uri.clone(), doc);
        self.template_problems(&uri);
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
        self.template_problems(uri);
    }

    pub fn close(&self, uri: &Url) {
        let Some(doc) = self.docs.write().remove(uri) else { return };
        if let Some((_, engine)) = self.workspace_of(&doc.path) {
            // Also drops a template's embedded PHP.
            engine.document_closed(doc.id);
        }
        self.analyzers.lock().closed(&doc.path);
        self.problems.lock().remove(&doc.path);
        self.notify::<lsp::notification::PublishDiagnostics>(lsp::PublishDiagnosticsParams {
            uri: doc.uri,
            diagnostics: Vec::new(),
            version: None,
        });
    }

    /// A Blade template's own problems (unclosed sections and pushes).
    fn template_problems(&self, uri: &Url) {
        let Some(doc) = self.snapshot(uri) else { return };
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

    /// Records the problems of `path` from `source` and publishes the file's
    /// problems from every source.
    pub fn problems_changed(&self, path: &Path, source: String, items: Vec<e::Diagnostic>) {
        let path = dunce::simplified(path).to_path_buf();
        let all: Vec<e::Diagnostic> = {
            let mut problems = self.problems.lock();
            let by_source = problems.entry(path.clone()).or_default();
            by_source.insert(source, items);
            by_source.values().flatten().cloned().collect()
        };
        let Some(uri) = convert::uri_of(&path) else { return };
        let version = self.docs.read().get(&uri).map(|doc| doc.version);
        let settings = self.settings.read();
        let diagnostics = all
            .into_iter()
            .map(|mut d| {
                if let Some(severity) = d.code.as_deref().and_then(|code| settings.severity(code)) {
                    d.severity = severity;
                }
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
        for w in self.workspaces.write().drain(..) {
            w.engine.shutdown();
        }
    }
}
