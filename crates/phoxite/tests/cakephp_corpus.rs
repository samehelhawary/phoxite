//! CakePHP support on CakePHP itself: the fixture application (`fixtures/cakephp-app`)
//! without its stand-in, on the real framework as an include path (`cakephp-framework`,
//! CakePHP 5, and `cakephp4-framework`, CakePHP 4), and CakePHP's application skeleton
//! (`cakephp`), which has no `vendor/` until Composer installs it.
//!
//! The engine's own tests use a small stand-in for CakePHP's API; this one checks that
//! what they assume holds for the real source: that tables, entities, components and
//! helpers are typed (CakePHP 5 documents generics, CakePHP 4 doesn't), that the names
//! resolve, and that the "unknown name" inspections say nothing of either application.
//! What the other inspections say is counted and printed to be read.
//!
//! Ignored: it needs the corpora
//! (`scripts/php-corpus.sh cakephp cakephp-framework cakephp4-framework`).
//!
//! ```text
//! cargo test -p phoxite --release --test cakephp_corpus -- --ignored --nocapture
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

fn corpus(name: &str) -> PathBuf {
    let root = std::env::var_os("PHP_CORPUS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../php-corpus"))
        .join(name);
    dunce::canonicalize(root).expect("the corpus (scripts/php-corpus.sh)")
}

struct Site {
    engine: Arc<PhpEngine>,
    root: PathBuf,
    next: u32,
    _cache: tempfile::TempDir,
    _copy: Option<tempfile::TempDir>,
}

fn copy(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap().flatten() {
        let path = entry.path();
        let target = to.join(entry.file_name());
        if path.is_dir() {
            copy(&path, &target);
        } else {
            std::fs::copy(&path, &target).unwrap();
        }
    }
}

impl Site {
    /// The project at `root`, with the framework corpus `framework` as an include path.
    fn open(root: PathBuf, framework: &str, copy: Option<tempfile::TempDir>) -> Self {
        let cache = tempfile::tempdir().unwrap();
        let include = corpus(framework).join("src");
        let config = EngineConfig {
            cache_dir: cache.path().join(CACHE_SUBDIR),
            layout: LayoutConfig { root: root.clone(), include_paths: vec![include], ..LayoutConfig::default() },
            stubs: StubFilter::default(),
        };
        let sink: SharedSink = Arc::new(|_: IdeEvent| {});
        let started = Instant::now();
        let engine = PhpEngine::open(config, sink).unwrap();
        assert!(engine.wait_idle(Duration::from_secs(600)), "the corpus indexes");
        println!("indexed in {:?}", started.elapsed());
        Self { engine, root, next: 1, _cache: cache, _copy: copy }
    }

    /// The fixture application without its stand-in, on the framework corpus `framework`.
    fn fixture(framework: &str) -> Self {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/cakephp-app");
        let tmp = tempfile::tempdir().unwrap();
        let root = dunce::canonicalize(tmp.path()).unwrap().join("app");
        copy(&fixture, &root);
        std::fs::remove_dir_all(root.join("lib")).unwrap();
        Self::open(root, framework, Some(tmp))
    }

    fn document(&mut self, file: &str, text: &str) -> DocId {
        let doc = DocId(self.next);
        self.next += 1;
        self.engine.document_opened(doc, &self.root.join(file), 1, text);
        doc
    }

    /// `file` with `from` replaced by `to`, which has the caret, opened.
    fn at(&mut self, file: &str, from: &str, to: &str) -> (DocId, Pos) {
        let source = std::fs::read_to_string(self.root.join(file)).unwrap().replace("\r\n", "\n");
        assert!(source.contains(from), "{from} in {file}");
        let source = source.replacen(from, to, 1);
        let at = source.find(CARET).expect("a caret");
        let text = format!("{}{}", &source[..at], &source[at + CARET.len()..]);
        let line = text[..at].matches('\n').count() as u32;
        let col = text[..at].rsplit('\n').next().unwrap_or_default().encode_utf16().count() as u32;
        (self.document(file, &text), Pos::new(line, col))
    }

    /// The files goto lands in from the caret, relative to the site where they are in it.
    fn goes_to(&mut self, file: &str, from: &str, to: &str) -> Vec<String> {
        let (doc, pos) = self.at(file, from, to);
        let found = self.engine.definition(doc, pos).unwrap_or_default();
        self.engine.document_closed(doc);
        found
            .iter()
            .map(|target| {
                let path = target.path.strip_prefix(&self.root).unwrap_or(&target.path);
                path.to_string_lossy().replace('\\', "/")
            })
            .collect()
    }

    /// What is offered at the caret.
    fn offered(&mut self, file: &str, from: &str, to: &str) -> Vec<String> {
        let (doc, pos) = self.at(file, from, to);
        let found = self.engine.completion(doc, pos, None, &CancellationToken::new()).expect("handled");
        self.engine.document_closed(doc);
        found.items.into_iter().map(|item| item.label).collect()
    }

    /// The reports of CakePHP's inspections on the PHP files of `folders`, and how many
    /// the other inspections make (printed).
    fn reports(&mut self, folders: &[&str]) -> (Vec<String>, usize, usize) {
        let mut files = Vec::new();
        for folder in folders {
            php_files(&self.root.join(folder), &mut files);
        }
        files.sort();
        let (mut unknown, mut others) = (Vec::new(), 0);
        let count = files.len();
        for path in files {
            let text = std::fs::read_to_string(&path).unwrap();
            let file = path.strip_prefix(&self.root).unwrap().to_string_lossy().replace('\\', "/");
            let doc = self.document(&file, &text);
            let found = self.engine.inspect(doc, &CancellationToken::new()).expect("open");
            for problem in &found.problems {
                let line =
                    format!("{file}:{} {} {}", problem.range.start.line + 1, problem.inspection, problem.message);
                if problem.inspection.contains("Cake") {
                    unknown.push(line);
                } else {
                    others += 1;
                    println!("{line}");
                }
            }
            self.engine.document_closed(doc);
        }
        (unknown, others, count)
    }
}

fn php_files(folder: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(folder).into_iter().flatten().flatten() {
        let path = entry.path();
        if path.is_dir() {
            php_files(&path, out);
        } else if phoxite::project::is_php_path(&path) {
            out.push(path);
        }
    }
}

fn has(found: &[String], names: &[&str]) {
    for name in names {
        assert!(found.contains(&(*name).to_owned()), "{name}: {found:?}");
    }
}

/// What holds for the fixture application on either version of the framework.
fn the_fixture_is_read_and_typed(framework: &str) {
    let mut site = Site::fixture(framework);
    assert!(site.engine.frameworks().contains(Framework::CakePhp));
    let books = "src/Controller/BooksController.php";

    // Names: the application's own, and CakePHP's components, behaviours and helpers.
    assert_eq!(
        site.goes_to(books, "fetchTable('Books')", "fetchTable('Bo/*|*/oks')"),
        ["src/Model/Table/BooksTable.php"]
    );
    assert_eq!(site.goes_to(books, "render('view')", "render('vi/*|*/ew')"), ["templates/Books/view.php"]);
    assert_eq!(site.goes_to(books, "['action' => 'index']", "['action' => 'in/*|*/dex']"), [books]);
    let app = "src/Controller/AppController.php";
    let flash = site.goes_to(app, "loadComponent('Flash')", "loadComponent('Fl/*|*/ash')");
    assert!(flash.len() == 1 && flash[0].ends_with("Controller/Component/FlashComponent.php"), "{flash:?}");
    let table = "src/Model/Table/BooksTable.php";
    let behavior = site.goes_to(table, "addBehavior('Timestamp')", "addBehavior('Time/*|*/stamp')");
    assert!(behavior.len() == 1 && behavior[0].ends_with("ORM/Behavior/TimestampBehavior.php"), "{behavior:?}");
    let index = "templates/Books/index.php";
    assert_eq!(
        site.goes_to(index, "element('book_row'", "element('book_/*|*/row'"),
        ["templates/element/book_row.php"]
    );
    assert_eq!(site.goes_to(index, "['action' => 'add']", "['action' => 'a/*|*/dd']"), [books]);
    assert_eq!(site.goes_to("config/routes.php", "'Books::view'", "'Books::vi/*|*/ew'"), [books]);

    // A message of the catalogue, a column of the migration.
    assert_eq!(
        site.goes_to(books, "__('The book has been saved.')", "__('The book has /*|*/been saved.')"),
        ["resources/locales/de/default.po"]
    );
    assert_eq!(
        site.goes_to("src/Model/Entity/Book.php", "$this->pages", "$this->pa/*|*/ges"),
        ["config/Migrations/20240101000000_CreateLibrary.php"]
    );

    // Types: the controller's table and what it finds, on the real declarations.
    let found = site.offered(books, "$this->Books->newEmptyEntity()", "$this->Books->/*|*/newEmptyEntity()");
    has(&found, &["countPublished", "Authors", "Reviews", "find", "newEmptyEntity"]);
    let found =
        site.offered(books, "$this->Books->newEmptyEntity()", "$this->Books->find()->first()->/*|*/newEmptyEntity()");
    has(&found, &["title", "author", "label"]);
    let found =
        site.offered(books, "$this->Books->newEmptyEntity()", "$this->Books->get(1)->author->/*|*/newEmptyEntity()");
    has(&found, &["name", "bookCount"]);
    let found =
        site.offered(books, "$this->Books->newEmptyEntity()", "$this->Books->Authors->get(1)->/*|*/newEmptyEntity()");
    has(&found, &["name", "bookCount"]);
    // Components: CakePHP's and the application's.
    let found = site.offered(books, "$this->Flash->success(", "$this->Flash->/*|*/success(");
    has(&found, &["success", "error"]);
    let found = site.offered(books, "$this->Shelf->capacity()", "$this->Shelf->/*|*/capacity()");
    has(&found, &["capacity"]);
    // A template: its variables, the view and CakePHP's helpers.
    let found =
        site.offered(index, "<?php foreach ($books as $book): ?>", "<?php foreach ($books as $book): $book->/*|*/ ?>");
    has(&found, &["title", "author", "reviews", "label"]);
    let found = site.offered("templates/Books/view.php", "$this->Stars->show(", "$this->/*|*/Stars->show(");
    has(&found, &["Stars", "Html", "Form", "element", "fetch"]);
    let found = site.offered("templates/Books/view.php", "$this->Stars->show(", "$this->Form->/*|*/Stars->show(");
    has(&found, &["create", "control"]);
    let found = site.offered("plugins/Reviews/templates/Reviews/index.php", "$review->rating", "$review->/*|*/rating");
    has(&found, &["rating", "body"]);

    let (unknown, others, count) = site.reports(&["config", "plugins", "src", "templates"]);
    println!("{count} files, {others} other problems");
    assert!(count >= 20, "{count}");
    assert!(unknown.is_empty(), "{}", unknown.join("\n"));
}

#[test]
#[ignore = "needs the corpus: scripts/php-corpus.sh cakephp-framework"]
fn the_fixture_is_read_and_typed_on_cakephp_5() {
    the_fixture_is_read_and_typed("cakephp-framework");
}

#[test]
#[ignore = "needs the corpus: scripts/php-corpus.sh cakephp4-framework"]
fn the_fixture_is_read_and_typed_on_cakephp_4() {
    the_fixture_is_read_and_typed("cakephp4-framework");
}

#[test]
#[ignore = "needs the corpora: scripts/php-corpus.sh cakephp cakephp-framework"]
fn cakephps_application_skeleton_reports_no_unknown_name() {
    let mut site = Site::open(corpus("cakephp"), "cakephp-framework", None);
    assert!(site.engine.frameworks().contains(Framework::CakePhp));
    let project = site.engine.cake();
    assert_eq!(project.namespace, "App");
    let keys: Vec<&str> = project.config.iter().map(|def| def.name.as_str()).collect();
    println!("{} configuration keys", keys.len());
    for key in ["debug", "App.namespace", "App.encoding", "Datasources.default.driver", "Error.errorLevel"] {
        assert!(keys.contains(&key), "{key}");
    }
    let app = "src/Controller/AppController.php";
    let flash = site.goes_to(app, "loadComponent('Flash')", "loadComponent('Fl/*|*/ash')");
    assert!(flash.len() == 1 && flash[0].ends_with("Controller/Component/FlashComponent.php"), "{flash:?}");
    let found = site.offered(
        app,
        "$this->loadComponent('Flash');",
        "$this->loadComponent('Flash');\n        $this->Flash->/*|*/;",
    );
    has(&found, &["success", "error", "set"]);
    assert_eq!(
        site.goes_to("config/routes.php", "'Pages::display'", "'Pages::dis/*|*/play'"),
        ["src/Controller/PagesController.php"]
    );
    // A template of the skeleton: `$this` is the application's view.
    let found = site.offered("templates/layout/default.php", "$this->fetch('title')", "$this->/*|*/fetch('title')");
    has(&found, &["fetch", "Html", "Flash", "element"]);

    let (unknown, others, count) = site.reports(&["config", "src", "templates"]);
    println!("{count} files, {others} other problems");
    assert!(count >= 15, "{count}");
    assert!(unknown.is_empty(), "{}", unknown.join("\n"));
}
