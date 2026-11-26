//! Yii 2, on a stand-in for its API written for these tests (the names of
//! its classes and methods, nothing of their code) and a small application.

use super::*;

/// The framework's classes.
const FRAMEWORK: &str = r#"<?php
namespace yii {
    class BaseYii {
        /** @var \yii\web\Application */
        public static $app;
        /** @var \yii\di\Container */
        public static $container;
        public static function t($category, $message, $params = [], $language = null) {}
        public static function getAlias($alias, $throwException = true) {}
        public static function setAlias($alias, $path) {}
        public static function createObject($type, array $params = []) {}
    }
}
namespace yii\base {
    class BaseObject {}
    class Component extends BaseObject {}
    class Model extends Component {
        public function rules() { return []; }
        public function attributeLabels() { return []; }
        public function getAttributeLabel($attribute) {}
        public function addError($attribute, $error = '') {}
    }
    class Module extends \yii\di\ServiceLocator {
        public $layout;
        public function getModule($id, $load = true) {}
    }
    class Application extends Module {}
    class Controller extends Component {
        public $layout;
        public function render($view, $params = []) {}
        public function renderPartial($view, $params = []) {}
    }
    class View extends Component {
        public function render($view, $params = [], $context = null) {}
    }
    class Widget extends Component {
        public function render($view, $params = []) {}
    }
}
namespace yii\di {
    class ServiceLocator extends \yii\base\Component {
        public function get($id, $throwException = true) {}
        public function has($id, $checkInstance = false) {}
    }
    class Container extends \yii\base\Component {
        public function get($class, $params = [], $config = []) {}
    }
}
namespace yii\web {
    /**
     * @property User $user
     */
    class Application extends \yii\base\Application {}
    class Controller extends \yii\base\Controller {
        public function redirect($url, $statusCode = 302) {}
    }
    class View extends \yii\base\View {
        public $title;
    }
    /**
     * @property IdentityInterface|null $identity
     */
    class User extends \yii\base\Component {
        public function getIsGuest() {}
    }
    interface IdentityInterface {}
}
namespace yii\db {
    class BaseActiveRecord extends \yii\base\Model {
        /** @return ActiveQuery */
        public static function find() {}
        public function hasOne($class, $link) {}
        public function hasMany($class, $link) {}
        public function getAttribute($name) {}
    }
    class ActiveRecord extends BaseActiveRecord {
        public static function tableName() {}
    }
    class ActiveQuery {
        /** @return $this */
        public function where($condition) {}
        /** @return $this */
        public function orderBy($columns) {}
        public function one($db = null) {}
        public function all($db = null) {}
    }
}
namespace yii\helpers {
    class BaseUrl {
        public static function to($url = '', $scheme = false) {}
        public static function toRoute($route, $scheme = false) {}
    }
    class Url extends BaseUrl {}
    class BaseHtml {
        public static function a($text, $url = null, $options = []) {}
    }
    class Html extends BaseHtml {}
}
namespace yii\caching {
    class FileCache extends \yii\base\Component {
        public function flush() {}
    }
}
namespace {
    class Yii extends \yii\BaseYii {}
}
"#;

const CONFIG: &str = r#"<?php
$params = require __DIR__ . '/params.php';
$config = [
    'id' => 'shop',
    'basePath' => dirname(__DIR__),
    'components' => [
        'cache' => ['class' => 'yii\caching\FileCache'],
        'mailer' => ['class' => \app\components\Mailer::class],
        'user' => ['identityClass' => 'app\models\User'],
    ],
    'modules' => [
        'admin' => ['class' => 'app\modules\admin\Module'],
    ],
    'params' => $params,
];
return $config;
"#;

const CLASSES: &str = r#"<?php
namespace app\components {
    class Mailer extends \yii\base\Component { public function send(): bool { return true; } }
}
"#;

const MODELS: &str = r#"<?php
namespace app\models;

class User extends \yii\db\ActiveRecord implements \yii\web\IdentityInterface
{
    public $name;
}

class Tag extends \yii\db\ActiveRecord
{
    public $label;
}

class Post extends \yii\db\ActiveRecord
{
    public $title;

    public function getAuthor()
    {
        return $this->hasOne(User::class, ['id' => 'author_id']);
    }

    public function getTags()
    {
        return $this->hasMany(Tag::class, ['post_id' => 'id']);
    }

    public function getSummary(): string
    {
        return '';
    }
}
"#;

const FORM: &str = "models/ContactForm.php";

const CONTACT: &str = r#"<?php
namespace app\models;

class ContactForm extends \yii\base\Model
{
    public $email;
    public $body;
    public $verifyCode;

    public function rules()
    {
        return [
            [['email', 'body'], 'required'],
            ['email', 'email'],
            ['verifyCode', 'captcha', 'skipOnEmpty' => ['never']],
        ];
    }

    public function attributeLabels()
    {
        return [
            'verifyCode' => 'Verification Code',
        ];
    }
}
"#;

const SITE: &str = "controllers/SiteController.php";

const CONTROLLER: &str = r#"<?php
namespace app\controllers;

use Yii;
use yii\helpers\Url;
use app\models\Post;

class SiteController extends \yii\web\Controller
{
    public $layout = 'main';

    public function actionIndex()
    {
        $post = Post::find()->where(['id' => 1])->one();
        return $this->render('index', ['post' => $post, 'heading' => Yii::t('app', 'Hello')]);
    }

    public function actionAbout()
    {
        Yii::$app->get('cache');
        $runtime = Yii::getAlias('@app/models/Post.php');
        return $this->redirect(['post/view-all', 'page' => 2]);
    }

    public function actionLinks()
    {
        return [Url::to(['about']), Url::toRoute('admin/user/index'), Url::to(['/post/view', 'id' => 1])];
    }
}
"#;

const POSTS: &str = r#"<?php
namespace app\controllers;

class PostController extends \yii\web\Controller
{
    public function actionView($id) {}
    public function actionViewAll() {}
    protected function actionHidden() {}
}
"#;

const USERS: &str = r#"<?php
namespace app\modules\admin\controllers;

class UserController extends \yii\web\Controller
{
    public function actionIndex() {}
}
"#;

const VIEW: &str = "views/site/index.php";

fn yii() -> Project {
    Project::new(&[
        ("composer.json", r#"{"require": {"yiisoft/yii2": "~2.0.50"}}"#),
        ("framework/classes.php", FRAMEWORK),
        ("config/web.php", CONFIG),
        ("config/params.php", "<?php\nreturn ['adminEmail' => 'admin@example.com'];\n"),
        ("components/classes.php", CLASSES),
        ("models/Post.php", MODELS),
        (FORM, CONTACT),
        (SITE, CONTROLLER),
        ("controllers/PostController.php", POSTS),
        (
            "modules/admin/Module.php",
            "<?php\nnamespace app\\modules\\admin;\nclass Module extends \\yii\\base\\Module {}\n",
        ),
        ("modules/admin/controllers/UserController.php", USERS),
        (VIEW, "<h1><?= $heading ?></h1>\n<?= $this->render('_row') ?>\n"),
        ("views/site/_row.php", "<p></p>\n"),
        ("views/site/about.php", "<p></p>\n"),
        ("views/layouts/main.php", "<html></html>\n"),
        ("views/post/view.php", "<p></p>\n"),
        ("messages/de/app.php", "<?php\nreturn [\n    'Hello' => 'Hallo',\n    'Bye' => 'Tschüss',\n];\n"),
    ])
}

fn at(from: &str, to: &str) -> String {
    assert!(CONTROLLER.contains(from), "{from}");
    CONTROLLER.replacen(from, to, 1)
}

impl Project {
    fn yii_goes(&mut self, file: &str, source: &str) -> Vec<(String, u32)> {
        let found = self.definition(file, source).unwrap_or_default();
        found.into_iter().map(|(file, line, _)| (file, line)).collect()
    }

    fn yii_complete(&mut self, file: &str, source: &str) -> Vec<String> {
        let (text, pos) = caret(source);
        let doc = self.open(file, &text);
        let found = self.engine.completion(doc, pos, None, &CancellationToken::new()).expect("handled");
        self.engine.document_closed(doc);
        found.items.into_iter().map(|item| item.label).collect()
    }

    fn yii_hover(&mut self, file: &str, source: &str) -> String {
        let (text, pos) = caret(source);
        let doc = self.open(file, &text);
        let hover = self.engine.hover(doc, pos);
        self.engine.document_closed(doc);
        hover.map(|hover| hover.markdown).unwrap_or_default()
    }

    fn yii_problems(&mut self, file: &str, source: &str) -> Vec<String> {
        let doc = self.open(file, source);
        let inspected = self.engine.inspect(doc, &CancellationToken::new()).expect("inspected");
        self.engine.document_closed(doc);
        let found = inspected.problems.iter().filter(|problem| problem.inspection.contains("Yii"));
        found.map(|problem| problem.message.clone()).collect()
    }

    /// The members offered after `body`, a statement of a function.
    fn yii_members(&mut self, body: &str) -> Vec<String> {
        let source = format!("<?php\nuse app\\models\\Post;\nfunction probe(Post $post) {{\n  {body}\n}}\n");
        let mut found = self.yii_complete("helpers/probe.php", &source);
        found.sort();
        found
    }
}

#[test]
fn the_names_of_a_yii_application_go_to_what_they_name() {
    let mut project = yii();
    assert!(project.engine.frameworks().contains(crate::frameworks::Framework::Yii));
    let mut goes = |from: &str, to: &str| project.yii_goes(SITE, &at(from, to));
    let file = |name: &str, line: u32| vec![(name.to_owned(), line)];
    // A view of the controller, its layout.
    assert_eq!(goes("render('index'", "render('ind/*|*/ex'"), file(VIEW, 0));
    assert_eq!(goes("$layout = 'main'", "$layout = 'ma/*|*/in'"), file("views/layouts/main.php", 0));
    // Routes: another controller's action, one of this controller, a module's.
    assert_eq!(goes("['post/view-all'", "['post/view-/*|*/all'"), file("controllers/PostController.php", 6));
    assert_eq!(goes("Url::to(['about'])", "Url::to(['ab/*|*/out'])"), file(SITE, 17));
    assert_eq!(
        goes("toRoute('admin/user/index')", "toRoute('admin/user/in/*|*/dex')"),
        file("modules/admin/controllers/UserController.php", 5)
    );
    assert_eq!(
        goes("['/post/view', 'id' => 1]", "['/post/vi/*|*/ew', 'id' => 1]"),
        file("controllers/PostController.php", 5)
    );
    // The configuration's names, an aliased path, a message and its category.
    assert_eq!(goes("get('cache')", "get('ca/*|*/che')"), file("config/web.php", 6));
    assert_eq!(goes("'@app/models/Post.php'", "'@app/models/Po/*|*/st.php'"), file("models/Post.php", 0));
    assert_eq!(goes("t('app', 'Hello')", "t('app', 'He/*|*/llo')"), file("messages/de/app.php", 2));
    assert_eq!(goes("t('app', 'Hello')", "t('a/*|*/pp', 'Hello')"), file("messages/de/app.php", 0));

    // A view names another beside it.
    let found = project.yii_goes(VIEW, "<?= $this->render('_r/*|*/ow') ?>\n");
    assert_eq!(found, file("views/site/_row.php", 0));

    let hover = project.yii_hover(SITE, &at("t('app', 'Hello')", "t('app', 'He/*|*/llo')"));
    assert!(hover.contains("**message** `Hello`") && hover.contains("de: Hallo"), "{hover}");
    let hover = project.yii_hover(SITE, &at("['post/view-all'", "['post/view-/*|*/all'"));
    assert!(hover.contains("PostController::actionViewAll()"), "{hover}");
}

#[test]
fn yii_names_complete_by_what_is_asked_for() {
    let mut project = yii();
    let mut offered = |from: &str, to: &str| project.yii_complete(SITE, &at(from, to));
    assert_eq!(offered("render('index'", "render('/*|*/'"), ["_row", "about", "index"]);
    assert_eq!(
        offered("['post/view-all'", "['/*|*/'"),
        ["admin/user/index", "post/view", "post/view-all", "site/about", "site/index", "site/links"]
    );
    assert_eq!(offered("get('cache')", "get('/*|*/')"), ["cache", "mailer", "user"]);
    assert_eq!(offered("t('app', 'Hello')", "t('app', '/*|*/')"), ["Bye", "Hello"]);
    assert_eq!(offered("t('app', 'Hello')", "t('/*|*/', 'Hello')"), ["app"]);
    let aliases = offered("'@app/models/Post.php'", "'/*|*/'");
    assert!(aliases.contains(&"@app".to_owned()) && aliases.contains(&"@runtime".to_owned()), "{aliases:?}");
}

#[test]
fn a_models_attributes_are_names_in_its_rules_and_labels() {
    let mut project = yii();
    let source = |from: &str, to: &str| {
        assert!(CONTACT.contains(from), "{from}");
        CONTACT.replacen(from, to, 1)
    };
    let file = |line: u32| vec![(FORM.to_owned(), line)];
    assert_eq!(project.yii_goes(FORM, &source("[['email', 'body']", "[['email', 'bo/*|*/dy']")), file(6));
    assert_eq!(project.yii_goes(FORM, &source("['email', 'email']", "['em/*|*/ail', 'email']")), file(5));
    assert_eq!(project.yii_goes(FORM, &source("'verifyCode' =>", "'verify/*|*/Code' =>")), file(7));
    // Not the validator, nor what its options list.
    assert!(project.yii_goes(FORM, &source("['email', 'email']", "['email', 'em/*|*/ail']")).is_empty());
    assert!(project.yii_goes(FORM, &source("['never']", "['nev/*|*/er']")).is_empty());
    let found = project.yii_complete(FORM, &source("[['email', 'body']", "[['/*|*/', 'body']"));
    assert_eq!(found, ["body", "email", "verifyCode"]);

    let missing = source("['email', 'email']", "['mail', 'email']").replacen("'verifyCode' =>", "'code' =>", 1);
    assert_eq!(project.yii_problems(FORM, &missing), ["Unknown attribute 'mail'", "Unknown attribute 'code'"]);
    assert!(project.yii_problems(FORM, CONTACT).is_empty());
}

#[test]
fn names_that_name_nothing_are_reported() {
    let mut project = yii();
    assert_eq!(project.yii_problems(SITE, CONTROLLER), Vec::<String>::new());
    let source = at("render('index'", "render('missing'")
        .replacen("['post/view-all'", "['post/nothing'", 1)
        .replacen("Url::to(['about'])", "Url::to(['gone'])", 1)
        .replacen("toRoute('admin/user/index')", "toRoute('nobody/index')", 1);
    assert_eq!(
        project.yii_problems(SITE, &source),
        [
            "Unknown view 'missing'",
            "Controller 'PostController' has no action 'nothing'",
            "Controller 'SiteController' has no action 'gone'",
            "Unknown route 'nobody/index'",
        ]
    );
}

#[test]
fn components_objects_and_records_are_typed() {
    let mut project = yii();
    // The configuration's components, by property and by id.
    assert_eq!(project.yii_members("\\Yii::$app->cache->/*|*/"), ["flush"]);
    assert_eq!(project.yii_members("\\Yii::$app->mailer->/*|*/"), ["send"]);
    assert_eq!(project.yii_members("\\Yii::$app->get('mailer')->/*|*/"), ["send"]);
    assert!(project.yii_members("\\Yii::$app->nothing->/*|*/").is_empty());
    // The identity class of the user component.
    let identity = project.yii_members("\\Yii::$app->user->identity->/*|*/");
    assert!(identity.contains(&"name".to_owned()), "{identity:?}");
    // The factory and the container.
    assert_eq!(project.yii_members("\\Yii::createObject(\\app\\components\\Mailer::class)->/*|*/"), ["send"]);
    assert_eq!(project.yii_members("\\Yii::createObject('app\\components\\Mailer')->/*|*/"), ["send"]);
    assert_eq!(project.yii_members("\\Yii::$container->get(\\app\\components\\Mailer::class)->/*|*/"), ["send"]);
    // A query keeps its model; a relation is the related model, or a list of them.
    let found = project.yii_members("Post::find()->where([])->orderBy('id')->one()->/*|*/");
    assert!(found.contains(&"title".to_owned()) && found.contains(&"getAuthor".to_owned()), "{found:?}");
    let found = project.yii_members("foreach (Post::find()->all() as $one) { $one->/*|*/ }");
    assert!(found.contains(&"title".to_owned()), "{found:?}");
    let found = project.yii_members("$post->author->/*|*/");
    assert!(found.contains(&"name".to_owned()), "{found:?}");
    let found = project.yii_members("foreach ($post->tags as $tag) { $tag->/*|*/ }");
    assert!(found.contains(&"label".to_owned()), "{found:?}");
    let hover = project.yii_hover(
        "helpers/probe.php",
        "<?php\nfunction probe(\\app\\models\\Post $post) {\n  $text = $post->summary;\n  return $te/*|*/xt;\n}\n",
    );
    assert!(hover.contains("string"), "{hover}");
}

#[test]
fn components_and_getters_are_members() {
    let mut project = yii();
    let found = project.yii_members("\\Yii::$app->/*|*/");
    assert!(found.contains(&"cache".to_owned()) && found.contains(&"mailer".to_owned()), "{found:?}");
    let found = project.yii_members("$post->/*|*/");
    for name in ["author", "tags", "summary", "title"] {
        assert!(found.contains(&name.to_owned()), "{name}: {found:?}");
    }
    // A component goes to its key of the configuration, a relation to its getter.
    let probe = "<?php\nfunction probe(\\app\\models\\Post $post) {\n  \\Yii::$app->mai/*|*/ler;\n}\n";
    assert_eq!(project.yii_goes("helpers/probe.php", probe), [("config/web.php".to_owned(), 7)]);
    let probe = "<?php\nfunction probe(\\app\\models\\Post $post) {\n  $post->aut/*|*/hor;\n}\n";
    assert_eq!(project.yii_goes("helpers/probe.php", probe), [("models/Post.php".to_owned(), 17)]);
    let hover = project.yii_hover("helpers/probe.php", probe);
    assert!(hover.contains("Read through `getAuthor()`") && hover.contains("User"), "{hover}");
}

#[test]
fn a_yii_view_has_what_its_controller_passes() {
    let mut project = yii();
    let found = project.yii_complete(VIEW, "<?php echo $post->/*|*/ ?>\n");
    assert!(found.contains(&"title".to_owned()) && found.contains(&"getAuthor".to_owned()), "{found:?}");
    let variables = project.engine.yii_view_variables(&project.root.join(VIEW));
    let names: Vec<&str> = variables.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names, ["heading", "post"]);
    // `$this` is the view.
    let found = project.yii_complete(VIEW, "<?php $this->/*|*/ ?>\n");
    assert!(found.contains(&"title".to_owned()) && found.contains(&"render".to_owned()), "{found:?}");
    // Nothing of a view is reported: its variables are set by who renders it.
    let doc = project.open(VIEW, "<h1><?= $heading ?></h1>\n<?= $post->title ?>\n<?= $this->render('_row') ?>\n");
    let inspected = project.engine.inspect(doc, &CancellationToken::new()).expect("inspected");
    project.engine.document_closed(doc);
    let messages: Vec<&str> = inspected.problems.iter().map(|problem| problem.message.as_str()).collect();
    assert!(messages.is_empty(), "{messages:?}");
    // Another view of the folder has none, and a file that is no view neither.
    assert!(project.engine.yii_view_variables(&project.root.join("views/site/about.php")).is_empty());
    assert!(project.engine.yii_view_variables(&project.root.join(SITE)).is_empty());
}

#[test]
fn a_view_and_an_action_have_usages() {
    let mut project = yii();
    let usages = |project: &mut Project, file: &str, source: &str| {
        let (text, pos) = caret(source);
        let doc = project.open(file, &text);
        let usages = project.engine.references(doc, pos, true, &CancellationToken::new()).expect("handled").unwrap();
        let mut found: Vec<_> = usages.iter().map(|usage| project.describe(&usage.path, usage.range)).collect();
        project.engine.document_closed(doc);
        found.sort();
        found
    };
    // The action named from its own controller and, with its controller, from a view.
    let view = "<?= \\yii\\helpers\\Url::to(['site/about']) ?>\n<?= \\yii\\helpers\\Url::to(['about']) ?>\n";
    std::fs::write(project.root.join("views/site/about.php"), view).unwrap();
    project.engine.files_changed(&[project.root.join("views/site/about.php")], &[]);
    assert!(project.engine.wait_idle(WAIT));
    let found = usages(&mut project, SITE, &at("Url::to(['about'])", "Url::to(['ab/*|*/out'])"));
    assert_eq!(
        found,
        [
            // The action itself, then what names it.
            target(SITE, 17, "actionAbout"),
            target(SITE, 26, "about"),
            target("views/site/about.php", 0, "site/about"),
            target("views/site/about.php", 1, "about")
        ]
    );
    // A view is found by its file: `index` of another controller is another view.
    let found = usages(&mut project, SITE, &at("render('index'", "render('ind/*|*/ex'"));
    assert_eq!(found, [target(SITE, 14, "index"), target(VIEW, 0, "")]);
}

#[test]
fn yii_names_are_only_read_in_a_yii_project() {
    let mut project = Project::new(&[("lib/classes.php", FRAMEWORK), (VIEW, "<p></p>\n"), (SITE, CONTROLLER)]);
    let found = project.definition(SITE, &at("render('index'", "render('ind/*|*/ex'"));
    assert!(found.as_ref().is_none_or(Vec::is_empty), "{found:?}");
}

// ------------------------------------------------- what a configuration moves

const MOVED_CONFIG: &str = r#"<?php
return [
    'id' => 'shop',
    'basePath' => dirname(__DIR__),
    'viewPath' => '@app/templates',
    'controllerMap' => [
        'account' => 'app\extra\ProfileController',
    ],
    'components' => [
        'mailer' => ['class' => \app\components\Mailer::class, 'viewPath' => '@app/letters'],
        'view' => ['theme' => ['pathMap' => ['@app/templates' => '@app/themes/dark']]],
        'urlManager' => ['rules' => [
            'posts' => 'post/view-all',
            'p/<id>' => 'post/view',
        ]],
    ],
    'params' => [
        'adminEmail' => 'admin@example.com',
        'mail' => [
            'from' => 'shop@example.com',
            'smtp' => ['host' => 'localhost'],
        ],
    ],
];
"#;

const MOVED_CLASSES: &str = r#"<?php
namespace yii\mail {
    class BaseMailer extends \yii\base\Component {
        public function compose($view = null, array $params = []) {}
    }
}
namespace app\components {
    class Mailer extends \yii\mail\BaseMailer { public function send(): bool { return true; } }
}
"#;

const PROFILE: &str = r#"<?php
namespace app\extra;

class ProfileController extends \yii\web\Controller
{
    public function actions()
    {
        return [
            'captcha' => ['class' => 'x\CaptchaAction'],
            'error' => 'x\ErrorAction',
        ];
    }

    public function actionShow() {}
}
"#;

const MOVED_SITE: &str = r#"<?php
namespace app\controllers;

use Yii;
use yii\helpers\Url;

class SiteController extends \yii\web\Controller
{
    public function behaviors()
    {
        return ['access' => ['only' => ['index'], 'rules' => []]];
    }

    public function actionIndex()
    {
        $from = Yii::$app->params['mail']['from'];
        $host = Yii::$app->params['mail']['smtp']['host'];
        Yii::$app->mailer->compose('hello');
        Yii::t('shop', 'Cart');
        Yii::getAlias('@yii/widgets2/Thing.php');
        Yii::getAlias('@store/a.txt');
        $this->redirect(['account/show']);
        Url::to(['account/captcha']);
        Url::to(['account/error']);
        $this->render('plain');
        return $this->render('index', ['post' => new \app\models\Post()]);
    }
}
"#;

const MIGRATION: &str = r#"<?php
class m240101_000000_create_post extends \yii\db\Migration
{
    public function safeUp()
    {
        $this->createTable('{{%post}}', [
            'id' => $this->primaryKey(),
            'title' => $this->string()->notNull(),
            'body' => $this->text(),
            'rating' => $this->float()->notNull(),
        ]);
        $this->createTable('{{%comment}}', ['id' => $this->primaryKey(), 'text' => $this->text()]);
    }
}
"#;

const COMMENT: &str = r#"<?php
namespace app\models;

class Comment extends \yii\db\ActiveRecord
{
    public function rules()
    {
        return [[['text'], 'required']];
    }
}

class Article extends \yii\db\ActiveRecord
{
    public static function tableName()
    {
        return '{{%post}}';
    }
}
"#;

const THEMED: &str = "themes/dark/site/index.php";

fn moved() -> Project {
    Project::new(&[
        ("composer.json", r#"{"require": {"yiisoft/yii2": "~2.0.50"}}"#),
        ("framework/classes.php", FRAMEWORK),
        ("config/web.php", MOVED_CONFIG),
        ("web/index.php", "<?php\nYii::setAlias('@store', dirname(__DIR__) . '/store');\n"),
        (
            "vendor/yiisoft/extensions.php",
            "<?php\n$vendorDir = dirname(__DIR__);\nreturn [\n  'x/widgets2' => [\n    'name' => 'x/widgets2',\n    'alias' => ['@yii/widgets2' => $vendorDir . '/x/widgets2/src'],\n  ],\n];\n",
        ),
        ("vendor/x/widgets2/src/Thing.php", "<?php\n"),
        ("store/a.txt", "a\n"),
        ("components/classes.php", MOVED_CLASSES),
        ("models/Post.php", MODELS),
        ("models/Comment.php", COMMENT),
        ("extra/ProfileController.php", PROFILE),
        (SITE, MOVED_SITE),
        ("controllers/PostController.php", POSTS),
        ("templates/site/index.php", "<p></p>\n"),
        ("templates/site/plain.php", "<p></p>\n"),
        ("templates/site/_part.php", "<p></p>\n"),
        (THEMED, "<?= $this->render('_part') ?>\n"),
        ("letters/hello.php", "<p></p>\n"),
        (
            "messages/fr/messages.po",
            "msgid \"\"\nmsgstr \"\"\n\"Language: fr\\n\"\n\n#: views/site/index.php\nmsgctxt \"shop\"\nmsgid \"Cart\"\nmsgstr \"Panier\"\n\nmsgid \"Nobody's\"\nmsgstr \"x\"\n",
        ),
        ("migrations/m240101_000000_create_post.php", MIGRATION),
    ])
}

fn moved_at(from: &str, to: &str) -> String {
    assert!(MOVED_SITE.contains(from), "{from}");
    MOVED_SITE.replacen(from, to, 1)
}

#[test]
fn views_are_where_the_configuration_puts_them() {
    let mut project = moved();
    let mut goes = |from: &str, to: &str| project.yii_goes(SITE, &moved_at(from, to));
    let file = |name: &str, line: u32| vec![(name.to_owned(), line)];
    // The `viewPath`; the theme's file where the theme has one.
    assert_eq!(goes("render('plain')", "render('pl/*|*/ain')"), file("templates/site/plain.php", 0));
    assert_eq!(goes("render('index'", "render('ind/*|*/ex'"), file(THEMED, 0));
    // The mailer's `viewPath`.
    assert_eq!(goes("compose('hello')", "compose('he/*|*/llo')"), file("letters/hello.php", 0));
    // A view of the theme names the views of the folder it stands for.
    assert_eq!(project.yii_goes(THEMED, "<?= $this->render('_pa/*|*/rt') ?>\n"), file("templates/site/_part.php", 0));
    let found = project.yii_complete(SITE, &moved_at("render('plain')", "render('/*|*/')"));
    for name in ["index", "plain", "_part"] {
        assert!(found.contains(&name.to_owned()), "{name}: {found:?}");
    }
    // It has what the controller passes, and reports nothing.
    let found = project.yii_complete(THEMED, "<?php echo $post->/*|*/ ?>\n");
    assert!(found.contains(&"title".to_owned()), "{found:?}");
    assert_eq!(project.yii_problems(SITE, MOVED_SITE), Vec::<String>::new());
    let source = moved_at("render('plain')", "render('nothing')");
    assert_eq!(project.yii_problems(SITE, &source), ["Unknown view 'nothing'"]);
}

#[test]
fn mapped_controllers_listed_actions_and_url_rules_are_routes() {
    let mut project = moved();
    let mut goes = |from: &str, to: &str| project.yii_goes(SITE, &moved_at(from, to));
    let file = |name: &str, line: u32| vec![(name.to_owned(), line)];
    // `controllerMap`: the mapped class's action, and the actions `actions()` lists.
    assert_eq!(goes("['account/show']", "['account/sh/*|*/ow']"), file("extra/ProfileController.php", 13));
    assert_eq!(goes("['account/captcha']", "['account/cap/*|*/tcha']"), file("extra/ProfileController.php", 8));
    assert_eq!(goes("['account/error']", "['account/er/*|*/ror']"), file("extra/ProfileController.php", 9));
    // The actions a filter is about.
    assert_eq!(goes("'only' => ['index']", "'only' => ['in/*|*/dex']"), file(SITE, 13));
    // A URL rule's route.
    let rule = MOVED_CONFIG.replacen("'posts' => 'post/view-all'", "'posts' => 'post/view-/*|*/all'", 1);
    assert_eq!(project.yii_goes("config/web.php", &rule), file("controllers/PostController.php", 6));
    let found = project.yii_complete(SITE, &moved_at("['account/show']", "['account//*|*/']"));
    for name in ["account/show", "account/captcha", "account/error"] {
        assert!(found.contains(&name.to_owned()), "{name}: {found:?}");
    }
    // An action neither a method nor listed.
    let source = moved_at("['account/error']", "['account/nope']");
    assert_eq!(project.yii_problems(SITE, &source), ["Controller 'ProfileController' has no action 'nope'"]);
}

#[test]
fn parameters_below_another_and_other_sources_are_names() {
    let mut project = moved();
    let mut goes = |from: &str, to: &str| project.yii_goes(SITE, &moved_at(from, to));
    let file = |name: &str, line: u32| vec![(name.to_owned(), line)];
    assert_eq!(goes("['mail']['from']", "['mail']['fr/*|*/om']"), file("config/web.php", 19));
    assert_eq!(goes("['mail']['from']", "['ma/*|*/il']['from']"), file("config/web.php", 18));
    assert_eq!(goes("['smtp']['host']", "['smtp']['ho/*|*/st']"), file("config/web.php", 20));
    // A gettext catalogue: the context is the category.
    assert_eq!(goes("t('shop', 'Cart')", "t('shop', 'Ca/*|*/rt')"), file("messages/fr/messages.po", 6));
    // An extension's alias, and one an entry script sets.
    assert_eq!(
        goes("'@yii/widgets2/Thing.php'", "'@yii/widgets2/Th/*|*/ing.php'"),
        file("vendor/x/widgets2/src/Thing.php", 0)
    );
    assert_eq!(goes("'@store/a.txt'", "'@store/a./*|*/txt'"), file("store/a.txt", 0));
    let found = project.yii_complete(SITE, &moved_at("['mail']['from']", "['mail']['/*|*/']"));
    assert_eq!(found, ["from", "smtp"]);
    // `params` of a view is the view's own, not the application's.
    let found = project.yii_goes(THEMED, "<?php $this->params['admin/*|*/Email']; ?>\n");
    assert_eq!(found, Vec::<(String, u32)>::new());
}

#[test]
fn a_records_columns_are_its_attributes() {
    let mut project = moved();
    let migration = "migrations/m240101_000000_create_post.php";
    let probe = |body: &str| {
        format!(
            "<?php\nuse app\\models\\Post;\nuse app\\models\\Article;\nfunction probe(Post $post, Article $article) {{\n  {body}\n}}\n"
        )
    };
    let found = project.yii_complete("helpers/probe.php", &probe("$post->/*|*/"));
    for name in ["id", "body", "rating", "title"] {
        assert!(found.contains(&name.to_owned()), "{name}: {found:?}");
    }
    // The table `tableName()` names.
    let found = project.yii_complete("helpers/probe.php", &probe("$article->/*|*/"));
    assert!(found.contains(&"rating".to_owned()), "{found:?}");
    // A column is typed, and goes to its migration.
    let hover = project.yii_hover("helpers/probe.php", &probe("$rating = $post->rating;\n  return $rat/*|*/ing;"));
    assert!(hover.contains("float"), "{hover}");
    let hover = project.yii_hover("helpers/probe.php", &probe("$body = $post->body;\n  return $bo/*|*/dy;"));
    assert!(hover.contains("string|null") || hover.contains("?string"), "{hover}");
    assert_eq!(project.yii_goes("helpers/probe.php", &probe("$post->bo/*|*/dy;")), [(migration.to_owned(), 8)]);
    // An attribute of a rule that is a column.
    let rules = COMMENT.replacen("[['text'], 'required']", "[['te/*|*/xt'], 'required']", 1);
    assert_eq!(project.yii_goes("models/Comment.php", &rules), [(migration.to_owned(), 11)]);
    // A configuration array names the class it makes.
    let made = probe("\\Yii::createObject(['class' => \\app\\components\\Mailer::class, 'x' => 1])->/*|*/");
    let mut found = project.yii_complete("helpers/probe.php", &made);
    found.sort();
    assert_eq!(found, ["compose", "send"]);
}

/// The project's model is read from its configuration: typing in a file it never read
/// keeps it, and typing in one it read has it read again.
#[test]
fn the_project_is_read_again_only_when_a_file_it_read_changes() {
    let mut project = yii();
    let params = |project: &Project| -> Vec<String> {
        project.engine.yii().params.iter().map(|param| param.name.clone()).collect()
    };
    assert_eq!(params(&project), ["adminEmail"]);

    // A controller's body: the configuration didn't read it. (Opening a document moves
    // the modification count; typing in it doesn't, while its declarations stay.)
    let controller = project.open(SITE, CONTROLLER);
    let first = project.engine.yii();
    project.engine.document_changed(
        controller,
        2,
        &CONTROLLER.replace("class SiteController", "class  SiteController"),
    );
    assert!(Arc::ptr_eq(&first, &project.engine.yii()), "an edit elsewhere read the project again");

    // The parameters file, as the editor has it.
    let file = project.open("config/params.php", "<?php\nreturn ['adminEmail' => 'admin@example.com'];\n");
    project.engine.document_changed(file, 2, "<?php\nreturn ['adminEmail' => 'a@example.com', 'pageSize' => 20];\n");
    assert_eq!(params(&project), ["adminEmail", "pageSize"]);
    // And closed again without saving: the file on disk is what counts.
    project.engine.document_closed(file);
    assert_eq!(params(&project), ["adminEmail"]);
}
