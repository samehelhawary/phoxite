# php-engine and OpenIDEA

The engine started as OpenIDEA's `crates/ide-php`. The plan is that this repository becomes
its home and OpenIDEA depends on it, so a fix lands once for every editor.

## Until OpenIDEA depends on this repository

The engine is still developed in OpenIDEA and copied here:

```sh
python scripts/sync_from_openidea.py ../openidea
```

The script replaces `crates/php-engine/{src,tests,examples,benches,stubs}`, the Laravel
generator templates (`crates/php-engine/templates`, from OpenIDEA's
`src/features/templates/files/laravel`), the fixtures (`php-library`, `laravel-app`,
`laravel-stubs`) and `docs/engine`. It renames the crate (`ide_php` to `php_engine`,
snapshot files included), leaves out OpenIDEA's own differential test, and records the
source commit in `crates/php-engine/SYNCED_FROM`. `crates/php-engine/Cargo.toml` belongs to
this repository and is not touched.

The engine itself no longer depends on any OpenIDEA crate: it has its own base types
(`php_engine::base`: positions, edits, diagnostics, ids, errors, events, settings), and
OpenIDEA converts at its edge (`crates/ide-host/src/php_convert.rs`).

## Switching OpenIDEA over

Once the OpenIDEA maintainers agree:

1. Move the history over: `git filter-repo --path crates/ide-php --path fixtures/php-library
   --path fixtures/laravel-app --path fixtures/laravel-stubs --path docs/php-engine` in a
   clone of OpenIDEA, then replace this repository's `crates/php-engine` with the result
   (renamed as the sync script does), so the engine's history comes along.
2. Publish `php-engine` to crates.io, or tag a release.
3. In OpenIDEA, replace the `ide-php` member with the dependency, keeping the crate's
   name so no import changes:

   ```toml
   # Cargo.toml [workspace.dependencies]
   ide-php = { package = "php-engine", version = "0.1" }   # or git + tag
   ```

   and delete `crates/ide-php`. The Laravel generator templates stay in OpenIDEA's
   `src/features/templates/files/laravel` for its file-template UI; the engine keeps its
   own copy, and a test on either side compares them.
4. OpenIDEA keeps its own tests (`ide-host` integration tests, `language_smoke`, the
   differential test against Intelephense); the engine's tests live here.

If OpenIDEA prefers to keep the engine, this repository stays a mirror fed by the sync
script.
