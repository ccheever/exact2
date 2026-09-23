# Beacons

A small exact2 game: walk a quiet field and light three beacons. WASD or arrow
keys move along world axes, Space jumps, E lights a nearby beacon. The on-screen
Move, Jump and Light controls also feed the engine input. Pause freezes the world;
Play again reconstructs it from seed 7.

The game is `logic/src/lib.rs`, `app.contract`, `proof.mjs`, its hostless tests
in `logic/tests/` and the proof's `pins.json`. From the repository root:

```sh
bun game/dev.mjs beacons                    # live world; gameplay edits carry state
bun game/prove.mjs beacons                  # GPU-less Linux proof against pins.json
bun game/games/beacons/proof.mjs web        # headless Chrome with a real GPU canvas
bun game/app/shells.mjs game/games/beacons --test   # Rust tests
```

The Linux proof skips 3D screenshot capture; `--paranoid` exercises continuous,
Save and FreshGame modes, and `bun game/prove.mjs beacons --repin` regenerates the
tick/save pins after every mode and host agrees. The proof writes its transcript,
snapshots, saves, cleanup receipt and `beacons.png` under `artifacts/`.

Evidence and limitations: [builder diary](../../diaries/001-beacons-exact-r5.md).
No headed feel or frame-pacing measurements were made by this builder.
