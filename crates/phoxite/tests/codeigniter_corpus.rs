//! CodeIgniter support on CodeIgniter itself: the `codeigniter` corpus (the framework's
//! repository, which has an application skeleton in `app/` beside `system/`) opened with
//! the engine, and a controller written against it.
//!
//! The engine's own tests use a small stand-in for CodeIgniter's API; this one checks
//! that what they assume holds for the real source: that its language files and filter
//! aliases are read, that its helper functions are typed, and that its names resolve.
//!
//! Ignored: it needs the corpus (`scripts/php-corpus.sh codeigniter`).
//!
//! ```text
//! cargo test -p phoxite --release --test codeigniter_corpus -- --ignored --nocapture
//! ```
//!
//! Environment: `PHP_CORPUS_DIR` (default `../php-corpus` next to the workspace).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use phoxite::base::{CancellationToken, DocId, IdeEvent, Pos, SharedSink};
use phoxite::engine::{CACHE_SUBDIR, EngineConfig, PhpEngine};
use phoxite::frameworks::Framework;
use phoxite::project::LayoutConfig;
use phoxite::stubs::StubFilter;

const CARET: &str = "/*|*/";

fn corpus() -> PathBuf {
    std::env::var_os("PHP_CORPUS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../php-corpus"))
        .join("codeigniter")
}

struct Site {
    engine: Arc<PhpEngine>,
    root: PathBuf,
    next: u32,
    _cache: tempfile::TempDir,
}

impl Site {
    /// A controller of the application with `body` in a method, opened as a document: where
    /// its caret is.
    fn open(&mut self, body: &str) -> (DocId, Pos) {
        let source = format!(
            "<?php\nnamespace App\\Controllers;\nclass Probe extends BaseController\n{{\n    public function run()\n    {{\n        {body}\n    }}\n}}\n"
        );
        let at = source.find(CARET).expect("a caret");
        let text = format!("{}{}", &source[..at], &source[at + CARET.len()..]);
        let line = text[..at].matches('\n').count() as u32;
        let col = text[..at].rsplit('\n').next().unwrap_or_default().encode_utf16().count() as u32;
        let doc = DocId(self.next);
        self.next += 1;
        self.engine.document_opened(doc, &self.root.join("app/Controllers/Probe.php"), 1, &text);
        (doc, Pos::new(line, col))
    }

    /// The files goto at the caret of `body` lands in, relative to the repository.
    fn goes_to(&mut self, body: &str) -> Vec<String> {
        let (doc, pos) = self.open(body);
        let found = self.engine.definition(doc, pos).unwrap_or_default();
        self.engine.document_closed(doc);
        let relative = |path: &Path| path.strip_prefix(&self.root).unwrap_or(path).to_string_lossy().replace('\\', "/");
        found.iter().map(|target| relative(&target.path)).collect()
    }

    /// What completion offers at the caret of `body`.
    fn completes(&mut self, body: &str) -> Vec<String> {
        let (doc, pos) = self.open(body);
        let found = self.engine.completion(doc, pos, None, &CancellationToken::new());
        self.engine.document_closed(doc);
        found.map(|found| found.items.into_iter().map(|item| item.label).collect()).unwrap_or_default()
    }
}

#[test]
#[ignore = "needs the corpora (scripts/php-corpus.sh)"]
fn codeigniter_itself() {
    let root = dunce::canonicalize(corpus()).expect("the codeigniter corpus (scripts/php-corpus.sh codeigniter)");
    let cache = tempfile::tempdir().unwrap();
    let config = EngineConfig {
        cache_dir: cache.path().join(CACHE_SUBDIR),
        layout: LayoutConfig { root: root.clone(), ..LayoutConfig::default() },
        stubs: StubFilter::default(),
    };
    let sink: SharedSink = Arc::new(|_: IdeEvent| {});
    let started = Instant::now();
    let engine = PhpEngine::open(config, sink).unwrap();
    assert!(engine.wait_idle(Duration::from_secs(900)), "CodeIgniter indexes");
    println!("indexed in {:?}", started.elapsed());
    let mut site = Site { engine, root, next: 1, _cache: cache };
    assert!(site.engine.frameworks().contains(Framework::CodeIgniter), "detected from its `spark`");

    let started = Instant::now();
    let project = site.engine.codeigniter();
    println!(
        "{} language lines, {} filter aliases, read in {:?}",
        project.lines.len(),
        project.filters.len(),
        started.elapsed()
    );
    assert!(
        project.lines.len() > 300 && project.filters.len() >= 5,
        "{} lines, {} filters",
        project.lines.len(),
        project.filters.len()
    );

    let has = |found: &[String], label: &str| found.iter().any(|known| known == label);
    // Files: a view of the skeleton, a helper and a language line of the framework.
    assert_eq!(site.goes_to("return view('welcome_/*|*/message');"), ["app/Views/welcome_message.php"]);
    assert_eq!(site.goes_to("helper('u/*|*/rl');"), ["system/Helpers/url_helper.php"]);
    assert_eq!(site.goes_to("return lang('Validation.requi/*|*/red');"), ["system/Language/en/Validation.php"]);
    let found = site.completes("helper('/*|*/');");
    assert!(has(&found, "form") && has(&found, "url"), "{found:?}");
    let found = site.completes("return lang('/*|*/');");
    assert!(has(&found, "Validation.required"), "{} lines", found.len());
    // A filter alias, a configuration class, a service's method.
    assert_eq!(site.goes_to("$options = ['filter' => 'cs/*|*/rf'];"), ["app/Config/Filters.php"]);
    // (The user guide's examples declare classes of the same name: the application's is first.)
    let found = site.goes_to("$app = config('Ap/*|*/p');");
    assert_eq!(found.first().map(String::as_str), Some("app/Config/App.php"));
    let found = site.goes_to("$routes = service('rout/*|*/es');");
    assert_eq!(found.first().map(String::as_str), Some("system/Config/Services.php"));
    let found = site.completes("$s = service('/*|*/');");
    assert!(has(&found, "request") && has(&found, "routes") && has(&found, "session"), "{} services", found.len());

    // Types: a configuration by its name, a service by its method.
    let found = site.completes("config('App')->/*|*/");
    assert!(has(&found, "baseURL"), "{found:?}");
    let found = site.completes("service('session')->/*|*/");
    assert!(has(&found, "get") && has(&found, "setFlashdata"), "a session: {} members", found.len());
    let found = site.completes("service('routes')->/*|*/");
    assert!(has(&found, "get") && has(&found, "group"), "a route collection: {} members", found.len());
}

fn open_corpus(root: PathBuf) -> Site {
    let cache = tempfile::tempdir().unwrap();
    let config = EngineConfig {
        cache_dir: cache.path().join(CACHE_SUBDIR),
        layout: LayoutConfig { root: root.clone(), ..LayoutConfig::default() },
        stubs: StubFilter::default(),
    };
    let sink: SharedSink = Arc::new(|_: IdeEvent| {});
    let engine = PhpEngine::open(config, sink).unwrap();
    assert!(engine.wait_idle(Duration::from_secs(900)), "indexed");
    Site { engine, root, next: 1, _cache: cache }
}

fn php_files(folder: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(folder) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            php_files(&path, out);
        } else if path.extension().is_some_and(|extension| extension == "php") {
            out.push(path);
        }
    }
}

impl Site {
    /// What the CodeIgniter inspections say of the PHP files below `folder`, as
    /// `file: message`.
    fn reported(&mut self, folder: &str) -> Vec<String> {
        let mut files = Vec::new();
        php_files(&self.root.join(folder), &mut files);
        files.sort();
        let mut out = Vec::new();
        for file in files {
            let Ok(text) = std::fs::read_to_string(&file) else { continue };
            let doc = DocId(self.next);
            self.next += 1;
            self.engine.document_opened(doc, &file, 1, &text);
            let inspected = self.engine.inspect(doc, &CancellationToken::new());
            self.engine.document_closed(doc);
            let Some(inspected) = inspected else { continue };
            let relative = file.strip_prefix(&self.root).unwrap_or(&file).to_string_lossy().replace('\\', "/");
            for problem in inspected.problems.iter().filter(|problem| problem.inspection.contains("CodeIgniter")) {
                out.push(format!("{relative}:{}: {}", problem.range.start.line + 1, problem.message));
            }
        }
        out
    }
}

/// The "unknown name" inspections on CodeIgniter's own code: nothing in its application
/// skeleton, and what they say of its tests and of the user guide's examples (which name
/// things that aren't there on purpose) printed to be read.
#[test]
#[ignore = "needs the corpora (scripts/php-corpus.sh)"]
fn codeigniter_names_nothing_unknown() {
    let root = dunce::canonicalize(corpus()).expect("the codeigniter corpus (scripts/php-corpus.sh codeigniter)");
    let mut site = open_corpus(root);
    let project = site.engine.codeigniter();
    println!(
        "{} namespaces, {} routes by path, rule sets {:?}",
        project.namespaces.len(),
        project.route_paths.len(),
        project.rule_sets
    );
    assert_eq!(site.reported("app"), Vec::<String>::new());
    // (The framework's own files aren't asked: its toolbar's views have a folder of their own.)
    assert_eq!(site.reported("system"), Vec::<String>::new());
    for folder in [
        "tests/_support",
        "user_guide_src/source/libraries/validation",
        "user_guide_src/source/incoming",
        "user_guide_src/source/outgoing",
        "user_guide_src/source/general",
    ] {
        let found = site.reported(folder);
        println!("{folder}: {} reported", found.len());
        for line in found.iter().take(40) {
            println!("  {line}");
        }
    }

    // Names of the framework itself.
    assert_eq!(
        site.goes_to("$rules = ['name' => 'required|min_le/*|*/ngth[3]'];\n        $this->validate($rules);"),
        Vec::<String>::new()
    );
    let found = site.goes_to("$this->validate(['name' => 'required|min_le/*|*/ngth[3]']);");
    assert_eq!(found, ["system/Validation/StrictRules/Rules.php"]);
    let found = site.completes("$this->validate(['name' => '/*|*/']);");
    assert!(
        found.iter().any(|rule| rule == "valid_email") && found.iter().any(|rule| rule == "is_unique"),
        "{found:?}"
    );
    let found = site.goes_to("\\CodeIgniter\\Events\\Events::on('pre_sys/*|*/tem', static function () {});");
    assert_eq!(found, ["system/CodeIgniter.php"]);
}

/// CodeIgniter 3 itself: its `application/` skeleton beside `system/`.
#[test]
#[ignore = "needs the corpora (scripts/php-corpus.sh)"]
fn codeigniter_3_itself() {
    let root = corpus().parent().expect("the corpora's folder").join("codeigniter3");
    let root = dunce::canonicalize(root).expect("the codeigniter3 corpus (scripts/php-corpus.sh codeigniter3)");
    let mut site = open_corpus(root);
    assert!(site.engine.frameworks().contains(Framework::CodeIgniter), "detected from `system/core/CodeIgniter.php`");
    let project = site.engine.codeigniter();
    println!("{} language lines, {} configuration items", project.lines.len(), project.config_items.len());
    assert!(project.legacy && project.lines.len() > 200 && project.config_items.len() > 20);
    assert_eq!(site.reported("application"), Vec::<String>::new());

    let welcome = site.root.join("application/controllers/Welcome.php");
    let text = std::fs::read_to_string(&welcome).unwrap();
    let mut at = |from: &str, to: &str| {
        assert!(text.contains(from), "{from}");
        let source = text.replacen(from, to, 1);
        let caret = source.find(CARET).unwrap();
        let text = format!("{}{}", &source[..caret], &source[caret + CARET.len()..]);
        let line = text[..caret].matches('\n').count() as u32;
        let col = text[..caret].rsplit('\n').next().unwrap_or_default().encode_utf16().count() as u32;
        let doc = DocId(site.next);
        site.next += 1;
        site.engine.document_opened(doc, &welcome, 1, &text);
        let found = site.engine.definition(doc, Pos::new(line, col)).unwrap_or_default();
        let labels = site.engine.completion(doc, Pos::new(line, col), None, &CancellationToken::new());
        site.engine.document_closed(doc);
        let relative = |path: &Path| path.strip_prefix(&site.root).unwrap_or(path).to_string_lossy().replace('\\', "/");
        let found: Vec<String> = found.iter().map(|target| relative(&target.path)).collect();
        let labels: Vec<String> =
            labels.map(|found| found.items.into_iter().map(|item| item.label).collect()).unwrap_or_default();
        (found, labels)
    };
    let view = "$this->load->view('welcome_message');";
    assert_eq!(at(view, "$this->load->view('welcome_/*|*/message');").0, ["application/views/welcome_message.php"]);
    assert_eq!(at(view, "$this->load->helper('u/*|*/rl');").0, ["system/helpers/url_helper.php"]);
    assert_eq!(at(view, "$this->load->library('sess/*|*/ion');").0, ["system/libraries/Session/Session.php"]);
    assert_eq!(
        at(view, "$this->lang->line('db_invalid_/*|*/connection_str');").0,
        ["system/language/english/db_lang.php"]
    );
    assert_eq!(at(view, "$this->config->item('base_/*|*/url');").0, ["application/config/config.php"]);
    assert_eq!(at(view, "redirect('welcome/in/*|*/dex');").0, ["application/controllers/Welcome.php"]);
    // What the loader gives the controller.
    let has = |found: &[String], label: &str| found.iter().any(|known| known == label);
    let found = at(view, "$this->load->/*|*/;").1;
    assert!(has(&found, "view") && has(&found, "model") && has(&found, "library"), "the loader: {found:?}");
    let found = at(view, "$this->input->/*|*/;").1;
    assert!(has(&found, "post") && has(&found, "get"), "the input: {found:?}");
    let found = at(view, "$this->session->/*|*/;").1;
    assert!(has(&found, "userdata") && has(&found, "set_userdata"), "the session: {found:?}");
    let found = at(view, "$this->db->/*|*/;").1;
    assert!(has(&found, "where") && has(&found, "get"), "the query builder: {found:?}");
}
