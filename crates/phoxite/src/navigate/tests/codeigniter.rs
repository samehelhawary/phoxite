//! CodeIgniter 4, on a stand-in for its API written for these tests (the
//! names of its classes and functions, nothing of their code) and a small
//! application.

use super::*;

/// The framework's classes and helper functions.
const SYSTEM: &str = r#"<?php
namespace CodeIgniter {
    class CodeIgniter {}
    class Controller { protected $helpers = []; }
    class Model { public function find($id = null) {} }
}
namespace CodeIgniter\Config {
    class BaseService {}
    class Services extends BaseService {
        public static function request(): \CodeIgniter\HTTP\Request {}
    }
}
namespace CodeIgniter\HTTP {
    class Request { public function getPost($index = null) {} }
    class RedirectResponse {
        public function route(string $route, array $params = []) {}
        public function to(string $uri) {}
    }
}
namespace CodeIgniter\View {
    class View {
        public function extend(string $layout) {}
        public function include(string $view) {}
    }
}
namespace {
    function view(string $name, array $data = [], array $options = []): string {}
    function helper($filenames) {}
    function lang(string $line, array $args = []): string {}
    function route_to(string $method, ...$params) {}
    function redirect(?string $route = null): \CodeIgniter\HTTP\RedirectResponse {}
    /** @return object|null */
    function service(string $name, ...$params) {}
    /** @return object|null */
    function model(string $name, bool $getShared = true) {}
    /** @return object|null */
    function config(string $name, bool $getShared = true) {}
}
"#;

const SERVICES: &str = r#"<?php
namespace Config;

class Services extends \CodeIgniter\Config\BaseService
{
    public static function catalog(bool $getShared = true): \App\Libraries\Catalog {}
}
"#;

const FILTERS: &str = r#"<?php
namespace Config;

class Filters
{
    public array $aliases = [
        'auth' => \App\Filters\Auth::class,
        'throttle' => 'App\Filters\Throttle',
    ];
    public array $globals = ['before' => ['throttle'], 'after' => []];
    public array $filters = ['auth' => ['before' => ['admin/*']]];
}
"#;

const ROUTES: &str = r#"<?php
$routes->get('books', 'Books::index', ['as' => 'books.list', 'filter' => 'auth:admin']);
$routes->get('books/(:num)', 'Books::show/$1', ['as' => 'books.show']);
$routes->group('admin', ['filter' => 'throttle'], static function ($routes) {});
"#;

const CLASSES: &str = r#"<?php
namespace Config { class Shop { public string $currency = 'EUR'; } }
namespace App\Models { class BookModel extends \CodeIgniter\Model { public function newest(): array { return []; } } }
namespace App\Libraries { class Catalog { public function count(): int { return 0; } } }
"#;

const CONTROLLER: &str = "app/Controllers/Books.php";

const BOOKS: &str = r#"<?php
namespace App\Controllers;

class Books extends \CodeIgniter\Controller
{
    protected $helpers = ['shop'];

    public function index()
    {
        helper(['shop', 'url']);
        $model = model('BookModel');
        $catalog = service('catalog');
        $shop = config('Shop');
        $title = lang('Shop.cart.empty');
        return view('books/list', ['books' => $model->newest(), 'catalog' => $catalog, 'title' => $title]);
    }

    public function show()
    {
        return redirect()->route('books.list');
    }

    public function link()
    {
        return route_to('books.show', 1);
    }
}
"#;

const VIEW: &str = "app/Views/books/list.php";

fn codeigniter() -> Project {
    Project::new(&[
        ("spark", "#!/usr/bin/env php\n"),
        ("system/Common.php", SYSTEM),
        ("system/Helpers/url_helper.php", "<?php\nfunction site_url() {}\n"),
        ("system/Language/en/Validation.php", "<?php\nreturn ['required' => 'Required.'];\n"),
        ("app/Config/Services.php", SERVICES),
        ("app/Config/Filters.php", FILTERS),
        ("app/Config/Routes.php", ROUTES),
        ("app/Config/Shop.php", CLASSES),
        (
            "app/Language/en/Shop.php",
            "<?php\nreturn [\n    'title' => 'Shop',\n    'cart' => ['empty' => 'Nothing yet'],\n];\n",
        ),
        ("app/Helpers/shop_helper.php", "<?php\nfunction shop_price() {}\n"),
        ("app/Views/layout.php", "<html></html>\n"),
        (VIEW, "<?php $this->extend('layout'); ?>\n<h1><?= $title ?></h1>\n"),
        (CONTROLLER, BOOKS),
    ])
}

fn at(from: &str, to: &str) -> String {
    assert!(BOOKS.contains(from), "{from}");
    BOOKS.replacen(from, to, 1)
}

impl Project {
    fn ci_complete(&mut self, file: &str, source: &str) -> Vec<String> {
        let (text, pos) = caret(source);
        let doc = self.open(file, &text);
        let found = self.engine.completion(doc, pos, None, &CancellationToken::new()).expect("handled");
        self.engine.document_closed(doc);
        found.items.into_iter().map(|item| item.label).collect()
    }

    fn ci_usages(&mut self, file: &str, source: &str) -> Vec<(String, u32, String)> {
        let (text, pos) = caret(source);
        let doc = self.open(file, &text);
        let usages = self.engine.references(doc, pos, true, &CancellationToken::new()).expect("handled").unwrap();
        let found = usages.iter().map(|usage| self.describe(&usage.path, usage.range)).collect();
        self.engine.document_closed(doc);
        found
    }

    fn ci_rename(&mut self, file: &str, source: &str, new_name: &str) -> Result<Vec<(String, u32, String)>, String> {
        let (text, pos) = caret(source);
        let doc = self.open(file, &text);
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

    fn ci_hover(&mut self, file: &str, source: &str) -> String {
        let (text, pos) = caret(source);
        let doc = self.open(file, &text);
        let hover = self.engine.hover(doc, pos);
        self.engine.document_closed(doc);
        hover.map(|hover| hover.markdown).unwrap_or_default()
    }
}

#[test]
fn the_names_of_a_codeigniter_application_go_to_what_they_name() {
    let mut project = codeigniter();
    assert!(project.engine.frameworks().contains(crate::frameworks::Framework::CodeIgniter));
    let mut files = |from: &str, to: &str| {
        let found = project.definition(CONTROLLER, &at(from, to)).unwrap_or_default();
        found.into_iter().map(|(file, line, _)| (file, line)).collect::<Vec<_>>()
    };
    let file = |name: &str, line: u32| vec![(name.to_owned(), line)];
    // Files: a view, a helper of the application and one of the framework.
    assert_eq!(files("view('books/list'", "view('books/li/*|*/st'"), file(VIEW, 0));
    assert_eq!(
        files("helper(['shop', 'url'])", "helper(['sh/*|*/op', 'url'])"),
        file("app/Helpers/shop_helper.php", 0)
    );
    assert_eq!(
        files("helper(['shop', 'url'])", "helper(['shop', 'u/*|*/rl'])"),
        file("system/Helpers/url_helper.php", 0)
    );
    assert_eq!(files("$helpers = ['shop']", "$helpers = ['sh/*|*/op']"), file("app/Helpers/shop_helper.php", 0));
    // A language line, nested; a route by its name.
    assert_eq!(files("lang('Shop.cart.empty')", "lang('Shop.cart.em/*|*/pty')"), file("app/Language/en/Shop.php", 3));
    assert_eq!(files("route('books.list')", "route('books.li/*|*/st')"), file("app/Config/Routes.php", 1));
    assert_eq!(files("route_to('books.show'", "route_to('books.sh/*|*/ow'"), file("app/Config/Routes.php", 2));
    // Classes and a service's method.
    assert_eq!(files("model('BookModel')", "model('Book/*|*/Model')"), file("app/Config/Shop.php", 2));
    assert_eq!(files("config('Shop')", "config('Sh/*|*/op')"), file("app/Config/Shop.php", 1));
    assert_eq!(files("service('catalog')", "service('cata/*|*/log')"), file("app/Config/Services.php", 5));

    let hover = project.ci_hover(CONTROLLER, &at("lang('Shop.cart.empty')", "lang('Shop.cart.em/*|*/pty')"));
    assert!(hover.contains("**language line** `Shop.cart.empty`") && hover.contains("Nothing yet"), "{hover}");
}

#[test]
fn names_complete_by_what_is_asked_for() {
    let mut project = codeigniter();
    let mut offered = |from: &str, to: &str| project.ci_complete(CONTROLLER, &at(from, to));
    assert_eq!(offered("view('books/list'", "view('/*|*/'"), ["books/list", "layout"]);
    assert_eq!(offered("helper(['shop', 'url'])", "helper(['/*|*/', 'url'])"), ["shop", "url"]);
    assert_eq!(
        offered("lang('Shop.cart.empty')", "lang('/*|*/')"),
        ["Shop.cart.empty", "Shop.title", "Validation.required"]
    );
    assert_eq!(offered("route('books.list')", "route('/*|*/')"), ["books.list", "books.show"]);
    assert_eq!(offered("service('catalog')", "service('/*|*/')"), ["catalog", "request"]);
    assert_eq!(offered("model('BookModel')", "model('/*|*/')"), ["BookModel"]);
    let found = offered("config('Shop')", "config('/*|*/')");
    assert!(found.contains(&"Shop".to_owned()) && found.contains(&"Filters".to_owned()), "{found:?}");
}

#[test]
fn a_route_and_a_filter_are_renamed() {
    let mut project = codeigniter();
    let routes = "app/Config/Routes.php";
    let source = at("route('books.list')", "route('books.li/*|*/st')");
    let places = vec![target(routes, 1, "books.list"), target(CONTROLLER, 19, "books.list")];
    assert_eq!(project.ci_usages(CONTROLLER, &source), places);
    assert_eq!(project.ci_rename(CONTROLLER, &source, "books.all"), Ok(places));

    // A filter: the alias before its arguments, in the routes and in the configuration.
    let source = ROUTES.replacen("'filter' => 'auth:admin'", "'filter' => 'au/*|*/th:admin'", 1);
    let found = project.definition(routes, &source);
    assert_eq!(found, Some(vec![target("app/Config/Filters.php", 6, "auth")]));
    let found = project.ci_rename(routes, &source, "member").expect("renamed");
    assert_eq!(
        found,
        [
            target("app/Config/Filters.php", 6, "auth"),
            target("app/Config/Filters.php", 10, "auth"),
            target(routes, 1, "auth")
        ]
    );
    let source = ROUTES.replacen("['filter' => 'throttle']", "['filter' => 'thr/*|*/ottle']", 1);
    let found = project.ci_usages(routes, &source);
    assert_eq!(
        found,
        [
            target("app/Config/Filters.php", 7, "throttle"),
            target("app/Config/Filters.php", 9, "throttle"),
            target(routes, 3, "throttle")
        ]
    );
    // Not a view, whose name is a file's.
    let found = project.ci_rename(CONTROLLER, &at("view('books/list'", "view('books/li/*|*/st'"), "books/all");
    assert!(found.is_err(), "{found:?}");
}

#[test]
fn the_helper_functions_are_typed() {
    let mut project = codeigniter();
    let mut members = |body: &str| {
        let source = format!("<?php\nfunction probe() {{\n  {body}\n}}\n");
        let mut found = project.ci_complete("app/Helpers/probe_helper.php", &source);
        found.sort();
        found
    };
    assert_eq!(members("model('BookModel')->/*|*/"), ["find", "newest"]);
    assert_eq!(members("model(\\App\\Models\\BookModel::class)->/*|*/"), ["find", "newest"]);
    assert_eq!(members("config('Shop')->/*|*/"), ["currency"]);
    // The application's service, and one only the framework has.
    assert_eq!(members("service('catalog')->/*|*/"), ["count"]);
    assert_eq!(members("service('request')->/*|*/"), ["getPost"]);
    assert!(members("service('nothing')->/*|*/").is_empty());
}

#[test]
fn a_view_has_what_its_controller_passes() {
    let mut project = codeigniter();
    let found = project.ci_complete(VIEW, "<?php echo $catalog->/*|*/ ?>\n");
    assert_eq!(found, ["count"]);
    let hover = project.ci_hover(VIEW, "<h1><?= $ti/*|*/tle ?></h1>\n");
    assert!(hover.contains("string"), "{hover}");
    let variables = project.engine.codeigniter_view_variables(&project.root.join(VIEW));
    let names: Vec<&str> = variables.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names, ["books", "catalog", "title"]);
    // A view extends another by its name.
    let found = project.definition(VIEW, "<?php $this->extend('lay/*|*/out'); ?>\n");
    assert_eq!(
        found.unwrap_or_default().into_iter().map(|(file, ..)| file).collect::<Vec<_>>(),
        ["app/Views/layout.php"]
    );
    // A file that is no view has none.
    assert!(project.engine.codeigniter_view_variables(&project.root.join(CONTROLLER)).is_empty());
}

#[test]
fn codeigniter_names_are_only_read_in_a_codeigniter_project() {
    let mut project =
        Project::new(&[("lib/Common.php", SYSTEM), ("app/Views/books/list.php", "<p></p>\n"), (CONTROLLER, BOOKS)]);
    let found = project.definition(CONTROLLER, &at("view('books/list'", "view('books/li/*|*/st'"));
    assert!(found.as_ref().is_none_or(Vec::is_empty), "{found:?}");
}

#[test]
fn a_routes_handler_is_a_controllers_method() {
    let router = "<?php\nnamespace CodeIgniter\\Router;\nclass RouteCollection {\n    public function get(string $from, $to, ?array $options = null) {}\n    public function post(string $from, $to, ?array $options = null) {}\n}\n";
    let routes = "<?php\n/** @var \\CodeIgniter\\Router\\RouteCollection $routes */\n$routes->get('books', 'Books::index');\n$routes->get('books/(:num)', 'Books::show/$1');\n$routes->post('admin', '\\App\\Controllers\\Books::link');\n$routes->get('home', 'Books');\n";
    let mut project = Project::new(&[
        ("spark", "\n"),
        ("system/Common.php", SYSTEM),
        ("system/Router/RouteCollection.php", router),
        ("app/Config/Routes.php", routes),
        (CONTROLLER, BOOKS),
    ]);
    let mut goes = |from: &str, to: &str| {
        assert!(routes.contains(from), "{from}");
        project.definition("app/Config/Routes.php", &routes.replacen(from, to, 1)).unwrap_or_default()
    };
    assert_eq!(goes("'Books::index'", "'Books::in/*|*/dex'"), [target(CONTROLLER, 7, "index")]);
    // Before what the method is passed; a class written out; a controller alone is its `index`.
    assert_eq!(goes("'Books::show/$1'", "'Books::sh/*|*/ow/$1'"), [target(CONTROLLER, 17, "show")]);
    assert_eq!(goes("Books::link'", "Books::li/*|*/nk'"), [target(CONTROLLER, 22, "link")]);
    assert_eq!(goes("'home', 'Books'", "'home', 'Bo/*|*/oks'"), [target(CONTROLLER, 7, "index")]);
    // Not the route's path.
    assert!(goes("get('books', 'Books::index')", "get('bo/*|*/oks', 'Books::index')").is_empty());
}

#[test]
fn a_layout_and_a_partial_share_their_views_data() {
    let mut project = Project::new(&[
        ("spark", "\n"),
        ("system/Common.php", SYSTEM),
        ("app/Config/Shop.php", CLASSES),
        ("app/Config/Services.php", SERVICES),
        ("app/Views/layout.php", "<html><?= $title ?></html>\n"),
        ("app/Views/books/row.php", "<li></li>\n"),
        (VIEW, "<?php $this->extend('layout'); ?>\n<?= $this->include('books/row') ?>\n"),
        (CONTROLLER, BOOKS),
    ]);
    let names = |project: &Project, view: &str| {
        let found = project.engine.codeigniter_view_variables(&project.root.join(view));
        found.iter().map(|(name, _)| name.clone()).collect::<Vec<_>>()
    };
    assert_eq!(names(&project, VIEW), ["books", "catalog", "title"]);
    assert_eq!(names(&project, "app/Views/layout.php"), ["books", "catalog", "title"]);
    assert_eq!(names(&project, "app/Views/books/row.php"), ["books", "catalog", "title"]);
    let found = project.ci_complete("app/Views/books/row.php", "<li><?= $catalog->/*|*/ ?></li>\n");
    assert_eq!(found, ["count"]);
}

#[test]
fn a_models_finders_return_its_return_type() {
    let models = "<?php\nnamespace App\\Entities { class Book { public string $title = ''; } }\nnamespace App\\Models {\n    use App\\Entities\\Book;\n    class EntityModel extends \\CodeIgniter\\Model { protected $returnType = Book::class; }\n    class NamedModel extends \\CodeIgniter\\Model { protected $returnType = '\\App\\Entities\\Book'; }\n    class ArrayModel extends \\CodeIgniter\\Model { protected $returnType = 'array'; }\n}\n";
    let system = SYSTEM.replace("class Model { public function find($id = null) {} }", "class Model { protected $returnType = 'array'; /** @return array|object|null */ public function find($id = null) {} /** @return array */ public function findAll() {} }");
    assert_ne!(system, SYSTEM);
    let mut project = Project::new(&[
        ("spark", "\n"),
        ("system/Common.php", system.as_str()),
        ("app/Models/Models.php", models),
        (CONTROLLER, "<?php\n"),
    ]);
    let mut members = |body: &str| {
        let source = format!("<?php\nfunction probe() {{\n  {body}\n}}\n");
        project.ci_complete("app/Helpers/probe_helper.php", &source)
    };
    assert_eq!(members("$book = model('EntityModel')->find(1); $book->/*|*/"), ["title"]);
    assert_eq!(members("foreach (model('EntityModel')->findAll() as $book) { $book->/*|*/ }"), ["title"]);
    assert_eq!(members("$book = model('NamedModel')->find(1); $book->/*|*/"), ["title"]);
    // A model returning arrays says what CodeIgniter declares.
    assert!(members("$book = model('ArrayModel')->find(1); $book->/*|*/").is_empty());
}

mod legacy;
mod more;
