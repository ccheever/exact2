# Beacons

A third-person game on exact2's `exact-game` simulation and `WorldSurface` renderer.
WASD/arrows move; Space jumps; E lights a nearby beacon. Pause freezes simulation
time. Light all three to reveal Play again. Seed 7 generates six crates.

The HUD reads `resource hud = exactSurface("world") as shape Hud` directly. Before
Play it has typed defaults; changed publications update it. There is no TypeScript
data module or JS engine. `World::emit` is the separate string-event path for a
canvas that declares a `message=` handler.

From the repository root:

```sh
bun game/games/beacons/proof.mjs
```

The proof builds through `host/web/build.mjs`, with `EXACT_APP_DIR` pointing here,
`EXACT_WEB_DIST` at this game's `dist/`, and `CARGO_TARGET_DIR` at its `target/`.
Both adapters use `exact-game-app` to bake the Contract UI; game logic edits do not
invalidate the UI bake. Existing asset directories are watched; when introducing
an asset directory, update `app.json` or its Contract reference as well.
The shared filesystem tool still builds under repository
`target/exact-filesystem-tool/`; all app staging is under the chosen Cargo target.

For interactive play after building:

```sh
EXACT_APP_DIR="$PWD/game/games/beacons" \
EXACT_WEB_DIST="$PWD/game/games/beacons/dist" \
CARGO_TARGET_DIR="$PWD/game/games/beacons/target" \
bun host/web/serve.mjs 8765 --loopback
```

Open http://127.0.0.1:8765 and press Play. Stop with Ctrl-C.

The proof opens and closes two fresh Chrome sessions using the existing agent
operations. HUD assertions and native/web hash pins cover movement, glow, pause,
completion and restart. It prints title first paint and wasm sizes. `artifacts/`
holds its screenshot, replies, transcript and saves. Two fresh native checkpoint
processes verify byte-identical continuation with held input and a future jump.
Browser save transport remains absent; this is a simulation persistence proof.

```sh
EXACT_UPDATE_TRUST=development CARGO_TARGET_DIR="$PWD/game/games/beacons/target" \
cargo test --offline --manifest-path game/games/beacons/Cargo.toml -p beacons-logic
```

Capsule radius is 0.4 m and total height 1.8 m; spheres have radius 0.5 m. Movement
uses fixed first-order acceleration, glow a 30-tick smoothstep, and camera following
0.1 per tick. All simulation state is `Data`; random draws and time come from the
world. The web proof is the interactive verification; Apple uses the same bake.

E1 measurement: 236 game lines (198 Rust + 38 Contract), 9 Rust adapter lines
or 62 with their Cargo manifests; the 199-line local builder is gone. A logic-only
edit rebuilt the world/GPU in 0.83 s and left the UI bake and app wasm unchanged
(the UI Cargo invocation took 0.08 s). These are warm local measurements.
