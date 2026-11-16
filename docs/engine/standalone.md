# How the engine became a standalone project

A short history, for whoever wonders why the code looks the way it does. The plan this
file used to hold is finished; what follows is what was done and how it turned out.

## Where it came from

The engine was written as `crates/ide-php` on a branch of
[OpenIDEA](https://github.com/l0ng-ai/openidea) (l0ng-ai/openidea#3343): a native PHP and
Laravel code-intelligence engine that ran only inside that IDE. The goal of the plan was
to reuse it in other editors by making it its own project that speaks LSP. The name,
Phoxite, is PHP + oxide.

## What was done

- **Cut loose from OpenIDEA.** The engine got its own base types (`phoxite::base`:
  positions, edits, diagnostics, ids, errors, events, the document buffer), its own
  settings, its own composer readers and its own project walk (`ignore` + `globset`), so
  it depends on no OpenIDEA crate.
- **A language server**, `crates/phoxite-lsp`: the engine over stdio, with the routing of
  Blade and Twig templates into their embedded PHP, the Laravel requests
  (`phoxite/laravel/*`) and the refactorings that ask first (`phoxite/refactor/*`).
- **A VS Code extension**, `editors/vscode`, with the Laravel tools and the refactoring
  dialogs, published to the VS Code Marketplace and Open VSX.
- **Releases** of the server for Windows, Linux and macOS, each binary with a `.sha256`
  file beside it.
- **Later additions:** PHPStan and Psalm on save, columns read from the project's SQLite
  database.

## What changed from the plan

The plan was a shared home: this repository would own the engine and OpenIDEA would
depend on it as a crate. OpenIDEA's maintainers preferred to keep their PHP support on
language servers and not take the engine into their tree, so:

- the engine is developed here, and nothing is copied between the repositories any more;
- OpenIDEA lists `phoxite-lsp` as a selectable PHP language server and installs it from
  this repository's releases;
- the Laravel tools reach OpenIDEA as a plugin (`editors/openidea`), through two
  additions to its plugin API made for the purpose.

`docs/openidea.md` describes that arrangement and what a release must keep for it.

## What is still open

- The server keeps its own copy of the Blade and Twig request routing; it was meant to
  move into the engine.
- Change Signature, the Extract Method options and the Generate member choosers have
  dialogs only in VS Code. In OpenIDEA they stay plain actions by its maintainers'
  decision (`docs/openidea.md`).
