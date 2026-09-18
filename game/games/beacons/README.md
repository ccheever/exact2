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
bun game/prove.mjs beacons --hosts web,linux --repeat 2 --compare-saves
cargo test --manifest-path game/Cargo.toml -p beacons-logic
bun game/dev.mjs beacons
```

The bake refreshes tracked `logic/Cargo.toml` and `app.json` with generated headers;
an authored file without the header wins. No per-game launcher or authored host crate is needed.

`proof.mjs` drives three fresh host processes through the shared agent API.
Linux checks the same HUD and accessible controls without a GPU; only the
screenshot is web-only. Evidence is in `artifacts/proof.txt`, `replies.json`, the binary `.world`
saves, `beacons.png`, and `process-cleanup.json`. It checks all six brief steps,
plus victory/restart and accessible UI. Native tests cover arrow/WASD parity,
seek partitioning, seed variation, speed/braking, jump height and no double jump.

See `../../diaries/001-beacons-exact-r4.md` for timings and limits. No headed feel,
native rendering parity or live hot-swap measurement is claimed.

E5 retains r4's 60 Hz movement and plain ground. Character is now its own saved
component, so the tick-907 endpoint hash moves from the pre-E5 `0x331c074e0f135059` to
`0x7dde46ef4bc4bdb6`; restart is a `#[restart]` boolean edge through setup.
The final save is 5,505 bytes, SHA-256
`e388bd748ade00ee7d4a4c50fcaba89024842005d200bef9697a5faae4e75a15`, identical
across both hosts and fresh-process restore. Warm Linux: 0.47–0.53 s external wall; one internal run was 0.506 s; the two-host repeated proof reports Linux 1.389/0.503 s and web 5.447/5.879 s.
Those Linux runs use the headless host on this arm64 Mac, not a new x86-64 sweep.
The global process audit stalled in ps on web; owned carrier processes were awaited.
