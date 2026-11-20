//! CodeIgniter 4: events, view cells, validation rules, tables and columns,
//! modules, routes named by their path, and the names that name nothing.

use super::*;

/// More of the framework's API than the first tests need.
const MORE: &str = r#"<?php
namespace CodeIgniter\Events {
    class Events {
        public static function on($eventName, $callback, $priority = 100) {}
        public static function trigger($eventName, ...$arguments): bool {}
    }
}
namespace CodeIgniter\Validation {
    interface ValidationInterface {}
    class Validation implements ValidationInterface {
        public function setRules(array $rules, array $errors = []) {}
        public function setRule(string $field, ?string $label, $rules, array $errors = []) {}
        public function run(?array $data = null, ?string $group = null): bool {}
    }
    class Rules {
        public function required($str = null): bool {}
        public function min_length(?string $str, string $val): bool {}
        public function in_list($value, string $list): bool {}
    }
    class FormatRules {
        public function valid_email(?string $str = null): bool {}
    }
}
namespace CodeIgniter\Database {
    class BaseBuilder {
        /** @return $this */
        public function where($key, $value = null) {}
        /** @return $this */
        public function orderBy(string $orderBy, string $direction = '') {}
        /** @return $this */
        public function join(string $table, $cond, string $type = '') {}
    }
    class BaseConnection {
        /** @return BaseBuilder */
        public function table($tableName) {}
    }
    class Forge {
        public function addField($fields) {}
        public function createTable(string $table, bool $ifNotExists = false) {}
        public function dropTable(string $tableName) {}
        public function addColumn(string $table, $fields) {}
    }
    class Migration {
        /** @var Forge */
        protected $forge;
    }
}
namespace CodeIgniter\Router {
    class RouteCollection {
        public function get(string $from, $to, ?array $options = null) {}
        public function group(string $name, ...$params) {}
    }
}
namespace {
    function view_cell(string $library, $params = null): string {}
}
"#;

const MIGRATION: &str = "app/Database/Migrations/2024-01-01-000000_CreateBooks.php";

const CREATE_BOOKS: &str = r#"<?php
namespace App\Database\Migrations;

class CreateBooks extends \CodeIgniter\Database\Migration
{
    public function up()
    {
        $this->forge->addField([
            'id' => ['type' => 'INT'],
            'title' => ['type' => 'VARCHAR'],
        ]);
        $this->forge->createTable('books');
        $this->forge->addColumn('books', ['price' => ['type' => 'INT']]);
    }

    public function down()
    {
        $this->forge->dropTable('books');
    }
}
"#;

const SHELF: &str = r#"<?php
namespace App\Models;

class ShelfModel extends \CodeIgniter\Model
{
    protected $table = 'books';
    protected $primaryKey = 'id';
    protected $allowedFields = ['title', 'price'];
    protected $validationRules = ['title' => 'required|min_length[3]'];

    public function cheap()
    {
        return $this->where('price <', 10)->orderBy('title');
    }
}
"#;

const VALIDATION: &str = r#"<?php
namespace Config;

use CodeIgniter\Validation\FormatRules;
use CodeIgniter\Validation\Rules;
use App\Validation\ShopRules;

class Validation
{
    public array $ruleSets = [Rules::class, FormatRules::class, ShopRules::class];
    public array $templates = ['list' => 'CodeIgniter\Validation\Views\list'];
    public array $signup = [
        'email' => 'required|valid_email',
        'isbn' => ['label' => 'ISBN', 'rules' => 'required|isbn'],
    ];
}
"#;

const SHOP_ROUTES: &str = r#"<?php
/** @var \CodeIgniter\Router\RouteCollection $routes */
$routes->get('books', 'Books::index', ['as' => 'books.list']);
$routes->get('about', 'Books::show');
$routes->group('admin', ['namespace' => 'App\Controllers\Admin'], static function ($routes) {
    $routes->get('books/(:num)', 'Stock::show/$1');
    $routes->get('gone', 'Stock::missing');
});
$routes->get('lost', 'Nowhere::index');
$routes->resource('photos');
"#;

const SHOP: &str = "app/Controllers/Shop.php";

fn shop(body: &str) -> String {
    format!(
        "<?php\nnamespace App\\Controllers;\n\nuse CodeIgniter\\Events\\Events;\n\nclass Shop extends \\CodeIgniter\\Controller\n{{\n    public function run(\\CodeIgniter\\Database\\BaseConnection $db, \\CodeIgniter\\Validation\\Validation $validation)\n    {{\n        {body}\n    }}\n}}\n"
    )
}

fn more() -> Project {
    more_with(&shop(""))
}

fn more_with(shop: &str) -> Project {
    Project::new(&[
        ("spark", "#!/usr/bin/env php\n"),
        ("system/Common.php", SYSTEM),
        ("system/More.php", MORE),
        ("system/Helpers/url_helper.php", "<?php\nfunction site_url() {}\n"),
        ("app/Config/Services.php", SERVICES),
        ("app/Config/Filters.php", FILTERS),
        ("app/Config/Routes.php", SHOP_ROUTES),
        ("app/Config/Validation.php", VALIDATION),
        (
            "app/Config/Autoload.php",
            "<?php\nnamespace Config;\nclass Autoload\n{\n    public $psr4 = [\n        'App' => APPPATH,\n        'Blog' => ROOTPATH . 'modules/Blog',\n    ];\n}\n",
        ),
        (
            "app/Config/Events.php",
            "<?php\nuse CodeIgniter\\Events\\Events;\nEvents::on('book_sold', static function () {});\n",
        ),
        (
            "app/Validation/ShopRules.php",
            "<?php\nnamespace App\\Validation;\nclass ShopRules\n{\n    public function isbn(?string $value): bool { return true; }\n}\n",
        ),
        (
            "app/Cells/Card.php",
            "<?php\nnamespace App\\Cells;\nclass Card\n{\n    public function render(): string { return ''; }\n    public function small(): string { return ''; }\n}\n",
        ),
        (
            "app/Controllers/Admin/Stock.php",
            "<?php\nnamespace App\\Controllers\\Admin;\nclass Stock extends \\CodeIgniter\\Controller\n{\n    public function show() {}\n}\n",
        ),
        ("app/Language/en/Shop.php", "<?php\nreturn ['title' => 'Shop'];\n"),
        ("app/Helpers/shop_helper.php", "<?php\nfunction shop_price() {}\n"),
        ("app/Views/layout.php", "<html></html>\n"),
        ("modules/Blog/Views/list.php", "<ul></ul>\n"),
        ("modules/Blog/Helpers/blog_helper.php", "<?php\nfunction blog_title() {}\n"),
        ("modules/Blog/Language/en/Blog.php", "<?php\nreturn ['title' => 'Blog'];\n"),
        ("modules/Blog/Config/Routes.php", "<?php\n$routes->get('blog', '\\Blog\\Controllers\\Posts::index');\n"),
        (MIGRATION, CREATE_BOOKS),
        ("app/Models/ShelfModel.php", SHELF),
        (CONTROLLER, BOOKS),
        (SHOP, shop),
    ])
}

impl Project {
    /// Where the name at the caret of `source` goes, as `file:line`.
    pub(super) fn ci_goes(&mut self, file: &str, source: &str) -> Vec<String> {
        let found = self.definition(file, source).unwrap_or_default();
        found.into_iter().map(|(file, line, _)| format!("{file}:{line}")).collect()
    }

    /// What the inspections `wanted` say of `source` as `file`.
    pub(super) fn ci_problems(&mut self, file: &str, source: &str, wanted: &str) -> Vec<String> {
        let doc = self.open(file, source);
        let inspected = self.engine.inspect(doc, &CancellationToken::new()).expect("inspected");
        self.engine.document_closed(doc);
        let found = inspected.problems.iter().filter(|problem| problem.inspection.contains(wanted));
        found.map(|problem| problem.message.clone()).collect()
    }
}

#[test]
fn an_event_and_a_view_cell_are_names() {
    let fired = shop(
        "Events::trigger('book_sold', 1);\n        echo view_cell('Card::small');\n        echo view_cell('Card');",
    );
    let mut project = more_with(&fired);
    // A listener goes to where the event is fired.
    let events = "<?php\nuse CodeIgniter\\Events\\Events;\nEvents::on('book_/*|*/sold', static function () {});\n";
    assert_eq!(project.ci_goes("app/Config/Events.php", events), [format!("{SHOP}:9")]);
    let found = project.ci_complete("app/Config/Events.php", &events.replace("book_/*|*/sold", "/*|*/"));
    assert_eq!(found, ["book_sold"]);
    let usages = project.ci_usages("app/Config/Events.php", events);
    assert_eq!(usages, [target("app/Config/Events.php", 2, "book_sold"), target(SHOP, 9, "book_sold")]);
    let renamed = project.ci_rename("app/Config/Events.php", events, "book_bought").unwrap();
    assert_eq!(renamed, [target("app/Config/Events.php", 2, "book_sold"), target(SHOP, 9, "book_sold")]);

    assert_eq!(project.ci_goes(SHOP, &fired.replace("'Card::small'", "'Card::sm/*|*/all'")), ["app/Cells/Card.php:5"]);
    // A cell alone is its `render`.
    assert_eq!(
        project.ci_goes(SHOP, &fired.replace("view_cell('Card')", "view_cell('Ca/*|*/rd')")),
        ["app/Cells/Card.php:4"]
    );
}

#[test]
fn a_rule_is_a_method_of_a_rule_set() {
    let body = "$validation->setRules(['title' => 'required|min_length[3]', 'tag' => ['rules' => ['in_list[a,b]', 'isbn']]]);\n        $validation->run([], 'signup');\n        $this->validate('signup');";
    let source = shop(body);
    let mut project = more_with(&source);
    let mut goes = |from: &str, to: &str| {
        assert!(source.contains(from), "{from}");
        project.ci_goes(SHOP, &source.replacen(from, to, 1))
    };
    // Each rule of a string, by the one the caret is in; a rule of the application's own set.
    assert_eq!(goes("'required|min_length[3]'", "'requ/*|*/ired|min_length[3]'"), ["system/More.php:15"]);
    assert_eq!(goes("'required|min_length[3]'", "'required|min_le/*|*/ngth[3]'"), ["system/More.php:16"]);
    assert_eq!(goes("'in_list[a,b]'", "'in_l/*|*/ist[a,b]'"), ["system/More.php:17"]);
    assert_eq!(goes("'isbn'", "'is/*|*/bn'"), ["app/Validation/ShopRules.php:4"]);
    // Not the field.
    assert!(goes("['title' =>", "['ti/*|*/tle' =>").is_empty());
    // A group is a property of the validation configuration.
    assert_eq!(goes("run([], 'signup')", "run([], 'sign/*|*/up')"), ["app/Config/Validation.php:11"]);
    assert_eq!(goes("validate('signup')", "validate('sign/*|*/up')"), ["app/Config/Validation.php:11"]);

    let found = project.ci_complete(SHOP, &source.replace("'required|min_length[3]'", "'required|/*|*/'"));
    assert_eq!(found, ["if_exist", "in_list", "isbn", "min_length", "permit_empty", "required", "valid_email"]);
    let found = project.ci_complete(SHOP, &source.replace("run([], 'signup')", "run([], '/*|*/')"));
    assert_eq!(found, ["signup"]);

    // In a model and in the configuration's groups; not in its templates.
    let shelf = SHELF.replace("'required|min_length[3]'", "'required|min_le/*|*/ngth[3]'");
    assert_eq!(project.ci_goes("app/Models/ShelfModel.php", &shelf), ["system/More.php:16"]);
    let config = VALIDATION.replace("'required|isbn'", "'required|is/*|*/bn'");
    assert_eq!(project.ci_goes("app/Config/Validation.php", &config), ["app/Validation/ShopRules.php:4"]);
    let config = VALIDATION.replace("Views\\list'", "Views\\li/*|*/st'");
    assert!(project.ci_goes("app/Config/Validation.php", &config).is_empty());
}

#[test]
fn a_table_and_its_columns_come_from_the_migrations() {
    let body = "$found = $db->table('books')->where('title', 'x')->orderBy('price');\n        $model = model('ShelfModel');\n        $model->where(['id' => 1]);";
    let source = shop(body);
    let mut project = more_with(&source);
    let mut goes = |from: &str, to: &str| {
        assert!(source.contains(from), "{from}");
        project.ci_goes(SHOP, &source.replacen(from, to, 1))
    };
    assert_eq!(goes("table('books')", "table('bo/*|*/oks')"), [format!("{MIGRATION}:11")]);
    // A builder's columns are its table's, along the chain.
    assert_eq!(goes("where('title'", "where('ti/*|*/tle'"), [format!("{MIGRATION}:9")]);
    assert_eq!(goes("orderBy('price')", "orderBy('pr/*|*/ice')"), [format!("{MIGRATION}:12")]);
    // A model's are those of its `$table`.
    assert_eq!(goes("['id' => 1]", "['i/*|*/d' => 1]"), [format!("{MIGRATION}:8")]);

    let found = project.ci_complete(SHOP, &source.replace("where('title'", "where('/*|*/'"));
    assert_eq!(found, ["id", "price", "title"]);
    let found = project.ci_complete(SHOP, &source.replace("table('books')", "table('/*|*/')"));
    assert_eq!(found, ["books"]);

    // In the model itself.
    let mut shelf = |from: &str, to: &str| {
        assert!(SHELF.contains(from), "{from}");
        project.ci_goes("app/Models/ShelfModel.php", &SHELF.replacen(from, to, 1))
    };
    assert_eq!(shelf("$table = 'books'", "$table = 'bo/*|*/oks'"), [format!("{MIGRATION}:11")]);
    assert_eq!(shelf("['title', 'price']", "['title', 'pr/*|*/ice']"), [format!("{MIGRATION}:12")]);
    assert_eq!(shelf("$primaryKey = 'id'", "$primaryKey = 'i/*|*/d'"), [format!("{MIGRATION}:8")]);
    assert_eq!(shelf("where('price <'", "where('pr/*|*/ice <'"), [format!("{MIGRATION}:12")]);
    assert_eq!(shelf("['title' => 'required", "['ti/*|*/tle' => 'required"), [format!("{MIGRATION}:9")]);

    // Find References of a column: wherever it is named, of that table.
    let usages =
        project.ci_usages("app/Models/ShelfModel.php", &SHELF.replace("['title', 'price']", "['title', 'pr/*|*/ice']"));
    assert_eq!(
        usages,
        [
            target(SHOP, 9, "price"),
            target(MIGRATION, 12, "price"),
            target("app/Models/ShelfModel.php", 7, "price"),
            target("app/Models/ShelfModel.php", 12, "price <"),
        ]
    );
}

#[test]
fn a_route_is_named_by_its_path_or_its_handler() {
    let source = shop(
        "route_to('about');\n        route_to('admin/books/(:num)', 1);\n        route_to('Books::show');\n        route_to('blog');",
    );
    let mut project = more_with(&source);
    let mut goes = |from: &str, to: &str| {
        assert!(source.contains(from), "{from}");
        project.ci_goes(SHOP, &source.replacen(from, to, 1))
    };
    assert_eq!(goes("'about'", "'ab/*|*/out'"), ["app/Config/Routes.php:3"]);
    // With the prefixes of the groups around it; a module's routes.
    assert_eq!(goes("'admin/books/(:num)'", "'admin/bo/*|*/oks/(:num)'"), ["app/Config/Routes.php:5"]);
    assert_eq!(goes("'blog'", "'bl/*|*/og'"), ["modules/Blog/Config/Routes.php:1"]);
    assert_eq!(goes("'Books::show'", "'Books::sh/*|*/ow'"), [format!("{CONTROLLER}:17")]);
    // One named by its path isn't renamed.
    let renamed = project.ci_rename(SHOP, &source.replacen("'about'", "'ab/*|*/out'", 1), "info");
    assert!(renamed.is_err(), "{renamed:?}");

    // A handler is in the namespace of its group.
    let routes = SHOP_ROUTES.replace("'Stock::show/$1'", "'Stock::sh/*|*/ow/$1'");
    assert_eq!(project.ci_goes("app/Config/Routes.php", &routes), ["app/Controllers/Admin/Stock.php:4"]);
    let found = project.ci_problems("app/Config/Routes.php", SHOP_ROUTES, "PhpUndefinedCodeIgniterController");
    assert_eq!(
        found,
        [
            "Controller 'App\\Controllers\\Admin\\Stock' has no method 'missing'",
            "Controller 'App\\Controllers\\Nowhere' not found"
        ]
    );
}

#[test]
fn a_module_has_views_helpers_and_lines_of_its_own() {
    let source = shop(
        "helper('blog');\n        echo lang('Blog.title');\n        return view('Blog\\Views\\list', ['posts' => $db]);",
    );
    let mut project = more_with(&source);
    let mut goes = |from: &str, to: &str| {
        assert!(source.contains(from), "{from}");
        project.ci_goes(SHOP, &source.replacen(from, to, 1))
    };
    assert_eq!(goes("'Blog\\Views\\list'", "'Blog\\Views\\li/*|*/st'"), ["modules/Blog/Views/list.php:0"]);
    assert_eq!(goes("helper('blog')", "helper('bl/*|*/og')"), ["modules/Blog/Helpers/blog_helper.php:0"]);
    assert_eq!(goes("'Blog.title'", "'Blog.ti/*|*/tle'"), ["modules/Blog/Language/en/Blog.php:1"]);
    let found = project.ci_complete(SHOP, &source.replace("'Blog\\Views\\list'", "'/*|*/'"));
    assert_eq!(found, ["Blog\\Views\\list", "layout"]);
    // The module's view has what it is passed.
    let found = project.engine.codeigniter_view_variables(&project.root.join("modules/Blog/Views/list.php"));
    assert_eq!(found.iter().map(|(name, _)| name.as_str()).collect::<Vec<_>>(), ["posts"]);
}

#[test]
fn the_paths_configuration_says_where_the_views_are() {
    let paths = "<?php\nnamespace Config;\nclass Paths\n{\n    public string $systemDirectory = __DIR__ . '/../../framework';\n    public string $viewDirectory = __DIR__ . '/../../templates';\n}\n";
    let source = shop("helper('url');\n        return view('home');");
    let mut project = Project::new(&[
        ("spark", "#!/usr/bin/env php\n"),
        ("framework/Common.php", SYSTEM),
        ("framework/Helpers/url_helper.php", "<?php\nfunction site_url() {}\n"),
        ("app/Config/Paths.php", paths),
        ("templates/home.php", "<h1></h1>\n"),
        (SHOP, &source),
    ]);
    assert_eq!(project.ci_goes(SHOP, &source.replace("view('home')", "view('ho/*|*/me')")), ["templates/home.php:0"]);
    assert_eq!(
        project.ci_goes(SHOP, &source.replace("helper('url')", "helper('u/*|*/rl')")),
        ["framework/Helpers/url_helper.php:0"]
    );
}

#[test]
fn a_name_that_names_nothing_is_reported() {
    let mut project = more();
    let body = "helper(['shop', 'nope']);\n        echo lang('Shop.title'), lang('Shop.nope'), lang('Other.line');\n        echo view('layout'), view('nope'), view('Blog\\Views\\list'), view('Vendor\\Views\\x');\n        route_to('books.list'); route_to('nope'); route_to('Books::show'); route_to('Books::nope');\n        $validation->setRules(['title' => 'required|nope[3]|isbn']);\n        $validation->run([], 'signup'); $validation->run([], 'nope');";
    let found = project.ci_problems(SHOP, &shop(body), "CodeIgniter");
    assert_eq!(
        found,
        [
            "Unknown helper 'nope'",
            "Unknown language line 'Shop.nope'",
            "Unknown view 'nope'",
            "Unknown route 'nope'",
            "Unknown route 'Books::nope'",
            "Unknown validation rule 'nope'",
            "Unknown rule group 'nope'",
        ]
    );
    let routes = "<?php\n/** @var \\CodeIgniter\\Router\\RouteCollection $routes */\n$routes->get('a', 'Books::index', ['filter' => 'auth']);\n$routes->get('b', 'Books::index', ['filter' => 'nope:1']);\n";
    let found = project.ci_problems("app/Config/Routes.php", routes, "PhpUnknownCodeIgniterFilter");
    assert_eq!(found, ["Unknown filter 'nope'"]);
}

#[test]
fn the_rest_of_what_names_nothing_is_reported() {
    let body = "echo view('Blog\\Views\\list'), view('Blog\\Views\\nope');\n        model('ShelfModel'); model('NopeModel'); model('App\\Models\\Gone');\n        config('Validation'); config('Nope');\n        service('catalog'); service('nope');\n        echo view_cell('Card'), view_cell('Card::nope'), view_cell('Nope::render');";
    let mut project = more();
    let found = project.ci_problems(SHOP, &shop(body), "CodeIgniter");
    assert_eq!(
        found,
        [
            "Unknown view 'Blog\\Views\\nope'",
            "Unknown model 'NopeModel'",
            "Unknown model 'App\\Models\\Gone'",
            "Unknown configuration 'Nope'",
            "Unknown service 'nope'",
            "View cell 'Card' has no method 'nope'",
            "Unknown view cell 'Nope'",
        ]
    );
}

#[test]
fn an_event_a_table_and_a_column_are_reported_when_asked() {
    let body = "Events::on('book_sold', 'trim'); Events::on('nothing_fires', 'trim');\n        $db->table('books')->where('title', 1)->where('nope', 1)->where('COUNT(id) >', 1);\n        $db->table('elsewhere')->where('anything', 1);";
    let fired = "<?php\nuse CodeIgniter\\Events\\Events;\nEvents::trigger('book_sold');\n";
    let mut project = more();
    let events = project.open("app/Config/Fired.php", fired);
    let wanted = "CodeIgniter";
    // Off unless turned on.
    assert_eq!(project.ci_problems(SHOP, &shop(body), wanted), Vec::<String>::new());
    let mut config = crate::inspect::InspectionConfig::default();
    for id in ["PhpUndefinedCodeIgniterEvent", "PhpUnknownCodeIgniterTable", "PhpUnknownCodeIgniterColumn"] {
        config.on.insert(id.to_owned());
    }
    project.engine.configure_inspections(config);
    // Not what isn't one column's name, nor a column of a table no migration makes.
    assert_eq!(
        project.ci_problems(SHOP, &shop(body), wanted),
        ["Unknown event 'nothing_fires'", "Unknown column 'nope' of table 'books'", "Unknown table 'elsewhere'"]
    );
    project.engine.document_closed(events);
}

#[test]
fn several_columns_in_a_string_and_generated_routes() {
    let source = shop(
        "$db->table('books')->select('title, price');\n        route_to('photos/new'); route_to('photos/(:segment)/edit');",
    );
    let mut project = more_with(&source);
    let found = project.ci_goes(SHOP, &source.replace("'title, price'", "'title, pr/*|*/ice'"));
    assert_eq!(found, [format!("{MIGRATION}:12")]);
    let found = project.ci_goes(SHOP, &source.replace("'title, price'", "'ti/*|*/tle, price'"));
    assert_eq!(found, [format!("{MIGRATION}:9")]);
    let found = project.ci_complete(SHOP, &source.replace("'title, price'", "'title, /*|*/'"));
    assert_eq!(found, ["id", "price", "title"]);
    // `resource('photos')` makes the routes of a resource.
    let found = project.ci_goes(SHOP, &source.replace("'photos/new'", "'photos/n/*|*/ew'"));
    assert_eq!(found, ["app/Config/Routes.php:9"]);
    let found = project.ci_goes(SHOP, &source.replace("'photos/(:segment)/edit'", "'photos/(:segment)/ed/*|*/it'"));
    assert_eq!(found, ["app/Config/Routes.php:9"]);
}

#[test]
fn an_expression_a_rules_table_and_a_path_are_names() {
    let source = shop(
        "$db->table('books')->where('COUNT(price) >', 1);\n        $validation->setRules(['title' => 'required|is_unique[books.title]']);\n        redirect()->to('books/show'); redirect()->to('admin/stock/show'); redirect()->to('about');",
    );
    let mut project = more_with(&source);
    let mut goes = |from: &str, to: &str| {
        assert!(source.contains(from), "{from}");
        project.ci_goes(SHOP, &source.replacen(from, to, 1))
    };
    // The column an expression names.
    assert_eq!(goes("'COUNT(price) >'", "'COUNT(pr/*|*/ice) >'"), [format!("{MIGRATION}:12")]);
    // The table and the field of `is_unique`.
    assert_eq!(goes("is_unique[books.title]", "is_unique[bo/*|*/oks.title]"), [format!("{MIGRATION}:11")]);
    assert_eq!(goes("is_unique[books.title]", "is_unique[books.ti/*|*/tle]"), [format!("{MIGRATION}:9")]);
    assert_eq!(goes("is_unique[books.title]", "is_un/*|*/ique[books.title]"), Vec::<String>::new());
    // A path: the controller's method auto-routing would call, a route of that path first.
    assert_eq!(goes("to('books/show')", "to('books/sh/*|*/ow')"), [format!("{CONTROLLER}:17")]);
    assert_eq!(goes("to('admin/stock/show')", "to('admin/stock/sh/*|*/ow')"), ["app/Controllers/Admin/Stock.php:4"]);
    assert_eq!(goes("to('about')", "to('ab/*|*/out')"), ["app/Config/Routes.php:3"]);

    let found = project.ci_complete(SHOP, &source.replace("is_unique[books.title]", "is_unique[books./*|*/]"));
    assert_eq!(found, ["id", "price", "title"]);
    let found = project.ci_complete(SHOP, &source.replace("is_unique[books.title]", "is_unique[/*|*/]"));
    assert_eq!(found, ["books"]);
}
