//! Drupal support on Drupal itself: the `drupal` corpus opened with the engine, and a
//! module's file written against it.
//!
//! The engine's own tests use a small stand-in for Drupal's API; this one checks that what
//! they assume holds for the real source: that its YAML files are found and read, that its
//! attributes and annotations declare entity types and plugins, and that its `*.api.php`
//! files type a hook's implementation.
//!
//! Ignored: it needs the corpus (`scripts/php-corpus.sh drupal`).
//!
//! ```text
//! cargo test -p phoxite --release --test drupal_corpus -- --ignored --nocapture
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

fn corpus(name: &str) -> PathBuf {
    std::env::var_os("PHP_CORPUS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../php-corpus"))
        .join(name)
}

/// The corpus `name` opened and indexed.
fn open(name: &str) -> Site {
    let root = dunce::canonicalize(corpus(name))
        .unwrap_or_else(|_| panic!("the {name} corpus (scripts/php-corpus.sh {name})"));
    let cache = tempfile::tempdir().unwrap();
    let config = EngineConfig {
        cache_dir: cache.path().join(CACHE_SUBDIR),
        layout: LayoutConfig { root: root.clone(), ..LayoutConfig::default() },
        stubs: StubFilter::default(),
    };
    let sink: SharedSink = Arc::new(|_: IdeEvent| {});
    let started = Instant::now();
    let engine = PhpEngine::open(config, sink).unwrap();
    assert!(engine.wait_idle(Duration::from_secs(900)), "{name} indexes");
    println!("{name} indexed in {:?}", started.elapsed());
    let site = Site { engine, root, next: 1, _cache: cache };
    assert!(site.engine.frameworks().contains(Framework::Drupal), "detected from its own files");
    site
}

struct Site {
    engine: Arc<PhpEngine>,
    root: PathBuf,
    next: u32,
    _cache: tempfile::TempDir,
}

impl Site {
    /// A module's file with `body`, opened as a document: where its caret is.
    fn open(&mut self, body: &str) -> (DocId, Pos) {
        let source = format!("<?php\n{body}\n");
        let at = source.find(CARET).expect("a caret");
        let text = format!("{}{}", &source[..at], &source[at + CARET.len()..]);
        let line = text[..at].matches('\n').count() as u32;
        let col = text[..at].rsplit('\n').next().unwrap_or_default().encode_utf16().count() as u32;
        let doc = DocId(self.next);
        self.next += 1;
        self.engine.document_opened(doc, &self.root.join("modules/phoxite_probe/phoxite_probe.module"), 1, &text);
        (doc, Pos::new(line, col))
    }

    /// The files goto at the caret of `body` lands in, relative to Drupal.
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
fn drupal_itself() {
    let mut site = open("drupal");

    // What its YAML files declare.
    let started = Instant::now();
    let model = site.engine.drupal();
    println!("YAML files read in {:?}", started.elapsed());
    println!(
        "{} files: {} services, {} routes, {} permissions, {} libraries, {} configurations, {} extensions",
        model.files.len(),
        model.services.services.len(),
        model.routes.len(),
        model.permissions.len(),
        model.libraries.len(),
        model.configs.len(),
        model.extensions.len(),
    );
    assert!(model.services.services.len() > 500 && model.routes.len() > 300 && model.extensions.len() > 50);
    assert_eq!(
        model.services.service_class("entity_type.manager").as_deref(),
        Some("Drupal\\Core\\Entity\\EntityTypeManager")
    );

    // What its PHP declares.
    let started = Instant::now();
    let names = site.engine.names();
    println!("names read in {:?}", started.elapsed());
    let count = |kind: DeclaredKind| names.of(&[kind]).count();
    let hooks = site.engine.resolver().drupal_hooks();
    println!(
        "{} entity types, {} plugins, {} theme hooks, {} hook functions",
        count(DeclaredKind::DrupalEntityType),
        count(DeclaredKind::DrupalPlugin),
        count(DeclaredKind::DrupalThemeHook),
        hooks.len(),
    );
    assert!(count(DeclaredKind::DrupalEntityType) > 20 && count(DeclaredKind::DrupalPlugin) > 300);
    assert!(hooks.len() > 150, "{} hooks", hooks.len());
    let node = names.named(&[DeclaredKind::DrupalEntityType], "node");
    assert!(
        node.iter().any(|node| node.classes.first().is_some_and(|class| class == "Drupal\\node\\Entity\\Node")),
        "{node:?}"
    );

    // The names of the YAML files.
    assert_eq!(site.goes_to("$m = \\Drupal::service('entity_type./*|*/manager');"), ["core/core.services.yml"]);
    assert_eq!(
        site.goes_to("$url = \\Drupal\\Core\\Url::fromRoute('system./*|*/admin');"),
        ["core/modules/system/system.routing.yml"]
    );
    let found = site.goes_to(
        "function phoxite_probe_can(\\Drupal\\Core\\Session\\AccountInterface $account) { return $account->hasPermission('administer /*|*/nodes'); }",
    );
    assert_eq!(found, ["core/modules/node/node.permissions.yml"]);
    assert_eq!(
        site.goes_to("$build = ['#attached' => ['library' => ['core/drupal./*|*/ajax']]];"),
        ["core/core.libraries.yml"]
    );
    let has = |found: &[String], label: &str| found.iter().any(|known| known == label);
    let found = site.completes("$url = \\Drupal\\Core\\Url::fromRoute('/*|*/');");
    assert!(
        has(&found, "system.admin") && has(&found, "entity.node.canonical") || has(&found, "node.add"),
        "{}",
        found.len()
    );

    // Entity types, plugins and hooks.
    let found = site.goes_to("$s = \\Drupal::entityTypeManager()->getStorage('no/*|*/de');");
    assert_eq!(found, ["core/modules/node/src/Entity/Node.php"]);
    let found = site.goes_to("\\Drupal::moduleHandler()->invokeAll('cr/*|*/on');");
    assert_eq!(found, ["core/core.api.php"]);
    let found = site.goes_to("$data = []; \\Drupal::moduleHandler()->alter('fo/*|*/rm', $data);");
    assert_eq!(found, ["core/lib/Drupal/Core/Form/form.api.php"]);

    // A configuration's key, and from a function implementing a hook to the hook.
    let found = site.goes_to("$name = \\Drupal::config('system.site')->get('na/*|*/me');");
    // The module's file, and those of the profiles that install their own.
    assert_eq!(found.first().map(String::as_str), Some("core/modules/system/config/install/system.site.yml"));
    let found = site.goes_to("function phoxite_probe_cr/*|*/on() {}");
    assert_eq!(found, ["core/core.api.php"]);
    // A menu link a YAML file declares, and a theme hook suggestion.
    let found = site.goes_to(
        "function phoxite_probe_link(\\Drupal\\Core\\Menu\\MenuLinkManagerInterface $links) { $links->createInstance('system./*|*/admin'); }",
    );
    assert_eq!(found, ["core/modules/system/system.links.menu.yml"]);

    // Types: a service, a storage and what it loads, a hook's implementation.
    let found = site.completes("\\Drupal::service('entity_type.manager')->/*|*/");
    assert!(has(&found, "getStorage"), "{found:?}");
    let found = site.completes("\\Drupal::entityTypeManager()->getStorage('node')->load(1)->/*|*/");
    assert!(has(&found, "getTitle") && has(&found, "isPublished"), "a Node: {} members", found.len());
    let found = site.completes("function phoxite_probe_node_presave($node) { $node->/*|*/ }");
    assert!(has(&found, "id") && has(&found, "save"), "an entity: {found:?}");
    let found =
        site.completes("function phoxite_probe_form_alter(&$form, $form_state, $form_id) { $form_state->/*|*/ }");
    assert!(has(&found, "getValue") && has(&found, "setRedirect"), "a form state: {found:?}");
}

/// Drupal 7 (`scripts/php-corpus.sh drupal7`): its hooks are functions and its modules have
/// `.info` files; the same kinds read them under its own functions.
#[test]
#[ignore = "needs the corpora (scripts/php-corpus.sh)"]
fn drupal_7_itself() {
    let mut site = open("drupal7");
    let model = site.engine.drupal();
    let names = site.engine.names();
    let count = |kind: DeclaredKind| names.of(&[kind]).count();
    let hooks = site.engine.resolver().drupal_hooks();
    println!(
        "{} extensions, {} hook functions, {} permissions, {} theme hooks, {} menu paths, {} variables",
        model.extensions.len(),
        hooks.len(),
        count(DeclaredKind::DrupalPermission),
        count(DeclaredKind::DrupalThemeHook),
        count(DeclaredKind::DrupalMenuPath),
        count(DeclaredKind::DrupalVariable),
    );
    assert!(
        model.extensions.len() > 40 && hooks.len() > 200,
        "{} extensions, {} hooks",
        model.extensions.len(),
        hooks.len()
    );
    assert!(count(DeclaredKind::DrupalPermission) > 50 && count(DeclaredKind::DrupalMenuPath) > 200);

    // A hook, by name and through `drupal_alter()`; from an implementation to it.
    assert_eq!(site.goes_to("module_invoke_all('cr/*|*/on');"), ["modules/system/system.api.php"]);
    assert_eq!(site.goes_to("$form = array(); drupal_alter('fo/*|*/rm', $form);"), ["modules/system/system.api.php"]);
    assert_eq!(site.goes_to("function phoxite_probe_cr/*|*/on() {}"), ["modules/system/system.api.php"]);
    // A permission of `hook_permission()`, a module's `.info`, a path of `hook_menu()`.
    assert_eq!(site.goes_to("$can = user_access('administer no/*|*/des');"), ["modules/node/node.module"]);
    assert_eq!(site.goes_to("$on = module_exists('no/*|*/de');"), ["modules/node/node.info"]);
    assert_eq!(site.goes_to("drupal_goto('admin/con/*|*/tent');"), ["modules/node/node.module"]);
    let found = site.goes_to("drupal_goto('node/12/ed/*|*/it');");
    assert_eq!(found, ["modules/node/node.module"], "through the placeholder of `node/%node/edit`");
    // A theme hook: where `hook_theme()` declares it, and the function rendering it.
    let found = site.goes_to("$out = theme('item_/*|*/list', array());");
    assert!(found.iter().any(|file| file == "includes/theme.inc") && found.len() >= 2, "{found:?}");
    // A variable, where it is set; a form by its function.
    let found = site.goes_to("$name = variable_get('site_/*|*/name', '');");
    assert!(!found.is_empty(), "`site_name` is set somewhere");
    let found = site.goes_to("$form = drupal_get_form('user_lo/*|*/gin');");
    assert_eq!(found, ["modules/user/user.module"]);
    // A table of `hook_schema()`.
    println!("{} tables", count(DeclaredKind::DrupalTable));
    assert_eq!(site.goes_to("$rows = db_select('no/*|*/de', 'n');"), ["modules/node/node.install"]);
    // A column of the table a query is on, after its alias.
    println!("{} columns", count(DeclaredKind::DrupalColumn));
    let found = site.goes_to("$query = db_select('node', 'n'); $query->condition('n.ni/*|*/d', 1);");
    assert_eq!(found, ["modules/node/node.install"]);
    // Along a chain, though Drupal 7 documents each method as returning `SelectQuery`; and
    // through the alias of a joined table.
    let found = site.goes_to("$q = db_select('node', 'n')->fields('n', array('title'))->condition('ni/*|*/d', 1);");
    assert_eq!(found, ["modules/node/node.install"]);
    let found = site.goes_to(
        "function phoxite_probe_q() { $q = db_select('node', 'n'); $q->join('users', 'u', 'u.uid = n.uid'); $q->condition('u.na/*|*/me', 'x'); }",
    );
    assert_eq!(found, ["modules/user/user.install"]);
    let found = site.completes("$query = db_select('node', 'n'); $query->condition('/*|*/', 1);");
    assert!(found.iter().any(|label| label == "nid") && found.iter().any(|label| label == "title"), "{found:?}");
    let found = site.completes("$can = user_access('/*|*/');");
    assert!(found.iter().any(|label| label == "access content"), "{} permissions", found.len());
}
