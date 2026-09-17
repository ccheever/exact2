# Beacons

A third-person game on exact2's `exact-game` simulation and `WorldSurface` renderer.
WASD/arrows move on world axes; Space jumps; E lights a nearby beacon. Pause freezes
simulation time. Light all three to reveal Play again. Seed 7 generates six crates.

From the repository root:

```sh
bun game/games/beacons/build.mjs
bun game/games/beacons/proof.mjs
```

The local builder is the repository web builder with imports and output paths
redirected so all generated files stay under this game. It selects development
trust, the private `target/`, and `dist/`. No engine or shared script changes.
Cargo uses the supplied lockfile and offline dependencies already on this machine.

For interactive play after building:

```sh
EXACT_APP_DIR="$PWD/game/games/beacons" \
EXACT_WEB_DIST="$PWD/game/games/beacons/dist" \
CARGO_TARGET_DIR="$PWD/game/games/beacons/target" \
bun host/web/serve.mjs 8765 --loopback
```

Open http://127.0.0.1:8765 and press Play. Stop the server with Ctrl-C.

`proof.mjs` opens and closes two fresh headless Chrome sessions using the existing
agent driver. It verifies the 1 mm movement pin, full-state repeatability, glow/HUD,
pause with a held movement key, winning, and restarting. `artifacts/` contains the
screenshot, full replies, state comparisons, transcript and simulation save files.
Step 5 uses two fresh native processes running this same game through `Sim`, with
a held key and a future queued jump across the checkpoint. It compares complete
binary saves after continuation. Browser save transport is absent from the driver;
this is not a browser-session save claim.

Logic tests (including jump height, no double jump, smooth camera, seeded scenery):

```sh
EXACT_UPDATE_TRUST=development CARGO_TARGET_DIR="$PWD/game/games/beacons/target" \
cargo test --offline --manifest-path game/games/beacons/Cargo.toml -p beacons-logic
```

The game has no textures or sound. Capsule dimensions are radius 0.4 m and total
height 1.8 m; spheres have the engine's default radius 0.5 m. Movement uses the
greybox's fixed first-order acceleration (0.2 per tick, 4 m/s target). The glow is a
30-tick smoothstep. Camera position follows at 0.1 per tick. All state is engine
`Data`; all random draws and time come from the world. Apple scaffolding is present
but the proof and delivery here target the web.
