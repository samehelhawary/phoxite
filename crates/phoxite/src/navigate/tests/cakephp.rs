//! CakePHP, on a stand-in for its API written for these tests (the names of
//! its classes and methods, nothing of their code) and a small application
//! with a plugin. The stand-in declares no generics, as CakePHP 4 doesn't:
//! what a table gives is the provider's to say.

use super::*;

/// The framework's classes.
const FRAMEWORK: &str = r#"<?php
namespace Cake\Datasource {
    interface EntityInterface {}
    interface QueryInterface {}
    interface ResultSetInterface {}
}
namespace Cake\ORM\Locator {
    interface LocatorInterface {
        public function get(string $alias, array $options = []);
    }
    trait LocatorAwareTrait {
        public function getTableLocator(): LocatorInterface {}
        public function fetchTable(?string $alias = null, array $options = []): \Cake\ORM\Table {}
    }
}
namespace Cake\ORM {
    class Entity implements \Cake\Datasource\EntityInterface {
        public function isNew(): bool {}
    }
    class Table {
        public function find(string $type = 'all'): Query {}
        public function get($primaryKey): \Cake\Datasource\EntityInterface {}
        public function newEmptyEntity(): \Cake\Datasource\EntityInterface {}
        public function newEntities(array $data): array {}
        public function save(\Cake\Datasource\EntityInterface $entity) {}
        public function belongsTo(string $associated, array $options = []): Association\BelongsTo {}
        public function hasMany(string $associated, array $options = []): Association\HasMany {}
        public function belongsToMany(string $associated, array $options = []): Association\BelongsToMany {}
        public function addBehavior(string $name, array $options = []) {}
        public function getBehavior(string $name): Behavior {}
        public function setEntityClass(string $name) {}
    }
    class Query implements \Cake\Datasource\QueryInterface {
        /** @return $this */
        public function where($conditions) {}
        /** @return $this */
        public function contain($associations) {}
        public function first() {}
        public function firstOrFail() {}
        public function all(): \Cake\Datasource\ResultSetInterface {}
        public function toArray(): array {}
    }
    class Association {
        public function getTarget(): Table {}
    }
    class Behavior {}
}
namespace Cake\ORM\Association {
    class BelongsTo extends \Cake\ORM\Association {}
    class HasMany extends \Cake\ORM\Association {}
    class BelongsToMany extends \Cake\ORM\Association {}
}
namespace Cake\ORM\Behavior {
    class TimestampBehavior extends \Cake\ORM\Behavior {
        public function touch($entity) {}
    }
}
namespace Cake\Controller {
    class Controller {
        use \Cake\ORM\Locator\LocatorAwareTrait;
        public function loadComponent(string $name, array $config = []): Component {}
        public function set($name, $value = null) {}
        public function render(?string $template = null, ?string $layout = null) {}
        public function redirect($url, int $status = 302) {}
        public function viewBuilder(): \Cake\View\ViewBuilder {}
        public function paginate($object = null, array $settings = []) {}
    }
    class Component {}
}
namespace Cake\Controller\Component {
    class FlashComponent extends \Cake\Controller\Component {
        public function success(string $message) {}
    }
}
namespace Cake\View {
    class View {
        public function element(string $name, array $data = [], array $options = []): string {}
        public function loadHelper(string $name, array $config = []) {}
        public function setLayout(string $name) {}
        public function fetch(string $name): string {}
        public function cell(string $cell, array $data = [], array $options = []) {}
    }
    class Helper {}
    class Cell {
        use \Cake\ORM\Locator\LocatorAwareTrait;
        public function set($name, $value = null) {}
    }
    class ViewBuilder {
        public function setTemplate(?string $name) {}
        public function setLayout(?string $name) {}
    }
}
namespace Cake\View\Helper {
    class HtmlHelper extends \Cake\View\Helper {
        public function link($title, $url = null, array $options = []): string {}
    }
}
namespace Cake\Core {
    class Configure {
        public static function read(?string $var = null, $default = null) {}
    }
}
namespace Cake\Routing {
    class Router {
        public static function url($url = null, bool $full = false): string {}
    }
    class RouteBuilder {
        public function connect($route, $defaults = [], array $options = []) {}
    }
}
"#;

const APP: &str = "src/Controller/AppController.php";

const APP_CONTROLLER: &str = r#"<?php
namespace App\Controller;

class AppController extends \Cake\Controller\Controller
{
    public function initialize(): void
    {
        $this->loadComponent('Flash');
        $this->loadComponent('Cart');
    }

    public function beforeRender()
    {
        $this->set('site', 'Shop');
    }
}
"#;

const ARTICLES: &str = "src/Controller/ArticlesController.php";

const CONTROLLER: &str = r#"<?php
namespace App\Controller;

use Cake\Core\Configure;
use Cake\Routing\Router;

class ArticlesController extends AppController
{
    public function index()
    {
        $articles = $this->Articles->find()->where(['published' => true])->toArray();
        $this->set(compact('articles'));
        $this->set('title', Configure::read('App.encoding'));
    }

    public function view($id)
    {
        $article = $this->fetchTable('Articles')->get($id);
        $this->set('article', $article);
        $this->viewBuilder()->setLayout('wide');
        return $this->render('view');
    }

    public function add()
    {
        $this->Flash->success('Saved');
        $tags = $this->fetchTable('Blog.Tags');
        Router::url(['_name' => 'login']);
        return $this->redirect(['action' => 'index']);
    }

    public function other()
    {
        $this->redirect(['plugin' => 'Blog', 'controller' => 'Posts', 'action' => 'recent']);
        return $this->redirect(['controller' => 'Authors', 'action' => 'profile']);
    }
}
"#;

const TABLE: &str = "src/Model/Table/ArticlesTable.php";

const ARTICLES_TABLE: &str = r#"<?php
namespace App\Model\Table;

class ArticlesTable extends \Cake\ORM\Table
{
    public function initialize(array $config): void
    {
        $this->addBehavior('Timestamp');
        $this->belongsTo('Authors');
        $this->hasMany('Comments');
        $this->belongsToMany('Blog.Tags');
    }

    public function published(): int
    {
        return 0;
    }
}
"#;

const MODELS: &str = r#"<?php
namespace App\Model\Table {
    class AuthorsTable extends \Cake\ORM\Table {}
    class CommentsTable extends \Cake\ORM\Table {}
}
namespace App\Model\Entity {
    /** @property string $title */
    class Article extends \Cake\ORM\Entity {}
    /** @property string $name */
    class Author extends \Cake\ORM\Entity {}
    /** @property string $body */
    class Comment extends \Cake\ORM\Entity {}
}
namespace App\Controller\Component {
    class CartComponent extends \Cake\Controller\Component {
        public function total(): int { return 0; }
    }
}
namespace App\View\Helper {
    class MarkdownHelper extends \Cake\View\Helper {
        public function toHtml(string $text): string { return $text; }
    }
}
namespace Blog\Model\Table {
    class TagsTable extends \Cake\ORM\Table {}
}
namespace Blog\Model\Entity {
    /** @property string $label */
    class Tag extends \Cake\ORM\Entity {}
}
"#;

const AUTHORS: &str = r#"<?php
namespace App\Controller;

class AuthorsController extends AppController
{
    public function profile() {}
}
"#;

const POSTS: &str = r#"<?php
namespace Blog\Controller;

class PostsController extends \Cake\Controller\Controller
{
    public function recent() {}
}
"#;

const CELL: &str = r#"<?php
namespace App\View\Cell;

class InboxCell extends \Cake\View\Cell
{
    public function display()
    {
        $this->set('unread', 3);
    }

    public function expanded() {}
}
"#;

const ROUTES: &str = r#"<?php
return function (\Cake\Routing\RouteBuilder $routes): void {
    $routes->connect('/login', ['controller' => 'Authors', 'action' => 'profile'], ['_name' => 'login']);
    $routes->connect('/articles/{id}', 'Articles::view');
};
"#;

const INDEX: &str = "templates/Articles/index.php";

const INDEX_TEMPLATE: &str = "<h1><?= $title ?></h1>\n<?= $this->element('sidebar', ['count' => 1]) ?>\n<?= $this->Html->link('Read', ['action' => 'view']) ?>\n<?= $this->cell('Inbox::expanded') ?>\n";

fn cake() -> Project {
    Project::new(&[
        ("composer.json", r#"{"require": {"cakephp/cakephp": "^5.0"}}"#),
        ("framework/classes.php", FRAMEWORK),
        (
            "config/app.php",
            "<?php\nreturn [\n    'debug' => true,\n    'App' => [\n        'namespace' => 'App',\n        'encoding' => 'UTF-8',\n    ],\n];\n",
        ),
        ("config/routes.php", ROUTES),
        (APP, APP_CONTROLLER),
        (ARTICLES, CONTROLLER),
        ("src/Controller/AuthorsController.php", AUTHORS),
        (TABLE, ARTICLES_TABLE),
        ("src/Model/models.php", MODELS),
        (
            "src/View/AppView.php",
            "<?php\nnamespace App\\View;\n\nclass AppView extends \\Cake\\View\\View\n{\n    public function initialize(): void\n    {\n        $this->loadHelper('Markdown');\n    }\n}\n",
        ),
        ("src/View/Cell/InboxCell.php", CELL),
        ("plugins/Blog/src/BlogPlugin.php", "<?php\nnamespace Blog;\nclass BlogPlugin {}\n"),
        ("plugins/Blog/src/Controller/PostsController.php", POSTS),
        (INDEX, INDEX_TEMPLATE),
        ("templates/Articles/view.php", "<p></p>\n"),
        ("templates/element/sidebar.php", "<p></p>\n"),
        ("templates/layout/default.php", "<html></html>\n"),
        ("templates/layout/wide.php", "<html></html>\n"),
        ("templates/cell/Inbox/display.php", "<p></p>\n"),
    ])
}

fn at(from: &str, to: &str) -> String {
    assert!(CONTROLLER.contains(from), "{from}");
    CONTROLLER.replacen(from, to, 1)
}

impl Project {
    fn cake_goes(&mut self, file: &str, source: &str) -> Vec<(String, u32)> {
        let found = self.definition(file, source).unwrap_or_default();
        found.into_iter().map(|(file, line, _)| (file, line)).collect()
    }

    fn cake_complete(&mut self, file: &str, source: &str) -> Vec<String> {
        let (text, pos) = caret(source);
        let doc = self.open(file, &text);
        let found = self.engine.completion(doc, pos, None, &CancellationToken::new()).expect("handled");
        self.engine.document_closed(doc);
        found.items.into_iter().map(|item| item.label).collect()
    }

    fn cake_hover(&mut self, file: &str, source: &str) -> String {
        let (text, pos) = caret(source);
        let doc = self.open(file, &text);
        let hover = self.engine.hover(doc, pos);
        self.engine.document_closed(doc);
        hover.map(|hover| hover.markdown).unwrap_or_default()
    }

    fn cake_problems(&mut self, file: &str, source: &str) -> Vec<String> {
        let doc = self.open(file, source);
        let inspected = self.engine.inspect(doc, &CancellationToken::new()).expect("inspected");
        self.engine.document_closed(doc);
        let found = inspected.problems.iter().filter(|problem| problem.inspection.contains("Cake"));
        found.map(|problem| problem.message.clone()).collect()
    }

    /// The members offered after `body`, a statement of an action of the
    /// articles controller.
    fn cake_members(&mut self, body: &str) -> Vec<String> {
        let source = at("$this->Flash->success('Saved');", body);
        let mut found = self.cake_complete(ARTICLES, &source);
        found.sort();
        found
    }
}

#[test]
fn the_names_of_a_cakephp_application_go_to_what_they_name() {
    let mut project = cake();
    assert!(project.engine.frameworks().contains(crate::frameworks::Framework::CakePhp));
    let mut goes = |from: &str, to: &str| project.cake_goes(ARTICLES, &at(from, to));
    let file = |name: &str, line: u32| vec![(name.to_owned(), line)];
    // A table, a plugin's.
    assert_eq!(goes("fetchTable('Articles')", "fetchTable('Arti/*|*/cles')"), file(TABLE, 3));
    assert_eq!(goes("fetchTable('Blog.Tags')", "fetchTable('Blog.Ta/*|*/gs')"), file("src/Model/models.php", 24));
    // A template of the controller, a layout.
    assert_eq!(goes("render('view')", "render('vi/*|*/ew')"), file("templates/Articles/view.php", 0));
    assert_eq!(goes("setLayout('wide')", "setLayout('wi/*|*/de')"), file("templates/layout/wide.php", 0));
    // A key of the configuration, a named route.
    assert_eq!(goes("read('App.encoding')", "read('App.enc/*|*/oding')"), file("config/app.php", 5));
    assert_eq!(goes("['_name' => 'login']", "['_name' => 'lo/*|*/gin']"), file("config/routes.php", 2));
    // Route arrays: an action of this controller, another controller and its action, a plugin's.
    assert_eq!(goes("['action' => 'index']", "['action' => 'in/*|*/dex']"), file(ARTICLES, 8));
    assert_eq!(
        goes(
            "'controller' => 'Authors', 'action' => 'profile'",
            "'controller' => 'Auth/*|*/ors', 'action' => 'profile'"
        ),
        file("src/Controller/AuthorsController.php", 3)
    );
    assert_eq!(
        goes(
            "'controller' => 'Authors', 'action' => 'profile'",
            "'controller' => 'Authors', 'action' => 'pro/*|*/file'"
        ),
        file("src/Controller/AuthorsController.php", 5)
    );
    assert_eq!(
        goes("'controller' => 'Posts', 'action' => 'recent'", "'controller' => 'Posts', 'action' => 'rec/*|*/ent'"),
        file("plugins/Blog/src/Controller/PostsController.php", 5)
    );
    assert_eq!(goes("['plugin' => 'Blog',", "['plugin' => 'Bl/*|*/og',"), file("plugins/Blog/src/BlogPlugin.php", 0));

    // What a class loads: a component of CakePHP, one of the application, a behaviour, an association.
    let found =
        project.cake_goes(APP, &APP_CONTROLLER.replacen("loadComponent('Cart')", "loadComponent('Ca/*|*/rt')", 1));
    assert_eq!(found, file("src/Model/models.php", 14));
    let found =
        project.cake_goes(APP, &APP_CONTROLLER.replacen("loadComponent('Flash')", "loadComponent('Fl/*|*/ash')", 1));
    assert_eq!(found[0].0, "framework/classes.php");
    let found = project
        .cake_goes(TABLE, &ARTICLES_TABLE.replacen("addBehavior('Timestamp')", "addBehavior('Time/*|*/stamp')", 1));
    assert_eq!(found[0].0, "framework/classes.php");
    let found =
        project.cake_goes(TABLE, &ARTICLES_TABLE.replacen("belongsTo('Authors')", "belongsTo('Auth/*|*/ors')", 1));
    assert_eq!(found, file("src/Model/models.php", 2));

    // In a template: an element, an action of the template's controller, a cell's method.
    let mut goes = |from: &str, to: &str| project.cake_goes(INDEX, &INDEX_TEMPLATE.replacen(from, to, 1));
    assert_eq!(goes("element('sidebar'", "element('side/*|*/bar'"), file("templates/element/sidebar.php", 0));
    assert_eq!(goes("['action' => 'view']", "['action' => 'vi/*|*/ew']"), file(ARTICLES, 15));
    assert_eq!(
        goes("cell('Inbox::expanded')", "cell('Inbox::expa/*|*/nded')"),
        file("src/View/Cell/InboxCell.php", 10)
    );
    // A route's string target.
    let found =
        project.cake_goes("config/routes.php", &ROUTES.replacen("'Articles::view'", "'Articles::vi/*|*/ew'", 1));
    assert_eq!(found, file(ARTICLES, 15));

    let hover = project.cake_hover(ARTICLES, &at("['action' => 'index']", "['action' => 'in/*|*/dex']"));
    assert!(hover.contains("**action** `index`") && hover.contains("ArticlesController::index()"), "{hover}");
    let hover = project.cake_hover(ARTICLES, &at("read('App.encoding')", "read('App.enc/*|*/oding')"));
    assert!(hover.contains("'UTF-8'"), "{hover}");
}

#[test]
fn cakephp_names_complete_by_what_is_asked_for() {
    let mut project = cake();
    let mut offered = |from: &str, to: &str| project.cake_complete(ARTICLES, &at(from, to));
    let found = offered("fetchTable('Articles')", "fetchTable('/*|*/')");
    for name in ["Articles", "Authors", "Comments", "Blog.Tags"] {
        assert!(found.contains(&name.to_owned()), "{name}: {found:?}");
    }
    assert_eq!(offered("render('view')", "render('/*|*/')"), ["index", "view"]);
    assert_eq!(offered("setLayout('wide')", "setLayout('/*|*/')"), ["default", "wide"]);
    assert_eq!(offered("['action' => 'index']", "['action' => '/*|*/']"), ["add", "index", "other", "view"]);
    let found = offered("read('App.encoding')", "read('/*|*/')");
    assert_eq!(found, ["App", "App.encoding", "App.namespace", "debug"]);
    assert_eq!(offered("['_name' => 'login']", "['_name' => '/*|*/']"), ["login"]);
    let found =
        project.cake_complete(APP, &APP_CONTROLLER.replacen("loadComponent('Cart')", "loadComponent('/*|*/')", 1));
    assert_eq!(found, ["Cart", "Flash"]);
    let found = project.cake_complete(INDEX, &INDEX_TEMPLATE.replacen("element('sidebar'", "element('/*|*/'", 1));
    assert_eq!(found, ["sidebar"]);
}

#[test]
fn cakephp_names_that_name_nothing_are_reported() {
    let mut project = cake();
    assert_eq!(project.cake_problems(ARTICLES, CONTROLLER), Vec::<String>::new());
    assert_eq!(project.cake_problems(APP, APP_CONTROLLER), Vec::<String>::new());
    assert_eq!(project.cake_problems(TABLE, ARTICLES_TABLE), Vec::<String>::new());
    assert_eq!(project.cake_problems(INDEX, INDEX_TEMPLATE), Vec::<String>::new());
    assert_eq!(project.cake_problems("config/routes.php", ROUTES), Vec::<String>::new());
    let source = at("render('view')", "render('missing')")
        .replacen("setLayout('wide')", "setLayout('nope')", 1)
        .replacen("['action' => 'index']", "['action' => 'gone']", 1);
    assert_eq!(
        project.cake_problems(ARTICLES, &source),
        ["Unknown layout 'nope'", "Unknown template 'missing'", "Controller 'ArticlesController' has no action 'gone'"]
    );
    let source = APP_CONTROLLER.replacen("loadComponent('Cart')", "loadComponent('Nothing')", 1);
    assert_eq!(project.cake_problems(APP, &source), ["Unknown component 'Nothing'"]);
    let source = ARTICLES_TABLE.replacen("addBehavior('Timestamp')", "addBehavior('Nope')", 1);
    assert_eq!(project.cake_problems(TABLE, &source), ["Unknown behavior 'Nope'"]);
    let source = INDEX_TEMPLATE.replacen("element('sidebar'", "element('nope'", 1);
    assert_eq!(project.cake_problems(INDEX, &source), ["Unknown element 'nope'"]);
}

#[test]
fn tables_entities_and_what_a_class_loads_are_typed() {
    let mut project = cake();
    let has = |found: &[String], names: &[&str]| {
        for name in names {
            assert!(found.contains(&(*name).to_owned()), "{name}: {found:?}");
        }
    };
    // The controller's table, by its name and through `fetchTable()`; a plugin's.
    has(&project.cake_members("$this->Articles->/*|*/;"), &["published", "find", "Authors", "Comments", "Tags"]);
    has(&project.cake_members("$this->fetchTable('Articles')->/*|*/;"), &["published"]);
    has(&project.cake_members("$this->fetchTable()->/*|*/;"), &["published"]);
    has(&project.cake_members("$this->getTableLocator()->get('Articles')->/*|*/;"), &["published"]);
    // A table's entity, and what a query of it finds.
    has(&project.cake_members("$this->Articles->get(1)->/*|*/;"), &["title", "isNew", "author", "comments", "tags"]);
    has(&project.cake_members("$this->Articles->newEmptyEntity()->/*|*/;"), &["title"]);
    has(&project.cake_members("$this->Articles->find()->where([])->first()->/*|*/;"), &["title"]);
    has(&project.cake_members("$this->Articles->find()->firstOrFail()->/*|*/;"), &["title"]);
    has(&project.cake_members("foreach ($this->Articles->find()->toArray() as $one) { $one->/*|*/; }"), &["title"]);
    has(&project.cake_members("foreach ($this->Articles->newEntities([]) as $one) { $one->/*|*/; }"), &["title"]);
    has(&project.cake_members("$this->fetchTable('Blog.Tags')->newEmptyEntity()->/*|*/;"), &["label"]);
    // An association is the table it leads to; an entity has what is associated.
    has(&project.cake_members("$this->Articles->Authors->/*|*/;"), &["find", "getTarget"]);
    has(&project.cake_members("$this->Articles->Authors->get(1)->/*|*/;"), &["name"]);
    has(&project.cake_members("$this->Articles->get(1)->author->/*|*/;"), &["name"]);
    has(&project.cake_members("foreach ($this->Articles->get(1)->comments as $one) { $one->/*|*/; }"), &["body"]);
    has(&project.cake_members("foreach ($this->Articles->get(1)->tags as $one) { $one->/*|*/; }"), &["label"]);
    // Components, the controller's and the ones of the class it extends; a behaviour.
    assert_eq!(project.cake_members("$this->Flash->/*|*/;"), ["success"]);
    assert_eq!(project.cake_members("$this->Cart->/*|*/;"), ["total"]);
    assert_eq!(project.cake_members("$this->Articles->getBehavior('Timestamp')->/*|*/;"), ["touch"]);
    has(&project.cake_members("$this->/*|*/;"), &["Articles", "Flash", "Cart", "index"]);
    // A component goes to where it is loaded.
    let found = project.cake_goes(ARTICLES, &at("$this->Flash->success", "$this->Fla/*|*/sh->success"));
    assert_eq!(found, [(APP.to_owned(), 7)]);
}

#[test]
fn a_cakephp_template_has_what_its_action_sets() {
    let mut project = cake();
    let has = |found: &[String], names: &[&str]| {
        for name in names {
            assert!(found.contains(&(*name).to_owned()), "{name}: {found:?}");
        }
    };
    // What the action sets, by name, by `compact()`; what every action is given.
    let found = project.cake_complete(INDEX, "<?php foreach ($articles as $one) { $one->/*|*/; } ?>\n");
    has(&found, &["title", "author"]);
    let found = project.cake_complete("templates/Articles/view.php", "<?php echo $article->/*|*/ ?>\n");
    has(&found, &["title"]);
    let hover = project.cake_hover(INDEX, "<?php $x = $site; echo $/*|*/x; ?>\n");
    assert!(hover.contains("string"), "{hover}");
    // `$this` is the application's view: its methods, the helpers it loads and CakePHP's.
    let found = project.cake_complete(INDEX, "<?php $this->/*|*/ ?>\n");
    has(&found, &["element", "Html", "Markdown"]);
    assert_eq!(project.cake_complete(INDEX, "<?php $this->Markdown->/*|*/ ?>\n"), ["toHtml"]);
    assert_eq!(project.cake_complete(INDEX, "<?php $this->Html->/*|*/ ?>\n"), ["link"]);
    // An element has what it is passed, a cell's template what its method sets.
    let hover = project.cake_hover("templates/element/sidebar.php", "<?php $x = $count; echo $/*|*/x; ?>\n");
    assert!(hover.contains("int"), "{hover}");
    let hover = project.cake_hover("templates/cell/Inbox/display.php", "<?php $x = $unread; echo $/*|*/x; ?>\n");
    assert!(hover.contains("int"), "{hover}");
}

#[test]
fn cakephp_names_are_only_read_in_a_cakephp_project() {
    let mut project = Project::new(&[
        ("framework/classes.php", FRAMEWORK),
        (ARTICLES, CONTROLLER),
        (APP, APP_CONTROLLER),
        (TABLE, ARTICLES_TABLE),
    ]);
    assert!(!project.engine.frameworks().contains(crate::frameworks::Framework::CakePhp));
    let source = at("render('view')", "render('missing')");
    assert_eq!(project.cake_problems(ARTICLES, &source), Vec::<String>::new());
    assert!(project.cake_goes(ARTICLES, &at("fetchTable('Articles')", "fetchTable('Arti/*|*/cles')")).is_empty());
}

#[test]
fn associations_are_names_where_a_query_contains_them() {
    let mut project = cake();
    let probe = |body: &str| at("$this->Flash->success('Saved');", body);
    let file = |name: &str, line: u32| vec![(name.to_owned(), line)];
    // On a query of the table's entity, and on the table itself.
    let source = probe("$this->Articles->find()->contain(['Auth/*|*/ors']);");
    assert_eq!(project.cake_goes(ARTICLES, &source), file(TABLE, 8));
    let source = probe("$this->Articles->find()->contain('Comm/*|*/ents');");
    assert_eq!(project.cake_goes(ARTICLES, &source), file(TABLE, 9));
    let source = probe("$this->Articles->getAssociation('Ta/*|*/gs');");
    assert_eq!(project.cake_goes(ARTICLES, &source), file(TABLE, 10));
    let source = probe("$this->Articles->find()->contain(['/*|*/']);");
    assert_eq!(project.cake_complete(ARTICLES, &source), ["Authors", "Comments", "Tags"]);
    let source = probe("$this->Articles->find()->contain(['Auth/*|*/ors']);");
    let hover = project.cake_hover(ARTICLES, &source);
    assert!(hover.contains("**association** `Authors`") && hover.contains("ArticlesTable"), "{hover}");
}

#[test]
fn what_is_paginated_is_the_tables_entity() {
    let mut project = cake();
    let has = |found: &[String], names: &[&str]| {
        for name in names {
            assert!(found.contains(&(*name).to_owned()), "{name}: {found:?}");
        }
    };
    // The controller's table, a table, a query.
    has(&project.cake_members("$this->paginate()->first()->/*|*/;"), &["title"]);
    has(&project.cake_members("$this->paginate($this->fetchTable('Authors'))->first()->/*|*/;"), &["name"]);
    has(&project.cake_members("$this->paginate($this->Articles->find())->toArray()[0]->/*|*/;"), &["title"]);
}

#[test]
fn a_template_and_an_action_have_usages() {
    let mut project = cake();
    let usages = |project: &mut Project, file: &str, source: &str| -> Vec<(String, u32)> {
        let (text, pos) = caret(source);
        let doc = project.open(file, &text);
        let found =
            project.engine.references(doc, pos, true, &CancellationToken::new()).expect("handled").unwrap_or_default();
        project.engine.document_closed(doc);
        let root = project.engine.root();
        let mut out: Vec<(String, u32)> = found
            .into_iter()
            .map(|target| {
                let path = target.path.strip_prefix(&root).unwrap_or(&target.path).to_string_lossy().replace('\\', "/");
                (path, target.range.start.line)
            })
            .collect();
        out.sort();
        out.dedup();
        out
    };
    // An action: the route array of its controller, the template's, the route's string.
    let found =
        usages(&mut project, INDEX, &INDEX_TEMPLATE.replacen("['action' => 'view']", "['action' => 'vi/*|*/ew']", 1));
    let files: Vec<&str> = found.iter().map(|(file, _)| file.as_str()).collect();
    assert!(files.contains(&INDEX) && files.contains(&"config/routes.php"), "{found:?}");
    // An element: where it is rendered.
    let found = usages(&mut project, INDEX, &INDEX_TEMPLATE.replacen("element('sidebar'", "element('side/*|*/bar'", 1));
    assert!(found.iter().any(|(file, line)| file == INDEX && *line == 1), "{found:?}");
}

// --------------------------------------------- what the rest of the files say

const MIGRATION: &str = r#"<?php
use Migrations\AbstractMigration;

class CreateArticles extends AbstractMigration
{
    public function change(): void
    {
        $table = $this->table('articles');
        $table->addColumn('title', 'string', ['null' => false]);
        $table->addColumn('views', 'integer', ['null' => true])
            ->addColumn('rating', 'float', ['null' => false]);
        $table->create();
    }
}
"#;

const THEMED_APP: &str = r#"<?php
namespace App\Controller;

class AppController extends \Cake\Controller\Controller
{
    public function initialize(): void
    {
        $this->loadComponent('Flash');
        $this->loadComponent('Cart');
        $this->viewBuilder()->setTheme('Modern');
    }
}
"#;

const REPORTS: &str = r#"<?php
namespace App\Controller;

class ReportsController extends AppController
{
    public function summary()
    {
        $this->viewBuilder()->setTemplatePath('Common');
        $title = __('Saved');
        $cart = __d('shop', 'Cart');
        $this->set(compact('title', 'cart'));
        return $this->render('totals');
    }
}
"#;

const NESTED: &str = r#"<?php
namespace App\Model\Table {
    class AuthorsTable extends \Cake\ORM\Table {}
    class CommentsTable extends \Cake\ORM\Table
    {
        public function initialize(array $config): void
        {
            $this->belongsTo('Authors');
        }
    }
}
namespace App\Model\Entity {
    /** @property string $title */
    class Article extends \Cake\ORM\Entity {}
    /** @property string $name */
    class Author extends \Cake\ORM\Entity {}
    /** @property string $body */
    class Comment extends \Cake\ORM\Entity {}
}
namespace App\Controller\Component {
    class CartComponent extends \Cake\Controller\Component {
        public function total(): int { return 0; }
    }
}
namespace App\View\Helper {
    class MarkdownHelper extends \Cake\View\Helper {
        public function toHtml(string $text): string { return $text; }
    }
}
namespace Blog\Model\Table {
    class TagsTable extends \Cake\ORM\Table {}
}
namespace Blog\Model\Entity {
    /** @property string $label */
    class Tag extends \Cake\ORM\Entity {}
}
"#;

const REPORT: &str = "src/Controller/ReportsController.php";

/// The application of [`cake`] with a migration, translations, a theme, a
/// folder of templates set in code and an association of an associated table.
fn cake_with_more() -> Project {
    let project = cake();
    let files: [(&str, &str); 9] = [
        ("config/Migrations/20240101000000_CreateArticles.php", MIGRATION),
        ("resources/locales/fr/default.po", "msgid \"Saved\"\nmsgstr \"Enregistré\"\n\nmsgid \"Bye\"\nmsgstr \"\"\n"),
        ("resources/locales/fr/shop.po", "msgid \"Cart\"\nmsgstr \"Panier\"\n"),
        (APP, THEMED_APP),
        (REPORT, REPORTS),
        ("src/Model/models.php", NESTED),
        ("plugins/Modern/templates/Articles/view.php", "<p></p>\n"),
        ("plugins/Modern/templates/layout/wide.php", "<html></html>\n"),
        ("templates/Common/totals.php", "<p></p>\n"),
    ];
    let mut changed = Vec::new();
    for (name, text) in files {
        let path = project.root.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, text).unwrap();
        changed.push(path);
    }
    project.engine.files_changed(&changed, &[]);
    assert!(project.engine.wait_idle(WAIT));
    project
}

#[test]
fn an_entitys_fields_are_its_tables_columns() {
    let mut project = cake_with_more();
    let migration = "config/Migrations/20240101000000_CreateArticles.php";
    let found = project.cake_members("$this->Articles->get(1)->/*|*/;");
    for name in ["id", "title", "views", "rating", "author"] {
        assert!(found.contains(&name.to_owned()), "{name}: {found:?}");
    }
    // A column is typed, and goes to its migration.
    let hover = project.cake_hover(
        ARTICLES,
        &at("$this->Flash->success('Saved');", "$views = $this->Articles->get(1)->views; echo $vie/*|*/ws;"),
    );
    assert!(hover.contains("int|null") || hover.contains("?int"), "{hover}");
    let hover = project.cake_hover(
        ARTICLES,
        &at("$this->Flash->success('Saved');", "$rating = $this->Articles->get(1)->rating; echo $rat/*|*/ing;"),
    );
    assert!(hover.contains("float") && !hover.contains("null"), "{hover}");
    let found =
        project.cake_goes(ARTICLES, &at("$this->Flash->success('Saved');", "$this->Articles->get(1)->vie/*|*/ws;"));
    assert_eq!(found, [(migration.to_owned(), 9)]);
}

#[test]
fn translations_are_names_of_their_catalogue() {
    let mut project = cake_with_more();
    let file = |name: &str, line: u32| vec![(name.to_owned(), line)];
    let found = project.cake_goes(REPORT, &REPORTS.replacen("__('Saved')", "__('Sa/*|*/ved')", 1));
    assert_eq!(found, file("resources/locales/fr/default.po", 0));
    // A message of another catalogue, and the catalogue.
    let found = project.cake_goes(REPORT, &REPORTS.replacen("__d('shop', 'Cart')", "__d('shop', 'Ca/*|*/rt')", 1));
    assert_eq!(found, file("resources/locales/fr/shop.po", 0));
    let found = project.cake_goes(REPORT, &REPORTS.replacen("__d('shop', 'Cart')", "__d('sh/*|*/op', 'Cart')", 1));
    assert_eq!(found, file("resources/locales/fr/shop.po", 0));
    assert_eq!(project.cake_complete(REPORT, &REPORTS.replacen("__('Saved')", "__('/*|*/')", 1)), ["Bye", "Saved"]);
    assert_eq!(
        project.cake_complete(REPORT, &REPORTS.replacen("__d('shop', 'Cart')", "__d('shop', '/*|*/')", 1)),
        ["Cart"]
    );
    let hover = project.cake_hover(REPORT, &REPORTS.replacen("__('Saved')", "__('Sa/*|*/ved')", 1));
    assert!(hover.contains("**message** `Saved`") && hover.contains("fr: Enregistré"), "{hover}");
}

#[test]
fn templates_are_where_the_code_puts_them() {
    let mut project = cake_with_more();
    let file = |name: &str, line: u32| vec![(name.to_owned(), line)];
    // A theme's template and layout come first; what it doesn't have is the application's.
    let found = project.cake_goes(ARTICLES, &at("render('view')", "render('vi/*|*/ew')"));
    assert_eq!(found, file("plugins/Modern/templates/Articles/view.php", 0));
    let found = project.cake_goes(ARTICLES, &at("setLayout('wide')", "setLayout('wi/*|*/de')"));
    assert_eq!(found, file("plugins/Modern/templates/layout/wide.php", 0));
    let found = project.cake_goes(ARTICLES, &at("render('view')", "render('ind/*|*/ex')"));
    assert_eq!(found, file(INDEX, 0));
    // A folder of templates set in code.
    let found = project.cake_goes(REPORT, &REPORTS.replacen("render('totals')", "render('tot/*|*/als')", 1));
    assert_eq!(found, file("templates/Common/totals.php", 0));
    assert_eq!(project.cake_problems(REPORT, REPORTS), Vec::<String>::new());
    assert_eq!(project.cake_problems(ARTICLES, CONTROLLER), Vec::<String>::new());
    // A name neither the theme nor the application has is still reported.
    let source = REPORTS.replacen("render('totals')", "render('nothing')", 1);
    assert_eq!(project.cake_problems(REPORT, &source), ["Unknown template 'nothing'"]);
}

#[test]
fn an_association_below_another_is_one_of_the_associated_table() {
    let mut project = cake_with_more();
    let probe = |body: &str| at("$this->Flash->success('Saved');", body);
    let file = |name: &str, line: u32| vec![(name.to_owned(), line)];
    // In the array under an association, and after a dot.
    let source = probe("$this->Articles->find()->contain(['Comments' => ['Auth/*|*/ors']]);");
    assert_eq!(project.cake_goes(ARTICLES, &source), file("src/Model/models.php", 7));
    let source = probe("$this->Articles->find()->contain('Comments.Auth/*|*/ors');");
    assert_eq!(project.cake_goes(ARTICLES, &source), file("src/Model/models.php", 7));
    let source = probe("$this->Articles->find()->contain(['Comments' => ['/*|*/']]);");
    assert_eq!(project.cake_complete(ARTICLES, &source), ["Authors"]);
    // The key itself is the table's own.
    let source = probe("$this->Articles->find()->contain(['Comm/*|*/ents' => ['Authors']]);");
    assert_eq!(project.cake_goes(ARTICLES, &source), file(TABLE, 9));
}
