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

- **Pull Members Up / Push Members Down** through OpenIDEA's own dialogs
  ([#3603](https://github.com/l0ng-ai/openidea/pull/3603)): a small adapter there sends
  `phoxite/refactor/hierarchyMembers` and `phoxite/refactor/hierarchy` to a server that
  advertises `experimental.phoxite.refactor`.
- **What a plugin needs to talk to the server** (plugin API 1.9, agreed in
  [#3542](https://github.com/l0ng-ai/openidea/issues/3542)): requests from a plugin to a
  running language server behind the grant `lsp:phoxite`
  ([#3588](https://github.com/l0ng-ai/openidea/pull/3588)), and
  `[[contributes.serverCommands]]`, which routes a command the server leaves to the
  client to the plugin ([#3597](https://github.com/l0ng-ai/openidea/pull/3597)).

The last two are merged into OpenIDEA's `main` and come with the release after 0.1.45.

## What is on the way

- **The Laravel tools as a plugin**, in `editors/openidea`, on top of that plugin API. The
  routes view, Go to Route and the route lenses are there; the Eloquent model view, the
  code generators, Run Artisan Command and "Create translation key…" are not yet, and
  the plugin is not published in OpenIDEA's plugin registry.
- Change Signature, the Extract Method options and the Generate member choosers have no
  dialog in OpenIDEA: it gets the plain actions (`docs/editors.md`).

## What a release must keep for OpenIDEA

OpenIDEA's installer depends on these, so a release that changes them breaks installs:

- Asset names `phoxite-lsp-<target>[.exe]` (OpenIDEA matches the Windows, Linux and macOS
  ones, x64 and arm64), each with a `<asset>.sha256` file containing `<hash>  <name>`.
- `phoxite-lsp --version`.
- `experimental.phoxite` in the server's capabilities, and the request names under
  `phoxite/laravel/*` and `phoxite/refactor/*` (`docs/editors.md`).
