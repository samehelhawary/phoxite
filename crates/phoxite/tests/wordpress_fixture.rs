//! WordPress support on the fixture site (`fixtures/wordpress-app`): a plugin and a theme
//! on a stand-in for WordPress's API.
//!
//! Every file of the plugin and the theme is inspected with nothing to report, the
//! inspection of hooks nothing fires included, and one name of each kind resolves.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use phoxite::base::{CancellationToken, DocId, IdeEvent, Pos, SharedSink};
use phoxite::engine::{CACHE_SUBDIR, EngineConfig, PhpEngine};
use phoxite::frameworks::Framework;
use phoxite::inspect::InspectionConfig;
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
            dunce::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/wordpress-app")).unwrap();
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
        } else if path.extension().is_some_and(|ext| ext == "php") {
            out.push(path);
        }
    }
}

#[test]
fn nothing_in_the_fixture_is_reported() {
    let mut site = Site::open();
    assert!(site.engine.frameworks().contains(Framework::WordPress));
    let mut config = InspectionConfig::default();
    config.on.insert("PhpUndefinedHook".to_owned());
    site.engine.configure_inspections(config);

    let mut files = Vec::new();
    // Not the stand-in for WordPress: its functions have parameters and no code.
    php_files(&site.root.join("wp-content"), &mut files);
    files.sort();
    assert!(files.len() >= 8, "{files:?}");
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
    const SHOP: &str = "wp-content/plugins/shop/shop.php";
    const CATALOG: &str = "wp-content/plugins/shop/includes/class-shop-catalog.php";
    const FUNCTIONS: &str = "wp-content/themes/shelf/functions.php";
    const INDEX: &str = "wp-content/themes/shelf/index.php";
    let mut site = Site::open();

    // Hooks: one WordPress fires beside its folders, one by a pattern, the plugin's own.
    assert_eq!(site.goes_to(SHOP, "add_action('init'", "add_action('in/*|*/it'"), ["wp-settings.php:8"]);
    assert_eq!(site.goes_to(SHOP, "'save_post_book'", "'save_post_/*|*/book'"), ["wp-includes/api.php:250"]);
    assert_eq!(
        site.goes_to(CATALOG, "has_action('shop_loaded'", "has_action('shop_/*|*/loaded'"),
        [format!("{SHOP}:34")]
    );
    // Callbacks: a function, a method of a class named by `::class`, one under a key.
    assert_eq!(site.goes_to(SHOP, "'init', 'shop_register'", "'init', 'shop_/*|*/register'"), [format!("{SHOP}:16")]);
    assert_eq!(
        site.goes_to(SHOP, "[Shop_Catalog::class, 'refresh']", "[Shop_Catalog::class, 'ref/*|*/resh']"),
        [format!("{CATALOG}:12")]
    );
    assert_eq!(
        site.goes_to(SHOP, "'permission_callback' => 'shop_public'", "'permission_callback' => 'shop_/*|*/public'"),
        [format!("{SHOP}:49")]
    );
    // The registries.
    assert_eq!(
        site.goes_to(CATALOG, "'post_type' => ['book']", "'post_type' => ['bo/*|*/ok']"),
        [format!("{SHOP}:18")]
    );
    assert_eq!(site.goes_to(INDEX, "is_singular(['book'])", "is_singular(['bo/*|*/ok'])"), [format!("{SHOP}:18")]);
    assert_eq!(
        site.goes_to(INDEX, "get_the_terms($post, 'genre')", "get_the_terms($post, 'gen/*|*/re')"),
        [format!("{SHOP}:19")]
    );
    assert_eq!(
        site.goes_to(CATALOG, "shortcode_exists('price')", "shortcode_exists('pri/*|*/ce')"),
        [format!("{SHOP}:20")]
    );
    assert_eq!(
        site.goes_to(SHOP, "get_option('shop_currency')", "get_option('shop_/*|*/currency')"),
        [format!("{SHOP}:21")]
    );
    assert_eq!(site.goes_to(SHOP, "has_block('shop/price')", "has_block('shop/pr/*|*/ice')"), [format!("{SHOP}:22")]);
    assert_eq!(
        site.goes_to(FUNCTIONS, "wp_enqueue_script('shelf-app')", "wp_enqueue_script('shelf-/*|*/app')"),
        [format!("{FUNCTIONS}:9")]
    );
    assert_eq!(
        site.goes_to(INDEX, "the_post_thumbnail('card')", "the_post_thumbnail('ca/*|*/rd')"),
        [format!("{FUNCTIONS}:7")]
    );
    assert_eq!(
        site.goes_to(INDEX, "'theme_location' => 'primary'", "'theme_location' => 'pri/*|*/mary'"),
        [format!("{FUNCTIONS}:8")]
    );
    // A block of a `block.json`, a REST route, and their usages in a script.
    let plugin = "wp-content/plugins/shop";
    let found = site.goes_to(SHOP, "has_block('shop/stock')", "has_block('shop/st/*|*/ock')");
    assert_eq!(found, [format!("{plugin}/blocks/stock/block.json:3")]);
    let found = site.goes_to(SHOP, "rest_url('shop/v1/books')", "rest_url('shop/v1/bo/*|*/oks')");
    assert_eq!(found, [format!("{SHOP}:23")]);
    let script = format!("{plugin}/assets/shop.js");
    let found = site.usages(SHOP, "has_block('shop/stock')", "has_block('shop/st/*|*/ock')");
    assert!(found.contains(&format!("{script}:2")), "{found:?}");
    let found = site.usages(SHOP, "'wp_ajax_shop_add'", "'wp_ajax_shop_/*|*/add'");
    assert!(found.contains(&format!("{script}:4")), "{found:?}");
    let found = site.usages(SHOP, "rest_url('shop/v1/books')", "rest_url('shop/v1/bo/*|*/oks')");
    assert!(found.contains(&format!("{script}:6")), "{found:?}");
    // The theme's files.
    let theme = "wp-content/themes/shelf";
    assert_eq!(
        site.goes_to(INDEX, "'template-parts/content', 'book'", "'template-parts/con/*|*/tent', 'book'"),
        [format!("{theme}/template-parts/content-book.php:1")]
    );
    assert_eq!(
        site.goes_to(INDEX, "get_header('shop')", "get_header('sh/*|*/op')"),
        [format!("{theme}/header-shop.php:1")]
    );
    assert_eq!(
        site.goes_to(FUNCTIONS, "get_theme_file_uri('/assets/app.js'))", "get_theme_file_uri('/assets/a/*|*/pp.js'))"),
        [format!("{theme}/assets/app.js:1")]
    );
    assert_eq!(
        site.goes_to(FUNCTIONS, "'template-parts/content.php']", "'template-parts/con/*|*/tent.php']"),
        [format!("{theme}/template-parts/content.php:1")]
    );
    // Types from docblocks: a named callback's parameter, a global in a template.
    assert_eq!(site.goes_to(SHOP, "$post->post_title);", "$post->post_/*|*/title);"), ["wp-includes/api.php:13"]);
    assert_eq!(site.goes_to(INDEX, "$post->post_title", "$post->post_/*|*/title"), ["wp-includes/api.php:13"]);
    assert_eq!(site.goes_to(CATALOG, "$wpdb->get_results", "$wpdb->get_/*|*/results"), ["wp-includes/api.php:50"]);
}
