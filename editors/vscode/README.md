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
| `phoxite.format.lineWidth` | The line length for Format Document. |
| `phoxite.server.path` | Use another `phoxite-lsp` binary. |

Disable Intelephense or other PHP language servers in the workspaces where you use this
extension; the extension offers to when it finds one.

## Licence

Apache-2.0. It bundles JetBrains phpstorm-stubs (Apache-2.0). Not affiliated with
JetBrains or Laravel.
