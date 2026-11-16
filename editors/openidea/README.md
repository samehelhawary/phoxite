# Laravel (Phoxite) for OpenIDEA

The Laravel tools for [OpenIDEA](https://github.com/l0ng-ai/openidea), as a plugin on top
of `phoxite-lsp`. The server does the work; the plugin asks it through its
`phoxite/laravel/*` requests (`docs/editors.md`) and shows the answers.

## What it has

The commands are in Search Everywhere under **Laravel**.

- **Laravel Routes**, a tool window listing the routes by route file; open one to go to
  it. **Go to Route…** is a picker over the same routes, and the routes lens on a
  controller method (or the URI after a route) goes to the route, asking which when there
  are several. **Copy Route URL…**, **Copy Route as HTTP Request…** and **Open Route in
  Browser…** take a route's URL with the application's `APP_URL`.
- **Eloquent Model**, a tool window with a model's columns, relations, scopes, accessors
  and related classes (factory, policy, migrations…); each item goes to its declaration.
  The lens on a model class opens it, as does **Show Eloquent Model…**. **Add Columns…**
  takes `title:string, body:text?, votes:integer` (a name, a `Blueprint` type, `?` for
  nullable), writes a migration and can add the columns to `$fillable`.
- **Generate…** creates controllers, Form Requests, migrations, jobs, policies, views and
  the rest of the server's generators, plus the code generations of the project's
  `ide.json`. It asks one thing at a time: the name, the module when the project has
  modules, then what the generator takes (its model, table, event, backing type or HTTP
  status), the inputs of an `ide.json` generation, and its options to tick.
- **New Eloquent Model…** asks for the name, the fields
  (`title:string, slug:string unique, votes:integer? index`; an enum lists its values,
  `status:enum(draft|published)`; a foreign key can say what deleting does,
  `user_id:foreignId cascade`; `subject:morphs` is the two columns of a polymorphic
  relation), the relations (`belongsTo:User, hasMany:Comment as comments,
  hasManyThrough:Post through User`) and what to create with it: migration,
  factory, seeder, resource controller, Form Requests, JSON resource, policy,
  timestamps, soft deletes.
- **Run Artisan Command…** lists the project's commands, Laravel's and the packages',
  asks for the arguments and runs `php artisan …` in a terminal. A console command class
  has a lens that runs it.
- **Create translation key…** (the quick-fix on a translation key some locale lacks) asks
  which locales get the key and their texts.

Both tool windows are read again when PHP files of the project change.

## What it doesn't have yet

Compared with the VS Code extension:

- New Eloquent Model has no composite indexes and no default values for fields, and its
  controller is a resource controller or none.
- Artisan always runs as `php artisan`. A plugin may run only the programs its manifest
  names for approval, so a setting for `sail` or `docker compose exec` would need each
  of them declared.
- A migration's file name carries the UTC time, not the local one: a plugin has no local
  clock to send the server.
- No Find Usages or Rename from the model view, and no editing of the generator
  templates.
- What a form shows at once in VS Code is asked one question at a time here.

## Permissions

- `lsp:phoxite` (required): send the running server its own requests, and run the
  commands its lenses carry.
- `project.read`, `storage`, `events` (required): name files relative to the project;
  remember which model each project's view shows; read the views again when PHP files
  of the project change.
- `project.write` (optional): the generators, Add Columns and Create translation key
  write files.
- `editor.read`, `editor.write` (optional): a file with unsaved changes is edited in its
  editor instead of on disk.
- `clipboard` (optional): Copy Route URL and Copy Route as HTTP Request.
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

## Releasing

The plugin is listed in OpenIDEA's marketplace through
[l0ng-ai/openidea-plugins](https://github.com/l0ng-ai/openidea-plugins), under the
publisher `phoxite`. Its releases are separate from the server's: the tag is
`openidea-plugin-v<version>`, and the release must **not** be marked latest, because
OpenIDEA installs `phoxite-lsp` from this repository's latest release.

```sh
# openidea-plugin: from an OpenIDEA release, or
# cargo install --locked --git https://github.com/l0ng-ai/openidea ide-host --bin openidea-plugin
openidea-plugin build
openidea-plugin pack . --key <the publisher key> --publisher phoxite
gh release create openidea-plugin-v0.1.0 phoxite.laravel-0.1.0.oiplugin --latest=false   --title "Laravel (Phoxite) for OpenIDEA 0.1.0"
```

Then a pull request to the registry adds a `[[versions]]` table with the package's URL
and SHA-256 to `plugins/phoxite.laravel.toml`. The registry checks a package with the
`openidea-plugin` of the latest OpenIDEA release, so a version can be listed only once
an OpenIDEA release has the plugin API it needs (1.9 for this one).

The publisher key is not in this repository. Its public key is
`DW0IGr83RQIIUkA2knXK+khG/erALjvNtHoO03ovK+Y=`.

## Tests

`cargo test` runs against the SDK's mock host. The tests in `src/real_answers.rs` replay
what a real `phoxite-lsp` answered on `fixtures/laravel-app`
(`tests/fixtures/*.json`). When the server's answers change shape, capture them again:

```sh
cargo build --release -p phoxite-lsp          # in the repository root
node editors/openidea/tests/capture.mjs target/release/phoxite-lsp
```
