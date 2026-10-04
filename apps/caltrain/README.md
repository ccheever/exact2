# Caltrain

The v1 app: the next trains from a Caltrain station, northbound and southbound,
with a station picker, search, nearby stations and a line map. It runs from one
`app.contract` on the web, macOS, iOS, tvOS and Linux, and every other host is
measured against it.

There is no TypeScript here. The data is a Rust crate, so this is the app to read
for a Rust data source, authored tests, routes and an optional GPU module.

## Run it

From the repository root:

```sh
bun host/web/dev.mjs                         # the web dev loop, at http://127.0.0.1:8765/
bun host/apple/build.mjs --run               # macOS
bun host/apple/build.mjs --ios --run         # an iOS Simulator
cargo build --release -p caltrain-linux      # Linux, headless anywhere
bun scripts/agent.mjs web --test apps/caltrain/app.test.contract
```

## Read in this order

| File | What it shows |
|---|---|
| `app.contract` | The whole interface: `routes`, one root component, the `resource` declarations that ask the data crate, derives, actions, and the view. |
| `app.test.contract` | Three tests in the agent's own operations (`tap`, `type`, `expect`), run on any host. |
| `data/src/lib.rs` | The data source: the corridor model, and the answer to each source the Contract names (`stations`, `nearest`, `station`, `board`, `search`). Values cross as the Contract's `shape`s. |
| `data/src/map.rs` | The line map, drawn with Canvas 2D. |
| `logic/` | The same data crate as a separately linked module, so it can be replaced live in development (LLP 1029.000). |
| `gpu/` | The optional GPU module: the aurora, the glass and the deck, as WGSL shaders loaded after first pixel (LLP 1009). |
| `app.json` | The manifest: identity, icons, host settings, delivery. |
| `web/`, `apple/`, `linux/` | Each host's crate: a few lines linking the plan and the data crate. |

For an app of your own, start with `bun scripts/exact.mjs new <path>`, which writes
a TypeScript app. The [human guide](../../docs/contract-for-humans.md) shows how to
use a Rust data crate instead.
