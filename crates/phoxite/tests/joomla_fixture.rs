//! Joomla support on the fixture site (`fixtures/joomla-app`): a component with its site and
//! administrator folders, a plugin, a module and a site template, on a stand-in for
//! Joomla's API.
//!
//! Every PHP file of the extensions is inspected with nothing to report (their templates
//! among them, whose `$this` is the view), one name of each kind resolves, and the XML,
//! language and asset files answer as documents.

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

const ADMIN: &str = "administrator/components/com_shop";
const SITE: &str = "components/com_shop";

#[test]
fn nothing_in_the_fixture_is_reported() {
    let mut site = Site::open("joomla-app");
    assert!(site.engine.frameworks().contains(Framework::Joomla));
    let found = reported(&mut site, &["administrator", "components", "modules", "plugins", "templates", "layouts"], 16);
    assert!(found.is_empty(), "{}", found.join("\n"));
}

#[test]
fn one_name_of_each_kind_goes_where_it_should() {
    let mut site = Site::open("joomla-app");
    let helper = &format!("{SITE}/src/Helper/ShopHelper.php");
    let goes = |site: &mut Site, file: &str, from: &str, to: &str| site.goes_to(file, from, to);
    assert_eq!(
        goes(&mut site, helper, "Text::_('COM_SHOP')", "Text::_('COM_/*|*/SHOP')"),
        ["language/en-GB/com_shop.ini:2"]
    );
    assert_eq!(
        goes(&mut site, helper, "render('joomla.content.tags'", "render('joomla.content.ta/*|*/gs'"),
        ["templates/blue/html/layouts/joomla/content/tags.php:1", "layouts/joomla/content/tags.php:1"]
    );
    assert_eq!(
        goes(&mut site, helper, "useScript('com_shop.cart')", "useScript('com_shop.ca/*|*/rt')"),
        ["media/com_shop/joomla.asset.json:7"]
    );
    assert_eq!(
        goes(&mut site, helper, "bootComponent('com_shop')", "bootComponent('com_/*|*/shop')"),
        [format!("{ADMIN}/shop.xml:1")]
    );
    assert_eq!(
        goes(&mut site, helper, "authorise('shop.ship'", "authorise('shop.sh/*|*/ip'"),
        [format!("{ADMIN}/access.xml:5")]
    );
    assert_eq!(
        goes(&mut site, helper, "getValue('title')", "getValue('ti/*|*/tle')"),
        [format!("{ADMIN}/forms/item.xml:4")]
    );
    assert_eq!(
        goes(&mut site, helper, "get('show_prices')", "get('show_/*|*/prices')"),
        [format!("{ADMIN}/config.xml:4")]
    );
    assert_eq!(
        goes(&mut site, helper, "from('#__shop_items AS a')", "from('#__shop_/*|*/items AS a')"),
        [format!("{ADMIN}/sql/install.mysql.utf8.sql:1")]
    );
    assert_eq!(
        goes(&mut site, helper, "dispatch('onContentPrepare')", "dispatch('onContent/*|*/Prepare')").len(),
        1,
        "the event's class"
    );
    assert_eq!(
        goes(&mut site, helper, "option=com_shop&view=items", "option=com_shop&view=it/*|*/ems"),
        [format!("{SITE}/src/View/Items/HtmlView.php:7")]
    );
    // A form, a table's key; a subscriber's handler; a view's sub-template, a template's override first.
    let model = &format!("{ADMIN}/src/Model/ItemModel.php");
    assert_eq!(
        goes(&mut site, model, "'item', ['control'", "'it/*|*/em', ['control'"),
        [format!("{ADMIN}/forms/item.xml:1")]
    );
    let table = &format!("{ADMIN}/src/Table/ItemTable.php");
    assert_eq!(
        goes(&mut site, table, "'#__shop_items', 'id'", "'#__shop_items', 'i/*|*/d'"),
        [format!("{ADMIN}/sql/install.mysql.utf8.sql:2")]
    );
    let plugin = "plugins/content/stamp/src/Extension/Stamp.php";
    assert_eq!(goes(&mut site, plugin, "=> 'stamp'", "=> 'st/*|*/amp'"), [format!("{plugin}:22")]);
    let view = &format!("{SITE}/tmpl/items/default.php");
    assert_eq!(
        goes(&mut site, view, "loadTemplate('row')", "loadTemplate('r/*|*/ow')"),
        [
            "templates/blue/html/com_shop/items/default_row.php:1".to_owned(),
            format!("{SITE}/tmpl/items/default_row.php:1")
        ]
    );
}

#[test]
fn types_come_from_the_site() {
    let mut site = Site::open("joomla-app");
    let helper = &format!("{SITE}/src/Helper/ShopHelper.php");
    let line = "$query = $this->db->getQuery(true);";
    let members = |site: &mut Site, body: &str| site.offered(helper, line, &format!("{body}\n        {line}"));
    has(&members(&mut site, "$app->/*|*/;"), &["getDocument", "getIdentity", "bootComponent"]);
    has(&members(&mut site, "$component->/*|*/;"), &["currency", "getMVCFactory"]);
    has(&members(&mut site, "$this->factory->createModel('Items')->/*|*/;"), &["getItems", "total"]);
    has(&members(&mut site, "$this->factory->createModel('Item', 'Administrator')->/*|*/;"), &["publish", "getForm"]);
    has(&members(&mut site, "$this->factory->createTable('Item', 'Administrator')->/*|*/;"), &["stock", "load"]);
    // A view's template and its override; a module's; a layout; the site template.
    let view = &format!("{SITE}/tmpl/items/default.php");
    has(&site.offered(view, "<h1>", "<?php $this->/*|*/ ?><h1>"), &["heading", "items", "loadTemplate"]);
    let row = "templates/blue/html/com_shop/items/default_row.php";
    has(&site.offered(row, "<ol>", "<?php $this->items[0]->/*|*/ ?><ol>"), &["title", "price"]);
    has(&site.offered("modules/mod_cart/tmpl/default.php", "<div", "<?php $app->/*|*/ ?><div"), &["getIdentity"]);
    has(&site.offered("layouts/joomla/content/tags.php", "<ul", "<?php $this->/*|*/ ?><ul"), &["render", "escape"]);
    has(
        &site.offered("templates/blue/index.php", "<html>", "<?php $this->/*|*/ ?><html>"),
        &["countModules", "getWebAssetManager"],
    );
    // An event handler's parameter, by what its class subscribes the method to.
    let plugin = "plugins/content/stamp/src/Extension/Stamp.php";
    has(&site.offered(plugin, "$item = $event->getItem();", "$event->/*|*/;"), &["getItem", "getName"]);
    has(
        &site.offered(plugin, "if ($event->getIsNew()", "$event->/*|*/;\n        if ($event->getIsNew()"),
        &["getIsNew"],
    );
}

/// The caret's place in the file `file`, where `from` is written `to` with the caret.
fn in_file(site: &Site, file: &str, from: &str, to: &str) -> (PathBuf, String, Pos) {
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
fn the_xml_language_and_asset_files_are_documents() {
    let site = Site::open("joomla-app");
    let goes = |file: &str, from: &str, to: &str| -> Vec<String> {
        let (path, text, pos) = in_file(&site, file, from, to);
        let found = site.engine.joomla_document_definition(&path, &text, pos).unwrap_or_default();
        found.iter().map(|target| site.place(&target.path, target.range.start.line)).collect()
    };
    let form = &format!("{ADMIN}/forms/item.xml");
    assert_eq!(goes(form, "COM_SHOP_FIELD_TITLE", "COM_SHOP_FIELD/*|*/_TITLE"), ["language/en-GB/com_shop.ini:4"]);
    assert_eq!(goes(form, "type=\"money\"", "type=\"mon/*|*/ey\""), [format!("{ADMIN}/src/Field/MoneyField.php:7")]);
    assert_eq!(goes(form, "type=\"text\"", "type=\"te/*|*/xt\"").len(), 1, "Joomla's own field");
    assert_eq!(
        goes(form, "joomla.form.field.number", "joomla.form.field.num/*|*/ber"),
        ["layouts/joomla/form/field/number.php:1"]
    );
    assert_eq!(
        goes(&format!("{ADMIN}/shop.xml"), "COM_SHOP_XML_DESCRIPTION", "COM_SHOP_XML/*|*/_DESCRIPTION"),
        ["language/en-GB/com_shop.ini:8"]
    );
    assert_eq!(
        goes("media/com_shop/joomla.asset.json", "\"core\"\n", "\"co/*|*/re\"\n"),
        ["media/com_shop/joomla.asset.json:15"]
    );
    // From a language file's key: where PHP and the XML use it.
    let (path, text, pos) =
        in_file(&site, "language/en-GB/com_shop.ini", "COM_SHOP_FIELD_TITLE=", "COM_SHOP_FIELD/*|*/_TITLE=");
    let cancel = CancellationToken::new();
    let usages = site.engine.joomla_document_references(&path, &text, pos, false, &cancel).expect("handled").unwrap();
    let places: Vec<String> = usages.iter().map(|usage| site.place(&usage.path, usage.range.start.line)).collect();
    assert_eq!(places, [format!("{ADMIN}/forms/item.xml:4")]);
    let (path, text, pos) = in_file(&site, form, "label=\"COM_SHOP_FIELD_PRICE\"", "label=\"COM_SHOP_F/*|*/\"");
    let offered: Vec<String> =
        site.engine.joomla_document_completion(&path, &text, pos).unwrap().into_iter().map(|item| item.label).collect();
    assert_eq!(offered, ["COM_SHOP_FIELD_PRICE", "COM_SHOP_FIELD_TITLE"]);
}
