# Phoxite

PHP, Laravel and Twig code intelligence written in Rust, packaged for any editor: a library
(`phoxite`), a language server (`phoxite-lsp`) and a VS Code extension.

Phoxite is a standalone engine: it depends on no editor.
[OpenIDEA](https://github.com/l0ng-ai/openidea) offers it as one of its PHP language
servers. What it does:

- **PHP:** completion with auto-import, hover, signature help, go to definition, type
  definition and implementation, Find Usages, rename (files named after a class move with
  it), document highlights, inlay hints, semantic highlighting, call and type hierarchies,
  quick-fixes and intentions, Optimize Imports, Format Document (PSR-12 via Mago), and
  inspections with per-inspection settings. Moving PHP files updates their namespaces and
  every reference. Change Signature, Extract Method with its options, the Generate member
  choosers and Pull Members Up / Push Members Down ask the user first: the VS Code
  extension has their dialogs, and `docs/editors.md` says what another client needs.
  PHPStan and Psalm can run on save when the project installs them.
- **Laravel:** routes, views and Blade components, config, env, translations, validation
  rules, Eloquent (columns from migrations, relations, scopes, factories), gates and
  policies, Artisan commands, Livewire, Inertia, macros and modules; route code lenses and
  URI hints; Laravel inspections with quick-fixes. The server also answers what an editor
  needs for a routes list, the Eloquent model view, the code generators, running Artisan
  commands and "Create translation key…" (`docs/editors.md`); the VS Code extension has
  them all.
- **Blade:** completion, hover, goto and usages in echoes, directives and `@php` blocks;
  directive, section, stack and component names; the template's own problems; formatting.
- **Twig:** the outline; completion of tags, filters, functions, tests, the variables and
  macros in scope and template names; hover; goto for template names, macros and the block
  a block overrides; the usages of a template name; rename of blocks and macros; formatting
  (indentation and the spacing inside tags); syntax errors. With Symfony's Twig bridge installed, its
  tags, filters and functions too.
- **Symfony:** the types of services (`$container->get()`), of Doctrine repositories, their
  finders and the entities' associations; route names and Twig template names with
  completion, goto, usages and rename, in PHP and in the templates; service ids,
  parameters, translation keys and the fields in a repository's criteria. Templates know
  the variables their controllers pass them and the project's own Twig filters.
- **WordPress:** hooks, shortcodes, post types, taxonomies, script and style handles,
  options, image sizes and menu locations with completion, goto, usages and rename;
  template parts; the types of a hook callback's parameters and of `global $wpdb`.

## Performance

The server is one native binary: it needs no PHP or Node.js, and it keeps its index on
disk, so a project opened before starts in milliseconds.

<!-- bench:summary:start -->
On laravel/framework v13.40.0 (3,177 PHP files), release build, 2026-11-17, Windows x86_64, 16 threads:

|  | Phoxite | Intelephense 1.18.5 |
| --- | --- | --- |
| Index from an empty cache | 1.5 s | 4.1 s |
| Memory once indexed | 78 MB | 324 MB |
| Memory after hover, completion and diagnostics in a 2,500-line file | 179 MB | not measured |

| Phoxite, same project |  |
| --- | --- |
| Start on the index of an earlier run | 59 ms |
| Hover right after a keystroke | 2.3 ms |
| Completion right after a keystroke | 7.2 ms |
| Diagnostics after an edit (120 ms of it is the debounce) | 216 ms |
<!-- bench:summary:end -->

Intelephense got no requests after indexing, so its memory is that of the index alone.
`node scripts/bench_report.mjs` measures all of this again and rewrites the tables; it
needs the corpus (`scripts/php-corpus.sh laravel-framework`) and
`npm i -g intelephense`. The budgets behind the latencies, and what each one measures,
are in [docs/engine/performance.md](docs/engine/performance.md); CI fails when one is
exceeded.

## Layout

| Path | What |
| --- | --- |
| `crates/phoxite` | The engine: parsing (Mago), indexes (redb), types, inspections, refactorings, Laravel. |
| `crates/phoxite-lsp` | The language server over stdio. |
| `editors/vscode` | The VS Code extension (also Cursor, VSCodium, Windsurf through Open VSX). |
| `editors/openidea` | The Laravel tools as an OpenIDEA plugin. |
| `docs/editors.md` | Setting the server up in OpenIDEA, Neovim, Helix, Zed and Sublime Text. |
| `docs/engine` | The engine's design notes and roadmap. |
| `docs/openidea.md` | How OpenIDEA uses Phoxite, and what a release must keep for it. |
| `CONTRIBUTING.md` | Building, testing and sending a change. |
| `CHANGELOG.md` | What each release changed. |

## Building

```sh
cargo build --release -p phoxite-lsp      # target/release/phoxite-lsp
cargo test --workspace
```

The VS Code extension, with a locally built server:

```sh
cd editors/vscode
npm install
npm run build
# Point "phoxite.server.path" at target/release/phoxite-lsp, then press F5
# in VS Code with this folder open, or package it:
node ../../scripts/package_vscode.mjs --server ../../target/release/phoxite-lsp
```

## Configuration

The server reads its settings from `initializationOptions.settings` and from
`workspace/didChangeConfiguration`, as one object or under a `phoxite` key:

```json
{
  "phpVersion": "8.3",
  "includePaths": ["../shared", "../../config.php"],
  "stubs": ["redis"],
  "exclude": ["node_modules", "/storage"],
  "inspections": { "PhpUndefinedMethod": { "enabled": false }, "PhpTypeMismatch": { "strict": true } },
  "laravel": {
    "routeHints": true, "modules": "auto", "detection": "auto", "database": "",
    "generation": { "module": "", "controllerSuffix": "Controller", "typeHints": true }
  },
  "frameworks": { "laravel": "auto", "symfony": "auto" },
  "format": { "lineWidth": 120, "tool": "phoxite", "twig": "phoxite" },
  "templates": { "path": ".phoxite/templates" },
  "analyzers": { "onSave": false, "phpPath": "", "nodePath": "" }
}
```

`initializationOptions.cacheDir` sets where the indexes live (default: the user's cache
folder, `phoxite/<hash of the folder>`).

A project can carry its settings itself, for everyone who works on it whatever their
editor: a `phoxite.json` at its root holds the same object.

```json
{
  "$schema": "https://raw.githubusercontent.com/samehelhawary/phoxite/main/editors/vscode/schemas/phoxite.schema.json",
  "phpVersion": "8.4",
  "stubs": ["redis"],
  "inspections": { "PhpUnusedParameter": { "enabled": false } }
}
```

What the file sets wins over the editor's settings, key by key, and the server follows
the file when it changes. A project doesn't choose the programs that run or the files
read outside it: `analyzers` and `laravel.database` are never taken from the file, and
`includePaths` only in a workspace the user trusts. A setting whose value doesn't fit is
named in a message and keeps its default.

## Checking a project from the command line

The same binary inspects a project without an editor, for a terminal or for CI:

```sh
phoxite-lsp check                      # the project of the current folder
phoxite-lsp check app routes           # only what is below these paths
phoxite-lsp check --format sarif > phoxite.sarif
```

It reports the problems of the project's own PHP files, Blade and Twig templates (not
`vendor/`) as an editor shows them, with the settings of the project's `phoxite.json`.
The exit code is 1 when there is an error or a warning; `--fail-on error` lets warnings
pass and `--fail-on never` only reports. `--format` takes `text`, `json`, `sarif`
(2.1.0, for code scanning) and `github`, which GitHub Actions shows as annotations on
the changed lines:

```yaml
- run: |
    curl -fsSLo phoxite-lsp https://github.com/samehelhawary/phoxite/releases/latest/download/phoxite-lsp-x86_64-unknown-linux-gnu
    chmod +x phoxite-lsp
    ./phoxite-lsp check --format github
```

Install the project's dependencies first (`composer install`): without `vendor/` every
class of a package is an undefined class. `phoxite-lsp check --help` lists the rest.

## Licence

Apache-2.0. See `NOTICE` and `THIRD_PARTY_LICENSES.md` for what is bundled (the
phpstorm-stubs, the Laravel generator templates).
