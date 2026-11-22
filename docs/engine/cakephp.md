# CakePHP 4 and 5

What the engine knows about a CakePHP application, where the code is, and what it doesn't
do. How a framework plugs into the engine is in [`frameworks.md`](frameworks.md).

CakePHP is MIT-licensed. Its book is the design input; nothing of it or of its source is
in this repository.

## Detection and the project's files

`frameworks/detect.rs`: Composer installing `cakephp/cakephp`, a `bin/cake` file (the
console entry script of the application skeleton), or `vendor/cakephp/cakephp/VERSION.txt`.
The `frameworks` setting forces it on or off.

`frameworks/cakephp/project.rs` (`CakeProject`, kept by the engine for an edit count):

- **The application's namespace:** `App.namespace` of `config/app.php`, else the namespace
  Composer loads from `src/`, else `App`.
- **Plugins:** the folders of `plugins/` (`plugins/Blog`, `plugins/Company/Blog`) and the
  ones Composer installed (`vendor/cakephp-plugins.php`). A plugin's classes are in the
  namespace of its name (`Company\Blog`), its templates in its `templates` folder.
- **Configuration:** the keys of `config/app.php`, `config/app_local.php` and the files
  `config/bootstrap.php` loads with `Configure::load()`, each as its dotted path
  (`Datasources.default.host`), read with the PHP-array reader
  ([`yii.md`](yii.md#the-php-array-reader)).
- **Named routes:** `'_name' => 'login'` in the options of `connect()`, and the name given
  to `get()`, `post()` and the like, in `config/routes.php`, a plugin's routes file and
  the application and plugin classes.
- **Translations:** the messages of `resources/locales/<language>/<domain>.po`, the
  application's and each plugin's, read with the gettext reader (`frameworks/po.rs`,
  shared with Yii).
- **Migrations:** the `config/Migrations` folder of the application and of each plugin.
- **CakePHP 3's layout:** where the project has `src/Template` and no `templates`,
  templates are `.ctp` files of `src/Template` (`Element`, `Layout`, `Cell`, `Email`,
  `Plugin`) and catalogues are in `src/Locale`.

## Conventions

A name is a class by where CakePHP looks for it (`frameworks/cakephp/classes.rs`), and
only when the project has that class:

| Name | Class |
| --- | --- |
| a table `Articles` | `App\Model\Table\ArticlesTable`; without one, CakePHP's `Table` |
| its entity | what `setEntityClass()` names, else `App\Model\Entity\Article`, else CakePHP's `Entity` |
| a component `Flash` | `App\Controller\Component\FlashComponent`, else `Cake\Controller\Component\FlashComponent` |
| a helper `Html` | `App\View\Helper\HtmlHelper`, else `Cake\View\Helper\HtmlHelper` |
| a behaviour `Timestamp` | `App\Model\Behavior\TimestampBehavior`, else `Cake\ORM\Behavior\TimestampBehavior` |
| a cell `Inbox` | `App\View\Cell\InboxCell` |
| a controller `Articles` under the prefix `Admin` | `App\Controller\Admin\ArticlesController` |
| `Blog.Tags` | the same in the plugin's namespace: `Blog\Model\Table\TagsTable` |
| `'className' => …` | that name, or that class, under the alias |

The singular and the plural of a name (`frameworks/cakephp/inflect.rs`) follow English
spelling rules and a list of irregular nouns written for this engine, not CakePHP's
inflector. A word they get wrong is a class that isn't found, nothing more: every guess is
checked against the project's classes.

What a class loads is read from its code (`frameworks/cakephp/facts.rs`,
`PhpEngine::cake_facts`, kept for an edit count), its own and that of the classes of the
project it extends, so an `AppController`'s components are every controller's:

- a controller's `loadComponent()`, `$components`, `loadModel()`, `$defaultTable`
  (`$modelClass` in CakePHP 4), and `viewBuilder()->addHelper()`;
- where its templates are: `setTheme()`, `setTemplatePath()` and `setLayoutPath()` given
  a string, on the view builder or on a view;
- a view's and a helper's `loadHelper()`, `addHelper()` and `$helpers`;
- a table's `belongsTo()`, `hasOne()`, `hasMany()`, `belongsToMany()` (with `className` and
  `propertyName`), `addBehavior()`, `setEntityClass()` and `setTable()`.

## Names

`frameworks/cakephp/refs.rs` (one type, `CakeName`):

| Kind | Names | Where |
| --- | --- | --- |
| `cakeTable` | a table class, by its alias | `fetchTable()`, `loadModel()`, a table locator's `get()`, `belongsTo()`, `hasOne()`, `hasMany()`, `belongsToMany()` and their `className` and `through` |
| `cakeComponent` | a component class | `loadComponent()` and its `className`, `$components`, the component registry |
| `cakeHelper` | a helper class | `loadHelper()`, `addHelper()`, `$helpers`, the view builder's `addHelper()`, `addHelpers()` and `setHelpers()`, the helper registry |
| `cakeBehavior` | a behaviour class | `addBehavior()`, `addBehaviors()`, `getBehavior()`, `hasBehavior()`, `removeBehavior()`, the behaviour registry |
| `cakeAssociation` | an association, where its table declares it | a query's and a table's `contain()`, `matching()`, `innerJoinWith()`, `leftJoinWith()`, `notMatching()`; `getAssociation()`, `hasAssociation()` |
| `cakeTemplate` | a template's file | a controller's and a cell's `render()`, `setTemplate()`, a view's `extend()` |
| `cakeLayout` | a file of `templates/layout` | `setLayout()`, the second argument of `render()` |
| `cakeElement` | a file of `templates/element` | a view's `element()`, `elementExists()` |
| `cakeCell` | a cell class, or its method (`Inbox::expanded`) | `cell()` |
| `cakeConfig` | a key of the configuration | `Configure::read()`, `check()`, `readOrFail()`, `consume()`, `consumeOrFail()`, `write()`, `delete()` |
| `cakeController` | a controller class | `'controller' => 'Articles'` of a route array |
| `cakeAction` | a method of a controller | `'action' => 'view'` of a route array; `'Articles::view'` of a route and of `Router::pathUrl()` |
| `cakePlugin` | a plugin: its plugin class, else its `composer.json` | `'plugin' => 'Blog'` of a route array, `addPlugin()`, a route builder's `plugin()`, `Plugin::isLoaded()` |
| `cakeRoute` | a named route | `'_name' => 'login'` |
| `cakeMessage` | a message of a catalogue, at its `msgid` | `__()`, `__n()`, `__d()`, `__dn()`, `__x()`, `__xn()`, `__dx()`, `__dxn()` |
| `cakeDomain` | a catalogue's files | the first argument of `__d()`, `__dn()`, `__dx()`, `__dxn()` |

All have completion, goto, hover and Find References. Nothing is renamed.

- **A template** (`CakeProject::template_files`) is found from who asks:
  `templates/Articles/` for `ArticlesController` (`templates/Admin/Articles/` under a
  prefix), `templates/cell/Inbox/` for a cell, `templates/email/html/` and `text/` for a
  mailer, and beside the template asking. `/Common/view` is from the `templates` folder,
  `Blog.sidebar` the plugin's. A plugin's templates are looked for in the application's
  own version first (`templates/plugin/Blog/`), then in the plugin, then in the
  application. CakePHP's own layouts and elements (`dev_error`) come last, from the
  `templates` folder beside the framework's `src`.
- **A theme** (`setTheme('Modern')` in the class asking, in a class it extends, or in
  the application's `AppController` or `AppView`) is a plugin whose templates come first;
  what it doesn't have is the application's. `setTemplatePath('Common')` replaces the
  folder of the class's templates, `setLayoutPath('admin')` is a folder below `layout`.
- **A route array** is read whole: an action is a method of the controller its array
  names, under the `prefix` and in the `plugin` beside it; without a `controller` it is
  of the controller asking, or of the one whose template is asking. `view-all` and
  `view_all` are `viewAll()`.
- **An association** in `contain()` is one of the table the call is on, or of the table of
  the entity the query finds. `Comments.Users` follows `Comments` to its table and looks
  for `Users` there, and so do the arrays of `contain(['Comments' => ['Users']])`: the
  keys above a string lead to its table (`SiteInfo::keys`). A key that is an option
  (`'sort'`, `'conditions'`) is no association and names nothing.
- **A message** is one of the catalogue `default`, or of the one `__d()` names. A message
  no catalogue has isn't reported: it is shown as written until it is translated.

### Names that name nothing

`ReferenceKind::undefined`, as inspections (`inspect/laravel.rs`, `CAKEPHP`):

| Inspection | Reported when |
| --- | --- |
| `PhpUndefinedCakeComponent`, `PhpUndefinedCakeHelper`, `PhpUndefinedCakeBehavior` | no class of the application, of the plugin named or of CakePHP is of that name. Not for a plugin Composer installed, whose classes may not be indexed |
| `PhpUndefinedCakeTemplate`, `PhpUndefinedCakeLayout`, `PhpUndefinedCakeElement` | the file is in none of the folders it could be in, the theme's and the template path's among them. Not where the file asking sets another view class (`setClassName()`), a theme or a path that isn't a string, or a theme the project has no plugin of |
| `PhpUndefinedCakeAction` | the controller has no such method and no `__call()`; only where a route array is what the method takes (`redirect()`, `Router::url()`, `Html->link()`, `Url->build()`, `Form->postLink()`) or a route's `'Controller::action'` |

Not in the files of `vendor/`. A table's alias is never reported: a table without a class
is CakePHP's `Table`.

## Types

`types/provider/cakephp.rs`, only in a CakePHP project:

- **Tables:** `fetchTable('Articles')`, a locator's `get('Articles')` and
  `loadModel('Articles')` are the table's class. A controller has the table of its name as
  a property (`$this->Articles`), and `fetchTable()` without a name is that table.
- **Entities:** a table's `get()`, `newEntity()`, `newEmptyEntity()`, `patchEntity()`,
  `save()`, `saveOrFail()` and `findOrCreate()` give its entity, `newEntities()` a list,
  `find()` a query of it: `first()` is the entity or null, `firstOrFail()` the entity,
  `toArray()` a list and `all()` a result set of it. `paginate()` gives the entity of the
  table, the query or the alias it is given, else of the controller's table.
- **Associations:** `$this->Articles->Authors` is the association and the table it leads
  to. `$article->author` is the associated entity or null, `$article->comments` a list.
- **An entity's fields** (`frameworks/cakephp/migrations.rs`, `PhpEngine::cake_tables`):
  the columns of its table, as the project's migrations make it, read without running
  them. A migration builds a table from `$this->table('articles')`, directly or through
  the variable it is assigned to: `addColumn()`, `removeColumn()`, `renameColumn()`,
  `changeColumn()`, `drop()`, `rename()`, in the order of the files' names, `down()`
  left out. A table has an `id` unless its options say otherwise. The entity's table is
  the table class of its name; the database table is what `setTable()` names, else the
  alias in snake case. `$article->views` is then the column's type (`integer` is an
  `int`, nullable with `'null' => true`; `datetime` is CakePHP's `DateTime`, or
  `FrozenTime` in CakePHP 4), the fields complete and go to the migration. A property
  the class documents wins. A migration is read again when its file changes.
- **Components, helpers, behaviours:** `$this->Flash` in a controller is the component it
  (or a class it extends) loads; `$this->Html` in a view is the helper of that name,
  loaded or not, as a view loads a helper when asked; `getBehavior('Timestamp')` is the
  behaviour's class.
- **Members:** the components, tables, associations, helpers and associated entities
  complete after `->` and go to where they are loaded.

CakePHP 5 documents generics (`Table<TBehaviors, TEntity>`, `SelectQuery<TSubject>`), and
a baked table says its entity with `@extends`. Where those declarations already name a
class of the project, they are kept; the provider answers where they say only
`EntityInterface`, as they do for a table without `@extends` and everywhere in CakePHP 4.

- **A template's variables** (`frameworks/cakephp/views.rs`): `templates/Articles/view.php`
  has what `ArticlesController::view()` sets (`set('article', $article)`,
  `set(compact('article'))`, `set(['article' => $article])`, `viewBuilder()->setVar()`),
  what `beforeFilter()`, `beforeRender()` and `initialize()` of the controller and of the
  classes it extends set, and what a method rendering it by name sets
  (`render('view')`). A cell's template has what the cell's method sets, a mail template
  what a mailer's method of its name passes, an element what `element('name', [...])`
  passes. `$this` in a template is the application's `AppView`, else CakePHP's `View`.

## Tests

- `frameworks/cakephp/{inflect,project,facts,migrations}.rs` and `frameworks/po.rs`: the
  words, the project model (CakePHP 3's layout, themes and template paths among it), a
  class's facts, the migrations and the gettext reader, on files written for the tests.
- `navigate/tests/cakephp.rs`: every feature, on a stand-in for CakePHP's API without
  generics.
- `tests/cakephp_fixture.rs` on `fixtures/cakephp-app`: nothing in the application is
  reported, one name of each kind goes where it should, and what is set and loaded is
  typed.
- `phoxite-lsp/src/tests.rs`: the fixture through the protocol.
- `editors/vscode/test/e2e` runs once on the fixture in VS Code (`cakeChecks`;
  `PHOXITE_E2E_ONLY=cakephp`).
- `tests/cakephp_corpus.rs` (ignored;
  `scripts/php-corpus.sh cakephp cakephp-framework cakephp4-framework`): the fixture
  application without its stand-in on the real CakePHP 5.4.4 and on CakePHP 4.6.6, where
  the names resolve to CakePHP's own classes, tables, entities, components and helpers
  are typed, and the only reports in its 25 files are the base class of the Migrations
  plugin, which isn't in the corpus; and CakePHP's application skeleton 5.4.0, where the
  CakePHP inspections report nothing.

## What cannot be read

All of these need the application to run, or a database:

- **A table no migration of the project makes** (a plugin's, or one made by hand): the
  database is never asked. The `@property` tags `bake` writes say its fields.
- **A theme, a template path, a component or a table named through a variable**, and
  what a behaviour or an event listener adds while the application runs (an association,
  a helper).
- **Another view class's templates** (`JsonView`, `setClassName()`): they need no file,
  or keep theirs elsewhere; nothing is reported there.
- **A route's path and placeholders**: a route is what the router matches a URL with,
  not a name. Its name, and the controller and action it leads to, are names.
- **A message's plural forms and compiled catalogues** (`.mo`): the `.po` beside one is
  read, and a message is found by its singular text.

In a CakePHP 3 project names find the `.ctp` templates, but a `.ctp` file isn't opened
as PHP, so nothing is offered inside one. CakePHP 2 and earlier are other frameworks.
