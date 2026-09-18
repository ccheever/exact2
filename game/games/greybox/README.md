# Grey box

A Contract title screen and one `world(seed, paused)` canvas. The native HUD reads
`exactSurface("world")`; there is no app data module. WASD or arrows move, Space
jumps, E/Enter lights the beacon, and Pause/Resume changes a live typed argument.

```sh
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
harness but these game proofs have only been driven on web for this change.

`GreyboxArgs` declares seed then `#[live] paused`. `Mesh` carries primitive sizes;
a zero-lag `Follow` retains the original camera offset. The engine places the camera after setup; the game calls `scene::follow` in ticks.
`Move` accelerates and brakes, `Jump` starts a ballistic hop, and `Gravity` steps
vertical velocity; the game integrates position and clamps it to the ground. Seekable Sim
observes changed components, springs and explicitly reported work when settling.

Current native/Chrome pins: setup `0xbba6329f68d0c2f1`, W for 1,500 ms
`0x5d40bcb196c6e6e2`, player `[0, 0.9, -5.3666644]`. Read all components with
`state world:*`; hold a key with `type world KeyW for 1500`. These are forms of the
existing eight operations.

For interactive development from the repository root:

```sh
EXACT_APP_DIR="$PWD/game/games/greybox" bun host/web/dev.mjs --loopback
```

The proof also checks browser first paint, the lazy GPU fetch after Play and the
published timing fields. These measure rendering opportunities, not scanout.
