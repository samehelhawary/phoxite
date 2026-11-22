//! Magento support on Magento itself (Magento Open Source 2.4.9, the corpus `magento`):
//!
//! - the fixture module (`fixtures/magento-app`) without its stand-in, on the real
//!   framework (`lib/internal/Magento/Framework`) and the Backend module as include
//!   paths;
//! - the whole checkout as a project: its 222 core modules, with what their XML
//!   declares counted, some of it looked up, the time the model takes to read printed,
//!   and a sample of core modules inspected.
//!
//! The engine's own tests use a small stand-in; this one checks that what they assume
//! holds for the real source: that the names resolve, that the object manager, the
//! generated classes and a template's block are typed, and that Magento's own
//! inspections say nothing about Magento's own code.
//!
//! Ignored: it needs the corpus (`scripts/php-corpus.sh magento`).
//!
//! ```text
//! cargo test -p phoxite --release --test magento_corpus -- --ignored --nocapture
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
const MODULE: &str = "app/code/Shop/Catalog";

fn corpus() -> PathBuf {
    let root = std::env::var_os("PHP_CORPUS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../php-corpus"))
        .join("magento");
    dunce::canonicalize(root).expect("the corpus (scripts/php-corpus.sh magento)")
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

impl Site {
    fn open(root: PathBuf, include: Vec<PathBuf>, copy: Option<tempfile::TempDir>) -> Self {
        let cache = tempfile::tempdir().unwrap();
        let config = EngineConfig {
            cache_dir: cache.path().join(CACHE_SUBDIR),
            layout: LayoutConfig { root: root.clone(), include_paths: include, ..LayoutConfig::default() },
            stubs: StubFilter::default(),
        };
        let sink: SharedSink = Arc::new(|_: IdeEvent| {});
        let started = Instant::now();
        let engine = PhpEngine::open(config, sink).unwrap();
        assert!(engine.wait_idle(Duration::from_secs(1800)), "the corpus indexes");
        println!("indexed in {:?}", started.elapsed());
        Self { engine, root, next: 1, _cache: cache, _copy: copy }
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
                format!("{}:{}", path.to_string_lossy().replace('\\', "/"), target.range.start.line + 1)
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

    /// The reports of Magento's inspections on the PHP files of `folders`, the files
    /// inspected, and how many reports the other inspections make.
    fn reports(&mut self, folders: &[&str]) -> (Vec<String>, usize, usize) {
        let mut files = Vec::new();
        for folder in folders {
            php_files(&self.root.join(folder), &mut files);
        }
        files.sort();
        let (mut unknown, mut others) = (Vec::new(), 0);
        let count = files.len();
        for path in files {
            let Ok(text) = std::fs::read_to_string(&path) else { continue };
            let file = path.strip_prefix(&self.root).unwrap().to_string_lossy().replace('\\', "/");
            let doc = self.document(&file, &text);
            let found = self.engine.inspect(doc, &CancellationToken::new()).expect("open");
            for problem in &found.problems {
                if problem.inspection.contains("Magento") {
                    let line = problem.range.start.line + 1;
                    unknown.push(format!("{file}:{line} {} {}", problem.inspection, problem.message));
                } else {
                    others += 1;
                }
            }
            self.engine.document_closed(doc);
        }
        (unknown, count, others)
    }
}

fn has(found: &[String], names: &[&str]) {
    for name in names {
        assert!(found.contains(&(*name).to_owned()), "{name}: {found:?}");
    }
}

#[test]
#[ignore = "needs the magento corpus"]
fn the_fixture_module_on_the_real_framework() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/magento-app");
    let tmp = tempfile::tempdir().unwrap();
    let root = dunce::canonicalize(tmp.path()).unwrap().join("app");
    copy(&fixture, &root);
    std::fs::remove_dir_all(root.join("lib")).unwrap();
    let magento = corpus();
    let include = vec![magento.join("lib/internal/Magento/Framework"), magento.join("app/code/Magento/Backend")];
    let mut site = Site::open(root, include, Some(tmp));
    assert!(site.engine.frameworks().contains(Framework::Magento));

    let (unknown, files, others) = site.reports(&["app"]);
    println!("fixture: {files} files, {others} reports of other inspections");
    assert!(unknown.is_empty(), "{}", unknown.join("\n"));

    let checkout = &format!("{MODULE}/Model/Checkout.php");
    let mut goes = |from: &str, to: &str| site.goes_to(checkout, from, to);
    assert_eq!(
        goes("dispatch('shop_cart_saved'", "dispatch('shop_cart/*|*/_saved'"),
        [format!("{MODULE}/etc/events.xml:3")]
    );
    assert_eq!(
        goes("getValue('shop/cart/limit')", "getValue('shop/cart/li/*|*/mit')"),
        [format!("{MODULE}/etc/config.xml:6"), format!("{MODULE}/etc/adminhtml/system.xml:8")]
    );
    assert_eq!(
        goes("isAllowed('Shop_Catalog::cart_edit')", "isAllowed('Shop_Catalog::cart_/*|*/edit')"),
        [format!("{MODULE}/etc/acl.xml:7")]
    );
    assert_eq!(
        goes("isEnabled('Shop_Catalog')", "isEnabled('Shop_Cat/*|*/alog')"),
        [format!("{MODULE}/registration.php:5")]
    );
    assert_eq!(
        goes("getBlock('shop.cart')", "getBlock('shop./*|*/cart')"),
        [format!("{MODULE}/view/frontend/layout/shop_cart_index.xml:5")]
    );
    let block = &format!("{MODULE}/Block/Cart.php");
    let found =
        site.goes_to(block, "$_template = 'Shop_Catalog::cart.phtml'", "$_template = 'Shop_Catalog::ca/*|*/rt.phtml'");
    assert_eq!(found, [format!("{MODULE}/view/frontend/templates/cart.phtml:1")]);
    let action = &format!("{MODULE}/Controller/Adminhtml/Cart/Index.php");
    let found =
        site.goes_to(action, "ADMIN_RESOURCE = 'Shop_Catalog::cart'", "ADMIN_RESOURCE = 'Shop_Catalog::ca/*|*/rt'");
    assert_eq!(found, [format!("{MODULE}/etc/acl.xml:6")]);
    let template = &format!("{MODULE}/view/frontend/templates/cart.phtml");
    let found = site.goes_to(template, "getChildHtml('cart.items')", "getChildHtml('cart.it/*|*/ems')");
    assert_eq!(found, [format!("{MODULE}/view/frontend/layout/shop_cart_index.xml:9")]);

    // Types: the generated classes, the object manager, a data object, a template's block.
    let line = "$saved = $cart->save() && $this->lazyCart->total() >= 0;";
    let members = |site: &mut Site, body: &str| site.offered(checkout, line, &format!("{body}\n        {line}"));
    has(&members(&mut site, "$this->cartFactory->create()->/*|*/;"), &["total", "save", "getData"]);
    has(&members(&mut site, "$this->lazyCart->/*|*/;"), &["total", "save"]);
    has(&members(&mut site, "$this->objects->get('ShopLogger')->/*|*/;"), &["log", "count"]);
    has(&members(&mut site, "$item->getExtensionAttributes()->/*|*/;"), &["getGiftNote", "setGiftNote", "getLogger"]);
    has(&members(&mut site, "$cart->setName('x')->/*|*/;"), &["total", "save"]);
    let first = "$totals = $block->getViewModel();";
    let offered = site.offered(template, first, &format!("{first}\n$block->/*|*/;"));
    has(&offered, &["title", "compact", "getChildHtml", "escapeHtml", "getUrl"]);
    has(&site.offered(template, first, &format!("{first}\n$totals->/*|*/;")), &["grand", "lines"]);
    has(
        &site.offered(template, first, &format!("{first}\n$escaper->/*|*/;")),
        &["escapeHtml", "escapeUrl", "escapeHtmlAttr"],
    );
    has(
        &site.offered(template, first, &format!("{first}\n$secureRenderer->/*|*/;")),
        &["renderTag", "renderEventListenerAsTag"],
    );
}

#[test]
#[ignore = "needs the magento corpus"]
fn magento_itself() {
    let mut site = Site::open(corpus(), Vec::new(), None);
    assert!(site.engine.frameworks().contains(Framework::Magento));

    // The model: read once, kept, read again when an XML file is written.
    let started = Instant::now();
    let project = site.engine.magento();
    let cold = started.elapsed();
    let started = Instant::now();
    let again = site.engine.magento();
    let kept = started.elapsed();
    assert!(Arc::ptr_eq(&project, &again));
    let touched = site.root.join("app/code/Magento/Wishlist/etc/events.xml");
    site.engine.files_changed(std::slice::from_ref(&touched), &[]);
    let started = Instant::now();
    let reread = site.engine.magento();
    let after_edit = started.elapsed();
    println!("model: {cold:?} cold, {kept:?} kept, {after_edit:?} after an XML file changed");
    assert_eq!(reread.modules.len(), project.modules.len());

    println!(
        "{} modules, {} themes, {} preferences, {} virtual types, {} plugins, {} observers, {} ACL resources, {} configuration paths, {} blocks, {} handles, {} extension attributes",
        project.modules.len(),
        project.themes.len(),
        project.preferences.len(),
        project.virtual_types.len(),
        project.plugins.len(),
        project.observers.len(),
        project.acl.len(),
        project.config.len(),
        project.blocks.len(),
        project.handles.len(),
        project.extension_attributes.len(),
    );
    assert!(project.modules.len() >= 200, "{}", project.modules.len());
    assert!(project.themes.len() >= 3);
    assert!(project.preferences.len() > 1000 && project.virtual_types.len() > 300 && project.plugins.len() > 300);
    assert!(project.observers.len() > 300 && project.acl.len() > 300 && project.config.len() > 1000);
    assert!(project.blocks.len() > 2000 && project.handles.len() > 500 && project.extension_attributes.len() > 50);

    // Some of it, looked up.
    assert!(
        project.module("Magento_Wishlist").is_some_and(|module| module.path.ends_with("app/code/Magento/Wishlist"))
    );
    let preferred = project.preferred("Magento\\Wishlist\\Controller\\WishlistProviderInterface", None).unwrap();
    assert_eq!(preferred.class, "Magento\\Wishlist\\Controller\\WishlistProvider");
    assert_eq!(
        project.virtual_class("Magento\\Wishlist\\Block\\Context").as_deref(),
        Some("Magento\\Catalog\\Block\\Product\\Context")
    );
    assert!(project.observers.iter().any(|observer| observer.event == "checkout_cart_add_product_complete"));
    assert!(project.acl.iter().any(|def| def.name == "Magento_Wishlist::config_wishlist" && def.detail.is_some()));
    assert!(project.config.iter().any(|def| def.name == "wishlist/email/email_identity"));
    let blocks = project.blocks_of("Magento_Wishlist::view.phtml");
    assert!(
        blocks.iter().any(|block| block.class.as_deref() == Some("Magento\\Wishlist\\Block\\Customer\\Wishlist")),
        "{blocks:?}"
    );
    let files = project.template_files("Magento_Wishlist::view.phtml", None, None);
    assert!(files.iter().any(|file| file.is_file()), "{files:?}");

    // The other declaration files, areas and themes.
    let names = project.mentions().count();
    println!(
        "{} routes, {} tables, {} files read, {} names written in them",
        project.routes.len(),
        project.tables.len(),
        project.files.len(),
        names
    );
    assert!(project.routes.len() > 80 && project.tables.len() > 300 && names > 20_000, "{names}");
    assert_eq!(project.route_modules("wishlist", Some("frontend")), ["Magento_Wishlist"]);
    assert!(
        project.table("wishlist").is_some_and(|table| table.columns.iter().any(|column| column.0 == "wishlist_id"))
    );
    let chain: Vec<&str> = project.theme_chain(Some("frontend")).iter().map(|theme| theme.code.as_str()).collect();
    assert_eq!(chain, ["Magento/luma", "Magento/blank"]);
    let resolvers =
        project.mentions().filter(|(file, _)| file.file.extension().is_some_and(|ext| ext == "graphqls")).count();
    assert!(resolvers > 200, "{resolvers}");

    // Every file the model reads, as a document. Magento's own XML names a few classes
    // it no longer has (12 in 2.4.9): each report must be of a class whose file isn't there.
    let started = Instant::now();
    let mut reported = Vec::new();
    for file in &project.files {
        let Ok(text) = std::fs::read_to_string(&file.file) else { continue };
        for problem in site.engine.magento_document_problems(&file.file, &text).unwrap_or_default() {
            let path = file.file.strip_prefix(&site.root).unwrap_or(&file.file).to_string_lossy().replace('\\', "/");
            reported.push(format!("{path}:{} {}", problem.range.start.line + 1, problem.message));
            let class = problem.message.split('\'').nth(1).expect("a class's name");
            let below = class.strip_prefix("Magento\\").expect("a core class").replace('\\', "/");
            let declared = site.root.join("app/code/Magento").join(format!("{below}.php"));
            assert!(
                problem.message.starts_with("Undefined class") && !declared.is_file(),
                "{path}: {}",
                problem.message
            );
        }
    }
    println!(
        "{} files looked at as documents in {:?}: {} problems",
        project.files.len(),
        started.elapsed(),
        reported.len()
    );
    println!("{}", reported.join("\n"));
    assert!(reported.len() <= 12, "{}", reported.join("\n"));

    // Core code: a template's block, a generated factory, the object manager.
    let template = "app/code/Magento/Wishlist/view/frontend/templates/item/list.phtml";
    let offered = site.offered(template, "<?php", "<?php $block->/*|*/; ?>\n<?php");
    has(&offered, &["getColumns", "getChildHtml", "escapeHtml"]);
    let helper = "app/code/Magento/Wishlist/Helper/Data.php";
    let goes = site.goes_to(helper, "'wishlist/general/active',", "'wishlist/general/act/*|*/ive',");
    assert!(goes.iter().any(|place| place.starts_with("app/code/Magento/Wishlist/etc/config.xml:")), "{goes:?}");

    // Magento's inspections on a sample of the core modules.
    let folders =
        ["app/code/Magento/Wishlist", "app/code/Magento/Review", "app/code/Magento/Newsletter", "app/code/Magento/Cms"];
    let started = Instant::now();
    let (unknown, files, others) = site.reports(&folders);
    println!("{files} files inspected in {:?}: {others} reports of other inspections", started.elapsed());
    assert!(unknown.is_empty(), "{}", unknown.join("\n"));
}
