# Beacons

A small third-person game on exact2's game add-on. Light all three beacons by moving within 1.5 m and pressing E. WASD/arrows move, Space jumps, Pause/Resume freezes the simulation, and Play again resets the same seeded scene.

From the repository root:

```sh
sh game/games/beacons/run.sh proof
sh game/games/beacons/run.sh test
sh game/games/beacons/run.sh dev
```

The launcher supplies this Mac's required `DEVELOPER_DIR`, development trust, and game-local build/output paths. The shared lockfile already has this game's packages. The proof also sets its own target directory when invoked directly with Bun.

`proof.mjs` drives actual browser input with the engine's shared harness. It asserts the 1.5-second position `[0, 0.9, -5.7333384]` within 1 mm, identical repeated runs, beacon emission at 0.1/1 seconds, HUD, frozen pause, byte-identical continuation in a fresh browser process, victory, restart, keyboard button activation, and child-process cleanup. Native simulation tests additionally check seek partitioning, seed variation, the 1.2 m jump, and no double jump.

Artifacts: `artifacts/proof.txt`, `replies.json`, `beacons-playing.png`, checkpoint/continuation saves, and `process-cleanup.json`. These and `target/`/`dist/` are generated outputs. `run-1.log` preserves the first proof's mistaken carrier-clock comparisons; `run-2.log` records the passing run. See the [build diary](../../diaries/001-beacons-exact-r2.md) for timings and limitations.
