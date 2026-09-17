# Beacons

A capsule, six seeded crates, three beacons, and a following camera. WASD or arrow
keys move, Space jumps, E/Enter lights a nearby beacon. The HUD and pause/restart
buttons are ordinary Contract views reading `exactSurface("world")`.

```sh
bun game/games/beacons/proof.mjs          # web
bun game/games/beacons/proof.mjs macos
```

The [shared harness](../../proof.mjs) builds changed inputs, records every operation,
closes every session, and verifies recorded children have exited. The proof runs
the route twice, compares every entity via `state world:*`, then captures a held-key
and queued-jump save through `screenshot ... world save` and resumes it in a fresh
session. Complete final save bytes must match. `artifacts/` holds the transcript,
replies, screenshots, saves, and process cleanup evidence.

`BeaconsArgs` declares positional order: `world(seed, run, paused)`. Changing seed
or run rebuilds; `#[live] paused` freezes the existing world. Tick reads typed
arguments. Geometry sizes live in `Mesh`; the camera is a saved `Follow` stepped by
`scene::follow(world)`. Movement uses the arriving `math::ease` time constant
0.074690334 seconds (the former 0.2 response at 60 Hz). Glow remains the game's
30-tick smoothstep. Sim observes settling; the game reports no busy bookkeeping.

Native and Chrome pin W for 1,500 ms at `0xf1bdfbe68b382647`, position
`[0, 0.9, -5.7333384]`. The 6 µm position change comes from easing arriving exactly
at the requested velocity. Tests run with the engine workspace, including jump,
seed, pause, restart and save/restore assertions:

```sh
cd game
EXACT_UPDATE_TRUST=development cargo test -p exact-game --test beacons
```

For the world dev loop, use `EXACT_APP_DIR="$PWD/game/games/beacons" bun
host/web/dev.mjs --loopback` from the repository root. Rust world edits reload the
surface module and carry its world; Contract edits use the same shared dev server.
