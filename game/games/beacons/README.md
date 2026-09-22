# Beacons

A small exact2 game: walk a quiet field and light three beacons. WASD or arrow
keys move along world axes, Space jumps, E lights a nearby beacon. The on-screen
Move, Jump and Light controls also feed the engine input. Pause freezes the world;
Play again reconstructs it from seed 7.

From the repository root on this Mac:

```sh
export DEVELOPER_DIR=/Library/Developer/CommandLineTools
export SDKROOT=/Library/Developer/CommandLineTools/SDKs/MacOSX26.sdk
export EXACT_UPDATE_TRUST=development
export CARGO_TARGET_DIR="$PWD/game/games/beacons/target"
bun game/dev.mjs beacons
```

Command-line proof, with headless Chrome and a real GPU canvas:

```sh
bun game/games/beacons/proof.mjs web
```

The same proof runs on the GPU-less host with `linux`; that run explicitly skips
3D screenshot capture. `--paranoid` exercises continuous, Save and FreshGame modes.
`bun game/prove.mjs beacons --repin` generates the shared tick/save pins after
agreement across modes and hosts. Empty pins do not weaken the proof's independent
position, two-run state equality and fresh-process byte-equality assertions.

Host-free gameplay test:

```sh
cargo test --manifest-path game/games/beacons/Cargo.toml -p beacons-logic --offline
```

The nested workspace is a task-scope workaround: the standard starter and bake
would otherwise write shared shells and a shared lockfile. Engine dependencies
are unchanged path dependencies. The existing bake generates thin adapters in
`.shells/` here. All game build products live in `target/`; the proof writes its
transcript, snapshots, saves, cleanup receipt and `beacons.png` under `artifacts/`.

Evidence and limitations: [builder diary](../../diaries/001-beacons-exact-r5.md).
No headed feel or frame-pacing measurements were made by this builder.
