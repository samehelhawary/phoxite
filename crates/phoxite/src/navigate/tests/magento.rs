//! Magento 2, on a stand-in for its API written for these tests (the names
//! of its classes and methods, nothing of its code) and a small module.

use super::*;

/// The framework's classes.
const FRAMEWORK: &str = r#"<?php
namespace Magento\Framework {
    interface ObjectManagerInterface {
        public function create($type, array $arguments = []);
        public function get($type);
    }
    interface AuthorizationInterface {
        public function isAllowed($resource, $privilege = null);
    }
    class DataObject {
        public function getData($key = '', $index = null) {}
        public function setData($key, $value = null) {}
        public function __call($method, $args) {}
    }
    class Escaper {
        public function escapeHtml($data, $allowedTags = null) {}
    }
}
namespace Magento\Framework\Event {
    interface ManagerInterface {
        public function dispatch($eventName, array $data = []);
    }
}
namespace Magento\Framework\App\Config {
    interface ScopeConfigInterface {
        public function getValue($path, $scopeType = 'default', $scopeCode = null);
        public function isSetFlag($path, $scopeType = 'default', $scopeCode = null);
    }
}
namespace Magento\Framework\Module {
    class Manager {
        public function isEnabled($moduleName) {}
    }
}
namespace Magento\Framework\Component {
    class ComponentRegistrar {
        const MODULE = 'module';
        public static function register($type, $componentName, $path) {}
    }
}
namespace Magento\Framework\Api {
    interface ExtensionAttributesInterface {}
    abstract class AbstractSimpleObject {}
}
namespace Magento\Framework\View {
    interface LayoutInterface {
        public function getBlock($name);
        public function getChildBlock($parentName, $alias);
    }
}
namespace Magento\Framework\View\Layout {
    interface ProcessorInterface {
        public function addHandle($handleName);
    }
}
namespace Magento\Framework\View\TemplateEngine {
    class Php {
        public function render($block, $fileName, array $dictionary = []) {}
    }
}
namespace Magento\Framework\View\Helper {
    class SecureHtmlRenderer {
        public function renderTag(string $tagName, array $attributes, ?string $content = null): string {}
    }
}
namespace Magento\Framework\View\Element {
    abstract class AbstractBlock extends \Magento\Framework\DataObject {
        public function getLayout() {}
        public function getChildBlock($alias) {}
        public function getChildHtml($alias = '', $useCache = true) {}
    }
    class Template extends AbstractBlock {
        protected $_template;
        public function setTemplate($template) {}
        public function getTemplate() {}
    }
}
namespace Magento\Backend\App {
    abstract class AbstractAction {
        const ADMIN_RESOURCE = 'Magento_Backend::admin';
    }
    abstract class Action extends AbstractAction {}
}
"#;

const REGISTRATION: &str = "<?php\nuse Magento\\Framework\\Component\\ComponentRegistrar;\n\nComponentRegistrar::register(ComponentRegistrar::MODULE, 'Shop_Catalog', __DIR__);\n";

const DI: &str = r#"<?xml version="1.0"?>
<config>
    <preference for="Shop\Catalog\Api\CartInterface" type="Shop\Catalog\Model\Cart"/>
    <virtualType name="ShopLogger" type="Shop\Catalog\Model\Logger"/>
    <type name="Shop\Catalog\Model\Cart">
        <plugin name="limit" type="Shop\Catalog\Plugin\CartLimit"/>
    </type>
</config>
"#;

const EVENTS: &str = r#"<?xml version="1.0"?>
<config>
    <event name="shop_cart_saved">
        <observer name="notify" instance="Shop\Catalog\Observer\Notify"/>
    </event>
    <event name="shop_cart_emptied">
        <observer name="notify" instance="Shop\Catalog\Observer\Notify"/>
    </event>
</config>
"#;

const ACL: &str = r#"<?xml version="1.0"?>
<config>
    <acl>
        <resources>
            <resource id="Magento_Backend::admin">
                <resource id="Shop_Catalog::cart" title="Carts"/>
                <resource id="Shop_Catalog::cart_edit" title="Edit carts"/>
            </resource>
        </resources>
    </acl>
</config>
"#;

const SYSTEM: &str = r#"<?xml version="1.0"?>
<config>
    <system>
        <section id="shop">
            <group id="cart">
                <field id="limit"><label>Cart limit</label></field>
                <field id="enabled"/>
            </group>
        </section>
    </system>
</config>
"#;

const CONFIG: &str = r#"<?xml version="1.0"?>
<config>
    <default>
        <shop>
            <cart>
                <limit>5</limit>
            </cart>
        </shop>
    </default>
</config>
"#;

const EXTENSION: &str = r#"<?xml version="1.0"?>
<config>
    <extension_attributes for="Shop\Catalog\Api\Data\ItemInterface">
        <attribute code="gift_note" type="string"/>
        <attribute code="logger" type="Shop\Catalog\Model\Logger"/>
    </extension_attributes>
</config>
"#;

const LAYOUT: &str = r#"<?xml version="1.0"?>
<page>
    <body>
        <referenceContainer name="content">
            <block class="Shop\Catalog\Block\Cart" name="shop.cart" template="Shop_Catalog::cart.phtml">
                <arguments>
                    <argument name="view_model" xsi:type="object">Shop\Catalog\ViewModel\Totals</argument>
                </arguments>
                <block class="Shop\Catalog\Block\Items" name="shop.cart.items" as="cart.items" template="Shop_Catalog::items.phtml"/>
            </block>
        </referenceContainer>
    </body>
</page>
"#;

const CLASSES: &str = r#"<?php
namespace Shop\Catalog\Api {
    interface CartInterface {
        public function total(): float;
    }
}
namespace Shop\Catalog\Api\Data {
    interface ItemInterface {
        /** @return \Shop\Catalog\Api\Data\ItemExtensionInterface|null */
        public function getExtensionAttributes();
    }
}
namespace Shop\Catalog\Model {
    class Cart extends \Magento\Framework\DataObject implements \Shop\Catalog\Api\CartInterface {
        public function total(): float { return 0.0; }
        public function save(int $times = 1): bool { return true; }
    }
    class Logger {
        public function log(string $line): void {}
    }
}
namespace Shop\Catalog\Plugin {
    class CartLimit {
        public function beforeSave($subject, int $times = 1) { return [$times]; }
        public function afterTotal($subject, $result) { return $result; }
        public function aroundTotal($subject, callable $proceed) { return $proceed(); }
        public function limit(): int { return 5; }
    }
}
namespace Shop\Catalog\ViewModel {
    class Totals {
        public function grand(): float { return 0.0; }
    }
}
namespace Shop\Catalog\Block {
    class Cart extends \Magento\Framework\View\Element\Template {
        protected $_template = 'Shop_Catalog::cart.phtml';
        public function lines(): array { return []; }
    }
    class Items extends \Magento\Framework\View\Element\Template {
        public function count(): int { return 0; }
    }
}
namespace Shop\Catalog\Controller\Adminhtml\Cart {
    class Index extends \Magento\Backend\App\Action {
        const ADMIN_RESOURCE = 'Shop_Catalog::cart';
    }
}
"#;

const SERVICE: &str = r#"<?php
namespace Shop\Catalog\Model;

use Magento\Framework\App\Config\ScopeConfigInterface;
use Magento\Framework\AuthorizationInterface;
use Magento\Framework\Event\ManagerInterface;
use Magento\Framework\Module\Manager;
use Magento\Framework\ObjectManagerInterface;
use Magento\Framework\View\LayoutInterface;
use Shop\Catalog\Api\Data\ItemInterface;
use Shop\Catalog\Block\Cart as CartBlock;

class Service
{
    public function __construct(
        private ManagerInterface $events,
        private ScopeConfigInterface $config,
        private AuthorizationInterface $authorization,
        private Manager $modules,
        private ObjectManagerInterface $objects,
        private LayoutInterface $layout,
        private CartBlock $block,
        private CartFactory $cartFactory,
        private ItemInterface $item,
    ) {}

    public function run()
    {
        $this->events->dispatch('shop_cart_saved', ['cart' => 1]);
        $limit = $this->config->getValue('shop/cart/limit');
        $allowed = $this->authorization->isAllowed('Shop_Catalog::cart');
        $enabled = $this->modules->isEnabled('Shop_Catalog');
        $this->block->setTemplate('Shop_Catalog::cart.phtml');
        $this->layout->getBlock('shop.cart');
        $this->block->getChildHtml('cart.items');
        $this->probe();
    }
}
"#;

const MODULE: &str = "app/code/Shop/Catalog";
const PROBE: &str = "app/code/Shop/Catalog/Model/Service.php";
const CART_TEMPLATE: &str = "app/code/Shop/Catalog/view/frontend/templates/cart.phtml";

fn shop() -> Project {
    let file = |rel: &str| format!("{MODULE}/{rel}");
    let files: Vec<(String, &str)> = vec![
        ("app/etc/di.xml".to_owned(), "<config/>\n"),
        ("framework/classes.php".to_owned(), FRAMEWORK),
        (file("registration.php"), REGISTRATION),
        (file("etc/module.xml"), "<config><module name=\"Shop_Catalog\"/></config>\n"),
        (file("etc/di.xml"), DI),
        (file("etc/events.xml"), EVENTS),
        (file("etc/acl.xml"), ACL),
        (file("etc/adminhtml/system.xml"), SYSTEM),
        (file("etc/config.xml"), CONFIG),
        (file("etc/extension_attributes.xml"), EXTENSION),
        (file("view/frontend/layout/shop_cart_index.xml"), LAYOUT),
        (CART_TEMPLATE.to_owned(), "<p><?= $block->getChildHtml('cart.items') ?></p>\n"),
        (file("view/frontend/templates/items.phtml"), "<p></p>\n"),
        (file("classes.php"), CLASSES),
        (PROBE.to_owned(), SERVICE),
    ];
    let files: Vec<(&str, &str)> = files.iter().map(|(rel, text)| (rel.as_str(), *text)).collect();
    Project::new(&files)
}

fn at(from: &str, to: &str) -> String {
    assert!(SERVICE.contains(from), "{from}");
    SERVICE.replacen(from, to, 1)
}

impl Project {
    fn magento_goes(&mut self, file: &str, source: &str) -> Vec<(String, u32)> {
        let found = self.definition(file, source).unwrap_or_default();
        found.into_iter().map(|(file, line, _)| (file, line)).collect()
    }

    fn magento_complete(&mut self, file: &str, source: &str) -> Vec<String> {
        let (text, pos) = caret(source);
        let doc = self.open(file, &text);
        let found = self.engine.completion(doc, pos, None, &CancellationToken::new()).expect("handled");
        self.engine.document_closed(doc);
        found.items.into_iter().map(|item| item.label).collect()
    }

    fn magento_hover(&mut self, file: &str, source: &str) -> String {
        let (text, pos) = caret(source);
        let doc = self.open(file, &text);
        let hover = self.engine.hover(doc, pos);
        self.engine.document_closed(doc);
        hover.map(|hover| hover.markdown).unwrap_or_default()
    }

    /// The problems of `source` whose inspection's name has `of` in it.
    fn magento_problems(&mut self, file: &str, source: &str, of: &str) -> Vec<String> {
        let doc = self.open(file, source);
        let inspected = self.engine.inspect(doc, &CancellationToken::new()).expect("inspected");
        self.engine.document_closed(doc);
        let found = inspected.problems.iter().filter(|problem| problem.inspection.contains(of));
        found.map(|problem| problem.message.clone()).collect()
    }

    fn magento_usages(&mut self, file: &str, source: &str) -> Vec<(String, u32)> {
        let (text, pos) = caret(source);
        let doc = self.open(file, &text);
        let usages = self.engine.references(doc, pos, true, &CancellationToken::new()).expect("handled").unwrap();
        let mut found: Vec<(String, u32)> = usages
            .iter()
            .map(|usage| {
                let (file, line, _) = self.describe(&usage.path, usage.range);
                (file, line)
            })
            .collect();
        self.engine.document_closed(doc);
        found.sort();
        found
    }

    /// The members offered after `body`, a statement of the probe's method.
    fn magento_members(&mut self, body: &str) -> Vec<String> {
        let source = at("$this->probe();", body);
        self.magento_complete(PROBE, &source)
    }
}

fn has(found: &[String], names: &[&str]) {
    for name in names {
        assert!(found.contains(&(*name).to_owned()), "{name}: {found:?}");
    }
}

fn lacks(found: &[String], names: &[&str]) {
    for name in names {
        assert!(!found.contains(&(*name).to_owned()), "{name}: {found:?}");
    }
}

#[test]
fn magento_names_go_to_the_xml_that_declares_them() {
    let mut project = shop();
    assert!(project.engine.frameworks().contains(crate::frameworks::Framework::Magento));
    let mut goes = |from: &str, to: &str| project.magento_goes(PROBE, &at(from, to));
    let file = |name: &str, line: u32| vec![(format!("{MODULE}/{name}"), line)];
    assert_eq!(goes("dispatch('shop_cart_saved'", "dispatch('shop_cart/*|*/_saved'"), file("etc/events.xml", 2));
    // A configuration path: its default and its field.
    assert_eq!(
        goes("getValue('shop/cart/limit')", "getValue('shop/cart/li/*|*/mit')"),
        [(format!("{MODULE}/etc/config.xml"), 5), (format!("{MODULE}/etc/adminhtml/system.xml"), 5)]
    );
    assert_eq!(goes("isAllowed('Shop_Catalog::cart')", "isAllowed('Shop_Catalog::ca/*|*/rt')"), file("etc/acl.xml", 5));
    assert_eq!(goes("isEnabled('Shop_Catalog')", "isEnabled('Shop_Cat/*|*/alog')"), file("registration.php", 3));
    assert_eq!(
        goes("setTemplate('Shop_Catalog::cart.phtml')", "setTemplate('Shop_Catalog::ca/*|*/rt.phtml')"),
        file("view/frontend/templates/cart.phtml", 0)
    );
    // A template named without its module is of the module naming it.
    assert_eq!(
        goes("setTemplate('Shop_Catalog::cart.phtml')", "setTemplate('ite/*|*/ms.phtml')"),
        file("view/frontend/templates/items.phtml", 0)
    );
    // A block by its name, a child by its alias.
    let layout = "view/frontend/layout/shop_cart_index.xml";
    assert_eq!(goes("getBlock('shop.cart')", "getBlock('shop./*|*/cart')"), file(layout, 4));
    assert_eq!(goes("getChildHtml('cart.items')", "getChildHtml('cart.it/*|*/ems')"), file(layout, 8));
    // In a class: a backend action's resource, a block's template.
    let classes = format!("{MODULE}/classes.php");
    let source = CLASSES.replacen(
        "const ADMIN_RESOURCE = 'Shop_Catalog::cart'",
        "const ADMIN_RESOURCE = 'Shop_Catalog::ca/*|*/rt'",
        1,
    );
    assert_eq!(project.magento_goes(&classes, &source), file("etc/acl.xml", 5));
    let source =
        CLASSES.replacen("$_template = 'Shop_Catalog::cart.phtml'", "$_template = 'Shop_Catalog::ca/*|*/rt.phtml'", 1);
    assert_eq!(project.magento_goes(&classes, &source), file("view/frontend/templates/cart.phtml", 0));
    // In a template.
    let found = project.magento_goes(CART_TEMPLATE, "<p><?= $block->getChildHtml('cart.it/*|*/ems') ?></p>\n");
    assert_eq!(found, file(layout, 8));

    let hover = project.magento_hover(PROBE, &at("dispatch('shop_cart_saved'", "dispatch('shop_cart/*|*/_saved'"));
    assert!(hover.contains("**event** `shop_cart_saved`") && hover.contains("Observer\\Notify"), "{hover}");
    let hover =
        project.magento_hover(PROBE, &at("isAllowed('Shop_Catalog::cart')", "isAllowed('Shop_Catalog::ca/*|*/rt')"));
    assert!(hover.contains("Carts") && hover.contains("etc/acl.xml:6"), "{hover}");
    let hover = project.magento_hover(PROBE, &at("getBlock('shop.cart')", "getBlock('shop./*|*/cart')"));
    assert!(hover.contains("Shop\\Catalog\\Block\\Cart"), "{hover}");
}

#[test]
fn magento_names_complete_by_what_is_asked_for() {
    let mut project = shop();
    let mut offered = |from: &str, to: &str| project.magento_complete(PROBE, &at(from, to));
    assert_eq!(offered("dispatch('shop_cart_saved'", "dispatch('/*|*/'"), ["shop_cart_emptied", "shop_cart_saved"]);
    assert_eq!(offered("getValue('shop/cart/limit')", "getValue('/*|*/')"), ["shop/cart/enabled", "shop/cart/limit"]);
    assert_eq!(
        offered("isAllowed('Shop_Catalog::cart')", "isAllowed('/*|*/')"),
        ["Magento_Backend::admin", "Shop_Catalog::cart", "Shop_Catalog::cart_edit"]
    );
    assert_eq!(offered("isEnabled('Shop_Catalog')", "isEnabled('/*|*/')"), ["Shop_Catalog"]);
    assert_eq!(
        offered("setTemplate('Shop_Catalog::cart.phtml')", "setTemplate('/*|*/')"),
        ["Shop_Catalog::cart.phtml", "Shop_Catalog::items.phtml"]
    );
    assert_eq!(offered("getBlock('shop.cart')", "getBlock('/*|*/')"), ["cart.items", "shop.cart", "shop.cart.items"]);
}

#[test]
fn magento_names_that_name_nothing_are_reported() {
    let mut project = shop();
    let classes = format!("{MODULE}/classes.php");
    assert_eq!(project.magento_problems(PROBE, SERVICE, "Magento"), Vec::<String>::new());
    assert_eq!(project.magento_problems(&classes, CLASSES, "Magento"), Vec::<String>::new());
    // An event and a configuration path may be used undeclared; so may what another
    // vendor's modules have.
    let source = at("dispatch('shop_cart_saved'", "dispatch('shop_anything'")
        .replacen("getValue('shop/cart/limit')", "getValue('shop/any/thing')", 1)
        .replacen("isAllowed('Shop_Catalog::cart')", "isAllowed('Other_Module::thing')", 1)
        .replacen("isEnabled('Shop_Catalog')", "isEnabled('Other_Module')", 1)
        .replacen("setTemplate('Shop_Catalog::cart.phtml')", "setTemplate('Other_Module::x.phtml')", 1);
    assert_eq!(project.magento_problems(PROBE, &source, "Magento"), Vec::<String>::new());
    let source = at("isAllowed('Shop_Catalog::cart')", "isAllowed('Shop_Catalog::nothing')")
        .replacen("isEnabled('Shop_Catalog')", "isEnabled('Shop_Missing')", 1)
        .replacen("setTemplate('Shop_Catalog::cart.phtml')", "setTemplate('Shop_Catalog::gone.phtml')", 1);
    assert_eq!(
        project.magento_problems(PROBE, &source, "Magento"),
        [
            "Unknown ACL resource 'Shop_Catalog::nothing'",
            "Unknown module 'Shop_Missing'",
            "Unknown template 'Shop_Catalog::gone.phtml'"
        ]
    );
    let source = CLASSES.replacen(
        "const ADMIN_RESOURCE = 'Shop_Catalog::cart'",
        "const ADMIN_RESOURCE = 'Shop_Catalog::lost'",
        1,
    );
    assert_eq!(project.magento_problems(&classes, &source, "Magento"), ["Unknown ACL resource 'Shop_Catalog::lost'"]);
}

#[test]
fn the_xml_declarations_are_usages() {
    let mut project = shop();
    let found = project.magento_usages(PROBE, &at("dispatch('shop_cart_saved'", "dispatch('shop_cart/*|*/_saved'"));
    assert_eq!(found, [(PROBE.to_owned(), 28), (format!("{MODULE}/etc/events.xml"), 2)]);
    let found =
        project.magento_usages(PROBE, &at("isAllowed('Shop_Catalog::cart')", "isAllowed('Shop_Catalog::ca/*|*/rt')"));
    assert_eq!(
        found,
        [(PROBE.to_owned(), 30), (format!("{MODULE}/classes.php"), 45), (format!("{MODULE}/etc/acl.xml"), 5)]
    );
    // A template: where the layout gives it to a block, where a class names it, its file.
    let found = project.magento_usages(
        PROBE,
        &at("setTemplate('Shop_Catalog::cart.phtml')", "setTemplate('Shop_Catalog::ca/*|*/rt.phtml')"),
    );
    assert_eq!(
        found,
        [
            (PROBE.to_owned(), 32),
            (format!("{MODULE}/classes.php"), 36),
            (format!("{MODULE}/view/frontend/layout/shop_cart_index.xml"), 4),
            (CART_TEMPLATE.to_owned(), 0),
        ]
    );
}

#[test]
fn generated_classes_are_classes() {
    let mut project = shop();
    // A factory of a class the project has is not an undefined class; one of nothing is.
    assert_eq!(project.magento_problems(PROBE, SERVICE, "PhpUndefinedClass"), Vec::<String>::new());
    let source = at("private CartFactory $cartFactory,", "private NothingFactory $cartFactory,");
    assert_eq!(project.magento_problems(PROBE, &source, "PhpUndefinedClass").len(), 1);
    // What a factory makes; a proxy and an interceptor are the class.
    has(&project.magento_members("$this->cartFactory->create()->/*|*/;"), &["total", "save"]);
    has(&project.magento_members("$proxy = new Cart\\Proxy(); $proxy->/*|*/;"), &["total", "save"]);
    has(
        &project.magento_members("$made = new \\Shop\\Catalog\\Api\\CartInterfaceFactory(); $made->create()->/*|*/;"),
        &["total"],
    );
    // Extension attributes: a getter and a setter each, with the declared type.
    let members = project.magento_members("$this->item->getExtensionAttributes()->/*|*/;");
    has(&members, &["getGiftNote", "setGiftNote", "getLogger", "setLogger"]);
    has(&project.magento_members("$this->item->getExtensionAttributes()->getLogger()->/*|*/;"), &["log"]);
    let hover = project.magento_hover(
        PROBE,
        &at("$this->probe();", "$note = $this->item->getExtensionAttributes()->getGiftNote(); echo $no/*|*/te;"),
    );
    assert!(hover.contains("string"), "{hover}");
}

#[test]
fn the_object_manager_and_data_objects_are_typed() {
    let mut project = shop();
    has(&project.magento_members("$this->objects->get(Cart::class)->/*|*/;"), &["total", "save"]);
    has(&project.magento_members("$this->objects->create('Shop\\\\Catalog\\\\Model\\\\Logger')->/*|*/;"), &["log"]);
    // A virtual type is the class its `di.xml` gives it.
    has(&project.magento_members("$this->objects->get('ShopLogger')->/*|*/;"), &["log"]);
    // A data object's setters return it.
    has(&project.magento_members("$cart = new Cart(); $cart->setName('x')->unsNote()->/*|*/;"), &["total", "save"]);
    let hover = project
        .magento_hover(PROBE, &at("$this->probe();", "$cart = new Cart(); $has = $cart->hasName(); echo $h/*|*/as;"));
    assert!(hover.contains("bool"), "{hover}");
    // The view model the layout gives a block.
    has(&project.magento_members("$this->block->getViewModel()->/*|*/;"), &["grand"]);
    has(&project.magento_members("$this->block->getData('view_model')->/*|*/;"), &["grand"]);
}

#[test]
fn a_template_has_its_block() {
    let mut project = shop();
    has(
        &project.magento_complete(CART_TEMPLATE, "<?php $block->/*|*/ ?>\n"),
        &["lines", "getChildHtml", "setTemplate"],
    );
    has(&project.magento_complete(CART_TEMPLATE, "<?php $viewModel->/*|*/ ?>\n"), &["grand"]);
    has(&project.magento_complete(CART_TEMPLATE, "<?php $escaper->/*|*/ ?>\n"), &["escapeHtml"]);
    has(&project.magento_complete(CART_TEMPLATE, "<?php $secureRenderer->/*|*/ ?>\n"), &["renderTag"]);
    has(&project.magento_complete(CART_TEMPLATE, "<?php $this->/*|*/ ?>\n"), &["render"]);
    // Another template has its own block; one the layout doesn't name has a template block.
    let items = format!("{MODULE}/view/frontend/templates/items.phtml");
    let members = project.magento_complete(&items, "<?php $block->/*|*/ ?>\n");
    has(&members, &["count"]);
    lacks(&members, &["lines"]);
    let other = format!("{MODULE}/view/frontend/templates/other.phtml");
    let members = project.magento_complete(&other, "<?php $block->/*|*/ ?>\n");
    has(&members, &["setTemplate"]);
    lacks(&members, &["lines", "count"]);
}

#[test]
fn a_plugins_methods_go_to_what_they_intercept() {
    let mut project = shop();
    let classes = format!("{MODULE}/classes.php");
    let mut goes = |from: &str, to: &str| project.magento_goes(&classes, &CLASSES.replacen(from, to, 1));
    assert_eq!(goes("function beforeSave(", "function before/*|*/Save("), [(classes.clone(), 15)]);
    assert_eq!(goes("function aroundTotal(", "function around/*|*/Total("), [(classes.clone(), 14)]);
    // One that intercepts nothing is reported; the plugin's own methods are not.
    assert_eq!(project.magento_problems(&classes, CLASSES, "PluginMethod"), Vec::<String>::new());
    let source = CLASSES.replacen("function afterTotal(", "function afterTotl(", 1);
    assert_eq!(
        project.magento_problems(&classes, &source, "PluginMethod"),
        ["'Cart' has no method 'totl' for this plugin to intercept"]
    );
}

#[test]
fn magento_names_are_only_read_in_a_magento_project() {
    let mut project = Project::new(&[
        ("framework/classes.php", FRAMEWORK),
        ("src/classes.php", CLASSES),
        ("src/Model/Service.php", SERVICE),
    ]);
    assert!(!project.engine.frameworks().contains(crate::frameworks::Framework::Magento));
    let found = project
        .magento_goes("src/Model/Service.php", &at("dispatch('shop_cart_saved'", "dispatch('shop_cart/*|*/_saved'"));
    assert_eq!(found, Vec::new());
    assert_eq!(project.magento_problems("src/Model/Service.php", SERVICE, "PhpUndefinedClass").len(), 1);
}
