# Phoxite for VS Code

PHP and Laravel code intelligence from a native engine written in Rust: no PHP, Node.js or
licence needed for any feature.

![Completion that follows an Eloquent model to its columns and relations](images/completion.gif)

## Quick start

1. Install the extension and open a folder with PHP in it. The server is part of the
   extension: there is nothing else to install or configure.
2. Open a PHP or Blade file. **Phoxite** in the language status (the `{}` in the status
   bar) shows the project being read, then says **Phoxite**, or **Phoxite: Laravel** in a
   Laravel project.
3. If Intelephense or another PHP language server is enabled, disable it for the
   workspace: see [Other PHP extensions](#other-php-extensions).

**Requirements.** VS Code 1.85 or newer, on Windows, macOS or Linux (x64 and arm64;
Alpine x64). PHP itself is needed for two things only, both optional: running Artisan
commands, and PHPStan or Psalm on save.

## PHP

- Completion with auto-import, named arguments, array keys and enum cases
- Hover, signature help, inlay hints (parameter names, types), semantic highlighting
- Go to Definition, Type Definition and Implementation; Find All References; call and type
  hierarchies; workspace symbols
- Rename (a class's file is renamed with it); moving files updates namespaces and references
- Quick-fixes and refactorings, Organize Imports, Format Document (PSR-12)
- Inspections: undefined classes, functions, methods and properties, type mismatches, unused
  imports and more, each one configurable (`phoxite.inspections`)
- PHPStan and Psalm on save, when the project installs them (`phoxite.analyzers.onSave`)

### Refactorings that ask first

![The Change Signature form beside the method it changes](images/change-signature.png)

- **Change Signature…** (the lightbulb on a function or method, the Command Palette, or
  **PHP Refactorings** in the editor's context menu) opens a form: rename it, change its
  visibility and return type, and add, remove, reorder and retype its parameters. Calls,
  overrides and implementations are updated; a new parameter can name the value existing
  calls pass.
- **Extract method…** / **Extract function…** on a selection asks for the name, the
  visibility, which variables to pass, whether it is static, and whether to replace the
  duplicates of the code.
- **Generate getters…**, **setters…**, **constructor…**, **Override methods…** and the other
  Source Actions ending in `…` let you tick the members first.
- **Pull Members Up…** and **Push Members Down…** move methods, properties and constants
  between a class and its parents or subclasses, list what would break and ask before
  going on.

## Laravel

![Route and view names completing inside the strings that name them](images/laravel.gif)

- Routes, views, Blade components, config, env, translations, validation rules, gates and
  policies, Artisan commands, Livewire, Inertia, macros and modules: completion, go to
  definition, hover, usages and rename for the strings that name them
- Eloquent: columns from the migrations, relations, scopes, factories; with
  `phoxite.laravel.database`, also the columns the project's SQLite database really has
- Code lenses on controller methods with their routes, the full URI after each route
- Laravel inspections with quick-fixes (create a missing view, translation key…)

### Laravel tools

![The Laravel Routes view beside a route file with its URIs after each route](images/routes.png)

The commands below are in the Command Palette under **Laravel** once a Laravel project is open.

- **Laravel Routes** (Explorer view) lists the routes by route file: click one to go to it,
  or copy its URL, open it in the browser or copy it as an HTTP request. **Go to Route…**
  searches them by method, URI, name or action.
- **Model view**: the lens on an Eloquent model class opens the model's table, columns,
  relations, scopes, accessors and related classes (factory, policy, migrations…). Each
  item goes to its declaration; right-click for Find Usages and Rename. **Add Columns…**
  writes a migration and adds the columns to `$fillable`.
  ![The model view of a Post model: columns, casts, relations and scopes](images/model-view.png)
- **Generate…** (also in the Explorer's context menu) creates controllers, Form Requests,
  migrations, jobs, mailables, policies, Blade components, tests and more, plus the code
  generations of the project's `ide.json`. **New Eloquent Model…** is a form for a model
  with its fields (an enum with its values, a foreign key with what deleting does, the
  columns of a polymorphic relation), relations (through another model and polymorphic
  ones too) and indexes, and optionally its migration, factory, seeder,
  controller, Form Requests, JSON resource and policy.
- **Run Artisan Command…** lists the project's commands, Laravel's and the packages', asks
  for arguments and runs the command in the terminal. A console command class has a
  **Run artisan …** lens. The project's own commands are also in **Tasks: Run Task**, and
  `tasks.json` can keep one with its arguments:
  `{ "type": "artisan", "command": "migrate", "arguments": "--seed" }`.
  **Run Last Artisan Command** repeats the last one with its arguments. Commands run
  as `phoxite.laravel.artisanCommand` says (`php artisan` by default); in a project set
  up for Laravel Sail the extension asks once whether to run them through Sail.
- **Create translation key…** (the lightbulb on a translation key some locale lacks) asks
  which locales get the key and their texts.
- **Edit Generator Template…** opens the project's copy of a generator's template
  (`.phoxite/templates/<name>.ft`); the generators use it instead of the default.

## Blade

![A Blade template with directives, echoes and a bound attribute highlighted inside HTML](images/blade.png)

Completion, hover, go to definition and usages inside echoes, directives and `@php` blocks;
directive, section, stack and component names; the template's own problems; formatting.

Highlighting covers Blade wherever it is written: in the text, inside HTML tags and
attribute values (`<a href="{{ route('home') }}">`, `@class([...])`), in a component's
bound attributes (`:type="$type"`) and in scripts and styles. Type `@` for snippets of
the directives (`@if`, `@foreach`, `@section`…), or `x-` for a component tag.

## Symfony

- The types of services (`$container->get('mailer')`) and of Doctrine repositories and
  their finders
- Route names (`generateUrl()`, `redirectToRoute()`, a URL generator's `generate()`, and
  `path()` / `url()` in templates) and template names (`render('page.html.twig')`,
  `{% extends %}`, `{% include %}`…): completion, Go to Definition, Find All References
  and Rename. Renaming a template moves its file
- Service ids (`$container->get('mailer')`) and parameters (`getParameter('app.locale')`)
  from the project's services files: completion, Go to Definition, Find All References
- Translation keys (`trans('form.title')`, `'form.title'|trans`) from the YAML catalogues
  of `translations/`: completion, Go to Definition in every locale, Find All References
- Doctrine: the entities' associations typed from their `#[ORM\…]` attributes, and the
  entity's fields in the keys of `findBy([...])`
- `PhpUndefinedTemplate` warns of a template PHP names that isn't in the project's
  template folders
- In a template: the variables the controller passes (`render('page.html.twig', ['post' =>
  $post])`), what follows a `.` after them (`post.title`, `post.author.name`, a loop's
  variable too), and the filters, functions and tests of the project's own Twig extensions

## WordPress

In WordPress itself, a plugin or a theme (detected from WordPress's files, a plugin or
theme header, or Composer):

- Hooks: `add_action('init', …)`, `add_filter()`, `remove_action()`, `has_filter()`… name
  what `do_action()` and `apply_filters()` fire. Completion of the hooks fired, Go to
  Definition to the firing call, Find All References, hover, and Rename for the hooks the
  project fires itself. A hook fired as `"save_post_{$post->post_type}"` is found from
  `'save_post_book'`
- The same for shortcodes (`add_shortcode()`), post types (`register_post_type()`,
  `'post_type' => 'book'` in a query), taxonomies, script and style handles
  (`wp_enqueue_script('app')`, a dependency list), options (`get_option()`), image sizes,
  menu locations and blocks (`register_block_type('shop/price')`, or the `name` of a
  `block.json`). A list of names counts too: `is_singular(['book', 'page'])`,
  `'post_type' => ['book']`
- REST routes: `rest_url('shop/v1/books')` and `new WP_REST_Request('GET',
  '/shop/v1/books')` go to the `register_rest_route()` call, patterns in the route
  included (`/books/12` to `/books/(?P<id>\d+)`)
- Find All References also lists the project's scripts: a block's name, a REST route's
  path, and the `action` of an AJAX request for its `wp_ajax_…` hook. Renaming a block,
  the last part of a REST route or an AJAX action changes PHP, the scripts and the
  `block.json`
- In a JavaScript or TypeScript file and in a `block.json`: Go to Definition, hover, Find
  All References and Rename on a string naming a block, a REST route or (after
  `action:`) an AJAX action, and completion of them. `"file:./render.php"` in a
  `block.json` goes to the file
- `get_template_part('template-parts/content', 'page')`: Go to Definition to the theme's
  file (a child theme's, else its parent's), completion of the theme's templates. The same for `get_header('shop')`,
  `get_footer()`, `get_sidebar()`, `locate_template()` and `get_theme_file_uri()`
- The callback of a hook is a callable (`'my_function'`, `[$this, 'save']`): Go to
  Definition, Find All References and Rename see it. So are the callbacks given under a
  key: `register_rest_route(…, ['callback' => [$this, 'run']])`, and under any key named
  `callback`, `…_callback` or `…_cb`
- A hook's callback, a closure or a function registered by name, has its parameters
  typed from the docblock at the hook's firing call; `global $wpdb, $post;`,
  `$GLOBALS['wpdb']` and the globals a template reads have the types the `@global` tags
  document
- `"PhpUndefinedHook": { "enabled": true }` in `phoxite.inspections` warns of a hook
  nothing fires (off by default: a plugin outside the workspace may fire it)

Nothing of WordPress is bundled. A plugin or theme opened on its own needs WordPress's
functions from somewhere: install `php-stubs/wordpress-stubs` with Composer, or add a
WordPress folder to `phoxite.includePaths`.

## Drupal

In a Drupal site, or a module or theme opened on its own (Drupal 8 and later; for Drupal
7: hooks, `theme()`, `user_access()`, `module_exists()`, menu paths, variables,
`drupal_get_form()` and the `.info` file). `.module`,
`.install`, `.theme`, `.profile`, `.engine` and `.inc` files open as PHP.

- Names from the YAML files, with completion, Go to Definition to the key, hover and Find
  All References: services (`\Drupal::service('shop.catalog')`, `$container->get()`),
  routes (`Url::fromRoute()`, `$this->redirect()`, `setRedirect()`), permissions
  (`hasPermission('administer shop')`), libraries (`'#attached' => ['library' =>
  ['shop/cart']]`), configuration (`\Drupal::config('shop.settings')`) and modules
  (`moduleExists()`)
- Rename of a service, a route, a permission or a library changes its key, the PHP strings
  and what the other YAML files write (`@shop.catalog`, `route_name:`, `_permission:`),
  for names of the site's own modules
- Names from PHP: entity types (`getStorage('node')`) and plugins
  (`createInstance('shop_cart')`) from their attribute or annotation, theme hooks
  (`'#theme' => 'shop_book'`) from `hook_theme()`, with the template beside it
- Hooks: `invokeAll('cron')`, `alter('form')` and `#[Hook('node_presave')]` go to the
  `hook_…()` function of the `*.api.php` file; Find All References lists the functions
  implementing the hook
- Types: `\Drupal::service('id')` is the service's class; `getStorage('node')->load(1)`
  is a `Node`; the parameters of `mymodule_form_alter(&$form, $form_state)` have the types
  the hook documents
- A template's `{% include '@shop/card.html.twig' %}` goes to the module's template;
  `attach_library('shop/cart')` is a library; a template completes the variables its
  theme hook declares
- Configuration keys: `\Drupal::config('shop.settings')->get('mail.from')` completes
  and goes to the key in `config/install/shop.settings.yml`
- Go to Definition on the name of `mymodule_form_alter()` goes to `hook_form_alter()`
- In the YAML files themselves (`*.services.yml`, `*.routing.yml`, `*.links.*.yml`,
  `*.libraries.yml`, `*.info.yml`, `*.permissions.yml`): Go to Definition, hover, Find All
  References, Rename and completion on services (`@shop.catalog`), routes
  (`route_name:`), permissions (`_permission:`), libraries, and on the classes and
  controller methods they name

Nothing of Drupal is bundled. A module opened on its own knows only its own names: add the
site's `core` folder to `phoxite.includePaths` for Drupal's classes and hooks.

## Twig

Highlighting of Twig in the text, inside HTML tags and attribute values, and in scripts and
styles; snippets for the tags (`block`, `if`, `for`, `macro`…).

- Completion of tags, filters (after `|`), functions, tests (after `is`), the variables and
  macros in scope, and template names in `extends`, `include`, `embed`, `import`, `from`,
  `use`, `include()` and `source()`
- Hover on a built-in tag, filter, function or test, and on a macro
- Go to Definition: a template name to its file, a macro call to the macro (in this
  template or the one it is imported from), a block's name to the block it overrides in
  the parent template
- Find All References on a template name: the templates naming it
- Rename a block, in every template of its `extends` and `use` family, or a macro, where it
  is declared, imported and called
- Format Document and Format Selection indent every line by the HTML elements and Twig
  blocks around it and put the spacing inside the tags right (`{{x|upper}}` →
  `{{ x|upper }}`). Nothing moves to another line; `<pre>`, `<textarea>`, `verbatim`,
  comments and tags written over several lines stay as written. The code of a script or
  a style is indented by its brackets, or keeps its own indentation where that isn't safe
  (a template literal, say); the code itself isn't reformatted. For that and for rewrapped
  lines, set `phoxite.format.twig` to `prettier`: the project's Prettier, with a plugin
  that reads Twig, then formats whole templates
- Syntax errors as you type; a tag Twig itself doesn't have is not one (extensions add
  tags). `"TwigSyntax": { "enabled": false }` in `phoxite.inspections` turns them off
- With Symfony's Twig bridge installed: its tags, filters and functions (`path`, `trans`…)

Templates are found in `templates/` (or the `default_path` and `paths` of
`config/packages/twig.yaml`), `app/Resources/views` and `resources/views`. Twig needs no
index, so it works in any folder.

## Settings

| Setting | |
| --- | --- |
| `phoxite.phpVersion` | The PHP version; empty: from `composer.json`. |
| `phoxite.includePaths` | Folders and single files outside the project to index (`../shared`, `../../config.php`). |
| `phoxite.stubs` | Extra extension stubs (`redis`, `swoole`…). |
| `phoxite.exclude` | Globs left out of the index. |
| `phoxite.inspections` | Per-inspection `enabled`, `strict` and `severity`. |
| `phoxite.frameworks` | Each framework's support: `auto` (detected), `enabled` or `disabled`. |
| `phoxite.laravel.*` | Detection, route hints, modules, Inertia page folders. |
| `phoxite.laravel.database` | A SQLite database to read columns from: `auto` (the one in `.env`) or a path. Empty: the migrations alone. |
| `phoxite.laravel.artisanCommand` | How Artisan runs: `php artisan`, `./vendor/bin/sail artisan`… |
| `phoxite.laravel.generation.*` | What the generators write: namespace, module, class suffixes, constructor promotion, return types. |
| `phoxite.templates.path` | The folder of edited generator templates. |
| `phoxite.format.lineWidth` | The line length for Format Document. |
| `phoxite.format.tool` | `phoxite` (the built-in PSR-12 formatter) or `pint`: the project's Laravel Pint formats whole PHP files. |
| `phoxite.format.twig` | `phoxite` (the built-in formatter of Twig templates) or `prettier`: the project's Prettier with a Twig plugin formats whole templates, rewrapping lines and formatting the JavaScript and CSS inside. |
| `phoxite.analyzers.nodePath` | The Node.js that runs the project's Prettier; empty: `node` on `PATH`. |
| `phoxite.analyzers.onSave` | Run the project's PHPStan and Psalm on each saved PHP file. |
| `phoxite.analyzers.phpPath` | The PHP interpreter that runs them; empty: `php` on `PATH`. |
| `phoxite.server.path` | Use another `phoxite-lsp` binary (`~` and a path relative to the workspace folder work). |
| `phoxite.server.logLevel` | How much the server writes to its log. |
| `phoxite.trace.server` | Log what the extension and the server say to each other. |
| `phoxite.warnAboutOtherServers` | Warn about extensions that answer the same requests. |

A project can carry the settings about its code itself: a `phoxite.json` at its root
holds them without the `phoxite.` prefix (`{ "phpVersion": "8.4", "stubs": ["redis"] }`),
with completion and validation as you type. What it sets wins over the settings above,
for everyone who opens the project, in any editor. It can't set `analyzers.*` or
`laravel.database`, and `includePaths` is read from it only in a trusted workspace.

## Several projects in one workspace

In a multi-root workspace each Laravel folder is its own project. A command is about the
project of the active editor (the first Laravel project when the editor is elsewhere);
a model view or a form stays with the project it was opened for. The routes view lists
the routes under their project, and Run Task offers each project's Artisan commands.

## When something is wrong

The **Phoxite** entry in the language status (the `{}` in the status bar of a PHP or
Blade file) says what the server is doing: starting, indexing, ready, or not running,
and whether the project is a Laravel one. Click it for **Restart the Language Server**,
**Show the Server Log** and **Index the Project Again**; the same commands are in the
Command Palette under **Phoxite**.

- **Nothing completes, or "the language server is not running".** Open the log (**Show
  the Server Log**): it says why the server didn't start. If `phoxite.server.path` is
  set, it has to name a `phoxite-lsp` that exists; empty uses the bundled one.
- **A class or a route that exists is reported missing.** **Index the Project Again**
  reads every file anew. Code outside the workspace folder is found through
  `phoxite.includePaths`, which takes folders and single files: each folder is indexed whole, and when it has a `composer.json`
  (or is the `vendor` folder of a project that has one), the folders that file autoloads
  are indexed too. After changing a `composer.json` outside the workspace, run **Index
  the Project Again**: the editor only reports changes inside it.
- **Completions and problems show twice.** Another PHP extension answers too: see below.
- **Laravel features are missing.** The status says **Phoxite: Laravel** when the folder
  is recognised as one (`composer.json` requires `laravel/framework`, the lock file lists
  it, or an `artisan` file exists); `phoxite.laravel.detection` can force it.
- **To report a problem**, set `phoxite.server.logLevel` to `debug`, restart the server,
  reproduce it, and attach the log to an
  [issue](https://github.com/samehelhawary/phoxite/issues).

## Other PHP extensions

Disable Intelephense or other PHP language servers in the workspaces where you use this
extension: both would answer every request, so completions and problems show twice. The
extension says so when it finds one (also for Laravel extensions that complete the same
names, and for ones that define the Blade or the Twig language too), once per workspace.

## Restricted Mode

In a workspace you haven't trusted, Phoxite still gives code intelligence, from the
files it reads. It runs nothing of the project there: Artisan, PHPStan, Psalm, Pint and
Prettier stay off, and the settings that name a program or a file outside the project
(`phoxite.server.path`, `phoxite.analyzers.phpPath`, `phoxite.analyzers.nodePath`,
`phoxite.laravel.artisanCommand`,
`phoxite.laravel.database`, `phoxite.includePaths`) are read from your own settings
only, not from the workspace's.

## Licence

Apache-2.0. It bundles JetBrains phpstorm-stubs (Apache-2.0). Not affiliated with
JetBrains or Laravel.
