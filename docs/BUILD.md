
## Building something you can ship

A plain `cargo build --release` is **not** a shippable binary.

Tauri computes `dev = !custom-protocol`. Without that feature the interface is
loaded from `devUrl` — `http://localhost:5173` — instead of the bundle in
`ui/dist`, and a packaged application opens on

    Could not connect to localhost: Connection refused

The feature is declared in `crates/dj-app/Cargo.toml` and is deliberately not
in `default`, because `tauri dev` wants the dev server and its hot reload.
`tauri build` turns it on itself, so CI is fine; a hand-rolled build is not.

```sh
# the interface first -- tauri-build refuses to compile dj-app without it
( cd ui && npm ci && npm run build )

# then a real release binary
cargo build --release -p dj-app --features custom-protocol
```

To check a binary you already have, run it: a working build shows the mixer,
a broken one shows the connection error. There is no way to tell from
`strings`, because Tauri stores the embedded interface compressed and keeps
`devUrl` in the embedded config either way.
