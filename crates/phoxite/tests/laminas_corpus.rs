//! Laminas support on Laminas itself: the fixture applications (`fixtures/laminas-app`,
//! `fixtures/mezzio-app`) without their stand-ins, on the real packages as include paths
//! (laminas-mvc 3.8 with laminas-view 2, laminas-servicemanager and laminas-router; mezzio
//! 3.29 with its router, templates, helpers, the laminas-view renderer and laminas-view
//! 3), and Laminas's MVC skeleton, which has no `vendor/` until Composer installs it.
//!
//! The engine's own tests use a small stand-in; this one checks that what they assume
//! holds for the real source: that the plugin managers' own plugins and helpers are
//! read, that services, plugins and helpers are typed, that the names resolve, and that
//! the "unknown name" inspections say nothing. What the other inspections say is counted
//! and printed: the packages the corpus doesn't have (the HTTP messages, the event
//! manager, the config aggregator) leave classes unknown.
//!
//! Ignored: it needs the corpora (`scripts/php-corpus.sh`, the names in each test).
//!
//! ```text
//! cargo test -p phoxite --release --test laminas_corpus -- --ignored --nocapture
//! ```
//!
//! Environment: `PHP_CORPUS_DIR` (default `../php-corpus` next to the workspace).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use phoxite::base::{CancellationToken, DocId, IdeEvent, Pos, SharedSink};
use phoxite::engine::{CACHE_SUBDIR, EngineConfig, PhpEngine};
use phoxite::frameworks::Framework;
use phoxite::frameworks::laminas::project::Manager;
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
    /// The project at `root`, with the `src` of the corpora `packages` as include paths.
    fn open(root: PathBuf, packages: &[&str], copy: Option<tempfile::TempDir>) -> Self {
        let cache = tempfile::tempdir().unwrap();
        let include: Vec<PathBuf> = packages.iter().map(|package| corpus(package).join("src")).collect();
        let config = EngineConfig {
            cache_dir: cache.path().join(CACHE_SUBDIR),
            layout: LayoutConfig { root: root.clone(), include_paths: include, ..LayoutConfig::default() },
            stubs: StubFilter::default(),
        };
        let sink: SharedSink = Arc::new(|_: IdeEvent| {});
        let started = Instant::now();
        let engine = PhpEngine::open(config, sink).unwrap();
        assert!(engine.wait_idle(Duration::from_secs(600)), "the corpus indexes");
        println!("indexed in {:?}", started.elapsed());
        Self { engine, root, next: 1, _cache: cache, _copy: copy }
    }

    /// The fixture application `name` without its stand-in, on the corpora `packages`.
    fn fixture(name: &str, packages: &[&str]) -> Self {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures").join(name);
        let tmp = tempfile::tempdir().unwrap();
        let root = dunce::canonicalize(tmp.path()).unwrap().join("app");
        copy(&fixture, &root);
        std::fs::remove_dir_all(root.join("lib")).unwrap();
        Self::open(root, packages, Some(tmp))
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

    /// The reports of Laminas's inspections on the PHP files of `folders`, and how many
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
                if problem.inspection.contains("Laminas") {
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

const MVC: [&str; 11] = [
    "laminas-mvc",
    "laminas-view2",
    "laminas-servicemanager",
    "laminas-router",
    "psr-container",
    "laminas-form",
    "laminas-validator",
    "laminas-filter",
    "laminas-hydrator",
    "laminas-inputfilter",
    "laminas-navigation",
];
const MEZZIO: [&str; 8] = [
    "mezzio-framework",
    "mezzio-router",
    "mezzio-template",
    "mezzio-helpers",
    "mezzio-laminasviewrenderer",
    "laminas-view",
    "laminas-servicemanager",
    "psr-container",
];

#[test]
#[ignore = "needs the corpora: scripts/php-corpus.sh laminas-mvc laminas-view2 laminas-servicemanager laminas-router psr-container"]
fn the_mvc_fixture_is_read_and_typed_on_laminas() {
    let mut site = Site::fixture("laminas-app", &MVC);
    assert!(site.engine.frameworks().contains(Framework::Laminas));
    let module = "module/Application/config/module.config.php";
    let controller = "module/Application/src/Controller/BookController.php";

    // Names: routes, templates, services, keys, an action.
    assert_eq!(site.goes_to(controller, "toRoute('book/view'", "toRoute('book/vi/*|*/ew'"), [module]);
    assert_eq!(
        site.goes_to(controller, "['action' => 'view', 'id' => 1]", "['action' => 'vi/*|*/ew', 'id' => 1]"),
        [controller]
    );
    assert_eq!(
        site.goes_to(controller, "setTemplate('application/book/view')", "setTemplate('application/book/vi/*|*/ew')"),
        ["module/Application/view/application/book/view.phtml"]
    );
    let factory = "module/Application/src/Service/CatalogFactory.php";
    assert_eq!(site.goes_to(factory, "get('clock')", "get('clo/*|*/ck')"), [module]);
    assert_eq!(
        site.goes_to(factory, "['library']['name']", "['library']['na/*|*/me']"),
        ["config/autoload/global.php"]
    );
    let index = "module/Application/view/application/book/index.phtml";
    assert_eq!(site.goes_to(index, "url('book/view'", "url('book/vi/*|*/ew'"), [module]);
    assert_eq!(
        site.goes_to(index, "partial('partial/book-row'", "partial('partial/book-/*|*/row'"),
        ["module/Application/view/partial/book-row.phtml"]
    );

    // Types: services, Laminas's own plugins and helpers on the real declarations.
    let found = site.offered(
        "module/Application/src/Controller/BookControllerFactory.php",
        "$container->get('catalog')",
        "$container->get('catalog')->/*|*/",
    );
    has(&found, &["newest", "find", "title"]);
    let found =
        site.offered(controller, "$this->redirect()->toRoute('home')", "$this->redirect()->/*|*/toRoute('home')");
    has(&found, &["toRoute", "toUrl", "refresh"]);
    let found = site.offered(controller, "$this->shelf()", "$this->/*|*/shelf()");
    has(&found, &["shelf", "redirect", "url", "params", "indexAction"]);
    let found =
        site.offered(index, "<?php foreach ($books as $book): ?>", "<?php foreach ($books as $book): $book->/*|*/ ?>");
    has(&found, &["title", "rating", "id"]);
    let found = site.offered(index, "$this->partial(", "$this->/*|*/partial(");
    has(&found, &["partial", "url", "stars", "escapeHtml", "headTitle", "plugin"]);
    let view = "module/Application/view/application/book/view.phtml";
    let found = site.offered(view, "<h1>", "<?php $this->plugin('url')->/*|*/ ?><h1>");
    has(&found, &["setRouter"]);

    // The other plugin managers' own services, read from the real classes.
    let project = site.engine.laminas();
    println!(
        "{} validators, {} filters, {} form elements, {} hydrators, {} input filters",
        project.validators.len(),
        project.filters.len(),
        project.forms.len(),
        project.hydrators.len(),
        project.input_filters.len()
    );
    for (manager, name, class) in [
        (Manager::Validators, "StringLength", "Laminas\\Validator\\StringLength"),
        (Manager::Filters, "StringTrim", "Laminas\\Filter\\StringTrim"),
        (Manager::Forms, "text", "Laminas\\Form\\Element\\Text"),
    ] {
        let found = project.resolved(manager, name).and_then(|service| service.target.clone());
        assert_eq!(found.as_deref(), Some(class), "{name}");
    }
    let filter = "module/Application/src/Form/BookFilter.php";
    let validator = site.goes_to(filter, "['name' => 'StringLength'", "['name' => 'String/*|*/Length'");
    assert!(validator.len() == 1 && validator[0].ends_with("StringLength.php"), "{validator:?}");
    // A configuration held in a variable, a route's parameter, a closure's service.
    assert_eq!(site.goes_to(factory, "$config['library']", "$config['libr/*|*/ary']"), ["config/autoload/global.php"]);
    assert_eq!(
        site.goes_to(controller, "['action' => 'view', 'id' => 1]", "['action' => 'view', 'i/*|*/d' => 1]"),
        [module]
    );
    let found = site.offered(factory, "$container->get('clock')", "$container->get('year')->/*|*/");
    assert_eq!(found, ["year"]);

    let (unknown, others, count) = site.reports(&["config", "module"]);
    println!("{count} files, {others} other problems");
    assert!(count >= 19, "{count}");
    assert!(unknown.is_empty(), "{}", unknown.join("\n"));
}

#[test]
#[ignore = "needs the corpora: scripts/php-corpus.sh mezzio-framework mezzio-router mezzio-template mezzio-helpers mezzio-laminasviewrenderer laminas-view laminas-servicemanager psr-container"]
fn the_mezzio_fixture_is_read_and_typed_on_mezzio() {
    let mut site = Site::fixture("mezzio-app", &MEZZIO);
    assert!(site.engine.frameworks().contains(Framework::Laminas));
    let handler = "src/App/src/Handler/HomePageHandler.php";
    assert_eq!(site.goes_to(handler, "generate('api.ping')", "generate('api.pi/*|*/ng')"), ["config/routes.php"]);
    assert_eq!(
        site.goes_to(handler, "render('app::home-page'", "render('app::home-/*|*/page'"),
        ["src/App/templates/app/home-page.phtml"]
    );
    let factory = "src/App/src/Handler/HomePageHandlerFactory.php";
    assert_eq!(site.goes_to(factory, "get('greeter')", "get('gree/*|*/ter')"), ["config/autoload/library.global.php"]);
    let found = site.offered(factory, "$container->get('greeter')", "$container->get('greeter')->/*|*/");
    assert_eq!(found, ["greet"]);
    // What the container makes of Mezzio's own classes.
    let found = site.offered(factory, "$container->get(UrlHelper::class)", "$container->get(UrlHelper::class)->/*|*/");
    has(&found, &["generate", "getRouteResult"]);
    let page = "src/App/templates/app/home-page.phtml";
    assert_eq!(site.goes_to(page, "url('home')", "url('ho/*|*/me')"), ["config/routes.php"]);
    let found = site.offered(page, "<?= $visits ?>", "<?= $visits ?><?php $this->/*|*/ ?>");
    has(&found, &["url", "partial", "escapeHtml"]);

    let (unknown, others, count) = site.reports(&["config", "src"]);
    println!("{count} files, {others} other problems");
    assert!(count >= 10, "{count}");
    assert!(unknown.is_empty(), "{}", unknown.join("\n"));
}

#[test]
#[ignore = "needs the corpora: scripts/php-corpus.sh laminas laminas-mvc laminas-view2 laminas-servicemanager laminas-router psr-container"]
fn laminass_mvc_skeleton_reports_no_unknown_name() {
    let mut site = Site::open(corpus("laminas"), &MVC, None);
    assert!(site.engine.frameworks().contains(Framework::Laminas));
    let project = site.engine.laminas();
    let routes: Vec<&str> = project.routes.iter().map(|route| route.name.as_str()).collect();
    assert_eq!(routes, ["home", "application"]);
    println!(
        "{} configuration keys, {} helpers, {} plugins",
        project.config.len(),
        project.helpers.len(),
        project.plugins.len()
    );
    let module = "module/Application/config/module.config.php";
    let controller = "module/Application/src/Controller/IndexController.php";
    assert_eq!(site.goes_to(module, "'action'     => 'index'", "'action'     => 'in/*|*/dex'"), [controller]);
    assert_eq!(
        site.goes_to(module, "'error/404'               =>", "'error/4/*|*/04'               =>"),
        ["module/Application/view/error/404.phtml"]
    );
    assert_eq!(
        site.goes_to(
            module,
            "'not_found_template'       => 'error/404'",
            "'not_found_template'       => 'error/4/*|*/04'"
        ),
        ["module/Application/view/error/404.phtml"]
    );
    // A template of the skeleton: `$this` is the renderer, with Laminas's helpers.
    let layout = "module/Application/view/layout/layout.phtml";
    let found = site.offered(layout, "$this->doctype()", "$this->/*|*/doctype()");
    has(&found, &["doctype", "headTitle", "url", "basePath", "partial"]);

    let (unknown, others, count) = site.reports(&["config", "module"]);
    println!("{count} files, {others} other problems");
    assert!(count >= 8, "{count}");
    assert!(unknown.is_empty(), "{}", unknown.join("\n"));
}
