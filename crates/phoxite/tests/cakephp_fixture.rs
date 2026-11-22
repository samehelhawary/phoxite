//! CakePHP support on the fixture application (`fixtures/cakephp-app`), on a stand-in for
//! CakePHP's API.
//!
//! Every PHP file of the application is inspected with nothing to report (its templates
//! among them, whose variables their action sets), and one name of each kind resolves.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use phoxite::base::{CancellationToken, DocId, IdeEvent, Pos, SharedSink};
use phoxite::engine::{CACHE_SUBDIR, EngineConfig, PhpEngine};
use phoxite::frameworks::Framework;
use phoxite::project::LayoutConfig;
use phoxite::stubs::StubFilter;

const CARET: &str = "/*|*/";

/// The application's folders: not `lib`, the stand-in for CakePHP.
const FOLDERS: [&str; 4] = ["config", "plugins", "src", "templates"];

struct Site {
    engine: Arc<PhpEngine>,
    root: PathBuf,
    next: u32,
    _cache: tempfile::TempDir,
}

impl Site {
    fn open() -> Self {
        let root =
            dunce::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/cakephp-app")).unwrap();
        let cache = tempfile::tempdir().unwrap();
        let config = EngineConfig {
            cache_dir: cache.path().join(CACHE_SUBDIR),
            layout: LayoutConfig { root: root.clone(), ..LayoutConfig::default() },
            stubs: StubFilter::default(),
        };
        let sink: SharedSink = Arc::new(|_: IdeEvent| {});
        let engine = PhpEngine::open(config, sink).unwrap();
        assert!(engine.wait_idle(Duration::from_secs(120)), "the fixture indexes");
        Self { engine, root, next: 1, _cache: cache }
    }

    fn document(&mut self, file: &str, text: &str) -> DocId {
        let doc = DocId(self.next);
        self.next += 1;
        self.engine.document_opened(doc, &self.root.join(file), 1, text);
        doc
    }

    /// `file` with `from` replaced by `to`, which has the caret, opened: the document and
    /// the caret.
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

    /// A place as a file relative to the site and a line from 1.
    fn place(&self, path: &Path, line: u32) -> String {
        let path = path.strip_prefix(&self.root).unwrap_or(path);
        format!("{}:{}", path.to_string_lossy().replace('\\', "/"), line + 1)
    }

    /// Where goto lands from the caret.
    fn goes_to(&mut self, file: &str, from: &str, to: &str) -> Vec<String> {
        let (doc, pos) = self.at(file, from, to);
        let found = self.engine.definition(doc, pos).unwrap_or_default();
        self.engine.document_closed(doc);
        found.iter().map(|target| self.place(&target.path, target.range.start.line)).collect()
    }

    /// What is offered at the caret.
    fn offered(&mut self, file: &str, from: &str, to: &str) -> Vec<String> {
        let (doc, pos) = self.at(file, from, to);
        let found = self.engine.completion(doc, pos, None, &CancellationToken::new()).expect("handled");
        self.engine.document_closed(doc);
        found.items.into_iter().map(|item| item.label).collect()
    }
}

fn php_files(folder: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(folder).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            php_files(&path, out);
        } else if phoxite::project::is_php_path(&path) {
            out.push(path);
        }
    }
}

#[test]
fn nothing_in_the_fixture_is_reported() {
    let mut site = Site::open();
    assert!(site.engine.frameworks().contains(Framework::CakePhp));
    let mut files = Vec::new();
    for folder in FOLDERS {
        php_files(&site.root.join(folder), &mut files);
    }
    files.sort();
    assert!(files.len() >= 25, "{files:?}");
    let mut reported = Vec::new();
    for path in files {
        let text = std::fs::read_to_string(&path).unwrap();
        let file = path.strip_prefix(&site.root).unwrap().to_string_lossy().replace('\\', "/");
        let doc = site.document(&file, &text);
        let found = site.engine.inspect(doc, &CancellationToken::new()).expect("open");
        for problem in &found.problems {
            reported.push(format!(
                "{file}:{} {} {}",
                problem.range.start.line + 1,
                problem.inspection,
                problem.message
            ));
        }
        site.engine.document_closed(doc);
    }
    assert!(reported.is_empty(), "{}", reported.join("\n"));
}

#[test]
fn one_name_of_each_kind_goes_where_it_should() {
    let mut site = Site::open();
    let books = "src/Controller/BooksController.php";
    let goes = |site: &mut Site, file: &str, from: &str, to: &str| site.goes_to(file, from, to);
    // Tables: the application's, a plugin's.
    assert_eq!(
        goes(&mut site, books, "fetchTable('Books')", "fetchTable('Bo/*|*/oks')"),
        ["src/Model/Table/BooksTable.php:7"]
    );
    assert_eq!(
        goes(&mut site, books, "fetchTable('Reviews.Reviews')", "fetchTable('Reviews.Rev/*|*/iews')"),
        ["plugins/Reviews/src/Model/Table/ReviewsTable.php:7"]
    );
    // A template, a layout, an element, a cell.
    assert_eq!(goes(&mut site, books, "render('view')", "render('vi/*|*/ew')"), ["templates/Books/view.php:1"]);
    assert_eq!(
        goes(&mut site, books, "setLayout('default')", "setLayout('def/*|*/ault')"),
        ["templates/layout/default.php:1"]
    );
    let index = "templates/Books/index.php";
    assert_eq!(
        goes(&mut site, index, "element('book_row'", "element('book_/*|*/row'"),
        ["templates/element/book_row.php:1"]
    );
    assert_eq!(goes(&mut site, index, "cell('Newest')", "cell('New/*|*/est')"), ["src/View/Cell/NewestCell.php:9"]);
    // The configuration, a named route.
    assert_eq!(goes(&mut site, books, "read('Library.name')", "read('Library.na/*|*/me')"), ["config/app.php:11"]);
    assert_eq!(goes(&mut site, books, "['_name' => 'home']", "['_name' => 'ho/*|*/me']"), ["config/routes.php:7"]);
    // Route arrays: this controller's action, another controller and its action, a plugin's.
    assert_eq!(goes(&mut site, books, "['action' => 'index']", "['action' => 'in/*|*/dex']"), [format!("{books}:10")]);
    assert_eq!(
        goes(
            &mut site,
            books,
            "['controller' => 'Authors', 'action' => 'view'",
            "['controller' => 'Auth/*|*/ors', 'action' => 'view'"
        ),
        ["src/Controller/AuthorsController.php:5"]
    );
    assert_eq!(
        goes(
            &mut site,
            books,
            "['controller' => 'Authors', 'action' => 'view'",
            "['controller' => 'Authors', 'action' => 'vi/*|*/ew'"
        ),
        ["src/Controller/AuthorsController.php:7"]
    );
    assert_eq!(
        goes(
            &mut site,
            books,
            "'controller' => 'Reviews', 'action' => 'index'",
            "'controller' => 'Reviews', 'action' => 'in/*|*/dex'"
        ),
        ["plugins/Reviews/src/Controller/ReviewsController.php:9"]
    );
    assert_eq!(
        goes(&mut site, books, "['plugin' => 'Reviews'", "['plugin' => 'Rev/*|*/iews'"),
        ["plugins/Reviews/src/ReviewsPlugin.php:1"]
    );
    assert_eq!(goes(&mut site, index, "['action' => 'add']", "['action' => 'a/*|*/dd']"), [format!("{books}:29")]);
    assert_eq!(goes(&mut site, "config/routes.php", "'Books::view'", "'Books::vi/*|*/ew'"), [format!("{books}:18")]);
    // What a class loads.
    let app = "src/Controller/AppController.php";
    assert_eq!(
        goes(&mut site, app, "loadComponent('Shelf')", "loadComponent('Sh/*|*/elf')"),
        ["src/Controller/Component/ShelfComponent.php:7"]
    );
    let table = "src/Model/Table/BooksTable.php";
    assert_eq!(
        goes(&mut site, table, "belongsTo('Authors')", "belongsTo('Auth/*|*/ors')"),
        ["src/Model/Table/AuthorsTable.php:7"]
    );
    assert_eq!(
        goes(&mut site, table, "['className' => 'Reviews.Reviews']", "['className' => 'Reviews.Rev/*|*/iews']"),
        ["plugins/Reviews/src/Model/Table/ReviewsTable.php:7"]
    );
    let behavior = goes(&mut site, table, "addBehavior('Timestamp')", "addBehavior('Time/*|*/stamp')");
    assert!(behavior.len() == 1 && behavior[0].starts_with("lib/cakephp.php:"), "{behavior:?}");
    assert_eq!(
        goes(&mut site, "src/View/AppView.php", "loadHelper('Stars')", "loadHelper('St/*|*/ars')"),
        ["src/View/Helper/StarsHelper.php:7"]
    );
    // A message of the translation catalogue.
    assert_eq!(
        goes(&mut site, books, "__('The book has been saved.')", "__('The book has /*|*/been saved.')"),
        ["resources/locales/de/default.po:5"]
    );
    // An entity's field that is a column of its table, where the migration makes it.
    assert_eq!(
        goes(&mut site, "src/Model/Entity/Book.php", "$this->pages", "$this->pa/*|*/ges"),
        ["config/Migrations/20240101000000_CreateLibrary.php:16"]
    );
    // A component of a controller, where the class it extends loads it.
    assert_eq!(
        goes(&mut site, books, "$this->Shelf->capacity()", "$this->She/*|*/lf->capacity()"),
        [format!("{app}:12")]
    );
}

#[test]
fn what_the_application_sets_and_loads_is_typed() {
    let mut site = Site::open();
    let has = |found: &[String], names: &[&str]| {
        for name in names {
            assert!(found.contains(&(*name).to_owned()), "{name}: {found:?}");
        }
    };
    // A template's variable, from its action: the entities a query found.
    let found = site.offered(
        "templates/Books/index.php",
        "<?php foreach ($books as $book): ?>",
        "<?php foreach ($books as $book): $book->/*|*/ ?>",
    );
    has(&found, &["title", "author", "reviews", "label"]);
    // An entity's association through a plugin's table; the view's helper.
    let found = site.offered("templates/Books/view.php", "$book->label()", "$book->author->/*|*/label()");
    has(&found, &["name", "books", "bookCount"]);
    let found = site.offered("templates/Books/view.php", "$this->Stars->show(", "$this->Stars->/*|*/show(");
    has(&found, &["show"]);
    let found = site.offered("templates/Books/view.php", "$this->Stars->show(", "$this->/*|*/Stars->show(");
    has(&found, &["Stars", "Html", "element"]);
    // A plugin's template, from the plugin's controller.
    let found = site.offered("plugins/Reviews/templates/Reviews/index.php", "$review->rating", "$review->/*|*/rating");
    has(&found, &["rating", "body"]);
    // A cell's template, an element.
    let found = site.offered("templates/cell/Newest/display.php", "$newest->title", "$newest->/*|*/title");
    has(&found, &["title"]);
    let found = site.offered("templates/element/book_row.php", "$book->title", "$book->/*|*/title");
    has(&found, &["title", "author"]);
    // A controller: its table and its associations, a component of the class it extends.
    let books = "src/Controller/BooksController.php";
    let found = site.offered(books, "$this->Books->newEmptyEntity()", "$this->Books->/*|*/newEmptyEntity()");
    has(&found, &["countPublished", "Authors", "Reviews", "find"]);
    let found = site.offered(books, "$this->Shelf->capacity()", "$this->Shelf->/*|*/capacity()");
    assert_eq!(found, ["capacity"]);
    // An entity's fields from the migration: one it documents, one it doesn't.
    let found = site.offered("src/Model/Entity/Book.php", "(int) $this->pages", "(int) $this->/*|*/pages");
    has(&found, &["title", "pages", "author"]);
    let found =
        site.offered(books, "$this->Books->newEmptyEntity()", "$this->Books->Authors->get(1)->/*|*/newEmptyEntity()");
    has(&found, &["name", "bookCount"]);
}
