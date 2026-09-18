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

`Character` uses 12 m/s² acceleration, 20 m/s² braking, a 4 m/s top speed,
and XZ bounds of ±19.6 metres. `near_xz` selects beacons around the player by name. The
90-tick W hash with the authored scene is `0x58d5d637a36c8365` on x86-64 Linux;
the previous procedural scene was measured on macOS and Chrome, which still need a sweep for this scene. The position is `[0, 0.9, -5.3666644]`. Gravity is 9.81 m/s²;
beacon glow uses a finite 0.5-second smoothstep tween. Scenery has no collisions.
The glow is material emission, without bloom. No textures or audio are used.

The ground and three beacon positions are now in `scene.json`, reusing Lanterns'
`../shared/solid.fragment.json`. Run `bun game/dev.mjs beacons --scene-only` to bake
content. Rust `spawn_crates` keeps procedural generation explicit and seeded.
See [typed scene usage](../../scene/README.md). Added scene/procedural provenance
changes world hashes; older comparison hashes above describe the previous artifact.
