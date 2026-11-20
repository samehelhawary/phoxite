# Changelog

## Unreleased

- **Drupal** (8 and later). In a Drupal site, or a module or theme opened on its own, the
  names its YAML files declare are references: services (`\Drupal::service()`,
  `$container->get()`), routes (`Url::fromRoute()`, `redirect()`, `setRedirect()`),
  permissions, libraries (`'#attached' => ['library' => […]]`), configuration and
  modules. So are the entity types and plugins that attributes and annotations declare,
  the theme hooks of `hook_theme()` (`'#theme' => '…'`, with the template), and hooks
  (`invokeAll()`, `alter()`, `#[Hook]` ↔ the `hook_…()` function). Each completes, goes
  to its declaration, has Find References and a hover; services, routes, permissions and
  libraries are renamed with the YAML files that name them.
  - Types: `\Drupal::service('id')` and `$container->get('id')` are the service's
    class; `getStorage('node')` is the entity type's storage, and what it loads is the
    entity; the parameters of a hook's implementation (`mymodule_form_alter()`, a method
    with `#[Hook]`) have the types the hook's function gives them.
  - `.module`, `.install`, `.theme`, `.profile`, `.engine` and `.inc` files are PHP: they
    are indexed, the server watches them, and the VS Code extension opens them as PHP.
  - The `templates` folder of a module or theme is the Twig namespace of its name
    (`@shop/card.html.twig`).
  - The YAML files are answered in too: Go to Definition, hover, Find References, Rename
    and completion on the services, routes, permissions and libraries they name, and
    Go to Definition on their classes and controller methods. The server asks the client
    for `*.yml` files only while a Drupal project is open.
  - Configuration keys (`\Drupal::config('shop.settings')->get('mail.from')`), the
    plugins a links or layouts file declares, derivative plugin ids, theme hook
    suggestions (`node__article`) and the preprocess functions of a theme hook.
  - Go to Definition on the name of a function implementing a hook goes to the hook.
  - A template completes the variables its theme hook declares, and
    `attach_library('…')` in it is a library.
  - `*.api.php` files, which document hooks as empty functions, no longer report unused
    parameters.
  - A template's variable has the type a preprocess function assigns it
    (`$variables['book'] = …`), so `{{ book.title }}` completes.
  - A configuration only a schema describes (`config/schema`) has its name and keys;
    migrations and breakpoints are plugins; a `#[Hook]` on a class types the method it
    names.
  - In a YAML file, a line is read under the key above it (list items, bracketed lists
    over several lines), `service.id:method` is a service and its method, and class
    names complete after `class:` and `_controller:`.
  - Drupal 7: a site or a module with a `.info` file is detected; `module_invoke_all()`,
    `drupal_alter()`, `theme()`, `user_access()` and `module_exists()` name hooks, theme
    hooks, permissions (`hook_permission()`) and modules, and a hook's implementation
    is typed and goes to its hook.
  - More of Drupal 7: menu paths (`drupal_goto('admin/shop')` ↔ `hook_menu()`, also
    through `%` placeholders), variables (`variable_get()` ↔ `variable_set()`), a form's
    function in `drupal_get_form()`, and its `.info` files (`dependencies[]`, `files[]`).
  - `->get('key')` on a configuration has the type its schema gives the key; a template's
    variable has the type a render array passes it (`'#book' => $book`).
  - In a YAML file, a mapping in braces and an alias (`*defaults`) are read.
  - Database tables: `db_select('node')` and the connection's `select()`, `insert()`…
    go to the table's entry in `hook_schema()`, and a column of a query
    (`$query->condition('n.nid', 5)`, `->fields('n', ['title'])`) goes to its field
    there and completes, along a chain of calls and through the alias of a joined table
    (`u.uid` after `->join('users', 'u', …)`).
  - A configuration key also takes its type from the value a file installs; a render
    array built up in a function (`$build['#book'] = $book`) types its template's
    variable.
  - Nothing of Drupal is bundled.
- The YAML reader follows anchors and aliases (`&defaults`, `*defaults`) and merges
  `<<: *defaults`: a Symfony or Drupal service defined that way has its class.
- A callback under a key named as one (`callback`, `…_callback`, `…_cb`, `page
  callback`) is a callable in any array, not only one that is a call's argument:
  `$items['x'] = ['access callback' => 'my_access'];`.
- A string under a key of an array can be a reference wherever the array is written
  (`CalleePattern::Entry`): Drupal's render arrays use it.
- **WordPress.** In a WordPress project (its core, a plugin or a theme), the names
  WordPress keeps in its registries are references: hooks (`add_action('init', …)` ↔
  `do_action('init')`, filters alike), shortcodes, post types, taxonomies, script and
  style handles, options, image sizes, menu locations, blocks (registered in PHP or
  declared by a `block.json`) and REST routes (`rest_url('shop/v1/books')` ↔
  `register_rest_route('shop/v1', '/books')`, patterns in the route included). A
  list of names counts where one is taken (`is_singular(['book', 'page'])`,
  `'post_type' => ['book']`). Each completes, goes to the
  call that fires or registers it, has Find References and a hover, and is renamed when
  the project declares it itself (not a name of WordPress or of a library). A hook fired
  under a name put together at run time (`"save_post_{$post->post_type}"`) is found by
  its pattern.
  - `get_template_part('template-parts/content', 'page')` goes to the theme's file and
    completes the theme's templates. So do `get_header('shop')`, `get_footer()`,
    `get_sidebar()`, `locate_template()`, `get_theme_file_path()` and
    `get_theme_file_uri()`. A child theme's file is looked for first, then its parent's.
  - Find References lists the project's scripts too, searched as text: a block's name, a
    REST route's path, and the `action` of an AJAX request for its `wp_ajax_…` hook.
    Renaming a block, the last part of a REST route or the action of an AJAX hook
    changes PHP, the scripts and the `block.json`.
  - In a script and in a `block.json`, a string naming a block, a REST route or an AJAX
    action has Go to Definition, hover, Find References, Rename and completion. The
    server asks the client for those documents only while a WordPress project is open
    (it registers for them, and unregisters when none is left): in other projects
    scripts aren't sent to it.
  - A hook's callback knows what the hook passes: the parameters of a closure given to
    `add_action()` or `add_filter()`, and of a function or method registered by name
    (`'shop_saved'`, `[$this, 'saved']`), take the types the docblock at the firing call
    documents.
  - `global $wpdb, $post;` has the types the `@global` tags document: the function's
    own, else what the project's functions (WordPress's among them) say of that name.
    So have `$GLOBALS['wpdb']` and a variable a file reads at its top level without
    setting it (a theme's template reads `$post`). This holds in any PHP project.
  - Callbacks given under a key of an array (`register_rest_route(…, ['callback' =>
    [$this, 'run']])`, `sanitize_callback`, `render_callback`…) are string callables
    like the others.
  - `PhpUndefinedHook` reports a hook listened to that nothing fires. It is off until
    turned on (`"PhpUndefinedHook": { "enabled": true }`): a hook may be fired by a
    plugin that isn't in the workspace.

  Nothing of WordPress is bundled. A plugin or theme opened on its own knows WordPress's
  functions and hooks from a stubs package installed with Composer
  (`php-stubs/wordpress-stubs`) or from a WordPress folder named in `includePaths`.
- The status item names the frameworks of the open projects (`Phoxite: WordPress`,
  `Phoxite: Laravel, Symfony`), not only Laravel. The server says them in
  `phoxite/laravel/status` and `phoxite/laravel/statusChanged` (`frameworks`).
- The `stubs` setting's description no longer names a `wordpress` stub set: there is
  none.
- A string that names a function or a method where a callable is expected is a name like
  any other: `array_map('App\\trim_all', …)`, `call_user_func('Foo::make')`,
  `usort($rows, [$this, 'compare'])`, `[Foo::class, 'make']`. Go to Definition and hover
  work on it, Find References counts it, and renaming the function, the method or the
  class changes it (before, a rename left these strings as they were). Function names
  complete inside such a string, and method names in `[$o, '…']` and after `'Foo::`. The
  same for a string a function declared to return a callable returns, and one assigned to
  a property typed as a callable. Only where a callable is expected and nothing else a
  string could be: for `callable|string` the string is left alone.
- In a Symfony project, the template names PHP writes (`render()`, `renderView()`,
  `renderBlock()`, `renderForm()`, `stream()` on a controller; `render()`, `display()`,
  `load()` on Twig's environment) complete, go to the template, and have Find References
  (the PHP strings and the templates naming it) and hover.
- `PhpUndefinedTemplate`: a Twig template PHP names that the project's template folders
  don't have. Not for a `@Namespace/…` name, which can be a bundle's, nor in a project
  without any template.
- `#[Template('page.html.twig')]` names a template, and `#[Autowire(service: 'mailer')]`
  and `#[Autowire(param: 'app.locale')]` a service and a parameter: an attribute's
  arguments are read like a call's.
- Service ids and parameters are renamed: the key in the YAML file, the PHP strings, the
  attributes, and `@mailer` / `%app.locale%` in the YAML files below `config/`, which are
  Find References' too. A service whose id is its class name isn't renamed this way.
- XLIFF (`.xlf`, `.xliff`; 1.2 and 2.0) and JSON translation catalogues are read beside the
  YAML ones, and translation keys complete before `|trans` in a template.
- Translation keys are renamed: in every catalogue, in PHP and in the templates. Of a
  nested key (`form: { title: … }`) the last part is renamed, to a name without a dot.
- A service after `alias:` or `parent:` in a YAML file is a usage of it, and the
  parameters and services of the files in `config/packages` are known without a services
  file importing them.
- In a template, a member of what the controller passes (`post.title`, `post.author.name`)
  has a hover with its type and goes to the property, or to the getter Twig calls.
- The usages of a route, a template or a translation key, and their renames, read a Twig
  template with unsaved changes as the editor has it.
- A Twig template knows what the controller rendering it passes: the keys of
  `render('page.html.twig', ['post' => $post])` complete as variables with their types, and
  after a `.` the public properties and methods of the variable's class do, a getter under
  the name Twig reads it by (`getTitle()` as `title`). A `{% for %}` loop's variable is an
  element of what the loop goes through.
- The filters, functions and tests a project's own Twig extensions add (`new
  TwigFilter('price', …)`, `#[AsTwigFilter('price')]`) complete in templates, and go to
  where PHP names them.
- Translation keys (`$translator->trans('form.title')`, `new TranslatableMessage(…)`,
  `t()`, and `'form.title'|trans` in a template) complete from the YAML catalogues of
  `translations/`, go to the key in every locale, and have Find References and a hover
  with the translations.
- Doctrine: a property with `#[ORM\OneToMany(targetEntity: Comment::class)]` or
  `#[ORM\ManyToMany]` declared as a bare `Collection` is a `Collection<int, Comment>`, and
  one with `#[ORM\ManyToOne]` or `#[ORM\OneToOne]` and no class in its type is the target.
  The keys of a repository's `findBy([...])`, `findOneBy([...])` and `count([...])` complete
  with the entity's fields and go to the property.
- Service ids (`$container->get('mailer')`, `has()`) and parameters (`getParameter('app.locale')`,
  a parameter bag's `get()`) complete from the project's services files, go to their key in
  the YAML file, and have Find References and hover. They aren't renamed yet.
- Rename of a Twig template, from PHP or from a template naming it: the file moves, and its
  name changes in every PHP string and every `{% extends %}`, `{% include %}` and the like.
- Route names inside a template's `path('…')` and `url('…')` complete, go to the route,
  and are usages of it: Find References on a route lists the templates too, and renaming a
  route renames it there.
- Twig templates (`*.twig`) are served: the outline; completion of tags, filters,
  functions, tests, the variables and macros in scope and template names; hover on the
  built-ins and on macros; Go to Definition for template names, macro calls and the block a
  block overrides; Find References on a template name; Rename of a block (in every template
  of its `extends` and `use` family) and of a macro (where it is declared, imported and
  called); Format Document, which indents every line by the HTML elements and Twig blocks around it
  and puts the spacing inside the tags right (`{{x|upper}}` → `{{ x|upper }}`), without
  moving anything to another line (the code of a `<script>` or a `<style>` is indented by
  its brackets); semantic tokens; and
  syntax errors as you type. A tag Twig itself doesn't have is not an error, since
  extensions add tags, and `"TwigSyntax": { "enabled": false }` under `inspections` turns
  the errors off. With Symfony's Twig bridge installed, its tags, filters and functions
  are known too. None of it needs the index.
- Format Document can use the project's **Prettier** for Twig templates: `format.twig`
  (`phoxite.format.twig` in VS Code) set to `prettier` runs `node_modules/prettier` on the
  template's text, with the project's Prettier configuration and its Twig plugin
  (`@zackad/prettier-plugin-twig`, say). Prettier also rewraps lines and formats the
  JavaScript and CSS inside, which the built-in formatter doesn't. In a trusted workspace
  only; without Prettier, for a selection, and when Prettier fails, the built-in formatter
  answers. `analyzers.nodePath` names the Node.js that runs it (`node` on `PATH` when
  empty). Tested against a stand-in and, by hand, against Prettier 3 with that plugin.
- The VS Code extension has a Twig language: highlighting in the text, inside HTML tags
  and attribute values and in scripts and styles, snippets for the tags, and a warning when
  another extension defines the language too.
- `phoxite-lsp check` reports the syntax errors of the Twig templates in the project's
  template folders (`TwigSyntax`).
- Clients are asked to watch `*.twig` and the YAML files below `config/`: a route or a
  service added to a YAML file outside the editor is known without a restart.
- Symfony route names (`generateUrl()`, `redirectToRoute()`, a URL generator's
  `generate()`, `path()` and `url()`) have Go to Definition, Find References, Rename and
  hover: to the key in a YAML routes file or the `name` of the `#[Route]` attribute.
  Renaming a route whose class has a name prefix changes the part the attribute writes.
  Completion and the undefined-route warning work as before.
- Completion, hover, Go to Definition and Rename work in strings inside attributes
  (`#[Route('/x', name: 'home')]`): the `#` of `#[` was read as the start of a comment.
- A new setting, `frameworks` (`phoxite.frameworks` in VS Code), turns each framework's
  support `enabled` or `disabled`; `auto`, the default, detects it. `frameworks.laravel`
  wins over `laravel.detection` unless it is `auto`. Laravel and Symfony are the
  frameworks with support today; the other names (`wordpress`, `drupal`, `codeigniter`,
  `yii`, `cakephp`, `laminas`, `magento`) are read now and take effect as each one's
  support is released.

## 0.6.1 — 2026-11-17

- An include path brings what its `composer.json` autoloads: the one at the top of the
  folder, and, for an include path that is a Composer `vendor` folder, the one of the
  project beside it. With `../vendor` listed, a `"Services\\": "PaymentGateway/"` rule
  of `../composer.json` gets `../PaymentGateway` indexed without listing it. The PHP
  version and the `ext-*` stubs still come from the project's own `composer.json`.
- The description of `phoxite.includePaths` says that it takes single files too
  (`../../config.php`), not only folders.

## 0.6.0 — 2026-11-17

- A file the indexer can't read without failing is left out, and the rest of the project
  is indexed. Indexing that fails as a whole is tried once more, then reported with a
  message instead of an "Indexing PHP" progress that never ends.
- A setting whose value doesn't fit (a number where a text is expected, an unknown
  choice) keeps its default and is named in a message. The other settings apply; before,
  one such value put every setting back to its default without a word.
- The PHP version and the `ext-*` requirements are read again when `composer.json`
  changes, and by **Index the Project Again**: no restart after raising `require.php` or
  adding an extension.
- A project can carry its settings: `phoxite.json` at its root holds the settings an
  editor would send, wins over them key by key, and is followed when it changes. It
  can't set `analyzers` or `laravel.database`, and `includePaths` only in a trusted
  workspace. VS Code completes and validates the file, and `ide.json` too.
- A changed severity of an inspection shows at once, not with the next edit of the file.
- **Property hooks** (PHP 8.4) are understood inside: `$value` of a `set` hook has the
  property's type (or the hook's own parameter its declared one), completion, hover and
  go to definition work in a hook's body, undefined and unused variables are reported
  there, and what a `get` hook returns is checked against the property's type. Before,
  a hook's body was read as part of whatever was around the class.
- Two inspections for members that exist but can't be used where they are. **Inaccessible
  member** (`PhpInaccessibleMember`): a private or protected method, property or class
  constant used from outside the classes allowed to, and a write to a `private(set)` or
  `protected(set)` property (PHP 8.4). **Readonly property written outside its scope**
  (`PhpReadonlyPropertyWrittenOutsideDeclarationScope`): its class, and from PHP 8.4
  its subclasses. They report what fails for certain and stay quiet where magic methods,
  a trait, a bound closure or a subclass that widens the member could make it work;
  `"strict": true` adds union receivers and closures inside classes.
- `phoxite-lsp check [PATH...]` inspects a project from the command line: the problems of
  its own PHP files and Blade templates as text, JSON, SARIF 2.1.0 or GitHub annotations
  (`--format`), and an exit code for CI (`--fail-on error|warning|never`). It reads the
  project's `phoxite.json`.

## 0.4.1 — 2026-11-16

- The notices and the documentation describe Phoxite as a standalone engine, with Sameh
  Elhawary as its author.
- The index is rebuilt once after the update: its stamp was renamed.
- The `ide.json` schema id is `phoxite://schemas/laravel-ide-json.json`.

## 0.4.0 — 2026-11-16

Versions with an even minor are stable; an odd minor (0.5.x) is a pre-release, published
to the pre-release channel of the extension stores. That is why this one is not 0.3.0.

- VS Code: **Restart the Language Server** works after a server that didn't start, and a
  second request waits for the first instead of starting another server. A missing
  `phoxite.server.path` is named before the start, with Show Log, Open Settings and
  Restart; the setting takes `~` and a path relative to the workspace folder.
- VS Code: the Laravel tools follow the server: they go when it stops and come back when
  it runs again. A request that fails says so instead of "not a Laravel project" or an
  empty routes view.
- VS Code: works in **Restricted Mode**. Code intelligence is on in a workspace you
  haven't trusted; Artisan, PHPStan and Psalm stay off, and the settings that name a
  program or a file outside the project are not read from the workspace.
- VS Code: a **Phoxite** entry in the language status shows what the server is doing and
  offers Restart, Show Log and Index Again. New settings `phoxite.server.logLevel` and
  `phoxite.trace.server`.
- VS Code: the warning about other extensions names more of them (DEVSENSE PHP, the
  Laravel extensions, the ones defining Blade), says what each doubles, and is shown
  once per workspace.
- VS Code, Blade: echoes, directives and a component's bound attributes are highlighted
  inside HTML tags, attribute values, scripts and styles, not only in the text. `<` and
  `>` are no longer bracket pairs (`{{ $a > $b }}` unbalanced the rest of the file),
  `{!! !!}` closes itself, Enter between tags indents, `#region` comments fold, and there
  are snippets for the directives.
- VS Code, several projects in one workspace: a model view, the New Model form and a
  generator stay with the project they were opened for, whatever is focused later (Add
  Columns could write into another project); Generate in the Explorer uses the clicked
  folder; Artisan runs in the Laravel project's folder, and Run Task lists the commands
  of every project; the routes view groups the routes by project.
- VS Code: the routes view has **Go to Route** and a **filter** in its title bar, reads
  the routes again when PHP files are created or deleted, and when empty says whether
  the project has no routes, none match the filter, or the server couldn't be asked.
- VS Code: the generators ask their questions as one input with a step counter and a
  **Back** button, instead of separate prompts that lost everything on Escape.
- VS Code: Change Signature keeps what was typed when its tab is hidden; the model view
  and the forms come back after a window reload. Enter in a text field of a form no
  longer submits it (Ctrl+Enter does), and the remove buttons are labelled for screen
  readers.
- VS Code: `phoxite.laravel.database` is a window setting (the server reads one value,
  so a per-folder one was never honoured).
- VS Code: the commands of the server's lenses run under the extension's own names, so
  another extension registering `laravel.modelView` can't break them.
- New Eloquent Model writes more of Laravel's schema: `enum` columns with their values,
  `morphs`, `foreignUuid`, what a foreign key does on delete (`cascade`, `restrict`, set
  null, no action), and the relations `hasManyThrough`, `morphOne` and `morphToMany`.
  The VS Code form and the OpenIDEA plugin offer them, and more column types (`char`,
  `longText`, `tinyInteger`, `unsignedInteger`, `time`, `ulid`, `ipAddress`) in New
  Model and Add Columns.
- Format Document can use the project's **Laravel Pint**: `format.tool` (`phoxite.format.tool`
  in VS Code) set to `pint` runs `vendor/bin/pint` on the file's text (unsaved changes
  included, the file on disk untouched) with the PHP of `analyzers.phpPath`, in a trusted
  workspace only. Without Pint, for a selection, and when Pint fails, the built-in
  formatter answers. Tested with a stand-in for Pint, not yet against a real one.
- VS Code, Artisan: **Run Last Artisan Command**; the arguments prompt shows the command
  that really runs (not always `php artisan`); a project set up for Laravel Sail is
  asked once whether Artisan should run through Sail.
- VS Code: the README shows the extension at work (`npm run screenshots` takes the
  pictures and the animations in a real VS Code, on the fixture).
- VS Code: the package carries `NOTICE` and the third-party licences; the Marketplace
  page links the issues and has a quick start and a troubleshooting section.
- Packaging is `node scripts/package_vscode.mjs` (it was a Python script). CI runs the
  extension end to end in VS Code on Linux, lints it, packages it as a dry run and
  checks that the tag, the manifests and the changelog state one version.
- The server tells the client when a folder is (or stops being) a Laravel project
  (`phoxite/laravel/statusChanged`), so clients no longer ask repeatedly at start.

## 0.2.2 — 2026-11-16

- The server asks the client to watch the files it indexes
  (`workspace/didChangeWatchedFiles`). Editors that report changes only for registered
  patterns (OpenIDEA among them) never told it about files written outside the editor:
  a generator's files, a `git checkout`, `composer install`.
- The "Indexing PHP" progress is reported: the server waited for a notification its own
  start-up had already read.
- Pull Members Up and the Generate actions add several imports to a file without
  imports as one block, not one blank line apart.
- The VS Code extension has an icon, and leaves watching files to the server.
- OpenIDEA offers Phoxite as a selectable PHP language server (`docs/openidea.md`).
- In OpenIDEA (on its `main`, after 0.1.45), Pull Members Up and Push Members Down work for PHP with
  Phoxite as the server.
- An OpenIDEA plugin in `editors/openidea` with the Laravel tools: the routes view and
  lenses, the Eloquent model view with Add Columns, the generators, New Eloquent Model,
  Run Artisan Command and "Create translation key…".

## 0.2.1 — 2026-11-15

- Each release has a `<binary>.sha256` file beside every `phoxite-lsp` binary.
- The server says in its capabilities (`experimental.phoxite`) that it answers the
  `phoxite/laravel/*` and `phoxite/refactor/*` requests.

## 0.2.0 — 2026-11-15

- Change Signature: a form to rename a function or method, change its visibility and
  return type, and add, remove, reorder and retype its parameters, updating the calls,
  overrides and implementations.
- Extract method and Extract function ask for the name, the visibility, what to pass,
  `static`, and whether to replace the duplicates.
- The Generate actions (getters, setters, constructor, Override methods…) let you tick the
  members.
- Pull Members Up and Push Members Down.
- PHPStan and Psalm on save, when the project installs them (`phoxite.analyzers.onSave`).
- Columns from the project's SQLite database along with the migrations
  (`phoxite.laravel.database`).
- The extension stores take a release's packages when the GitHub release is published.

## 0.1.0 — 2026-11-14

The first release: the engine, a language server and a VS Code extension.

### PHP

- Completion with auto-import, hover, signature help, inlay hints and semantic
  highlighting.
- Go to Definition, Type Definition and Implementation, Find All References, document
  highlights, workspace symbols, call and type hierarchies.
- Rename (a class's file moves with it); moving PHP files updates namespaces and references.
- Quick-fixes and refactorings, Organize Imports, Format Document.
- Inspections with per-inspection settings.

### Laravel

- Completion, navigation, hover, usages and rename for routes, views, Blade components,
  config, env, translations, validation rules, gates and policies, Artisan commands,
  Livewire, Inertia, macros and modules.
- Eloquent columns from the migrations, relations, scopes and factories.
- Route code lenses on controller methods and the full URI after each route.
- A routes view and Go to Route.
- The Eloquent model view, with Add Columns.
- Code generators, the `ide.json` code generations and a New Eloquent Model form, with
  templates you can edit.
- Run Artisan Command, a lens on console command classes, and `artisan` tasks.
- "Create translation key…" on a key some locale lacks.

### Blade

- Completion, hover, navigation and usages in echoes, directives and `@php` blocks.
- Directive, section, stack and component names; the template's own problems; formatting.

### Other editors

- `phoxite-lsp` binaries for Windows, Linux and macOS, with setup notes for Neovim, Helix,
  Zed and Sublime Text (`docs/editors.md`).
