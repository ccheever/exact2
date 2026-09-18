# Grey box

A Contract title screen and one `world(seed, paused, again)` canvas. The native HUD reads
`exactSurface("world")`; there is no app data module. WASD or arrows move, Space
jumps, E/Enter lights the beacon, and Pause/Resume changes a live typed argument.

```sh
bun game/games/greybox/proof.mjs linux
bun game/games/greybox/proof.mjs web
bun game/games/greybox/proof.mjs macos
```

The [shared harness](../../proof.mjs) builds changed inputs into this game's `dist/`
with `game/target/`, serves and drives fresh sessions, records operations and
assertions, then closes all sessions and checks every recorded child has exited.
The script proves world reads, real input, native hash parity, picking, glow, HUD,
settling, pause/resume, focused-button behavior, and byte-identical save continuation
in a second session. Artifacts are in `artifacts/` (`proof.txt`, `replies.json`, PNGs,
saves, `process-cleanup.json`). macOS capture needs screen-recording permission;
its existing proof skips screenshots when unavailable. iOS is supported by the
harness; this change was driven on web and macOS, with native parity also checked on x86-64 Linux.

`GreyboxArgs` declares seed, `#[live] paused`, and a boolean `#[restart] restart`. `Mesh` carries primitive sizes;
a zero-lag `Follow` retains the original camera offset. The engine places the camera after setup; the game calls `scene::follow` in ticks.
`Character` composes `Move`, `Jump` and `Gravity`, integrates the pose, lands at
0.9 metres and clamps XZ to ±19.6 metres. `near_xz` selects beacons by player name. Seekable Sim
observes changed components, springs and explicitly reported work when settling.

E5 pins checked in Chrome wasm and the Linux headless host on this arm64 Mac: setup `0x9a871d8582d905e7`, W for 1,500 ms
`0x71f43e51a13cc49f`, player `[0, 0.9, -5.3666644]`. Read all components with
`state world:*`; hold a key with `type world KeyW for 1500`. These are forms of the
existing eight operations.

For interactive development from the repository root:

```sh
EXACT_APP_DIR="$PWD/game/games/greybox" bun host/web/dev.mjs --loopback
```

The proof also checks browser first paint, the lazy GPU fetch after Play and the
published timing fields. These measure rendering opportunities, not scanout.


The HUD uses `publish_record(&Hud { beacons })` over `Data`; Contract checks its
shape. `world.character("player").step(wish, jump)` drives the named Character component;
its default speed, acceleration, braking, jump and gravity are explicit in setup. Proximity uses global positions and `nearest_xz_where` selects an unlit candidate
before comparing distance; equal distances choose the lowest entity index. Native
saves use `sim.save()?`; proofs use `session.world("world").save(path)` over the
existing screenshot-save transport. Native layout/pick reads have typed results.
Sound playback uses `&World` inside the beacon loop, and wind is
`AudioSource::new("wind").gain(0.3)`. The centred victory overlay declares Play again with an accessible name and autofocus.
The boolean restart edge uses the setup reconstruction path. The Character component
changes the old setup/forward hashes, while the 60 Hz movement and sound sequence stay
the same. No new x86-64 or macOS GUI sweep is claimed here.

The nearest-beacon idiom intentionally changed the interaction to light one unlit
beacon per press. Previously every unlit beacon within the radius lit together.
The single target makes each press match the visible nearest-target interaction;
`near_xz` remains available for games that want the original all-in-radius behavior.
