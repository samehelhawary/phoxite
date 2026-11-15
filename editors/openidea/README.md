# Laravel (Phoxite) for OpenIDEA

The Laravel tools for [OpenIDEA](https://github.com/l0ng-ai/openidea), as a plugin on top
of `phoxite-lsp`. The server does the work; the plugin asks it through its
`phoxite/laravel/*` requests (`docs/editors.md`) and shows the answers.

## What it has

The commands are in Search Everywhere under **Laravel**.

- **Laravel Routes**, a tool window listing the routes by route file; open one to go to
  it. **Go to Route…** is a picker over the same routes, and the routes lens on a
  controller method (or the URI after a route) goes to the route, asking which when there
  are several.
- **Eloquent Model**, a tool window with a model's columns, relations, scopes, accessors
  and related classes (factory, policy, migrations…); each item goes to its declaration.
  The lens on a model class opens it, as does **Show Eloquent Model…**. **Add Columns…**
  takes `title:string, body:text?, votes:integer` (a name, a `Blueprint` type, `?` for
  nullable), writes a migration and can add the columns to `$fillable`.
- **Generate…** creates controllers, Form Requests, migrations, jobs, policies, views and
  the rest of the server's generators, plus the code generations of the project's
  `ide.json`. It asks for the name, the model where the generator takes one, and the
  inputs of an `ide.json` generation.
- **New Eloquent Model…** asks for the name, the fields (as for Add Columns) and what to
  create with it: migration, factory, seeder, resource controller, Form Requests, JSON
  resource, policy, timestamps, soft deletes.
- **Run Artisan Command…** lists the project's commands, Laravel's and the packages',
  asks for the arguments and runs `php artisan …` in a terminal. A console command class
  has a lens that runs it.
- **Create translation key…** (the quick-fix on a translation key some locale lacks) asks
  which locales get the key and their texts.

## What it doesn't have yet

Compared with the VS Code extension:

- The generators ask only for the name and the model. Their other options (the table of a
  migration, the event of a listener, API controllers, the module to generate into…)
  keep their defaults, and files go to the module of the project's settings.
- New Eloquent Model has no relations, indexes, unique or default values for fields.
- A migration's file name carries the UTC time, not the local one: a plugin has no local
  clock to send the server.
- No "copy URL" or "open in browser" on a route, no Find Usages or Rename from the model
  view, no editing of the generator templates, and Artisan always runs as `php artisan`.

## Permissions

- `lsp:phoxite` (required): send the running server its own requests, and run the
  commands its lenses carry.
- `project.read`, `storage` (required): name files relative to the project; remember
  which model each project's view shows.
- `project.write` (optional): the generators, Add Columns and Create translation key
  write files.
- `editor.read`, `editor.write` (optional): a file with unsaved changes is edited in its
  editor instead of on disk.
- `process:php` (optional): Run Artisan Command.

It needs Phoxite chosen as the PHP server (**Settings › Languages & Frameworks › Language
Servers**). The `lsp:` permission and the lens commands are plugin API 1.9, which is not
released yet: the SDK is pinned to the OpenIDEA commit that added it (`Cargo.toml`).

## Building

```sh
rustup target add wasm32-wasip2
cargo test
cargo build --release --target wasm32-wasip2
mkdir -p dist && cp target/wasm32-wasip2/release/phoxite_openidea.wasm dist/plugin.wasm
```

Then install this folder as a plugin from **Settings › Plugins** in OpenIDEA.

## Tests

`cargo test` runs against the SDK's mock host. The tests in `src/real_answers.rs` replay
what a real `phoxite-lsp` answered on `fixtures/laravel-app`
(`tests/fixtures/*.json`). When the server's answers change shape, capture them again:

```sh
cargo build --release -p phoxite-lsp          # in the repository root
node editors/openidea/tests/capture.mjs target/release/phoxite-lsp
```
