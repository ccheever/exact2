# Beacons

A small exact2 game: WASD/arrows move, Space jumps, E lights a nearby beacon.
Light all three, then Play again. Pause/Resume freezes the simulation.

From the repository root:

```sh
export DEVELOPER_DIR=/Library/Developer/CommandLineTools
export SDKROOT=/Library/Developer/CommandLineTools/SDKs/MacOSX26.sdk
export EXACT_UPDATE_TRUST=development EXACT_IDENTITY=-
bun game/games/beacons/proof.mjs linux
bun game/games/beacons/proof.mjs web
env -u SDKROOT DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer bun game/games/beacons/proof.mjs ios
env -u SDKROOT DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer bun game/prove.mjs beacons --hosts web,linux,ios --repeat 2 --compare-saves
cargo test --manifest-path game/Cargo.toml -p beacons-logic
bun game/dev.mjs beacons
```

The bake uses [generated host adapters](../../app.mjs) and per-key manifest overrides.

`proof.mjs` drives three fresh host processes through the shared agent API.
Linux checks the same HUD and accessible controls without a GPU; screenshots run on web and Apple hosts. Evidence is in `artifacts/proof.txt`, `replies.json`, the binary `.world`
saves, `beacons.png`, and `process-cleanup.json`. It checks all six brief steps,
plus victory/restart and accessible UI. Native tests cover arrow/WASD parity,
seek partitioning, seed variation, speed/braking, jump height and no double jump.

See `../../diaries/001-beacons-exact-r4.md` for timings and limits. No headed feel,
native rendering parity or live hot-swap measurement is claimed.

E5 retains r4's 60 Hz movement and plain ground. Character is now its own saved
component; the tick-907 endpoint is in [pins.json](pins.json); restart is a `#[restart]` boolean edge through setup.
Continuation hashes and save digests are recorded in [pins.json](pins.json).
