//! CodeIgniter support on the fixture application (`fixtures/codeigniter-app`), on a
//! stand-in for CodeIgniter's API.
//!
//! Every PHP file of `app/` is inspected with nothing to report (its views among them,
//! whose variables their controller passes), and one name of each kind resolves.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use phoxite::base::{CancellationToken, DocId, IdeEvent, Pos, SharedSink};
use phoxite::engine::{CACHE_SUBDIR, EngineConfig, PhpEngine};
use phoxite::frameworks::Framework;
use phoxite::project::LayoutConfig;
use phoxite::stubs::StubFilter;

const CARET: &str = "/*|*/";

struct Site {
    engine: Arc<PhpEngine>,
    root: PathBuf,
    next: u32,
    _cache: tempfile::TempDir,
}

impl Site {
    fn open() -> Self {
        let root =
            dunce::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/codeigniter-app")).unwrap();
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

    /// The usages of what the caret is on.
    fn usages(&mut self, file: &str, from: &str, to: &str) -> Vec<String> {
        let (doc, pos) = self.at(file, from, to);
        let found = self.engine.references(doc, pos, true, &CancellationToken::new()).expect("handled").unwrap();
        self.engine.document_closed(doc);
        found.iter().map(|usage| self.place(&usage.path, usage.range.start.line)).collect()
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
    assert!(site.engine.frameworks().contains(Framework::CodeIgniter));
    let mut files = Vec::new();
    // Not the stand-in for CodeIgniter.
    php_files(&site.root.join("app"), &mut files);
    files.sort();
    assert!(files.len() >= 10, "{files:?}");
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
fn the_fixtures_names_resolve() {
    const BOOKS: &str = "app/Controllers/Books.php";
    const VIEW: &str = "app/Views/books/list.php";
    let mut site = Site::open();

    // Files: a view, a helper of the application and one of the framework, a language line.
    assert_eq!(site.goes_to(BOOKS, "view('books/list'", "view('books/li/*|*/st'"), [format!("{VIEW}:1")]);
    assert_eq!(
        site.goes_to(BOOKS, "helper(['shop', 'url'])", "helper(['sh/*|*/op', 'url'])"),
        ["app/Helpers/shop_helper.php:1"]
    );
    assert_eq!(
        site.goes_to(BOOKS, "helper(['shop', 'url'])", "helper(['shop', 'u/*|*/rl'])"),
        ["system/Helpers/url_helper.php:1"]
    );
    assert_eq!(
        site.goes_to(BOOKS, "lang('Shop.cart.empty')", "lang('Shop.cart.em/*|*/pty')"),
        ["app/Language/en/Shop.php:6"]
    );
    // A route, a filter, and their usages.
    assert_eq!(site.goes_to(BOOKS, "route('books.list')", "route('books.li/*|*/st')"), ["app/Config/Routes.php:6"]);
    let found = site.usages(BOOKS, "route('books.list')", "route('books.li/*|*/st')");
    assert_eq!(found, ["app/Config/Routes.php:6", "app/Controllers/Books.php:28", "app/Filters/Auth.php:13"]);
    let found = site.goes_to("app/Config/Routes.php", "'filter' => 'auth'", "'filter' => 'au/*|*/th'");
    assert_eq!(found, ["app/Config/Filters.php:11"]);
    let found = site.usages("app/Config/Routes.php", "'filter' => 'auth'", "'filter' => 'au/*|*/th'");
    assert_eq!(found, ["app/Config/Filters.php:11", "app/Config/Filters.php:16", "app/Config/Routes.php:6"]);
    // Classes, and a service's method.
    assert_eq!(site.goes_to(BOOKS, "model('BookModel')", "model('Book/*|*/Model')"), ["app/Models/BookModel.php:7"]);
    assert_eq!(site.goes_to(BOOKS, "config('Shop')", "config('Sh/*|*/op')"), ["app/Config/Shop.php:5"]);
    assert_eq!(site.goes_to(BOOKS, "service('catalog')", "service('cata/*|*/log')"), ["app/Config/Services.php:10"]);

    // Types: what the helper functions make, and what the controller passes the view.
    assert_eq!(site.goes_to(BOOKS, "$model->titles()", "$model->tit/*|*/les()"), ["app/Models/BookModel.php:12"]);
    assert_eq!(site.goes_to(BOOKS, "$shop->currency", "$shop->curr/*|*/ency"), ["app/Config/Shop.php:7"]);
    assert_eq!(site.goes_to(VIEW, "$catalog->count()", "$catalog->co/*|*/unt()"), ["app/Libraries/Catalog.php:7"]);
    // An event, a view cell, a validation rule and a rule group, a table and its columns,
    // a route named by its path.
    const ORDERS: &str = "app/Controllers/Orders.php";
    const MIGRATION: &str = "app/Database/Migrations/2024-01-01-000000_CreateBooks.php";
    let found = site.goes_to("app/Config/Events.php", "'order_placed'", "'order_/*|*/placed'");
    assert_eq!(found, [format!("{ORDERS}:18")]);
    assert_eq!(site.goes_to(ORDERS, "'Card::render'", "'Card::ren/*|*/der'"), ["app/Cells/Card.php:7"]);
    assert_eq!(site.goes_to(ORDERS, "'required|valid_email'", "'required|valid_/*|*/email'"), ["system/More.php:59"]);
    assert_eq!(
        site.goes_to(ORDERS, "'required|min_length[3]'", "'required|min_/*|*/length[3]'"),
        ["system/More.php:51"]
    );
    assert_eq!(site.goes_to(ORDERS, "'order')", "'or/*|*/der')"), ["app/Config/Validation.php:14"]);
    assert_eq!(site.goes_to(ORDERS, "table('books')", "table('bo/*|*/oks')"), [format!("{MIGRATION}:16")]);
    assert_eq!(site.goes_to(ORDERS, "where('title'", "where('ti/*|*/tle'"), [format!("{MIGRATION}:13")]);
    assert_eq!(site.goes_to(ORDERS, "orderBy('price')", "orderBy('pr/*|*/ice')"), [format!("{MIGRATION}:14")]);
    assert_eq!(site.goes_to(ORDERS, "route_to('orders')", "route_to('ord/*|*/ers')"), ["app/Config/Routes.php:8"]);
    let found = site.goes_to("app/Models/ShelfModel.php", "['title', 'price']", "['title', 'pr/*|*/ice']");
    assert_eq!(found, [format!("{MIGRATION}:14")]);
    // A route's handler is a method of a controller.
    let found = site.goes_to("app/Config/Routes.php", "'Books::show/$1'", "'Books::sh/*|*/ow/$1'");
    assert_eq!(found, ["app/Controllers/Books.php:26"]);
    // A view names its layout and the object rendering it.
    assert_eq!(site.goes_to(VIEW, "extend('layout')", "extend('lay/*|*/out')"), ["app/Views/layout.php:1"]);
    assert_eq!(site.goes_to(VIEW, "$this->section", "$this->sec/*|*/tion"), ["system/Common.php:90"]);
}
