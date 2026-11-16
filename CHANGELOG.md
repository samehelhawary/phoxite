# Changelog

## Unreleased

- The server asks the client to watch the files it indexes
  (`workspace/didChangeWatchedFiles`). Editors that report changes only for registered
  patterns (OpenIDEA among them) never told it about files written outside the editor:
  a generator's files, a `git checkout`, `composer install`.
- The "Indexing PHP" progress is reported: the server waited for a notification its own
  start-up had already read.
- Pull Members Up and the Generate actions add several imports to a file without
  imports as one block, not one blank line apart.
- The VS Code extension has an icon, and leaves watching files to the server.
- The engine is developed in this repository now; it is no longer copied from OpenIDEA,
  which offers Phoxite as a selectable PHP language server (`docs/openidea.md`).
- In OpenIDEA (after 0.1.45), Pull Members Up and Push Members Down work for PHP with
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

The first release: the engine of [OpenIDEA](https://github.com/l0ng-ai/openidea) as a
language server and a VS Code extension.

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
