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
harness; this change was driven on web and macOS, with native parity also checked on x86-64 Linux.

`GreyboxArgs` declares seed then `#[live] paused`. `Mesh` carries primitive sizes;
a zero-lag `Follow` retains the original camera offset. The engine places the camera after setup; the game calls `scene::follow` in ticks.
`Character` composes `Move`, `Jump` and `Gravity`, integrates the pose, lands at
0.9 metres and clamps XZ to ±19.6 metres. `near_xz` selects beacons by player name. Seekable Sim
observes changed components, springs and explicitly reported work when settling.

Current arm64 macOS / x86-64 Linux / Chrome wasm pins: setup `0x7df5e5a89b4d0207`, W for 1,500 ms
`0x0f14b8b231091d12`, player `[0, 0.9, -5.3666644]`. Read all components with
`state world:*`; hold a key with `type world KeyW for 1500`. These are forms of the
existing eight operations.

For interactive development from the repository root:

```sh
EXACT_APP_DIR="$PWD/game/games/greybox" bun host/web/dev.mjs --loopback
```

The proof also checks browser first paint, the lazy GPU fetch after Play and the
published timing fields. These measure rendering opportunities, not scanout.


The HUD uses `publish_record(&Hud { beacons })` over `Data`; Contract checks its
shape. Player movement requires `query.one().expect("one player")`, and only ground
and bounds override `Character` defaults. Proximity uses global positions. Native
saves use `sim.save()?`; proofs use `session.world("world").save(path)` over the
existing screenshot-save transport. Native layout/pick reads have typed results.
Sound playback uses `&World` inside the beacon loop, and wind is
`AudioSource::new("wind").gain(0.3)`. This game has no restart control.
