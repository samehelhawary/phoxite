# Changelog

## Unreleased

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
