# Contributing to Phoxite

Bug reports, fixes and features are welcome. For anything larger than a fix, open an issue
first so the approach can be agreed before you write the code.

## Where a change goes

| Path | What | Notes |
| --- | --- | --- |
| `crates/phoxite` | The engine | Copied from OpenIDEA; see below. |
| `crates/phoxite-lsp` | The language server | Developed here. |
| `editors/vscode` | The VS Code extension | Developed here. |
| `docs/engine` | The engine's design notes | Copied from OpenIDEA with the engine. |

The engine is still developed in [OpenIDEA](https://github.com/l0ng-ai/openidea)
(`crates/ide-php`) and copied here by `scripts/sync_from_openidea.py`, which overwrites
`crates/phoxite`, the fixtures and `docs/engine`. Until that changes (`docs/openidea.md`),
a pull request that edits those paths can't be merged as it is: describe the problem in an
issue here, or send the change to OpenIDEA.

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

## What a pull request needs

- A test that fails without the change: an LSP test in `crates/phoxite-lsp/src/tests.rs`
  for the server, a unit test in `editors/vscode/test` for the extension's logic.
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
