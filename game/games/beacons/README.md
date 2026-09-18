# Beacons

A small exact2 game: WASD/arrows move, Space jumps, E lights a nearby beacon.
Light all three, then Play again. Pause/Resume freezes the simulation.

From the repository root:

```sh
sh game/games/beacons/run.sh test
sh game/games/beacons/run.sh proof
sh game/games/beacons/run.sh dev
```

The launcher sets the required Apple SDK, development trust and game-local build
outputs. The existing generated Beacons host shells are reused without edits.
The target cache was seeded using an APFS copy-on-write copy of `game/target`.

`proof.mjs` drives three separate headless Chrome processes through the engine's
agent API. Evidence is in `artifacts/proof.txt`, `replies.json`, the binary `.world`
saves, `beacons.png`, and `process-cleanup.json`. It checks all six brief steps,
plus victory/restart and accessible UI. Native tests cover arrow/WASD parity,
seek partitioning, seed variation, speed/braking, jump height and no double jump.

See `../../diaries/001-beacons-exact-r4.md` for timings and limits. No headed feel,
native rendering parity or live hot-swap measurement is claimed.
