# Changelog

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
