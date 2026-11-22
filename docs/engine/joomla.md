# Joomla

What the engine knows about a Joomla site or an extension opened on its own, where the
code is, and what it doesn't do. How a framework plugs into the engine is in
[`frameworks.md`](frameworks.md).

Joomla is GPL-2.0-or-later. Its manual is the design input; nothing of it or of Joomla's
source is in this repository. The target is Joomla 6 (6.2.0, the fixture's shape and the
corpus). Joomla 5.4 and 4 have the same namespaced API and extension layout, so most of
this works there, but nothing is checked on them. Joomla 3's unnamespaced classes
(`JFactory`, `JText`) are not known.

## Detection

`frameworks/detect.rs`. No Composer package names a Joomla site, so by its files:
`libraries/src/Version.php` (the CMS library's version class), or an extension opened on
its own: an XML file at the top of the project that is a manifest
(`<extension type="component">`). The `frameworks` setting forces it on or off.

## The INI reader

`frameworks/joomla/ini.rs`: a language file's `KEY="value"` lines, each with where the
key is written. `;` starts a comment; a quote inside a value is `\"`; a file of an older
extension may leave the quotes out. A line that isn't a `KEY=` is skipped.

## The project model

`frameworks/joomla/project.rs` (`JoomlaProject`, `PhpEngine::joomla()`):

- **Extensions:** components (`components/com_*`, `administrator/components/com_*`,
  `api/components/com_*`), modules (`modules/mod_*`, `administrator/modules/mod_*`),
  plugins (`plugins/<group>/<name>`) and templates (`templates/*`,
  `administrator/templates/*`). A component has one folder per client; its manifest is in
  its administrator folder. Each folder has the namespace its manifest gives
  (`<namespace path="src">`), a component's and a module's with the client after it:
  `Joomla\Component\Content\Site`, `…\Administrator`, `…\Api`.
- **An extension opened alone:** a project whose own folder has a manifest is that
  extension, laid out as its package is (`<files folder="site">`,
  `<administration><files folder="admin">`, or the folder itself for a module, a plugin
  and a template).
- **Language constants:** the lines of `language/<tag>/*.ini`, the site's, the
  administrator's, the API's and an extension's own. British English is first where a
  constant has several declarations.
- **Layouts:** the files of `layouts/` (`joomla.content.tags` is
  `layouts/joomla/content/tags.php`), a component's `layouts/`, and a template's
  overrides in `html/layouts/`, which come first.
- **Web assets:** the names of each `joomla.asset.json` (`media/*/`, `media_source/*/`, a
  template's), with their type and their dependencies.
- **ACL actions:** `access.xml`'s `<action name title>`.
- **Forms:** each XML file of a `forms` folder, with its fields; **parameters:** the
  fields of `config.xml` and of a manifest's `<config>`.
- **Events:** the event class of each name, from the map of
  `Joomla\CMS\Event\CoreEventAware` (154 in 6.2). An event with a variable name isn't in
  the map and has no class here.
- **Tables:** the `CREATE TABLE` statements of the MySQL scripts
  (`installation/sql/mysql`, an extension's `sql/`), with their columns, under the name
  they are written with (`#__content`).
- **Every name an XML or asset file writes** (`Mention`): language constants (the
  labels, descriptions, titles and hints of a form, of `config.xml`, of `access.xml`, of
  a menu item's options; a manifest's name, description and menu texts), field types
  with the namespaces `addfieldprefix` gives, classes written with their namespace,
  layouts, ACL actions, asset dependencies.

Nothing is read per keystroke: `JoomlaCache` keeps what each file declares with the
file's modification time and the project put together from them; the project is dropped
when one of these files is written (`PhpEngine::files_changed`).

**A manifest's namespace is a PSR-4 root.** No `composer.json` names an extension's
namespace, so where the autoload map has nothing for a folder, the extension's manifest
says (`PhpEngine::joomla_namespace_of_dir`): a class file moved inside an extension gets
the namespace of its new folder. (Finding a class needs none of this: the index has every
class under the namespace its file declares.)

## Names

`frameworks/joomla/refs.rs` (one type, `JoomlaName`):

| Kind | Names | Where |
| --- | --- | --- |
| `joomlaLanguage` | a language constant, where a `.ini` file declares it | `Text::_('COM_X_Y')`, `sprintf()`, `plural()`, `script()`, `alt()`, and `_()` on a language object |
| `joomlaLayout` | a layout's file, a template's override first | `LayoutHelper::render('joomla.content.tags')`, `debug()`, `new FileLayout('id')`, `sublayout()` |
| `joomlaHtml` | an HTML helper's method: `behavior.core` is `core()` of `Joomla\CMS\HTML\Helpers\Behavior` | `HTMLHelper::_('behavior.core')` |
| `joomlaAsset` | a web asset of a `joomla.asset.json` | `useScript()`, `useStyle()`, `usePreset()`, their `register…` and `disable…` forms, `useAsset()`, `getAsset()` |
| `joomlaExtension` | a component, a module or a plugin: its manifest | `bootComponent('com_content')`, `ComponentHelper::getParams()`, `ModuleHelper::getModule()`, `PluginHelper::getPlugin('content', 'pagebreak')` |
| `joomlaEvent` | an event: its class | `$dispatcher->dispatch('onContentPrepare', …)`, the keys of `getSubscribedEvents()` |
| `joomlaHandler` | the method handling an event | the values of `getSubscribedEvents()` |
| `joomlaAction` | an ACL action of an `access.xml`, the component's own first | `$user->authorise('core.edit', …)` |
| `joomlaForm` | a form: `forms/<name>.xml` of the extension | the second argument of `loadForm('com_content.article', 'article')` |
| `joomlaField` | a field of the extension's forms | `$form->getValue('title')`, `setValue()`, `getField()`, `getInput()`, `getLabel()`, `setFieldAttribute()`, `removeField()` |
| `joomlaParam` | a parameter of `config.xml` or a manifest | `$params->get('show_title')` on a registry |
| `joomlaTable` | a table of the SQL scripts | `'#__content'` given to a query's `from()`, `join()`, `update()`…, to `quoteName()`, to a table class's constructor; an alias after it is left out |
| `joomlaColumn` | a column of the table named before it | the key a table class's constructor is given |
| `joomlaTemplate` | a view's sub-template: `default_links.php` beside `default.php`, a template's override first | `$this->loadTemplate('links')` in a view's template |
| `joomlaRoute` | the parts of a URL | `Route::_('index.php?option=com_content&view=article&task=article.save')`, `Route::link()` |

All have completion, goto, hover and Find References; the XML and asset files naming a
constant, a layout or an asset are listed with the PHP.

- **A URL** is read part by part, by where the caret is: `option=` is the component
  (its manifest), `view=` the view's class in that component (`…\View\Article\HtmlView`,
  the client being the file's own), `task=controller.method` the controller's method.
  Without an `option`, the component the file is in.
- **A field, a parameter, a form and an ACL action** are looked for in the extension the
  file is in first, then anywhere.
- **Completion of a language constant** is by what is written (there are thousands);
  before anything is, the constants of the file's own extension.

**Renamed:** a language constant, in PHP, in the language files that declare it and in
the XML naming it (`rename_target`), where the project's own files declare it. Nothing
else: the other names are file names, class names or parts of them.

### Names that name nothing

`ReferenceKind::undefined`, as inspections (`inspect/laravel.rs`, `JOOMLA`):

| Inspection | Reported when |
| --- | --- |
| `PhpUndefinedJoomlaLanguage` | a constant starting with the prefix of the extension the file is in (`COM_CONTENT_`, `PLG_CONTENT_PAGEBREAK_`) that no language file has, where that extension has language files |
| `PhpUndefinedJoomlaLayout` | one of Joomla's layouts (`joomla.…`) that isn't there, where the project has those |

A constant of another extension or of Joomla itself (`JYES`) is never reported: it may be
in a language pack that isn't in the project. The other kinds are not reported either:
an asset can be registered at run time, an event dispatched for plugins that aren't
installed, a table made by an older script. Not in the files of `vendor/`.

## Types

`types/provider/joomla.rs`, only in a Joomla project (`TypeProvider::applies`):

- **The application:** `Factory::getApplication()` is `CMSApplication`, so
  `getDocument()`, `getIdentity()`, `getInput()` and the rest are there (the interface it
  is documented to return has part of them).
- **A component:** `bootComponent('com_content')` is the component's extension class
  (the class of its administrator namespace's `Extension` folder ending in `Component`).
- **The MVC factory:** `createModel('Article', 'Administrator')` and
  `createTable('Article')` are `<namespace>\<Client>\Model\ArticleModel` and
  `…\Table\ArticleTable`, in the component the calling class is of; without a client,
  the calling class's, then the site's, then the administrator's. Outside a component's
  classes, the one class of that name the project has.
- **A controller and a view:** `$this->getModel('Article')` the same; in a view,
  `$this->getModel()` is the model named after the view and `$this->get('Items')` is what
  that model's `getItems()` returns.
- **A container:** `$container->get(Foo::class)` is `Foo`.
- **An event handler's parameter** that nothing types, or that is declared as the base
  event: the event class of the name its class subscribes the method to
  (`TypeProvider::parameter`; the `'onEvent' => 'handler'` pairs of
  `getSubscribedEvents()` are read from the class's text, `views::subscribed`), else of
  the method's own name.
- **A plugin's application:** `$this->getApplication()` is typed by its declaration;
  nothing is added.

**A template's variables** (`frameworks/joomla/views.rs`, through
`Declarations::view_variable`):

| File | Variables |
| --- | --- |
| a view's template, `tmpl/<view>/<layout>.php` of a component | `$this` is the view: `<namespace>\View\<View>\HtmlView` (the feed, JSON or raw view when there is no HTML one), else Joomla's `HtmlView` |
| its override in a template, `html/com_x/<view>/<layout>.php` | the same view |
| a module's template, `tmpl/*.php`, and its override `html/mod_x/*.php` | `$module`, `$app`, `$input`, `$params`, `$template` |
| a layout | `$this` is `Joomla\CMS\Layout\FileLayout` |
| a site template's `index.php`, `component.php`, `error.php`, `offline.php` | `$this` is `Joomla\CMS\Document\HtmlDocument` |

A template is included by a method of that object, so its code runs in the class: the
view's protected properties (`$this->item`) are reachable and complete
(`ExprTypes::view_class`).

## XML, language and asset files as documents

`frameworks/joomla/documents.rs`, the pattern of Drupal's YAML files: the
`joomla_document_*` functions take the file's path, its open text and a position.

| Name | Where | Answer |
| --- | --- | --- |
| a language constant | a label, a description, a title or a hint in a form, `config.xml`, `access.xml`, a menu item's options; a manifest's texts; the key of a `.ini` line | `joomlaLanguage`: its declaration, its usages in PHP and the other files, its rename |
| a field's type | `type="…"` of a form's field | the field's class: in the namespaces `addfieldprefix` gives (`modal_article` is `Modal\ArticleField`), then Joomla's `Form\Field` |
| a class | a rule's, a filter's or a field's class written with its namespace | the class |
| a layout | `layout="joomla.form.field.radio.switcher"` | `joomlaLayout` |
| an ACL action, a component | `access.xml` | `joomlaAction`, `joomlaExtension` |
| a web asset | a name or a dependency of a `joomla.asset.json` | `joomlaAsset` |

Goto, hover, Find References and completion for each; rename for a language constant.
Find References on a field's class (from PHP or from the XML) lists the fields of that
type. Completion offers the constants starting with what is written, the field types the
form's namespaces and Joomla have, the layouts, the assets.

The server (`phoxite-lsp`): `Kind::JoomlaFile` for an `.xml`, an `.ini` or a
`joomla.asset.json` that nothing else claims; the engine says whether the project is a
Joomla one. The client is asked for `**/*.xml`, `**/language/**/*.ini` and
`**/joomla.asset.json` once the project is known to be Joomla (ids `phoxite/joomla/…`),
so no other project sends them.

## Tests

- `frameworks/joomla/{ini,project,views}.rs`: unit tests.
- `navigate/tests/joomla.rs` (10 tests), on a stand-in for Joomla's API.
- `fixtures/joomla-app` (a component with both its folders, a plugin, a module, a site
  template with overrides; `libraries/joomla.php` as the stand-in) and
  `tests/joomla_fixture.rs`.
- The server: `a_joomla_site_names_what_its_files_declare` (`phoxite-lsp/src/tests.rs`).
- VS Code: the `joomla` part of the end-to-end suite (`joomlaChecks`;
  `PHOXITE_E2E_ONLY=joomla`).
- `tests/joomla_corpus.rs` (ignored): below.

## On Joomla itself

`tests/joomla_corpus.rs`, on Joomla 6.2.0 (3,297 PHP files, 668 XML files, 663 language
files) with the framework packages it installs as include paths (registry, DI, event,
database, input), measured 2026-11-23 on the development machine, release build:

- **The site as a project:** indexed in 2.4 s. The model reads 1,145 files: 285
  extension folders, 13,854 language constants, 199 layouts, 216 web assets, 220 ACL
  actions, 332 forms, 154 events and 102 tables.
- **The model's cost:** 395 ms to read from nothing, under a microsecond when kept,
  270 ms after one language file changed. It is paid on the first request after a change
  to one of these files, never per keystroke.
- **Names and types in core code:** a subscriber's event and its handler
  (`plugins/content/pagebreak`), a constant through `Text` and through the language
  object, a view's template (`$this->item`, a protected property of the article view),
  an event handler's parameter, the application.
- **Joomla's inspections** on `com_content` (both folders), the content plugins and
  `mod_custom` (105 files, 3.0 s): one report, and it is right —
  `administrator/components/com_content/src/Service/HTML/Icon.php` renders
  `joomla.content.icons.print_screen`, a layout Joomla 6.2 doesn't have. The other
  inspections make 408 reports there: the framework packages the corpus doesn't have
  leave classes unknown.

## Not done

What cannot be read without running the site:

- A layout, a template, a form or a component named by a variable.
- An event with a variable name (`onContentPrepareForm` is in the map; `on<Type>…`
  built from a string isn't), and what a plugin that isn't installed listens to.
- What `AdminModel::getItem()` returns: an object whose fields are the table's columns
  at run time. A table's columns are known for goto and completion of its key only.
- Which template is active, and so which override wins: every template's is listed.
- A web asset registered in PHP at run time; a language constant of a language pack
  that isn't in the project.
- A module's own variables (`getLayoutData()`): only the five every module template has.

Not planned, as the plan says: the JavaScript side that 6.1 and 6.2 added, warnings for
the classes Joomla 6 removed (they are unknown classes already), and Joomla 3.
