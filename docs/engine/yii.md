# Yii 2

What the engine knows about a Yii 2 application, where the code is, and what it doesn't do.
How a framework plugs into the engine is in [`frameworks.md`](frameworks.md).

Yii is BSD-3-Clause. Its guide is the design input; nothing of it or of its source is in
this repository.

## Detection and configuration

`frameworks/detect.rs`: Composer installing `yiisoft/yii2`, a `yii` file at the root (the
console entry script of both application templates), or `vendor/yiisoft/yii2/Yii.php`. The
`frameworks` setting forces it on or off.

`frameworks/yii/project.rs` (`YiiProject`, kept by the engine for an edit count) reads the
configuration with the PHP-array reader (below):

- **The applications:** a folder whose `config` folder has `main.php`, `main-local.php`,
  `web.php`, `web-local.php`, `console.php` or `test.php`: the project itself (the basic
  template) or its first-level folders (the advanced template's `common`, `frontend`,
  `backend`, `console`). The folder above `config` is the application's base path.
- **Components and modules:** the keys of `components` and `modules`, each with the class
  its value names (`'yii\caching\FileCache'`, `Foo::class`, `['class' => …]`,
  `['__class' => …]`), and the `identityClass` of the `user` component.
- **Parameters:** the keys of `params`, and the keys of the arrays below them
  (`'mail' => ['from' => …]`).
- **Aliases:** Yii's own (`@app`, `@vendor`, `@runtime`, `@webroot`, `@yii`, `@bower`,
  `@npm`), the application folders of the advanced template, the keys of `aliases`, the
  `Yii::setAlias()` calls of the files of `config/` (`bootstrap.php` first) and of the
  entry scripts (`web/index.php`, `yii`), and the aliases of the installed extensions
  (`vendor/yiisoft/extensions.php`).
- **Controllers:** `controllerNamespace`, else `app\controllers`; and `controllerMap` of
  an application or a module (an id and its class).
- **Where views are:** the `viewPath` and `layoutPath` of an application or a module, the
  `viewPath` of the mailer, and a theme's `pathMap` (without one, the application's
  folder to the theme's `basePath`). A module's folder is its namespace's alias
  (`app\modules\admin` is `@app/modules/admin`).
- **Messages:** the files of `messages/<language>/` of each application and of the
  `basePath` of the sources of the `i18n` component. A PHP file is its category (`app.php`
  is `app`, or what `fileMap` says) and its string keys are the messages. A gettext
  catalogue (`.po`) has the messages that have a context (`msgctxt`), which is their
  category.
- **Migrations:** the `migrations` folder of each application, and the `migrationPath`
  and `migrationNamespaces` of a `migrate` controller of `controllerMap`.

## The PHP-array reader

`frameworks/php_array.rs` reads a PHP file that returns an array, without running it. It
is shared: Yii's configuration and messages, Symfony's `services.php`, `routes.php` and PHP
translation catalogues (`symfony/mod.rs`, `symfony/translations.rs`), and Laminas when its
phase comes. Laravel's `frameworks/laravel/config.rs` uses its helpers.

It follows what configuration files are written with:

- `return [...]`, `return array(...)`, in a `namespace` too;
- `return $config;` where the file assigns `$config` an array, with the later
  `$config['modules']['debug'] = …` and `$config['bootstrap'][] = …` put into it (those
  inside an `if` too);
- a value that is a variable the file assigns (`'db' => $db`);
- `require` and `include` of a path written with strings, `__DIR__`, `__FILE__` and
  `dirname()`;
- `array_merge()`, `array_replace()`, their `_recursive` forms, a static `merge()`
  (`ArrayHelper::merge()`) and `...` in an array;
- a static `config()` given one array (Symfony 7.4's `App::config([...])` and
  `Routes::config([...])`).

Keys and values are strings, numbers, `true`, `false`, `null`, `Foo::class` (as a fully
qualified name, through the file's namespace and imports), `Foo::BAR`, and strings joined
with `.`. Anything else is kept as its source text. Every entry has its file and the place
of its key.

## Names

`frameworks/yii/refs` (one type, `YiiName`):

| Kind | Names | Where |
| --- | --- | --- |
| `yiiView` | a view's file, found as Yii finds it from who asks | a controller's, a view's and a widget's `render()`, `renderPartial()`, `renderAjax()` |
| `yiiLayout` | a file of the `layouts` of the module's views | the `$layout` of a controller or a module |
| `yiiMail` | a view of `@app/mail` | a mailer's `compose()` (one view, or `['html' => …, 'text' => …]`) |
| `yiiRoute` | an action | `Url::to([...])`, `Url::toRoute()`, `redirect([...])`, `run()`, `runAction()`, `Html::a($text, [...])`, `Html::beginForm([...])`, `createUrl()`, `'url' => [...]`, `'loginUrl' => [...]`, `'defaultRoute'`; the URL manager's rules (`'rules' => ['posts' => 'post/index']`, `'route' => 'post/index'`); a filter's actions (`'only' => ['logout']`, `'except'`, `'actions' => ['delete' => ['post']]`) |
| `yiiComponent` | a key of `components` | `get()`, `has()`, `clear()` of an application or a module |
| `yiiModule` | a key of `modules` | `getModule()`, `hasModule()` |
| `yiiParam` | a key of `params`, or of an array below it | `Yii::$app->params['adminEmail']`, `Yii::$app->params['mail']['from']`: a key read from the `params` of an application or a module (`CalleePattern::Read`; a view's own `$this->params` isn't one) |
| `yiiAlias` | the file an aliased path names, else where the alias is declared | `Yii::getAlias()`, `Yii::setAlias()`, `renderFile()`, `beginContent()` |
| `yiiCategory` | a message file | `Yii::t('app', …)` |
| `yiiMessage` | a message of that category | `Yii::t(…, 'Hello')` |
| `yiiAttribute` | a property of the model: declared, documented with `@property`, a column of a record's table, or read through `getName()` | the attributes of a rule of `rules()`, the keys of `attributeLabels()` and `attributeHints()`, the lists of `scenarios()`, `getAttributeLabel()`, `hasErrors()`, `addError()`, `getAttribute()`… |

All have completion, goto, hover and Find References. Nothing is renamed.

- **A view's file** (`YiiProject::view_file`): `@alias/path`; `//path` from the
  application's `views`; `/path` from the `views` of the module the asking file is in;
  else a folder by who asks: beside a view, `views/<controller-id>` for a controller
  (`views/admin/post-comment` for `controllers/admin/PostCommentController.php`), `views`
  beside any other class (a widget). A name without an extension is a `.php` file.
  The file is then where the configuration puts it (`YiiProject::actual`): in the
  `viewPath`, and in a theme when the theme has the file. A file of a theme or of a
  `viewPath` asks as the file of `views` it stands for (`YiiProject::logical`), and is
  a view (its variables, `$this`).
- **A mail view** is a file of the mailer's `viewPath` of the asking application, else of
  another's (the advanced template's `@common/mail`), else of `mail`.
- **A route** is `[module/][folders/]controller/action`: the method `action<Name>` of
  `<Name>Controller` in a `controllers` folder of an application, or of a module (the
  folder of the class its configuration names). One segment is an action of the
  controller asking, or of the controller the asking view belongs to. A route naming only
  a controller is its `index` action. Every usage of one action is one thing: `about` in
  `SiteController` and `site/about` elsewhere are found together.
  - A controller of `controllerMap` comes first: `account/show` is `actionShow()` of the
    class mapped to `account`.
  - An action without a method is its key in the array the controller's `actions()`
    returns (`'captcha' => [...]`), of the controller or of a class of the project it
    extends.
  - Where several applications have the controller (the advanced template's `frontend`
    and `backend`), it is the asking application's, else the one that has the action.
- **A component, a module and a parameter** several applications configure (the advanced
  template's `frontend` and `backend`) go to the configuration of the application the
  asking file is in, when it has one.
- **A name after a variable a view is passed** (`$form->getAttributeLabel('email')` in a
  view) is typed by what the view is passed: the string references type a file's
  receivers knowing which file it is (`refs::sites_at`). CodeIgniter's views gain the same.
- **A rule's attributes** are its first element, a string or a list. The validator's name
  and the lists among its options aren't attributes: the kind looks at where the string
  is in the returned array.

### Names that name nothing

`ReferenceKind::undefined`, as inspections (`inspect/laravel.rs`, `YII`):

| Inspection | Reported when |
| --- | --- |
| `PhpUndefinedYiiView`, `PhpUndefinedYiiLayout` | the file isn't there and its folder is (a theme or a `viewPath` may hold the views of a folder that isn't) |
| `PhpUndefinedYiiRoute` | the controller has no such action, as a method or as a key of its `actions()` (when that returns one array written out; one built with `array_merge()` may list more); or `controller/action` names no controller, is no module's, and the project has controllers. Not for the keys of an array (`'url' => [...]`), and only for a route of lower-case letters, digits, `-` and `/` |
| `PhpUnknownYiiAttribute` | in `rules()`, `attributeLabels()`, `attributeHints()` or `scenarios()` of a model that isn't an Active Record and declares neither `attributes()` nor `__get()`: no property and no getter of that name |

Not in the files of `vendor/`.

## Types

`types/provider/yii.rs`, only where `yii\BaseYii` is known:

- **Components:** `Yii::$app->mailer` and `Yii::$app->get('mailer')` are the class the
  configuration gives the component. A component Yii documents (`$user`) is the
  configured class when the configuration names one. They complete after `Yii::$app->`
  and go to their key of the configuration.
- **The identity:** `Yii::$app->user->identity` is the `identityClass`, or null.
- **Objects:** `Yii::createObject(Foo::class)`, `Yii::createObject('app\Foo')`,
  `Yii::createObject(['class' => Foo::class, …])` and `Yii::$container->get(Foo::class)`
  are a `Foo`.
- **Properties read through a getter:** on a `yii\base\BaseObject`, `$object->name` is
  what `getName()` returns. The getters of the project's classes complete as properties
  and go to the getter.
- **Active Record:** `Post::find()` is an `ActiveQuery<Post>` (the query class `find()`
  declares, when it declares one), which the methods returning `$this` keep; `one()` is a
  `Post` or null and `all()` a list. `hasOne(Author::class, …)` is an
  `ActiveQuery<Author>` and `hasMany(Tag::class, …)` an `ActiveQuery<list<Tag>>`, so
  `$post->author` (through `getAuthor()`) is an `Author` or null and `$post->tags` a list
  of `Tag`.
- **A record's columns** (`frameworks/yii/migrations.rs`): the tables the project's
  migrations make, read without running them: `createTable()`, `addColumn()`,
  `dropColumn()`, `renameColumn()`, `alterColumn()`, `dropTable()`, `renameTable()`, in
  the order of the files' names, `down()` and `safeDown()` left out. A record is of the
  table its `tableName()` returns as a string, else of the one Yii names after the class
  (`PostComment` is `post_comment`). `$post->title` is then the column's type
  (`$this->string()->notNull()` is a `string`, `$this->integer()` an `int` or null), the
  columns complete after `$post->` and go to the migration, and they are attributes for
  `yiiAttribute`. A property the class declares or documents wins. A migration is read
  again when its file changes.
- **A view's variables** (`frameworks/yii/views.rs`): `$this->render('index', ['post' =>
  $post])` makes `$post` a variable of `views/site/index.php`, of the type passed, and a
  mail view has what `compose()` passes. `$this` in a view is `yii\web\View`. They reach
  the view's own inference through `Declarations::view_variable`: a view's variables
  aren't reported as undefined.

## Tests

- `frameworks/php_array.rs`, `frameworks/yii/project.rs` and
  `frameworks/yii/migrations.rs`: the reader, the project model and the migrations, on
  files written for the tests.
- `navigate/tests/yii.rs`: every feature, on a stand-in for Yii's API written for the
  tests.
- `symfony/tests.rs` and `symfony/translations.rs`: Symfony's PHP configuration and
  catalogues.
- `tests/yii_fixture.rs` on `fixtures/yii-app`: nothing in the application is reported,
  one name of each kind goes where it should, and what is passed and configured is typed.
- `phoxite-lsp/src/tests.rs`: the fixture through the protocol.
- `editors/vscode/test/e2e` runs once on the fixture in VS Code (`yiiChecks`;
  `PHOXITE_E2E_ONLY=yii`).
- `tests/yii_corpus.rs` (ignored; `scripts/php-corpus.sh yii yii-advanced yii-framework`):
  Yii's basic and advanced application templates 2.0.55 on the real framework. The
  configuration, the views, the routes and the types hold there, each application of the
  advanced template has its own views and parameters, its mail views are the common
  application's, its `user` table has the columns of its two migrations, and the Yii
  inspections report nothing in either (62 files of the advanced one).

## What cannot be read

All of these need the application to run, or a database:

- **A table no migration of the project makes** (one of an extension, or made by hand):
  the database is never asked. Declare the attributes with `@property`.
- **Messages kept in the database** (`DbMessageSource`) and in compiled `.mo` files (the
  `.po` beside one is read).
- **A view, a route or a parameter named through a variable or built at run time**, a
  `viewPath` or a theme set in code (`$this->viewPath = …` in `init()`), an alias set
  outside the configuration, the entry scripts and `extensions.php`.
- **A URL rule's pattern** (`'post/<id:\d+>'`): only the route it leads to is a name, and
  a route with placeholders (`'<controller>/<action>'`) names nothing.
- **The local files of the advanced template** (`main-local.php`) before `init` makes
  them: `environments/` isn't read.

Yii 1.1 and Yii 3 are other frameworks with other APIs; this is Yii 2 only.
