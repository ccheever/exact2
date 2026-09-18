# Beacons

A small third-person game on exact2's game add-on. Light all three beacons by moving within 1.5 m and pressing E. WASD/arrows move, Space jumps, Pause/Resume freezes the simulation, and Play again resets the same seeded scene.

From the repository root:

```sh
bun game/games/beacons/proof.mjs web
EXACT_UPDATE_TRUST=development cargo test --manifest-path game/Cargo.toml -p beacons-logic
bun game/dev.mjs beacons
```

The game is `logic/`, `app.contract`, `app.json`, and `proof.mjs`. The shared
resolver generates its GPU/web/Apple shells in `game/.shells/`; the game workspace
owns the lockfile and build cache. On this Mac, set
`DEVELOPER_DIR=/Library/Developer/CommandLineTools` in the shell when needed.

`proof.mjs` drives actual browser input with the engine's shared harness. It asserts the pinned 1.5-second position, identical repeated runs, beacon emission at 0.1/1 seconds, HUD, frozen pause, byte-identical continuation in a fresh browser process, victory, restart, keyboard button activation, and child-process cleanup. Native simulation tests additionally check seek partitioning, seed variation, the 1.2 m jump, and no double jump.

Artifacts: `artifacts/proof.txt`, `replies.json`, `beacons-playing.png`, checkpoint/continuation saves, and `process-cleanup.json`. These and `target/`/`dist/` are generated outputs. `run-1.log` preserves the first proof's mistaken carrier-clock comparisons; `run-2.log` records the passing run. See the [build diary](../../diaries/001-beacons-exact-r2.md) for timings and limitations.
