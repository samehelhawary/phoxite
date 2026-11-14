# Using phoxite-lsp in other editors

`phoxite-lsp` talks LSP over stdin/stdout. Download the binary for your platform from
the GitHub releases (or `cargo install --path crates/phoxite-lsp`) and put it on `PATH`.

Turn off other PHP language servers (Intelephense, Phpactor) for the same files; otherwise
completions and problems show twice.

Settings go in the initialization options as `{ "settings": { … } }` (see the README for the
keys); `cacheDir` sets the index folder.

## Neovim (0.11+)

```lua
vim.filetype.add({ pattern = { [".*%.blade%.php"] = "blade" } })

vim.lsp.config("phoxite", {
  cmd = { "phoxite-lsp" },
  filetypes = { "php", "blade" },
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
    "PHP": { "language_servers": ["phoxite", "!intelephense", "!phpactor"] }
  }
}
```

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
      "selector": "embedding.php | text.html.blade",
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
