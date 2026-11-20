//! Drupal support on the fixture site (`fixtures/drupal-app`): a module and a theme on a
//! stand-in for Drupal's API.
//!
//! Every PHP file of the module and the theme is inspected with nothing to report, and
//! one name of each kind resolves.

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
            dunce::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/drupal-app")).unwrap();
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
    assert!(site.engine.frameworks().contains(Framework::Drupal));
    let mut files = Vec::new();
    // Not the stand-in for Drupal: its methods have parameters and no code.
    php_files(&site.root.join("modules"), &mut files);
    php_files(&site.root.join("themes"), &mut files);
    files.sort();
    assert!(files.len() >= 10, "{files:?}");
    assert!(files.iter().any(|file| file.extension().is_some_and(|ext| ext == "module")), "shop.module is PHP");
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
    const SHOP: &str = "modules/shop";
    let controller = format!("{SHOP}/src/Controller/BookController.php");
    let controller = controller.as_str();
    let module = format!("{SHOP}/shop.module");
    let module = module.as_str();
    let mut site = Site::open();

    // The names of the YAML files.
    let found = site.goes_to(controller, "service('shop.catalog')", "service('shop.cat/*|*/alog')");
    assert_eq!(found, [format!("{SHOP}/shop.services.yml:2")]);
    let found = site.goes_to(controller, "'Books', 'shop.books'", "'Books', 'shop.bo/*|*/oks'");
    assert_eq!(found, [format!("{SHOP}/shop.routing.yml:1")]);
    let found = site.goes_to(controller, "redirect('shop.books')", "redirect('shop.bo/*|*/oks')");
    assert_eq!(found, [format!("{SHOP}/shop.routing.yml:1")]);
    let found = site.goes_to(controller, "hasPermission('administer shop')", "hasPermission('admin/*|*/ister shop')");
    assert_eq!(found, [format!("{SHOP}/shop.permissions.yml:1")]);
    let found = site.goes_to(controller, "[] = 'shop/base'", "[] = 'shop/ba/*|*/se'");
    assert_eq!(found, [format!("{SHOP}/shop.libraries.yml:6")]);
    let found = site.goes_to(controller, "config('shop.settings')", "config('shop.set/*|*/tings')");
    assert_eq!(found, [format!("{SHOP}/config/install/shop.settings.yml:1")]);
    // Usages count the YAML files.
    let found = site.usages(controller, "service('shop.catalog')", "service('shop.cat/*|*/alog')");
    assert_eq!(
        found,
        [format!("{SHOP}/shop.services.yml:2"), format!("{SHOP}/shop.services.yml:8"), format!("{controller}:21")]
    );
    let found = site.usages(controller, "'Books', 'shop.books'", "'Books', 'shop.bo/*|*/oks'");
    assert!(
        found.contains(&format!("{SHOP}/shop.links.menu.yml:3")) && found.contains(&format!("{module}:31")),
        "{found:?}"
    );

    // What PHP declares: entity types (an attribute, an annotation), a plugin, a theme hook and its template.
    let found =
        site.goes_to(&format!("{SHOP}/src/Catalog.php"), "getStorage('shop_book')", "getStorage('shop_/*|*/book')");
    assert_eq!(found, [format!("{SHOP}/src/Entity/Book.php:9")]);
    let found = site.goes_to(controller, "getStorage('shop_shelf')", "getStorage('shop_/*|*/shelf')");
    assert_eq!(found, [format!("{SHOP}/src/Entity/Shelf.php:11")]);
    let found = site.goes_to(controller, "createInstance('shop_cart')", "createInstance('shop_/*|*/cart')");
    assert_eq!(found, [format!("{SHOP}/src/Plugin/Block/CartBlock.php:8")]);
    let found = site.goes_to(controller, "'#theme' => 'shop_book'", "'#theme' => 'shop_/*|*/book'");
    assert_eq!(found, [format!("{module}:9"), format!("{SHOP}/templates/shop-book.html.twig:1")]);

    // Hooks: an attribute's by a placeholder, an alter hook, and the functions implementing one.
    let hooks = format!("{SHOP}/src/Hook/ShopHooks.php");
    let found = site.goes_to(&hooks, "Hook('shop_book_presave')", "Hook('shop_book_/*|*/presave')");
    assert_eq!(found, ["core/core.api.php:27"]);
    let found = site.goes_to(module, "alter('form'", "alter('fo/*|*/rm'");
    assert_eq!(found, ["core/core.api.php:37"]);
    let found = site.usages(&hooks, "invokeAll('entity_presave'", "invokeAll('entity_/*|*/presave'");
    assert!(found.contains(&format!("{module}:17")), "{found:?}");

    // Types: a service, a storage and what it loads, a hook's parameters.
    let found = site.goes_to(controller, "$catalog->books()", "$catalog->bo/*|*/oks()");
    assert_eq!(found, [format!("{SHOP}/src/Catalog.php:15")]);
    let found = site.goes_to(&format!("{SHOP}/src/Catalog.php"), "$book->title !== ''", "$book->ti/*|*/tle !== ''");
    assert_eq!(found, [format!("{SHOP}/src/Entity/Book.php:12")]);
    let found = site.goes_to(controller, "$shelf->room", "$shelf->ro/*|*/om");
    assert_eq!(found, [format!("{SHOP}/src/Entity/Shelf.php:17")]);
    let found = site.goes_to(module, "$form_state->setRedirect", "$form_state->set/*|*/Redirect");
    assert_eq!(found, ["core/lib/Drupal.php:122"]);
    let found = site.goes_to(&hooks, "$entity->label()", "$entity->la/*|*/bel()");
    assert_eq!(found, ["core/lib/Drupal.php:165"]);
    // What the remaining pieces add: a configuration's key, an implementation's hook, a preprocess function.
    let settings = format!("{SHOP}/config/install/shop.settings.yml");
    let found = site.goes_to(controller, "->get('currency')", "->get('curr/*|*/ency')");
    assert_eq!(found, [format!("{settings}:1")]);
    let found = site.goes_to(module, "->get('mail.from')", "->get('mail.fr/*|*/om')");
    assert_eq!(found, [format!("{settings}:3")]);
    let found = site.goes_to(module, "function shop_entity_presave", "function shop_entity_/*|*/presave");
    assert_eq!(found, ["core/core.api.php:17"]);
    let found = site.usages(controller, "'#theme' => 'shop_book'", "'#theme' => 'shop_/*|*/book'");
    assert!(found.contains(&format!("{module}:38")), "{found:?}");
    let variables = site.engine.twig_template_variables("@shop/shop-book.html.twig");
    assert!(variables.iter().any(|(name, _)| name == "book"), "{variables:?}");
}
