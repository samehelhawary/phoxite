# Phoxite and OpenIDEA

[OpenIDEA](https://github.com/l0ng-ai/openidea) uses Phoxite the way any other editor
does, through `phoxite-lsp`.

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

All of it is on OpenIDEA's `main`. Its latest release, 0.1.45, was cut the day before the
first of these was merged, so none of it is in a release yet.

## What is on the way

- **The Laravel tools as a plugin**, in `editors/openidea`, on top of that plugin API: the
  routes view and lenses, the Eloquent model view with Add Columns, the generators, New
  Eloquent Model, Run Artisan Command and "Create translation key…". Its README lists
  what it lacks next to the VS Code extension. It is not yet published in OpenIDEA's
  plugin registry. It was tried in an OpenIDEA built from `main` on Windows, on a copy of
  `fixtures/laravel-app`: both views, the lenses, the generators, New Eloquent Model, Add
  Columns and Create translation key. Run Artisan Command was not run there (no PHP on
  that machine). It can be listed once an OpenIDEA release has plugin API 1.9: the
  registry checks a package with the tool of the latest release.

## What stays as it is

Change Signature, the Extract Method options and the Generate member choosers are plain
actions in OpenIDEA (`docs/editors.md` says what a client without the dialogs gets). That
is its maintainers' decision, not something left to do: OpenIDEA runs these the same way
for every language server, and when it gets dialogs for them it will design them once in
the host for all servers, with their own request shape.

## What a release must keep for OpenIDEA

OpenIDEA's installer depends on these, so a release that changes them breaks installs:

- Asset names `phoxite-lsp-<target>[.exe]` (OpenIDEA matches the Windows, Linux and macOS
  ones, x64 and arm64), each with a `<asset>.sha256` file containing `<hash>  <name>`.
- `phoxite-lsp --version`.
- `experimental.phoxite` in the server's capabilities, and the request names under
  `phoxite/laravel/*` and `phoxite/refactor/*` (`docs/editors.md`).
