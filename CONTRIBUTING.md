# Contributing to Phoxite

Bug reports, fixes and features are welcome. For anything larger than a fix, open an issue
first so the approach can be agreed before you write the code.

## Where a change goes

| Path | What | Notes |
| --- | --- | --- |
| `crates/phoxite` | The engine | Its tests and fixtures (`fixtures/`) go with it. |
| `crates/phoxite-lsp` | The language server | |
| `editors/vscode` | The VS Code extension | |
| `docs/engine` | The engine's design notes | Update the note a change makes stale. |

Everything is developed in this repository, the engine included. OpenIDEA uses the server
like any other editor (`docs/openidea.md`), so a change to release asset names, the
`.sha256` files, `--version` or the `phoxite/*` requests needs a word there first.

## Building and testing

You need Rust (the version in `Cargo.toml`'s `rust-version`) and, for the extension,
Node.js 22.

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

The extension:

```sh
cd editors/vscode
npm ci
npm run typecheck
npm test
```

Its end-to-end suite runs inside a downloaded VS Code against a server you built:

```sh
cargo build -p phoxite-lsp
cd editors/vscode
PHOXITE_SERVER=../../target/debug/phoxite-lsp npm run test:e2e
```

CI runs the first two blocks on every pull request; run them before you push.

Some engine tests and benches are ignored unless real code is there to run them on. Two
scripts fetch it, at pinned versions, into `../php-corpus` (or `PHP_CORPUS_DIR`). They run
in bash and need git; the second also needs PHP and Composer, and on Windows downloads a
portable PHP when there is none. `--check` says what is missing without fetching:

```sh
scripts/php-corpus.sh     # laravel/framework, symfony, WordPress
scripts/laravel-apps.sh   # two Laravel applications with their vendor/, for LARAVEL_APP_DIR
cargo test -p phoxite --release --test undefined_symbols -- --ignored --nocapture
```

## What a pull request needs

- A test that fails without the change: a test in `crates/phoxite` for the engine, an LSP
  test in `crates/phoxite-lsp/src/tests.rs` for the server, a unit test in `editors/vscode/test` for the extension's logic.
- The documentation it makes stale, updated in the same pull request (`README.md`,
  `docs/editors.md`, `editors/vscode/README.md`, `CHANGELOG.md`).
- A new server request or client command described in `docs/editors.md`, so editors other
  than VS Code can use it.

## Clean-room rule

The Laravel support is written from Laravel's own source and documentation, and from the
public documentation of other tools. Never decompile or copy code from Laravel Idea,
PhpStorm, Intelephense or any other closed-source product, and don't contribute code you
can't license under Apache-2.0.

## Licence

By contributing you agree that your contribution is licensed under the Apache License 2.0,
as the rest of the project is.
