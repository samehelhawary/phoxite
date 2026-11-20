//! WordPress support on WordPress itself: the `wordpress` corpus opened with the engine,
//! and a plugin's file written against it.
//!
//! The engine's own tests use a small stand-in for WordPress's API; this one checks that
//! what they assume holds for the real source: that its hooks are found where it fires
//! them, that its docblocks type a hook's callback and its global variables, and that
//! the names it registers are read.
//!
//! Ignored: it needs the corpus (`scripts/php-corpus.sh wordpress`).
//!
//! ```text
//! cargo test -p phoxite --test wordpress_corpus -- --ignored --nocapture
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
use phoxite::summary::DeclaredKind;

const CARET: &str = "/*|*/";

fn corpus() -> PathBuf {
    std::env::var_os("PHP_CORPUS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../php-corpus"))
        .join("wordpress")
}

struct Site {
    engine: Arc<PhpEngine>,
    root: PathBuf,
    next: u32,
    _cache: tempfile::TempDir,
}

impl Site {
    /// A plugin's file with `body`, opened as a document: where its caret is.
    fn open(&mut self, body: &str) -> (DocId, Pos) {
        let source = format!("<?php\n{body}\n");
        let at = source.find(CARET).expect("a caret");
        let text = format!("{}{}", &source[..at], &source[at + CARET.len()..]);
        let line = text[..at].matches('\n').count() as u32;
        let col = text[..at].rsplit('\n').next().unwrap_or_default().encode_utf16().count() as u32;
        let doc = DocId(self.next);
        self.next += 1;
        self.engine.document_opened(doc, &self.root.join("wp-content/plugins/phoxite-probe/probe.php"), 1, &text);
        (doc, Pos::new(line, col))
    }

    /// The files goto at the caret of `body` lands in, relative to WordPress.
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
fn wordpress_itself() {
    let root = dunce::canonicalize(corpus()).expect("the wordpress corpus (scripts/php-corpus.sh wordpress)");
    let cache = tempfile::tempdir().unwrap();
    let config = EngineConfig {
        cache_dir: cache.path().join(CACHE_SUBDIR),
        layout: LayoutConfig { root: root.clone(), ..LayoutConfig::default() },
        stubs: StubFilter::default(),
    };
    let sink: SharedSink = Arc::new(|_: IdeEvent| {});
    let started = Instant::now();
    let engine = PhpEngine::open(config, sink).unwrap();
    assert!(engine.wait_idle(Duration::from_secs(600)), "WordPress indexes");
    println!("indexed in {:?}", started.elapsed());
    let mut site = Site { engine, root, next: 1, _cache: cache };

    assert!(site.engine.frameworks().contains(Framework::WordPress), "detected from its own files");

    // What it fires and registers.
    let started = Instant::now();
    let names = site.engine.names();
    println!("names read in {:?}", started.elapsed());
    let count = |kinds: &[DeclaredKind]| names.of(kinds).count();
    let hooks = count(&[DeclaredKind::Action, DeclaredKind::Filter]);
    let dynamic = names.of(&[DeclaredKind::Action, DeclaredKind::Filter]).filter(|hook| hook.dynamic).count();
    println!(
        "{hooks} firing calls ({dynamic} under a name made at run time), {} post types, {} taxonomies, {} scripts, {} styles, {} shortcodes, {} options, {} image sizes",
        count(&[DeclaredKind::PostType]),
        count(&[DeclaredKind::Taxonomy]),
        count(&[DeclaredKind::Script]),
        count(&[DeclaredKind::Style]),
        count(&[DeclaredKind::Shortcode]),
        count(&[DeclaredKind::Option]),
        count(&[DeclaredKind::ImageSize]),
    );
    assert!(hooks > 2000, "{hooks} firing calls");
    assert!(dynamic > 100, "{dynamic} dynamic hooks");
    for (kind, name) in [
        (DeclaredKind::Action, "init"),
        (DeclaredKind::Action, "save_post"),
        (DeclaredKind::Filter, "the_content"),
        (DeclaredKind::PostType, "page"),
        (DeclaredKind::Taxonomy, "category"),
        (DeclaredKind::Shortcode, "gallery"),
    ] {
        assert!(!names.named(&[kind], name).is_empty(), "{kind:?} {name}");
    }
    let saved = names.named(&[DeclaredKind::Action], "save_post_book");
    assert!(saved.iter().any(|hook| hook.dynamic && hook.name == "save_post_*"), "{saved:?}");

    // A hook goes to where WordPress fires it, by name and by pattern; a post type to its registration.
    assert_eq!(site.goes_to("add_action('in/*|*/it', 'probe_boot');"), ["wp-settings.php"]);
    assert!(
        site.goes_to("add_action('save_post_/*|*/book', 'probe_boot');").contains(&"wp-includes/post.php".to_owned())
    );
    assert_eq!(site.goes_to("$q = new WP_Query(['post_type' => 'pa/*|*/ge']);"), ["wp-includes/post.php"]);
    // One of a list of names.
    assert_eq!(site.goes_to("if (is_singular(['post', 'pa/*|*/ge'])) {}"), ["wp-includes/post.php"]);
    // A REST route of WordPress, whose controller keeps the namespace and the base in properties.
    let routes = count(&[DeclaredKind::RestRoute]);
    println!("{routes} REST routes");
    let found = site.goes_to("$url = rest_url('wp/v2/us/*|*/ers');");
    assert!(found.iter().any(|file| file.ends_with("class-wp-rest-users-controller.php")), "{found:?}");
    // The callback is a callable, from the `@param callable` of `add_action()`'s docblock.
    let found = site.goes_to("add_action('init', 'probe_/*|*/boot');\nfunction probe_boot() {}");
    assert_eq!(found, ["wp-content/plugins/phoxite-probe/probe.php"]);
    // WordPress can't be renamed from a plugin.
    let (doc, pos) = site.open("add_action('in/*|*/it', 'probe_boot');");
    let renamed = site.engine.rename(doc, pos, "started", &CancellationToken::new());
    site.engine.document_closed(doc);
    assert!(!matches!(renamed, Some(Ok(_))), "a hook of WordPress isn't renamed");

    let has = |found: &[String], label: &str| found.iter().any(|known| known == label);
    // Completion offers the hooks, not the patterns.
    let found = site.completes("add_action('/*|*/', 'probe_boot');");
    assert!(has(&found, "init") && has(&found, "wp_enqueue_scripts"), "{} items", found.len());
    assert!(!found.iter().any(|label| label.contains('*')));

    // A callback gets what the hook's docblock says it passes: a closure, and a function by name.
    let found = site.completes("add_action('save_post', function ($id, $post) { $post->/*|*/ }, 10, 2);");
    assert!(has(&found, "post_title"), "a WP_Post: {found:?}");
    let found = site
        .completes("add_action('save_post', 'probe_saved', 10, 2);\nfunction probe_saved($id, $post) { $post->/*|*/ }");
    assert!(has(&found, "post_title"), "a WP_Post: {found:?}");
    let found = site
        .completes("add_filter('get_terms', function ($terms, $taxonomies, $args, $query) { $query->/*|*/ }, 10, 4);");
    assert!(has(&found, "query_vars"), "a WP_Term_Query: {found:?}");

    // Global variables have the types WordPress documents them with.
    let found = site.completes("function probe_rows() { global $wpdb; $wpdb->/*|*/ }");
    assert!(has(&found, "prepare") && has(&found, "prefix"), "{found:?}");
    let found = site.completes("function probe_query() { global $wp_query; $wp_query->/*|*/ }");
    assert!(has(&found, "have_posts"), "{found:?}");
    let found = site.completes("function probe_post() { global $post; $post->/*|*/ }");
    assert!(has(&found, "post_title"), "{found:?}");
    let found = site.completes("echo $wp_query->/*|*/;");
    assert!(has(&found, "have_posts"), "at the top level of a file: {found:?}");
}
