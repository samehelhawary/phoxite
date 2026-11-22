//! Laminas MVC and Mezzio, on a stand-in for their API written for these
//! tests (the names of their classes and methods, nothing of their code)
//! and two small applications.

use super::*;

/// The frameworks' classes.
const FRAMEWORK: &str = r#"<?php
namespace Psr\Container {
    interface ContainerInterface {
        public function get(string $id);
        public function has(string $id): bool;
    }
}
namespace Laminas\ServiceManager {
    interface ServiceLocatorInterface extends \Psr\Container\ContainerInterface {
        public function build(string $name, ?array $options = null);
    }
}
namespace Laminas\ServiceManager\Factory {
    class InvokableFactory {}
}
namespace Laminas\Mvc\Controller {
    abstract class AbstractController {
        public function plugin($name, ?array $options = null) {}
        public function __call($method, $params) {}
    }
    abstract class AbstractActionController extends AbstractController {}
    class PluginManager {
        protected $aliases = ['url' => Plugin\Url::class, 'redirect' => Plugin\Redirect::class];
    }
}
namespace Laminas\Mvc\Controller\Plugin {
    class Url {
        public function fromRoute($route = null, $params = [], $options = []) {}
    }
    class Redirect {
        public function toRoute($route = null, $params = [], $options = []) {}
    }
}
namespace Laminas\View\Model {
    class ViewModel {
        public function __construct($variables = null, $options = null) {}
        public function setTemplate(string $template) {}
        public function setVariable(string $name, $value) {}
    }
}
namespace Laminas\View\Renderer {
    class PhpRenderer {
        public function plugin($name, ?array $options = null) {}
        public function __call($method, $argv) {}
        public function render($nameOrModel, $values = null) {}
    }
}
namespace Laminas\View {
    class HelperPluginManager {
        protected $aliases = ['url' => Helper\Url::class, 'partial' => Helper\Partial::class];
    }
}
namespace Laminas\View\Helper {
    class Url {
        public function __invoke($name = null, $params = [], $options = []): string {}
    }
    class Partial {
        public function __invoke($name = null, $values = null): string {}
    }
}
namespace Mezzio {
    class Application {
        public function get(string $path, $middleware, ?string $name = null) {}
        public function route(string $path, $middleware, ?array $methods = null, ?string $name = null) {}
    }
}
namespace Mezzio\Template {
    interface TemplateRendererInterface {
        public function render(string $name, $params = []): string;
    }
}
namespace Mezzio\Helper {
    class UrlHelper {
        public function generate(?string $routeName = null, array $routeParams = []): string {}
    }
}
"#;

const MODULE: &str = "module/Application/config/module.config.php";

const MODULE_CONFIG: &str = r#"<?php
namespace Application;

use Laminas\ServiceManager\Factory\InvokableFactory;

return [
    'router' => [
        'routes' => [
            'home' => [
                'type' => 'Literal',
                'options' => ['route' => '/', 'defaults' => ['controller' => Controller\IndexController::class, 'action' => 'index']],
            ],
            'blog' => [
                'type' => 'Literal',
                'options' => ['route' => '/blog', 'defaults' => ['controller' => Controller\BlogController::class, 'action' => 'list']],
                'child_routes' => [
                    'post' => ['type' => 'Segment', 'options' => ['route' => '/:id', 'defaults' => ['action' => 'show']]],
                ],
            ],
        ],
    ],
    'controllers' => ['factories' => [Controller\IndexController::class => InvokableFactory::class]],
    'service_manager' => [
        'aliases' => ['mailer' => Service\Mailer::class],
        'factories' => [Service\Mailer::class => Service\MailerFactory::class],
        'invokables' => ['stamp' => Service\Stamp::class],
    ],
    'controller_plugins' => ['invokables' => ['greet' => Plugin\Greet::class]],
    'view_helpers' => ['invokables' => ['price' => Helper\Price::class]],
    'view_manager' => [
        'template_map' => ['layout/layout' => __DIR__ . '/../view/layout/layout.phtml'],
        'template_path_stack' => [__DIR__ . '/../view'],
    ],
];
"#;

const CLASSES: &str = r#"<?php
namespace Application\Service {
    class Mailer {
        public function send(): bool { return true; }
    }
    class MailerFactory {
        public function __invoke($container): Mailer { return new Mailer(); }
    }
    class Stamp {
        public function now(): int { return 0; }
    }
}
namespace Application\Plugin {
    class Greet {
        public function __invoke(string $name): string { return $name; }
        public function shout(): string { return ''; }
    }
}
namespace Application\Helper {
    class Price {
        public function __invoke(float $amount): string { return ''; }
    }
}
namespace Application\Model {
    class Post {
        public string $title = '';
    }
}
"#;

const INDEX: &str = "module/Application/src/Controller/IndexController.php";

const CONTROLLER: &str = r#"<?php
namespace Application\Controller;

use Laminas\Mvc\Controller\AbstractActionController;
use Laminas\View\Model\ViewModel;
use Psr\Container\ContainerInterface;

class IndexController extends AbstractActionController
{
    public function __construct(private ContainerInterface $container) {}

    public function indexAction()
    {
        $mailer = $this->container->get('mailer');
        $host = $this->container->get('config')['db']['host'];
        $this->greet('you');
        return new ViewModel(['post' => new \Application\Model\Post(), 'count' => 3]);
    }

    public function listAction()
    {
        $view = new ViewModel();
        $view->setVariable('total', 2);
        $view->setTemplate('application/index/special');
        $this->redirect()->toRoute('blog/post', ['action' => 'show']);
        return $view;
    }

    public function goAction()
    {
        return $this->url()->fromRoute('home');
    }
}
"#;

const BLOG: &str = "module/Application/src/Controller/BlogController.php";

const BLOG_CONTROLLER: &str = r#"<?php
namespace Application\Controller;

class BlogController extends \Laminas\Mvc\Controller\AbstractActionController
{
    public function listAction() {}
    public function showAction() {}
}
"#;

const VIEW: &str = "module/Application/view/application/index/index.phtml";

const VIEW_TEMPLATE: &str = "<a href=\"<?= $this->url('home') ?>\">Home</a>\n<?= $this->partial('application/index/row', ['label' => 'x']) ?>\n<?= $this->price(2.0) ?>\n";

fn mvc() -> Project {
    Project::new(&[
        ("composer.json", r#"{"require": {"laminas/laminas-mvc": "^3.8"}}"#),
        ("framework/classes.php", FRAMEWORK),
        ("config/modules.config.php", "<?php\nreturn ['Application'];\n"),
        (
            "config/autoload/global.php",
            "<?php\nreturn [\n    'db' => [\n        'host' => 'localhost',\n        'port' => 3306,\n    ],\n];\n",
        ),
        (MODULE, MODULE_CONFIG),
        ("module/Application/src/classes.php", CLASSES),
        (INDEX, CONTROLLER),
        (BLOG, BLOG_CONTROLLER),
        (VIEW, VIEW_TEMPLATE),
        ("module/Application/view/application/index/special.phtml", "<p></p>\n"),
        ("module/Application/view/application/index/row.phtml", "<p></p>\n"),
        ("module/Application/view/layout/layout.phtml", "<html></html>\n"),
    ])
}

fn at(from: &str, to: &str) -> String {
    assert!(CONTROLLER.contains(from), "{from}");
    CONTROLLER.replacen(from, to, 1)
}

impl Project {
    fn laminas_goes(&mut self, file: &str, source: &str) -> Vec<(String, u32)> {
        let found = self.definition(file, source).unwrap_or_default();
        found.into_iter().map(|(file, line, _)| (file, line)).collect()
    }

    fn laminas_complete(&mut self, file: &str, source: &str) -> Vec<String> {
        let (text, pos) = caret(source);
        let doc = self.open(file, &text);
        let found = self.engine.completion(doc, pos, None, &CancellationToken::new()).expect("handled");
        self.engine.document_closed(doc);
        found.items.into_iter().map(|item| item.label).collect()
    }

    fn laminas_hover(&mut self, file: &str, source: &str) -> String {
        let (text, pos) = caret(source);
        let doc = self.open(file, &text);
        let hover = self.engine.hover(doc, pos);
        self.engine.document_closed(doc);
        hover.map(|hover| hover.markdown).unwrap_or_default()
    }

    fn laminas_problems(&mut self, file: &str, source: &str) -> Vec<String> {
        let doc = self.open(file, source);
        let inspected = self.engine.inspect(doc, &CancellationToken::new()).expect("inspected");
        self.engine.document_closed(doc);
        let found = inspected.problems.iter().filter(|problem| problem.inspection.contains("Laminas"));
        found.map(|problem| problem.message.clone()).collect()
    }

    /// The members offered after `body`, a statement of an action of the
    /// index controller.
    fn laminas_members(&mut self, body: &str) -> Vec<String> {
        let source = at("$this->greet('you');", body);
        let mut found = self.laminas_complete(INDEX, &source);
        found.sort();
        found
    }
}

fn has(found: &[String], names: &[&str]) {
    for name in names {
        assert!(found.contains(&(*name).to_owned()), "{name}: {found:?}");
    }
}

#[test]
fn the_names_of_an_mvc_application_go_to_what_they_name() {
    let mut project = mvc();
    assert!(project.engine.frameworks().contains(crate::frameworks::Framework::Laminas));
    let mut goes = |from: &str, to: &str| project.laminas_goes(INDEX, &at(from, to));
    let file = |name: &str, line: u32| vec![(name.to_owned(), line)];
    // A service, a key of the configuration below another.
    assert_eq!(goes("get('mailer')", "get('mai/*|*/ler')"), file(MODULE, 23));
    assert_eq!(goes("['db']['host']", "['db']['ho/*|*/st']"), file("config/autoload/global.php", 3));
    assert_eq!(goes("['db']['host']", "['d/*|*/b']['host']"), file("config/autoload/global.php", 2));
    // Routes, a child route; the action of the route a URL is of.
    assert_eq!(goes("fromRoute('home')", "fromRoute('ho/*|*/me')"), file(MODULE, 8));
    assert_eq!(goes("toRoute('blog/post'", "toRoute('blog/po/*|*/st'"), file(MODULE, 16));
    assert_eq!(goes("['action' => 'show']", "['action' => 'sh/*|*/ow']"), file(BLOG, 6));
    // A template the action names.
    assert_eq!(
        goes("setTemplate('application/index/special')", "setTemplate('application/index/spe/*|*/cial')"),
        file("module/Application/view/application/index/special.phtml", 0)
    );

    // In the configuration: an action of the controller beside it, a template a map names.
    let found =
        project.laminas_goes(MODULE, &MODULE_CONFIG.replacen("'action' => 'index'", "'action' => 'in/*|*/dex'", 1));
    assert_eq!(found, file(INDEX, 11));
    let found =
        project.laminas_goes(MODULE, &MODULE_CONFIG.replacen("['layout/layout' =>", "['layout/lay/*|*/out' =>", 1));
    assert_eq!(found, file("module/Application/view/layout/layout.phtml", 0));
    // In a template: a route, a partial, a helper.
    let mut goes = |from: &str, to: &str| project.laminas_goes(VIEW, &VIEW_TEMPLATE.replacen(from, to, 1));
    assert_eq!(goes("url('home')", "url('ho/*|*/me')"), file(MODULE, 8));
    assert_eq!(
        goes("partial('application/index/row'", "partial('application/index/r/*|*/ow'"),
        file("module/Application/view/application/index/row.phtml", 0)
    );
    assert_eq!(goes("$this->price(2.0)", "$this->pri/*|*/ce(2.0)"), file("module/Application/src/classes.php", 20));

    let hover = project.laminas_hover(INDEX, &at("fromRoute('home')", "fromRoute('ho/*|*/me')"));
    assert!(hover.contains("**route** `home`") && hover.contains("IndexController::indexAction()"), "{hover}");
    let hover = project.laminas_hover(INDEX, &at("['db']['host']", "['db']['ho/*|*/st']"));
    assert!(hover.contains("'localhost'"), "{hover}");
}

#[test]
fn laminas_names_complete_by_what_is_asked_for() {
    let mut project = mvc();
    let mut offered = |from: &str, to: &str| project.laminas_complete(INDEX, &at(from, to));
    has(&offered("get('mailer')", "get('/*|*/')"), &["mailer", "stamp", "Application\\Service\\Mailer"]);
    assert_eq!(offered("fromRoute('home')", "fromRoute('/*|*/')"), ["blog", "blog/post", "home"]);
    assert_eq!(offered("['db']['host']", "['db']['/*|*/']"), ["host", "port"]);
    assert_eq!(offered("['action' => 'show']", "['action' => '/*|*/']"), ["list", "show"]);
    has(
        &offered("setTemplate('application/index/special')", "setTemplate('/*|*/')"),
        &["layout/layout", "application/index/index", "application/index/special"],
    );
}

#[test]
fn laminas_names_that_name_nothing_are_reported() {
    let mut project = mvc();
    assert_eq!(project.laminas_problems(INDEX, CONTROLLER), Vec::<String>::new());
    assert_eq!(project.laminas_problems(MODULE, MODULE_CONFIG), Vec::<String>::new());
    assert_eq!(project.laminas_problems(VIEW, VIEW_TEMPLATE), Vec::<String>::new());
    let source = at("get('mailer')", "get('nothing')")
        .replacen("setTemplate('application/index/special')", "setTemplate('application/index/nope')", 1)
        .replacen("toRoute('blog/post', ['action' => 'show'])", "toRoute('blog/post', ['action' => 'gone'])", 1)
        .replacen("fromRoute('home')", "fromRoute('nowhere')", 1);
    assert_eq!(
        project.laminas_problems(INDEX, &source),
        [
            "Unknown service 'nothing'",
            "Unknown template 'application/index/nope'",
            "Controller 'BlogController' has no action 'gone'",
            "Unknown route 'nowhere'",
        ]
    );
    let source = VIEW_TEMPLATE.replacen("url('home')", "url('nowhere')", 1);
    assert_eq!(project.laminas_problems(VIEW, &source), ["Unknown route 'nowhere'"]);
}

#[test]
fn services_plugins_and_helpers_are_typed() {
    let mut project = mvc();
    // A service: an alias to a class a factory makes, an invokable, a class's name.
    assert_eq!(project.laminas_members("$this->container->get('mailer')->/*|*/;"), ["send"]);
    assert_eq!(project.laminas_members("$this->container->get('stamp')->/*|*/;"), ["now"]);
    assert_eq!(
        project.laminas_members("$this->container->get(\\Application\\Service\\Mailer::class)->/*|*/;"),
        ["send"]
    );
    let hover = project.laminas_hover(
        INDEX,
        &at("$this->greet('you');", "$config = $this->container->get('config'); echo $con/*|*/fig;"),
    );
    assert!(hover.contains("array"), "{hover}");
    // A controller's plugins: what calling one gives, the plugin itself, Laminas's own.
    let hover = project
        .laminas_hover(INDEX, &at("$this->greet('you');", "$greeting = $this->greet('you'); echo $gree/*|*/ting;"));
    assert!(hover.contains("string"), "{hover}");
    has(&project.laminas_members("$this->plugin('greet')->/*|*/;"), &["shout"]);
    assert_eq!(project.laminas_members("$this->redirect()->/*|*/;"), ["toRoute"]);
    has(&project.laminas_members("$this->/*|*/;"), &["greet", "url", "redirect", "indexAction"]);
    // A plugin goes to its class.
    let found = project.laminas_goes(INDEX, &at("$this->greet('you');", "$this->gre/*|*/et('you');"));
    assert_eq!(found, [("module/Application/src/classes.php".to_owned(), 14)]);
}

#[test]
fn an_mvc_template_has_what_its_action_gives_it() {
    let mut project = mvc();
    // What the view model is made with; `$this` is the renderer, with its helpers.
    has(&project.laminas_complete(VIEW, "<?php echo $post->/*|*/ ?>\n"), &["title"]);
    let hover = project.laminas_hover(VIEW, "<?php $x = $count; echo $/*|*/x; ?>\n");
    assert!(hover.contains("int"), "{hover}");
    has(&project.laminas_complete(VIEW, "<?php $this->/*|*/ ?>\n"), &["price", "url", "partial", "plugin"]);
    let hover = project.laminas_hover(VIEW, "<?php $x = $this->price(1.0); echo $/*|*/x; ?>\n");
    assert!(hover.contains("string"), "{hover}");
    // A template an action names, a partial.
    let special = "module/Application/view/application/index/special.phtml";
    let hover = project.laminas_hover(special, "<?php $x = $total; echo $/*|*/x; ?>\n");
    assert!(hover.contains("int"), "{hover}");
    let row = "module/Application/view/application/index/row.phtml";
    let hover = project.laminas_hover(row, "<?php $x = $label; echo $/*|*/x; ?>\n");
    assert!(hover.contains("string"), "{hover}");
}

// --------------------------------------------------------------------- Mezzio

const HANDLER: &str = "src/App/src/Handler/HomePageHandler.php";

const HOME_HANDLER: &str = r#"<?php
namespace App\Handler;

use Mezzio\Helper\UrlHelper;
use Mezzio\Template\TemplateRendererInterface;

class HomePageHandler
{
    public function __construct(private TemplateRendererInterface $template, private UrlHelper $url) {}

    public function handle()
    {
        $link = $this->url->generate('api.ping');
        return $this->template->render('app::home-page', ['name' => 'World', 'visits' => 3]);
    }
}
"#;

const ROUTES: &str = r#"<?php
use App\Handler\HomePageHandler;
use App\Handler\PingHandler;

return static function (\Mezzio\Application $app): void {
    $app->get('/', HomePageHandler::class, 'home');
    $app->get('/api/ping', PingHandler::class, 'api.ping');
};
"#;

const PROVIDER: &str = r#"<?php
namespace App;

class ConfigProvider
{
    public function __invoke(): array
    {
        return [
            'dependencies' => $this->getDependencies(),
            'templates' => $this->getTemplates(),
        ];
    }

    public function getDependencies(): array
    {
        return [
            'invokables' => [Handler\PingHandler::class => Handler\PingHandler::class],
            'factories' => [Handler\HomePageHandler::class => Handler\HomePageHandlerFactory::class],
        ];
    }

    public function getTemplates(): array
    {
        return ['paths' => ['app' => [__DIR__ . '/../templates/app']]];
    }
}
"#;

const PAGE: &str = "src/App/templates/app/home-page.phtml";

fn mezzio() -> Project {
    Project::new(&[
        ("composer.json", r#"{"require": {"mezzio/mezzio": "^3.29"}}"#),
        ("framework/classes.php", FRAMEWORK),
        (
            "config/config.php",
            "<?php\n$aggregator = new ConfigAggregator([\n    App\\ConfigProvider::class,\n]);\nreturn $aggregator->getMergedConfig();\n",
        ),
        ("config/routes.php", ROUTES),
        ("src/App/src/ConfigProvider.php", PROVIDER),
        (HANDLER, HOME_HANDLER),
        ("src/App/src/Handler/PingHandler.php", "<?php\nnamespace App\\Handler;\nclass PingHandler {}\n"),
        (PAGE, "<h1></h1>\n"),
        ("src/App/templates/app/about.phtml", "<p></p>\n"),
    ])
}

#[test]
fn a_mezzio_application_names_its_routes_templates_and_services() {
    let mut project = mezzio();
    assert!(project.engine.frameworks().contains(crate::frameworks::Framework::Laminas));
    let file = |name: &str, line: u32| vec![(name.to_owned(), line)];
    let found =
        project.laminas_goes(HANDLER, &HOME_HANDLER.replacen("generate('api.ping')", "generate('api.pi/*|*/ng')", 1));
    assert_eq!(found, file("config/routes.php", 6));
    let found = project
        .laminas_goes(HANDLER, &HOME_HANDLER.replacen("render('app::home-page'", "render('app::home-/*|*/page'", 1));
    assert_eq!(found, file(PAGE, 0));
    let found =
        project.laminas_complete(HANDLER, &HOME_HANDLER.replacen("generate('api.ping')", "generate('/*|*/')", 1));
    assert_eq!(found, ["api.ping", "home"]);
    let found =
        project.laminas_complete(HANDLER, &HOME_HANDLER.replacen("render('app::home-page'", "render('/*|*/'", 1));
    assert_eq!(found, ["app::about", "app::home-page"]);
    // Nothing is reported, where a route is named either; a name that names nothing is.
    assert_eq!(project.laminas_problems(HANDLER, HOME_HANDLER), Vec::<String>::new());
    assert_eq!(project.laminas_problems("config/routes.php", ROUTES), Vec::<String>::new());
    let source = HOME_HANDLER.replacen("generate('api.ping')", "generate('nowhere')", 1).replacen(
        "render('app::home-page'",
        "render('app::nope'",
        1,
    );
    assert_eq!(project.laminas_problems(HANDLER, &source), ["Unknown route 'nowhere'", "Unknown template 'app::nope'"]);
    // The template has what the handler passes.
    let hover = project.laminas_hover(PAGE, "<?php $x = $visits; echo $/*|*/x; ?>\n");
    assert!(hover.contains("int"), "{hover}");
    let hover = project.laminas_hover(PAGE, "<?php $x = $name; echo $/*|*/x; ?>\n");
    assert!(hover.contains("string"), "{hover}");
}

#[test]
fn laminas_names_are_only_read_in_a_laminas_project() {
    let mut project = Project::new(&[
        ("framework/classes.php", FRAMEWORK),
        ("module/Application/src/classes.php", CLASSES),
        (INDEX, CONTROLLER),
    ]);
    assert!(!project.engine.frameworks().contains(crate::frameworks::Framework::Laminas));
    let source = at("fromRoute('home')", "fromRoute('nowhere')");
    assert_eq!(project.laminas_problems(INDEX, &source), Vec::<String>::new());
    assert!(project.laminas_goes(INDEX, &at("get('mailer')", "get('mai/*|*/ler')")).is_empty());
}

// ------------------------------------------- the rest of what a project says

/// More of the frameworks' classes: the other plugin managers.
const MORE_FRAMEWORK: &str = r#"<?php
namespace Laminas\ServiceManager {
    class AbstractPluginManager implements ServiceLocatorInterface {
        public function get(string $id) {}
        public function has(string $id): bool {}
        public function build(string $name, ?array $options = null) {}
    }
}
namespace Laminas\Validator {
    class ValidatorPluginManager extends \Laminas\ServiceManager\AbstractPluginManager {
        protected $aliases = [
            'stringlength' => StringLength::class,
            'stringLength' => StringLength::class,
            'StringLength' => StringLength::class,
        ];
    }
    class StringLength {
        public function setMax(int $max) {}
    }
}
namespace Laminas\Filter {
    class FilterPluginManager extends \Laminas\ServiceManager\AbstractPluginManager {
        protected $aliases = ['stringtrim' => StringTrim::class, 'StringTrim' => StringTrim::class];
    }
    class StringTrim {
        public function filter($value) {}
    }
}
namespace Laminas\Form {
    class FormElementManager extends \Laminas\ServiceManager\AbstractPluginManager {
        protected $aliases = ['text' => Element\Text::class, 'Text' => Element\Text::class];
    }
    class Fieldset {
        public function add($elementOrFieldset, array $flags = []) {}
    }
    class Form extends Fieldset {}
}
namespace Laminas\Form\Element {
    class Text {
        public function setLabel($label) {}
    }
}
"#;

const MORE_MODULE: &str = r#"<?php
namespace Application;

return [
    'router' => [
        'routes' => [
            'post' => [
                'type' => 'Segment',
                'options' => ['route' => '/post/:id[/:slug]', 'defaults' => ['controller' => Controller\PostController::class, 'action' => 'show']],
                'child_routes' => [
                    'edit' => ['type' => 'Segment', 'options' => ['route' => '/edit/:tab', 'defaults' => ['action' => 'show']]],
                ],
            ],
        ],
    ],
    'service_manager' => [
        'factories' => [
            'clock' => function ($container): Service\Clock { return $container->get('x'); },
            'mailer' => fn () => new Service\Mailer(),
            Service\Mailer::class => Service\MailerFactory::class,
        ],
        'delegators' => [
            Service\Mailer::class => [Service\LoggingDelegator::class],
        ],
    ],
    'validators' => ['invokables' => ['Isbn' => Validator\Isbn::class]],
    'form_elements' => ['invokables' => ['rating' => Form\Rating::class]],
    'navigation' => [
        'default' => [['label' => 'Home', 'route' => 'post']],
        'footer' => [],
    ],
    'view_manager' => [
        'layout' => 'layout/main',
        'template_path_stack' => [__DIR__ . '/../view'],
    ],
];
"#;

const MORE_CLASSES: &str = r#"<?php
namespace Application {
    class Module {
        public function getConfig() {
            return include __DIR__ . '/../config/module.config.php';
        }
        public function getServiceConfig() {
            return ['invokables' => ['stamp' => Service\Stamp::class]];
        }
        public function getViewHelperConfig() {
            return ['invokables' => ['badge' => Helper\Badge::class]];
        }
    }
}
namespace Application\Service {
    class Clock {
        public function year(): int { return 0; }
    }
    class Mailer {
        public function send(): bool { return true; }
    }
    class LoggingMailer extends Mailer {
        public function log(): void {}
    }
    class MailerFactory {
        public function __invoke($container): Mailer { return new Mailer(); }
    }
    class LoggingDelegator {
        public function __invoke($container, $name, callable $callback): LoggingMailer { return new LoggingMailer(); }
    }
    class Stamp {
        public function now(): int { return 0; }
    }
}
namespace Application\Helper {
    class Badge {
        public function __invoke(): string { return ''; }
    }
}
namespace Application\Validator {
    class Isbn {
        public function isValid($value): bool { return true; }
    }
}
namespace Application\Form {
    class Rating {
        public function setStars(int $stars) {}
    }
}
namespace Application\Model {
    class Post {
        public string $title = '';
    }
}
"#;

const POST: &str = "module/Application/src/Controller/PostController.php";

const POST_CONTROLLER: &str = r#"<?php
namespace Application\Controller;

use Laminas\Mvc\Controller\AbstractActionController;
use Laminas\View\Model\ViewModel;
use Psr\Container\ContainerInterface;

class PostController extends AbstractActionController
{
    public function __construct(
        private ContainerInterface $container,
        private \Laminas\Validator\ValidatorPluginManager $validators,
        private \Laminas\Form\FormElementManager $elements,
    ) {}

    public function showAction()
    {
        $config = $this->container->get('config');
        $layout = $config['view_manager']['layout'];
        $this->probe();
        $this->layout()->setVariable('title', 'Posts');
        $this->layout()->year = 2026;
        $sidebar = new ViewModel(['links' => 3]);
        $sidebar->setTemplate('application/post/sidebar');
        $view = new ViewModel(['post' => new \Application\Model\Post()]);
        $view->addChild($sidebar, 'sidebar');
        $this->url()->fromRoute('post/edit', ['id' => 1, 'tab' => 'meta']);
        return $view;
    }
}
"#;

const SPECS: &str = "module/Application/src/specs.php";

const SPEC_FILE: &str = r#"<?php
namespace Application;

$inputs = [
    'title' => [
        'validators' => [['name' => 'StringLength', 'options' => ['max' => 10]], ['name' => 'Isbn']],
        'filters' => [['name' => 'StringTrim']],
    ],
];
$form = new \Laminas\Form\Form();
$form->add(['type' => 'text', 'name' => 'title']);
$form->add(['type' => 'rating', 'name' => 'stars']);
"#;

const MAIN: &str = "module/Application/view/layout/main.phtml";

fn mvc_with_more() -> Project {
    Project::new(&[
        ("composer.json", r#"{"require": {"laminas/laminas-mvc": "^3.8"}}"#),
        ("framework/classes.php", FRAMEWORK),
        ("framework/more.php", MORE_FRAMEWORK),
        ("config/modules.config.php", "<?php\nreturn ['Application'];\n"),
        (MODULE, MORE_MODULE),
        ("module/Application/src/Module.php", MORE_CLASSES),
        (POST, POST_CONTROLLER),
        (SPECS, SPEC_FILE),
        ("module/Application/view/application/post/show.phtml", "<p></p>\n"),
        ("module/Application/view/application/post/sidebar.phtml", "<p></p>\n"),
        (MAIN, "<?= $this->navigation('navigation') ?>\n"),
    ])
}

fn post_at(from: &str, to: &str) -> String {
    assert!(POST_CONTROLLER.contains(from), "{from}");
    POST_CONTROLLER.replacen(from, to, 1)
}

impl Project {
    /// The members offered after `body`, a statement of the post controller's action.
    fn post_members(&mut self, body: &str) -> Vec<String> {
        let mut found = self.laminas_complete(POST, &post_at("$this->probe();", body));
        found.sort();
        found
    }
}

#[test]
fn a_configuration_held_in_a_variable_has_its_keys() {
    let mut project = mvc_with_more();
    let file = |name: &str, line: u32| vec![(name.to_owned(), line)];
    let found = project.laminas_goes(POST, &post_at("['view_manager']['layout']", "['view_manager']['lay/*|*/out']"));
    assert_eq!(found, file(MODULE, 32));
    let found = project.laminas_goes(POST, &post_at("['view_manager']['layout']", "['view_/*|*/manager']['layout']"));
    assert_eq!(found, file(MODULE, 31));
    let found = project.laminas_complete(POST, &post_at("['view_manager']['layout']", "['view_manager']['/*|*/']"));
    assert_eq!(found, ["layout", "template_path_stack"]);
    // A variable assigned something else is no configuration.
    let source = post_at("$config = $this->container->get('config');", "$config = $this->container->get('clock');");
    let found = project
        .laminas_goes(POST, &source.replacen("['view_manager']['layout']", "['view_manager']['lay/*|*/out']", 1));
    assert_eq!(found, Vec::<(String, u32)>::new());
}

#[test]
fn the_other_plugin_managers_have_their_services() {
    let mut project = mvc_with_more();
    let file = |name: &str, line: u32| vec![(name.to_owned(), line)];
    // Types: a manager's own service, Laminas's and the configuration's.
    assert_eq!(project.post_members("$this->validators->get('StringLength')->/*|*/;"), ["setMax"]);
    assert_eq!(project.post_members("$this->validators->get('Isbn')->/*|*/;"), ["isValid"]);
    assert_eq!(project.post_members("$this->elements->get('rating')->/*|*/;"), ["setStars"]);
    // Names: a validator and a filter of a specification, a form element, a manager's service.
    let mut goes = |from: &str, to: &str| project.laminas_goes(SPECS, &SPEC_FILE.replacen(from, to, 1));
    assert_eq!(goes("['name' => 'Isbn']", "['name' => 'Is/*|*/bn']"), file("module/Application/src/Module.php", 40));
    assert_eq!(goes("['name' => 'StringLength'", "['name' => 'String/*|*/Length'")[0].0, "framework/more.php");
    assert_eq!(goes("['name' => 'StringTrim']", "['name' => 'String/*|*/Trim']")[0].0, "framework/more.php");
    assert_eq!(goes("['type' => 'text',", "['type' => 'te/*|*/xt',")[0].0, "framework/more.php");
    assert_eq!(
        goes("['type' => 'rating',", "['type' => 'rat/*|*/ing',"),
        file("module/Application/src/Module.php", 45)
    );
    let found = project.laminas_goes(POST, &post_at("$this->probe();", "$this->validators->get('Is/*|*/bn');"));
    assert_eq!(found, file("module/Application/src/Module.php", 40));
    // A name is offered once, as a class is written.
    let found = project.laminas_complete(SPECS, &SPEC_FILE.replacen("['name' => 'Isbn']", "['name' => '/*|*/']", 1));
    assert_eq!(found, ["Isbn", "StringLength"]);
    // A navigation container of the configuration.
    let found = project.laminas_goes(MAIN, "<?= $this->navigation('navi/*|*/gation') ?>\n");
    assert_eq!(found, file(MODULE, 28));
    let found = project.laminas_complete(MAIN, "<?= $this->navigation('/*|*/') ?>\n");
    assert_eq!(found, ["default", "footer", "navigation"]);
}

#[test]
fn delegators_closures_and_a_modules_methods_say_what_a_service_is() {
    let mut project = mvc_with_more();
    // What the last delegator wraps the service in.
    let found = project.post_members("$this->container->get(\\Application\\Service\\Mailer::class)->/*|*/;");
    assert_eq!(found, ["log", "send"]);
    // A factory written in place: what it is declared to return, what it makes.
    assert_eq!(project.post_members("$this->container->get('clock')->/*|*/;"), ["year"]);
    assert_eq!(project.post_members("$this->container->get('mailer')->/*|*/;"), ["send"]);
    // What a `Module` returns from `getServiceConfig()` and `getViewHelperConfig()`.
    assert_eq!(project.post_members("$this->container->get('stamp')->/*|*/;"), ["now"]);
    let found = project.laminas_complete(MAIN, "<?php $this->/*|*/ ?>\n");
    has(&found, &["badge", "url"]);
    let found = project.laminas_goes(POST, &post_at("$this->probe();", "$this->container->get('sta/*|*/mp');"));
    assert_eq!(found, [("module/Application/src/Module.php".to_owned(), 7)]);
}

#[test]
fn a_layout_and_a_child_view_model_have_their_own_variables() {
    let mut project = mvc_with_more();
    let show = "module/Application/view/application/post/show.phtml";
    has(&project.laminas_complete(show, "<?php echo $post->/*|*/ ?>\n"), &["title"]);
    // The child's variables are its template's, not the action's.
    let hover = project.laminas_hover(show, "<?php $x = $links; echo $/*|*/x; ?>\n");
    assert!(!hover.contains("int"), "{hover}");
    let sidebar = "module/Application/view/application/post/sidebar.phtml";
    let hover = project.laminas_hover(sidebar, "<?php $x = $links; echo $/*|*/x; ?>\n");
    assert!(hover.contains("int"), "{hover}");
    // The layout: what a controller sets on it, by a call and by a property.
    let hover = project.laminas_hover(MAIN, "<?php $x = $title; echo $/*|*/x; ?>\n");
    assert!(hover.contains("string"), "{hover}");
    let hover = project.laminas_hover(MAIN, "<?php $x = $year; echo $/*|*/x; ?>\n");
    assert!(hover.contains("int"), "{hover}");
}

#[test]
fn a_routes_parameters_are_names() {
    let mut project = mvc_with_more();
    let file = |name: &str, line: u32| vec![(name.to_owned(), line)];
    let found =
        project.laminas_goes(POST, &post_at("['id' => 1, 'tab' => 'meta']", "['id' => 1, 'ta/*|*/b' => 'meta']"));
    assert_eq!(found, file(MODULE, 10));
    // A child route has its parents' parameters.
    let found = project.laminas_complete(POST, &post_at("['id' => 1, 'tab' => 'meta']", "['/*|*/' => 1]"));
    assert_eq!(found, ["id", "slug", "tab"]);
    let hover =
        project.laminas_hover(POST, &post_at("['id' => 1, 'tab' => 'meta']", "['id' => 1, 'ta/*|*/b' => 'meta']"));
    assert!(hover.contains("**route parameter** `tab`") && hover.contains("/edit/:tab"), "{hover}");
}

#[test]
fn zend_frameworks_names_are_laminass() {
    let framework = FRAMEWORK.replace("namespace Laminas\\", "namespace Zend\\");
    let controller = CONTROLLER.replace("use Laminas\\", "use Zend\\");
    let module = MODULE_CONFIG.replace("use Laminas\\", "use Zend\\");
    let mut project = Project::new(&[
        ("composer.json", r#"{"require": {"zendframework/zend-mvc": "^3.1"}}"#),
        ("framework/classes.php", &framework),
        ("config/modules.config.php", "<?php\nreturn ['Application'];\n"),
        ("config/autoload/global.php", "<?php\nreturn ['db' => ['host' => 'localhost']];\n"),
        (MODULE, &module),
        ("module/Application/src/classes.php", CLASSES),
        (INDEX, &controller),
        (BLOG, &BLOG_CONTROLLER.replace("\\Laminas\\", "\\Zend\\")),
        (VIEW, VIEW_TEMPLATE),
        ("module/Application/view/application/index/special.phtml", "<p></p>\n"),
        ("module/Application/view/application/index/row.phtml", "<p></p>\n"),
    ]);
    assert!(project.engine.frameworks().contains(crate::frameworks::Framework::Laminas));
    let at = |from: &str, to: &str| controller.replacen(from, to, 1);
    let found = project.laminas_goes(INDEX, &at("toRoute('blog/post'", "toRoute('blog/po/*|*/st'"));
    assert_eq!(found, [(MODULE.to_owned(), 16)]);
    let found = project.laminas_complete(INDEX, &at("$this->greet('you');", "$this->container->get('mailer')->/*|*/;"));
    assert_eq!(found, ["send"]);
    let found = project.laminas_complete(INDEX, &at("$this->greet('you');", "$this->redirect()->/*|*/;"));
    assert_eq!(found, ["toRoute"]);
    has(&project.laminas_complete(VIEW, "<?php echo $post->/*|*/ ?>\n"), &["title"]);
    has(&project.laminas_complete(VIEW, "<?php $this->/*|*/ ?>\n"), &["price", "url", "partial"]);
    assert_eq!(project.laminas_problems(INDEX, &controller), Vec::<String>::new());
}

#[test]
fn a_mezzio_twig_template_has_what_its_handler_passes() {
    let provider = PROVIDER.replace(
        "return ['paths' => ['app' => [__DIR__ . '/../templates/app']]];",
        "return ['extension' => 'html.twig', 'paths' => ['app' => [__DIR__ . '/../templates/app']]];",
    );
    assert!(provider.contains("html.twig"));
    let mut project = Project::new(&[
        ("composer.json", r#"{"require": {"mezzio/mezzio": "^3.29"}}"#),
        ("framework/classes.php", FRAMEWORK),
        (
            "config/config.php",
            "<?php\n$aggregator = new ConfigAggregator([\n    App\\ConfigProvider::class,\n]);\nreturn $aggregator->getMergedConfig();\n",
        ),
        ("config/routes.php", ROUTES),
        ("src/App/src/ConfigProvider.php", &provider),
        (HANDLER, HOME_HANDLER),
        ("src/App/templates/app/home-page.html.twig", "<h1>{{ name }}</h1>\n"),
    ]);
    // The name goes to the Twig file, and the template has the handler's variables.
    let found = project
        .laminas_goes(HANDLER, &HOME_HANDLER.replacen("render('app::home-page'", "render('app::home-/*|*/page'", 1));
    assert_eq!(found, [("src/App/templates/app/home-page.html.twig".to_owned(), 0)]);
    let variables = project.engine.twig_variables("@app/home-page.html.twig");
    assert_eq!(variables, [("name".to_owned(), "string".to_owned()), ("visits".to_owned(), "int".to_owned())]);
    assert_eq!(project.laminas_problems(HANDLER, HOME_HANDLER), Vec::<String>::new());
}
