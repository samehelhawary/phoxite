# Laravel (Phoxite) for OpenIDEA

The Laravel tools for [OpenIDEA](https://github.com/l0ng-ai/openidea), as a plugin on top
of `phoxite-lsp`. The server does the work; the plugin asks it through its
`phoxite/laravel/*` requests (`docs/editors.md`) and shows the answers.

So far:

- **Laravel Routes**, a tool window listing the routes by route file; open one to
  go to it.
- **Laravel: Go to Route…**, a picker over the same routes.
- **Route lenses**: clicking the routes lens on a controller method, or the URI after a
  route, goes to the route (or asks which, when there are several).

It needs Phoxite chosen as the PHP server (**Settings › Languages & Frameworks › Language
Servers**) and the `lsp:phoxite` permission, which lets the plugin send the running
server its own requests and run the commands the server's lenses carry. That is plugin
API 1.9, which is not released yet: the SDK is pinned to the OpenIDEA commit that added
it (`Cargo.toml`).

## Building

```sh
rustup target add wasm32-wasip2
cargo test
cargo build --release --target wasm32-wasip2
mkdir -p dist && cp target/wasm32-wasip2/release/phoxite_openidea.wasm dist/plugin.wasm
```

Then install this folder as a plugin from **Settings › Plugins** in OpenIDEA.
