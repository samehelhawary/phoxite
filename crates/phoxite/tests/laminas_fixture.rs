//! Laminas support on the fixture applications (`fixtures/laminas-app`, an MVC one, and
//! `fixtures/mezzio-app`), each on a stand-in for the framework's API.
//!
//! Every PHP file of the applications is inspected with nothing to report (their templates
//! among them, whose variables an action or a handler gives), and one name of each kind
//! resolves.

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
    fn open(fixture: &str) -> Self {
        let root =
            dunce::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures").join(fixture)).unwrap();
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

/// The PHP files of the application's folders, inspected: what is reported.
fn reported(site: &mut Site, folders: &[&str], at_least: usize) -> Vec<String> {
    let mut files = Vec::new();
    for folder in folders {
        php_files(&site.root.join(folder), &mut files);
    }
    files.sort();
    assert!(files.len() >= at_least, "{files:?}");
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
    reported
}

fn has(found: &[String], names: &[&str]) {
    for name in names {
        assert!(found.contains(&(*name).to_owned()), "{name}: {found:?}");
    }
}

#[test]
fn nothing_in_the_fixtures_is_reported() {
    let mut site = Site::open("laminas-app");
    assert!(site.engine.frameworks().contains(Framework::Laminas));
    let found = reported(&mut site, &["config", "module"], 19);
    assert!(found.is_empty(), "{}", found.join("\n"));
    let mut site = Site::open("mezzio-app");
    assert!(site.engine.frameworks().contains(Framework::Laminas));
    let found = reported(&mut site, &["config", "src"], 10);
    assert!(found.is_empty(), "{}", found.join("\n"));
}

#[test]
fn one_name_of_each_kind_goes_where_it_should() {
    let mut site = Site::open("laminas-app");
    let module = "module/Application/config/module.config.php";
    let controller = "module/Application/src/Controller/BookController.php";
    let goes = |site: &mut Site, file: &str, from: &str, to: &str| site.goes_to(file, from, to);
    // Routes: one, a child, the action of the route a URL is of.
    assert_eq!(goes(&mut site, controller, "toRoute('home')", "toRoute('ho/*|*/me')"), [format!("{module}:10")]);
    assert_eq!(
        goes(&mut site, controller, "toRoute('book/view'", "toRoute('book/vi/*|*/ew'"),
        [format!("{module}:30")]
    );
    assert_eq!(
        goes(&mut site, controller, "['action' => 'view', 'id' => 1]", "['action' => 'vi/*|*/ew', 'id' => 1]"),
        [format!("{controller}:24")]
    );
    // A template an action names, a partial, one a map names.
    assert_eq!(
        goes(
            &mut site,
            controller,
            "setTemplate('application/book/view')",
            "setTemplate('application/book/vi/*|*/ew')"
        ),
        ["module/Application/view/application/book/view.phtml:1"]
    );
    let index = "module/Application/view/application/book/index.phtml";
    assert_eq!(
        goes(&mut site, index, "partial('partial/book-row'", "partial('partial/book-/*|*/row'"),
        ["module/Application/view/partial/book-row.phtml:1"]
    );
    assert_eq!(
        goes(&mut site, module, "'not_found_template' => 'error/404'", "'not_found_template' => 'error/4/*|*/04'"),
        ["module/Application/view/error/404.phtml:1"]
    );
    // A service, a key of the configuration.
    let factory = "module/Application/src/Service/CatalogFactory.php";
    assert_eq!(goes(&mut site, factory, "get('clock')", "get('clo/*|*/ck')"), [format!("{module}:57")]);
    assert_eq!(
        goes(&mut site, factory, "['library']['name']", "['library']['na/*|*/me']"),
        ["config/autoload/global.php:5"]
    );
    // A key of a configuration held in a variable; a parameter of a route; a validator.
    assert_eq!(
        goes(&mut site, factory, "$config['library']", "$config['libr/*|*/ary']"),
        ["config/autoload/global.php:4"]
    );
    assert_eq!(
        goes(&mut site, controller, "['action' => 'view', 'id' => 1]", "['action' => 'view', 'i/*|*/d' => 1]"),
        [format!("{module}:30")]
    );
    let validator = goes(
        &mut site,
        "module/Application/src/Form/BookFilter.php",
        "['name' => 'StringLength'",
        "['name' => 'String/*|*/Length'",
    );
    assert!(validator.len() == 1 && validator[0].starts_with("lib/laminas.php:"), "{validator:?}");
    // An action of the controller beside it in the configuration.
    assert_eq!(
        goes(&mut site, module, "'action' => 'view',", "'action' => 'vi/*|*/ew',"),
        [format!("{controller}:24")]
    );
    // In a template: a route, a helper; in a controller, a plugin.
    assert_eq!(goes(&mut site, index, "url('book/view'", "url('book/vi/*|*/ew'"), [format!("{module}:30")]);
    let view = "module/Application/view/application/book/view.phtml";
    assert_eq!(
        goes(&mut site, view, "$this->stars(", "$this->sta/*|*/rs("),
        ["module/Application/src/View/Helper/Stars.php:7"]
    );
    assert_eq!(
        goes(&mut site, controller, "$this->shelf()", "$this->she/*|*/lf()"),
        ["module/Application/src/Controller/Plugin/Shelf.php:7"]
    );

    // Mezzio: a route where `config/routes.php` names it, a template of a namespace, a service.
    let mut site = Site::open("mezzio-app");
    let handler = "src/App/src/Handler/HomePageHandler.php";
    assert_eq!(goes(&mut site, handler, "generate('api.ping')", "generate('api.pi/*|*/ng')"), ["config/routes.php:9"]);
    assert_eq!(
        goes(&mut site, handler, "render('app::home-page'", "render('app::home-/*|*/page'"),
        ["src/App/templates/app/home-page.phtml:1"]
    );
    let factory = "src/App/src/Handler/HomePageHandlerFactory.php";
    assert_eq!(
        goes(&mut site, factory, "get('greeter')", "get('gree/*|*/ter')"),
        ["config/autoload/library.global.php:9"]
    );
    assert_eq!(
        goes(&mut site, "src/App/templates/app/home-page.phtml", "url('home')", "url('ho/*|*/me')"),
        ["config/routes.php:8"]
    );
}

#[test]
fn what_the_configuration_makes_and_an_action_gives_is_typed() {
    let mut site = Site::open("laminas-app");
    // A service: an alias of a class its factory makes, an invokable.
    let factory = "module/Application/src/Controller/BookControllerFactory.php";
    let found = site.offered(factory, "$container->get('catalog')", "$container->get('catalog')->/*|*/");
    has(&found, &["newest", "find", "title"]);
    let catalog = "module/Application/src/Service/CatalogFactory.php";
    let found = site.offered(catalog, "$container->get('clock')", "$container->get('clock')->/*|*/");
    assert_eq!(found, ["year"]);
    // A controller's plugins, Laminas's own among them.
    let controller = "module/Application/src/Controller/BookController.php";
    let found = site.offered(controller, "$this->shelf()", "$this->/*|*/shelf()");
    has(&found, &["shelf", "redirect", "url", "indexAction"]);
    let found =
        site.offered(controller, "$this->redirect()->toRoute('home')", "$this->redirect()->/*|*/toRoute('home')");
    assert_eq!(found, ["toRoute"]);
    // A template: what the action gives it, a partial's, and the renderer's helpers.
    let index = "module/Application/view/application/book/index.phtml";
    let found =
        site.offered(index, "<?php foreach ($books as $book): ?>", "<?php foreach ($books as $book): $book->/*|*/ ?>");
    has(&found, &["title", "rating", "id"]);
    let found = site.offered(index, "$this->partial(", "$this->/*|*/partial(");
    has(&found, &["partial", "url", "stars", "plugin"]);
    let view = "module/Application/view/application/book/view.phtml";
    let found = site.offered(view, "$book->title", "$book->/*|*/title");
    has(&found, &["title", "rating"]);
    let row = "module/Application/view/partial/book-row.phtml";
    let found = site.offered(row, "$book->title", "$book->/*|*/title");
    has(&found, &["title"]);

    // A service a closure makes; a child view model's and the layout's variables.
    let found = site.offered(catalog, "$container->get('clock')", "$container->get('year')->/*|*/");
    assert_eq!(found, ["year"]);
    for (template, variable, ty) in [
        ("module/Application/view/partial/related.phtml", "$count", "int"),
        ("module/Application/view/layout/layout.phtml", "$section", "string"),
    ] {
        let text = std::fs::read_to_string(site.root.join(template)).unwrap().replace("\r\n", "\n");
        let at = text.find(variable).expect("the variable");
        let line = text[..at].matches('\n').count() as u32;
        let col = text[..at].rsplit('\n').next().unwrap_or_default().len() as u32 + 2;
        let doc = site.document(template, &text);
        let hover = site.engine.hover(doc, Pos::new(line, col)).map(|hover| hover.markdown).unwrap_or_default();
        assert!(hover.contains(ty) && hover.contains(variable), "{template}: {hover}");
        site.engine.document_closed(doc);
    }

    // Mezzio: a service by its alias; what the handler passes its template.
    let mut site = Site::open("mezzio-app");
    let factory = "src/App/src/Handler/HomePageHandlerFactory.php";
    let found = site.offered(factory, "$container->get('greeter')", "$container->get('greeter')->/*|*/");
    assert_eq!(found, ["greet"]);
    let page = "src/App/templates/app/home-page.phtml";
    let doc = {
        let text = std::fs::read_to_string(site.root.join(page)).unwrap();
        site.document(page, &text)
    };
    let hover = site.engine.hover(doc, Pos::new(1, 8)).map(|hover| hover.markdown).unwrap_or_default();
    assert!(hover.contains("int") && hover.contains("$visits"), "{hover}");
    site.engine.document_closed(doc);
}
