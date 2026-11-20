//! WordPress hooks, on a stand-in for WordPress's Plugin API written for
//! these tests (the functions' names and parameters, nothing of their code).

use super::*;
use crate::complete::Completions;

/// The Plugin API's functions.
const PLUGIN_API: &str = r"<?php
/** @param callable $callback */
function add_action($hook_name, $callback, $priority = 10, $accepted_args = 1) {}
/** @param callable $callback */
function add_filter($hook_name, $callback, $priority = 10, $accepted_args = 1) {}
/** @param callable $callback */
function remove_action($hook_name, $callback, $priority = 10) {}
function has_action($hook_name, $callback = false) {}
function do_action($hook_name, ...$arg) {}
function apply_filters($hook_name, $value, ...$args) {}
";

/// What WordPress fires.
const CORE: &str = r#"<?php
function wp_boot($post, $title, $kind)
{
    do_action('init');
    do_action("save_post_{$post->post_type}", $post->ID);
    do_action('manage_' . $kind . '_posts_columns');
    return apply_filters('the_title', $title, $post->ID);
}
"#;

const SHOP: &str = "wp-content/plugins/shop/shop.php";

/// A plugin's file.
const PLUGIN: &str = r"<?php
/* Plugin Name: Shop */

add_action('init', 'shop_boot');
add_action('save_post_book', 'shop_boot');
add_filter('the_title', 'shop_title');
add_action('manage_book_posts_columns', 'shop_boot');
do_action('shop_loaded', 1);
add_action('shop_loaded', 'shop_boot');
if (has_action('shop_loaded')) {
    remove_action('shop_loaded', 'shop_boot');
}
add_action('nothing_here', 'shop_boot');

function shop_boot() {}
function shop_title($title) { return $title; }
";

fn wordpress() -> Project {
    Project::new(&[
        ("wp-config.php", "<?php\n"),
        ("wp-includes/plugin.php", PLUGIN_API),
        ("wp-includes/load.php", CORE),
        (SHOP, PLUGIN),
    ])
}

fn at(from: &str, to: &str) -> String {
    assert!(PLUGIN.contains(from), "{from}");
    PLUGIN.replacen(from, to, 1)
}

impl Project {
    fn wp_complete(&mut self, source: &str) -> Completions {
        let (text, pos) = caret(source);
        let doc = self.open(SHOP, &text);
        let found = self.engine.completion(doc, pos, None, &CancellationToken::new()).expect("handled");
        self.engine.document_closed(doc);
        found
    }

    fn wp_usages(&mut self, source: &str) -> Vec<(String, u32, String)> {
        let (text, pos) = caret(source);
        let doc = self.open(SHOP, &text);
        let usages = self.engine.references(doc, pos, true, &CancellationToken::new()).expect("handled").unwrap();
        let found = usages.iter().map(|usage| self.describe(&usage.path, usage.range)).collect();
        self.engine.document_closed(doc);
        found
    }

    /// The edits of a rename, or why it is refused.
    fn wp_rename(&mut self, source: &str, new_name: &str) -> Result<Vec<(String, u32, String)>, String> {
        let (text, pos) = caret(source);
        let doc = self.open(SHOP, &text);
        let edit = self.engine.rename(doc, pos, new_name, &CancellationToken::new());
        self.engine.document_closed(doc);
        let edit = match edit {
            Some(Ok(edit)) => edit,
            Some(Err(error)) => return Err(error.to_string()),
            None => return Err("nothing to rename".to_owned()),
        };
        let mut found = Vec::new();
        for file in &edit.files {
            let crate::base::FileEdit::Edit { path, edits, .. } = file else { panic!("only text edits") };
            for edit in edits {
                assert_eq!(edit.new_text, new_name);
                found.push(self.describe(path, edit.range));
            }
        }
        found.sort();
        Ok(found)
    }

    fn wp_hover(&mut self, source: &str) -> Option<String> {
        let (text, pos) = caret(source);
        let doc = self.open(SHOP, &text);
        let hover = self.engine.hover(doc, pos);
        self.engine.document_closed(doc);
        hover.map(|hover| hover.markdown)
    }
}

#[test]
fn a_hook_goes_to_where_it_is_fired() {
    let mut project = wordpress();
    let found = project.definition(SHOP, &at("add_action('init'", "add_action('in/*|*/it'"));
    assert_eq!(found, Some(vec![target("wp-includes/load.php", 3, "init")]));
    let found = project.definition(SHOP, &at("add_filter('the_title'", "add_filter('the_/*|*/title'"));
    assert_eq!(found, Some(vec![target("wp-includes/load.php", 6, "the_title")]));
    // A hook the plugin fires itself, from a function that only asks about it.
    let found = project.definition(SHOP, &at("has_action('shop_loaded'", "has_action('shop_/*|*/loaded'"));
    assert_eq!(found, Some(vec![target(SHOP, 7, "shop_loaded")]));

    // A name put together at run time is found by its pattern: an interpolated string, a concatenation.
    let found = project.definition(SHOP, &at("'save_post_book'", "'save_post_/*|*/book'"));
    assert_eq!(found, Some(vec![target("wp-includes/load.php", 4, "save_post_{$post->post_type}")]));
    let found = project.definition(SHOP, &at("'manage_book_posts_columns'", "'manage_book_/*|*/posts_columns'"));
    assert_eq!(found, Some(vec![target("wp-includes/load.php", 5, "'manage_' . $kind . '_posts_columns'")]));

    let hover = project.wp_hover(&at("add_action('init'", "add_action('in/*|*/it'")).expect("a hover");
    assert!(hover.contains("**action** `init`") && hover.contains("wp-includes/load.php:4"), "{hover}");
    let hover = project.wp_hover(&at("'save_post_book'", "'save_post_/*|*/book'")).expect("a hover");
    assert!(hover.contains("fired as `save_post_*`"), "{hover}");
}

#[test]
fn hooks_complete_and_callbacks_are_callables() {
    let mut project = wordpress();
    let found = project.wp_complete(&at("add_action('nothing_here'", "add_action('/*|*/'"));
    let labels: Vec<&str> = found.items.iter().map(|item| item.label.as_str()).collect();
    for name in ["init", "the_title", "shop_loaded"] {
        assert!(labels.contains(&name), "{name} in {labels:?}");
    }
    assert!(!labels.iter().any(|label| label.contains('*')), "no pattern is offered: {labels:?}");
    let detail = |label: &str| found.items.iter().find(|item| item.label == label).and_then(|item| item.detail.clone());
    assert_eq!(detail("init").as_deref(), Some("action"));
    assert_eq!(detail("the_title").as_deref(), Some("filter"));

    // The callback is a string callable: it goes to the function, and the function's usages count it.
    let found = project
        .definition(SHOP, &at("add_filter('the_title', 'shop_title'", "add_filter('the_title', 'shop_/*|*/title'"));
    assert_eq!(found, Some(vec![target(SHOP, 15, "shop_title")]));
    let found = project.wp_usages(&at("function shop_title", "function shop_/*|*/title"));
    assert_eq!(found, [target(SHOP, 5, "shop_title"), target(SHOP, 15, "shop_title")]);
}

#[test]
fn the_usages_of_a_hook_are_the_calls_naming_it() {
    let mut project = wordpress();
    let found = project.wp_usages(&at("do_action('shop_loaded'", "do_action('shop_/*|*/loaded'"));
    assert_eq!(
        found,
        [
            target(SHOP, 7, "shop_loaded"),
            target(SHOP, 8, "shop_loaded"),
            target(SHOP, 9, "shop_loaded"),
            target(SHOP, 10, "shop_loaded"),
        ]
    );
    // From a listener, across files.
    let found = project.wp_usages(&at("add_action('init'", "add_action('in/*|*/it'"));
    assert_eq!(found, [target(SHOP, 3, "init"), target("wp-includes/load.php", 3, "init")]);
}

#[test]
fn a_hook_the_project_fires_is_renamed() {
    let mut project = wordpress();
    let found = project.wp_rename(&at("add_action('shop_loaded'", "add_action('shop_/*|*/loaded'"), "shop_ready");
    assert_eq!(
        found,
        Ok(vec![
            target(SHOP, 7, "shop_loaded"),
            target(SHOP, 8, "shop_loaded"),
            target(SHOP, 9, "shop_loaded"),
            target(SHOP, 10, "shop_loaded"),
        ])
    );
    // Not one of WordPress, one fired under a name made at run time, or one nothing fires.
    for (from, to) in [
        ("add_action('init'", "add_action('in/*|*/it'"),
        ("'save_post_book'", "'save_post_/*|*/book'"),
        ("'nothing_here'", "'nothing_/*|*/here'"),
    ] {
        let found = project.wp_rename(&at(from, to), "other");
        assert!(found.is_err(), "{to}: {found:?}");
    }
    // Nor one fired from a file beside WordPress's folders (`wp-settings.php` fires `init`).
    let mut beside = Project::new(&[
        ("wp-settings.php", "<?php\ndo_action('wp_loaded');\n"),
        ("wp-includes/plugin.php", PLUGIN_API),
        ("wp-admin/admin.php", "<?php\n"),
        (SHOP, "<?php\n"),
    ]);
    let found = beside.wp_rename("<?php\nadd_action('wp_/*|*/loaded', 'shop_boot');\n", "other");
    assert!(found.is_err(), "{found:?}");
    // Nor to a name that isn't one.
    let found = project.wp_rename(&at("add_action('shop_loaded'", "add_action('shop_/*|*/loaded'"), "shop ready");
    assert!(found.is_err(), "{found:?}");
}

/// A theme registering and naming one of everything.
const THEME: &str = r"<?php
function theme_setup()
{
    register_post_type('book', ['public' => true]);
    register_taxonomy('genre', 'book');
    add_shortcode('price', 'theme_price');
    wp_register_script('theme-app', 'app.js');
    wp_enqueue_style('theme-main', 'main.css');
    add_option('theme_color', 'blue');
    add_image_size('card', 300, 200);
    register_nav_menus(['primary' => 'Primary', 'footer' => 'Footer']);
}

function theme_use($post)
{
    $books = get_posts(['post_type' => 'book', 'numberposts' => 5]);
    $query = new WP_Query(['post_type' => 'book']);
    $genres = get_the_terms($post, 'genre');
    $terms = get_terms(['taxonomy' => 'genre']);
    if (shortcode_exists('price')) {}
    wp_enqueue_script('theme-app');
    wp_enqueue_script('theme-extra', 'extra.js', ['theme-app', 'jquery']);
    wp_enqueue_style('theme-main');
    $color = get_option('theme_color');
    the_post_thumbnail('card');
    wp_nav_menu(['theme_location' => 'primary']);
    $other = get_option('siteurl');
}
";

const FUNCTIONS: &str = "wp-content/themes/shelf/functions.php";

/// Declarations of what [`THEME`] calls, as WordPress has them.
const THEME_API: &str = r"<?php
function register_post_type($post_type, $args = []) {}
function register_taxonomy($taxonomy, $object_type, $args = []) {}
/** @param callable $callback */
function add_shortcode($tag, $callback) {}
function shortcode_exists($tag) {}
function wp_register_script($handle, $src, $deps = [], $ver = false, $args = []) {}
function wp_enqueue_script($handle, $src = '', $deps = [], $ver = false, $args = []) {}
function wp_enqueue_style($handle, $src = '', $deps = [], $ver = false, $media = 'all') {}
function add_option($option, $value = '') {}
function get_option($option, $default_value = false) {}
function add_image_size($name, $width = 0, $height = 0, $crop = false) {}
function the_post_thumbnail($size = 'post-thumbnail', $attr = '') {}
function register_nav_menus($locations = []) {}
function wp_nav_menu($args = []) {}
function get_posts($args = null) {}
function get_the_terms($post, $taxonomy) {}
function get_terms($args = [], $deprecated = '') {}
class WP_Query { public function __construct($query = '') {} }
";

#[test]
fn the_names_wordpress_registers_go_to_their_registration() {
    let mut project = Project::new(&[
        ("wp-config.php", "<?php\n"),
        ("wp-includes/api.php", THEME_API),
        ("wp-content/themes/shelf/style.css", "/*\nTheme Name: Shelf\n*/\n"),
        (FUNCTIONS, THEME),
    ]);
    let mut goes = |from: &str, to: &str, line: u32, text: &str| {
        assert!(THEME.contains(from), "{from}");
        let source = THEME.replacen(from, to, 1);
        let (source_text, pos) = caret(&source);
        let found = project.definition(FUNCTIONS, &source);
        assert_eq!(found, Some(vec![target(FUNCTIONS, line, text)]), "{to}");
        // And its hover says what it is.
        let doc = project.open(FUNCTIONS, &source_text);
        let hover = project.engine.hover(doc, pos).map(|hover| hover.markdown);
        project.engine.document_closed(doc);
        hover.unwrap_or_else(|| panic!("a hover at {to}"))
    };
    let hover = goes("get_posts(['post_type' => 'book'", "get_posts(['post_type' => 'bo/*|*/ok'", 3, "book");
    assert!(hover.contains("**post type** `book`") && hover.contains("Registered in"), "{hover}");
    goes("new WP_Query(['post_type' => 'book'", "new WP_Query(['post_type' => 'bo/*|*/ok'", 3, "book");
    // The post type a taxonomy is for, where the taxonomy is registered.
    goes("register_taxonomy('genre', 'book'", "register_taxonomy('genre', 'bo/*|*/ok'", 3, "book");
    goes("get_the_terms($post, 'genre'", "get_the_terms($post, 'gen/*|*/re'", 4, "genre");
    goes("get_terms(['taxonomy' => 'genre'", "get_terms(['taxonomy' => 'gen/*|*/re'", 4, "genre");
    goes("shortcode_exists('price'", "shortcode_exists('pri/*|*/ce'", 5, "price");
    // A script enqueued by its handle, and as another's dependency.
    goes("wp_enqueue_script('theme-app')", "wp_enqueue_script('theme-/*|*/app')", 6, "theme-app");
    goes("['theme-app', 'jquery']", "['theme-/*|*/app', 'jquery']", 6, "theme-app");
    // A style enqueued with its source is registered there.
    goes("wp_enqueue_style('theme-main')", "wp_enqueue_style('theme-/*|*/main')", 7, "theme-main");
    let hover = goes("get_option('theme_color'", "get_option('theme_/*|*/color'", 8, "theme_color");
    assert!(hover.contains("**option** `theme_color`") && hover.contains("Set in"), "{hover}");
    goes("the_post_thumbnail('card'", "the_post_thumbnail('ca/*|*/rd'", 9, "card");
    // A menu location is a key of the array registering it.
    goes("['theme_location' => 'primary'", "['theme_location' => 'pri/*|*/mary'", 10, "primary");

    // Completion offers what is registered, of the kind asked for.
    let source = THEME.replacen("get_option('siteurl')", "get_option('/*|*/')", 1);
    let (text, pos) = caret(&source);
    let doc = project.open(FUNCTIONS, &text);
    let found = project.engine.completion(doc, pos, None, &CancellationToken::new()).expect("handled");
    project.engine.document_closed(doc);
    let labels: Vec<&str> = found.items.iter().map(|item| item.label.as_str()).collect();
    assert!(labels.contains(&"theme_color") && !labels.contains(&"book"), "{labels:?}");

    // Usages are the calls naming it, and a rename changes them all.
    let source = THEME.replacen("register_post_type('book'", "register_post_type('bo/*|*/ok'", 1);
    let (text, pos) = caret(&source);
    let doc = project.open(FUNCTIONS, &text);
    let usages = project.engine.references(doc, pos, true, &CancellationToken::new()).expect("handled").unwrap();
    let lines: Vec<u32> = usages.iter().map(|usage| usage.range.start.line).collect();
    assert_eq!(lines, [3, 4, 15, 16]);
    let edit = project.engine.rename(doc, pos, "novel", &CancellationToken::new()).expect("handled").unwrap();
    project.engine.document_closed(doc);
    let edits: usize = edit
        .files
        .iter()
        .map(|file| match file {
            crate::base::FileEdit::Edit { edits, .. } => edits.len(),
            _ => 0,
        })
        .sum();
    assert_eq!(edits, 4);
}

#[test]
fn a_global_variable_has_the_type_its_global_tags_document() {
    let core = r"<?php
class wpdb { public $prefix = ''; public function prepare($query) {} }
class WP_Post { public $post_title = ''; }
class Legacy { public function old() {} }

/**
 * @global wpdb $wpdb WordPress database abstraction object.
 * @global WP_Post|null $post Global post object.
 */
function wp_first() {}

/**
 * @global wpdb $wpdb
 * @global Legacy $post
 */
function wp_second() {}

/** @global WP_Post|null $post */
function wp_third() {}
";
    let mut project = Project::new(&[("wp-config.php", "<?php\n"), ("wp-includes/core.php", core), (SHOP, "<?php\n")]);
    let mut members = |body: &str| {
        let source = format!("<?php\nclass Shop {{ public function open() {{}} }}\n{body}\n");
        let found = project.wp_complete(&source);
        found.items.iter().map(|item| item.label.clone()).collect::<Vec<_>>()
    };
    // In a function that documents nothing: what the project's tags say of the name.
    let found = members("function shop_rows() { global $wpdb; $wpdb->/*|*/ }");
    assert!(found.contains(&"prepare".to_owned()) && found.contains(&"prefix".to_owned()), "{found:?}");
    // The type most tags write, in a method too.
    let found = members("class Rows { public function all() { global $post, $wpdb; $post->/*|*/ } }");
    assert_eq!(found, ["post_title"]);
    // The function's own tag comes first.
    let found = members("/** @global Shop $post */\nfunction shop_own() { global $post; $post->/*|*/ }");
    assert_eq!(found, ["open"]);
    // Through `$GLOBALS`, and read at the top level of a file (a theme's template).
    let found = members("function shop_raw() { $GLOBALS['wpdb']->/*|*/ }");
    assert!(found.contains(&"prepare".to_owned()), "{found:?}");
    assert_eq!(members("echo $post->/*|*/;"), ["post_title"]);
    // Not a variable the file sets itself, nor one read in a function without `global`.
    assert_eq!(members("$post = new Shop();\n$post->/*|*/;"), ["open"]);
    assert!(members("function shop_local() { $post->/*|*/ }").is_empty());
    // A variable nothing documents is still unknown.
    let found = members("function shop_other() { global $nothing; $nothing->/*|*/ }");
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn a_hook_callback_gets_what_the_hook_passes() {
    let core = r#"<?php
class WP_Post { public $post_title = ''; }
class WP_Term { public $slug = ''; }
/** @param callable $callback */
function add_action($hook_name, $callback, $priority = 10, $accepted_args = 1) {}
/** @param callable $callback */
function add_filter($hook_name, $callback, $priority = 10, $accepted_args = 1) {}
function do_action($hook_name, ...$arg) {}
function apply_filters($hook_name, $value, ...$args) {}

function wp_save($post, $term, $update)
{
    /**
     * Fires once a post has been saved.
     *
     * @param int     $post_id Post ID.
     * @param WP_Post $post    Post object.
     * @param bool    $update  Whether this is an existing post being updated.
     */
    do_action('save_post', $post->ID, $post, $update);

    /**
     * Filters a term before it is shown.
     *
     * @param WP_Term $term The term.
     */
    $term = apply_filters("get_{$term->taxonomy}", $term);

    do_action('undocumented', $post);
    return $term;
}
"#;
    let mut project = Project::new(&[("wp-config.php", "<?php\n"), ("wp-includes/core.php", core), (SHOP, "<?php\n")]);
    let mut members = |body: &str| {
        let found = project.wp_complete(&format!("<?php\n{body}\n"));
        found.items.iter().map(|item| item.label.clone()).collect::<Vec<_>>()
    };
    // A closure's parameters, by position.
    assert_eq!(members("add_action('save_post', function ($id, $post) { $post->/*|*/ }, 10, 2);"), ["post_title"]);
    assert_eq!(members("add_action('save_post', fn ($id, $saved) => $saved->/*|*/);"), ["post_title"]);
    // A filter's value, of a hook fired under a name made at run time.
    assert_eq!(members("add_filter('get_genre', function ($term) { return $term->/*|*/; });"), ["slug"]);
    // The closure's own type comes first, and a hook nothing documents says nothing.
    let found = members("add_action('save_post', function ($id, WP_Term $post) { $post->/*|*/ });");
    assert_eq!(found, ["slug"]);
    assert!(members("add_action('undocumented', function ($post) { $post->/*|*/ });").is_empty());

    // A function and a method registered by name: their parameters, by position.
    let found =
        members("add_action('save_post', 'shop_saved', 10, 2);\nfunction shop_saved($id, $post) { $post->/*|*/ }");
    assert_eq!(found, ["post_title"]);
    let class = "class Shop_Hooks {\n    public function boot() {\n        add_action('save_post', [$this, 'saved'], 10, 2);\n        add_filter('get_genre', [self::class, 'term']);\n    }\n    public function saved($id, $post) { BODY }\n    public static function term($term) { TERM }\n}";
    assert_eq!(members(&class.replace("BODY", "$post->/*|*/").replace("TERM", "")), ["post_title"]);
    assert_eq!(members(&class.replace("TERM", "$term->/*|*/").replace("BODY", "")), ["slug"]);
    // Registered for two hooks: what either passes. A declared type still comes first.
    let found = members(
        "add_action('save_post', 'shop_both', 10, 2);\nadd_filter('get_genre', 'shop_both');\nfunction shop_both($first) { $first->/*|*/ }",
    );
    assert_eq!(found, ["slug"], "the first parameter of `save_post` is an int");
    let found = members(
        "add_action('save_post', 'shop_typed', 10, 2);\nfunction shop_typed($id, WP_Term $post) { $post->/*|*/ }",
    );
    assert_eq!(found, ["slug"]);
}

#[test]
fn a_callback_under_a_key_of_an_array_is_a_callable() {
    let api = "<?php\nfunction register_rest_route($route_namespace, $route, $args = [], $override = false) {}\n";
    let routes = r"<?php
class Shop_Routes
{
    public function register()
    {
        register_rest_route('shop/v1', '/books', [
            'methods' => 'GET',
            'callback' => [$this, 'books'],
            'permission_callback' => 'shop_can_read',
        ]);
        register_rest_route('shop/v1', '/book', [
            ['methods' => 'GET', 'callback' => [$this, 'books']],
            ['methods' => 'POST', 'callback' => 'Shop_Routes::create', 'args' => ['title' => 'books']],
        ]);
    }

    public function books() {}
    public static function create() {}
}

function shop_can_read() { return true; }
";
    let mut project = Project::new(&[("wp-config.php", "<?php\n"), ("wp-includes/rest.php", api), (SHOP, routes)]);
    let mut at = |from: &str, to: &str| {
        assert!(routes.contains(from), "{from}");
        project.definition(SHOP, &routes.replacen(from, to, 1))
    };
    assert_eq!(
        at("'callback' => [$this, 'books'],", "'callback' => [$this, 'bo/*|*/oks'],"),
        Some(vec![target(SHOP, 16, "books")])
    );
    assert_eq!(at("'shop_can_read',", "'shop_can_/*|*/read',"), Some(vec![target(SHOP, 20, "shop_can_read")]));
    // One endpoint among several, and a `Class::method` string.
    assert_eq!(at("'Shop_Routes::create'", "'Shop_Routes::cre/*|*/ate'"), Some(vec![target(SHOP, 17, "create")]));
    // Not a string under another key.
    let found = at("['title' => 'books']", "['title' => 'bo/*|*/oks']");
    assert!(found.as_ref().is_none_or(Vec::is_empty), "{found:?}");

    // The method's usages count them, and a rename changes them.
    let source = routes.replacen("public function books", "public function bo/*|*/oks", 1);
    let lines: Vec<u32> = project.wp_usages(&source).into_iter().map(|(_, line, _)| line).collect();
    assert_eq!(lines, [7, 11, 16]);
    let found = project.wp_rename(&source, "list_books").expect("renamed");
    assert_eq!(found.len(), 3, "{found:?}");
}

#[test]
fn a_template_part_goes_to_the_themes_file() {
    const INDEX: &str = "wp-content/themes/shelf/index.php";
    let index = r"<?php
get_template_part('template-parts/content', 'page');
get_template_part('template-parts/content', 'book');
get_template_part('template-parts/content');
get_template_part('template-parts/missing');
get_template_part('');
";
    let mut project = Project::new(&[
        ("wp-config.php", "<?php\n"),
        ("wp-includes/api.php", "<?php\nfunction get_template_part($slug, $name = null, $args = []) {}\n"),
        ("wp-content/themes/shelf/style.css", "/*\nTheme Name: Shelf\n*/\n"),
        ("wp-content/themes/shelf/template-parts/content.php", "<?php\n"),
        ("wp-content/themes/shelf/template-parts/content-page.php", "<?php\n"),
        ("wp-content/themes/shelf/footer.php", "<?php\n"),
        // Another theme's files aren't this one's.
        ("wp-content/themes/other/style.css", "/*\nTheme Name: Other\n*/\n"),
        ("wp-content/themes/other/template-parts/content-book.php", "<?php\n"),
        (INDEX, index),
    ]);
    let mut file = |from: &str, to: &str| {
        assert!(index.contains(from), "{from}");
        let found = project.definition(INDEX, &index.replacen(from, to, 1));
        found.unwrap_or_default().into_iter().map(|(file, ..)| file).collect::<Vec<_>>()
    };
    // The file of the slug and the name, else the slug's.
    assert_eq!(
        file("content', 'page'", "con/*|*/tent', 'page'"),
        ["wp-content/themes/shelf/template-parts/content-page.php"]
    );
    assert_eq!(
        file("content', 'book'", "con/*|*/tent', 'book'"),
        ["wp-content/themes/shelf/template-parts/content.php"]
    );
    assert_eq!(file("content');", "con/*|*/tent');"), ["wp-content/themes/shelf/template-parts/content.php"]);
    assert!(file("parts/missing'", "parts/mis/*|*/sing'").is_empty());

    let source = index.replacen("get_template_part('')", "get_template_part('/*|*/')", 1);
    let (text, pos) = caret(&source);
    let doc = project.open(INDEX, &text);
    let found = project.engine.completion(doc, pos, None, &CancellationToken::new()).expect("handled");
    project.engine.document_closed(doc);
    let labels: Vec<&str> = found.items.iter().map(|item| item.label.as_str()).collect();
    assert_eq!(labels, ["footer", "index", "template-parts/content", "template-parts/content-page"]);
}

#[test]
fn a_hook_nothing_fires_is_reported_when_asked_for() {
    let mut project = wordpress();
    let hooks = |project: &mut Project| {
        let doc = project.open(SHOP, PLUGIN);
        let found = project.engine.inspect(doc, &CancellationToken::new()).expect("open");
        project.engine.document_closed(doc);
        let reported = found.problems.iter().filter(|problem| problem.inspection == "PhpUndefinedHook");
        reported.map(|problem| (problem.range.start.line, problem.message.clone())).collect::<Vec<_>>()
    };
    // Off until turned on: a hook may be fired by a plugin that isn't here.
    assert_eq!(hooks(&mut project), []);
    let mut config = crate::inspect::InspectionConfig::default();
    config.on.insert("PhpUndefinedHook".to_owned());
    project.engine.configure_inspections(config);
    // Not the hooks WordPress fires, by name or by pattern, nor the plugin's own.
    assert_eq!(hooks(&mut project), [(12, "Hook 'nothing_here' is not fired anywhere".to_owned())]);
}

#[test]
fn hooks_are_only_read_in_a_wordpress_project() {
    let mut project = Project::new(&[("lib/plugin.php", PLUGIN_API), ("lib/load.php", CORE), (SHOP, "<?php\n")]);
    let source =
        PLUGIN.replace("/* Plugin Name: Shop */", "").replacen("add_action('init'", "add_action('in/*|*/it'", 1);
    let found = project.definition(SHOP, &source);
    assert!(found.as_ref().is_none_or(Vec::is_empty), "{found:?}");
    // The callback is a callable whatever the project is.
    let source = PLUGIN.replacen("'init', 'shop_boot'", "'init', 'shop_/*|*/boot'", 1);
    assert_eq!(project.definition(SHOP, &source), Some(vec![target(SHOP, 14, "shop_boot")]));
}

#[test]
fn each_item_of_a_list_of_names_is_a_name() {
    let api = r"<?php
function register_post_type($post_type, $args = []) {}
function register_taxonomy($taxonomy, $object_type, $args = []) {}
function is_singular($post_types = '') {}
function is_tax($taxonomy = '', $term = '') {}
function get_posts($args = null) {}
function get_terms($args = [], $deprecated = '') {}
class WP_Query { public function __construct($query = '') {} }
";
    let theme = r"<?php
register_post_type('book', ['taxonomies' => ['genre']]);
register_post_type('film');
register_taxonomy('genre', ['book', 'film']);
if (is_singular(['book', 'film']) || is_tax(['genre'])) {}
$found = get_posts(['post_type' => ['book', 'film'], 'post_status' => ['book']]);
$query = new WP_Query(['post_type' => ['film']]);
$terms = get_terms(['taxonomy' => ['genre'], 'fields' => 'ids']);
";
    let mut project = Project::new(&[
        ("wp-config.php", "<?php\n"),
        ("wp-includes/api.php", api),
        ("wp-content/themes/shelf/style.css", "/*\nTheme Name: Shelf\n*/\n"),
        (FUNCTIONS, theme),
    ]);
    let mut goes = |from: &str, to: &str| {
        assert!(theme.contains(from), "{from}");
        project.definition(FUNCTIONS, &theme.replacen(from, to, 1)).unwrap_or_default()
    };
    assert_eq!(
        goes("is_singular(['book', 'film']", "is_singular(['book', 'fi/*|*/lm']"),
        [target(FUNCTIONS, 2, "film")]
    );
    assert_eq!(goes("'genre', ['book', 'film']", "'genre', ['bo/*|*/ok', 'film']"), [target(FUNCTIONS, 1, "book")]);
    assert_eq!(goes("is_tax(['genre']", "is_tax(['gen/*|*/re']"), [target(FUNCTIONS, 3, "genre")]);
    // Under the key of a query's arguments, and of a post type's.
    assert_eq!(
        goes("'post_type' => ['book', 'film']", "'post_type' => ['bo/*|*/ok', 'film']"),
        [target(FUNCTIONS, 1, "book")]
    );
    assert_eq!(
        goes("WP_Query(['post_type' => ['film']", "WP_Query(['post_type' => ['fi/*|*/lm']"),
        [target(FUNCTIONS, 2, "film")]
    );
    assert_eq!(goes("'taxonomy' => ['genre']", "'taxonomy' => ['gen/*|*/re']"), [target(FUNCTIONS, 3, "genre")]);
    assert_eq!(goes("'taxonomies' => ['genre']", "'taxonomies' => ['gen/*|*/re']"), [target(FUNCTIONS, 3, "genre")]);
    // Not a list under another key.
    assert!(goes("'post_status' => ['book']", "'post_status' => ['bo/*|*/ok']").is_empty());

    // The usages of a post type count the items.
    let source = theme.replacen("register_post_type('film'", "register_post_type('fi/*|*/lm'", 1);
    let (text, pos) = caret(&source);
    let doc = project.open(FUNCTIONS, &text);
    let usages = project.engine.references(doc, pos, true, &CancellationToken::new()).expect("handled").unwrap();
    project.engine.document_closed(doc);
    let lines: Vec<u32> = usages.iter().map(|usage| usage.range.start.line).collect();
    assert_eq!(lines, [2, 3, 4, 5, 6]);
}

#[test]
fn a_block_goes_to_its_registration() {
    let api = r"<?php
function register_block_type($block_type, $args = []) {}
function has_block($block_name, $post = null) {}
function register_block_style($block_name, $style_properties) {}
";
    let plugin = r"<?php
register_block_type('shop/price', ['render_callback' => 'shop_price']);
register_block_type('build/blocks/price');
register_block_type(__DIR__ . '/build/stock');
if (has_block('shop/price')) {}
register_block_style('shop/price', ['name' => 'big']);
has_block('');
function shop_price() {}
";
    let mut project = Project::new(&[("wp-config.php", "<?php\n"), ("wp-includes/blocks.php", api), (SHOP, plugin)]);
    let found = project.definition(SHOP, &plugin.replacen("has_block('shop/price'", "has_block('shop/pr/*|*/ice'", 1));
    assert_eq!(found, Some(vec![target(SHOP, 1, "shop/price")]));
    let found = project.wp_usages(&plugin.replacen(
        "register_block_type('shop/price'",
        "register_block_type('shop/pr/*|*/ice'",
        1,
    ));
    assert_eq!(found.iter().map(|(_, line, _)| *line).collect::<Vec<_>>(), [1, 4, 5]);
    // The folder of a `block.json` isn't a name: only the block is offered.
    let found = project.wp_complete(&plugin.replacen("has_block('')", "has_block('/*|*/')", 1));
    let labels: Vec<&str> = found.items.iter().map(|item| item.label.as_str()).collect();
    assert_eq!(labels, ["shop/price"]);
    // The callback under `render_callback` is a callable.
    let found = project.definition(SHOP, &plugin.replacen("'shop_price']", "'shop_/*|*/price']", 1));
    assert_eq!(found, Some(vec![target(SHOP, 7, "shop_price")]));
}

#[test]
fn the_files_a_theme_names_are_gone_to() {
    const INDEX: &str = "wp-content/themes/shelf/index.php";
    let api = r"<?php
function get_header($name = null, $args = []) {}
function get_footer($name = null, $args = []) {}
function get_sidebar($name = null, $args = []) {}
function locate_template($template_names, $load = false, $load_once = true, $args = []) {}
function get_theme_file_uri($file = '') {}
";
    let index = r"<?php
get_header('shop');
get_header('missing');
get_footer('shop');
get_sidebar('shop');
locate_template(['inc/missing.php', 'inc/setup.php'], true);
locate_template('inc/setup.php');
$url = get_theme_file_uri('/assets/app.js');
$none = get_theme_file_uri('../other/style.css');
get_header('');
locate_template('');
";
    let mut project = Project::new(&[
        ("wp-config.php", "<?php\n"),
        ("wp-includes/api.php", api),
        ("wp-content/themes/shelf/style.css", "/*\nTheme Name: Shelf\n*/\n"),
        ("wp-content/themes/shelf/header.php", "<?php\n"),
        ("wp-content/themes/shelf/header-shop.php", "<?php\n"),
        ("wp-content/themes/shelf/footer.php", "<?php\n"),
        ("wp-content/themes/shelf/inc/setup.php", "<?php\n"),
        ("wp-content/themes/shelf/assets/app.js", "\n"),
        ("wp-content/themes/other/style.css", "/*\nTheme Name: Other\n*/\n"),
        (INDEX, index),
    ]);
    let mut file = |from: &str, to: &str| {
        assert!(index.contains(from), "{from}");
        let found = project.definition(INDEX, &index.replacen(from, to, 1));
        found.unwrap_or_default().into_iter().map(|(file, ..)| file).collect::<Vec<_>>()
    };
    // The named one, else the plain one; nothing when the theme has neither.
    assert_eq!(file("get_header('shop'", "get_header('sh/*|*/op'"), ["wp-content/themes/shelf/header-shop.php"]);
    assert_eq!(file("get_header('missing'", "get_header('mis/*|*/sing'"), ["wp-content/themes/shelf/header.php"]);
    assert_eq!(file("get_footer('shop'", "get_footer('sh/*|*/op'"), ["wp-content/themes/shelf/footer.php"]);
    assert!(file("get_sidebar('shop'", "get_sidebar('sh/*|*/op'").is_empty());
    // A path in the theme: one of a list, alone, with a leading slash.
    assert_eq!(
        file("'inc/setup.php'], true", "'inc/set/*|*/up.php'], true"),
        ["wp-content/themes/shelf/inc/setup.php"]
    );
    assert!(file("['inc/missing.php'", "['inc/mis/*|*/sing.php'").is_empty());
    assert_eq!(
        file("locate_template('inc/setup.php'", "locate_template('inc/set/*|*/up.php'"),
        ["wp-content/themes/shelf/inc/setup.php"]
    );
    assert_eq!(file("'/assets/app.js'", "'/assets/a/*|*/pp.js'"), ["wp-content/themes/shelf/assets/app.js"]);
    // Not out of the theme.
    assert!(file("'../other/style.css'", "'../other/st/*|*/yle.css'").is_empty());

    let mut offered = |from: &str, to: &str| {
        let source = index.replacen(from, to, 1);
        let (text, pos) = caret(&source);
        let doc = project.open(INDEX, &text);
        let found = project.engine.completion(doc, pos, None, &CancellationToken::new()).expect("handled");
        project.engine.document_closed(doc);
        found.items.into_iter().map(|item| item.label).collect::<Vec<_>>()
    };
    assert_eq!(offered("get_header('')", "get_header('/*|*/')"), ["shop"]);
    let found = offered("locate_template('')", "locate_template('/*|*/')");
    assert_eq!(
        found,
        ["assets/app.js", "footer.php", "header-shop.php", "header.php", "inc/setup.php", "index.php", "style.css"]
    );
}

#[test]
fn scripts_and_block_json_name_blocks_and_ajax_actions() {
    const BLOCK_JSON: &str = "wp-content/plugins/shop/blocks/stock/block.json";
    const SCRIPT: &str = "wp-content/plugins/shop/assets/shop.js";
    let api = r"<?php
/** @param callable $callback */
function add_action($hook_name, $callback, $priority = 10, $accepted_args = 1) {}
function do_action($hook_name, ...$arg) {}
function register_block_type($block_type, $args = []) {}
function has_block($block_name, $post = null) {}
function wp_ajax() { do_action('wp_ajax_' . $_REQUEST['action']); do_action('wp_ajax_nopriv_' . $_REQUEST['action']); }
";
    let plugin = r"<?php
register_block_type('shop/price');
register_block_type(__DIR__ . '/blocks/stock');
if (has_block('shop/stock') || has_block('shop/price')) {}
add_action('wp_ajax_shop_add', 'shop_add');
add_action('wp_ajax_nopriv_shop_add', 'shop_add');
has_block('');
function shop_add() {}
";
    let block = "{\n  \"apiVersion\": 3,\n  \"attributes\": { \"name\": { \"type\": \"string\" } },\n  \"name\": \"shop/stock\",\n  \"title\": \"Stock\"\n}\n";
    let script = "registerBlockType('shop/stock', {});\nregisterBlockType(\"shop/price\", {});\njQuery.post(ajaxurl, { action: 'shop_add', id: 1 });\nconst label = 'shop_add';\n";
    let mut project = Project::new(&[
        ("wp-config.php", "<?php\n"),
        ("wp-includes/api.php", api),
        (BLOCK_JSON, block),
        (SCRIPT, script),
        ("wp-content/plugins/shop/node_modules/lib/index.js", "registerBlockType('shop/stock', {});\n"),
        (SHOP, plugin),
    ]);
    let at = |from: &str, to: &str| {
        assert!(plugin.contains(from), "{from}");
        plugin.replacen(from, to, 1)
    };
    // A block a `block.json` declares: goto, completion beside the one PHP registers.
    let found = project.definition(SHOP, &at("has_block('shop/stock')", "has_block('shop/st/*|*/ock')"));
    assert_eq!(found, Some(vec![target(BLOCK_JSON, 3, "shop/stock")]));
    let found = project.wp_complete(&at("has_block('')", "has_block('/*|*/')"));
    let labels: Vec<&str> = found.items.iter().map(|item| item.label.as_str()).collect();
    assert_eq!(labels, ["shop/price", "shop/stock"]);

    // Its usages count the scripts (not those of `node_modules`), and a rename changes the file and them.
    let source = at("has_block('shop/stock')", "has_block('shop/st/*|*/ock')");
    let found = project.wp_usages(&source);
    assert_eq!(
        found,
        [target(SCRIPT, 0, "shop/stock"), target(BLOCK_JSON, 3, "shop/stock"), target(SHOP, 3, "shop/stock")]
    );
    let found = project.wp_rename(&source, "shop/in-stock").expect("renamed");
    assert_eq!(
        found,
        [target(SCRIPT, 0, "shop/stock"), target(BLOCK_JSON, 3, "shop/stock"), target(SHOP, 3, "shop/stock")]
    );
    let found = project.wp_usages(&at("register_block_type('shop/price')", "register_block_type('shop/pr/*|*/ice')"));
    assert_eq!(found, [target(SCRIPT, 1, "shop/price"), target(SHOP, 1, "shop/price"), target(SHOP, 3, "shop/price")]);

    // The hook of an AJAX request: the script's action is a usage, of either hook; not any string saying it.
    for hook in ["'wp_ajax_shop_add'", "'wp_ajax_nopriv_shop_add'"] {
        let source = at(hook, &hook.replacen("shop_add", "shop_/*|*/add", 1));
        let found = project.wp_usages(&source);
        let script: Vec<_> = found.iter().filter(|(file, ..)| file == SCRIPT).collect();
        assert_eq!(script, [&target(SCRIPT, 2, "shop_add")], "{hook}");
        // A rename is of the action: in both hooks and in the script.
        let found = project.wp_rename(&source, "shop_put").expect("renamed");
        assert_eq!(found, [target(SCRIPT, 2, "shop_add"), target(SHOP, 4, "shop_add"), target(SHOP, 5, "shop_add")]);
    }
}

#[test]
fn a_rest_route_is_named_by_its_path() {
    const SCRIPT: &str = "wp-content/plugins/shop/assets/shop.js";
    let api = r"<?php
function register_rest_route($route_namespace, $route, $args = [], $override = false) {}
function rest_url($path = '', $scheme = 'rest') {}
function rest_do_request($request) {}
class WP_REST_Request { public function __construct($method = '', $route = '', $attributes = []) {} }
";
    let plugin = r"<?php
register_rest_route('shop/v1', '/books', ['methods' => 'GET', 'callback' => 'shop_books']);
register_rest_route('shop/v1', '/books/(?P<id>\d+)', ['methods' => 'GET', 'callback' => 'shop_books']);
register_rest_route('shop/v1', '/books/(?P<id>[\d]+)/copies', ['callback' => 'shop_books']);
$url = rest_url('shop/v1/books');
$request = new WP_REST_Request('GET', '/shop/v1/books?per_page=5');
$one = rest_do_request('/shop/v1/books/12');
$copies = rest_url('shop/v1/books/12/copies');
$none = rest_url('shop/v1/films');
$typed = rest_do_request('');
function shop_books() {}
";
    let script =
        "apiFetch({ path: '/shop/v1/books' });\nfetch(root + 'shop/v1/books?page=2');\nfetch('/shop/v1/bookshelf');\n";
    let mut project =
        Project::new(&[("wp-config.php", "<?php\n"), ("wp-includes/api.php", api), (SCRIPT, script), (SHOP, plugin)]);
    let at = |from: &str, to: &str| {
        assert!(plugin.contains(from), "{from}");
        plugin.replacen(from, to, 1)
    };
    // With a slash before or a query after, and through a pattern.
    let found = project.definition(SHOP, &at("rest_url('shop/v1/books')", "rest_url('shop/v1/bo/*|*/oks')"));
    assert_eq!(found, Some(vec![target(SHOP, 1, "/books")]));
    let found = project.definition(SHOP, &at("'/shop/v1/books?per_page=5'", "'/shop/v1/bo/*|*/oks?per_page=5'"));
    assert_eq!(found, Some(vec![target(SHOP, 1, "/books")]));
    let found = project.definition(SHOP, &at("'/shop/v1/books/12'", "'/shop/v1/bo/*|*/oks/12'"));
    assert_eq!(found, Some(vec![target(SHOP, 2, r"/books/(?P<id>\d+)")]));
    let found = project.definition(SHOP, &at("'shop/v1/books/12/copies'", "'shop/v1/bo/*|*/oks/12/copies'"));
    assert_eq!(found, Some(vec![target(SHOP, 3, r"/books/(?P<id>[\d]+)/copies")]));
    let found = project.definition(SHOP, &at("rest_url('shop/v1/films')", "rest_url('shop/v1/fi/*|*/lms')"));
    assert!(found.as_ref().is_none_or(Vec::is_empty), "{found:?}");
    let hover = project.wp_hover(&at("'/shop/v1/books/12'", "'/shop/v1/bo/*|*/oks/12'")).expect("a hover");
    assert!(hover.contains("**REST route** `shop/v1/books/12`") && hover.contains("shop/v1/books/*"), "{hover}");

    // Completion offers the routes written out, with the slash a request needs.
    let found = project.wp_complete(&at("rest_do_request('')", "rest_do_request('/*|*/')"));
    let inserted: Vec<&str> = found.items.iter().map(|item| item.insert_text.as_str()).collect();
    assert_eq!(inserted, ["/shop/v1/books"]);

    // Usages, from a usage and from the route's string: PHP and the scripts.
    let all = [
        target(SCRIPT, 0, "shop/v1/books"),
        target(SCRIPT, 1, "shop/v1/books"),
        target(SHOP, 1, "/books"),
        target(SHOP, 4, "shop/v1/books"),
        target(SHOP, 5, "/shop/v1/books?per_page=5"),
    ];
    assert_eq!(project.wp_usages(&at("rest_url('shop/v1/books')", "rest_url('shop/v1/bo/*|*/oks')")), all);
    assert_eq!(project.wp_usages(&at("'shop/v1', '/books',", "'shop/v1', '/bo/*|*/oks',")), all);
    // A rename is of the path's last part: in the routes that say it, in PHP and in the scripts.
    let found = project.wp_rename(&at("rest_url('shop/v1/books')", "rest_url('shop/v1/bo/*|*/oks')"), "novels");
    assert_eq!(
        found,
        Ok(vec![
            target(SCRIPT, 0, "books"),
            target(SCRIPT, 1, "books"),
            target(SHOP, 1, "books"),
            target(SHOP, 2, "books"),
            target(SHOP, 3, "books"),
            target(SHOP, 4, "books"),
            target(SHOP, 5, "books"),
            target(SHOP, 6, "books"),
            target(SHOP, 7, "books"),
        ])
    );
    let found = project.wp_rename(&at("rest_url('shop/v1/books')", "rest_url('shop/v1/bo/*|*/oks')"), "my/novels");
    assert!(found.is_err(), "{found:?}");
}

#[test]
fn a_child_theme_has_its_parents_files() {
    const INDEX: &str = "wp-content/themes/shelf-child/index.php";
    let api = "<?php\nfunction get_template_part($slug, $name = null, $args = []) {}\nfunction get_header($name = null, $args = []) {}\nfunction locate_template($template_names, $load = false) {}\n";
    let index = r"<?php
get_header();
get_header('shop');
get_template_part('parts/card');
get_template_part('parts/row');
locate_template('inc/setup.php');
get_template_part('');
";
    let mut project = Project::new(&[
        ("wp-config.php", "<?php\n"),
        ("wp-includes/api.php", api),
        ("wp-content/themes/shelf/style.css", "/*\nTheme Name: Shelf\n*/\n"),
        ("wp-content/themes/shelf/header.php", "<?php\n"),
        ("wp-content/themes/shelf/header-shop.php", "<?php\n"),
        ("wp-content/themes/shelf/parts/card.php", "<?php\n"),
        ("wp-content/themes/shelf/parts/row.php", "<?php\n"),
        ("wp-content/themes/shelf/inc/setup.php", "<?php\n"),
        ("wp-content/themes/shelf-child/style.css", "/*\n Theme Name: Shelf Child\n Template: shelf\n*/\n"),
        ("wp-content/themes/shelf-child/header.php", "<?php\n"),
        ("wp-content/themes/shelf-child/parts/card.php", "<?php\n"),
        (INDEX, index),
    ]);
    let mut file = |from: &str, to: &str| {
        assert!(index.contains(from), "{from}");
        let found = project.definition(INDEX, &index.replacen(from, to, 1));
        found.unwrap_or_default().into_iter().map(|(file, ..)| file).collect::<Vec<_>>()
    };
    // The child's file when it has one, else the parent's.
    assert_eq!(file("'parts/card'", "'parts/ca/*|*/rd'"), ["wp-content/themes/shelf-child/parts/card.php"]);
    assert_eq!(file("'parts/row'", "'parts/r/*|*/ow'"), ["wp-content/themes/shelf/parts/row.php"]);
    assert_eq!(file("'inc/setup.php'", "'inc/set/*|*/up.php'"), ["wp-content/themes/shelf/inc/setup.php"]);
    // The parent's named header before the child's plain one.
    assert_eq!(file("get_header('shop')", "get_header('sh/*|*/op')"), ["wp-content/themes/shelf/header-shop.php"]);

    let source = index.replacen("get_template_part('')", "get_template_part('/*|*/')", 1);
    let (text, pos) = caret(&source);
    let doc = project.open(INDEX, &text);
    let found = project.engine.completion(doc, pos, None, &CancellationToken::new()).expect("handled");
    project.engine.document_closed(doc);
    let labels: Vec<&str> = found.items.iter().map(|item| item.label.as_str()).collect();
    assert_eq!(labels, ["header", "header-shop", "inc/setup", "index", "parts/card", "parts/row"]);
}

#[test]
fn a_key_named_as_a_callback_holds_a_callable() {
    let plugin = r"<?php
class Shop_Customizer
{
    public function add_setting($id, $args = []) {}

    public function register()
    {
        $this->add_setting('shop_color', ['default' => 'shop_clean', 'sanitize_callback' => 'shop_clean']);
        shop_field(['label' => 'Color', 'callback' => [$this, 'render'], 'meta_box_cb' => 'shop_clean']);
        $field = new Shop_Field(['render_callback' => [self::class, 'render']]);
    }

    public function render() {}
}

class Shop_Field { public function __construct($args = []) {} }
function shop_field($args) {}
function shop_clean($value) { return $value; }
";
    let mut project = Project::new(&[("wp-config.php", "<?php\n"), (SHOP, plugin)]);
    let mut at = |from: &str, to: &str| {
        assert!(plugin.contains(from), "{from}");
        project.definition(SHOP, &plugin.replacen(from, to, 1))
    };
    // In a method call, a function call and a `new`.
    assert_eq!(
        at("'sanitize_callback' => 'shop_clean'", "'sanitize_callback' => 'shop_/*|*/clean'"),
        Some(vec![target(SHOP, 17, "shop_clean")])
    );
    assert_eq!(at("[$this, 'render'], 'meta", "[$this, 'ren/*|*/der'], 'meta"), Some(vec![target(SHOP, 12, "render")]));
    assert_eq!(at("[self::class, 'render']", "[self::class, 'ren/*|*/der']"), Some(vec![target(SHOP, 12, "render")]));
    // Not under a key that says nothing.
    let found = at("'default' => 'shop_clean'", "'default' => 'shop_/*|*/clean'");
    assert!(found.as_ref().is_none_or(Vec::is_empty), "{found:?}");

    // The function's usages count them.
    let source = plugin.replacen("function shop_clean", "function shop_/*|*/clean", 1);
    let lines: Vec<u32> = project.wp_usages(&source).into_iter().map(|(_, line, _)| line).collect();
    assert_eq!(lines, [7, 8, 17]);
}

#[test]
fn a_rest_route_is_read_through_the_properties_of_its_controller() {
    let api = r"<?php
function register_rest_route($route_namespace, $route, $args = [], $override = false) {}
function rest_url($path = '', $scheme = 'rest') {}
";
    let controllers = r"<?php
class Shop_Books_Controller
{
    protected $namespace = 'shop/v1';
    protected $rest_base;

    public function __construct()
    {
        $this->rest_base = 'books';
    }

    public function register_routes()
    {
        register_rest_route($this->namespace, '/' . $this->rest_base, []);
        register_rest_route($this->namespace, '/' . $this->rest_base . '/(?P<id>[d]+)', []);
        register_rest_route($this->namespace, '/' . $this->kind, []);
        register_rest_route($this->other, '/films', []);
    }
}
";
    let plugin = "<?php
$a = rest_url('shop/v1/books');
$b = rest_url('shop/v1/books/7');
$c = rest_url('shop/v1/films');
$d = rest_url('');
";
    let mut project = Project::new(&[
        (
            "wp-config.php",
            "<?php
",
        ),
        ("wp-includes/api.php", api),
        ("wp-content/plugins/shop/controllers.php", controllers),
        (SHOP, plugin),
    ]);
    let mut lines = |from: &str, to: &str| {
        let found = project.definition(SHOP, &plugin.replacen(from, to, 1)).unwrap_or_default();
        found.into_iter().map(|(_, line, _)| line).collect::<Vec<_>>()
    };
    assert_eq!(lines("'shop/v1/books')", "'shop/v1/bo/*|*/oks')"), [13]);
    assert_eq!(lines("'shop/v1/books/7'", "'shop/v1/bo/*|*/oks/7'"), [14]);
    // A base that isn't known is anything under the namespace; a namespace that isn't known is no route.
    assert_eq!(lines("'shop/v1/films'", "'shop/v1/fi/*|*/lms'"), [15]);
    let found = project.wp_complete(&plugin.replacen("rest_url('')", "rest_url('/*|*/')", 1));
    let labels: Vec<&str> = found.items.iter().map(|item| item.label.as_str()).collect();
    assert_eq!(labels, ["shop/v1/books"]);
}

#[test]
fn a_folder_given_to_register_block_type_is_not_a_block() {
    let api = "<?php
function register_block_type($block_type, $args = []) {}
function has_block($block_name) {}
";
    let plugin = "<?php
register_block_type('build/price');
register_block_type('shop/note');
has_block('');
";
    let mut project = Project::new(&[
        (
            "wp-config.php",
            "<?php
",
        ),
        ("wp-includes/api.php", api),
        (
            "wp-content/plugins/shop/build/price/block.json",
            "{ \"name\": \"shop/price\" }
",
        ),
        (SHOP, plugin),
    ]);
    let found = project.wp_complete(&plugin.replacen("has_block('')", "has_block('/*|*/')", 1));
    let labels: Vec<&str> = found.items.iter().map(|item| item.label.as_str()).collect();
    assert_eq!(labels, ["shop/note", "shop/price"]);
}

#[test]
fn a_script_and_a_block_json_are_answered_in() {
    const SCRIPT: &str = "wp-content/plugins/shop/assets/shop.js";
    const BLOCK_JSON: &str = "wp-content/plugins/shop/blocks/stock/block.json";
    let api = r"<?php
/** @param callable $callback */
function add_action($hook_name, $callback, $priority = 10, $accepted_args = 1) {}
function register_block_type($block_type, $args = []) {}
function register_rest_route($route_namespace, $route, $args = [], $override = false) {}
";
    let plugin = r"<?php
register_block_type('shop/price');
register_rest_route('shop/v1', '/books', []);
add_action('wp_ajax_shop_add', 'shop_add');
add_action('wp_ajax_nopriv_shop_add', 'shop_add');
function shop_add() {}
";
    let script = "registerBlockType('shop/price', {});\njQuery.post(ajaxurl, { action: 'shop_add' });\napiFetch({ path: '/shop/v1/books?page=2' });\nconst label = 'shop_add';\njQuery.post(ajaxurl, { action: '' });\napiFetch({ path: '' });\nregisterBlockType('', {});\n";
    let block =
        "{\n  \"name\": \"shop/stock\",\n  \"parent\": [\"shop/price\"],\n  \"render\": \"file:./render.php\"\n}\n";
    let project = Project::new(&[
        ("wp-config.php", "<?php\n"),
        ("wp-includes/api.php", api),
        (SCRIPT, script),
        (BLOCK_JSON, block),
        ("wp-content/plugins/shop/blocks/stock/render.php", "<?php\n"),
        (SHOP, plugin),
    ]);
    let root = project.engine.root();
    let in_text = |text: &str, needle: &str| {
        let at = text.find(needle).unwrap_or_else(|| panic!("{needle}")) + 1;
        let line = text[..at].matches('\n').count() as u32;
        Pos::new(line, text[..at].rsplit('\n').next().unwrap().len() as u32)
    };
    let goes = |file: &str, text: &str, needle: &str| {
        let found = project.engine.wordpress_script_definition(&root.join(file), text, in_text(text, needle));
        found.unwrap_or_default().iter().map(|target| project.describe(&target.path, target.range)).collect::<Vec<_>>()
    };
    // A block, a route with a slash before and a query after, and the calls listening to an action.
    assert_eq!(goes(SCRIPT, script, "shop/price"), [target(SHOP, 1, "shop/price")]);
    assert_eq!(goes(SCRIPT, script, "/shop/v1/books"), [target(SHOP, 2, "/books")]);
    assert_eq!(
        goes(SCRIPT, script, "shop_add' }"),
        [target(SHOP, 3, "wp_ajax_shop_add"), target(SHOP, 4, "wp_ajax_nopriv_shop_add")]
    );
    // Not a string that only says the same.
    assert!(goes(SCRIPT, script, "shop_add';").is_empty());
    // In a `block.json`: another block, and a file.
    assert_eq!(goes(BLOCK_JSON, block, "shop/price"), [target(SHOP, 1, "shop/price")]);
    let found = goes(BLOCK_JSON, block, "file:./render.php");
    assert_eq!(
        found.iter().map(|(file, ..)| file.as_str()).collect::<Vec<_>>(),
        ["wp-content/plugins/shop/blocks/stock/render.php"]
    );

    let hover = |needle: &str| {
        let found = project.engine.wordpress_script_hover(&root.join(SCRIPT), script, in_text(script, needle));
        found.map(|hover| hover.markdown).unwrap_or_default()
    };
    assert!(hover("shop/price").contains("**block** `shop/price`"));
    assert!(hover("/shop/v1/books").contains("**REST route** `shop/v1/books`"));
    assert!(hover("shop_add' }").contains("**AJAX action** `shop_add`"), "{}", hover("shop_add' }"));

    // Usages, and a rename from the script: the action in both hooks and here.
    let cancel = CancellationToken::new();
    let at = in_text(script, "shop/price");
    let found = project.engine.wordpress_script_references(&root.join(SCRIPT), script, at, true, &cancel);
    let found: Vec<_> =
        found.unwrap().unwrap().iter().map(|usage| project.describe(&usage.path, usage.range)).collect();
    assert_eq!(
        found,
        [target(SCRIPT, 0, "shop/price"), target(BLOCK_JSON, 2, "shop/price"), target(SHOP, 1, "shop/price")]
    );
    let at = in_text(script, "shop_add' }");
    let edit = project.engine.wordpress_script_rename(&root.join(SCRIPT), script, at, "shop_put", &cancel);
    let mut edits = Vec::new();
    for file in &edit.unwrap().unwrap().files {
        let crate::base::FileEdit::Edit { path, edits: found, .. } = file else { panic!("only text edits") };
        edits.extend(found.iter().map(|edit| project.describe(path, edit.range)));
    }
    edits.sort();
    assert_eq!(edits, [target(SCRIPT, 1, "shop_add"), target(SHOP, 3, "shop_add"), target(SHOP, 4, "shop_add")]);

    // Completion by what the line says before the string.
    let offered = |needle: &str| {
        let found = project.engine.wordpress_script_completion(&root.join(SCRIPT), script, {
            let at = script.find(needle).unwrap() + needle.len() - 1;
            let line = script[..at].matches('\n').count() as u32;
            Pos::new(line, script[..at].rsplit('\n').next().unwrap().len() as u32)
        });
        found.map(|found| found.into_iter().map(|item| item.label).collect::<Vec<_>>())
    };
    assert_eq!(offered("action: ''"), Some(vec!["shop_add".to_owned()]));
    assert_eq!(offered("path: ''"), Some(vec!["/shop/v1/books".to_owned()]));
    assert_eq!(offered("registerBlockType(''"), Some(vec!["shop/price".to_owned(), "shop/stock".to_owned()]));
    assert_eq!(offered("label = 'shop_add'"), None);
}
