//! Magento support on the fixture project (`fixtures/magento-app`): one module under
//! `app/code` and a theme, on a stand-in for the framework's API.
//!
//! Every PHP file of the module is inspected with nothing to report (its templates among
//! them, whose `$block` the layout XML says; the classes Magento would generate among the
//! names), and one name of each kind resolves.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use phoxite::base::{CancellationToken, DocId, IdeEvent, Pos, SharedSink};
use phoxite::engine::{CACHE_SUBDIR, EngineConfig, PhpEngine};
use phoxite::frameworks::Framework;
use phoxite::project::LayoutConfig;
use phoxite::stubs::StubFilter;

const CARET: &str = "/*|*/";
const MODULE: &str = "app/code/Shop/Catalog";

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

/// The PHP files of the project's folders, inspected: what is reported.
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
fn nothing_in_the_fixture_is_reported() {
    let mut site = Site::open("magento-app");
    assert!(site.engine.frameworks().contains(Framework::Magento));
    let found = reported(&mut site, &["app"], 15);
    assert!(found.is_empty(), "{}", found.join("\n"));
}

#[test]
fn one_name_of_each_kind_goes_where_it_should() {
    let mut site = Site::open("magento-app");
    let checkout = &format!("{MODULE}/Model/Checkout.php");
    let goes = |site: &mut Site, file: &str, from: &str, to: &str| site.goes_to(file, from, to);
    assert_eq!(
        goes(&mut site, checkout, "dispatch('shop_cart_saved'", "dispatch('shop_cart/*|*/_saved'"),
        [format!("{MODULE}/etc/events.xml:3")]
    );
    assert_eq!(
        goes(&mut site, checkout, "getValue('shop/cart/limit')", "getValue('shop/cart/li/*|*/mit')"),
        [format!("{MODULE}/etc/config.xml:6"), format!("{MODULE}/etc/adminhtml/system.xml:8")]
    );
    assert_eq!(
        goes(&mut site, checkout, "isAllowed('Shop_Catalog::cart_edit')", "isAllowed('Shop_Catalog::cart_/*|*/edit')"),
        [format!("{MODULE}/etc/acl.xml:7")]
    );
    assert_eq!(
        goes(&mut site, checkout, "isEnabled('Shop_Catalog')", "isEnabled('Shop_Cat/*|*/alog')"),
        [format!("{MODULE}/registration.php:5")]
    );
    assert_eq!(
        goes(&mut site, checkout, "getBlock('shop.cart')", "getBlock('shop./*|*/cart')"),
        [format!("{MODULE}/view/frontend/layout/shop_cart_index.xml:5")]
    );
    // A template: the theme's version first, then the module's.
    let block = &format!("{MODULE}/Block/Cart.php");
    assert_eq!(
        goes(
            &mut site,
            block,
            "setTemplate('Shop_Catalog::cart/summary.phtml')",
            "setTemplate('Shop_Catalog::cart/sum/*|*/mary.phtml')"
        ),
        [
            "app/design/frontend/Shop/blue/Shop_Catalog/templates/cart/summary.phtml:1".to_owned(),
            "app/design/frontend/Shop/base/Shop_Catalog/templates/cart/summary.phtml:1".to_owned(),
            format!("{MODULE}/view/frontend/templates/cart/summary.phtml:1"),
        ]
    );
    assert_eq!(
        goes(
            &mut site,
            block,
            "$_template = 'Shop_Catalog::cart.phtml'",
            "$_template = 'Shop_Catalog::ca/*|*/rt.phtml'"
        ),
        [format!("{MODULE}/view/frontend/templates/cart.phtml:1")]
    );
    let action = &format!("{MODULE}/Controller/Adminhtml/Cart/Index.php");
    assert_eq!(
        goes(&mut site, action, "ADMIN_RESOURCE = 'Shop_Catalog::cart'", "ADMIN_RESOURCE = 'Shop_Catalog::ca/*|*/rt'"),
        [format!("{MODULE}/etc/acl.xml:6")]
    );
    // A child block by its alias, in a template.
    let template = &format!("{MODULE}/view/frontend/templates/cart.phtml");
    assert_eq!(
        goes(&mut site, template, "getChildHtml('cart.items')", "getChildHtml('cart.it/*|*/ems')"),
        [format!("{MODULE}/view/frontend/layout/shop_cart_index.xml:9")]
    );
    // A plugin's method: the method it intercepts.
    let plugin = &format!("{MODULE}/Plugin/CartLimit.php");
    assert_eq!(
        goes(&mut site, plugin, "function beforeSave(", "function before/*|*/Save("),
        [format!("{MODULE}/Model/Cart.php:15")]
    );
}

#[test]
fn types_come_from_the_xml() {
    let mut site = Site::open("magento-app");
    let checkout = &format!("{MODULE}/Model/Checkout.php");
    let line = "$saved = $cart->save() && $this->lazyCart->total() >= 0;";
    let members = |site: &mut Site, body: &str| site.offered(checkout, line, &format!("{body}\n        {line}"));
    // What a factory makes, a proxy, a virtual type, an extension attribute.
    has(&members(&mut site, "$this->cartFactory->create()->/*|*/;"), &["total", "save"]);
    has(&members(&mut site, "$this->lazyCart->/*|*/;"), &["total", "save"]);
    has(&members(&mut site, "$this->objects->get('ShopLogger')->/*|*/;"), &["log", "count"]);
    has(&members(&mut site, "$item->getExtensionAttributes()->/*|*/;"), &["getGiftNote", "setGiftNote", "getLogger"]);
    has(&members(&mut site, "$item->getExtensionAttributes()->getLogger()->/*|*/;"), &["log"]);
    // A data object's setters return it.
    has(&members(&mut site, "$cart->setName('x')->/*|*/;"), &["total", "save"]);
    // A template's block and view model; a block without a class is a template block.
    let template = &format!("{MODULE}/view/frontend/templates/cart.phtml");
    let first = "$totals = $block->getViewModel();";
    has(&site.offered(template, first, &format!("{first}\n$block->/*|*/;")), &["title", "compact", "getChildHtml"]);
    has(&site.offered(template, first, &format!("{first}\n$totals->/*|*/;")), &["grand", "lines"]);
    has(&site.offered(template, first, &format!("{first}\n$viewModel->/*|*/;")), &["grand", "lines"]);
    let items = &format!("{MODULE}/view/frontend/templates/cart/items.phtml");
    has(&site.offered(items, "<ul>", "<ul><?php $block->/*|*/ ?>"), &["names"]);
    // A theme's template is the module's.
    let theme = "app/design/frontend/Shop/blue/Shop_Catalog/templates/cart/summary.phtml";
    has(&site.offered(theme, "<p class", "<?php $block->/*|*/ ?><p class"), &["getTemplate", "setTemplate"]);
}

/// The caret's place in the XML file `file`, where `from` is written `to` with the caret: the
/// file's path, its text without the caret, and the caret.
fn in_xml(site: &Site, file: &str, from: &str, to: &str) -> (PathBuf, String, Pos) {
    let path = site.root.join(file);
    let source = std::fs::read_to_string(&path).unwrap().replace("\r\n", "\n");
    assert!(source.contains(from), "{from} in {file}");
    let source = source.replacen(from, to, 1);
    let at = source.find(CARET).expect("a caret");
    let text = format!("{}{}", &source[..at], &source[at + CARET.len()..]);
    let line = text[..at].matches('\n').count() as u32;
    let col = text[..at].rsplit('\n').next().unwrap_or_default().encode_utf16().count() as u32;
    (path, text, Pos::new(line, col))
}

#[test]
fn the_xml_files_are_documents() {
    let mut site = Site::open("magento-app");
    let goes = |site: &Site, file: &str, from: &str, to: &str| -> Vec<String> {
        let (path, text, pos) = in_xml(site, file, from, to);
        let found = site.engine.magento_document_definition(&path, &text, pos).unwrap_or_default();
        found.iter().map(|target| site.place(&target.path, target.range.start.line)).collect()
    };
    let di = &format!("{MODULE}/etc/di.xml");
    // A class, a plugin, an observer, a service's method, a resource, a template, a block.
    assert_eq!(
        goes(&site, di, "type=\"Shop\\Catalog\\Model\\Cart\"", "type=\"Shop\\Catalog\\Model\\Ca/*|*/rt\""),
        [format!("{MODULE}/Model/Cart.php:8")]
    );
    assert_eq!(
        goes(&site, di, "Plugin\\CartLimit", "Plugin\\Cart/*|*/Limit"),
        [format!("{MODULE}/Plugin/CartLimit.php:7")]
    );
    let events = &format!("{MODULE}/etc/events.xml");
    assert_eq!(
        goes(&site, events, "Observer\\Notify", "Observer\\Not/*|*/ify"),
        [format!("{MODULE}/Observer/Notify.php:9")]
    );
    let webapi = &format!("{MODULE}/etc/webapi.xml");
    assert_eq!(
        goes(&site, webapi, "method=\"total\"", "method=\"to/*|*/tal\""),
        [format!("{MODULE}/Api/CartInterface.php:7")]
    );
    assert_eq!(
        goes(&site, webapi, "ref=\"Shop_Catalog::cart\"", "ref=\"Shop_Catalog::ca/*|*/rt\""),
        [format!("{MODULE}/etc/acl.xml:6")]
    );
    let layout = &format!("{MODULE}/view/frontend/layout/shop_cart_index.xml");
    assert_eq!(
        goes(&site, layout, "template=\"Shop_Catalog::cart.phtml\"", "template=\"Shop_Catalog::ca/*|*/rt.phtml\""),
        [format!("{MODULE}/view/frontend/templates/cart.phtml:1")]
    );
    let default = &format!("{MODULE}/view/frontend/layout/default.xml");
    assert_eq!(
        goes(&site, default, "after=\"shop.cart.items\"", "after=\"shop.cart.it/*|*/ems\""),
        [format!("{layout}:9")]
    );
    assert_eq!(
        goes(&site, default, "handle=\"shop_cart_index\"", "handle=\"shop_ca/*|*/rt_index\""),
        [format!("{layout}:2")]
    );

    // Hover: the preference of the file's area.
    let admin = &format!("{MODULE}/etc/adminhtml/di.xml");
    let (path, text, pos) = in_xml(&site, admin, "Api\\CartInterface", "Api\\Cart/*|*/Interface");
    let hover = site.engine.magento_document_hover(&path, &text, pos).expect("a hover").markdown;
    assert!(hover.contains("Preference: `Shop\\Catalog\\Model\\AdminCart`"), "{hover}");

    // Usages: the class's in PHP and in the XML.
    let (path, text, pos) = in_xml(
        &site,
        di,
        "<type name=\"Shop\\Catalog\\Model\\Cart\"",
        "<type name=\"Shop\\Catalog\\Model\\Ca/*|*/rt\"",
    );
    let cancel = CancellationToken::new();
    let usages = site.engine.magento_document_references(&path, &text, pos, false, &cancel).expect("handled").unwrap();
    let files: Vec<String> = usages.iter().map(|usage| site.place(&usage.path, usage.range.start.line)).collect();
    has(
        &files,
        &[
            &format!("{MODULE}/etc/di.xml:3"),
            &format!("{MODULE}/Plugin/CartLimit.php:5"),
            &format!("{MODULE}/Model/AdminCart.php:5"),
        ],
    );

    // Completion: the classes of a namespace; the templates.
    let (path, text, pos) =
        in_xml(&site, di, "type=\"Shop\\Catalog\\Model\\Cart\"/>", "type=\"Shop\\Catalog\\Model\\/*|*/\"/>");
    let offered: Vec<String> = site
        .engine
        .magento_document_completion(&path, &text, pos)
        .unwrap()
        .into_iter()
        .map(|item| item.label)
        .collect();
    has(&offered, &["Shop\\Catalog\\Model\\Cart", "Shop\\Catalog\\Model\\Logger", "ShopLogger"]);

    // Nothing in the fixture's XML is reported; a class that isn't there is.
    for file in [di, admin, events, webapi, layout, default] {
        let text = std::fs::read_to_string(site.root.join(file)).unwrap();
        let problems = site.engine.magento_document_problems(&site.root.join(file), &text).expect("a document");
        assert!(problems.is_empty(), "{file}: {problems:?}");
    }
    let text = std::fs::read_to_string(site.root.join(di)).unwrap().replace("Plugin\\CartLimit", "Plugin\\Gone");
    let problems = site.engine.magento_document_problems(&site.root.join(di), &text).unwrap();
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert_eq!(problems[0].message, "Undefined class 'Shop\\Catalog\\Plugin\\Gone'");

    // Routes, tables and columns, from PHP.
    let controller = &format!("{MODULE}/Controller/Cart/Index.php");
    assert_eq!(
        site.goes_to(controller, "_redirect('shop/cart/index')", "_redirect('shop/cart/in/*|*/dex')"),
        [format!("{controller}:7")]
    );
    let resource = &format!("{MODULE}/Model/ResourceModel/Cart.php");
    assert_eq!(
        site.goes_to(resource, "_init('shop_cart', 'cart_id')", "_init('shop_/*|*/cart', 'cart_id')"),
        [format!("{MODULE}/etc/db_schema.xml:3")]
    );
    assert_eq!(
        site.goes_to(resource, "_init('shop_cart', 'cart_id')", "_init('shop_cart', 'cart/*|*/_id')"),
        [format!("{MODULE}/etc/db_schema.xml:4")]
    );
}
