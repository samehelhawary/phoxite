# Plan: standalone PHP engine + LSP server + VS Code extension

## Status (2026-11-15)

**This plan is finished, and its "shared home" decision was replaced.** Phases 0–4 and 6
are done and released (Phoxite 0.2.1, on the VS Code Marketplace and Open VSX). Phase 5,
OpenIDEA depending on this repository as a crate, will not happen: OpenIDEA's maintainers
keep their PHP support on language servers, so Phoxite stays in this repository and
OpenIDEA lists `phoxite-lsp` as a selectable PHP server (l0ng-ai/openidea#3541, merged).
`docs/openidea.md` describes that arrangement. The rest of this file is kept as the
record of how the engine was cut loose; where it says "this repository" for Phase 0 it
means OpenIDEA.

## Status as written on 2026-11-14

- **Phase 0 done** (this repository): `ide-php` depends on no OpenIDEA crate (`cargo tree -p ide-php
  -e normal` lists no `ide-*` crate; only the differential test keeps them as dev-dependencies).
  It has its own base types in `phoxite::base` (positions, edits, diagnostics, ids, errors, events, the
  `Document` buffer), its own settings (`base::settings::PhpConfig`, `LaravelSettings`), the composer
  readers that were in `ide_sdk::php`, and `project::walk` in place of `ide_vfs`. `ide-host` converts at
  its edge with `crates/ide-host/src/php_convert.rs` (`.core()` / `.php()`); Settings › PHP becomes a
  `PhpConfig` there (`php_config`). Step 6 (moving the Blade/Twig routing of `ide-host` into the engine)
  is not done: the LSP server has its own copy of that routing for now.
- **Phases 1–4 done locally** in `D:\projects\phoxite` (not pushed, no GitHub repo yet): the engine
  copied by `scripts/sync_from_openidea.py` (crate renamed `phoxite`), the `phoxite-lsp` server,
  the VS Code extension, CI and a release workflow. See that repository's `README.md`,
  `docs/editors.md` and `docs/openidea.md`.
- **Phase 5 waits** for the OpenIDEA maintainers (see `docs/openidea.md` in the new repository).

## Context

`crates/ide-php` (≈118k lines, 900 tests, PR l0ng-ai/openidea#3343) is a native PHP/Laravel code-intelligence
engine that today only runs inside OpenIDEA. The goal is to reuse it in other editors, starting with VS Code,
by turning it into its own project that speaks LSP.

Decisions made:

- **Shared home.** A new repo owns the engine. OpenIDEA's `ide-php` becomes a thin dependency on it, so fixes
  land once for every editor. The OpenIDEA maintainers have to agree to this.
- **v1 scope = PHP + Laravel core.** v1 covers everything that maps onto standard LSP: completion, goto, hover,
  usages, rename, code actions, formatting, diagnostics/inspections, inlay hints, semantic tokens, code lenses and
  Blade. The Laravel custom UI (Model view, generators, routes tree, Artisan, live DB schema) comes in a later phase.

The name is **Phoxite** (PHP + oxide): repo `phoxite`, crates `phoxite` (library) and `phoxite-lsp` (binary), the
VS Code extension "Phoxite — PHP & Laravel IntelliSense" with settings under `phoxite.*`. The plan below was
written with the working name `php-engine`.

## What has to be cut loose

Coupling of `ide-php` today (from `crates/phoxite/Cargo.toml` and its imports):

| Dependency | What is used                                                                                                                                                                                                                                                                                                                                           | Problem                                                                               | Action                                                                              |
| ---------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------- |
| `ide-core` | `Pos`, `Range`, `DocId`, `TextEdit`, `WorkspaceEdit`, `FileEdit`, `Location`, `OutlineItem`, `Severity`, `Visibility`, `Diagnostic`, `CancellationToken`, `IdeResult/IdeError/ErrorKind`, `IdeEvent`/`SharedSink`/`ServerStatus`/`ServerId`, `settings::{Settings, LaravelSettings, LaravelModuleSystem, LaravelCodeGeneration}`, `InspectionOverride` | OpenIDEA-private, pulls specta/windows-sys; `Settings` is the whole IDE settings tree | Engine-owned `base` module with the same shapes; engine-owned config structs        |
| `ide-sdk`  | 5 fns in `ide_sdk::php` (`stub_key`, `stub_sets`, `default_stub_sets`, `language_level`, `include_paths`)                                                                                                                                                                                                                                              | pulls ide-language, ide-run                                                           | Move those functions into the engine (they only read settings plus `composer.json`) |
| `ide-vfs`  | `walk_files`, `ExcludeMatcher`                                                                                                                                                                                                                                                                                                                         | pulls gix, notify, trash                                                              | Re-implement on `ignore` + `globset` inside the engine                              |

Host-side logic that belongs to the engine but lives in `ide-host` today. Every editor needs it, so it must move down:

- `crates/ide-host/src/php.rs` (1.1k lines): `PhpService`, which adapts engine results to editor requests, merges route hints into inlay hints, builds route/model lenses, runs code-action query/resolve and the hierarchy refactorings, and handles `will_rename_files`.
- `crates/ide-host/src/blade.rs` (600 lines): the Blade service, which routes requests into the embedded PHP document, provides directive/component completion and view usages, runs template quick-fixes, and is where `BladeObserver` publishes diagnostics.
- `crates/ide-host/src/twig.rs`, `crates/ide-host/src/laravel_files.rs`, and the non-UI parts of `crates/ide-host/src/ops/laravel.rs` and `ops/routes.rs` (`live_schema`, the route list, the generation context, generators). The custom-UI parts are needed only for the later phase.

## Phase 0: decouple inside OpenIDEA (follow-up PR to #3343)

Do this on a branch off `feat/php-engine`, so the boundary is proven while 900 tests and the OpenIDEA integration
tests still guard it. The result is upstreamable whether or not the split happens.

1. Add `phoxite::base` with `Pos`, `Range`, `DocId`, `TextEdit`, `WorkspaceEdit`, `Location`, `OutlineItem`,
   `Severity`, `Visibility`, `Diagnostic`, `CancellationToken`, `Error/Result`. Keep the same field layout and UTF-16 columns.
   Replace `ide_core` imports throughout the crate. In ide-host, add `From` conversions in one module
   (`crates/ide-host/src/php_convert.rs`).
2. Replace `IdeEvent`/`SharedSink` with an engine `Event` enum: `Diagnostics { doc|path, source, items }`,
   `IndexState { indexing, done, total }`, `Status`. Use a `trait EventSink`. ide-host maps it to `IdeEvent`.
   The mapping already exists in the `Producer` and `IndexStates` types in `ide-host/src/php.rs`.
3. Replace `&Settings` in `LayoutConfig::from_settings`, `StubFilter::from_settings` and `InspectionConfig::from_settings`
   with an engine `EngineSettings` (php version, extensions, include paths, excludes, inspection overrides,
   `LaravelSettings` copy). ide-host builds it from `Settings`.
4. Move the five `ide_sdk::php` functions into `phoxite::project` / `phoxite::stubs`. `ide-sdk` then calls the
   engine's versions or keeps its own copies.
5. Re-implement `walk_files` and `ExcludeMatcher` in `phoxite::project::walk` on `ignore` + `globset`.
6. Move the protocol-neutral logic of `php.rs`, `blade.rs`, `twig.rs` and `laravel_files.rs` into a new
   `phoxite::service` module. It exposes a `PhpWorkspace` that owns the engine(s) plus the Blade/Twig routing, and
   it answers in engine types. ide-host keeps only the `LanguageService` impl doing `ide_lsp::types` conversion.
7. Result: `ide-php/Cargo.toml` has no `ide-*` dependencies (dev-dependencies for the differential test excepted;
   gate that test behind a feature or leave it in OpenIDEA).

Check: `cargo test -p ide-php`, `cargo test -p ide-host --test integration php|laravel|blade`,
`language_smoke php_native`, and `cargo tree -p ide-php` shows no `ide-` crates.

## Phase 1: create the repo

1. Once #3343 has merged (or the maintainers agree), extract with history:
   `git filter-repo --path crates/ide-php --path fixtures/php-library --path fixtures/laravel-app --path docs/php-engine`,
   then move the crate to `crates/php-engine/`.
2. Workspace `Cargo.toml` with the exact versions OpenIDEA's workspace pins today (`mago-* =1.59.0`, redb, postcard,
   rayon, …). Copy rust-toolchain, rustfmt.toml and the clippy settings from OpenIDEA.
3. Licensing: Apache-2.0 `LICENSE` and a `NOTICE` crediting OpenIDEA as origin. `THIRD_PARTY_LICENSES.md` lists
   Mago (MIT/Apache) and phpstorm-stubs (Apache-2.0, the embedded `stubs/phpstorm-stubs.zip`).
4. CI (GitHub Actions): fmt, clippy `-D warnings`, `cargo test` on windows/linux/macos, and the criterion bench as
   a non-blocking job. The real-app `laravel_parity` tests stay `#[ignore]` and run on manual dispatch with a
   corpus checkout.
5. Publish `php-engine` to crates.io (or use git tags at first).

## Phase 2: LSP server (`crates/php-engine-lsp`)

Stack: `lsp-server` + `lsp-types`, the rust-analyzer crates. The engine is synchronous and takes
`CancellationToken`s, which fits this model better than an async framework.

- **Main loop:** the main thread reads messages. Requests go to a worker pool. `$/cancelRequest` fires that request's
  `CancellationToken`. Each request snapshots the document version and returns `ContentModified` when the document is stale.
- **Documents:** incremental `textDocument/didChange`, applied to a line-indexed buffer. LSP's default
  `positionEncoding` is UTF-16, which matches the engine, so advertise `utf-16`. Then `engine.document_opened/changed/closed`.
  Blade documents (`languageId: blade`) go to the service's Blade path.
- **Files:** use `workspace/didChangeWatchedFiles` (register a dynamic watcher for `**/*.php`,
  `composer.json`, `composer.lock`, `.env*`, `config/**`, `lang/**`, `resources/views/**`) and pass changes to
  `engine.files_changed`. Use `workspace/willRenameFiles` → `will_rename_files` for namespace and class moves.
- **Workspace:** one engine per workspace folder; `workspace/didChangeWorkspaceFolders`.
- **Configuration:** `initializationOptions.cacheDir` (the client passes its storage dir; the default is the OS cache dir),
  then `workspace/configuration` section `phpEngine` → `EngineSettings`. Re-apply on `didChangeConfiguration`.
- **Events:** `Event::Diagnostics` → `textDocument/publishDiagnostics`. `IndexState` → `window/workDoneProgress`
  ("Indexing PHP… n/m"). Status/errors go to `window/logMessage`, using `tracing` with an LSP writer.
- **Capabilities (v1):** completion + resolve, hover, signatureHelp, definition/declaration/typeDefinition/
  implementation, references, documentHighlight, documentSymbol, workspace/symbol, codeAction + resolve
  (quick-fixes, intentions, Optimize Imports as `source.organizeImports`), prepareRename/rename,
  formatting/rangeFormatting (PHP and Blade), inlayHint, semanticTokens/full (legend from
  `phoxite::semantic::{TOKEN_TYPES, TOKEN_MODIFIERS}`), codeLens, documentLink, typeHierarchy, callHierarchy.
- **Commands:** route lenses and route hints carry route JSON, and model lenses carry `MODEL_VIEW_COMMAND`. Pass them
  through as client-side commands (`phpEngine.route.*`, `phpEngine.modelView`), which the extension handles.
- **Paths:** use `url::Url` ↔ `PathBuf` with `dunce` on Windows (drive-letter case, UNC).
- **Tests:** an in-process `Connection::memory()` harness driving the fixtures (`php-library`, `laravel-app`).
  Port the scenarios of OpenIDEA's `crates/ide-host/tests/integration/php_*.rs`, `laravel*.rs`, `blade*.rs` and
  `language_smoke` `PHP_NATIVE`/`LARAVEL` expectations as LSP tests.

## Phase 3: VS Code extension (`editors/vscode`)

- TypeScript, `vscode-languageclient` v9, bundled with esbuild. Activation on `onLanguage:php`, `onLanguage:blade`
  and `workspaceContains:composer.json`.
- **Server binary:** shipped inside platform-specific VSIXs. The `phpEngine.server.path` setting overrides it for development.
- **`contributes`:**
  - Settings: `phpEngine.phpVersion`, `extensions`, `includePaths`, `exclude`, `inspections.<id>.{enabled,severity}`, `laravel.*` (mirroring `LaravelSettings`), `format.lineWidth`.
  - The `blade` language with a language-configuration and a TextMate grammar. Use an MIT-licensed grammar or write one; check the licence before copying.
  - Commands, plus a status-bar item showing indexing progress.
- **Client commands:** the route lens actions (Go to Declaration, Copy URL, Open in Browser), modelled on
  `src/features/http-client/laravel/routes.ts` in OpenIDEA. The Model view shows a "coming soon" message until phase 5.
- **Coexistence:** if Intelephense or another PHP language server is active, warn and offer to disable it for the
  workspace. Duplicate completions are the most common complaint.
- **Tests:** `@vscode/test-electron` on the fixture projects, covering completion, goto, a quick-fix, rename, a Blade view goto, and route lenses.

## Phase 4: release

- GitHub Actions matrix: `x86_64/aarch64-pc-windows-msvc`, `x86_64/aarch64-unknown-linux-gnu`,
  `x86_64-unknown-linux-musl` (Alpine/containers), and `x86_64/aarch64-apple-darwin`.
- `vsce package --target <platform>` per binary. Publish to the VS Code Marketplace and Open VSX (Cursor, VSCodium, Windsurf).
- Attach the raw `php-engine-lsp` binaries to GitHub releases, with setup docs for Neovim (`nvim-lspconfig`), Helix,
  Zed and Sublime LSP. Those editors need no extra code beyond the docs.

## Phase 5: OpenIDEA consumes the shared crate

- Before Phase 1, raise it with the maintainers in #3343, or in an issue after it merges.
- Replace `crates/ide-php` with `php-engine = { version = "x.y" }` (or `git` + `tag`). ide-host's
  `php.rs`/`blade.rs`/`twig.rs` keep only the `LanguageService` conversion added in Phase 0.
- OpenIDEA-only tests (`language_smoke`, ide-host integration) stay in OpenIDEA. Engine tests live in the new repo.

## Phase 6: later (after v1)

- Custom LSP requests for the Laravel UI: `phpEngine/laravelRoutes`, `modelView`, `generationContext`,
  `generateClass`, `newModel`, `addColumns`, `artisanCommands`, `refreshSchema`, `createTranslation`. These are ported
  from `crates/ide-host/src/ops/laravel.rs` and `ops/routes.rs`. In VS Code they become a routes TreeView, a Model view
  webview, a generator QuickPick flow, and Artisan tasks.
- Change Signature and Pull Members Up/Push Down need dialogs. They become a code action with a client command that
  opens a QuickPick or webview and then calls resolve.
- Live DB schema: SQLite through `rusqlite` first, MySQL/Postgres later. PHPStan/Psalm on save comes from
  `php_analyzers.rs`.

## Risks

- **Memory/latency in VS Code:** the redb cache moves to the extension's global storage. Measure Monica and Bagisto with
  the existing bench and corpus.
- **Watcher gaps:** VS Code drops watcher events on large git checkouts. Rescan on `composer.lock` change and offer a
  "Rescan project" command (`engine.rescan()`).
- **Mago is pinned exactly** (`=1.59.0`). Upgrades are deliberate and run the full test suite.
- **Upstream may decline the dependency.** Then fall back to keeping OpenIDEA as the source and syncing with `git subtree`
  until they decide. Phase 0 is still worth merging.

## Verification

- Phase 0: `cargo test -p ide-php` (900), the ide-host php/laravel/blade integration tests, `language_smoke php_native`,
  and `cargo tree -p ide-php | grep ide-` is empty. Drive the OpenIDEA dev app once on the Laravel fixture.
- Phase 2: the LSP harness tests, and a manual run in Neovim against `fixtures/laravel-app`.
- Phase 3: extension tests. Manually, open Monica and Bagisto in VS Code with the extension and repeat the in-app
  checklist from `docs/php-engine/laravel.md` (routes, views, config, validation, Eloquent, inspections and quick-fixes, rename).
- Phase 4: install each platform VSIX on a clean VM or container. Check that the server starts and indexing finishes.
