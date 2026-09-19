# Grey box

A Contract title screen and one `world(seed, paused, again)` canvas. The native HUD reads
`exactSurface("world")`; there is no app data module. WASD or arrows move, Space
jumps, E/Enter lights the beacon, and Pause/Resume changes a live typed argument.

```sh
bun game/games/greybox/proof.mjs linux
bun game/games/greybox/proof.mjs web
bun game/games/greybox/proof.mjs macos
```

[proof.mjs](proof.mjs) checks the HUD, input, picking, glow, settling, pause,
keyboard focus and fresh-process continuation. [pins.json](pins.json) owns the
expected hashes and save digests; receipts go in `artifacts/`.

See [the game setup and tick](logic/src/lib.rs) for Character movement, camera
following and synthesized sound. `state world:*` reads every component;
`type world KeyW for 1500` holds a key for a deterministic interval.

For interactive development from the repository root:

```sh
EXACT_APP_DIR="$PWD/game/games/greybox" bun host/web/dev.mjs --loopback
```

The nearest-beacon idiom intentionally changed the interaction to light one unlit
beacon per press. Previously every unlit beacon within the radius lit together.
The single target makes each press match the visible nearest-target interaction;
`near_xz` remains available for games that want the original all-in-radius behavior.
