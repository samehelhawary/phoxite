# Using phoxite-lsp in other editors

`phoxite-lsp` talks LSP over stdin/stdout. Download the binary for your platform from
the GitHub releases (or `cargo install --path crates/phoxite-lsp`) and put it on `PATH`.
Each binary has a `.sha256` file beside it.

A file named `*.blade.php` is a Blade template whatever language id the client sends, and
a file named `*.twig` is a Twig template. The server gives Twig templates an outline,
completion, hover, Go to Definition (template names, macros, the block a block overrides),
the usages of a template name, rename of blocks and macros, formatting (indentation and the
spacing inside tags), semantic tokens and their syntax errors. Highlighting of the tags themselves
is the editor's.

Drupal's `.module`, `.install`, `.theme`, `.profile`, `.engine` and `.inc` files are PHP to
the server whatever language id the client sends; give them the `php` filetype in your
editor so that it attaches the server to them. In a Drupal project the server also answers
in the module's YAML files, and registers for `**/*.yml` itself where the client takes
that; elsewhere, attach it to `yaml` beside your YAML server.

In a WordPress project the server also answers in JavaScript and TypeScript files and in
`block.json` (Go to Definition, hover, references, rename and completion of the blocks,
REST routes and AJAX actions their strings name). It registers for those documents itself
while a WordPress project is open, so a client that takes such registrations (VS Code's
client) needs nothing; in another client, attach the server to those filetypes
beside your JavaScript server. It answers nothing in them in other projects.

Turn off other PHP language servers (Intelephense, Phpactor) for the same files; otherwise
completions and problems show twice.

Settings go in the initialization options as `{ "settings": { … } }` (see the README for the
keys); `cacheDir` sets the index folder. Or leave the editor out of it: a `phoxite.json` at
the root of a project holds the same keys, wins over what the editor sends, and is read
again when it changes (the README says which keys it may not set). A client that watches
files for the server reports its changes like those of any other file; without one, run
`phoxite.server.rescan` or restart the server after editing it.

`phoxite-lsp check` inspects a project without an editor and prints its problems; see
the README.

## OpenIDEA

Nothing to download: pick **Phoxite** for PHP in **Settings › Languages & Frameworks ›
Language Servers** and OpenIDEA installs `phoxite-lsp` from the GitHub releases. Intelephense
stays its default, and `laravel-lsp` is not started while Phoxite serves the project. See
`docs/openidea.md` for what else is on the way there.

## Neovim (0.11+)

```lua
vim.filetype.add({ pattern = { [".*%.blade%.php"] = "blade" } })

vim.lsp.config("phoxite", {
  cmd = { "phoxite-lsp" },
  filetypes = { "php", "blade", "twig" },
  root_markers = { "composer.json", ".git" },
  init_options = { settings = { phpVersion = "", laravel = { routeHints = true } } },
})
vim.lsp.enable("phoxite")
```

The route lenses run the client command `laravel.routeActions`; without a handler Neovim
shows them but does nothing when one is clicked. A minimal handler:

```lua
vim.lsp.commands["laravel.routeActions"] = function(command)
  local routes = vim.json.decode(command.arguments[1])
  vim.ui.select(routes, { format_item = function(r) return table.concat(r.methods, "|") .. " " .. r.uri end },
    function(route) if route then vim.cmd.edit(route.path) end end)
end
```

## Helix

`languages.toml`:

```toml
[language-server.phoxite]
command = "phoxite-lsp"
config = { settings = { laravel = { routeHints = true } } }

[[language]]
name = "php"
language-servers = ["phoxite"]

[[language]]
name = "blade"
scope = "text.html.php.blade"
file-types = [{ glob = "*.blade.php" }]
language-servers = ["phoxite"]

[[language]]
name = "twig"
language-servers = ["phoxite"]
```

## Zed

Zed's PHP extension starts Intelephense or Phpactor. Point it at phoxite instead in
`settings.json`:

```json
{
  "lsp": {
    "phoxite": {
      "binary": { "path": "phoxite-lsp", "arguments": ["--stdio"] },
      "initialization_options": { "settings": {} }
    }
  },
  "languages": {
    "PHP": { "language_servers": ["phoxite", "!intelephense", "!phpactor"] },
    "Twig": { "language_servers": ["phoxite"] }
  }
}
```

The `Twig` entry needs a Zed extension that defines the Twig language.

A Zed extension that registers the `phoxite` server is planned; until then the PHP
extension's server entry has to be overridden as above.

## Sublime Text (LSP package)

`LSP.sublime-settings`:

```json
{
  "clients": {
    "phoxite": {
      "enabled": true,
      "command": ["phoxite-lsp"],
      "selector": "embedding.php | text.html.blade | text.html.twig",
      "initializationOptions": { "settings": {} }
    }
  }
}
```

## What each editor gets

Everything is plain LSP except the client commands the server's results carry:

| Command | Arguments | Where |
| --- | --- | --- |
| `laravel.routeActions` | the routes as a JSON string | route code lenses and URI hints |
| `laravel.modelView` | the model's class name | the code lens on Eloquent models |
| `laravel.runArtisan` | the command's name (`posts:publish`) | the code lens on console command classes |
| `laravel.createTranslation` | the key and its locales as a JSON string | the action "Create translation key…" on a translation key |
| `phoxite.openLocation` | an LSP `Location` | quick-fixes that create a file (a missing view) |

and one server command, `phoxite.server.rescan` (`workspace/executeCommand`), which indexes the
workspace folders again.

The server's capabilities carry `experimental.phoxite` with `laravel: true` and
`refactor: true`: a client can check them before sending the requests below.

## Refactorings that ask first

Change Signature, the Extract Method dialog and the Generate actions with a member chooser
need a dialog, so the server offers them only to a client that declares
`experimental.phoxite.dialogs: true` in its capabilities. Other clients get the plain
actions (Extract method with a default name, Generate getters for every property). Each of
these code actions carries a client command and no edit:

| Command | Arguments | Request to send back |
| --- | --- | --- |
| `phoxite.changeSignature` | the document's URI, the data (`position`, `signature`) | `phoxite/refactor/changeSignature` with the wanted signature in `data.change` |
| `phoxite.extractMethod` | the URI, the data (`defaults`, `duplicates`, `staticRequired`, `usesThis`…) | `phoxite/refactor/extractMethod` with the choice in `data.change` |
| `phoxite.generateMembers` | the URI, the data (`title`, `members`) | `phoxite/refactor/generate` with the member ids in `data.chosen` |

The client shows its dialog, puts the answer in the data and sends `{ uri, data }`; the
answer is `{ edit }`, an LSP `WorkspaceEdit` to apply.

Pull Members Up and Push Members Down are two requests any client can send, with `kind`
`pullUp` or `pushDown`:

| Request | Params | Result |
| --- | --- | --- |
| `phoxite/refactor/hierarchyMembers` | `uri`, `position`, `kind` | `{ className, targets, members }`: where the members can go and which can move |
| `phoxite/refactor/hierarchy` | `uri`, `position`, `kind`, `target` (a target's `id`, for pull up), `choices` (`{ id, makeAbstract }`) | `{ problems, edit }` |

A problem has a `severity` (`info`, `warning`, `error`, `fatal`) and a `message`. With a
fatal one there is no `edit`; the client confirms the others before applying it.

## The database schema

The setting `laravel.database` has the server read the tables and columns of a SQLite
database, read-only, and use them along with the migrations (the database wins on a
column's type and nullability): `auto` for the database the project's `.env` names
(`DB_CONNECTION=sqlite`, with `DB_DATABASE` or `database/database.sqlite`), or the path of a
SQLite file relative to the project. Empty, the default, uses the migrations alone.

The server reads the database when the project opens and when the setting changes. To
follow later changes, the client reports the database file and `.env` in
`workspace/didChangeWatchedFiles`, or sends `phoxite/laravel/refreshSchema`. MySQL and
PostgreSQL are not read.

## PHPStan and Psalm

With the setting `analyzers.onSave` on, the server runs the analysers the project installs
(`vendor/bin/phpstan`; `vendor/bin/psalm` with a `psalm.xml`) on each PHP file the client
reports as saved (`textDocument/didSave`), using the interpreter of `analyzers.phpPath`
(`php` when empty). Their problems are published with the file's other diagnostics under
the sources `phpstan` and `psalm`, without those the engine already reports.

These tools run code of the project. A client with a notion of workspace trust sends
`trusted: false` in `initializationOptions` for a workspace the user doesn't trust, and the
notification `phoxite/workspaceTrust` with `{ trusted }` when that changes; nothing runs
while it is false.

## Laravel requests

The data behind the Laravel tools of the VS Code extension is there for any client, as
`phoxite/laravel/*` requests:

| Request | Params | Result |
| --- | --- | --- |
| `phoxite/laravel/status` | | `{ laravel }`: a Laravel project is open |
| `phoxite/laravel/routes` | `query` | the routes matching every term of `query` (all when empty) |
| `phoxite/laravel/modelView` | `class` | the model's table, key, columns, relations, scopes, accessors and related classes |
| `phoxite/laravel/addColumns` | `class`, `columns` (`{ name, ty, nullable }`), `fillable` | an edit: a migration, and `$fillable` |
| `phoxite/laravel/artisanCommands` | | the commands with their arguments and options, or `null` |
| `phoxite/laravel/generationContext` | | the namespace, modules, models and generators, or `null` |
| `phoxite/laravel/generateClass` | `spec` (`{ generator, name, module, model, … }`) | an edit |
| `phoxite/laravel/newModel` | `spec` (`{ name, fields, relations, indexes, migration, factory, … }`) | an edit |
| `phoxite/laravel/createTranslation` | `key`, `texts` (`{ locale, text }`) | an edit |
| `phoxite/laravel/templates` | | the generators' templates: `{ name, path, body, edited }` |
| `phoxite/laravel/refreshSchema` | | the database read again: `{ path, tables, columns }`, or `null` when the setting names none |

- The server tells when a workspace folder turns out to be a Laravel project, or stops
  being one, with the notification `phoxite/laravel/statusChanged`:
  `{ laravel, folders }` (the folders' URIs). It is sent once the index knows and again
  on each change, so a client needs `phoxite/laravel/status` only for what is known
  now. A server that sends it has `experimental.phoxite.laravelStatus` in its
  capabilities.
- Every request takes an optional `folder`: the URI of a workspace folder (or of a file in
  one), choosing the project when several are open. Without it the first Laravel project
  answers.
- Places in files are `{ path, range }` with `range.start.line` and `range.start.col`
  (0-based, UTF-16), as in the routes of `laravel.routeActions`.
- An edit is `{ edit, created }`: an LSP `WorkspaceEdit` for the client to apply and save,
  and the URIs of the files it creates.
- `addColumns`, `generateClass` and `newModel` take an optional `timestamp`
  (`2024_05_01_120000`, the client's local time) for the migration's file name; without it
  the server uses UTC.
- A field of `newModel` is `{ name, ty, nullable, unique, index, default, values,
  onDelete }`: `ty` is a `Blueprint` method; `values` are those of an `enum`; `onDelete`
  (`cascade`, `restrict`, `setNull`, `noAction`) is for a foreign key (`foreignId`,
  `foreignUuid`, `foreignUlid`); `morphs` makes the `{name}_type` and `{name}_id`
  columns. A relation is `{ kind, model, name, through }`, with `kind` one of
  `belongsTo`, `hasOne`, `hasMany`, `belongsToMany`, `hasManyThrough` (which names the
  model between in `through`), `morphOne`, `morphMany`, `morphToMany`, `morphTo`.
- A generator's `options` say which members of `spec` it reads (`model`, `related`,
  `table`, `create`, `event`, `queued`, `api`, `backing`, `status`); the `fields` of an
  `ide.json` code generation go in `spec.inputs`, by field id.
- A template the user edited is `<templates.path>/<name>.ft` in the project (default
  `.phoxite/templates`): `Laravel Model.ft` replaces the default "Laravel Model".

A minimal Neovim command listing the routes:

```lua
vim.api.nvim_create_user_command("LaravelRoutes", function()
  local client = vim.lsp.get_clients({ name = "phoxite" })[1]
  client:request("phoxite/laravel/routes", { query = "" }, function(_, routes)
    vim.ui.select(routes or {}, {
      format_item = function(r) return table.concat(r.methods, "|") .. " " .. r.uri end,
    }, function(route)
      if route then
        vim.cmd.edit(route.path)
        vim.api.nvim_win_set_cursor(0, { route.range.start.line + 1, route.range.start.col })
      end
    end)
  end)
end, {})
```
