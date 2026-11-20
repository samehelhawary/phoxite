//! CodeIgniter 3, on a stand-in for its API written for these tests.

use super::*;

const CORE: &str = r#"<?php
class CI_Controller { public function __construct() {} }
class CI_Model {}
class CI_Loader {
    public function view($view, $vars = array(), $return = FALSE) {}
    public function model($model, $name = '', $db_conn = FALSE) {}
    public function library($library, $params = NULL, $object_name = NULL) {}
    public function helper($helpers = array()) {}
}
class CI_Lang { public function line($line, $log_errors = TRUE) {} }
class CI_Config { public function item($item, $index = '') {} }
class CI_Session { public function userdata($key = NULL) {} }
class CI_DB_query_builder { public function get($table = '') {} }
function site_url($uri = '', $protocol = NULL) {}
function redirect($uri = '', $method = 'auto', $code = NULL) {}
"#;

const LEGACY_BOOKS: &str = r#"<?php
class Books extends CI_Controller
{
    public function index()
    {
        $this->load->helper('url');
        $this->load->library('session');
        $this->load->model('book_model');
        $title = $this->lang->line('shop_title');
        $shown = $this->config->item('per_page');
        $this->load->view('books/list', array('title' => $title, 'shown' => $shown, 'books' => $this->book_model->newest()));
    }

    public function show()
    {
        redirect('books/index');
    }
}
"#;

const LEGACY_CONTROLLER: &str = "application/controllers/Books.php";
const LEGACY_VIEW: &str = "application/views/books/list.php";
const LIST: &str = "<h1><?= $title ?></h1>\n<a href=\"<?= site_url('books/show/1') ?>\">one</a>\n";

fn legacy() -> Project {
    Project::new(&[
        ("index.php", "<?php\n\t$system_path = 'system';\n\t$application_folder = 'application';\n"),
        ("system/core/CodeIgniter.php", CORE),
        ("system/helpers/url_helper.php", "<?php\nfunction base_url() {}\n"),
        (
            "system/language/english/form_validation_lang.php",
            "<?php\n$lang['form_validation_required'] = 'Required.';\n",
        ),
        ("application/config/config.php", "<?php\n$config['base_url'] = '';\n$config['per_page'] = 20;\n"),
        ("application/language/english/shop_lang.php", "<?php\n$lang['shop_title'] = 'Shop';\n"),
        (
            "application/models/Book_model.php",
            "<?php\nclass Book_model extends CI_Model\n{\n    public function newest() { return array(); }\n}\n",
        ),
        ("application/libraries/Basket.php", "<?php\nclass Basket {}\n"),
        (LEGACY_VIEW, LIST),
        (LEGACY_CONTROLLER, LEGACY_BOOKS),
    ])
}

#[test]
fn the_loaders_names_go_to_what_they_load() {
    let mut project = legacy();
    assert!(project.engine.codeigniter().legacy);
    let mut goes = |from: &str, to: &str| {
        assert!(LEGACY_BOOKS.contains(from), "{from}");
        project.ci_goes(LEGACY_CONTROLLER, &LEGACY_BOOKS.replacen(from, to, 1))
    };
    assert_eq!(goes("view('books/list'", "view('books/li/*|*/st'"), [format!("{LEGACY_VIEW}:0")]);
    assert_eq!(goes("helper('url')", "helper('u/*|*/rl')"), ["system/helpers/url_helper.php:0"]);
    assert_eq!(goes("library('session')", "library('sess/*|*/ion')"), ["system/core/CodeIgniter.php:11"]);
    assert_eq!(goes("model('book_model')", "model('book_/*|*/model')"), ["application/models/Book_model.php:1"]);
    assert_eq!(goes("line('shop_title')", "line('shop_/*|*/title')"), ["application/language/english/shop_lang.php:1"]);
    assert_eq!(goes("item('per_page')", "item('per_/*|*/page')"), ["application/config/config.php:2"]);
    // A path is a controller's method.
    assert_eq!(goes("redirect('books/index')", "redirect('books/in/*|*/dex')"), [format!("{LEGACY_CONTROLLER}:3")]);

    let found = project.ci_complete(LEGACY_CONTROLLER, &LEGACY_BOOKS.replace("model('book_model')", "model('/*|*/')"));
    assert_eq!(found, ["book_model"]);
    let found = project.ci_complete(LEGACY_CONTROLLER, &LEGACY_BOOKS.replace("library('session')", "library('/*|*/')"));
    assert_eq!(found, ["basket"]);
    let found = project.ci_complete(LEGACY_CONTROLLER, &LEGACY_BOOKS.replace("item('per_page')", "item('/*|*/')"));
    assert_eq!(found, ["base_url", "per_page"]);
    let found = project.ci_complete(LEGACY_CONTROLLER, &LEGACY_BOOKS.replace("line('shop_title')", "line('/*|*/')"));
    assert_eq!(found, ["form_validation_required", "shop_title"]);
}

#[test]
fn a_controller_has_what_the_loader_made() {
    let mut project = legacy();
    let mut members = |from: &str, to: &str| {
        assert!(LEGACY_BOOKS.contains(from), "{from}");
        project.ci_complete(LEGACY_CONTROLLER, &LEGACY_BOOKS.replacen(from, to, 1))
    };
    assert_eq!(members("redirect('books/index');", "$this->book_model->/*|*/;"), ["newest"]);
    assert_eq!(members("redirect('books/index');", "$this->session->/*|*/;"), ["userdata"]);
    assert_eq!(members("redirect('books/index');", "$this->db->/*|*/;"), ["get"]);
    assert_eq!(members("redirect('books/index');", "$this->load->/*|*/;"), ["helper", "library", "model", "view"]);
    // Nothing of the controller is reported.
    let found = project.ci_problems(LEGACY_CONTROLLER, LEGACY_BOOKS, "");
    assert_eq!(found, Vec::<String>::new());
}

#[test]
fn a_view_has_what_it_is_passed_and_the_controller() {
    let mut project = legacy();
    let found = project.engine.codeigniter_view_variables(&project.root.join(LEGACY_VIEW));
    assert_eq!(found.iter().map(|(name, _)| name.as_str()).collect::<Vec<_>>(), ["books", "shown", "title"]);
    assert_eq!(project.ci_problems(LEGACY_VIEW, LIST, ""), Vec::<String>::new());
    let found = project.ci_goes(LEGACY_VIEW, &LIST.replace("'books/show/1'", "'books/sh/*|*/ow/1'"));
    assert_eq!(found, [format!("{LEGACY_CONTROLLER}:13")]);
    let found = project.ci_complete(LEGACY_VIEW, "<?php $this->load->/*|*/ ?>\n");
    assert_eq!(found, ["helper", "library", "model", "view"]);
    // A view that isn't there.
    let source = LEGACY_BOOKS.replace("view('books/list'", "view('books/nope'");
    let found = project.ci_problems(LEGACY_CONTROLLER, &source, "CodeIgniter");
    assert_eq!(found, ["Unknown view 'books/nope'"]);
}

#[test]
fn a_library_an_alias_and_a_route_of_the_application() {
    let body = "$this->load->model('book_model', 'books');\n        $this->load->library('basket');\n        redirect('products/7');";
    let source = LEGACY_BOOKS.replace("redirect('books/index');", body);
    assert_ne!(source, LEGACY_BOOKS);
    let mut project = Project::new(&[
        ("index.php", "<?php\n\t$system_path = 'system';\n"),
        ("system/core/CodeIgniter.php", CORE),
        (
            "application/config/routes.php",
            "<?php\n$route['default_controller'] = 'books';\n$route['products/(:num)'] = 'books/show/$1';\n",
        ),
        (
            "application/models/Book_model.php",
            "<?php\nclass Book_model extends CI_Model\n{\n    public function newest() { return array(); }\n}\n",
        ),
        ("application/libraries/Basket.php", "<?php\nclass Basket\n{\n    public function total() { return 0; }\n}\n"),
        (LEGACY_VIEW, LIST),
        (LEGACY_CONTROLLER, &source),
    ]);
    let mut members = |code: &str| {
        let text = source.replace("redirect('products/7');", &format!("redirect('products/7');\n        {code}"));
        project.ci_complete(LEGACY_CONTROLLER, &text)
    };
    // A model under the name it was loaded with; a library of the application.
    assert_eq!(members("$this->books->/*|*/;"), ["newest"]);
    assert_eq!(members("$this->basket->/*|*/;"), ["total"]);
    // A path goes through the routes; what a route is sent to is a controller's method.
    let found = project.ci_goes(LEGACY_CONTROLLER, &source.replace("'products/7'", "'products/7/*|*/'"));
    assert_eq!(found, [format!("{LEGACY_CONTROLLER}:13")]);
    let routes = "<?php\n$route['products/(:num)'] = 'books/sh/*|*/ow/$1';\n";
    assert_eq!(project.ci_goes("application/config/routes.php", routes), [format!("{LEGACY_CONTROLLER}:13")]);
}

#[test]
fn the_database_and_a_library_under_another_name() {
    let body = "$this->load->library('basket', NULL, 'cart');\n        $this->db->where('title', 'Dune');\n        $this->db->get('books');";
    let source = LEGACY_BOOKS.replace("redirect('books/index');", body);
    let migration = "<?php\nclass Migration_Books extends CI_Migration\n{\n    public function up()\n    {\n        $this->dbforge->add_field(array(\n            'id' => array('type' => 'INT'),\n            'title' => array('type' => 'VARCHAR'),\n        ));\n        $this->dbforge->create_table('books');\n    }\n}\n";
    let mut project = Project::new(&[
        ("index.php", "<?php\n\t$system_path = 'system';\n"),
        ("system/core/CodeIgniter.php", CORE),
        ("application/migrations/001_books.php", migration),
        ("application/libraries/Basket.php", "<?php\nclass Basket\n{\n    public function total() { return 0; }\n}\n"),
        (LEGACY_VIEW, LIST),
        (LEGACY_CONTROLLER, &source),
    ]);
    let text = source.replace("$this->db->get('books');", "$this->db->get('books');\n        $this->cart->/*|*/;");
    assert_eq!(project.ci_complete(LEGACY_CONTROLLER, &text), ["total"]);
    // A table of the migrations; a column of the table the function asks.
    let found = project.ci_goes(LEGACY_CONTROLLER, &source.replace("get('books')", "get('bo/*|*/oks')"));
    assert_eq!(found, ["application/migrations/001_books.php:9"]);
    let found = project.ci_goes(LEGACY_CONTROLLER, &source.replace("where('title'", "where('ti/*|*/tle'"));
    assert_eq!(found, ["application/migrations/001_books.php:7"]);
    let found = project.ci_complete(LEGACY_CONTROLLER, &source.replace("where('title'", "where('/*|*/'"));
    assert_eq!(found, ["id", "title"]);
}
