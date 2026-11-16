# Phoxite

PHP and Laravel code intelligence written in Rust, packaged for any editor: a library
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
  "includePaths": ["../shared"],
  "stubs": ["redis"],
  "exclude": ["node_modules", "/storage"],
  "inspections": { "PhpUndefinedMethod": { "enabled": false }, "PhpTypeMismatch": { "strict": true } },
  "laravel": {
    "routeHints": true, "modules": "auto", "detection": "auto", "database": "",
    "generation": { "module": "", "controllerSuffix": "Controller", "typeHints": true }
  },
  "format": { "lineWidth": 120, "tool": "phoxite" },
  "templates": { "path": ".phoxite/templates" },
  "analyzers": { "onSave": false, "phpPath": "" }
}
```

`initializationOptions.cacheDir` sets where the indexes live (default: the user's cache
folder, `phoxite/<hash of the folder>`).

## Licence

Apache-2.0. See `NOTICE` and `THIRD_PARTY_LICENSES.md` for what is bundled (the
phpstorm-stubs, the Laravel generator templates).
