//! Yii support on Yii itself: the `yii` corpus (Yii's basic application template) opened
//! with the engine, the `yii-framework` corpus (the framework) as an include path, since
//! the template has no `vendor/` until Composer installs it.
//!
//! The engine's own tests use a small stand-in for Yii's API; this one checks that what
//! they assume holds for the real source: that the configuration is read, that the
//! components and Active Record are typed, that the names resolve, and that the "unknown
//! name" inspections say nothing of the template. What the other inspections say is
//! printed to be read: the template's extensions (Bootstrap, the mailer, the debugger)
//! aren't in the corpus, so their classes are unknown.
//!
//! Ignored: it needs the corpora (`scripts/php-corpus.sh yii yii-framework`).
//!
//! ```text
//! cargo test -p phoxite --release --test yii_corpus -- --ignored --nocapture
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
    std::env::var_os("PHP_CORPUS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../php-corpus"))
        .join(name)
}

struct Site {
    engine: Arc<PhpEngine>,
    root: PathBuf,
    next: u32,
    _cache: tempfile::TempDir,
}

impl Site {
    fn open() -> Self {
        Self::open_corpus("yii")
    }

    fn open_corpus(name: &str) -> Self {
        let root = dunce::canonicalize(corpus(name)).expect("the corpus (scripts/php-corpus.sh)");
        let framework =
            dunce::canonicalize(corpus("yii-framework")).expect("the yii-framework corpus (scripts/php-corpus.sh)");
        let cache = tempfile::tempdir().unwrap();
        let config = EngineConfig {
            cache_dir: cache.path().join(CACHE_SUBDIR),
            layout: LayoutConfig { root: root.clone(), include_paths: vec![framework], ..LayoutConfig::default() },
            stubs: StubFilter::default(),
        };
        let sink: SharedSink = Arc::new(|_: IdeEvent| {});
        let started = Instant::now();
        let engine = PhpEngine::open(config, sink).unwrap();
        assert!(engine.wait_idle(Duration::from_secs(600)), "the corpus indexes");
        println!("indexed in {:?}", started.elapsed());
        Self { engine, root, next: 1, _cache: cache }
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

    /// The files goto lands in from the caret, relative to the template.
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

    fn offered(&mut self, file: &str, from: &str, to: &str) -> Vec<String> {
        let (doc, pos) = self.at(file, from, to);
        let found = self.engine.completion(doc, pos, None, &CancellationToken::new()).expect("handled");
        self.engine.document_closed(doc);
        found.items.into_iter().map(|item| item.label).collect()
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

#[test]
#[ignore = "needs the corpora: scripts/php-corpus.sh yii yii-framework"]
fn yiis_basic_template_is_read_typed_and_reports_no_unknown_name() {
    let mut site = Site::open();
    assert!(site.engine.frameworks().contains(Framework::Yii));

    // The configuration.
    let project = site.engine.yii();
    let components: Vec<&str> = project.components.iter().map(|def| def.name.as_str()).collect();
    println!("components: {components:?}");
    println!("modules: {:?}", project.modules.iter().map(|def| def.name.as_str()).collect::<Vec<_>>());
    println!("params: {:?}", project.params.iter().map(|def| def.name.as_str()).collect::<Vec<_>>());
    for name in ["request", "cache", "user", "mailer", "db", "urlManager"] {
        assert!(components.contains(&name), "{name}: {components:?}");
    }
    assert_eq!(project.identity.as_deref(), Some("app\\models\\User"));
    assert!(project.component("cache").and_then(|def| def.detail.as_deref()) == Some("yii\\caching\\FileCache"));
    assert!(project.params.iter().any(|def| def.name == "adminEmail"));

    // Names.
    let controller = "controllers/SiteController.php";
    assert_eq!(site.goes_to(controller, "render('index')", "render('ind/*|*/ex')"), ["views/site/index.php"]);
    assert_eq!(site.goes_to(controller, "render('about')", "render('ab/*|*/out')"), ["views/site/about.php"]);
    let routes = site.offered("views/layouts/_header.php", "['/site/index']", "['/*|*/']");
    println!("routes: {routes:?}");
    for route in ["site/index", "site/login", "site/contact", "site/about", "site/logout"] {
        assert!(routes.contains(&route.to_owned()), "{route}: {routes:?}");
    }
    assert_eq!(site.goes_to("views/layouts/_header.php", "['/site/about']", "['/site/ab/*|*/out']"), [controller]);

    // Types: a component of the configuration, the identity, a query's result.
    let found = site.offered(controller, "Yii::$app->user->logout();", "Yii::$app->cache->/*|*/;");
    assert!(found.contains(&"flush".to_owned()) && found.contains(&"getOrSet".to_owned()), "{found:?}");
    let found = site.offered(controller, "Yii::$app->user->logout();", "Yii::$app->user->identity->/*|*/;");
    assert!(found.contains(&"username".to_owned()) && found.contains(&"getAuthKey".to_owned()), "{found:?}");
    let found = site.offered(controller, "Yii::$app->user->logout();", "Yii::$app->db->/*|*/;");
    assert!(found.contains(&"createCommand".to_owned()), "{found:?}");

    // What the inspections say of the template.
    let mut files = Vec::new();
    for folder in ["commands", "config", "controllers", "mail", "models", "views", "widgets"] {
        php_files(&site.root.join(folder), &mut files);
    }
    files.sort();
    assert!(files.len() >= 20, "{files:?}");
    let mut unknown = Vec::new();
    let mut others = 0;
    for path in files {
        let text = std::fs::read_to_string(&path).unwrap();
        let file = path.strip_prefix(&site.root).unwrap().to_string_lossy().replace('\\', "/");
        let doc = site.document(&file, &text);
        let found = site.engine.inspect(doc, &CancellationToken::new()).expect("open");
        for problem in &found.problems {
            let line = format!("{file}:{} {} {}", problem.range.start.line + 1, problem.inspection, problem.message);
            if problem.inspection.contains("Yii") {
                unknown.push(line);
            } else {
                others += 1;
                println!("{line}");
            }
        }
        site.engine.document_closed(doc);
    }
    println!("{others} other problems");
    assert!(unknown.is_empty(), "{}", unknown.join("\n"));
}

/// The inspections' reports on the PHP files of `folders`, tests left out: those of the
/// Yii inspections, and how many others.
fn reports(site: &mut Site, folders: &[&str]) -> (Vec<String>, usize, usize) {
    let mut files = Vec::new();
    for folder in folders {
        php_files(&site.root.join(folder), &mut files);
    }
    files.retain(|path| !path.components().any(|part| part.as_os_str() == "tests"));
    files.sort();
    let mut unknown = Vec::new();
    let mut others = 0;
    let count = files.len();
    for path in files {
        let text = std::fs::read_to_string(&path).unwrap();
        let file = path.strip_prefix(&site.root).unwrap().to_string_lossy().replace('\\', "/");
        let doc = site.document(&file, &text);
        let found = site.engine.inspect(doc, &CancellationToken::new()).expect("open");
        for problem in &found.problems {
            if problem.inspection.contains("Yii") {
                unknown.push(format!(
                    "{file}:{} {} {}",
                    problem.range.start.line + 1,
                    problem.inspection,
                    problem.message
                ));
            } else {
                others += 1;
            }
        }
        site.engine.document_closed(doc);
    }
    (unknown, others, count)
}

#[test]
#[ignore = "needs the corpora: scripts/php-corpus.sh yii-advanced yii-framework"]
fn yiis_advanced_template_has_an_application_per_folder() {
    let mut site = Site::open_corpus("yii-advanced");
    assert!(site.engine.frameworks().contains(Framework::Yii));
    let project = site.engine.yii();
    let apps: Vec<String> = project
        .apps
        .iter()
        .map(|app| app.strip_prefix(&site.root).unwrap().to_string_lossy().replace('\\', "/"))
        .collect();
    assert_eq!(apps, ["backend", "common", "console", "frontend"]);
    println!("namespaces: {:?}", project.controller_namespaces);
    for namespace in ["backend\\controllers", "frontend\\controllers"] {
        assert!(project.controller_namespaces.iter().any(|known| known == namespace), "{namespace}");
    }
    assert_eq!(project.identity.as_deref(), Some("common\\models\\User"));
    let params: Vec<&str> = project.params.iter().map(|def| def.name.as_str()).collect();
    println!("params: {params:?}");
    assert!(params.contains(&"adminEmail") && params.contains(&"supportEmail"), "{params:?}");
    // The aliases its bootstrap file sets.
    let frontend = project.alias_path("@frontend/views").expect("an alias");
    assert_eq!(frontend, site.root.join("frontend").join("views"));

    // Each application's views and routes are its own.
    let controller = "frontend/controllers/SiteController.php";
    assert_eq!(site.goes_to(controller, "render('index')", "render('ind/*|*/ex')"), ["frontend/views/site/index.php"]);
    let backend = "backend/controllers/SiteController.php";
    assert_eq!(site.goes_to(backend, "render('index')", "render('ind/*|*/ex')"), ["backend/views/site/index.php"]);
    assert_eq!(
        site.goes_to(controller, "params['adminEmail']", "params['admin/*|*/Email']"),
        // The application's own, not the other applications'.
        ["frontend/config/params.php"]
    );
    let found = site.offered(controller, "Yii::$app->user->logout();", "Yii::$app->user->identity->/*|*/;");
    assert!(found.contains(&"username".to_owned()) && found.contains(&"getAuthKey".to_owned()), "{found:?}");

    // The mail views are the common application's.
    assert_eq!(
        site.goes_to("frontend/models/SignupForm.php", "'emailVerify-html'", "'emailVerify-/*|*/html'"),
        ["common/mail/emailVerify-html.php"]
    );
    // The table the console's migrations make, a later migration's column with it.
    let tables = site.engine.yii_tables();
    let user = tables.iter().find(|table| table.name == "user").expect("the user table");
    let columns: Vec<&str> = user.columns.iter().map(|column| column.name.as_str()).collect();
    println!("user: {columns:?}");
    for name in ["id", "username", "status", "verification_token"] {
        assert!(columns.contains(&name), "{name}: {columns:?}");
    }
    let resolver = site.engine.resolver();
    let table = phoxite::frameworks::yii::migrations::table_of(&resolver, "common\\models\\User");
    assert_eq!(table.as_deref(), Some("user"));
    drop(resolver);

    let (unknown, others, count) = reports(&mut site, &["backend", "common", "console", "frontend"]);
    println!("{count} files, {others} other problems");
    assert!(count >= 40, "{count}");
    assert!(unknown.is_empty(), "{}", unknown.join("\n"));
}
