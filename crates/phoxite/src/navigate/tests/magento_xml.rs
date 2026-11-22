//! Magento 2: the XML files as documents, a class's and a method's usages
//! in them, renames across PHP and XML, routes, tables, areas and themes.
//! On a stand-in for Magento's API written for these tests.

use super::*;

const FRAMEWORK: &str = r#"<?php
namespace Magento\Framework {
    class DataObject {
        public function __call($method, $args) {}
    }
    interface UrlInterface {
        public function getUrl($routePath = null, $routeParams = null);
    }
    interface AuthorizationInterface {
        public function isAllowed($resource, $privilege = null);
    }
}
namespace Magento\Framework\Event {
    interface ManagerInterface {
        public function dispatch($eventName, array $data = []);
    }
}
namespace Magento\Framework\App {
    class ResourceConnection {
        public function getTableName($modelEntity) {}
    }
}
namespace Magento\Framework\App\Action {
    abstract class Action {
        protected function _redirect($path, $arguments = []) {}
    }
}
namespace Magento\Framework\Model\ResourceModel\Db {
    abstract class AbstractDb {
        protected function _init($mainTable, $idFieldName) {}
        public function getTable($tableName) {}
    }
}
namespace Magento\Framework\View {
    interface LayoutInterface {
        public function getBlock($name);
    }
}
namespace Magento\Framework\View\Element {
    class Template extends \Magento\Framework\DataObject {
        protected $_template;
        public function setTemplate($template) {}
    }
}
"#;

const REGISTRATION: &str = "<?php\nuse Magento\\Framework\\Component\\ComponentRegistrar;\n\nComponentRegistrar::register(ComponentRegistrar::MODULE, 'Shop_Catalog', __DIR__);\n";

const DI: &str = r#"<?xml version="1.0"?>
<config>
    <preference for="Shop\Catalog\Api\CartInterface" type="Shop\Catalog\Model\Cart"/>
    <virtualType name="ShopLogger" type="Shop\Catalog\Model\Logger"/>
    <type name="Shop\Catalog\Model\Cart">
        <plugin name="limit" type="Shop\Catalog\Plugin\CartLimit"/>
        <arguments>
            <argument name="logger" xsi:type="object">ShopLogger</argument>
        </arguments>
    </type>
</config>
"#;

const ADMIN_DI: &str = r#"<?xml version="1.0"?>
<config>
    <preference for="Shop\Catalog\Api\CartInterface" type="Shop\Catalog\Model\AdminCart"/>
</config>
"#;

const EVENTS: &str = r#"<?xml version="1.0"?>
<config>
    <event name="shop_cart_saved">
        <observer name="notify" instance="Shop\Catalog\Observer\Notify"/>
    </event>
</config>
"#;

const ACL: &str = r#"<?xml version="1.0"?>
<config>
    <acl>
        <resources>
            <resource id="Shop_Catalog::cart" title="Carts"/>
        </resources>
    </acl>
</config>
"#;

const SYSTEM: &str = r#"<?xml version="1.0"?>
<config>
    <system>
        <section id="shop">
            <resource>Shop_Catalog::cart</resource>
            <group id="cart">
                <field id="limit">
                    <source_model>Shop\Catalog\Model\Logger</source_model>
                </field>
            </group>
        </section>
    </system>
</config>
"#;

const ROUTES: &str = r#"<?xml version="1.0"?>
<config>
    <router id="standard">
        <route id="shop" frontName="shop">
            <module name="Shop_Catalog"/>
        </route>
    </router>
</config>
"#;

const SCHEMA: &str = r#"<?xml version="1.0"?>
<schema>
    <table name="shop_cart" comment="Carts">
        <column xsi:type="int" name="cart_id"/>
        <column xsi:type="decimal" name="total"/>
    </table>
</schema>
"#;

const WEBAPI: &str = r#"<?xml version="1.0"?>
<routes>
    <route url="/V1/carts" method="GET">
        <service class="Shop\Catalog\Api\CartInterface" method="total"/>
        <resources>
            <resource ref="Shop_Catalog::cart"/>
        </resources>
    </route>
</routes>
"#;

const GRAPHQL: &str = "type Query {\n    cart: Cart @resolver(class: \"\\\\Shop\\\\Catalog\\\\Model\\\\Cart\")\n}\n";

const LAYOUT: &str = r#"<?xml version="1.0"?>
<page>
    <body>
        <block class="Shop\Catalog\Block\Cart" name="shop.cart" template="Shop_Catalog::cart.phtml">
            <block name="shop.cart.items" template="items.phtml"/>
        </block>
    </body>
</page>
"#;

const DEFAULT_LAYOUT: &str = r#"<?xml version="1.0"?>
<page>
    <update handle="shop_cart_index"/>
    <body>
        <referenceBlock name="shop.cart">
            <block name="shop.badge" after="shop.cart.items"/>
        </referenceBlock>
        <move element="shop.cart.items" destination="shop.cart"/>
    </body>
</page>
"#;

const CLASSES: &str = r#"<?php
namespace Shop\Catalog\Api {
    interface CartInterface {
        public function total(): float;
    }
}
namespace Shop\Catalog\Model {
    class Cart extends \Magento\Framework\DataObject implements \Shop\Catalog\Api\CartInterface {
        public function total(): float { return 0.0; }
        public function save(int $times = 1): bool { return true; }
    }
    class AdminCart extends Cart {}
    class Logger {
        public function log(string $line): void {}
    }
}
namespace Shop\Catalog\Model\ResourceModel {
    class Cart extends \Magento\Framework\Model\ResourceModel\Db\AbstractDb {
        protected function _construct() { $this->_init('shop_cart', 'cart_id'); }
    }
}
namespace Shop\Catalog\Plugin {
    class CartLimit {
        public function beforeSave($subject, int $times = 1) { return [$times]; }
        public function afterTotal($subject, $result) { return $result; }
    }
}
namespace Shop\Catalog\Observer {
    class Notify {}
}
namespace Shop\Catalog\Block {
    class Cart extends \Magento\Framework\View\Element\Template {}
}
"#;

/// In its own file: an action is found by its folder.
const CONTROLLER: &str = r#"<?php
namespace Shop\Catalog\Controller\Cart;

class Index extends \Magento\Framework\App\Action\Action
{
    public function execute() { return $this->_redirect('shop/cart'); }
}
"#;

const SERVICE: &str = r#"<?php
namespace Shop\Catalog\Model;

use Magento\Framework\App\ResourceConnection;
use Magento\Framework\AuthorizationInterface;
use Magento\Framework\Event\ManagerInterface;
use Magento\Framework\UrlInterface;
use Magento\Framework\View\LayoutInterface;
use Shop\Catalog\Block\Cart as CartBlock;

class Service
{
    public function __construct(
        private ManagerInterface $events,
        private UrlInterface $url,
        private ResourceConnection $resource,
        private LayoutInterface $layout,
        private AuthorizationInterface $authorization,
        private CartBlock $block,
        private Cart $cart,
    ) {}

    public function run()
    {
        $this->events->dispatch('shop_cart_saved');
        $this->url->getUrl('shop/cart/index');
        $this->resource->getTableName('shop_cart');
        $this->layout->getBlock('shop.cart.items');
        $this->authorization->isAllowed('Shop_Catalog::cart');
        $this->block->setTemplate('Shop_Catalog::cart.phtml');
        $this->cart->save();
    }
}
"#;

const MODULE: &str = "app/code/Shop/Catalog";
const PROBE: &str = "app/code/Shop/Catalog/Model/Service.php";
const BASE_THEME: &str = "app/design/frontend/Shop/base_theme";
const CHILD_THEME: &str = "app/design/frontend/Shop/child";

fn shop() -> Project {
    let file = |rel: &str| format!("{MODULE}/{rel}");
    let files: Vec<(String, &str)> = vec![
        ("app/etc/di.xml".to_owned(), "<config/>\n"),
        ("framework/classes.php".to_owned(), FRAMEWORK),
        (file("registration.php"), REGISTRATION),
        (file("etc/module.xml"), "<config>\n    <module name=\"Shop_Catalog\"/>\n</config>\n"),
        (file("etc/di.xml"), DI),
        (file("etc/adminhtml/di.xml"), ADMIN_DI),
        (file("etc/events.xml"), EVENTS),
        (file("etc/acl.xml"), ACL),
        (file("etc/adminhtml/system.xml"), SYSTEM),
        (file("etc/frontend/routes.xml"), ROUTES),
        (file("etc/db_schema.xml"), SCHEMA),
        (file("etc/webapi.xml"), WEBAPI),
        (file("etc/schema.graphqls"), GRAPHQL),
        (file("view/frontend/layout/shop_cart_index.xml"), LAYOUT),
        (file("view/frontend/layout/default.xml"), DEFAULT_LAYOUT),
        (file("view/frontend/templates/cart.phtml"), "<p></p>\n"),
        (file("view/frontend/templates/items.phtml"), "<p></p>\n"),
        (format!("{BASE_THEME}/theme.xml"), "<theme><title>Base</title></theme>\n"),
        (format!("{BASE_THEME}/Shop_Catalog/templates/cart.phtml"), "<p></p>\n"),
        (format!("{CHILD_THEME}/theme.xml"), "<theme><title>Child</title><parent>Shop/base_theme</parent></theme>\n"),
        (format!("{CHILD_THEME}/Shop_Catalog/templates/cart.phtml"), "<p></p>\n"),
        (file("classes.php"), CLASSES),
        (file("Controller/Cart/Index.php"), CONTROLLER),
        (PROBE.to_owned(), SERVICE),
    ];
    let files: Vec<(&str, &str)> = files.iter().map(|(rel, text)| (rel.as_str(), *text)).collect();
    Project::new(&files)
}

/// What an edit replaces, as `(file, line, replaced text, new text)`, and the files it moves.
type Edits = (Vec<(String, u32, String, String)>, Vec<(String, String)>);

fn module(rel: &str) -> String {
    format!("{MODULE}/{rel}")
}

fn replaced(source: &str, from: &str, to: &str) -> String {
    assert!(source.contains(from), "{from}");
    source.replacen(from, to, 1)
}

impl Project {
    fn mx_places(&self, targets: Vec<Target>) -> Vec<(String, u32)> {
        let rel = |path: &Path| path.strip_prefix(&self.root).unwrap_or(path).to_string_lossy().replace('\\', "/");
        targets.iter().map(|target| (rel(&target.path), target.range.start.line)).collect()
    }

    /// Goto from the caret of `source`, the text of the XML file `file`.
    fn mx_goes(&self, file: &str, source: &str) -> Vec<(String, u32)> {
        let (text, pos) = caret(source);
        let found = self.engine.magento_document_definition(&self.root.join(file), &text, pos);
        self.mx_places(found.unwrap_or_default())
    }

    fn mx_hover(&self, file: &str, source: &str) -> String {
        let (text, pos) = caret(source);
        let hover = self.engine.magento_document_hover(&self.root.join(file), &text, pos);
        hover.map(|hover| hover.markdown).unwrap_or_default()
    }

    fn mx_usages(&self, file: &str, source: &str) -> Vec<(String, u32)> {
        let (text, pos) = caret(source);
        let cancel = CancellationToken::new();
        let found = self.engine.magento_document_references(&self.root.join(file), &text, pos, true, &cancel);
        let mut found: Vec<(String, u32)> = found
            .expect("something there")
            .unwrap()
            .iter()
            .map(|usage| {
                let (file, line, _) = self.describe(&usage.path, usage.range);
                (file, line)
            })
            .collect();
        found.sort();
        found.dedup();
        found
    }

    fn mx_offered(&self, file: &str, source: &str) -> Vec<String> {
        let (text, pos) = caret(source);
        let found = self.engine.magento_document_completion(&self.root.join(file), &text, pos);
        found.unwrap_or_default().into_iter().map(|item| item.label).collect()
    }

    /// What an edit replaces, as `(file, line, replaced text, new text)`,
    /// and the files it moves.
    fn mx_edits(&self, edit: &crate::base::WorkspaceEdit) -> Edits {
        let rel = |path: &Path| path.strip_prefix(&self.root).unwrap_or(path).to_string_lossy().replace('\\', "/");
        let (mut edits, mut moves) = (Vec::new(), Vec::new());
        for file in &edit.files {
            match file {
                crate::base::FileEdit::Edit { path, edits: found, .. } => {
                    for edit in found {
                        let (file, line, old) = self.describe(path, edit.range);
                        edits.push((file, line, old, edit.new_text.clone()));
                    }
                }
                crate::base::FileEdit::Rename { from, to } => moves.push((rel(from), rel(to))),
                _ => {}
            }
        }
        edits.sort();
        moves.sort();
        (edits, moves)
    }

    /// A rename at the caret of `source`, the text of the PHP file `file`.
    fn mx_rename(&mut self, file: &str, source: &str, new_name: &str) -> Edits {
        let (text, pos) = caret(source);
        let doc = self.open(file, &text);
        let edit = self.engine.rename(doc, pos, new_name, &CancellationToken::new()).expect("renameable").unwrap();
        let found = self.mx_edits(&edit);
        self.engine.document_closed(doc);
        found
    }

    /// A rename at the caret of `source`, the text of the XML file `file`.
    fn mx_rename_xml(&self, file: &str, source: &str, new_name: &str) -> Edits {
        let (text, pos) = caret(source);
        let cancel = CancellationToken::new();
        let path = self.root.join(file);
        assert!(self.engine.magento_document_prepare_rename(&path, &text, pos).flatten().is_some(), "renameable");
        let edit = self.engine.magento_document_rename(&path, &text, pos, new_name, &cancel).expect("handled").unwrap();
        self.mx_edits(&edit)
    }

    fn mx_usages_php(&mut self, file: &str, source: &str) -> Vec<(String, u32)> {
        let (text, pos) = caret(source);
        let doc = self.open(file, &text);
        let usages = self.engine.references(doc, pos, false, &CancellationToken::new()).expect("handled").unwrap();
        let mut found: Vec<(String, u32)> = usages
            .iter()
            .map(|usage| {
                let (file, line, _) = self.describe(&usage.path, usage.range);
                (file, line)
            })
            .collect();
        self.engine.document_closed(doc);
        found.sort();
        found.dedup();
        found
    }

    fn mx_goes_php(&mut self, file: &str, source: &str) -> Vec<(String, u32)> {
        let found = self.definition(file, source).unwrap_or_default();
        found.into_iter().map(|(file, line, _)| (file, line)).collect()
    }

    fn mx_offered_php(&mut self, file: &str, source: &str) -> Vec<String> {
        let (text, pos) = caret(source);
        let doc = self.open(file, &text);
        let found = self.engine.completion(doc, pos, None, &CancellationToken::new()).expect("handled");
        self.engine.document_closed(doc);
        found.items.into_iter().map(|item| item.label).collect()
    }
}

fn has(found: &[String], names: &[&str]) {
    for name in names {
        assert!(found.contains(&(*name).to_owned()), "{name}: {found:?}");
    }
}

#[test]
fn an_xml_file_goes_to_what_it_names() {
    let project = shop();
    let di = module("etc/di.xml");
    let classes = module("classes.php");
    // A class, a virtual type where it is used, a plugin.
    let found = project
        .mx_goes(&di, &replaced(DI, "type=\"Shop\\Catalog\\Model\\Cart\"", "type=\"Shop\\Catalog\\Model\\Ca/*|*/rt\""));
    assert_eq!(found, [(classes.clone(), 7)]);
    let found = project.mx_goes(&di, &replaced(DI, ">ShopLogger<", ">Shop/*|*/Logger<"));
    assert_eq!(found, [(di.clone(), 3)]);
    let found = project.mx_goes(&di, &replaced(DI, "Plugin\\CartLimit", "Plugin\\Cart/*|*/Limit"));
    assert_eq!(found, [(classes.clone(), 22)]);
    // An observer's class, a service's method, a resolver's class written with doubled backslashes.
    let found =
        project.mx_goes(&module("etc/events.xml"), &replaced(EVENTS, "Observer\\Notify", "Observer\\Not/*|*/ify"));
    assert_eq!(found, [(classes.clone(), 28)]);
    let found =
        project.mx_goes(&module("etc/webapi.xml"), &replaced(WEBAPI, "method=\"total\"", "method=\"to/*|*/tal\""));
    assert_eq!(found, [(classes.clone(), 3)]);
    let found =
        project.mx_goes(&module("etc/schema.graphqls"), &replaced(GRAPHQL, "Model\\\\Cart", "Model\\\\Ca/*|*/rt"));
    assert_eq!(found, [(classes.clone(), 7)]);
    // Names: an ACL resource from `system.xml` and `webapi.xml`, a template, a block, a handle.
    let acl = vec![(module("etc/acl.xml"), 4)];
    let found = project.mx_goes(
        &module("etc/adminhtml/system.xml"),
        &replaced(SYSTEM, ">Shop_Catalog::cart<", ">Shop_Catalog::ca/*|*/rt<"),
    );
    assert_eq!(found, acl);
    let found = project.mx_goes(
        &module("etc/webapi.xml"),
        &replaced(WEBAPI, "ref=\"Shop_Catalog::cart\"", "ref=\"Shop_Catalog::ca/*|*/rt\""),
    );
    assert_eq!(found, acl);
    let layout = module("view/frontend/layout/shop_cart_index.xml");
    let default = module("view/frontend/layout/default.xml");
    // A template: the theme that inherits first, then its parent, then the module.
    let found =
        project.mx_goes(&layout, &replaced(LAYOUT, "Shop_Catalog::cart.phtml", "Shop_Catalog::ca/*|*/rt.phtml"));
    assert_eq!(
        found,
        [
            (format!("{CHILD_THEME}/Shop_Catalog/templates/cart.phtml"), 0),
            (format!("{BASE_THEME}/Shop_Catalog/templates/cart.phtml"), 0),
            (module("view/frontend/templates/cart.phtml"), 0),
        ]
    );
    // One written without its module is of the layout file's module.
    let found = project.mx_goes(&layout, &replaced(LAYOUT, "\"items.phtml\"", "\"it/*|*/ems.phtml\""));
    assert_eq!(found, [(module("view/frontend/templates/items.phtml"), 0)]);
    let found = project
        .mx_goes(&default, &replaced(DEFAULT_LAYOUT, "after=\"shop.cart.items\"", "after=\"shop.cart./*|*/items\""));
    assert_eq!(found, [(layout.clone(), 4)]);
    let found = project.mx_goes(
        &default,
        &replaced(DEFAULT_LAYOUT, "element=\"shop.cart.items\"", "element=\"shop.ca/*|*/rt.items\""),
    );
    assert_eq!(found, [(layout.clone(), 4)]);
    let found = project
        .mx_goes(&default, &replaced(DEFAULT_LAYOUT, "handle=\"shop_cart_index\"", "handle=\"shop_ca/*|*/rt_index\""));
    assert_eq!(found, [(layout.clone(), 1)]);

    // Hover: a class with its preference for the file's area and its plugins.
    let hover = project.mx_hover(
        &di,
        &replaced(DI, "for=\"Shop\\Catalog\\Api\\CartInterface\"", "for=\"Shop\\Catalog\\Api\\Cart/*|*/Interface\""),
    );
    assert!(hover.contains("Preference: `Shop\\Catalog\\Model\\Cart`"), "{hover}");
    let admin = module("etc/adminhtml/di.xml");
    let hover = project.mx_hover(&admin, &replaced(ADMIN_DI, "Api\\CartInterface", "Api\\Cart/*|*/Interface"));
    assert!(hover.contains("Preference: `Shop\\Catalog\\Model\\AdminCart`"), "{hover}");
    let hover = project.mx_hover(
        &di,
        &replaced(DI, "<type name=\"Shop\\Catalog\\Model\\Cart\"", "<type name=\"Shop\\Catalog\\Model\\Ca/*|*/rt\""),
    );
    assert!(hover.contains("Plugins: `Shop\\Catalog\\Plugin\\CartLimit`"), "{hover}");
    let hover = project.mx_hover(&di, &replaced(DI, ">ShopLogger<", ">Shop/*|*/Logger<"));
    assert!(hover.contains("virtual type") && hover.contains("Shop\\Catalog\\Model\\Logger"), "{hover}");
}

#[test]
fn an_xml_file_completes_what_a_place_takes() {
    let project = shop();
    let di = module("etc/di.xml");
    // The classes of a namespace, and the virtual types.
    let offered = project.mx_offered(
        &di,
        &replaced(DI, "type=\"Shop\\Catalog\\Model\\Cart\"/>", "type=\"Shop\\Catalog\\Model\\/*|*/\"/>"),
    );
    has(&offered, &["Shop\\Catalog\\Model\\Cart", "Shop\\Catalog\\Model\\Logger", "ShopLogger"]);
    // An empty place.
    let offered = project.mx_offered(
        &di,
        &replaced(
            DI,
            "<plugin name=\"limit\" type=\"Shop\\Catalog\\Plugin\\CartLimit\"/>",
            "<plugin name=\"limit\" type=\"/*|*/\"/>",
        ),
    );
    has(&offered, &["Shop", "ShopLogger"]);
    let layout = module("view/frontend/layout/shop_cart_index.xml");
    let offered = project.mx_offered(&layout, &replaced(LAYOUT, "template=\"items.phtml\"", "template=\"/*|*/\""));
    assert_eq!(offered, ["Shop_Catalog::cart.phtml", "Shop_Catalog::items.phtml"]);
    let default = module("view/frontend/layout/default.xml");
    let offered = project.mx_offered(
        &default,
        &replaced(DEFAULT_LAYOUT, "referenceBlock name=\"shop.cart\"", "referenceBlock name=\"/*|*/\""),
    );
    has(&offered, &["shop.cart", "shop.cart.items", "shop.badge"]);
    let offered =
        project.mx_offered(&module("etc/webapi.xml"), &replaced(WEBAPI, "ref=\"Shop_Catalog::cart\"", "ref=\"/*|*/\""));
    assert_eq!(offered, ["Shop_Catalog::cart"]);
}

#[test]
fn a_class_and_a_method_are_used_in_the_xml() {
    let mut project = shop();
    let classes = module("classes.php");
    // From PHP: the class's places in `di.xml`, the GraphQL schema.
    let found = project.mx_usages_php(
        &classes,
        &replaced(
            CLASSES,
            "class Cart extends \\Magento\\Framework\\DataObject",
            "class Ca/*|*/rt extends \\Magento\\Framework\\DataObject",
        ),
    );
    for expected in [
        (module("etc/di.xml"), 2),
        (module("etc/di.xml"), 4),
        (module("etc/schema.graphqls"), 1),
        (PROBE.to_owned(), 19),
    ] {
        assert!(found.contains(&expected), "{expected:?}: {found:?}");
    }
    // From the XML: the same, with the PHP.
    let found = project.mx_usages(
        &module("etc/di.xml"),
        &replaced(DI, "<type name=\"Shop\\Catalog\\Model\\Cart\"", "<type name=\"Shop\\Catalog\\Model\\Ca/*|*/rt\""),
    );
    for expected in
        [(classes.clone(), 7), (module("etc/di.xml"), 2), (module("etc/schema.graphqls"), 1), (PROBE.to_owned(), 19)]
    {
        assert!(found.contains(&expected), "{expected:?}: {found:?}");
    }
    // A method: the plugin intercepting it, the web API naming it.
    let found = project
        .mx_usages_php(&classes, &replaced(CLASSES, "public function save(int", "public function sa/*|*/ve(int"));
    assert_eq!(found, [(PROBE.to_owned(), 30), (classes.clone(), 23)]);
    let found = project.mx_usages_php(
        &classes,
        &replaced(
            CLASSES,
            "interface CartInterface {\n        public function total()",
            "interface CartInterface {\n        public function to/*|*/tal()",
        ),
    );
    for expected in [(classes.clone(), 24), (module("etc/webapi.xml"), 3)] {
        assert!(found.contains(&expected), "{expected:?}: {found:?}");
    }
    // Its hover says what intercepts it.
    let (text, pos) = caret(&replaced(SERVICE, "$this->cart->save();", "$this->cart->sa/*|*/ve();"));
    let doc = project.open(PROBE, &text);
    let hover = project.engine.hover(doc, pos).map(|hover| hover.markdown).unwrap_or_default();
    project.engine.document_closed(doc);
    assert!(hover.contains("Intercepted by `CartLimit::beforeSave()`"), "{hover}");
    // A virtual type's usages.
    let found = project.mx_usages(&module("etc/di.xml"), &replaced(DI, ">ShopLogger<", ">Shop/*|*/Logger<"));
    assert_eq!(found, [(module("etc/di.xml"), 3), (module("etc/di.xml"), 7)]);
}

#[test]
fn renaming_a_class_reaches_the_xml() {
    let mut project = shop();
    let classes = module("classes.php");
    let source = replaced(CLASSES, "class Logger {", "class Log/*|*/ger {");
    let (edits, _) = project.mx_rename(&classes, &source, "Journal");
    let xml: Vec<_> = edits.iter().filter(|(file, ..)| file.ends_with(".xml")).cloned().collect();
    assert_eq!(
        xml,
        [
            (module("etc/adminhtml/system.xml"), 7, "Logger".to_owned(), "Journal".to_owned()),
            (module("etc/di.xml"), 3, "Logger".to_owned(), "Journal".to_owned()),
        ]
    );
    // From the XML: the class's rename.
    let (edits, _) =
        project.mx_rename_xml(&module("etc/di.xml"), &replaced(DI, "Model\\Logger", "Model\\Log/*|*/ger"), "Journal");
    assert!(edits.contains(&(classes.clone(), 12, "Logger".to_owned(), "Journal".to_owned())), "{edits:?}");
    assert!(
        edits.contains(&(module("etc/adminhtml/system.xml"), 7, "Logger".to_owned(), "Journal".to_owned())),
        "{edits:?}"
    );
    // A virtual type's rename.
    let (edits, _) =
        project.mx_rename_xml(&module("etc/di.xml"), &replaced(DI, ">ShopLogger<", ">Shop/*|*/Logger<"), "ShopJournal");
    assert_eq!(
        edits,
        [
            (module("etc/di.xml"), 3, "ShopLogger".to_owned(), "ShopJournal".to_owned()),
            (module("etc/di.xml"), 7, "ShopLogger".to_owned(), "ShopJournal".to_owned()),
        ]
    );
}

#[test]
fn moving_a_class_file_reaches_the_xml() {
    let project = Project::new(&[
        ("app/etc/di.xml", "<config/>\n"),
        ("composer.json", r#"{"autoload": {"psr-4": {"Shop\\Catalog\\": "app/code/Shop/Catalog/"}}}"#),
        ("app/code/Shop/Catalog/registration.php", REGISTRATION),
        ("app/code/Shop/Catalog/Model/Logger.php", "<?php\nnamespace Shop\\Catalog\\Model;\n\nclass Logger {}\n"),
        (
            "app/code/Shop/Catalog/etc/di.xml",
            "<config>\n    <virtualType name=\"ShopLogger\" type=\"\\Shop\\Catalog\\Model\\Logger\"/>\n</config>\n",
        ),
        (
            "app/code/Shop/Catalog/etc/schema.graphqls",
            "type Query {\n    log: Log @resolver(class: \"Shop\\\\Catalog\\\\Model\\\\Logger\")\n}\n",
        ),
    ]);
    let moves = vec![(
        project.root.join("app/code/Shop/Catalog/Model/Logger.php"),
        project.root.join("app/code/Shop/Catalog/Service/Logger.php"),
    )];
    let edit = project.engine.file_moves_edit(&moves, &CancellationToken::new()).unwrap();
    let (edits, _) = project.mx_edits(&edit);
    let other: Vec<_> = edits.iter().filter(|(file, ..)| !file.ends_with(".php")).cloned().collect();
    assert_eq!(
        other,
        [
            (
                "app/code/Shop/Catalog/etc/di.xml".to_owned(),
                1,
                "\\Shop\\Catalog\\Model\\Logger".to_owned(),
                "\\Shop\\Catalog\\Service\\Logger".to_owned()
            ),
            (
                "app/code/Shop/Catalog/etc/schema.graphqls".to_owned(),
                1,
                "Shop\\\\Catalog\\\\Model\\\\Logger".to_owned(),
                "Shop\\\\Catalog\\\\Service\\\\Logger".to_owned()
            ),
        ]
    );
}

#[test]
fn magento_names_are_renamed_in_php_and_in_xml() {
    let mut project = shop();
    // An event.
    let (edits, moves) = project.mx_rename(
        PROBE,
        &replaced(SERVICE, "dispatch('shop_cart_saved')", "dispatch('shop_cart/*|*/_saved')"),
        "shop_cart_stored",
    );
    assert_eq!(
        edits,
        [
            (PROBE.to_owned(), 24, "shop_cart_saved".to_owned(), "shop_cart_stored".to_owned()),
            (module("etc/events.xml"), 2, "shop_cart_saved".to_owned(), "shop_cart_stored".to_owned()),
        ]
    );
    assert!(moves.is_empty());
    // The same from the XML.
    let (from_xml, _) = project.mx_rename_xml(
        &module("etc/events.xml"),
        &replaced(EVENTS, "name=\"shop_cart_saved\"", "name=\"shop_ca/*|*/rt_saved\""),
        "shop_cart_stored",
    );
    assert_eq!(from_xml, edits);
    // A block: where it is declared, referred to, moved and placed after.
    let (edits, _) = project.mx_rename(
        PROBE,
        &replaced(SERVICE, "getBlock('shop.cart.items')", "getBlock('shop.cart./*|*/items')"),
        "shop.cart.lines",
    );
    let places: Vec<(String, u32)> = edits.iter().map(|(file, line, ..)| (file.clone(), *line)).collect();
    assert_eq!(
        places,
        [
            (PROBE.to_owned(), 27),
            (module("view/frontend/layout/default.xml"), 5),
            (module("view/frontend/layout/default.xml"), 7),
            (module("view/frontend/layout/shop_cart_index.xml"), 4),
        ]
    );
    assert!(edits.iter().all(|(_, _, old, new)| old == "shop.cart.items" && new == "shop.cart.lines"), "{edits:?}");
    // An ACL resource: the part after its module, in every file naming it.
    let (edits, _) = project.mx_rename(
        PROBE,
        &replaced(SERVICE, "isAllowed('Shop_Catalog::cart')", "isAllowed('Shop_Catalog::ca/*|*/rt')"),
        "basket",
    );
    assert_eq!(
        edits,
        [
            (PROBE.to_owned(), 28, "cart".to_owned(), "basket".to_owned()),
            (module("etc/acl.xml"), 4, "cart".to_owned(), "basket".to_owned()),
            (module("etc/adminhtml/system.xml"), 4, "cart".to_owned(), "basket".to_owned()),
            (module("etc/webapi.xml"), 5, "cart".to_owned(), "basket".to_owned()),
        ]
    );
    // A template: its files move, in the module and in the themes.
    let (edits, moves) = project.mx_rename(
        PROBE,
        &replaced(SERVICE, "setTemplate('Shop_Catalog::cart.phtml')", "setTemplate('Shop_Catalog::ca/*|*/rt.phtml')"),
        "basket.phtml",
    );
    assert_eq!(
        edits,
        [
            (PROBE.to_owned(), 29, "cart.phtml".to_owned(), "basket.phtml".to_owned()),
            (module("view/frontend/layout/shop_cart_index.xml"), 3, "cart.phtml".to_owned(), "basket.phtml".to_owned()),
        ]
    );
    assert_eq!(
        moves,
        [
            (module("view/frontend/templates/cart.phtml"), module("view/frontend/templates/basket.phtml")),
            (
                format!("{BASE_THEME}/Shop_Catalog/templates/cart.phtml"),
                format!("{BASE_THEME}/Shop_Catalog/templates/basket.phtml")
            ),
            (
                format!("{CHILD_THEME}/Shop_Catalog/templates/cart.phtml"),
                format!("{CHILD_THEME}/Shop_Catalog/templates/basket.phtml")
            ),
        ]
    );
    // One written without its module, from the layout file.
    let layout = module("view/frontend/layout/shop_cart_index.xml");
    let (edits, moves) =
        project.mx_rename_xml(&layout, &replaced(LAYOUT, "\"items.phtml\"", "\"it/*|*/ems.phtml\""), "lines.phtml");
    assert_eq!(edits, [(layout.clone(), 4, "items.phtml".to_owned(), "lines.phtml".to_owned())]);
    assert_eq!(moves, [(module("view/frontend/templates/items.phtml"), module("view/frontend/templates/lines.phtml"))]);
}

#[test]
fn routes_tables_and_columns_are_names() {
    let mut project = shop();
    let classes = module("classes.php");
    // A URL path: its action's class; its front name: the route.
    let found =
        project.mx_goes_php(PROBE, &replaced(SERVICE, "getUrl('shop/cart/index')", "getUrl('shop/cart/in/*|*/dex')"));
    let controller = module("Controller/Cart/Index.php");
    assert_eq!(found, [(controller.clone(), 3)]);
    let found =
        project.mx_goes_php(PROBE, &replaced(SERVICE, "getUrl('shop/cart/index')", "getUrl('sh/*|*/op/cart/index')"));
    assert_eq!(found, [(module("etc/frontend/routes.xml"), 3)]);
    // The parts left out are `index`.
    let found = project
        .mx_goes_php(&controller, &replaced(CONTROLLER, "_redirect('shop/cart')", "_redirect('shop/ca/*|*/rt')"));
    assert_eq!(found, [(controller.clone(), 3)]);
    let offered = project.mx_offered_php(PROBE, &replaced(SERVICE, "getUrl('shop/cart/index')", "getUrl('/*|*/')"));
    assert_eq!(offered, ["shop/cart/index"]);
    // A table and a column of it.
    let schema = module("etc/db_schema.xml");
    let found =
        project.mx_goes_php(PROBE, &replaced(SERVICE, "getTableName('shop_cart')", "getTableName('shop_/*|*/cart')"));
    assert_eq!(found, [(schema.clone(), 2)]);
    let found = project.mx_goes_php(
        &classes,
        &replaced(CLASSES, "_init('shop_cart', 'cart_id')", "_init('shop_cart', 'cart/*|*/_id')"),
    );
    assert_eq!(found, [(schema.clone(), 3)]);
    let offered = project
        .mx_offered_php(&classes, &replaced(CLASSES, "_init('shop_cart', 'cart_id')", "_init('shop_cart', '/*|*/')"));
    assert_eq!(offered, ["cart_id", "total"]);
    let found =
        project.mx_usages_php(PROBE, &replaced(SERVICE, "getTableName('shop_cart')", "getTableName('shop_/*|*/cart')"));
    assert_eq!(found, [(PROBE.to_owned(), 26), (classes.clone(), 18), (schema.clone(), 2)]);
}

#[test]
fn what_an_xml_file_names_that_isnt_there_is_reported() {
    let project = shop();
    let problems = |file: &str, text: &str| -> Vec<String> {
        let found = project.engine.magento_document_problems(&project.root.join(file), text).expect("a document");
        found.into_iter().map(|problem| format!("{} {}", problem.range.start.line, problem.message)).collect()
    };
    let di = module("etc/di.xml");
    assert_eq!(problems(&di, DI), Vec::<String>::new());
    let layout = module("view/frontend/layout/shop_cart_index.xml");
    assert_eq!(problems(&layout, LAYOUT), Vec::<String>::new());
    assert_eq!(problems(&module("etc/schema.graphqls"), GRAPHQL), Vec::<String>::new());
    // A class of the project's module that isn't there; another vendor's may be anywhere.
    let text = replaced(DI, "Plugin\\CartLimit", "Plugin\\Gone")
        .replace("Shop\\Catalog\\Model\\Logger", "Other\\Module\\Model\\Logger");
    assert_eq!(problems(&di, &text), ["5 Undefined class 'Shop\\Catalog\\Plugin\\Gone'"]);
    // A generated class is there.
    let text = replaced(DI, "type=\"Shop\\Catalog\\Model\\Cart\"/>", "type=\"Shop\\Catalog\\Model\\Cart\\Proxy\"/>");
    assert_eq!(problems(&di, &text), Vec::<String>::new());
    let text = replaced(LAYOUT, "\"items.phtml\"", "\"gone.phtml\"");
    assert_eq!(problems(&layout, &text), ["4 Unknown template 'gone.phtml'"]);
    // Not a file the model reads.
    assert!(project.engine.magento_document_problems(&project.root.join("phpunit.xml"), "<phpunit/>").is_none());
}
