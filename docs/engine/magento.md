# Magento 2

What the engine knows about a Magento 2 project, where the code is, and what it doesn't
do. How a framework plugs into the engine is in [`frameworks.md`](frameworks.md).

Magento Open Source is OSL-3.0. Its developer documentation is the design input; nothing
of it or of its source is in this repository. The target is Magento Open Source 2.4.9.

## Detection

`frameworks/detect.rs`: an `app/etc/di.xml` file, Composer installing
`magento/framework` or one of the product packages, or a module opened alone (a
`registration.php` beside an `etc/module.xml`). The `frameworks` setting forces it on or
off.

## The XML reader

`base/xml.rs`, written for this: `parse(text)` gives the elements of a document, each
with its attributes, its children and its text, and the range of each name, attribute
value and text. No dependency was added; the engine reads YAML the same way
(`base/yaml.rs`).

It never fails. A file being edited reads as far as it makes sense: an element that is
never closed ends with the document, a closing tag that closes nothing is skipped, a tag
being typed ends where the next one starts. Comments, processing instructions and the
document type are skipped; a CDATA section is text. The five entities and character
references are decoded. Namespaces are not resolved: `xsi:type` is an attribute of that
name.

## The project model

`frameworks/magento/project.rs` (`MagentoProject`, `PhpEngine::magento()`):

- **Modules:** a folder with a `registration.php` under `app/code/*/*` or `vendor/*/*`,
  or the project itself, named as its registration names it (`Vendor_Module`).
- **Themes:** the folders of `app/design/<area>/<Vendor>/<theme>` and the themes
  Composer installed (a registration naming `<area>/<Vendor>/<theme>`), each with the
  parent its `theme.xml` names.
- **`di.xml`** (`app/etc/di.xml`, each module's `etc/di.xml` and `etc/<area>/di.xml`):
  preferences, virtual types (one of a virtual type is followed to its class), plugins
  that aren't disabled, each with the area of its file.
- **`events.xml`:** each event's observers.
- **`acl.xml`:** resources, with their titles.
- **`adminhtml/system.xml`:** fields as `section/group/field`, groups inside groups
  too, with their labels. **`config.xml`:** the defaults, as paths, with their values.
- **`extension_attributes.xml`:** each interface's attributes, with their types.
- **`routes.xml`:** each route's id, front name, area and modules.
- **`db_schema.xml`:** each table's columns, with their types.
- **Layout** (`view/<area>/layout/*.xml`, and a theme's
  `Vendor_Module/layout/*.xml`): a file is a handle; its blocks and containers, each
  block with its class, its alias (`as`), its template and the class of its
  `view_model` argument. A `referenceBlock` that sets a template, a class or a view
  model is kept for the block it refers to.
- **The other files of `etc`** (`widget.xml`, `indexer.xml`, `cache.xml`, `sales.xml`…,
  `XmlKind::Other`): read for what they name only — every attribute and text that is a
  class's name with its namespace, and the modules a `module` attribute names. Nothing
  is reported in them.
- **Every name a file writes** (`Mention`, with what it is, where it is written, and
  whether it is declared there): the classes, events, ACL resources, configuration
  paths, templates, handles, blocks, modules, tables and routes of the files above, and
  those of `webapi.xml` (a service's class and method, its resources), `crontab.xml` (a
  job's class and method), `communication.xml` and `queue_consumer.xml` (a handler's),
  `menu.xml`, `module.xml` (the module and those it is loaded after), UI components
  (`view/<area>/ui_component/*.xml`) and GraphQL schemas (`etc/schema.graphqls`, scanned
  for `class: "…"`, with its backslashes doubled). They are what Find References and a
  rename reach in these files, and what the caret is on in one.

`XmlKind::of(path)` says which of these a file is, by its name and its folder; the
loader and the server both ask it.

**Areas** (`area_of`): a file is of the area its folders say — `etc/<area>/`,
`view/<area>/`, `app/design/<area>/`, a class below `Adminhtml` — or of none. Where a
preference or a virtual type is asked for with an area (`preferred(of, area)`), the
area's `di.xml` wins over the global ones, and the global ones over another area's; the
last declared wins among equals. A PHP file outside those folders has no area, and gets
the global answer.

**Themes** (`theme_chain`, `theme_parents`): a template is looked for in the themes of
its area, a theme before the themes it inherits from, then in the module's folder for
the area, then in `view/base`.

Magento is large, so nothing here is read per keystroke. `MagentoCache` keeps what each
file declares with the file's modification time, and the project put together from
them. The project is dropped when a watched file or a `registration.php` is written,
created or removed (`PhpEngine::files_changed`; the server watches `**/etc/**/*.xml`,
`**/layout/*.xml`, `**/ui_component/*.xml`, `**/etc/**/schema.graphqls` and
`**/theme.xml`); putting it together again reads only the files whose time changed. The
other frameworks' models are kept per edit count; this one is not.

## Generated classes

`frameworks/magento/generated.rs`. Magento writes some classes on demand, into
`generated/`. A project that never ran `setup:di:compile`, or keeps that folder out of
the editor, still has them:

| Class | Is |
| --- | --- |
| `FooFactory` | `create(array $data = []): Foo`, for a class or an interface `Foo` the project has |
| `Foo\Proxy`, `Foo\Interceptor` | a subclass of `Foo`; for an interface, a class implementing it |
| `FooExtensionInterface` | a getter and a setter per attribute `extension_attributes.xml` declares for `FooInterface`, with the declared type (`Vendor\Type[]` kept) |
| `FooExtension` | the class implementing it |

Each is written as PHP and read like a file, so it is a class like any other: it
completes, it is a type, hover and its members work, and `PhpUndefinedClass` doesn't
report it. `PhpEngine::class_like` asks for one last, after the project's files, the
stubs and Laravel's aliases, so a real class of that name (in `generated/`, or a factory
written by hand) is the one found. The answer is kept per edit count
(`PhpEngine::magento_generated`); only a name of one of these shapes is asked about
(`generated::is_candidate`).

Goto on such a class has nowhere to go: there is no file.

## Names

`frameworks/magento/refs.rs` (one type, `MagentoName`):

| Kind | Names | Where |
| --- | --- | --- |
| `magentoEvent` | an event, where an `events.xml` observes it | `dispatch('name')` of the event manager |
| `magentoConfigPath` | a configuration path: its field in `system.xml`, its default in `config.xml` | `getValue('a/b/c')`, `isSetFlag()` of the scope configuration |
| `magentoAcl` | an ACL resource of an `acl.xml`, where it has its title | `isAllowed('Vendor_Module::res')`, `const ADMIN_RESOURCE` of a backend action |
| `magentoTemplate` | a template's file, a theme's version first | `setTemplate('Vendor_Module::x.phtml')`, a block's `$_template` |
| `magentoHandle` | a layout handle: the layout files of that name | `addHandle('name')` |
| `magentoBlock` | a block or a container, by its name, else by the alias its parent has it under | `getBlock('name')`, `getChildBlock()`, `getChildHtml('alias')` |
| `magentoModule` | a module, where its `registration.php` names it | `isEnabled('Vendor_Module')`, `getDir()`, `getModuleDir()`, and the registration itself |
| `magentoRoute` | a URL path `front/controller/action`: the action's class, in the modules `routes.xml` gives the front name (`Vendor\Module\Controller\[Adminhtml\]Controller\Action`); on the front name, the route | `getUrl('wishlist/index/add')`, `_redirect()`, a redirect's `setPath()` |
| `magentoTable` | a table of a `db_schema.xml` | `getTableName('wishlist')`, `getTable()`, a resource model's `_init('wishlist', 'wishlist_id')` |
| `magentoColumn` | a column of the table named before it | the second argument of `_init()` |

All have completion, goto, hover and Find References. Find References lists the XML too
(`ReferenceKind::other_usages`, from the model's mentions): an event's observers, a
resource in `acl.xml`, `system.xml`, `menu.xml` and `webapi.xml`, a block where it is
declared, referred to (`referenceBlock`), moved (`<move>`) and placed (`before`,
`after`), a template where the layout gives it to a block.

- **A template named without its module** (`setTemplate('cart.phtml')`) is of the
  module of the file naming it.
- **A URL path's parts left out are `index`** (`shop/cart` is `shop/cart/index`); a
  controller's part with underscores is a folder path (`product_view` is
  `Product\View`). A path with `*` (the current request's part) names nothing.
  Completion offers the paths of the classes in each route's `Controller` folder.
- **A class constant's value** is a place a name can be: a pattern on a property named
  `::NAME` is about the constant `NAME` of the class and its subclasses
  (`CalleePattern::Property`, `walk_in_class_like_constant_item` in
  `frameworks/refs/mod.rs`). `ADMIN_RESOURCE` is the first use.

### Renames

`rename_target`, from a string of PHP or from inside an XML file:

| Renamed | What changes |
| --- | --- |
| an event | every `dispatch()` naming it and every `events.xml` observing it |
| a block's name | the PHP naming it and every layout file: its declaration, `referenceBlock`, `<move>`, `before` and `after` |
| an ACL resource | the part after `Vendor_Module::`, in PHP, `acl.xml`, `system.xml`, `menu.xml` and `webapi.xml` |
| a template | the path after `Vendor_Module::`, in PHP and in the layout; its file moves, in the module and in every theme that has a version (`rename_files`). A usage written without its module is renamed as written |

Only where the project's own files declare it (not one of `vendor/`). Not renamed: a
configuration path (its parts are separate elements of `system.xml` and `config.xml`),
a module (its folder, its namespace and every id starting with it), a route (its parts
are a front name and class names) and a table (the database has it).

### Names that name nothing

`ReferenceKind::undefined`, as inspections (`inspect/laravel.rs`, `MAGENTO`):

| Inspection | Reported when |
| --- | --- |
| `PhpUndefinedMagentoTemplate` | a `.phtml` id whose module the project has, with no file in the module or in a theme |
| `PhpUndefinedMagentoAcl` | `Vendor_Module::resource` that no `acl.xml` has, where some `acl.xml` declares a resource of that module |
| `PhpUndefinedMagentoModule` | `Vendor_Module` the project doesn't have, where it has another module of that vendor |

Each asks that the project have what would declare the name: a vendor that isn't
installed is not known to lack anything. Events, configuration paths, routes and tables
are never reported: an event is dispatched for observers that may not exist, a path is
read without a field or a default, a router may match without `routes.xml`, and a table
may come from an install script. Not in the files of `vendor/`.

## XML files as documents

`frameworks/magento/documents.rs`, the pattern of Drupal's YAML files
([`drupal.md`](drupal.md)): `magento_document_definition`, `_hover`, `_references`,
`_completion`, `_prepare_rename` and `_rename` take the file's path, its open text and a
position. The open text is read like the file on disk (`project::read`), and the caret
is on one of the names it writes:

| Name | Where | Answer |
| --- | --- | --- |
| a class | `di.xml` (`preference`, `type`, `virtualType`, `plugin`, an object argument), `events.xml`, layout (`block class`, an object argument), `system.xml` (`source_model`, `backend_model`, `frontend_model`), `extension_attributes.xml`, `webapi.xml`, `crontab.xml`, the queue files, UI components, a GraphQL schema's resolvers | goto its declaration; hover with the preference of the file's area and the plugins on it; its usages in PHP and in the other files; its rename (its short name, as a rename in PHP does) |
| a method beside a class | `webapi.xml` (`service method`), `crontab.xml`, `communication.xml` | goto the method, its usages |
| a virtual type | where a `di.xml` declares or names one | its declaration, the places naming it, its rename in every `di.xml` |
| an event, an ACL resource, a configuration path, a template, a handle, a block, a module, a table, a route | the files of the model | its kind above, as for a string of PHP |
| a UI component | layout (`<uiComponent name>`) | its file |

Completion offers what the place takes: the namespaces and classes below what is
written (and the virtual types), or the kind's names. An empty value is read with one
letter put at the caret, to see what the place takes.

**Diagnostics** (`magento_document_problems`, source `magento`), in the project's own
files, once the index is ready:

| Code | Reported when |
| --- | --- |
| `MagentoXmlUndefinedClass` | a class no file declares, Magento wouldn't generate and no virtual type names, where its namespace (`Vendor\Module\…`) is a module the project has |
| `MagentoXmlUndefinedTemplate` | a `.phtml` template whose module the project has, with no file in the module or a theme |

The server (`phoxite-lsp`): `Kind::MagentoXml` for a file `XmlKind::of` knows; the
branches beside Drupal's in `handlers.rs`; the diagnostics from
`State::template_problems`, again when the index state changes. The client is asked for
the files once the project is known to be Magento (`register_documents("xml", …)`, ids
`phoxite/xml/…`), so the extension lists no XML selector and a project that isn't
Magento sends none.

## A class and a method in the XML

Beside Laravel's hooks in `navigate/references.rs`, `rename.rs` and `moves.rs`:

- **Find References on a class** adds the places the files name it
  (`magento_class_usages`). **On a method**, the `before`, `after` and `around` methods
  of the plugins registered on its class, on a class below it or on one above it, and
  the XML naming the method (`magento_method_usages`).
- **Renaming a class** edits those places (its short name), and **moving its file**
  rewrites them with the new namespace, spelled as each was written: with its leading
  backslash, with its backslashes doubled in a GraphQL schema (`documents::respell`).
- **Hover on a method** says which plugin methods intercept it.

## Plugins

`frameworks/magento/plugins.rs`. A class a `di.xml` registers on a type
(`<type name="Target"><plugin type="Plugin"/></type>`) intercepts its methods by name:
`beforeSave()`, `afterSave()` and `aroundSave()` are about `save()`.

- Goto on the name of such a method, where it is declared, goes to the method it
  intercepts, in each type the plugin is registered on (`PhpEngine::definition`, before
  the caret's own symbol, like a Drupal hook implementation).
- `PhpUndefinedMagentoPluginMethod` (`inspect/magento.rs`) reports one whose types have
  no such method. Only where every type the plugin is registered on is a class the
  project has; a method that doesn't start with one of the three words is the plugin's
  own.

A virtual type a plugin is registered on is the class it is of.

## Types

`types/provider/magento.rs`, only in a Magento project (`TypeProvider::applies`):

- **The object manager:** `get(Foo::class)` and `create(Foo::class)` are `Foo`; a
  virtual type's name is the class its `di.xml` gives it. An interface stays the
  interface: the preference is what runs, not what the code may rely on.
- **Data objects:** on `Magento\Framework\DataObject` and its subclasses, for a method
  the class neither declares nor documents, `setFooBar()` and `unsFooBar()` return the
  object and `hasFooBar()` a `bool`. `getFooBar()` stays `mixed`: what a data object
  holds isn't declared anywhere.
- **View models:** `$block->getViewModel()` and `$block->getData('view_model')` are the
  class of the `view_model` argument the layout gives blocks of that class, where at
  most three differ. A class every template shares (`Template`) has too many to say.

**A template's variables** (`frameworks/magento/views.rs`, through
`Declarations::view_variable`), in a `.phtml` file of a `templates` folder (a module's
`view/<area>/templates`, a theme's `Vendor_Module/templates`):

| Variable | Is |
| --- | --- |
| `$block` | the class of the block the layout gives the template; every such class when several blocks render it; `Magento\Framework\View\Element\Template` when the layout names none |
| `$this` | `Magento\Framework\View\TemplateEngine\Php` |
| `$escaper` | `Magento\Framework\Escaper` |
| `$secureRenderer` | `Magento\Framework\View\Helper\SecureHtmlRenderer` |
| `$viewModel` | the class of the block's `view_model` argument, when the layout gives one |

A block declared for the template's own area is preferred to one of another area. A
`/** @var \Vendor\Module\Block\X $block */` in the template still wins: these are what a
variable is without one.

## Tests

- `base/xml.rs`, `frameworks/magento/{project,generated,plugins,documents}.rs`: unit
  tests.
- `navigate/tests/magento.rs` (9 tests) and `navigate/tests/magento_xml.rs` (8 tests:
  the documents, a class's and a method's usages, renames, a moved file, routes and
  tables, the diagnostics), on a stand-in for Magento's API.
- `fixtures/magento-app` (a module under `app/code`, two themes of which one inherits
  from the other, `lib/magento.php` as the stand-in) and `tests/magento_fixture.rs`:
  every PHP file inspected with nothing reported, one name of each kind, the types, the
  XML files as documents.
- The server: `a_magento_module_names_what_its_xml_declares` (`phoxite-lsp/src/tests.rs`):
  PHP and a template, an `events.xml` written outside the editor, an XML document (goto,
  completion, hover, a diagnostic) and the registration for XML files.
- VS Code: the `magento` part of the end-to-end suite (`magentoChecks`, 6 checks;
  `PHOXITE_E2E_ONLY=magento`).
- `tests/magento_corpus.rs` (ignored; `scripts/php-corpus.sh magento`): below.

## On Magento itself

`tests/magento_corpus.rs`, on Magento Open Source 2.4.9 (25,391 PHP files, 8,846 XML
files), measured 2026-11-22 on the development machine, release build:

- **The fixture module on the real framework** (`lib/internal/Magento/Framework` and the
  Backend module as include paths, the stand-in removed): every name resolves, the
  generated classes, the object manager, the data object and the template's variables
  are typed, and none of Magento's inspections reports anything in its 20 files.
- **The checkout as a project:** indexed in 12.9 s. The model reads 2,428 files: 222
  modules, 3 themes (`Magento/luma` before `Magento/blank`, which it inherits from),
  1,180 preferences, 564 virtual types, 594 plugins, 396 observers, 439 ACL resources,
  2,319 configuration paths, 2,106 blocks, 858 layout handles, 68 extension
  attributes, 99 routes and 352 tables, with 23,641 names written in them.
- **The model's cost:** 390 ms to read from nothing, under a microsecond when kept,
  170 ms after one XML file changed (the folders are listed again; one file is read
  again). It is paid on the first request after a change to one of these files, never
  per keystroke.
- **Magento's inspections on core PHP:** nothing, in the 722 PHP files of the Wishlist,
  Review, Newsletter and Cms modules (inspected in 5.3 s). The other inspections make
  2,628 reports there: the corpus is the repository without its `vendor/`, so the
  classes of Composer's packages are unknown.
- **The XML diagnostics on every file the model reads** (1.1 s for the 2,428): 12
  reports, all `MagentoXmlUndefinedClass`, all of classes Magento's own `di.xml` files
  still name and no longer have (`Magento\Deploy\Model\Deployer`,
  `Magento\Indexer\Model\HandlerPool`, `Magento\CatalogInventory\Setup\UpgradeData`…).
  The test checks each against the file system.

## Not done

What cannot be read without running the application:

- A template, a block's class or a handle set from a variable or built at run time;
  layout stored in the database or generated by code.
- Routers that match without `routes.xml` (CMS pages, URL rewrites), and a URL path
  with `*` in it, which stands for the current request's.
- What a data object holds: the type behind `getFooBar()`, an observer's
  `$observer->getData()`.
- Which area a PHP class outside `Adminhtml`, `etc/<area>` and `view/<area>` runs in: it
  gets the global `di.xml`'s answer.
- Which theme is active: every theme of the area is looked in, the inheriting ones
  first.
- A table an install script makes rather than a `db_schema.xml`.

Left out as a different framework: Magento 1.

Read for the classes they name and nothing else, each being a format of its own with
no name a PHP file writes: a UI component's fields and data sources, a GraphQL schema's
types and fields, and the files of `etc` the model has no reading of (`widget.xml`,
`indexer.xml`, `mview.xml`, `email_templates.xml`…).
