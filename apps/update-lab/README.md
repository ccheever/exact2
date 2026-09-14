# Update Lab

A small, backend-free app for manually testing Contract, TypeScript, and Rust
updates on web, macOS, and iOS. All commands below run from the exact2 repo root.

Start the development server in one terminal (the dedicated output directory
keeps other apps' web builds separate):

```sh
EXACT_WEB_DIST=target/update-lab-web bun host/web/dev.mjs --app update-lab --port 8777
```

Open http://localhost:8777 in a browser. Leave the server running, then launch
each native app from another terminal with that same development URL:

```sh
bun host/apple/build.mjs update-lab-apple --run --url http://127.0.0.1:8777
bun host/apple/build.mjs update-lab-apple --ios --run --url http://127.0.0.1:8777
```

The iOS command uses a simulator. For a connected iPhone, use `--device` in place
of `--ios` and use the LAN URL printed by the server instead of `127.0.0.1`.
The phone needs a development signing profile covering `com.exact.updatelab`.

## The experiment

1. Increment the counter a few times and edit the note on each platform.
2. Both probes run automatically every 250 ms, matching the hosts' current
   clock cadence. Use **Pause** for manual testing with the two Run buttons.
   Each result records the counter value used for that invocation.
3. Edit one layer at a time and watch its version and result change:

| Layer | Edit | What to observe |
| --- | --- | --- |
| Contract | `app.contract`: `Contract v1` and the blue card's color | New UI; counter and note should carry across a compatible reload. |
| TypeScript | `app.ts`: `VERSION` and `MULTIPLIER` | New TS result on the next probe tick, without rebuilding the native executable. |
| Rust | `data/src/probe.rs`: `VERSION` and `MULTIPLIER` | The watcher builds the separate logic module, then all connected clients restart with carry and show its new answer on the next probe tick. |

Counter and note are deliberately in-memory Contract state, not durable storage.
A full browser reload or native process restart starts them over. **Reset** resets
only counter and note. Live mode also carries across compatible updates.

A result is the version that actually answered the last probe, not a claim
about the latest source on disk. Neither probe runs at bake or before first pixel.
Both have the same Contract result shape. `data/src/lib.rs` routes `rustProbe`
to real Rust on every platform and TypeScript calls to the existing executor.
`logic/` exports that Rust probe as a separate loadable module. Replacements use
browser Wasm on web, Wasm followed by native promotion on macOS development
builds, and interpreted Wasm on iOS devices and simulators. One verified
candidate contains both language modules;
changing either language retains the current version of the other.
The lab selects `rust.platforms.macos.dev = "tiered"`. The `Executor` line shows
the executor that actually answered its Rust diagnostic call.
On macOS, a new version can answer in Wasm while the OS assesses its native
library. Promotion changes only that Rust executor; it does not restart the
page, reset the note/counter, or restart TypeScript. If native loading fails,
the new Wasm continues. `rust.platforms.macos.dev = "native"` restores native-only
updates; `"wasm"` stays interpreted. The lab explicitly declares its zero-state
Rust module eligible using `export!(Probe, Probe, stateless)`.

The visible propagation delay includes up to one 250 ms probe interval after
the update commits. Probe execution is local; the timer does not poll the
development server, whose updates arrive through SSE.

Rebuild older native installations once with the commands above to install this
composition. Subsequent saves in `data/src/probe.rs` need no app rebuild or
manual Cargo command. Changes to the host or module ABI still require a new
client binary. The normal development server watches both languages on save;
`dev.rebuild.rust = "manual"` in `app.json` switches Rust to `r` + Enter in the
server terminal or `bun scripts/rust.mjs update-lab`. See the root README for
global, environment and platform controls.

This fixture exercises the local development loop. It does not configure signed
production delivery; the existing TypeScript module bake uses `deploy.store = 0`.
