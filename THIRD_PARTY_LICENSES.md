# Third-party licences

What Phoxite bundles besides its own code. Rust crates it is built from keep their own
licences (`cargo about` or `cargo license` lists them); the release workflow attaches that
list to each release.

## JetBrains phpstorm-stubs

`crates/phoxite/stubs/phpstorm-stubs.zip` (embedded in the engine for built-in
functions, classes and constants) contains the extension folders' `.php` files of
[phpstorm-stubs](https://github.com/JetBrains/phpstorm-stubs) release `v2026.3` (commit
`912ae721089fc48534d3a5795997ee8a2802b8cd`), copied unmodified, together with that release's
`LICENSE` and `README.md`. Copyright JetBrains s.r.o. and contributors, licensed under the
Apache License 2.0 (full text in the archive's `LICENSE` and at
<https://www.apache.org/licenses/LICENSE-2.0>). The stubs' documentation comments contain
material by the PHP Documentation Group, licensed under
[Creative Commons Attribution 3.0](https://www.php.net/manual/en/cc.license.php) (CC-BY 3.0).

## Laravel framework generator stubs

The Laravel code-generation templates in `crates/phoxite/templates/laravel/*.ft`
(embedded as the generators' default templates) follow the shape and documentation comments
of the `make:` command stubs of [laravel/framework](https://github.com/laravel/framework),
adapted to the engine's template variables. Copyright (c) Taylor Otwell, licensed under the
[MIT License](https://github.com/laravel/framework/blob/12.x/LICENSE.md).

The `ide.json` schema (`crates/phoxite/src/frameworks/laravel/ide-json.schema.json`) is
written from the format's public description and contains no third-party text. The Laravel
stubs under `fixtures/laravel-stubs` are test fixtures and are not shipped.

## Mago

The PHP and Twig parsers and the formatter are the [Mago](https://github.com/carthage-software/mago)
crates (`mago-syntax`, `mago-formatter`, `mago-twig-syntax` and others), dual-licensed under
the MIT License and the Apache License 2.0.

## SQLite

The language server reads a project's SQLite database with [SQLite](https://www.sqlite.org),
compiled in through the `rusqlite` crate (MIT License). SQLite itself is in the
[public domain](https://www.sqlite.org/copyright.html).

## The Blade grammar of the VS Code extension

`editors/vscode/syntaxes/blade.tmLanguage.json` is written for this project and contains no
third-party text.

## The Twig grammar of the VS Code extension

`editors/vscode/syntaxes/twig.tmLanguage.json`, its language configuration and its snippets
are written for this project, from Twig's public documentation of its syntax, and contain
no third-party text.
