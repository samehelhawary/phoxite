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

Everything is plain LSP except two client commands the server's results carry:

| Command | Arguments | Where |
| --- | --- | --- |
| `laravel.routeActions` | the routes as a JSON string | route code lenses and URI hints |
| `laravel.modelView` | the model's class name | the code lens on Eloquent models |
| `phoxite.openLocation` | an LSP `Location` | quick-fixes that create a file (a missing view) |

and one server command, `phoxite.rescan` (`workspace/executeCommand`), which indexes the
workspace folders again.
