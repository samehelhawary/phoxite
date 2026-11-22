//! Yii support on the fixture application (`fixtures/yii-app`), on a stand-in for
//! Yii's API.
//!
//! Every PHP file of the application is inspected with nothing to report (its views among
//! them, whose variables their controller passes), and one name of each kind resolves.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use phoxite::base::{CancellationToken, DocId, IdeEvent, Pos, SharedSink};
use phoxite::engine::{CACHE_SUBDIR, EngineConfig, PhpEngine};
use phoxite::frameworks::Framework;
use phoxite::project::LayoutConfig;
use phoxite::stubs::StubFilter;

const CARET: &str = "/*|*/";

/// The application's folders: not `lib`, the stand-in for Yii.
const FOLDERS: [&str; 9] =
    ["config", "components", "controllers", "mail", "migrations", "models", "modules", "views", "widgets"];

struct Site {
    engine: Arc<PhpEngine>,
    root: PathBuf,
    next: u32,
    _cache: tempfile::TempDir,
}

impl Site {
    fn open() -> Self {
        let root = dunce::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/yii-app")).unwrap();
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
    assert!(site.engine.frameworks().contains(Framework::Yii));
    let mut files = Vec::new();
    for folder in FOLDERS {
        php_files(&site.root.join(folder), &mut files);
    }
    files.sort();
    assert!(files.len() >= 20, "{files:?}");
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
    let controller = "controllers/SiteController.php";
    let goes = |site: &mut Site, file: &str, from: &str, to: &str| site.goes_to(file, from, to);
    // Views: a controller's, its layout, one from the application's views, a widget's, a mail view.
    assert_eq!(goes(&mut site, controller, "render('index'", "render('ind/*|*/ex'"), ["views/site/index.php:1"]);
    assert_eq!(goes(&mut site, controller, "$layout = 'main'", "$layout = 'ma/*|*/in'"), ["views/layouts/main.php:1"]);
    assert_eq!(
        goes(&mut site, "views/site/index.php", "'//book/_row'", "'//book/_r/*|*/ow'"),
        ["views/book/_row.php:1"]
    );
    assert_eq!(
        goes(&mut site, "widgets/Shelf.php", "render('shelf'", "render('sh/*|*/elf'"),
        ["widgets/views/shelf.php:1"]
    );
    assert_eq!(
        goes(&mut site, "models/ContactForm.php", "compose('welcome'", "compose('wel/*|*/come'"),
        ["mail/welcome.php:1"]
    );
    // Routes: another controller's action, this controller's, a module's, one in the configuration.
    assert_eq!(
        goes(&mut site, controller, "['book/view', 'id' => 1]", "['book/vi/*|*/ew', 'id' => 1]"),
        ["controllers/BookController.php:12"]
    );
    assert_eq!(
        goes(&mut site, controller, "redirect(['index'])", "redirect(['in/*|*/dex'])"),
        [format!("{controller}:13")]
    );
    assert_eq!(
        goes(&mut site, "controllers/BookController.php", "'/admin/report/index'", "'/admin/report/in/*|*/dex'"),
        ["modules/admin/controllers/ReportController.php:10"]
    );
    assert_eq!(
        goes(&mut site, "config/web.php", "'defaultRoute' => 'site/index'", "'defaultRoute' => 'site/in/*|*/dex'"),
        [format!("{controller}:13")]
    );
    // The configuration's names.
    assert_eq!(goes(&mut site, controller, "get('cache')", "get('ca/*|*/che')"), ["config/web.php:14"]);
    assert_eq!(goes(&mut site, controller, "Yii::$app->catalog;", "Yii::$app->cata/*|*/log;"), ["config/web.php:17"]);
    assert_eq!(
        goes(&mut site, "controllers/BookController.php", "getAlias('@covers')", "getAlias('@cov/*|*/ers')"),
        ["config/web.php:11"]
    );
    // A message and its category.
    assert_eq!(
        goes(&mut site, controller, "t('app', 'Newest books')", "t('app', 'Newest bo/*|*/oks')"),
        ["messages/de/app.php:4"]
    );
    assert_eq!(
        goes(&mut site, controller, "t('app', 'Newest books')", "t('a/*|*/pp', 'Newest books')"),
        ["messages/de/app.php:1"]
    );
    // A model's attributes, in its rules and where a method takes one.
    let form = "models/ContactForm.php";
    assert_eq!(goes(&mut site, form, "[['email', 'body']", "[['email', 'bo/*|*/dy']"), [format!("{form}:14")]);
    assert_eq!(goes(&mut site, form, "addError('body'", "addError('bo/*|*/dy'"), [format!("{form}:14")]);
    // In a view, of the model the view is passed.
    assert_eq!(
        goes(&mut site, "views/site/contact.php", "getAttributeLabel('email')", "getAttributeLabel('em/*|*/ail')"),
        [format!("{form}:11")]
    );
    // A parameter of the configuration.
    assert_eq!(
        goes(&mut site, "models/ContactForm.php", "params['adminEmail']", "params['admin/*|*/Email']"),
        ["config/params.php:4"]
    );
    // One below another.
    assert_eq!(
        goes(&mut site, "models/ContactForm.php", "params['mail']['from']", "params['mail']['fr/*|*/om']"),
        ["config/params.php:7"]
    );
    // A record's column, where its migration makes it.
    assert_eq!(
        goes(&mut site, "models/Book.php", "$this->pages", "$this->pa/*|*/ges"),
        ["migrations/m240101_000000_create_library.php:17"]
    );
}

#[test]
fn what_the_application_passes_and_configures_is_typed() {
    let mut site = Site::open();
    // A view's variable, from its controller: a list of books.
    let found = site.offered(
        "views/site/index.php",
        "<?php foreach ($books as $book): ?>",
        "<?php foreach ($books as $book): $book->/*|*/ ?>",
    );
    for name in ["title", "author", "label"] {
        assert!(found.contains(&name.to_owned()), "{name}: {found:?}");
    }
    // A relation of a model found by a query, in a view of the module.
    let found = site.offered(
        "modules/admin/views/report/index.php",
        "count($author->books)",
        "count($author->books[0]->/*|*/)",
    );
    assert!(found.contains(&"title".to_owned()), "{found:?}");
    // A component of the configuration, and what its getter reads.
    let found = site.offered("controllers/SiteController.php", "$catalog->newest()", "$catalog->/*|*/newest()");
    for name in ["newest", "perPage", "size"] {
        assert!(found.contains(&name.to_owned()), "{name}: {found:?}");
    }
    // A record's columns, from the migration: one it documents, one it doesn't, one added later.
    let found = site.offered("models/Book.php", "(int) $this->pages", "(int) $this->/*|*/pages");
    for name in ["title", "pages", "published_at"] {
        assert!(found.contains(&name.to_owned()), "{name}: {found:?}");
    }
}
