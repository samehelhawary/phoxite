# Laminas MVC and Mezzio

What the engine knows about a Laminas MVC or a Mezzio application, where the code is, and
what it doesn't do. How a framework plugs into the engine is in
[`frameworks.md`](frameworks.md).

Laminas and Mezzio are BSD-3-Clause. Their documentation is the design input; nothing of
it or of their source is in this repository.

## Detection and configuration

`frameworks/detect.rs`: Composer installing `laminas/laminas-mvc` or `mezzio/mezzio`, a
`config/modules.config.php` file (an MVC application's module list), or
`vendor/laminas/laminas-mvc/composer.json`. The `frameworks` setting forces it on or off.
Both are the framework `laminas`, and so are Zend Framework 2 and 3 and Zend Expressive
(`zendframework/zend-mvc`, `zendframework/zend-expressive`): every class the support
asks about is known by its `Zend\…` name too (`Zend\Expressive\…` for Mezzio's).

`frameworks/laminas/project.rs` (`LaminasProject`, kept by the engine for an edit count)
reads the configuration with the PHP-array reader
([`yii.md`](yii.md#the-php-array-reader)), in the order it is merged:

- **MVC:** the modules `config/modules.config.php` lists that Composer installed (their
  `Module` and their `ConfigProvider`), the project's own `Module` classes, each
  `module/<Name>/config/module.config.php` of the project, then
  `config/autoload/*.global.php` and `*.local.php`. Of a `Module`, `getConfig()` and
  the methods returning one plugin manager's configuration: `getServiceConfig()`,
  `getControllerConfig()`, `getControllerPluginConfig()`, `getViewHelperConfig()`,
  `getFormElementConfig()`, `getValidatorConfig()`, `getFilterConfig()`,
  `getHydratorConfig()`, `getInputFilterConfig()`.
- **Mezzio** (a `config/config.php` and no `config/application.config.php`): the
  `ConfigProvider` classes `config/config.php` names, those of the project's `src/`, then
  `config/autoload/`; and the routes of `config/routes.php`.

A `ConfigProvider` is read as a class: the reader takes the array its `__invoke()`
returns, and a value written `$this->getDependencies()` is the array that method returns
(`php_array::read_method`). An installed module or provider is found by its class, through
the index.

What is kept:

- **Services:** `service_manager` (MVC) and `dependencies` (Mezzio): `aliases`,
  `factories`, `invokables`, `services`, `delegators` (a service's last one), and whether
  there are abstract factories. A factory written as a closure keeps the class it makes:
  its declared return type, else the class of the `new` it returns
  (`php_array::Node::Closure`).
- **The other plugin managers** (`Manager::ALL`): `controllers`, `controller_plugins`,
  `view_helpers`, `form_elements`, `validators`, `filters`, `hydrators` and
  `input_filters`, each after what Laminas's own plugin manager lists (the
  `'name' => Foo::class` pairs of its class, read from its file: 124 validators, 138
  filters and 110 form elements on the packages of the corpus). The navigation
  containers are the keys of `navigation`.
- **Routes:** `router.routes`, a child route as `parent/child`, each with the controller
  and action of its defaults, its path and the parameters the path names (`:id`,
  `[/:slug]`, FastRoute's `{id}` and `{id:\d+}`), its parents' first. Mezzio's `$app->get('/', Handler::class, 'home')`,
  `route()`, `setName()`, and routes written as configuration (`routes`).
- **Templates:** `view_manager.template_map` and `template_path_stack`; Mezzio's
  `templates.paths` (`app::home-page` is `templates/app/home-page.phtml`), `map`,
  `layout` and `extension` (of several parts too: `html.twig`, which is also what a
  project on Mezzio's Twig renderer has when it names none).
- **Configuration:** every key, with the keys above it.

A module's configuration reaches the model twice, by its file and through its `Module`:
what is written once is kept once.

## Names

`frameworks/laminas/refs.rs` (one type, `LaminasName`):

| Kind | Names | Where |
| --- | --- | --- |
| `laminasRoute` | a route, where the configuration or `config/routes.php` names it | `url('name')` in a template, a controller's `url()->fromRoute()` and `redirect()->toRoute()`, Mezzio's `UrlHelper::generate()` and `RouterInterface::generateUri()`, the name `$app->get()` gives, `'route' => 'name'` |
| `laminasTemplate` | a template's file | `setTemplate()`, `partial()`, `partialLoop()`, `layout()`, a renderer's `render()`, the keys of `template_map`, `'layout' =>`, `'not_found_template' =>`, `'exception_template' =>` |
| `laminasService` | a service, where a configuration declares it; a class's name is the class | `get()`, `has()` and `build()` of a PSR container or a service locator |
| `laminasPlugin` | a controller plugin's class | `plugin('name')` in a controller |
| `laminasHelper` | a view helper's class | `plugin('name')` in a template or on a renderer |
| `laminasAction` | a controller's action: `'show'` is `showAction()` | `'action' => 'show'` |
| `laminasConfig` | a key of the configuration, below the keys read before it | `$container->get('config')['db']['host']`, and `$config['db']['host']` where `$config` was assigned that call |
| `laminasValidator`, `laminasFilter` | a validator's and a filter's class | `'validators' => [['name' => 'StringLength']]`, `'filters' => [['name' => 'StringTrim']]`, `attachByName()` |
| `laminasFormElement` | a form element's class | `'type' => 'text'` of what a form's `add()` is given, and of a `'spec'` |
| `laminasNavigation` | a navigation container | `$this->navigation('navigation')` |
| `laminasRouteParam` | a parameter of a route's path | the keys of the parameters of `url('name', ['id' => 1])`, `fromRoute()`, `toRoute()`, `generate()` |

All have completion, goto, hover and Find References. Nothing is renamed.

- **An action** is of the controller beside it (`'controller' => Foo::class`); in a
  route's defaults without one, of the route the string is written in and then of its
  parents (a child route has its parent's controller); in the parameters of a URL
  (`toRoute('blog/post', ['action' => 'show'])`), of the route the URL is of; else of
  the controller asking.
- **A key of the configuration** is read from what a call returns: the string
  references take the keys read from a method's result (`CalleePattern::ResultKey`,
  with the keys read before in `CallTarget::Result`), and the kind asks that the call be
  `get('config')`. A variable assigned such a call in the same function stands for it,
  until it is assigned again.
- **A service** is looked for in the manager the call is on: `get('StringLength')` on a
  validator plugin manager is a validator.
- **A plugin's and a helper's name** has several spellings in a plugin manager
  (`escapeHtml`, `escapehtml`, `EscapeHtml`): the one a method is written with is offered
  (`LaminasProject::callable_names`), and a name is found whatever its case.

### Names that name nothing

`ReferenceKind::undefined`, as inspections (`inspect/references.rs`, `LAMINAS`), only where
the string is an argument of a call, never for an array's entry:

| Inspection | Reported when |
| --- | --- |
| `PhpUndefinedLaminasRoute` | no configuration names the route, and the project has routes. Not where a route is named (`$app->get(…, 'home')`) |
| `PhpUndefinedLaminasTemplate` | the file is in none of the places the name could be, and the project has a template path or a namespace |
| `PhpUndefinedLaminasService` | a plain name (not a class's) no configuration declares, in a project without abstract factories. Not for `config` and the services an application has without declaring them |
| `PhpUndefinedLaminasAction` | the controller has no such `…Action()` method |

Not in the files of `vendor/`.

## Types

`types/provider/laminas.rs`, only in a Laminas or a Mezzio project
(`TypeProvider::applies`):

- **Services:** `$container->get('mailer')`: an alias is followed, an invokable is its
  class, a factory is what its `__invoke()` declares it returns, a closure what it is
  declared to return or makes. A service with delegators is what its last delegator
  factory's `__invoke()` declares, when that is a class. `get()` on the form element,
  validator, filter, hydrator and input filter managers is that manager's service. `InvokableFactory` and
  the reflection-based factories make the class the service is named after. A name that
  is a class is that class. `get('config')` is an array.
- **Controller plugins:** `$this->greet()` in a controller is what the plugin's
  `__invoke()` returns, or the plugin when it declares nothing; `$this->plugin('greet')`
  is the plugin. A method the controller has or documents (`@method`) stays that method.
- **View helpers:** the same in a template and on a renderer.
- The plugins and helpers complete after `$this->` and go to their class.

- **A template's variables** (`frameworks/laminas/views.rs`): in an MVC application,
  what an action gives its view model (`new ViewModel([...])`, a returned array,
  `setVariable()`, `setVariables()`), for the template the action names
  (`setTemplate()`) or the one of its name
  (`application/index/index` for `indexAction()` of
  `Application\Controller\IndexController`). Each view model of a method is followed by
  the variable it is assigned to, so a child (`addChild()`) with its own template has its
  own variables. The layout has what a controller gives it in any method
  (`$this->layout()->setVariable()`, `$this->layout()->title = …`), for the layout it
  names, else the configuration's, else `layout/layout`. In Mezzio, what
  `$this->template->render('app::home-page', [...])` passes; in both, what
  `partial('name', [...])` passes. `$this` in a template is laminas-view 3's `Template`
  where the project has it, else `PhpRenderer`, else Plates's `Template`. A Twig
  template of a Mezzio application is named `@app/home-page.html.twig` by the Twig
  side (its namespaces are template folders, `twig::TemplatePaths`), and gets the same
  variables through `PhpEngine::twig_template_variables`.

## Tests

- `frameworks/php_array.rs` and `frameworks/laminas/project.rs`: a class's configuration
  and the project model of an MVC and of a Mezzio application, on files written for the
  tests.
- `navigate/tests/laminas.rs`: every feature, on a stand-in for the API, in an MVC and
  in a Mezzio application.
- `tests/laminas_fixture.rs` on `fixtures/laminas-app` and `fixtures/mezzio-app`: nothing
  is reported, one name of each kind goes where it should, and what is configured and
  given is typed.
- `phoxite-lsp/src/tests.rs`: the MVC fixture through the protocol.
- `editors/vscode/test/e2e` runs once on the MVC fixture in VS Code (`laminasChecks`;
  `PHOXITE_E2E_ONLY=laminas`).
- `tests/laminas_corpus.rs` (ignored; `scripts/php-corpus.sh` with the names each test
  lists): the MVC fixture without its stand-in on laminas-mvc 3.8.0, laminas-view 2.44.0,
  laminas-servicemanager 4.6.0 and laminas-router 3.19.0; the Mezzio fixture on mezzio
  3.29.0 with its router, templates, helpers, the laminas-view renderer and laminas-view
  3.2.0; and Laminas's MVC skeleton 2.4.0. On each the names resolve, Laminas's own
  plugins and helpers are read from the plugin managers (213 helper names in the
  skeleton), services, plugins and helpers are typed, and the Laminas inspections report
  nothing. Mezzio's skeleton has no application until its installer writes one, so it
  isn't a corpus.

## What cannot be read

All of these need the application to run:

- **What an abstract factory makes** of a name that isn't a class: whether it can is
  decided by its `canCreate()`. Such a name isn't reported where the project has abstract
  factories.
- **A configuration passed around as a value:** an array given to a constructor or kept
  in a property is an array like any other. Its keys are names where it is read from the
  container, or from a variable assigned that call in the same function.
- **Configuration built at run time:** by a listener, by a factory written as a closure
  that decides what to make, by a `Module` that computes rather than returns an array.
- **A plugin's or a service's name built at run time**, and a template, a route or a
  layout named through a variable.
- **The pages of a navigation container:** a page's `'route'` and `'action'` are names;
  the tree of pages isn't read.
- **A Mezzio application's route patterns:** a route is found by its name; whether a URL
  matches its path is the router's to say.
