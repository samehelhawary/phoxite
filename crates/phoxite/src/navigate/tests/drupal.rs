//! Drupal, on a stand-in for its API written for these tests (the names of
//! its classes and methods, nothing of their code) and a module of YAML
//! files.

use super::*;

/// The classes and functions the module below calls.
const CORE: &str = r#"<?php
namespace {
    class Drupal {
        /** @return mixed */
        public static function service($id) {}
        /** @return \Drupal\Core\Entity\EntityTypeManagerInterface */
        public static function entityTypeManager() {}
        /** @return \Drupal\Core\Extension\ModuleHandlerInterface */
        public static function moduleHandler() {}
        /** @return \Drupal\Core\Config\Config */
        public static function config($name) {}
        public static function entityQuery($entity_type, $conjunction = 'AND') {}
    }
}
namespace Psr\Container {
    interface ContainerInterface { public function get(string $id); public function has(string $id): bool; }
}
namespace Symfony\Component\DependencyInjection {
    interface ContainerInterface extends \Psr\Container\ContainerInterface {}
}
namespace Drupal\Core {
    class Url { public static function fromRoute($route_name, $route_parameters = [], $options = []) {} }
    class Link { public static function createFromRoute($text, $route_name, $route_parameters = []) {} }
}
namespace Drupal\Core\Controller {
    class ControllerBase {
        protected function redirect($route_name, array $route_parameters = []) {}
        protected function config($name) {}
    }
}
namespace Drupal\Core\Session {
    interface AccountInterface { public function hasPermission($permission); }
}
namespace Drupal\Core\Access {
    class AccessResult {
        public static function allowedIfHasPermission($account, $permission) {}
        public static function allowedIfHasPermissions($account, array $permissions, $conjunction = 'AND') {}
    }
}
namespace Drupal\Core\Extension {
    interface ModuleHandlerInterface {
        public function moduleExists($module);
        public function invokeAll($hook, array $args = []);
        public function invoke($module, $hook, array $args = []);
        public function alter($type, &$data, &$context1 = NULL, &$context2 = NULL);
    }
}
namespace Drupal\Component\Plugin {
    interface PluginManagerInterface {
        public function createInstance($plugin_id, array $configuration = []);
        public function getDefinition($plugin_id, $exception_on_invalid = TRUE);
    }
}
namespace Drupal\Core\Entity {
    interface EntityInterface { public function id(); }
    interface EntityStorageInterface {
        /** @return \Drupal\Core\Entity\EntityInterface|null */
        public function load($id);
        /** @return \Drupal\Core\Entity\EntityInterface[] */
        public function loadMultiple(?array $ids = NULL);
        /** @return \Drupal\Core\Entity\EntityInterface */
        public function create(array $values = []);
    }
    interface EntityTypeManagerInterface extends \Drupal\Component\Plugin\PluginManagerInterface {
        /** @return \Drupal\Core\Entity\EntityStorageInterface */
        public function getStorage($entity_type_id);
    }
}
namespace Drupal\Core\Block {
    interface BlockManagerInterface extends \Drupal\Component\Plugin\PluginManagerInterface {}
}
namespace Drupal\Core\Hook\Attribute {
    #[\Attribute]
    class Hook { public function __construct(public string $hook, public string $method = '') {} }
}
"#;

/// The hooks Drupal documents, as an `*.api.php` file does.
const API: &str = r#"<?php
/**
 * @param \Drupal\shop\Entity\Book $entity The entity.
 */
function hook_entity_presave($entity) {}
/**
 * @param \Drupal\shop\Entity\Book $entity The entity.
 */
function hook_ENTITY_TYPE_presave($entity) {}
function hook_form_alter(array &$form, \Drupal\shop\Catalog $form_state, $form_id) {}
function hook_form_FORM_ID_alter(array &$form, \Drupal\shop\Catalog $form_state, $form_id) {}
function hook_theme($existing, $type, $theme, $path) {}
function hook_shop_books(\Drupal\shop\Catalog $catalog) {}
"#;

const SERVICES: &str = r#"services:
  shop.catalog:
    class: Drupal\shop\Catalog
    arguments: ['@shop.prices']
  shop.prices:
    class: Drupal\shop\Prices
  shop.books:
    alias: shop.catalog
"#;

const ROUTING: &str = r#"shop.books:
  path: '/books'
  defaults:
    _controller: '\Drupal\shop\Controller\BookController::list'
  requirements:
    _permission: 'administer shop+access content'
shop.book:
  path: '/books/{book}'
  requirements:
    _permission: 'access content'
"#;

const LINKS: &str = "shop.books_link:\n  title: 'Books'\n  route_name: shop.books\n  parent: system.admin\n";

const PERMISSIONS: &str = "administer shop:\n  title: 'Administer the shop'\naccess content:\n  title: 'View'\n";

const LIBRARIES: &str = "cart:\n  js:\n    js/cart.js: {}\n  dependencies:\n    - shop/base\nbase:\n  css:\n    theme:\n      css/base.css: {}\n";

const INFO: &str = "name: Shop\ntype: module\ncore_version_requirement: ^10\nlibraries:\n  - shop/base\n";

const ENTITY: &str = r#"<?php
namespace Drupal\shop\Entity;

use Drupal\Core\Entity\EntityInterface;

/**
 * Defines the book entity.
 *
 * @ContentEntityType(
 *   id = "shop_book",
 *   label = @Translation("Book"),
 *   handlers = {
 *     "storage" = "Drupal\shop\BookStorage",
 *     "list_builder" = "Drupal\shop\BookListBuilder",
 *   },
 * )
 */
class Book implements EntityInterface {
    public string $title = '';
    public function id() {}
}

#[ContentEntityType(id: 'shop_shelf', handlers: ['list_builder' => ShelfList::class])]
class Shelf implements EntityInterface {
    public string $room = '';
    public function id() {}
}
"#;

const CLASSES: &str = r#"<?php
namespace Drupal\shop {
    class Catalog { public function books(): array { return []; } }
    class Prices { public function of($book): int { return 0; } }
    class BookStorage implements \Drupal\Core\Entity\EntityStorageInterface {
        public function load($id) {}
        public function loadMultiple(?array $ids = NULL) {}
        public function create(array $values = []) {}
        public function newest() {}
    }
}
namespace Drupal\shop\Plugin\Block {
    /**
     * @Block(
     *   id = "shop_cart",
     *   admin_label = @Translation("Cart"),
     * )
     */
    class CartBlock {}

    #[Block(id: 'shop_offers')]
    class OffersBlock {}
}
namespace Drupal\shop\Plugin\Field {
    #[FieldType(id: 'shop_price')]
    class PriceItem {}
}
"#;

const MODULE: &str = "modules/shop/shop.module";

/// The module's procedural file.
const SHOP: &str = r#"<?php

use Drupal\Core\Access\AccessResult;
use Drupal\Core\Link;
use Drupal\Core\Url;

function shop_theme($existing, $type, $theme, $path) {
  return [
    'shop_book' => ['variables' => ['book' => NULL]],
    'shop_price_tag' => ['variables' => ['price' => 0]],
  ];
}

function shop_entity_presave($entity) {
  $entity->title;
}

function shop_shop_book_presave($entity) {
  $entity->title;
}

function shop_form_shop_book_form_alter(&$form, $form_state, $form_id) {
  $form_state->books();
}

function shop_page(\Drupal\Core\Session\AccountInterface $account, \Psr\Container\ContainerInterface $container, \Drupal\Component\Plugin\PluginManagerInterface $blocks) {
  $catalog = \Drupal::service('shop.catalog');
  $catalog->books();
  $prices = $container->get('shop.prices');
  $url = Url::fromRoute('shop.books');
  $link = Link::createFromRoute('Books', 'shop.book');
  if ($account->hasPermission('administer shop')) {}
  $access = AccessResult::allowedIfHasPermissions($account, ['administer shop', 'access content']);
  $build = ['#theme' => 'shop_book', '#attached' => ['library' => ['shop/cart']]];
  $build['#attached']['library'][] = 'shop/base';
  $build['#theme'] = 'shop_price_tag';
  $name = \Drupal::config('shop.settings');
  $on = \Drupal::moduleHandler()->moduleExists('shop');
  $storage = \Drupal::entityTypeManager()->getStorage('shop_book');
  $book = $storage->load(1);
  $shelves = \Drupal::entityTypeManager()->getStorage('shop_shelf')->loadMultiple();
  $block = $blocks->createInstance('shop_cart');
  \Drupal::moduleHandler()->invokeAll('shop_books', [$catalog]);
  \Drupal::moduleHandler()->alter('form', $build);
  \Drupal::moduleHandler()->invokeAll('entity_presave', [$book]);
  return [$prices, $url, $link, $access, $name, $on, $book, $shelves, $block];
}
"#;

fn drupal() -> Project {
    Project::new(&[
        ("core/lib/Drupal.php", CORE),
        ("core/lib/shop.api.php", API),
        ("modules/shop/shop.info.yml", INFO),
        ("modules/shop/shop.services.yml", SERVICES),
        ("modules/shop/shop.routing.yml", ROUTING),
        ("modules/shop/shop.links.menu.yml", LINKS),
        ("modules/shop/shop.permissions.yml", PERMISSIONS),
        ("modules/shop/shop.libraries.yml", LIBRARIES),
        ("modules/shop/config/install/shop.settings.yml", "currency: EUR\n"),
        ("modules/shop/templates/shop-book.html.twig", "<h2>{{ book }}</h2>\n"),
        ("modules/shop/src/Entity/Book.php", ENTITY),
        ("modules/shop/src/Catalog.php", CLASSES),
        // A test module's names aren't the site's.
        ("modules/shop/tests/modules/shop_test/shop_test.routing.yml", "shop_test.page:\n  path: '/test'\n"),
        (MODULE, SHOP),
    ])
}

fn at(from: &str, to: &str) -> String {
    assert!(SHOP.contains(from), "{from}");
    SHOP.replacen(from, to, 1)
}

impl Project {
    fn drupal_complete(&mut self, source: &str) -> Vec<String> {
        let (text, pos) = caret(source);
        let doc = self.open(MODULE, &text);
        let found = self.engine.completion(doc, pos, None, &CancellationToken::new()).expect("handled");
        self.engine.document_closed(doc);
        found.items.into_iter().map(|item| item.label).collect()
    }

    fn drupal_usages(&mut self, source: &str) -> Vec<(String, u32, String)> {
        let (text, pos) = caret(source);
        let doc = self.open(MODULE, &text);
        let usages = self.engine.references(doc, pos, true, &CancellationToken::new()).expect("handled").unwrap();
        let found = usages.iter().map(|usage| self.describe(&usage.path, usage.range)).collect();
        self.engine.document_closed(doc);
        found
    }

    fn drupal_rename(&mut self, source: &str, new_name: &str) -> Result<Vec<(String, u32, String)>, String> {
        let (text, pos) = caret(source);
        let doc = self.open(MODULE, &text);
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
            found.extend(edits.iter().map(|edit| self.describe(path, edit.range)));
        }
        found.sort();
        Ok(found)
    }

    fn drupal_hover(&mut self, source: &str) -> String {
        let (text, pos) = caret(source);
        let doc = self.open(MODULE, &text);
        let hover = self.engine.hover(doc, pos);
        self.engine.document_closed(doc);
        hover.map(|hover| hover.markdown).unwrap_or_default()
    }
}

#[test]
fn the_project_is_drupal_and_its_module_files_are_php() {
    let project = drupal();
    assert!(project.engine.frameworks().contains(crate::frameworks::Framework::Drupal));
    // `shop.module` is indexed: its functions are known.
    let found = project.engine.declarations_of(
        crate::names::NameKind::Function,
        "shop_page",
        std::path::Path::new(""),
        &crate::summary::FileSummary::default(),
    );
    assert_eq!(found.len(), 1);
    let model = project.engine.drupal();
    assert_eq!(model.routes.iter().map(|route| route.name.as_str()).collect::<Vec<_>>(), ["shop.books", "shop.book"]);
    assert_eq!(model.extensions.len(), 1);
}

#[test]
fn the_names_of_the_yaml_files_go_to_their_keys() {
    let mut project = drupal();
    let mut goes = |from: &str, to: &str| project.definition(MODULE, &at(from, to)).unwrap_or_default();
    assert_eq!(
        goes("service('shop.catalog')", "service('shop.cat/*|*/alog')"),
        [target("modules/shop/shop.services.yml", 1, "shop.catalog")]
    );
    assert_eq!(
        goes("get('shop.prices')", "get('shop.pri/*|*/ces')"),
        [target("modules/shop/shop.services.yml", 4, "shop.prices")]
    );
    let routing = "modules/shop/shop.routing.yml";
    assert_eq!(goes("fromRoute('shop.books')", "fromRoute('shop.bo/*|*/oks')"), [target(routing, 0, "shop.books")]);
    assert_eq!(goes("'Books', 'shop.book'", "'Books', 'shop.bo/*|*/ok'"), [target(routing, 6, "shop.book")]);
    let permissions = "modules/shop/shop.permissions.yml";
    assert_eq!(
        goes("hasPermission('administer shop')", "hasPermission('admin/*|*/ister shop')"),
        [target(permissions, 0, "administer shop")]
    );
    assert_eq!(
        goes("['administer shop', 'access content']", "['administer shop', 'access con/*|*/tent']"),
        [target(permissions, 2, "access content")]
    );
    let libraries = "modules/shop/shop.libraries.yml";
    assert_eq!(goes("'library' => ['shop/cart']", "'library' => ['shop/ca/*|*/rt']"), [target(libraries, 0, "cart")]);
    assert_eq!(goes("[] = 'shop/base'", "[] = 'shop/ba/*|*/se'"), [target(libraries, 5, "base")]);
    // A configuration and a module go to their files.
    let found = goes("config('shop.settings')", "config('shop.set/*|*/tings')");
    assert_eq!(
        found.iter().map(|(file, ..)| file.as_str()).collect::<Vec<_>>(),
        ["modules/shop/config/install/shop.settings.yml"]
    );
    let found = goes("moduleExists('shop')", "moduleExists('sh/*|*/op')");
    assert_eq!(found.iter().map(|(file, ..)| file.as_str()).collect::<Vec<_>>(), ["modules/shop/shop.info.yml"]);

    let hover = project.drupal_hover(&at("fromRoute('shop.books')", "fromRoute('shop.bo/*|*/oks')"));
    assert!(hover.contains("**route** `shop.books`") && hover.contains("`/books`"), "{hover}");
    let hover = project.drupal_hover(&at("service('shop.catalog')", "service('shop.cat/*|*/alog')"));
    assert!(hover.contains("**service** `shop.catalog`") && hover.contains("Drupal\\shop\\Catalog"), "{hover}");

    // Completion offers the kind asked for, not a test module's.
    let found = project.drupal_complete(&at("fromRoute('shop.books')", "fromRoute('/*|*/')"));
    assert_eq!(found, ["shop.book", "shop.books"]);
    let found = project.drupal_complete(&at("hasPermission('administer shop')", "hasPermission('/*|*/')"));
    assert_eq!(found, ["access content", "administer shop"]);
    let found = project.drupal_complete(&at("'library' => ['shop/cart']", "'library' => ['/*|*/']"));
    assert_eq!(found, ["shop/base", "shop/cart"]);
}

#[test]
fn usages_and_renames_count_the_yaml_files() {
    let mut project = drupal();
    let services = "modules/shop/shop.services.yml";
    let places = vec![
        target(MODULE, 26, "shop.catalog"),
        target(services, 1, "shop.catalog"),
        target(services, 7, "shop.catalog"),
    ];
    let source = at("service('shop.catalog')", "service('shop.cat/*|*/alog')");
    assert_eq!(project.drupal_usages(&source), places);
    assert_eq!(project.drupal_rename(&source, "shop.library"), Ok(places));

    // A route: its key, the links file, PHP.
    let source = at("fromRoute('shop.books')", "fromRoute('shop.bo/*|*/oks')");
    let found = project.drupal_rename(&source, "shop.list").expect("renamed");
    assert_eq!(
        found,
        [
            target("modules/shop/shop.links.menu.yml", 2, "shop.books"),
            target(MODULE, 29, "shop.books"),
            target("modules/shop/shop.routing.yml", 0, "shop.books"),
        ]
    );
    // A permission, with spaces: its key, the routes requiring it, PHP.
    let source = at("hasPermission('administer shop')", "hasPermission('admin/*|*/ister shop')");
    let found = project.drupal_rename(&source, "manage shop").expect("renamed");
    assert_eq!(
        found,
        [
            target(MODULE, 31, "administer shop"),
            target(MODULE, 32, "administer shop"),
            target("modules/shop/shop.permissions.yml", 0, "administer shop"),
            target("modules/shop/shop.routing.yml", 5, "administer shop"),
        ]
    );
    // A library: the key after the module's name, where the YAML files list it.
    let source = at("[] = 'shop/base'", "[] = 'shop/ba/*|*/se'");
    let found = project.drupal_rename(&source, "reset").expect("renamed");
    assert_eq!(
        found,
        [
            target("modules/shop/shop.info.yml", 4, "base"),
            target("modules/shop/shop.libraries.yml", 4, "base"),
            target("modules/shop/shop.libraries.yml", 5, "base"),
            target(MODULE, 34, "base"),
        ]
    );
    // Not a configuration, whose name is a file's.
    let found = project.drupal_rename(&at("config('shop.settings')", "config('shop.set/*|*/tings')"), "shop.other");
    assert!(found.is_err(), "{found:?}");
}

#[test]
fn entity_types_plugins_and_theme_hooks_come_from_php() {
    let mut project = drupal();
    let mut goes = |from: &str, to: &str| project.definition(MODULE, &at(from, to)).unwrap_or_default();
    // An annotation's id, and an attribute's.
    let book = "modules/shop/src/Entity/Book.php";
    assert_eq!(goes("getStorage('shop_book')", "getStorage('shop_/*|*/book')"), [target(book, 9, "shop_book")]);
    assert_eq!(goes("getStorage('shop_shelf')", "getStorage('shop_/*|*/shelf')"), [target(book, 22, "shop_shelf")]);
    assert_eq!(
        goes("createInstance('shop_cart')", "createInstance('shop_/*|*/cart')"),
        [target("modules/shop/src/Catalog.php", 14, "shop_cart")]
    );
    // A theme hook: the key of `hook_theme()`, and its template.
    assert_eq!(
        goes("'#theme' => 'shop_book'", "'#theme' => 'shop_/*|*/book'"),
        [target(MODULE, 8, "shop_book"), target("modules/shop/templates/shop-book.html.twig", 0, "")]
    );
    assert_eq!(
        goes("$build['#theme'] = 'shop_price_tag'", "$build['#theme'] = 'shop_price_/*|*/tag'"),
        [target(MODULE, 9, "shop_price_tag")]
    );

    let found = project.drupal_complete(&at("getStorage('shop_book')", "getStorage('/*|*/')"));
    assert_eq!(found, ["shop_book", "shop_shelf"]);
    let found = project.drupal_complete(&at("'#theme' => 'shop_book'", "'#theme' => '/*|*/'"));
    assert_eq!(found, ["shop_book", "shop_price_tag"]);
    // Every plugin for a manager that doesn't say its type.
    let found = project.drupal_complete(&at("createInstance('shop_cart')", "createInstance('/*|*/')"));
    assert_eq!(found, ["shop.books_link", "shop_cart", "shop_offers", "shop_price"]);
    let hover = project.drupal_hover(&at("getStorage('shop_book')", "getStorage('shop_/*|*/book')"));
    assert!(hover.contains("**entity type** `shop_book`") && hover.contains("Drupal\\shop\\Entity\\Book"), "{hover}");
}

#[test]
fn a_block_manager_offers_blocks() {
    let mut project = drupal();
    let source = "<?php\nfunction shop_blocks(\\Drupal\\Core\\Block\\BlockManagerInterface $blocks) {\n  $blocks->createInstance('/*|*/');\n}\n";
    assert_eq!(project.drupal_complete(source), ["shop_cart", "shop_offers"]);
}

#[test]
fn services_and_storages_are_typed() {
    let mut project = drupal();
    let mut members = |body: &str| {
        let source = format!("<?php\nfunction shop_probe($container) {{\n  {body}\n}}\n");
        let mut found = project.drupal_complete(&source);
        found.sort();
        found
    };
    assert_eq!(members("\\Drupal::service('shop.catalog')->/*|*/"), ["books"]);
    // Through an alias, and from a container.
    assert_eq!(members("\\Drupal::service('shop.books')->/*|*/"), ["books"]);
    let source = "<?php\nfunction shop_made(\\Psr\\Container\\ContainerInterface $container) {\n  $container->get('shop.prices')->/*|*/\n}\n";
    assert_eq!(project.drupal_complete(source), ["of"]);
    let mut members = |body: &str| {
        let source = format!("<?php\nfunction shop_probe() {{\n  {body}\n}}\n");
        let mut found = project.drupal_complete(&source);
        found.sort();
        found
    };
    // The storage handler the entity type names, and the entity it loads.
    let storage = "$storage = \\Drupal::entityTypeManager()->getStorage('shop_book');";
    assert_eq!(members(&format!("{storage}\n  $storage->/*|*/")), ["create", "load", "loadMultiple", "newest"]);
    assert_eq!(members(&format!("{storage}\n  $storage->load(1)->/*|*/")), ["id", "title"]);
    assert_eq!(members(&format!("{storage}\n  $storage->create()->/*|*/")), ["id", "title"]);
    assert_eq!(
        members(&format!("{storage}\n  foreach ($storage->loadMultiple() as $book) {{ $book->/*|*/ }}")),
        ["id", "title"]
    );
    // An entity type that names no storage: the interface, of its class.
    let shelves = "\\Drupal::entityTypeManager()->getStorage('shop_shelf')";
    assert_eq!(members(&format!("{shelves}->load(1)->/*|*/")), ["id", "room"]);
    // An entity type nothing declares says what Drupal declares.
    assert_eq!(members("\\Drupal::entityTypeManager()->getStorage('nothing')->load(1)->/*|*/"), ["id"]);
}

#[test]
fn a_hook_is_its_api_function() {
    let mut project = drupal();
    let api = "core/lib/shop.api.php";
    let found = project.definition(MODULE, &at("invokeAll('shop_books'", "invokeAll('shop_/*|*/books'"));
    assert_eq!(found, Some(vec![target(api, 12, "hook_shop_books")]));
    // `alter('form')` is `hook_form_alter`.
    let found = project.definition(MODULE, &at("alter('form'", "alter('fo/*|*/rm'"));
    assert_eq!(found, Some(vec![target(api, 9, "hook_form_alter")]));
    // An attribute's hook, by a placeholder.
    let source = "<?php\nnamespace Drupal\\shop\\Hook;\nuse Drupal\\Core\\Hook\\Attribute\\Hook;\nclass ShopHooks {\n  #[Hook('shop_book_/*|*/presave')]\n  public function saved($entity) {}\n}\n";
    assert_eq!(project.definition(MODULE, source), Some(vec![target(api, 8, "hook_ENTITY_TYPE_presave")]));

    let found = project.drupal_complete(&at("invokeAll('shop_books'", "invokeAll('/*|*/'"));
    assert_eq!(found, ["entity_presave", "form_alter", "shop_books", "theme"]);
    let found = project.drupal_complete(&at("alter('form'", "alter('/*|*/'"));
    assert_eq!(found, ["form"]);
    let hover = project.drupal_hover(&at("alter('form'", "alter('fo/*|*/rm'"));
    assert!(hover.contains("**hook** `form_alter`") && hover.contains("hook_form_alter()"), "{hover}");

    // The usages of a hook count the functions implementing it.
    let source = at("invokeAll('entity_presave'", "invokeAll('entity_/*|*/presave'");
    let found = project.drupal_usages(&source);
    assert!(found.contains(&target(MODULE, 13, "shop_entity_presave")), "{found:?}");
}

#[test]
fn a_hook_implementation_gets_the_hooks_parameter_types() {
    let mut project = drupal();
    let mut members = |from: &str, to: &str| {
        let mut found = project.drupal_complete(&at(from, to));
        found.sort();
        found
    };
    // By the hook's name, by a placeholder, and of an alter hook.
    assert_eq!(
        members(
            "function shop_entity_presave($entity) {\n  $entity->title;",
            "function shop_entity_presave($entity) {\n  $entity->/*|*/;"
        ),
        ["id", "title"]
    );
    assert_eq!(
        members(
            "function shop_shop_book_presave($entity) {\n  $entity->title;",
            "function shop_shop_book_presave($entity) {\n  $entity->/*|*/;"
        ),
        ["id", "title"]
    );
    assert_eq!(members("$form_state->books();", "$form_state->/*|*/;"), ["books"]);
    // A method with the attribute.
    let source = "<?php\nnamespace Drupal\\shop\\Hook;\nuse Drupal\\Core\\Hook\\Attribute\\Hook;\nclass ShopHooks {\n  #[Hook('shop_books')]\n  public function books($catalog) { $catalog->/*|*/ }\n}\n";
    assert_eq!(project.drupal_complete(source), ["books"]);
    // A function that is no hook's stays as it is.
    let source = "<?php\nfunction shop_helper($entity) { $entity->/*|*/ }\n";
    assert!(project.drupal_complete(source).is_empty());
}

#[test]
fn drupal_names_are_only_read_in_a_drupal_project() {
    let mut project =
        Project::new(&[("lib/Drupal.php", CORE), ("modules/shop/shop.routing.yml", ROUTING), (MODULE, "<?php\n")]);
    let found = project.definition(MODULE, &at("fromRoute('shop.books')", "fromRoute('shop.bo/*|*/oks')"));
    assert!(found.as_ref().is_none_or(Vec::is_empty), "{found:?}");
}

/// More of Drupal's API, for the tests below.
const MORE: &str = r#"<?php
namespace Drupal\Core\Config {
    abstract class ConfigBase { public function get($key = '') {} }
    class Config extends ConfigBase { public function set($key, $value) {} }
    interface ConfigFactoryInterface {
        /** @return \Drupal\Core\Config\Config */
        public function getEditable($name);
    }
}
namespace Drupal\Core\Menu {
    interface MenuLinkManagerInterface extends \Drupal\Component\Plugin\PluginManagerInterface {}
}
"#;

const SETTINGS: &str = "currency: EUR\nmail:\n  from: shop@example.com\n  copies: 2\nlangcode: en\n";

fn more() -> Project {
    Project::new(&[
        ("core/lib/Drupal.php", CORE),
        ("core/lib/More.php", MORE),
        ("core/lib/shop.api.php", API),
        ("modules/shop/shop.info.yml", INFO),
        ("modules/shop/shop.services.yml", SERVICES),
        ("modules/shop/shop.routing.yml", ROUTING),
        ("modules/shop/shop.links.menu.yml", LINKS),
        ("modules/shop/shop.permissions.yml", PERMISSIONS),
        ("modules/shop/shop.libraries.yml", LIBRARIES),
        ("modules/shop/config/install/shop.settings.yml", SETTINGS),
        ("modules/shop/templates/shop-book.html.twig", "<h2>{{ book }}</h2>\n{{ attach_library('shop/cart') }}\n"),
        ("modules/shop/templates/shop-book--large.html.twig", "<h1>{{ book }}</h1>\n"),
        ("modules/shop/src/Entity/Book.php", ENTITY),
        ("modules/shop/src/Catalog.php", CLASSES),
        (
            "modules/shop/shop.theme.inc",
            "<?php\nfunction shop_preprocess_shop_book(&$variables) {}\nfunction template_preprocess_shop_book(&$variables) {}\n",
        ),
        (MODULE, SHOP),
    ])
}

#[test]
fn the_example_code_of_an_api_file_is_left_alone() {
    let mut project = more();
    let reported = |project: &mut Project, file: &str, text: &str| {
        let doc = project.open(file, text);
        let found = project.engine.inspect(doc, &CancellationToken::new()).expect("open");
        project.engine.document_closed(doc);
        found.problems.iter().filter(|problem| problem.inspection == "PhpUnusedParameter").count()
    };
    assert_eq!(reported(&mut project, "core/lib/shop.api.php", API), 0);
    // The same functions in another file are reported.
    assert!(reported(&mut project, "core/lib/hooks.php", API) > 0);
}

#[test]
fn an_implementation_goes_to_its_hook() {
    let mut project = more();
    let api = "core/lib/shop.api.php";
    let found = project.definition(MODULE, &at("function shop_entity_presave", "function shop_entity_/*|*/presave"));
    assert_eq!(found, Some(vec![target(api, 4, "hook_entity_presave")]));
    // By a placeholder.
    let found =
        project.definition(MODULE, &at("function shop_shop_book_presave", "function shop_shop_book_/*|*/presave"));
    assert_eq!(found, Some(vec![target(api, 8, "hook_ENTITY_TYPE_presave")]));
    // A function that implements none is itself.
    let found = project.definition(MODULE, &at("function shop_page", "function shop_/*|*/page"));
    assert_eq!(found, Some(vec![target(MODULE, 25, "shop_page")]));
}

#[test]
fn a_theme_suggestion_is_its_hook_and_a_template_has_its_variables() {
    let mut project = more();
    // The suggestion's own template before the hook's.
    let source = at("'#theme' => 'shop_book'", "'#theme' => 'shop_book__/*|*/large'");
    assert_eq!(
        project.definition(MODULE, &source),
        Some(vec![target(MODULE, 8, "shop_book"), target("modules/shop/templates/shop-book--large.html.twig", 0, "")])
    );
    let source = at("'#theme' => 'shop_book'", "'#theme' => 'shop_book__/*|*/other'");
    assert_eq!(
        project.definition(MODULE, &source),
        Some(vec![target(MODULE, 8, "shop_book"), target("modules/shop/templates/shop-book.html.twig", 0, "")])
    );
    // The functions preparing its variables are usages of it.
    let source = at("'#theme' => 'shop_book'", "'#theme' => 'shop_/*|*/book'");
    let found = project.drupal_usages(&source);
    let functions: Vec<&str> =
        found.iter().map(|(_, _, text)| text.as_str()).filter(|text| text.contains("preprocess")).collect();
    assert_eq!(functions, ["shop_preprocess_shop_book", "template_preprocess_shop_book"]);

    // What the hook's template is given, and what every template has.
    let variables = |project: &Project, template: &str| {
        let found = project.engine.twig_template_variables(template);
        found.into_iter().map(|(name, _)| name).collect::<Vec<_>>()
    };
    let found = variables(&project, "@shop/shop-book.html.twig");
    assert!(found.contains(&"book".to_owned()) && found.contains(&"attributes".to_owned()), "{found:?}");
    assert!(variables(&project, "@shop/shop-book--large.html.twig").contains(&"book".to_owned()));
    assert!(variables(&project, "@shop/nothing.html.twig").is_empty());
}

#[test]
fn a_yaml_file_declares_plugins() {
    let mut project = more();
    let source = "<?php\nfunction shop_links(\\Drupal\\Core\\Menu\\MenuLinkManagerInterface $links, \\Drupal\\Component\\Plugin\\PluginManagerInterface $blocks) {\n  $links->createInstance('/*|*/');\n}\n";
    assert_eq!(project.drupal_complete(source), ["shop.books_link"]);
    let source = source.replace("'/*|*/'", "'shop.books_/*|*/link'");
    assert_eq!(
        project.definition(MODULE, &source),
        Some(vec![target("modules/shop/shop.links.menu.yml", 0, "shop.books_link")])
    );
    // A derivative is its base plugin.
    let source = "<?php\nfunction shop_derived(\\Drupal\\Component\\Plugin\\PluginManagerInterface $blocks) {\n  $blocks->createInstance('shop_cart:ma/*|*/in');\n}\n";
    assert_eq!(project.definition(MODULE, source), Some(vec![target("modules/shop/src/Catalog.php", 14, "shop_cart")]));
}

#[test]
fn a_configurations_keys_are_its_files() {
    let mut project = more();
    let settings = "modules/shop/config/install/shop.settings.yml";
    let probe = |body: &str| {
        format!(
            "<?php\nfunction shop_settings(\\Drupal\\Core\\Config\\ConfigFactoryInterface $factory) {{\n  {body}\n}}\n"
        )
    };
    // Straight on the call, through a variable, and from the factory; a nested key.
    let found = project.definition(MODULE, &probe("return \\Drupal::config('shop.settings')->get('curr/*|*/ency');"));
    assert_eq!(found, Some(vec![target(settings, 0, "currency")]));
    let found = project.definition(
        MODULE,
        &probe("$config = \\Drupal::config('shop.settings');\n  return $config->get('mail.fr/*|*/om');"),
    );
    assert_eq!(found, Some(vec![target(settings, 2, "from")]));
    let found =
        project.definition(MODULE, &probe("$factory->getEditable('shop.settings')->set('mail.cop/*|*/ies', 3);"));
    assert_eq!(found, Some(vec![target(settings, 3, "copies")]));
    let found = project.drupal_complete(&probe("return \\Drupal::config('shop.settings')->get('/*|*/');"));
    assert_eq!(found, ["currency", "mail", "mail.copies", "mail.from"]);
    let hover = project.drupal_hover(&probe("return \\Drupal::config('shop.settings')->get('curr/*|*/ency');"));
    assert!(
        hover.contains("**configuration key** `currency` of `shop.settings`") && hover.contains("`EUR`"),
        "{hover}"
    );
    // A configuration that isn't known says nothing.
    let found = project.definition(MODULE, &probe("return \\Drupal::config('other.settings')->get('curr/*|*/ency');"));
    assert!(found.as_ref().is_none_or(Vec::is_empty), "{found:?}");
}

#[test]
fn a_yaml_file_and_a_template_are_answered_in() {
    let project = more();
    let root = project.engine.root();
    let caret = |text: &str, needle: &str, offset: usize| {
        let at = text.find(needle).unwrap_or_else(|| panic!("{needle}")) + offset;
        let line = text[..at].matches('\n').count() as u32;
        Pos::new(line, text[..at].rsplit('\n').next().unwrap().len() as u32)
    };
    let goes = |file: &str, text: &str, needle: &str, offset: usize| {
        let found = project.engine.drupal_document_definition(&root.join(file), text, caret(text, needle, offset));
        found.unwrap_or_default().iter().map(|target| project.describe(&target.path, target.range)).collect::<Vec<_>>()
    };
    let services = "modules/shop/shop.services.yml";
    let routing = "modules/shop/shop.routing.yml";
    // A service after `@` and after `alias:`; a class and a controller's method.
    assert_eq!(goes(services, SERVICES, "'@shop.prices'", 4), [target(services, 4, "shop.prices")]);
    assert_eq!(goes(services, SERVICES, "alias: shop.catalog", 10), [target(services, 1, "shop.catalog")]);
    assert_eq!(
        goes(services, SERVICES, "Drupal\\shop\\Catalog", 14),
        [target("modules/shop/src/Catalog.php", 2, "Catalog")]
    );
    // A permission among those a route lists, and a route in a links file.
    let permissions = "modules/shop/shop.permissions.yml";
    assert_eq!(goes(routing, ROUTING, "access content'", 3), [target(permissions, 2, "access content")]);
    assert_eq!(goes(routing, ROUTING, "administer shop+", 3), [target(permissions, 0, "administer shop")]);
    assert_eq!(
        goes("modules/shop/shop.links.menu.yml", LINKS, "route_name: shop.books", 14),
        [target(routing, 0, "shop.books")]
    );
    // A library in the lists of an info and a libraries file.
    let libraries = "modules/shop/shop.libraries.yml";
    assert_eq!(goes("modules/shop/shop.info.yml", INFO, "- shop/base", 5), [target(libraries, 5, "base")]);
    assert_eq!(goes(libraries, LIBRARIES, "- shop/base", 5), [target(libraries, 5, "base")]);
    // Not a word that names nothing.
    assert!(goes(routing, ROUTING, "path: '/books'", 8).is_empty());

    // A key declaring a name: its usages, and its rename.
    let cancel = CancellationToken::new();
    let found = project.engine.drupal_document_references(
        &root.join(routing),
        ROUTING,
        caret(ROUTING, "shop.books:", 3),
        true,
        &cancel,
    );
    let found: Vec<_> =
        found.unwrap().unwrap().iter().map(|usage| project.describe(&usage.path, usage.range)).collect();
    assert_eq!(
        found,
        [
            target("modules/shop/shop.links.menu.yml", 2, "shop.books"),
            target(MODULE, 29, "shop.books"),
            target(routing, 0, "shop.books"),
        ]
    );
    let edit = project.engine.drupal_document_rename(
        &root.join(libraries),
        LIBRARIES,
        caret(LIBRARIES, "base:", 1),
        "reset",
        &cancel,
    );
    let mut edits = Vec::new();
    for file in &edit.unwrap().unwrap().files {
        let crate::base::FileEdit::Edit { path, edits: found, .. } = file else { panic!("only text edits") };
        edits.extend(found.iter().map(|edit| project.describe(path, edit.range)));
    }
    edits.sort();
    assert_eq!(
        edits,
        [
            target("modules/shop/shop.info.yml", 4, "base"),
            target(libraries, 4, "base"),
            target(libraries, 5, "base"),
            target(MODULE, 34, "base"),
        ]
    );
    let hover =
        project.engine.drupal_document_hover(&root.join(services), SERVICES, caret(SERVICES, "'@shop.prices'", 4));
    assert!(hover.is_some_and(|hover| hover.markdown.contains("**service** `shop.prices`")));

    // Completion by the line's key.
    let offered = |file: &str, text: &str, needle: &str, offset: usize| {
        let found = project.engine.drupal_document_completion(&root.join(file), text, caret(text, needle, offset));
        found.map(|found| found.into_iter().map(|item| item.label).collect::<Vec<_>>())
    };
    let links = "shop.other:\n  route_name: \n";
    assert_eq!(
        offered("modules/shop/shop.links.menu.yml", links, "route_name: ", 12),
        Some(vec!["shop.books".to_owned(), "shop.book".to_owned()])
    );
    let wired = "services:\n  shop.other:\n    arguments: ['@']\n";
    let found = offered(services, wired, "'@'", 2).unwrap();
    assert!(found.contains(&"shop.catalog".to_owned()) && found.contains(&"shop.prices".to_owned()), "{found:?}");
    assert_eq!(offered(routing, "shop.x:\n  path: ''\n", "path: ''", 7), None);

    // In a template: the library given to `attach_library()`.
    let template = "modules/shop/templates/shop-book.html.twig";
    let text = "<h2>{{ book }}</h2>\n{{ attach_library('shop/cart') }}\n{{ attach_library('') }}\n";
    assert_eq!(goes(template, text, "'shop/cart'", 4), [target(libraries, 0, "cart")]);
    assert_eq!(
        offered(template, text, "attach_library('')", 16),
        Some(vec!["shop/cart".to_owned(), "shop/base".to_owned()])
    );
    assert!(goes(template, text, "book }}", 1).is_empty());
}

#[test]
fn a_hook_on_a_class_types_the_method_it_names() {
    let mut project = more();
    let source = "<?php\nnamespace Drupal\\shop\\Hook;\nuse Drupal\\Core\\Hook\\Attribute\\Hook;\n#[Hook('shop_books', method: 'run')]\nclass Books {\n  public function run($catalog) { $catalog->/*|*/ }\n}\n";
    assert_eq!(project.drupal_complete(source), ["books"]);
    // Without a method: `__invoke`.
    let source = "<?php\nnamespace Drupal\\shop\\Hook;\nuse Drupal\\Core\\Hook\\Attribute\\Hook;\n#[Hook('shop_books')]\nclass Invoked {\n  public function __invoke($catalog) { $catalog->/*|*/ }\n}\n";
    assert_eq!(project.drupal_complete(source), ["books"]);
}

#[test]
fn a_schema_describes_a_configuration_no_file_installs() {
    let schema = "shop.limits:\n  type: config_object\n  label: 'Limits'\n  mapping:\n    books:\n      type: integer\n      label: 'Books a visitor may hold'\n    mail:\n      type: mapping\n      mapping:\n        from:\n          type: string\nshop.shelf.*:\n  type: config_entity\n  mapping:\n    room:\n      type: string\n";
    let mut project = Project::new(&[
        ("core/lib/Drupal.php", CORE),
        ("core/lib/More.php", MORE),
        ("modules/shop/shop.info.yml", INFO),
        ("modules/shop/config/schema/shop.schema.yml", schema),
        (MODULE, "<?php\n"),
    ]);
    let file = "modules/shop/config/schema/shop.schema.yml";
    let probe = |body: &str| format!("<?php\nfunction shop_limits() {{\n  {body}\n}}\n");
    // The configuration's name, and its keys.
    let found = project.definition(MODULE, &probe("return \\Drupal::config('shop.lim/*|*/its');"));
    assert_eq!(found, Some(vec![target(file, 0, "shop.limits")]));
    let found = project.definition(MODULE, &probe("return \\Drupal::config('shop.limits')->get('mail.fr/*|*/om');"));
    assert_eq!(found, Some(vec![target(file, 10, "from")]));
    let found = project.drupal_complete(&probe("return \\Drupal::config('shop.limits')->get('/*|*/');"));
    assert_eq!(found, ["books", "mail", "mail.from"]);
    // One of several configurations a pattern describes.
    let found = project.definition(MODULE, &probe("return \\Drupal::config('shop.shelf.main')->get('ro/*|*/om');"));
    assert_eq!(found, Some(vec![target(file, 15, "room")]));
    // The pattern itself is no configuration to offer.
    let found = project.drupal_complete(&probe("return \\Drupal::config('/*|*/');"));
    assert_eq!(found, ["shop.limits"]);
}

#[test]
fn a_yaml_line_is_read_under_the_key_above_it() {
    let project = more();
    let root = project.engine.root();
    let caret = |text: &str, needle: &str, offset: usize| {
        let at = text.find(needle).unwrap_or_else(|| panic!("{needle}")) + offset;
        let line = text[..at].matches('\n').count() as u32;
        Pos::new(line, text[..at].rsplit('\n').next().unwrap().len() as u32)
    };
    let goes = |file: &str, text: &str, needle: &str, offset: usize| {
        let found = project.engine.drupal_document_definition(&root.join(file), text, caret(text, needle, offset));
        found.unwrap_or_default().iter().map(|target| project.describe(&target.path, target.range)).collect::<Vec<_>>()
    };
    let offered = |file: &str, text: &str, needle: &str, offset: usize| {
        let found = project.engine.drupal_document_completion(&root.join(file), text, caret(text, needle, offset));
        found.map(|found| found.into_iter().map(|item| item.label).collect::<Vec<_>>())
    };
    let services = "modules/shop/shop.services.yml";
    // A list over several lines, as items and in brackets.
    let wired = "services:\n  shop.other:\n    class: Drupal\\shop\\Catalog\n    arguments:\n      - '@shop.prices'\n    calls: [\n      '@shop.catalog',\n    ]\n    parent:\n      shop.prices\n";
    assert_eq!(goes(services, wired, "- '@shop.prices'", 5), [target(services, 4, "shop.prices")]);
    assert_eq!(goes(services, wired, "'@shop.catalog',", 4), [target(services, 1, "shop.catalog")]);
    // The value on the line after its key.
    assert_eq!(goes(services, wired, "      shop.prices\n", 9), [target(services, 4, "shop.prices")]);
    let links = "shop.other:\n  route_name:\n    shop.books\n";
    assert_eq!(
        goes("modules/shop/shop.links.menu.yml", links, "    shop.books", 7),
        [target("modules/shop/shop.routing.yml", 0, "shop.books")]
    );

    // `service:method`: the service, and the method of its class.
    let routing = "shop.x:\n  path: '/x'\n  defaults:\n    _controller: 'shop.catalog:books'\n";
    let file = "modules/shop/shop.routing.yml";
    assert_eq!(goes(file, routing, "shop.catalog:books", 4), [target(services, 1, "shop.catalog")]);
    assert_eq!(goes(file, routing, "shop.catalog:books", 15), [target("modules/shop/src/Catalog.php", 2, "books")]);

    // A class: the namespaces and classes below what is written.
    let typed = "services:\n  shop.other:\n    class: Drupal\\shop\\\n";
    let found = offered(services, typed, "Drupal\\shop\\\n", 12).unwrap();
    assert!(
        found.contains(&"Drupal\\shop\\Catalog".to_owned()) && found.contains(&"Drupal\\shop\\Entity".to_owned()),
        "{found:?}"
    );
    assert!(!found.iter().any(|label| label.starts_with("Drupal\\Core")), "{found:?}");
}

#[test]
fn a_preprocess_function_types_the_templates_variables() {
    let prepared = "<?php\nfunction shop_preprocess_shop_book(&$variables) {\n  $variables['catalog'] = \\Drupal::service('shop.catalog');\n  $variables['book'] = new \\Drupal\\shop\\Entity\\Book();\n  $other['book'] = 1;\n}\nfunction shop_elsewhere(&$variables) {\n  $variables['nothing'] = 1;\n}\n";
    let project = Project::new(&[
        ("core/lib/Drupal.php", CORE),
        ("modules/shop/shop.info.yml", INFO),
        ("modules/shop/shop.services.yml", SERVICES),
        ("modules/shop/src/Entity/Book.php", ENTITY),
        ("modules/shop/src/Catalog.php", CLASSES),
        ("modules/shop/shop.theme.inc", prepared),
        (MODULE, SHOP),
    ]);
    let found = project.engine.twig_template_variables("@shop/shop-book.html.twig");
    let of = |name: &str| found.iter().find(|(known, _)| known == name).map(|(_, ty)| ty.to_string());
    assert_eq!(of("book").as_deref(), Some("Drupal\\shop\\Entity\\Book"));
    // One the function adds to those `hook_theme()` names.
    assert_eq!(of("catalog").as_deref(), Some("Drupal\\shop\\Catalog"));
    assert_eq!(of("attributes").as_deref(), Some("mixed"));
    assert_eq!(of("nothing"), None);
}

/// A Drupal 7 site: its hooks are functions, its modules have `.info` files.
fn drupal_7() -> Project {
    let api = "<?php\n/** @param \\stdClass $node The node. */\nfunction hook_node_presave($node) {}\nfunction hook_form_alter(&$form, &$form_state, $form_id) {}\nfunction hook_permission() {}\nfunction hook_theme($existing, $type, $theme, $path) {}\nfunction hook_shop_books($count) {}\n";
    let core = "<?php\nfunction module_invoke_all($hook) {}\nfunction module_implements($hook) {}\nfunction drupal_alter($type, &$data) {}\nfunction theme($hook, $variables = array()) {}\nfunction user_access($string, $account = NULL) {}\nfunction module_exists($module) {}\n";
    let module = "<?php\nfunction shop_permission() {\n  return array(\n    'administer shop' => array('title' => t('Administer the shop')),\n  );\n}\nfunction shop_theme() {\n  return array(\n    'shop_book' => array('variables' => array('book' => NULL)),\n  );\n}\nfunction theme_shop_book($variables) {}\nfunction shop_node_presave($node) {}\nfunction shop_page() {\n  if (user_access('administer shop') && module_exists('shop')) {\n    module_invoke_all('shop_books', 1);\n    drupal_alter('form', $form);\n    return theme('shop_book', array());\n  }\n}\n";
    Project::new(&[
        ("modules/system/system.module", core),
        ("modules/system/system.api.php", api),
        ("sites/all/modules/shop/shop.info", "name = Shop\ncore = 7.x\n"),
        ("sites/all/modules/shop/shop.module", module),
    ])
}

#[test]
fn drupal_7_has_the_same_hooks_under_other_functions() {
    const SEVEN: &str = "sites/all/modules/shop/shop.module";
    let mut project = drupal_7();
    assert!(project.engine.frameworks().contains(crate::frameworks::Framework::Drupal));
    let text = std::fs::read_to_string(project.engine.root().join(SEVEN)).unwrap();
    let mut goes = |from: &str, to: &str| {
        assert!(text.contains(from), "{from}");
        project.definition(SEVEN, &text.replacen(from, to, 1)).unwrap_or_default()
    };
    let api = "modules/system/system.api.php";
    assert_eq!(
        goes("module_invoke_all('shop_books'", "module_invoke_all('shop_/*|*/books'"),
        [target(api, 6, "hook_shop_books")]
    );
    assert_eq!(goes("drupal_alter('form'", "drupal_alter('fo/*|*/rm'"), [target(api, 3, "hook_form_alter")]);
    // A permission is a key of `hook_permission()`, a module its `.info`.
    assert_eq!(
        goes("user_access('administer shop')", "user_access('admin/*|*/ister shop')"),
        [target(SEVEN, 3, "administer shop")]
    );
    let found = goes("module_exists('shop')", "module_exists('sh/*|*/op')");
    assert_eq!(found.iter().map(|(file, ..)| file.as_str()).collect::<Vec<_>>(), ["sites/all/modules/shop/shop.info"]);
    // A theme hook: its key, and the function rendering it.
    assert_eq!(
        goes("theme('shop_book'", "theme('shop_/*|*/book'"),
        [target(SEVEN, 8, "shop_book"), target(SEVEN, 11, "theme_shop_book")]
    );
    // From an implementation to its hook.
    assert_eq!(
        goes("function shop_node_presave", "function shop_node_/*|*/presave"),
        [target(api, 2, "hook_node_presave")]
    );
}

#[test]
fn drupal_7_menu_paths_variables_and_form_functions() {
    const SEVEN: &str = "sites/all/modules/shop/shop.module";
    let module = "<?php\nfunction shop_menu() {\n  $items['admin/shop'] = array(\n    'title' => 'Shop',\n    'page callback' => 'drupal_get_form',\n    'page arguments' => array('shop_settings_form'),\n    'access callback' => 'shop_access',\n  );\n  $items['shop/%shop_book/edit'] = array('page callback' => 'shop_edit');\n  return $items;\n}\nfunction shop_access() {}\nfunction shop_edit($book) {}\nfunction shop_settings_form($form, &$form_state) {}\nfunction shop_install() {\n  variable_set('shop_currency', 'EUR');\n}\nfunction shop_use() {\n  $currency = variable_get('shop_currency', 'USD');\n  drupal_goto('admin/shop');\n  $link = l('Edit', 'shop/12/edit');\n  return drupal_get_form('shop_settings_form');\n}\n";
    let core = "<?php\nfunction variable_get($name, $default = NULL) {}\nfunction variable_set($name, $value) {}\nfunction drupal_goto($path = '') {}\nfunction l($text, $path, array $options = array()) {}\nfunction drupal_get_form($form_id) {}\n";
    let mut project = Project::new(&[
        ("modules/system/system.module", core),
        ("sites/all/modules/shop/shop.info", "name = Shop\ncore = 7.x\n"),
        (SEVEN, module),
    ]);
    let mut goes = |from: &str, to: &str| {
        assert!(module.contains(from), "{from}");
        project.definition(SEVEN, &module.replacen(from, to, 1)).unwrap_or_default()
    };
    // A path of `hook_menu()`, written out and through a `%` placeholder.
    assert_eq!(goes("drupal_goto('admin/shop')", "drupal_goto('admin/sh/*|*/op')"), [target(SEVEN, 2, "admin/shop")]);
    assert_eq!(
        goes("l('Edit', 'shop/12/edit')", "l('Edit', 'shop/12/ed/*|*/it')"),
        [target(SEVEN, 8, "shop/%shop_book/edit")]
    );
    // A variable, where it is set.
    assert_eq!(
        goes("variable_get('shop_currency'", "variable_get('shop_/*|*/currency'"),
        [target(SEVEN, 15, "shop_currency")]
    );
    // A form is its function; a menu item's callbacks are functions.
    assert_eq!(
        goes("drupal_get_form('shop_settings_form')", "drupal_get_form('shop_settings_/*|*/form')"),
        [target(SEVEN, 13, "shop_settings_form")]
    );
    assert_eq!(
        goes("'access callback' => 'shop_access'", "'access callback' => 'shop_/*|*/access'"),
        [target(SEVEN, 11, "shop_access")]
    );
    assert_eq!(
        goes("'page callback' => 'shop_edit'", "'page callback' => 'shop_/*|*/edit'"),
        [target(SEVEN, 12, "shop_edit")]
    );
}

#[test]
fn a_schema_types_a_configurations_keys() {
    let schema = "shop.limits:\n  type: config_object\n  mapping:\n    books:\n      type: integer\n    open:\n      type: boolean\n    label:\n      type: label\n";
    let mut project = Project::new(&[
        ("core/lib/Drupal.php", CORE),
        ("core/lib/More.php", MORE),
        ("modules/shop/shop.info.yml", INFO),
        ("modules/shop/config/schema/shop.schema.yml", schema),
        (MODULE, "<?php\n"),
    ]);
    let mut hover = |key: &str| {
        let source = format!(
            "<?php\nfunction shop_limits() {{\n  $value = \\Drupal::config('shop.limits')->get('{key}');\n  return $va/*|*/lue;\n}}\n"
        );
        project.drupal_hover(&source)
    };
    assert!(hover("books").contains("int"), "{}", hover("books"));
    assert!(hover("open").contains("bool"), "{}", hover("open"));
    assert!(hover("label").contains("string"), "{}", hover("label"));
    assert!(!hover("nothing").contains("int"));
}

#[test]
fn a_render_array_types_the_templates_variables() {
    let page = "<?php\nfunction shop_show() {\n  $book = new \\Drupal\\shop\\Entity\\Book();\n  return ['#theme' => 'shop_book', '#book' => $book, '#weight' => 5];\n}\n";
    let project = Project::new(&[
        ("core/lib/Drupal.php", CORE),
        ("modules/shop/shop.info.yml", INFO),
        ("modules/shop/src/Entity/Book.php", ENTITY),
        ("modules/shop/shop.pages.inc", page),
        (MODULE, SHOP),
    ]);
    let found = project.engine.twig_template_variables("@shop/shop-book.html.twig");
    let of = |name: &str| found.iter().find(|(known, _)| known == name).map(|(_, ty)| ty.to_string());
    assert_eq!(of("book").as_deref(), Some("Drupal\\shop\\Entity\\Book"));
    // A property of the render array that is no variable of the hook.
    assert_eq!(of("weight"), None);
}

#[test]
fn braces_aliases_and_info_files_are_read() {
    let project = Project::new(&[
        ("core/lib/Drupal.php", CORE),
        ("modules/shop/shop.info.yml", INFO),
        ("modules/shop/shop.routing.yml", ROUTING),
        ("modules/shop/shop.services.yml", SERVICES),
        (
            "sites/all/modules/old/old.info",
            "name = Old\ncore = 7.x\ndependencies[] = shop (>=1.0)\nfiles[] = old.test\n",
        ),
        ("sites/all/modules/old/old.test", "<?php\n"),
        (MODULE, "<?php\n"),
    ]);
    let root = project.engine.root();
    let caret = |text: &str, needle: &str, offset: usize| {
        let at = text.find(needle).unwrap_or_else(|| panic!("{needle}")) + offset;
        let line = text[..at].matches('\n').count() as u32;
        Pos::new(line, text[..at].rsplit('\n').next().unwrap().len() as u32)
    };
    let goes = |file: &str, text: &str, needle: &str, offset: usize| {
        let found = project.engine.drupal_document_definition(&root.join(file), text, caret(text, needle, offset));
        found.unwrap_or_default().iter().map(|target| project.describe(&target.path, target.range)).collect::<Vec<_>>()
    };
    // A mapping in braces: the key before the caret.
    let links = "shop.one: { title: 'Books', route_name: shop.books, weight: 1 }\n";
    assert_eq!(
        goes("modules/shop/shop.links.menu.yml", links, "shop.books,", 7),
        [target("modules/shop/shop.routing.yml", 0, "shop.books")]
    );
    assert!(goes("modules/shop/shop.links.menu.yml", links, "'Books'", 3).is_empty());
    // An alias goes to its anchor (in the document as it is, whatever is saved).
    let services = "services:
  _defaults: &defaults
    public: true
  shop.other:
    <<: *defaults
    class: X
";
    let file = root.join("modules/shop/shop.services.yml");
    let found = project.engine.drupal_document_definition(&file, services, caret(services, "*defaults", 4)).unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!((found[0].path.clone(), found[0].range.start.line, found[0].range.start.col), (file, 1, 14));
    // Drupal 7's `.info`: a module it depends on, and a file it lists.
    let info = "name = Old\ncore = 7.x\ndependencies[] = shop (>=1.0)\nfiles[] = old.test\n";
    let found = goes("sites/all/modules/old/old.info", info, "shop (>=1.0)", 2);
    assert_eq!(found.iter().map(|(file, ..)| file.as_str()).collect::<Vec<_>>(), ["modules/shop/shop.info.yml"]);
    let found = goes("sites/all/modules/old/old.info", info, "old.test", 3);
    assert_eq!(found.iter().map(|(file, ..)| file.as_str()).collect::<Vec<_>>(), ["sites/all/modules/old/old.test"]);
}

#[test]
fn a_service_merged_from_an_anchor_has_its_class() {
    let services = "services:\n  _base: &base\n    class: Drupal\\shop\\Catalog\n    public: true\n  shop.merged:\n    <<: *base\n    arguments: []\n";
    let mut project = Project::new(&[
        ("core/lib/Drupal.php", CORE),
        ("modules/shop/shop.info.yml", INFO),
        ("modules/shop/shop.services.yml", services),
        ("modules/shop/src/Catalog.php", CLASSES),
        (MODULE, "<?php\n"),
    ]);
    let source = "<?php\nfunction shop_probe() {\n  \\Drupal::service('shop.merged')->/*|*/\n}\n";
    assert_eq!(project.drupal_complete(source), ["books"]);
}

#[test]
fn a_render_array_built_up_types_the_templates_variables() {
    let page = "<?php\nfunction shop_show() {\n  $build = [];\n  $build['#theme'] = 'shop_book';\n  $build['#book'] = new \\Drupal\\shop\\Entity\\Book();\n  return $build;\n}\nfunction shop_other() {\n  $build['#book'] = 5;\n  $list = ['#theme' => 'shop_price_tag'];\n  $list['#price'] = 1.5;\n  return [$build, $list];\n}\n";
    let project = Project::new(&[
        ("core/lib/Drupal.php", CORE),
        ("modules/shop/shop.info.yml", INFO),
        ("modules/shop/src/Entity/Book.php", ENTITY),
        ("modules/shop/shop.pages.inc", page),
        (MODULE, SHOP),
    ]);
    let of = |template: &str, name: &str| {
        let found = project.engine.twig_template_variables(template);
        found.iter().find(|(known, _)| known == name).map(|(_, ty)| ty.to_string())
    };
    // Only what the function theming the variable assigns to it.
    assert_eq!(of("@shop/shop-book.html.twig", "book").as_deref(), Some("Drupal\\shop\\Entity\\Book"));
    // An array assigned whole, then added to.
    assert_eq!(of("@shop/shop-price-tag.html.twig", "price").as_deref(), Some("float"));
}

#[test]
fn a_configuration_key_has_the_type_of_the_value_installed() {
    let mut project = more();
    let mut hover = |key: &str| {
        let source = format!(
            "<?php\nfunction shop_settings() {{\n  $value = \\Drupal::config('shop.settings')->get('{key}');\n  return $va/*|*/lue;\n}}\n"
        );
        project.drupal_hover(&source)
    };
    assert!(hover("currency").contains("string"), "{}", hover("currency"));
    assert!(hover("mail.copies").contains("int"), "{}", hover("mail.copies"));
    assert!(hover("mail").contains("array"), "{}", hover("mail"));
}

#[test]
fn a_table_is_a_key_of_hook_schema() {
    const INSTALL: &str = "sites/all/modules/shop/shop.install";
    let install = "<?php\nfunction shop_schema() {\n  $schema['shop_book'] = array(\n    'fields' => array('bid' => array('type' => 'serial')),\n  );\n  $schema['shop_shelf'] = array('fields' => array());\n  return $schema;\n}\nfunction shop_update_7001() {\n  $rows = db_select('shop_book', 'b')->execute();\n  db_insert('shop_shelf');\n  db_delete('');\n}\n";
    let core = "<?php\nfunction db_select($table, $alias = NULL) {}\nfunction db_insert($table) {}\nfunction db_delete($table) {}\n";
    let mut project = Project::new(&[
        ("modules/system/system.module", core),
        ("sites/all/modules/shop/shop.info", "name = Shop\ncore = 7.x\n"),
        (INSTALL, install),
    ]);
    let found =
        project.definition(INSTALL, &install.replacen("db_select('shop_book'", "db_select('shop_/*|*/book'", 1));
    assert_eq!(found, Some(vec![target(INSTALL, 2, "shop_book")]));
    let found =
        project.definition(INSTALL, &install.replacen("db_insert('shop_shelf')", "db_insert('shop_/*|*/shelf')", 1));
    assert_eq!(found, Some(vec![target(INSTALL, 5, "shop_shelf")]));
    let (text, pos) = caret(&install.replacen("db_delete('')", "db_delete('/*|*/')", 1));
    let doc = project.open(INSTALL, &text);
    let found = project.engine.completion(doc, pos, None, &CancellationToken::new()).expect("handled");
    project.engine.document_closed(doc);
    let labels: Vec<&str> = found.items.iter().map(|item| item.label.as_str()).collect();
    assert_eq!(labels, ["shop_book", "shop_shelf"]);
}

#[test]
fn a_column_is_a_field_of_its_tables_schema() {
    const INSTALL: &str = "sites/all/modules/shop/shop.install";
    let install = "<?php\nfunction shop_schema() {\n  $schema['shop_book'] = array(\n    'fields' => array(\n      'bid' => array('type' => 'serial'),\n      'title' => array('type' => 'varchar'),\n    ),\n    'primary key' => array('bid'),\n  );\n  $schema['shop_shelf'] = array('fields' => array('sid' => array('type' => 'serial')));\n  return $schema;\n}\nfunction shop_rows() {\n  $query = db_select('shop_book', 'b');\n  $query->condition('b.bid', 1);\n  $query->fields('b', array('title'));\n  $query->orderBy('');\n  db_select('shop_shelf', 's')->condition('sid', 2);\n  return $query;\n}\n";
    let core = "<?php\nclass SelectQuery {\n  public function condition($field, $value = NULL) {}\n  public function fields($alias, array $fields = array()) {}\n  public function orderBy($field, $direction = 'ASC') {}\n}\n/** @return SelectQuery */\nfunction db_select($table, $alias = NULL) {}\n";
    let mut project = Project::new(&[
        ("modules/system/system.module", core),
        ("sites/all/modules/shop/shop.info", "name = Shop\ncore = 7.x\n"),
        (INSTALL, install),
    ]);
    let mut goes = |from: &str, to: &str| {
        assert!(install.contains(from), "{from}");
        project.definition(INSTALL, &install.replacen(from, to, 1)).unwrap_or_default()
    };
    // Through a variable and after an alias; an item of a list; straight on the call.
    assert_eq!(goes("condition('b.bid'", "condition('b.b/*|*/id'"), [target(INSTALL, 4, "bid")]);
    assert_eq!(goes("array('title'));", "array('ti/*|*/tle'));"), [target(INSTALL, 5, "title")]);
    assert_eq!(goes("condition('sid'", "condition('s/*|*/id'"), [target(INSTALL, 9, "sid")]);
    let (text, pos) = caret(&install.replacen("orderBy('')", "orderBy('/*|*/')", 1));
    let doc = project.open(INSTALL, &text);
    let found = project.engine.completion(doc, pos, None, &CancellationToken::new()).expect("handled");
    project.engine.document_closed(doc);
    let labels: Vec<&str> = found.items.iter().map(|item| item.label.as_str()).collect();
    assert_eq!(labels, ["bid", "title"], "the columns of the query's table");
}

#[test]
fn a_render_array_a_function_returns_types_the_variables_added_to_it() {
    let page = "<?php\nclass ShopThing { public $title; }\nfunction shop_build() {\n  return ['#theme' => 'shop_book'];\n}\nfunction shop_show() {\n  $build = shop_build();\n  $build['#book'] = new ShopThing();\n  $other = shop_elsewhere();\n  $other['#book'] = 5;\n  return $build;\n}\nfunction shop_elsewhere() {\n  return [];\n}\n";
    let project = Project::new(&[
        ("core/lib/Drupal.php", CORE),
        ("modules/shop/shop.info.yml", INFO),
        ("modules/shop/shop.pages.inc", page),
        (MODULE, SHOP),
    ]);
    let found = project.engine.twig_template_variables("@shop/shop-book.html.twig");
    let book = found.iter().find(|(name, _)| name == "book").map(|(_, ty)| ty.to_string());
    assert_eq!(book.as_deref(), Some("ShopThing"));
}

#[test]
fn a_column_keeps_its_table_along_a_chain_and_through_a_join() {
    const INSTALL: &str = "sites/all/modules/shop/shop.install";
    let install = "<?php\nfunction shop_schema() {\n  $schema['shop_book'] = array(\n    'fields' => array(\n      'bid' => array('type' => 'serial'),\n      'title' => array('type' => 'varchar'),\n    ),\n  );\n  $schema['shop_shelf'] = array('fields' => array('sid' => array('type' => 'serial')));\n  return $schema;\n}\nfunction shop_chain() {\n  return db_select('shop_book', 'b')->fields('b', array('title'))->orderBy('title')->condition('bid', 1);\n}\nfunction shop_joined() {\n  $query = db_select('shop_book', 'b');\n  $query->join('shop_shelf', 's', 's.sid = b.bid');\n  $query->condition('s.sid', 2);\n  $query->condition('b.bid', 3);\n  return $query;\n}\n";
    // As Drupal 7 documents them: each method returns the query's class, not `$this`.
    let core = "<?php\nclass SelectQuery {\n  /** @return SelectQuery */\n  public function condition($field, $value = NULL) {}\n  /** @return SelectQuery */\n  public function fields($alias, array $fields = array()) {}\n  /** @return SelectQuery */\n  public function orderBy($field, $direction = 'ASC') {}\n  public function join($table, $alias = NULL, $condition = NULL) {}\n}\n/** @return SelectQuery */\nfunction db_select($table, $alias = NULL) {}\n";
    let mut project = Project::new(&[
        ("modules/system/system.module", core),
        ("sites/all/modules/shop/shop.info", "name = Shop\ncore = 7.x\n"),
        (INSTALL, install),
    ]);
    let mut goes = |from: &str, to: &str| {
        assert!(install.contains(from), "{from}");
        project.definition(INSTALL, &install.replacen(from, to, 1)).unwrap_or_default()
    };
    // After two calls of the chain.
    assert_eq!(goes("->condition('bid', 1)", "->condition('b/*|*/id', 1)"), [target(INSTALL, 4, "bid")]);
    assert_eq!(goes("->orderBy('title')", "->orderBy('ti/*|*/tle')"), [target(INSTALL, 5, "title")]);
    // The alias of the joined table, and of the query's own.
    assert_eq!(goes("condition('s.sid', 2)", "condition('s.s/*|*/id', 2)"), [target(INSTALL, 8, "sid")]);
    assert_eq!(goes("condition('b.bid', 3)", "condition('b.b/*|*/id', 3)"), [target(INSTALL, 4, "bid")]);
}

#[test]
fn a_render_array_built_in_another_file_types_the_variables_added_to_it() {
    let builder = "<?php\nfunction shop_build() {\n  return ['#theme' => 'shop_book'];\n}\n";
    let page = "<?php\nclass ShopThing { public $title; }\nfunction shop_show() {\n  $build = shop_build();\n  $build['#book'] = new ShopThing();\n  return $build;\n}\n";
    let project = Project::new(&[
        ("core/lib/Drupal.php", CORE),
        ("modules/shop/shop.info.yml", INFO),
        ("modules/shop/shop.build.inc", builder),
        ("modules/shop/shop.pages.inc", page),
        (MODULE, SHOP),
    ]);
    let found = project.engine.twig_template_variables("@shop/shop-book.html.twig");
    let book = found.iter().find(|(name, _)| name == "book").map(|(_, ty)| ty.to_string());
    assert_eq!(book.as_deref(), Some("ShopThing"));
}
