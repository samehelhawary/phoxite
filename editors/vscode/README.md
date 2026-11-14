# Phoxite for VS Code

PHP and Laravel code intelligence from a native engine written in Rust: no PHP, Node.js or
licence needed for any feature.

## PHP

- Completion with auto-import, named arguments, array keys and enum cases
- Hover, signature help, inlay hints (parameter names, types), semantic highlighting
- Go to Definition, Type Definition and Implementation; Find All References; call and type
  hierarchies; workspace symbols
- Rename (a class's file is renamed with it); moving files updates namespaces and references
- Quick-fixes and refactorings, Organize Imports, Format Document (PSR-12)
- Inspections: undefined classes, functions, methods and properties, type mismatches, unused
  imports and more, each one configurable (`phoxite.inspections`)

## Laravel

- Routes, views, Blade components, config, env, translations, validation rules, gates and
  policies, Artisan commands, Livewire, Inertia, macros and modules: completion, go to
  definition, hover, usages and rename for the strings that name them
- Eloquent: columns from the migrations, relations, scopes, factories
- Code lenses on controller methods with their routes, the full URI after each route
- Laravel inspections with quick-fixes (create a missing view, translation key…)

### Laravel tools

The commands below are in the Command Palette under **Laravel** once a Laravel project is open.

- **Laravel Routes** (Explorer view) lists the routes by route file: click one to go to it,
  or copy its URL, open it in the browser or copy it as an HTTP request. **Go to Route…**
  searches them by method, URI, name or action.
- **Model view**: the lens on an Eloquent model class opens the model's table, columns,
  relations, scopes, accessors and related classes (factory, policy, migrations…). Each
  item goes to its declaration; right-click for Find Usages and Rename. **Add Columns…**
  writes a migration and adds the columns to `$fillable`.
- **Generate…** (also in the Explorer's context menu) creates controllers, Form Requests,
  migrations, jobs, mailables, policies, Blade components, tests and more, plus the code
  generations of the project's `ide.json`. **New Eloquent Model…** is a form for a model
  with its fields, relations and indexes, and optionally its migration, factory, seeder,
  controller, Form Requests, JSON resource and policy.
- **Run Artisan Command…** lists the project's commands, Laravel's and the packages', asks
  for arguments and runs the command in the terminal. A console command class has a
  **Run artisan …** lens. The project's own commands are also in **Tasks: Run Task**, and
  `tasks.json` can keep one with its arguments:
  `{ "type": "artisan", "command": "migrate", "arguments": "--seed" }`.
- **Create translation key…** (the lightbulb on a translation key some locale lacks) asks
  which locales get the key and their texts.
- **Edit Generator Template…** opens the project's copy of a generator's template
  (`.phoxite/templates/<name>.ft`); the generators use it instead of the default.

## Blade

Completion, hover, go to definition and usages inside echoes, directives and `@php` blocks;
directive, section, stack and component names; the template's own problems; formatting.

## Settings

| Setting | |
| --- | --- |
| `phoxite.phpVersion` | The PHP version; empty: from `composer.json`. |
| `phoxite.includePaths` | Folders outside the project to index. |
| `phoxite.stubs` | Extra extension stubs (`redis`, `swoole`…). |
| `phoxite.exclude` | Globs left out of the index. |
| `phoxite.inspections` | Per-inspection `enabled`, `strict` and `severity`. |
| `phoxite.laravel.*` | Detection, route hints, modules, Inertia page folders. |
| `phoxite.laravel.artisanCommand` | How Artisan runs: `php artisan`, `./vendor/bin/sail artisan`… |
| `phoxite.laravel.generation.*` | What the generators write: namespace, module, class suffixes, constructor promotion, return types. |
| `phoxite.templates.path` | The folder of edited generator templates. |
| `phoxite.format.lineWidth` | The line length for Format Document. |
| `phoxite.server.path` | Use another `phoxite-lsp` binary. |

Disable Intelephense or other PHP language servers in the workspaces where you use this
extension; the extension offers to when it finds one.

## Licence

Apache-2.0. It bundles JetBrains phpstorm-stubs (Apache-2.0). Not affiliated with
JetBrains or Laravel.
