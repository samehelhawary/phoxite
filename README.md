# php-engine

PHP and Laravel code intelligence written in Rust, packaged for any editor: a library
(`php-engine`), a language server (`php-engine-lsp`) and a VS Code extension.

It is the native PHP engine of [OpenIDEA](https://github.com/l0ng-ai/openidea), where it
replaced Intelephense, and keeps its feature set:

- **PHP:** completion with auto-import, hover, signature help, go to definition, type
  definition and implementation, Find Usages, rename (files named after a class move with
  it), document highlights, inlay hints, semantic highlighting, call and type hierarchies,
  quick-fixes and intentions, Optimize Imports, Format Document (PSR-12 via Mago), and
  inspections with per-inspection settings. Moving PHP files updates their namespaces and
  every reference.
- **Laravel:** routes, views and Blade components, config, env, translations, validation
  rules, Eloquent (columns from migrations, relations, scopes, factories), gates and
  policies, Artisan commands, Livewire, Inertia, macros and modules; route code lenses and
  URI hints; Laravel inspections with quick-fixes.
- **Blade:** completion, hover, goto and usages in echoes, directives and `@php` blocks;
  directive, section, stack and component names; the template's own problems; formatting.

## Layout

| Path | What |
| --- | --- |
| `crates/php-engine` | The engine: parsing (Mago), indexes (redb), types, inspections, refactorings, Laravel. |
| `crates/php-engine-lsp` | The language server over stdio. |
| `editors/vscode` | The VS Code extension (also Cursor, VSCodium, Windsurf through Open VSX). |
| `docs/editors.md` | Setting the server up in Neovim, Helix, Zed and Sublime Text. |
| `docs/engine` | The engine's design notes and roadmap, from OpenIDEA. |
| `docs/openidea.md` | How this repository and OpenIDEA share the engine. |

## Building

```sh
cargo build --release -p php-engine-lsp      # target/release/php-engine-lsp
cargo test --workspace
```

The VS Code extension, with a locally built server:

```sh
cd editors/vscode
npm install
npm run build
# Point "phpEngine.server.path" at target/release/php-engine-lsp, then press F5
# in VS Code with this folder open, or package it:
python ../../scripts/package_vscode.py --server ../../target/release/php-engine-lsp
```

## Configuration

The server reads its settings from `initializationOptions.settings` and from
`workspace/didChangeConfiguration`, as one object or under a `phpEngine` key:

```json
{
  "phpVersion": "8.3",
  "includePaths": ["../shared"],
  "stubs": ["redis"],
  "exclude": ["node_modules", "/storage"],
  "inspections": { "PhpUndefinedMethod": { "enabled": false }, "PhpTypeMismatch": { "strict": true } },
  "laravel": { "routeHints": true, "modules": "auto", "detection": "auto" },
  "format": { "lineWidth": 120 }
}
```

`initializationOptions.cacheDir` sets where the indexes live (default: the user's cache
folder, `php-engine/<hash of the folder>`).

## Licence

Apache-2.0. See `NOTICE` and `THIRD_PARTY_LICENSES.md` for what is bundled (the
phpstorm-stubs, the Laravel generator templates).
