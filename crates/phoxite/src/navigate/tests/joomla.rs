//! Joomla, on a stand-in for its API written for these tests (the names of
//! its classes and methods, nothing of its code) and a small site with one
//! component and one plugin.

use super::*;

const FRAMEWORK: &str = r#"<?php
namespace Joomla\CMS {
    abstract class Factory {
        /** @return Application\CMSApplicationInterface */
        public static function getApplication() {}
    }
}
namespace Joomla\CMS\Application {
    interface CMSApplicationInterface extends \Joomla\CMS\Extension\ExtensionManagerInterface {
        public function getIdentity();
    }
    abstract class CMSApplication implements CMSApplicationInterface {
        public function getIdentity() {}
        public function getDocument() {}
        public function bootComponent($component): \Joomla\CMS\Extension\ComponentInterface {}
    }
}
namespace Joomla\CMS\Extension {
    interface ComponentInterface {}
    interface ExtensionManagerInterface {
        public function bootComponent($component): ComponentInterface;
    }
    class MVCComponent implements ComponentInterface {
        public function getMVCFactory(): \Joomla\CMS\MVC\Factory\MVCFactoryInterface {}
    }
}
namespace Joomla\CMS\MVC\Factory {
    interface MVCFactoryInterface {
        public function createModel($name, $prefix = '', array $config = []);
        public function createTable($name, $prefix = '', array $config = []);
    }
}
namespace Joomla\CMS\MVC\Controller {
    class BaseController {
        public function getModel($name = '', $prefix = '', $config = []) {}
    }
}
namespace Joomla\CMS\MVC\Model {
    class BaseDatabaseModel {}
    class FormModel extends BaseDatabaseModel {
        protected function loadForm($name, $source = null, $options = [], $clear = false, $xpath = null) {}
    }
}
namespace Joomla\CMS\MVC\View {
    abstract class AbstractView {
        public function getModel($name = null) {}
        public function get($property, $default = null) {}
    }
    class HtmlView extends AbstractView {
        public function loadTemplate($tpl = null) {}
    }
}
namespace Joomla\CMS\Table {
    abstract class Table {
        public function __construct($table, $key, $db) {}
        public function load($keys = null, $reset = true) {}
    }
}
namespace Joomla\CMS\Language {
    class Text {
        public static function _($string, $jsSafe = false) {}
        public static function sprintf($string) {}
    }
}
namespace Joomla\CMS\Layout {
    class LayoutHelper {
        public static function render($layoutFile, $displayData = null, $basePath = '', $options = null) {}
    }
    class FileLayout {
        public function __construct($layoutId, $basePath = null, $options = null) {}
        public function render($displayData = []) {}
    }
}
namespace Joomla\CMS\HTML {
    abstract class HTMLHelper {
        public static function _(string $key, ...$methodArgs) {}
    }
}
namespace Joomla\CMS\HTML\Helpers {
    abstract class Behavior {
        public static function core() {}
    }
}
namespace Joomla\CMS\WebAsset {
    class WebAssetManager {
        public function __call($method, $arguments) {}
        public function useAsset(string $type, string $name) {}
    }
}
namespace Joomla\CMS\User {
    class User {
        public function authorise($action, $assetname = null) {}
    }
}
namespace Joomla\CMS\Form {
    class Form {
        public function getValue($name, $group = null, $default = null) {}
    }
}
namespace Joomla\CMS\Router {
    class Route {
        public static function _($url, $xhtml = true) {}
    }
}
namespace Joomla\CMS\Plugin {
    abstract class CMSPlugin implements \Joomla\Event\SubscriberInterface {
        /** @return \Joomla\CMS\Application\CMSApplicationInterface */
        protected function getApplication() {}
    }
}
namespace Joomla\CMS\Event\Content {
    class ContentPrepareEvent extends \Joomla\Event\Event {
        public function getItem() {}
    }
    class AfterSaveEvent extends \Joomla\Event\Event {
        public function getIsNew(): bool {}
    }
}
namespace Joomla\Event {
    class Event {
        public function getName() {}
    }
    interface SubscriberInterface {
        public static function getSubscribedEvents(): array;
    }
    interface DispatcherInterface {
        public function dispatch(string $name, ?Event $event = null);
    }
}
namespace Joomla\Registry {
    class Registry {
        public function get($path, $default = null) {}
    }
}
namespace Joomla\Database {
    interface QueryInterface {
        public function from($tables);
    }
}
namespace Joomla\DI {
    class Container implements \Psr\Container\ContainerInterface {
        public function get($resourceName) {}
    }
}
namespace Psr\Container {
    interface ContainerInterface {
        public function get(string $id);
    }
}
"#;

const EVENTS: &str = "<?php\nnamespace Joomla\\CMS\\Event;\n\ntrait CoreEventAware\n{\n    private static $eventNameToConcreteClass = [\n        'onContentPrepare' => Content\\ContentPrepareEvent::class,\n        'onContentAfterSave' => Content\\AfterSaveEvent::class,\n    ];\n}\n";

const MANIFEST: &str = r#"<?xml version="1.0"?>
<extension type="component" method="upgrade">
    <name>com_shop</name>
    <description>COM_SHOP_XML_DESCRIPTION</description>
    <namespace path="src">Acme\Component\Shop</namespace>
</extension>
"#;

const ACCESS: &str = r#"<?xml version="1.0"?>
<access component="com_shop">
    <section name="component">
        <action name="core.edit" title="JACTION_EDIT"/>
        <action name="shop.ship" title="COM_SHOP_ACTION_SHIP"/>
    </section>
</access>
"#;

const CONFIG: &str = r#"<?xml version="1.0"?>
<config>
    <fieldset name="shop">
        <field name="show_prices" type="radio" label="COM_SHOP_SHOW_PRICES"/>
    </fieldset>
</config>
"#;

const FORM: &str = r#"<?xml version="1.0"?>
<form>
    <fieldset addfieldprefix="Acme\Component\Shop\Administrator\Field">
        <field name="title" type="text" label="COM_SHOP_FIELD_TITLE"/>
        <field name="price" type="money" layout="joomla.form.field.number"/>
    </fieldset>
</form>
"#;

const ASSETS: &str = r#"{
  "name": "com_shop",
  "assets": [
    {
      "name": "com_shop.cart",
      "type": "script",
      "uri": "com_shop/cart.js",
      "dependencies": ["core"]
    },
    { "name": "core", "type": "script", "uri": "system/core.js" }
  ]
}
"#;

const LANGUAGE: &str = "; Shop\nCOM_SHOP=\"Shop\"\nCOM_SHOP_HITS=\"Hits: %d\"\nCOM_SHOP_FIELD_TITLE=\"Title\"\nCOM_SHOP_SHOW_PRICES=\"Show prices\"\nCOM_SHOP_ACTION_SHIP=\"Ship\"\nCOM_SHOP_XML_DESCRIPTION=\"A shop\"\n";

const SQL: &str = "CREATE TABLE IF NOT EXISTS `#__shop_items` (\n  `id` int unsigned NOT NULL AUTO_INCREMENT,\n  `title` varchar(255) NOT NULL DEFAULT '',\n  PRIMARY KEY (`id`)\n) ENGINE=InnoDB;\n";

/// The component's classes, in one file.
const CLASSES: &str = r#"<?php
namespace Acme\Component\Shop\Administrator\Extension {
    class ShopComponent extends \Joomla\CMS\Extension\MVCComponent {
        public function ship(): bool { return true; }
    }
}
namespace Acme\Component\Shop\Administrator\Model {
    class ItemModel extends \Joomla\CMS\MVC\Model\FormModel {
        public function getForm($data = [], $loadData = true) {
            return $this->loadForm('com_shop.item', 'item', ['control' => 'jform']);
        }
        public function publish(): bool { return true; }
    }
}
namespace Acme\Component\Shop\Administrator\Table {
    class ItemTable extends \Joomla\CMS\Table\Table {
        public function __construct($db) { parent::__construct('#__shop_items', 'id', $db); }
        public function stock(): int { return 0; }
    }
}
namespace Acme\Component\Shop\Administrator\Field {
    class MoneyField {}
}
namespace Acme\Component\Shop\Site\Model {
    class ItemsModel extends \Joomla\CMS\MVC\Model\BaseDatabaseModel {
        /** @return \Acme\Component\Shop\Site\Model\Row[] */
        public function getItems(): array { return []; }
        public function total(): int { return 0; }
    }
    class Row {
        public string $title = '';
    }
}
namespace Acme\Component\Shop\Site\View\Items {
    class HtmlView extends \Joomla\CMS\MVC\View\HtmlView {
        public $items;
        public function display($tpl = null) { $this->items = $this->get('Items'); }
        public function heading(): string { return ''; }
    }
}
namespace Acme\Component\Shop\Site\Controller {
    class DisplayController extends \Joomla\CMS\MVC\Controller\BaseController {
        public function display() { return $this->getModel('Items'); }
    }
    class ItemController extends \Joomla\CMS\MVC\Controller\BaseController {
        public function save() {}
    }
}
"#;

const PLUGIN_MANIFEST: &str = "<?xml version=\"1.0\"?>\n<extension type=\"plugin\" group=\"content\">\n    <name>plg_content_stamp</name>\n    <namespace path=\"src\">Acme\\Plugin\\Content\\Stamp</namespace>\n</extension>\n";

const PLUGIN: &str = r#"<?php
namespace Acme\Plugin\Content\Stamp\Extension;

use Joomla\CMS\Plugin\CMSPlugin;
use Joomla\Event\Event;

final class Stamp extends CMSPlugin
{
    public static function getSubscribedEvents(): array
    {
        return [
            'onContentPrepare' => 'stamp',
            'onContentAfterSave' => 'onContentAfterSave',
        ];
    }

    public function stamp($event)
    {
        $item = $event->getItem();
    }

    public function onContentAfterSave(Event $event)
    {
        $new = $event->getIsNew();
    }
}
"#;

/// The probe: a helper of the component's site folder.
const HELPER: &str = r#"<?php
namespace Acme\Component\Shop\Site\Helper;

use Joomla\CMS\Factory;
use Joomla\CMS\Form\Form;
use Joomla\CMS\HTML\HTMLHelper;
use Joomla\CMS\Language\Text;
use Joomla\CMS\Layout\LayoutHelper;
use Joomla\CMS\MVC\Factory\MVCFactoryInterface;
use Joomla\CMS\Router\Route;
use Joomla\CMS\User\User;
use Joomla\CMS\WebAsset\WebAssetManager;
use Joomla\Database\QueryInterface;
use Joomla\DI\Container;
use Joomla\Event\DispatcherInterface;
use Joomla\Registry\Registry;

class ShopHelper
{
    public function run(
        WebAssetManager $assets,
        User $user,
        Form $form,
        Registry $params,
        QueryInterface $query,
        DispatcherInterface $dispatcher,
        MVCFactoryInterface $factory,
        Container $container,
    ) {
        echo Text::_('COM_SHOP_HITS');
        echo LayoutHelper::render('joomla.content.tags', []);
        HTMLHelper::_('behavior.core');
        $assets->useScript('com_shop.cart');
        Factory::getApplication()->bootComponent('com_shop');
        $dispatcher->dispatch('onContentPrepare');
        $user->authorise('shop.ship', 'com_shop');
        $form->getValue('title');
        $params->get('show_prices');
        $query->from('#__shop_items AS a');
        echo Route::_('index.php?option=com_shop&view=items&task=item.save');
        $this->probe();
    }
}
"#;

const ADMIN: &str = "administrator/components/com_shop";
const SITE: &str = "components/com_shop";
const PROBE: &str = "components/com_shop/src/Helper/ShopHelper.php";
const VIEW: &str = "components/com_shop/tmpl/items/default.php";
const STAMP: &str = "plugins/content/stamp/src/Extension/Stamp.php";

fn site() -> Project {
    let files: Vec<(String, &str)> = vec![
        ("libraries/src/Version.php".to_owned(), "<?php\nnamespace Joomla\\CMS;\n\nfinal class Version {}\n"),
        ("libraries/src/Event/CoreEventAware.php".to_owned(), EVENTS),
        ("libraries/stand-in.php".to_owned(), FRAMEWORK),
        (format!("{ADMIN}/shop.xml"), MANIFEST),
        (format!("{ADMIN}/access.xml"), ACCESS),
        (format!("{ADMIN}/config.xml"), CONFIG),
        (format!("{ADMIN}/forms/item.xml"), FORM),
        (format!("{ADMIN}/sql/install.mysql.utf8.sql"), SQL),
        (format!("{ADMIN}/classes.php"), CLASSES),
        (format!("{SITE}/tmpl/items/default.php"), "<h1><?php echo $this->heading(); ?></h1>\n"),
        (PROBE.to_owned(), HELPER),
        ("media/com_shop/joomla.asset.json".to_owned(), ASSETS),
        ("language/en-GB/com_shop.ini".to_owned(), LANGUAGE),
        ("layouts/joomla/content/tags.php".to_owned(), "<?php\n"),
        ("layouts/joomla/form/field/number.php".to_owned(), "<?php\n"),
        (
            "templates/blue/templateDetails.xml".to_owned(),
            "<extension type=\"template\" client=\"site\"><name>blue</name></extension>\n",
        ),
        ("templates/blue/html/layouts/joomla/content/tags.php".to_owned(), "<?php\n"),
        ("templates/blue/html/com_shop/items/default.php".to_owned(), "<?php\n"),
        ("templates/blue/index.php".to_owned(), "<?php\n"),
        (
            "modules/mod_cart/mod_cart.xml".to_owned(),
            "<extension type=\"module\" client=\"site\"><name>mod_cart</name></extension>\n",
        ),
        ("modules/mod_cart/tmpl/default.php".to_owned(), "<?php\n"),
        ("plugins/content/stamp/stamp.xml".to_owned(), PLUGIN_MANIFEST),
        (STAMP.to_owned(), PLUGIN),
    ];
    let files: Vec<(&str, &str)> = files.iter().map(|(rel, text)| (rel.as_str(), *text)).collect();
    Project::new(&files)
}

fn at(from: &str, to: &str) -> String {
    assert!(HELPER.contains(from), "{from}");
    HELPER.replacen(from, to, 1)
}

impl Project {
    fn joomla_goes(&mut self, file: &str, source: &str) -> Vec<(String, u32)> {
        let found = self.definition(file, source).unwrap_or_default();
        found.into_iter().map(|(file, line, _)| (file, line)).collect()
    }

    fn joomla_complete(&mut self, file: &str, source: &str) -> Vec<String> {
        let (text, pos) = caret(source);
        let doc = self.open(file, &text);
        let found = self.engine.completion(doc, pos, None, &CancellationToken::new()).expect("handled");
        self.engine.document_closed(doc);
        found.items.into_iter().map(|item| item.label).collect()
    }

    fn joomla_hover(&mut self, file: &str, source: &str) -> String {
        let (text, pos) = caret(source);
        let doc = self.open(file, &text);
        let hover = self.engine.hover(doc, pos);
        self.engine.document_closed(doc);
        hover.map(|hover| hover.markdown).unwrap_or_default()
    }

    fn joomla_problems(&mut self, file: &str, source: &str) -> Vec<String> {
        let doc = self.open(file, source);
        let inspected = self.engine.inspect(doc, &CancellationToken::new()).expect("inspected");
        self.engine.document_closed(doc);
        let found = inspected.problems.iter().filter(|problem| problem.inspection.contains("Joomla"));
        found.map(|problem| problem.message.clone()).collect()
    }

    fn joomla_usages(&mut self, file: &str, source: &str) -> Vec<(String, u32)> {
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
        found.dedup();
        found
    }

    /// The members offered after `body`, a statement of the probe's method.
    fn joomla_members(&mut self, body: &str) -> Vec<String> {
        let source = at("$this->probe();", body);
        self.joomla_complete(PROBE, &source)
    }
}

fn has(found: &[String], names: &[&str]) {
    for name in names {
        assert!(found.contains(&(*name).to_owned()), "{name}: {found:?}");
    }
}

#[test]
fn joomla_names_go_to_what_they_name() {
    let mut project = site();
    assert!(project.engine.frameworks().contains(crate::frameworks::Framework::Joomla));
    let mut goes = |from: &str, to: &str| project.joomla_goes(PROBE, &at(from, to));
    let file = |name: &str, line: u32| vec![(name.to_owned(), line)];
    let classes = format!("{ADMIN}/classes.php");
    assert_eq!(
        goes("Text::_('COM_SHOP_HITS')", "Text::_('COM_SHOP/*|*/_HITS')"),
        file("language/en-GB/com_shop.ini", 2)
    );
    // A layout: a template's override, then the layout.
    assert_eq!(
        goes("render('joomla.content.tags'", "render('joomla.content.ta/*|*/gs'"),
        [
            ("templates/blue/html/layouts/joomla/content/tags.php".to_owned(), 0),
            ("layouts/joomla/content/tags.php".to_owned(), 0)
        ]
    );
    assert_eq!(
        goes("HTMLHelper::_('behavior.core')", "HTMLHelper::_('behavior.co/*|*/re')"),
        file("libraries/stand-in.php", 80)
    );
    assert_eq!(
        goes("useScript('com_shop.cart')", "useScript('com_shop.ca/*|*/rt')"),
        file("media/com_shop/joomla.asset.json", 4)
    );
    assert_eq!(
        goes("bootComponent('com_shop')", "bootComponent('com_/*|*/shop')"),
        file(&format!("{ADMIN}/shop.xml"), 0)
    );
    assert_eq!(
        goes("dispatch('onContentPrepare')", "dispatch('onContent/*|*/Prepare')"),
        file("libraries/stand-in.php", 111)
    );
    assert_eq!(goes("authorise('shop.ship'", "authorise('shop.sh/*|*/ip'"), file(&format!("{ADMIN}/access.xml"), 4));
    assert_eq!(goes("getValue('title')", "getValue('ti/*|*/tle')"), file(&format!("{ADMIN}/forms/item.xml"), 3));
    assert_eq!(goes("get('show_prices')", "get('show_/*|*/prices')"), file(&format!("{ADMIN}/config.xml"), 3));
    assert_eq!(
        goes("from('#__shop_items AS a')", "from('#__shop_/*|*/items AS a')"),
        file(&format!("{ADMIN}/sql/install.mysql.utf8.sql"), 0)
    );
    // The parts of a URL: the component, its view, a controller's task.
    let route = "Route::_('index.php?option=com_shop&view=items&task=item.save')";
    assert_eq!(
        goes(route, "Route::_('index.php?option=com_/*|*/shop&view=items&task=item.save')"),
        file(&format!("{ADMIN}/shop.xml"), 0)
    );
    assert_eq!(goes(route, "Route::_('index.php?option=com_shop&view=it/*|*/ems&task=item.save')"), file(&classes, 34));
    assert_eq!(goes(route, "Route::_('index.php?option=com_shop&view=items&task=item.sa/*|*/ve')"), file(&classes, 45));
    // In the component's classes: a form, a table and its key.
    let goes = |project: &mut Project, from: &str, to: &str| {
        assert!(CLASSES.contains(from), "{from}");
        project.joomla_goes(&classes, &CLASSES.replacen(from, to, 1))
    };
    assert_eq!(
        goes(&mut project, "'item', ['control'", "'it/*|*/em', ['control'"),
        file(&format!("{ADMIN}/forms/item.xml"), 0)
    );
    assert_eq!(
        goes(&mut project, "__construct('#__shop_items', 'id'", "__construct('#__shop_/*|*/items', 'id'"),
        file(&format!("{ADMIN}/sql/install.mysql.utf8.sql"), 0)
    );
    assert_eq!(
        goes(&mut project, "__construct('#__shop_items', 'id'", "__construct('#__shop_items', 'i/*|*/d'"),
        file(&format!("{ADMIN}/sql/install.mysql.utf8.sql"), 1)
    );
    // A subscriber: an event's class, the method handling it.
    let found = project
        .joomla_goes(STAMP, &PLUGIN.replacen("'onContentPrepare' => 'stamp'", "'onContent/*|*/Prepare' => 'stamp'", 1));
    assert_eq!(found, file("libraries/stand-in.php", 111));
    let found = project
        .joomla_goes(STAMP, &PLUGIN.replacen("'onContentPrepare' => 'stamp'", "'onContentPrepare' => 'st/*|*/amp'", 1));
    assert_eq!(found, file(STAMP, 16));

    let hover = project.joomla_hover(PROBE, &at("Text::_('COM_SHOP_HITS')", "Text::_('COM_SHOP/*|*/_HITS')"));
    assert!(hover.contains("**language constant** `COM_SHOP_HITS`") && hover.contains("Hits: %d"), "{hover}");
    let hover = project.joomla_hover(PROBE, &at("useScript('com_shop.cart')", "useScript('com_shop.ca/*|*/rt')"));
    assert!(hover.contains("script, needs core"), "{hover}");
}

#[test]
fn joomla_names_complete_by_what_is_asked_for() {
    let mut project = site();
    let mut offered = |from: &str, to: &str| project.joomla_complete(PROBE, &at(from, to));
    has(
        &offered("Text::_('COM_SHOP_HITS')", "Text::_('/*|*/')"),
        &["COM_SHOP", "COM_SHOP_HITS", "COM_SHOP_FIELD_TITLE"],
    );
    has(
        &offered("render('joomla.content.tags'", "render('/*|*/'"),
        &["joomla.content.tags", "joomla.form.field.number"],
    );
    assert_eq!(offered("useScript('com_shop.cart')", "useScript('/*|*/')"), ["com_shop.cart", "core"]);
    has(&offered("bootComponent('com_shop')", "bootComponent('/*|*/')"), &["com_shop", "mod_cart", "stamp"]);
    assert_eq!(
        offered("dispatch('onContentPrepare')", "dispatch('/*|*/')"),
        ["onContentAfterSave", "onContentPrepare"]
    );
    assert_eq!(offered("authorise('shop.ship'", "authorise('/*|*/'"), ["core.edit", "shop.ship"]);
    assert_eq!(offered("getValue('title')", "getValue('/*|*/')"), ["price", "title"]);
    assert_eq!(offered("get('show_prices')", "get('/*|*/')"), ["show_prices"]);
    assert_eq!(offered("from('#__shop_items AS a')", "from('/*|*/')"), ["#__shop_items"]);
}

#[test]
fn joomla_names_that_name_nothing_are_reported() {
    let mut project = site();
    assert_eq!(project.joomla_problems(PROBE, HELPER), Vec::<String>::new());
    // Another extension's constant and Joomla's own may be anywhere; so may a layout of an extension.
    let source = at("Text::_('COM_SHOP_HITS')", "Text::_('COM_OTHER_THING') . Text::_('JYES')").replacen(
        "render('joomla.content.tags'",
        "render('other.layout'",
        1,
    );
    assert_eq!(project.joomla_problems(PROBE, &source), Vec::<String>::new());
    let source = at("Text::_('COM_SHOP_HITS')", "Text::_('COM_SHOP_GONE')").replacen(
        "render('joomla.content.tags'",
        "render('joomla.content.gone'",
        1,
    );
    assert_eq!(
        project.joomla_problems(PROBE, &source),
        ["Unknown language constant 'COM_SHOP_GONE'", "Unknown layout 'joomla.content.gone'"]
    );
}

#[test]
fn a_language_constant_is_used_and_renamed_in_php_ini_and_xml() {
    let mut project = site();
    let source = "<?php\nnamespace Acme\\Component\\Shop\\Site\\Helper;\n\nuse Joomla\\CMS\\Language\\Text;\n\nclass Titles\n{\n    public function title()\n    {\n        return Text::_('COM_SHOP_FIELD/*|*/_TITLE');\n    }\n}\n";
    let file = "components/com_shop/src/Helper/Titles.php";
    let found = project.joomla_usages(file, source);
    assert_eq!(
        found,
        [(format!("{ADMIN}/forms/item.xml"), 3), (file.to_owned(), 9), ("language/en-GB/com_shop.ini".to_owned(), 3),]
    );
    let (text, pos) = caret(source);
    let doc = project.open(file, &text);
    let edit =
        project.engine.rename(doc, pos, "COM_SHOP_FIELD_NAME", &CancellationToken::new()).expect("renameable").unwrap();
    let mut edits = Vec::new();
    for changed in &edit.files {
        let crate::base::FileEdit::Edit { path, edits: found, .. } = changed else { panic!("only text edits") };
        edits.extend(found.iter().map(|edit| (project.describe(path, edit.range), edit.new_text.clone())));
    }
    edits.sort();
    project.engine.document_closed(doc);
    let expected = |file: &str, line: u32| {
        ((file.to_owned(), line, "COM_SHOP_FIELD_TITLE".to_owned()), "COM_SHOP_FIELD_NAME".to_owned())
    };
    assert_eq!(
        edits,
        [
            expected(&format!("{ADMIN}/forms/item.xml"), 3),
            expected(file, 9),
            expected("language/en-GB/com_shop.ini", 3)
        ]
    );
}

#[test]
fn the_application_the_mvc_factory_and_a_container_are_typed() {
    let mut project = site();
    // The application: more than the interface it is documented to return.
    has(&project.joomla_members("Factory::getApplication()->/*|*/;"), &["getDocument", "getIdentity", "bootComponent"]);
    // A component and what its factory makes, by the client asked for and by the caller's.
    has(
        &project.joomla_members("Factory::getApplication()->bootComponent('com_shop')->/*|*/;"),
        &["ship", "getMVCFactory"],
    );
    has(&project.joomla_members("$factory->createModel('Item', 'Administrator')->/*|*/;"), &["publish", "getForm"]);
    has(&project.joomla_members("$factory->createModel('Items')->/*|*/;"), &["getItems", "total"]);
    has(&project.joomla_members("$factory->createTable('Item', 'Administrator')->/*|*/;"), &["stock", "load"]);
    // A container makes the class it is asked for.
    has(
        &project.joomla_members("$container->get(\\Acme\\Component\\Shop\\Site\\Model\\ItemsModel::class)->/*|*/;"),
        &["total"],
    );
    // A controller's model.
    let classes = format!("{ADMIN}/classes.php");
    let source = CLASSES.replacen("return $this->getModel('Items');", "return $this->getModel('Items')->/*|*/;", 1);
    has(&project.joomla_complete(&classes, &source), &["getItems", "total"]);
    // A view: its own model, and what the model's getter returns.
    let source = CLASSES.replacen("$this->items = $this->get('Items');", "$this->getModel()->/*|*/;", 1);
    has(&project.joomla_complete(&classes, &source), &["getItems", "total"]);
    let source =
        CLASSES.replacen("$this->items = $this->get('Items');", "$rows = $this->get('Items'); $rows[0]->/*|*/;", 1);
    has(&project.joomla_complete(&classes, &source), &["title"]);
}

#[test]
fn an_event_handlers_parameter_is_its_events_class() {
    let mut project = site();
    // By the name the method is subscribed to, whatever the method is called.
    let source = PLUGIN.replacen("$item = $event->getItem();", "$event->/*|*/;", 1);
    has(&project.joomla_complete(STAMP, &source), &["getItem", "getName"]);
    // Declared as the base event: the event's own class.
    let source = PLUGIN.replacen("$new = $event->getIsNew();", "$event->/*|*/;", 1);
    has(&project.joomla_complete(STAMP, &source), &["getIsNew"]);
    // A plugin's application.
    let source = PLUGIN.replacen("$new = $event->getIsNew();", "$this->getApplication()->/*|*/;", 1);
    has(&project.joomla_complete(STAMP, &source), &["getIdentity", "bootComponent"]);
}

#[test]
fn a_template_has_its_view_a_layout_its_renderer() {
    let mut project = site();
    has(&project.joomla_complete(VIEW, "<?php $this->/*|*/ ?>\n"), &["heading", "items", "loadTemplate"]);
    // Its override in a template is the same view's.
    let overridden = "templates/blue/html/com_shop/items/default.php";
    has(&project.joomla_complete(overridden, "<?php $this->/*|*/ ?>\n"), &["heading", "loadTemplate"]);
    has(&project.joomla_complete("layouts/joomla/content/tags.php", "<?php $this->/*|*/ ?>\n"), &["render"]);
    // A module's template.
    has(&project.joomla_complete("modules/mod_cart/tmpl/default.php", "<?php $params->/*|*/ ?>\n"), &["get"]);
    has(&project.joomla_complete("modules/mod_cart/tmpl/default.php", "<?php $app->/*|*/ ?>\n"), &["getIdentity"]);
}

#[test]
fn joomla_names_are_only_read_in_a_joomla_project() {
    let mut project = Project::new(&[("lib/stand-in.php", FRAMEWORK), ("src/Helper/ShopHelper.php", HELPER)]);
    assert!(!project.engine.frameworks().contains(crate::frameworks::Framework::Joomla));
    let found = project
        .joomla_goes("src/Helper/ShopHelper.php", &at("Text::_('COM_SHOP_HITS')", "Text::_('COM_SHOP/*|*/_HITS')"));
    assert_eq!(found, Vec::new());
}

impl Project {
    fn joomla_document_goes(&self, file: &str, source: &str) -> Vec<(String, u32)> {
        let (text, pos) = caret(source);
        let found = self.engine.joomla_document_definition(&self.root.join(file), &text, pos).unwrap_or_default();
        let rel = |path: &Path| path.strip_prefix(&self.root).unwrap_or(path).to_string_lossy().replace('\\', "/");
        found.iter().map(|target| (rel(&target.path), target.range.start.line)).collect()
    }

    fn joomla_document_offers(&self, file: &str, source: &str) -> Vec<String> {
        let (text, pos) = caret(source);
        let found = self.engine.joomla_document_completion(&self.root.join(file), &text, pos);
        found.unwrap_or_default().into_iter().map(|item| item.label).collect()
    }

    fn joomla_document_usages(&self, file: &str, source: &str) -> Vec<(String, u32)> {
        let (text, pos) = caret(source);
        let cancel = CancellationToken::new();
        let found = self.engine.joomla_document_references(&self.root.join(file), &text, pos, true, &cancel);
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
}

#[test]
fn the_xml_language_and_asset_files_are_documents() {
    let project = site();
    let form = format!("{ADMIN}/forms/item.xml");
    let replaced = |source: &str, from: &str, to: &str| {
        assert!(source.contains(from), "{from}");
        source.replacen(from, to, 1)
    };
    // A label's constant, a field's type by the form's prefix, a layout.
    let found =
        project.joomla_document_goes(&form, &replaced(FORM, "COM_SHOP_FIELD_TITLE", "COM_SHOP_FIELD/*|*/_TITLE"));
    assert_eq!(found, [("language/en-GB/com_shop.ini".to_owned(), 3)]);
    let found = project.joomla_document_goes(&form, &replaced(FORM, "type=\"money\"", "type=\"mon/*|*/ey\""));
    assert_eq!(found, [(format!("{ADMIN}/classes.php"), 21)]);
    let found = project
        .joomla_document_goes(&form, &replaced(FORM, "joomla.form.field.number", "joomla.form.field.num/*|*/ber"));
    assert_eq!(found, [("layouts/joomla/form/field/number.php".to_owned(), 0)]);
    // A manifest's description, an action's title, an asset's dependency.
    let manifest = format!("{ADMIN}/shop.xml");
    let found = project.joomla_document_goes(
        &manifest,
        &replaced(MANIFEST, "COM_SHOP_XML_DESCRIPTION", "COM_SHOP_XML/*|*/_DESCRIPTION"),
    );
    assert_eq!(found, [("language/en-GB/com_shop.ini".to_owned(), 6)]);
    let access = format!("{ADMIN}/access.xml");
    let found =
        project.joomla_document_goes(&access, &replaced(ACCESS, "COM_SHOP_ACTION_SHIP", "COM_SHOP_ACTION/*|*/_SHIP"));
    assert_eq!(found, [("language/en-GB/com_shop.ini".to_owned(), 5)]);
    let assets = "media/com_shop/joomla.asset.json";
    let found = project.joomla_document_goes(assets, &replaced(ASSETS, "[\"core\"]", "[\"co/*|*/re\"]"));
    assert_eq!(found, [(assets.to_owned(), 9)]);

    // From a language file's key: its usages in PHP and in the XML.
    let ini = "language/en-GB/com_shop.ini";
    let found = project.joomla_document_usages(ini, &replaced(LANGUAGE, "COM_SHOP_HITS=", "COM_SHOP/*|*/_HITS="));
    assert_eq!(found, [(PROBE.to_owned(), 29), (ini.to_owned(), 2)]);
    let found =
        project.joomla_document_usages(ini, &replaced(LANGUAGE, "COM_SHOP_SHOW_PRICES=", "COM_SHOP_SHOW/*|*/_PRICES="));
    assert_eq!(found, [(format!("{ADMIN}/config.xml"), 3), (ini.to_owned(), 4)]);
    // A field class's usages include the field of that type.
    let (text, pos) = caret(&replaced(FORM, "type=\"money\"", "type=\"mon/*|*/ey\""));
    let hover = project.engine.joomla_document_hover(&project.root.join(&form), &text, pos).unwrap().markdown;
    assert!(hover.contains("Acme\\Component\\Shop\\Administrator\\Field\\MoneyField"), "{hover}");
    let found = project.joomla_document_usages(&form, &replaced(FORM, "type=\"money\"", "type=\"mon/*|*/ey\""));
    assert_eq!(found, [(format!("{ADMIN}/classes.php"), 21), (form.clone(), 4)]);

    // Completion: constants by what is written, the field types, the layouts.
    let offered = project
        .joomla_document_offers(&form, &replaced(FORM, "label=\"COM_SHOP_FIELD_TITLE\"", "label=\"COM_SHOP_F/*|*/\""));
    assert_eq!(offered, ["COM_SHOP_FIELD_TITLE"]);
    let offered = project.joomla_document_offers(&form, &replaced(FORM, "type=\"money\"", "type=\"/*|*/\""));
    assert_eq!(offered, ["money"]);
    let offered = project.joomla_document_offers(
        &form,
        &replaced(FORM, "layout=\"joomla.form.field.number\"", "layout=\"joomla./*|*/\""),
    );
    has(&offered, &["joomla.content.tags", "joomla.form.field.number"]);

    // A constant is renamed from its language file.
    let (text, pos) = caret(&replaced(LANGUAGE, "COM_SHOP_HITS=", "COM_SHOP/*|*/_HITS="));
    let path = project.root.join(ini);
    assert!(project.engine.joomla_document_prepare_rename(&path, &text, pos).flatten().is_some());
    let cancel = CancellationToken::new();
    let edit = project.engine.joomla_document_rename(&path, &text, pos, "COM_SHOP_VIEWS", &cancel).unwrap().unwrap();
    let mut files: Vec<String> = edit
        .files
        .iter()
        .filter_map(|file| match file {
            crate::base::FileEdit::Edit { path, .. } => Some(project.describe(path, Range::default()).0),
            _ => None,
        })
        .collect();
    files.sort();
    assert_eq!(files, [PROBE.to_owned(), ini.to_owned()]);
    // Not a document outside what the model reads.
    assert!(
        project
            .engine
            .joomla_document_definition(&project.root.join("readme.txt"), "COM_SHOP", Pos::new(0, 2))
            .is_none()
    );
}

#[test]
fn a_view_loads_its_sub_templates_and_a_moved_class_gets_its_manifests_namespace() {
    let mut project = Project::new(&[
        ("libraries/src/Version.php", "<?php\n"),
        ("libraries/stand-in.php", FRAMEWORK),
        ("administrator/components/com_shop/shop.xml", MANIFEST),
        ("administrator/components/com_shop/classes.php", CLASSES),
        ("components/com_shop/tmpl/items/default.php", "<?php echo $this->loadTemplate('row'); ?>\n"),
        ("components/com_shop/tmpl/items/default_row.php", "<p></p>\n"),
        ("components/com_shop/tmpl/items/default_empty.php", "<p></p>\n"),
        (
            "components/com_shop/src/Helper/PriceHelper.php",
            "<?php\nnamespace Acme\\Component\\Shop\\Site\\Helper;\n\nclass PriceHelper {}\n",
        ),
        (
            "templates/blue/templateDetails.xml",
            "<extension type=\"template\" client=\"site\"><name>blue</name></extension>\n",
        ),
        ("templates/blue/html/com_shop/items/default_row.php", "<p></p>\n"),
    ]);
    let found = project.joomla_goes(VIEW, "<?php echo $this->loadTemplate('r/*|*/ow'); ?>\n");
    assert_eq!(
        found,
        [
            ("templates/blue/html/com_shop/items/default_row.php".to_owned(), 0),
            ("components/com_shop/tmpl/items/default_row.php".to_owned(), 0),
        ]
    );
    assert_eq!(project.joomla_complete(VIEW, "<?php echo $this->loadTemplate('/*|*/'); ?>\n"), ["empty", "row"]);
    // A class file moved inside the component: the namespace its manifest gives the folder.
    let moves = vec![(
        project.root.join("components/com_shop/src/Helper/PriceHelper.php"),
        project.root.join("components/com_shop/src/Service/PriceHelper.php"),
    )];
    let edit = project.engine.file_moves_edit(&moves, &CancellationToken::new()).unwrap();
    let texts: Vec<String> = edit
        .files
        .iter()
        .filter_map(|file| match file {
            crate::base::FileEdit::Edit { edits, .. } => {
                Some(edits.iter().map(|edit| edit.new_text.clone()).collect::<Vec<_>>())
            }
            _ => None,
        })
        .flatten()
        .collect();
    assert!(texts.iter().any(|text| text.contains("Acme\\Component\\Shop\\Site\\Service")), "{texts:?}");
}
