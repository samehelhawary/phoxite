# Drupal

What the engine knows about a Drupal project (8 and later), where the code is, and what it
doesn't do. The user-facing list is in `editors/vscode/README.md`; how a framework plugs
into the engine is in [`frameworks.md`](frameworks.md).

## Nothing is bundled

Drupal is GPL and this engine is Apache-2.0. The engine has no copy of Drupal's code, of
its documentation or of a list of its hooks, services or routes. It knows:

- the names of the classes and methods of Drupal's public API that take a name
  (`Url::fromRoute()`, `EntityTypeManagerInterface::getStorage()`, …) and the names and
  layout of the YAML files a module has, as Drupal's documentation describes them;
- whatever the project's files say: Drupal's own when the project has it (`core/`), or a
  folder named in `includePaths`.

A module opened on its own has its own names, and none of the core's.

## Detection and file types

`frameworks/detect.rs`: Composer installing `drupal/core` or `drupal/core-recommended`;
`core/lib/Drupal.php` (also below `web/` and `docroot/`); or a `*.info.yml` with a core
requirement at the root (a module or theme on its own). The `frameworks` setting forces
it on or off.

`.module`, `.install`, `.theme`, `.profile`, `.engine` and `.inc` are PHP
(`project::PHP_EXTENSIONS`), in every project: a file of such a name that isn't PHP has no
`<?php` and declares nothing. The server watches them, and the VS Code extension
associates them with PHP.

## The YAML files

`frameworks/drupal/project.rs` (`DrupalProject`, kept by the engine until a YAML file
changes) finds these files by name anywhere in the project:

| File | Read for |
| --- | --- |
| `*.services.yml` | services and parameters (Symfony's format, read by `SymfonyConfig`) |
| `*.routing.yml` | route names and paths |
| `*.permissions.yml` | permission names and titles |
| `*.libraries.yml` | library names: `<extension>/<key>` |
| `*.info.yml` | the modules, themes and profiles |
| `config/install/*.yml`, `config/optional/*.yml` | configuration names: the file's name |
| `*.links.{menu,task,action,contextual}.yml`, `*.layouts.yml`, `*.breakpoints.yml`, `migrations/*.yml` | the plugins they declare (menu links, local tasks and actions, contextual links, layouts, breakpoints, migrations by their `id`); usages of route names |
| `config/schema/*.yml` | the configurations it describes and their keys, for one no file installs (`node.type.*` describes `node.type.article`) |
| `<name>.info` | Drupal 7's modules and themes |

`vendor`, `node_modules`, `tests` and folders starting with a dot aren't looked into: a
test module's routes aren't the site's. On Drupal 11's own tree that is 884 files, read in
about 0.4 s once.

## From PHP

Recorded as name facts (`summary/extract/names.rs`, see `wordpress.md` for the mechanism):

- **Entity types:** the `id` of a `#[ContentEntityType]`, `#[ConfigEntityType]` or
  `#[EntityType]` attribute, or of the same annotation in the class's docblock, with the
  entity's class and the `storage` handler it names.
- **Plugins:** the `id` of the attribute or annotation of a class in a `Plugin` namespace,
  with the attribute's short name as the plugin's type (`Block`).
- **Theme hooks:** the keys of the array a function named `…_theme()`, or a method with
  `#[Hook('theme')]`, returns, each with the variables its template is given (the keys of
  `variables`, or the `render element`).
- **Hook methods:** a method with `#[Hook('name')]` is recorded as a callback of
  `drupal:name`; on a class, the method the attribute names (`method: 'run'`), else
  `__invoke`.

Those a file below a `tests` folder declares aren't offered.

Hooks are the functions named `hook_…` of the global namespace, which the `*.api.php`
files declare (`Declarations::hook_functions`, read from the index).

## Names

`frameworks/drupal/refs` (`DrupalName` for all but hooks):

| Kind | Declared by | Named in |
| --- | --- | --- |
| `drupalService` | a key of `services:` | `\Drupal::service()`, `$container->get()`; `@id`, `alias:` and `parent:` in services files |
| `drupalRoute` | a key of a routing file | `Url::fromRoute()`, `new Url()`, `Link::createFromRoute()`, `redirect()`, `setRedirect()`, `generateFromRoute()`; `route_name:`, `base_route:` of links files |
| `drupalPermission` | a key of a permissions file | `hasPermission()`, `AccessResult::allowedIfHasPermission()` and `…Permissions()`; `_permission:` of routing files |
| `drupalLibrary` | a key of a libraries file | `'#attached' => ['library' => […]]`, `$build['#attached']['library'][] = …`; the lists of info and libraries files; `attach_library()` in a template |
| `drupalConfig` | a file of `config/install` or `config/optional` | `\Drupal::config()`, `$this->config()`, the config factory's `get()` and `getEditable()` |
| `drupalConfigKey` | a key of that file, nested ones with dots | `->get('…')`, `->set('…')`, `->clear('…')` on the configuration |
| `drupalModule` | a `*.info.yml` | `moduleExists()`, `getModule()` |
| `drupalEntityType` | an entity type's attribute or annotation | `getStorage()`, `getDefinition()`, `getViewBuilder()` and the other handlers; `\Drupal::entityQuery()` |
| `drupalPlugin` | a plugin's attribute or annotation; a key of a links or layouts file | `createInstance()`, `getDefinition()`, `hasDefinition()` of a plugin manager |
| `drupalThemeHook` | a key of what `hook_theme()` returns | `'#theme' => '…'`, `$build['#theme'] = '…'` |
| `drupalHook` | a `hook_…()` function | `invokeAll()`, `invoke()`, `hasImplementations()`, `alter()`, `#[Hook('…')]` |

All have completion, goto, hover and Find References.

- **Strings under a key of an array** are a kind of place the shared machinery gained for
  this: `CalleePattern::Entry("#theme")` is the value under that key of any array literal,
  and of `$x['#theme'] = …`, with the positions below it (`#attached` → `library` → an
  item).
- **Rename** is for services (not those whose id is a class name), routes, permissions and
  libraries (the part after the extension's name), when no declaring file is below a
  `core` or `contrib` folder or in a library. The YAML usages change with the key.
- **Plugins** are offered by type when the manager's name says it (`BlockManagerInterface`
  offers the `Block` plugins), else all of them.
- **A theme hook** also goes to its template: `<name>.html.twig` with dashes, below the
  `templates` folder of the extension declaring it. A suggestion (`node__article`) is its
  base hook's, with its own template first when there is one. The functions
  `<extension>_preprocess_<hook>()` and `template_preprocess_<hook>()` are usages of it.
- **A derivative plugin** (`system_menu_block:main`) is its base plugin's.
- **A configuration's keys** (`refs/config_key.rs`) need the configuration's name: the
  type provider types `\Drupal::config('shop.settings')`, `$this->config()` and the
  factory's `get()` and `getEditable()` as the configuration class of that name, which a
  variable keeps. Not renamed: a site's stored configuration has the key too.
- **From an implementation to its hook:** Go to Definition on the name of a function
  `<module>_<hook>()` goes to the hook's function (`PhpEngine::definition`).
- **Hooks:** `alter('form')` names `hook_form_alter`. A hook documented with a placeholder
  (`hook_ENTITY_TYPE_presave`, `hook_form_FORM_ID_alter`) is found from a name that fits
  it. Find References lists the functions implementing a hook: `<extension>_<name>()` for
  each extension of the project.
- **No "unknown" inspection:** a module opened on its own doesn't have the core's files.

## Types

- **Services** (`types/provider/drupal.rs`): `\Drupal::service('id')` and
  `$container->get('id')` are the class the id's definition makes, through aliases and
  parents. The engine adds Drupal's services to the configuration the Symfony provider
  already answers from (`PhpEngine::symfony_config`).
- **Configuration keys:** `->get('key')` on a configuration of a known name has the
  type its schema gives the key (`integer`, `boolean`, `string` and the string-like
  types, `mapping` and `sequence` as arrays). Where no schema says, the type of the
  value a file installs (`DrupalProject::installed_types`).
- **Entity storages:** `getStorage('node')` is the storage handler the entity type names
  (else `EntityStorageInterface`) of the entity's class; on it `load()`,
  `loadUnchanged()` and `loadRevision()` are `?Node`, `create()` is `Node`, and
  `loadMultiple()` and `loadByProperties()` are lists of it.
- **Hook implementations** (`Resolver::drupal_implemented_hook`): a parameter nothing
  types of a function `<module>_<hook>()` has the type the hook's function gives that
  parameter, by declaration or `@param`. The hook is the longest end of the function's
  name that is one, an exact name before a placeholder's. The same for a method with
  `#[Hook('name')]`. Only in a project that has `hook_…` functions.

## Twig

The `templates` folder of each module and theme is a namespace of its name
(`twig/templates.rs`): `{% include '@shop/shop-book.html.twig' %}` goes to the module's
file.

A template has the variables its theme hook declares (`shop-book.html.twig` is
`shop_book`; `node--article.html.twig` its base hook), and those Drupal gives every
template (`attributes`, `title_prefix`…): completion offers them
(`frameworks/drupal/templates.rs`). A variable a preprocess function assigns
(`$variables['book'] = …` in `<extension>_preprocess_<hook>()`) has the type of what is
assigned, so `{{ book.title }}` completes; such a function also adds variables. So has
one a render array passes: `['#theme' => 'shop_book', '#book' => $book]`, or built up in
one function (`$build['#theme'] = 'shop_book'; $build['#book'] = $book;`), or returned
by a function and added to (`$build = shop_build(); $build['#book'] = $book;`), in the
same file or another.

The string of `attach_library('…')` is a library: goto, hover, completion and usages.

## The YAML files as documents

`frameworks/drupal/documents.rs`, the server's `Kind::DrupalYaml`. The server registers
for `**/*.yml` while a Drupal project is open (`Server::register_documents`), so no
client lists them. A file is read by what it is, line by line:

| Where | What |
| --- | --- |
| `@id`; `alias:` and `parent:` of a services file | a service |
| `route_name:`, `base_route:` of a links file | a route |
| `parent:`, `parent_id:` of a links file | a plugin |
| `_permission:` of a routing file | each permission it lists |
| a list item of an info or a libraries file | a library; `drupal:node` is the module `node` |
| a class's name, with `::method` or not | the class or the method |
| a key declaring a service, a route, a permission, a library or a link | that name |

Each has goto, hover, Find References and Rename through the name's reference kind, and
completion by the line's key.

A line without a key of its own is read under the key above it (`parent_key`): an item of
its list, an entry of its bracketed list on a later line, or the value on the next line.
In a mapping written in braces, the key before the caret is the one (`{ route_name:
shop.books }`). An alias (`*defaults`) goes to its anchor; the YAML reader
(`base/yaml.rs`) puts what an alias stands for in its place and merges `<<: *defaults`,
so a service whose class comes from an anchor has it. `shop.controller:list` is a
service and a method of its class. After `class:`,
`_controller:` and `_form:`, completion offers the namespaces and classes below what is
written so far.

`*.api.php` files are example code: `PhpUnusedParameter` says nothing in them.

## Drupal 7

A site with `modules/system/system.module`, or a module whose `<name>.info` says
`core = 7.x`, is a Drupal project too. Its hooks are the same `hook_…()` functions, so
the types of an implementation's parameters and the goto from an implementation work as
they are. The kinds take its functions beside Drupal 8's methods:

| Kind | Declared by | Named in |
| --- | --- | --- |
| `drupalHook` | a `hook_…()` function | `module_invoke_all()`, `module_implements()`, `module_invoke()`, `module_hook()`, `drupal_alter()` |
| `drupalThemeHook` | a key of `hook_theme()` | `theme('…')`; it also goes to `theme_<hook>()` |
| `drupalPermission` | a key of what `hook_permission()` returns (`DeclaredKind::DrupalPermission`) | `user_access()` |
| `drupalModule` | a `<name>.info` | `module_exists()`; `dependencies[]` of an `.info` |
| `drupalMenuPath` | a key of what `hook_menu()` returns or fills (`$items['admin/shop'] = array(…)`) | `drupal_goto()`, `url()`, `l()`, `menu_get_item()`; `node/12/edit` is `node/%node/edit` |
| `drupalVariable` | `variable_set()` | `variable_get()`, `variable_del()` |
| `drupalTable` | a key of what `hook_schema()` returns or fills (Drupal 8 and later too) | `db_select()`, `db_insert()`, `db_update()`, `db_delete()`, `db_merge()`, `drupal_write_record()`; the connection's `select()`, `insert()`… and a query's `join()` |

| `drupalColumn` | a key of the `fields` of a table's entry (`DeclaredKind::DrupalColumn`, as `table.column`) | `->condition('nid', …)`, `->fields('n', array('nid'))`, `->orderBy()`, `->groupBy()`, `->isNull()` on a query; `->fields(array('title' => …))` of an insert or an update |

A column needs its table: the type provider types `db_select('node', 'n')` and the
connection's `select()`, `insert()`, `update()`, `delete()` and `merge()` as the query
class of that table, which a variable keeps and which goes on along a chain
(`db_select('node')->fields(…)->condition('nid')`: Drupal documents the query's class as
what those methods return, so the provider gives the receiver's table to the result). A
column may be written after an alias (`n.nid`); the alias of a joined table (`u.uid`
after `->join('users', 'u', …)`) is that table's, by the calls of the function around the
string (`refs/column.rs`).

A form is its function: the string of `drupal_get_form('shop_form')` is a callable. So
are a menu item's `'page callback'` and `'access callback'`, as any key named as a
callback in any array (`navigate/callable.rs`). An `.info` file is a document like the
YAML ones: `dependencies[] = node` is a module, `files[] = shop.test` a file.

On Drupal 7.103 itself (`tests/drupal_corpus.rs`): 55 extensions, 364 hooks, 74
permissions, 175 theme hooks, 494 menu paths and 205 variables are read.

## Tests

- `navigate/tests/drupal.rs`: every feature, on a stand-in for Drupal's API written for
  the tests.
- `tests/drupal_fixture.rs` on `fixtures/drupal-app`, a module and a theme on a stand-in
  API: nothing in them is reported, and one name of each kind goes where it should.
- `phoxite-lsp/src/tests.rs`: the fixture through the protocol, a route added outside the
  editor, and a template's namespace.
- `editors/vscode/test/e2e` runs once on the fixture in VS Code (`drupalChecks`, 10 checks;
  `PHOXITE_E2E_ONLY=drupal`).
- `tests/drupal_corpus.rs` (ignored; `scripts/php-corpus.sh drupal`): Drupal 11 itself.
  1,189 services, 333 routes, 111 permissions, 324 libraries and 87 extensions are read
  from its YAML files, and 99 entity types, 1,027 plugins and 297 hooks from its PHP (its
  tests' included in those three counts); the names resolve, and a service, a storage and
  a hook's implementation are typed from its own files.

## Not done

What is left can't be read from the project:

- Drupal 7's own documentation rarely types a hook's parameters (`@param $node`, with a
  description), so an implementation's are seldom typed. Nothing in the code says more,
  and guessing a class from a parameter's name would be wrong often enough to mislead.
- A table joined in another function than the one writing the column, a query passed as
  an argument, and a render array a method (not a function) builds for another to fill
  aren't followed: each would need the flow of values across calls.
