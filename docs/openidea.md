# Phoxite and OpenIDEA

The engine started as `crates/ide-php` on a branch of
[OpenIDEA](https://github.com/l0ng-ai/openidea). Its maintainers chose to keep OpenIDEA's
PHP support on language servers instead of taking the engine into their tree, so this
repository is the engine's home: it is developed here, and OpenIDEA uses it the way any
other editor does, through `phoxite-lsp`.

`crates/phoxite/SYNCED_FROM` records the OpenIDEA commit the engine was last copied from.
Nothing is copied any more.

## What OpenIDEA has

- **Phoxite as a PHP language server.** `languages/php.toml` lists `phoxite` next to
  Intelephense (the default) and phpactor. Pick it in **Settings › Languages & Frameworks ›
  Language Servers**; OpenIDEA downloads `phoxite-lsp` from this repository's GitHub
  releases and checks it against the `.sha256` file beside each binary. While Phoxite
  serves a project, `laravel-lsp` is not started (`replaces = ["laravel-lsp"]`).
- `.blade.php` files open with the language id `php` there; the server treats a file as
  Blade by its name, so they get code insight (without Blade highlighting).

## What is on the way

- **The Laravel tools as a plugin** (the routes view, the Eloquent model view, the code
  generators, Run Artisan Command, "Create translation key…"), in `editors/openidea`. It
  talks to the server through the `phoxite/laravel/*` requests, which needs two additions
  to OpenIDEA's plugin API 1.9 agreed in
  [l0ng-ai/openidea#3542](https://github.com/l0ng-ai/openidea/issues/3542): requests from a
  plugin to a running language server (grant `lsp:phoxite`), and commands a server hands
  to the client routed to the plugin.
- **Pull Members Up / Push Members Down** through OpenIDEA's own dialogs, by a small
  adapter in OpenIDEA that sends `phoxite/refactor/*` to a server advertising
  `experimental.phoxite.refactor`.

## What a release must keep for OpenIDEA

OpenIDEA's installer depends on these, so a release that changes them breaks installs:

- Asset names `phoxite-lsp-<target>[.exe]` (OpenIDEA matches the Windows, Linux and macOS
  ones, x64 and arm64), each with a `<asset>.sha256` file containing `<hash>  <name>`.
- `phoxite-lsp --version`.
- `experimental.phoxite` in the server's capabilities, and the request names under
  `phoxite/laravel/*` and `phoxite/refactor/*` (`docs/editors.md`).
