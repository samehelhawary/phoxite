# CodeIgniter

What the engine knows about a CodeIgniter application (4, and 3 at the end), where the code
is, and what it doesn't do. The user-facing list is in `editors/vscode/README.md`; how a
framework plugs into the engine is in [`frameworks.md`](frameworks.md).

CodeIgniter is MIT. Its user guide and its source are the design input; nothing of either
is in this repository.

## Detection and layout

`frameworks/detect.rs`: Composer installing `codeigniter4/framework` (or 3's
`codeigniter/framework`), a `spark` file at the root, or 3's
`system/core/CodeIgniter.php`. The `frameworks` setting forces it on or off.

`frameworks/codeigniter/project.rs` reads where things are:

- The application is in `app/` and the framework in
  `vendor/codeigniter4/framework/system/` or `system/` beside it, unless
  `Config/Paths.php` says otherwise (`$appDirectory`, `$systemDirectory`,
  `$viewDirectory`, written with strings, `__DIR__`, `dirname(__DIR__, 2)`,
  `realpath()`, `DIRECTORY_SEPARATOR` and the path constants). The paths
  file itself is the one `public/index.php` or `spark` names when it isn't
  `app/Config/Paths.php`.
- **Modules:** the `$psr4` of `Config/Autoload.php` (`'Blog' => ROOTPATH . 'modules/Blog'`)
  and the namespaces of Composer's `vendor/composer/autoload_psr4.php`, as CodeIgniter
  discovers them. A module's views, helpers, language files and `Config/Routes.php` are
  read with the application's.

## Names

`frameworks/codeigniter/refs` (one type, `CodeIgniterName`):

| Kind | Names | Where |
| --- | --- | --- |
| `codeigniterView` | a file of `app/Views` (`books/list`), or of a module (`Blog\Views\list`) | `view()`; a view's `$this->extend()`, `$this->include()` |
| `codeigniterHelper` | `<name>_helper.php` of the application, a module or the framework | `helper()` (one or a list), a controller's `$helpers` |
| `codeigniterLang` | a line of a language file: `File.key`, nested ones with more dots | `lang()` |
| `codeigniterRoute` | a route's `as`; its path when it has none (with the prefixes of the groups around it), the paths `resource()` and `presenter()` make among them; its handler (`Books::show`) | `route_to()`, `url_to()`, `redirect()`, `redirect()->route()` |
| `codeigniterFilter` | a key of `Config\Filters::$aliases` | `'filter' => '…'` of a route's or a group's options (`auth:admin` is `auth`); the lists of the filters configuration |
| `codeigniterController` | a method of a controller, `Books::show/$1`, in the `namespace` of the group around it, else what `setDefaultNamespace()` was given, else `App\Controllers` | the handler of a route |
| `codeigniterConfig` | the class `Config\<Name>` | `config()` |
| `codeigniterModel` | the class `App\Models\<Name>`, or the class written | `model()` |
| `codeigniterService` | a static method of `Config\Services`, else of the framework's | `service()`, `single_service()` |
| `codeigniterCell` | a method of a class of `App\Cells`: `Card::render`, `Card` | `view_cell()` |
| `codeigniterEvent` | what `Events::trigger('name')` fires (`DeclaredKind::CodeIgniterEvent`) | `Events::on()`, `trigger()`, `removeListener()` |
| `codeigniterRule` | a method of a class of `Config\Validation::$ruleSets`: each rule of `required\|min_length[3]` | `setRules()`, `setRule()`, a controller's `validate()` and `validateData()`, a model's `$validationRules`, the groups of `Config\Validation` |
| `codeigniterRuleGroup` | a property of `Config\Validation` | `run($data, 'group')`, `setRuleGroup()`, a controller's `validate('group')` |
| `codeigniterTable` | a table a migration makes: `$this->forge->createTable('books')` | the connection's `table()`, a model's `$table`, `from()`, `join()`, the forge's methods |
| `codeigniterColumn` | a field of that table: a key of `addField([...])` before `createTable()` (or along its chain), of `addColumn('books', [...])` | a model's `$allowedFields`, `$primaryKey`, `$createdField`…, the keys of its `$validationRules` and `$casts`; `where()`, `like()`, `orderBy()`, `select()`, `set()`… and the keys of their arrays, on a model or on a builder of a known table |

All have goto, hover and Find References; all but a route's handler complete. A route name
(an `as`), a filter alias and an event are renamed.

- **A rule string** is split at the `|` outside brackets; goto and completion are of the
  rule the caret is in. In `is_unique[books.title]` and `is_not_unique[…]`, the table and
  its field are names too. In an expression (`where('COUNT(price) >', 3)`) the name the
  caret is on is the column.
- **A path** (`codeigniterUri`: `site_url('books/show')`, `redirect()->to()`) is a route
  of that path, else the controller's method auto-routing would call (`Books::show`,
  `Books::getShow`), folders first. Of several columns in a string (`select('title, price')`), they
  are of the column the caret is in.
- **A column's table** is the one written before it (`books.title`), the one the builder
  was made for (`$db->table('books')` is typed `BaseBuilder<'books'>`, which the methods
  returning `$this` keep), or the `$table` of the model the method is called on.
- **`$this` in a view** is the object rendering it: a method called on `$this` outside any
  class has the receiver `TEMPLATE_THIS` (`frameworks/refs`).
- **A route's `$routes`** is typed by the `/** @var RouteCollection $routes */` at the top
  of the routes file (an inline `@var` naming a variable declares it when nothing in the
  file sets it, in any PHP file), and the closure of `$routes->group()` is given the
  collection.
- A property pattern named `*` is about every property of its classes: the groups of
  `Config\Validation`.

### Names that name nothing

`ReferenceKind::undefined`, as inspections (`inspect/laravel.rs`, `CODEIGNITER`):

| Inspection | Reported when |
| --- | --- |
| `PhpUndefinedCodeIgniterView` | the view's file isn't there (a module's: when its namespace is known) |
| `PhpUndefinedCodeIgniterHelper` | no helpers folder has it (and the framework's folder is known) |
| `PhpUndefinedCodeIgniterLanguageLine` | its file has lines and not this one |
| `PhpUndefinedCodeIgniterRoute` | no route has that name or path; a handler's method isn't there |
| `PhpUnknownCodeIgniterFilter` | no alias, in a routes file or in the filters configuration |
| `PhpUndefinedCodeIgniterController` | a route's handler names a class or a method that isn't there (a controller with `_remap` takes any) |
| `PhpUnknownCodeIgniterRule` | no rule set has the rule (and every rule set is known) |
| `PhpUnknownCodeIgniterRuleGroup` | `Config\Validation` has no such property |
| `PhpUndefinedCodeIgniterModel`, `PhpUndefinedCodeIgniterConfig` | the class isn't there, and for a short name no class of that name is in any namespace (CodeIgniter looks in all of them) |
| `PhpUndefinedCodeIgniterCell` | the same, or the class has no such method |
| `PhpUndefinedCodeIgniterService` | no services class (the application's, the framework's, a module's) has the method |
| `PhpUndefinedCodeIgniterEvent` (off unless turned on) | nothing indexed fires the event: a package may |
| `PhpUnknownCodeIgniterTable` (off unless turned on) | no migration of the project makes the table: it may be made elsewhere |
| `PhpUnknownCodeIgniterColumn` (off unless turned on) | the table is a migration's and has no such field; one plain name only |

A name with a backslash (a class, a module's view) is checked when its string is
single-quoted and has no escape (`ReferenceKind::checks_backslashes`). Not in the
framework's own files. A filter and a rule group aren't reported when a module
has a `Config\Registrar`, which adds them when the application starts.

## Types

- **Helper functions** (`types/provider/codeigniter.rs`): `service('name')` and
  `single_service('name')` are what `Config\Services::name()` returns; `model('BookModel')`
  is `App\Models\BookModel`; `config('App')` is `Config\App`. Only where
  `CodeIgniter\CodeIgniter` is known.
- **A model's finders:** on a model whose `$returnType` names a class, `find()` and
  `first()` are that class or null and `findAll()` a list of it.
- **A view's variables** (`frameworks/codeigniter/views.rs`): `view('books/list', ['books'
  => $books])` makes `$books` a variable of the view, of the type passed. A layout the view
  extends and a view it includes have the same. They reach the view's own inference through
  `Declarations::view_variable`: a view's variables aren't reported as undefined.

## CodeIgniter 3

A project without `spark` whose `index.php` sets `$system_path`, or with
`system/core/CodeIgniter.php` (`CodeIgniterProject::legacy`). The application is in
`application/` and the framework in `system/`, or where `index.php` says
(`$application_folder`, `$system_path`, `$view_folder`).

- **The loader's names**, on the same kinds: `$this->load->view('books/list')`,
  `->helper('url')`, `->model('book_model')` (the class of that name, in any case),
  `->library('session')` (`codeigniterLibrary`: the class, or the framework's `CI_`
  one).
- **Language lines:** `$this->lang->line('key')` and `lang('key')` are the `$lang['key']`
  of `language/<idiom>/*_lang.php`, the application's and the framework's.
- **Configuration items** (`codeigniterConfigItem`): `$this->config->item('key')` and
  `config_item('key')` are the `$config['key']` of `application/config/*.php`.
- **A path is a controller's method** (`codeigniterUri`): `site_url('books/show/3')`,
  `redirect('books/show')`, `anchor()`, `form_open()` go to `Books::show`, folders first.
  A path a route of `config/routes.php` matches goes where the route sends it
  (`$route['products/(:num)'] = 'catalog/lookup/$1'`), and what a route is sent to is
  such a path itself.
- **What the loader gives a controller or a model** (`magic_property`): `$this->load` is
  `CI_Loader`, `$this->db` the query builder, `$this->session` the class `CI_Session` (any
  `CI_<Name>`), `$this->book_model` the model `Book_model`, `$this->books` the model
  loaded as `->model('book_model', 'books')` (`DeclaredKind::CodeIgniterLoaded`), and
  `$this->basket` the class of `application/libraries/Basket.php`
  (`Declarations::loaded_class`), under another name when it was loaded so
  (`->library('basket', NULL, 'cart')`).
- **The database:** the tables and fields of the migrations
  (`$this->dbforge->add_field()`, `create_table()`, `add_column()`) are the names of
  `$this->db->get('books')`, `from()`, `insert()`… and of `where('title', …)`,
  `order_by()`, `select()`. The builder doesn't say its table until the query is run: a
  column's table is the one written before it, the first argument of `insert()` and
  `update()`, else a table the function around names that has the column. The undefined-property
  inspection takes a property a provider types as defined.
- **A view** has what `$this->load->view('name', array(...))` passes it, and `$this` in it
  has what a controller has.

## Tests

- `navigate/tests/codeigniter.rs`, `codeigniter/more.rs` and `codeigniter/legacy.rs`: every
  feature, on stand-ins for CodeIgniter's API written for the tests.
- `tests/codeigniter_fixture.rs` on `fixtures/codeigniter-app`: nothing in `app/` is
  reported, and one name of each kind goes where it should.
- `phoxite-lsp/src/tests.rs`: the fixture through the protocol.
- `editors/vscode/test/e2e` runs once on the fixture in VS Code (`codeigniterChecks`;
  `PHOXITE_E2E_ONLY=codeigniter`).
- `tests/codeigniter_corpus.rs` (ignored; `scripts/php-corpus.sh codeigniter
  codeigniter3`): CodeIgniter 4.6 itself (names, types, a rule and an event of the
  framework; nothing reported in its `app/` and `system/`, and what the inspections say of
  its tests and of the user guide's examples printed to be read) and CodeIgniter 3.1.13
  itself (275 language lines, 56 configuration items, the loader's names, what a
  controller has, nothing reported in `application/`).

## Not done

- **A builder whose table isn't a string literal** (`$db->table($this->table)`): its
  columns aren't known.
- **The fields other rules name** (`matches[password]`, `required_with[email]`): the other
  keys of the same array.
- **`only` and `except`** of `resource()` aren't applied to the routes it makes.
- **Paths and namespaces** written with a variable or a constant of the application.
- **HMVC extensions** of CodeIgniter 3 (third-party: `$this->load->module()`).
