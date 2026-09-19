# Lanterns — Godot 4.7.2

A complete one-level 3D game built against the shared Lanterns contract. The
scene is constructed in Godot at runtime from the unchanged `level.json`.
The fox is the unchanged Khronos `Fox.glb`; its license is in
`assets/Fox-LICENSE.md`.

## Play

Use WASD or the arrow keys to walk, hold Shift to run, Space to jump, and E to
light the nearest lantern. The on-screen direction, Jump, and Light buttons
provide the same controls for touch. Pause, Save, Load, and Sound controls are
always visible. Push the real rigid-body crate against the eastern ledge, jump
onto it, and jump again to reach lantern 12.

One delivered-input deviation is recorded: ordinary human directional input
walks at 2.4 units/s and Shift runs at the shared 4.5 units/s. The common
browser API always uses the specified 4.5 units/s. Other comparison lanes may
use 4.5 units/s for all human movement, so human movement speed is not an exact
cross-game parity claim.

The 1280×720 game remains landscape-letterboxed in a 390×844 portrait viewport.
All touch hit targets work, but the Godot web export's fallback font rendered
the four arrow glyphs as missing-glyph boxes in headless Chromium. Their fixed
up/left/down/right layout remains clickable; this is a recorded presentation
limitation, not a claim of polished portrait UI.

The game has title, pause, win, and loss screens. It saves the clock, player
transform and velocity, crate transform and velocity, lit set, and phase.
Native saves use `user://`; web saves use the same file plus a localStorage
mirror so a browser reload can resume immediately.

## Build and run

The project pins Godot 4.7.2 and Bun 1.3.14. Install the matching Godot web
export templates in the normal user-local template directory, then:

```sh
bun install --frozen-lockfile
GODOT=/path/to/Godot_v4.7.2-stable_linux.x86_64 bun run build
bun run serve
```

Open <http://127.0.0.1:4173/>. The checked-in `dist/` is the production web
export used for the recorded proof. Godot generates that bundle directly; no
application JavaScript is bundled, so Rolldown has no input in this lane.

Useful verification:

```sh
GODOT=/path/to/godot bun run smoke:native
CHROMIUM=/path/to/chromium bun run smoke:web
CHROMIUM=/path/to/chromium bun run playthrough
```

`?agent=1` exposes the common async
`window.lanterns.command(request)` API. Agent mode disables wall-clock
gameplay and advances only queued steps. Godot's time scale and physics tick
rate rise together, so each CharacterBody/RigidBody solver step remains 1/60
second. The crate freezes between requests.

## Evidence and timing

- `evidence/native-smoke.txt`: native headless import and 60 live physics ticks.
- `evidence/web-gameplay.png`: rendered gameplay after API movement.
- `evidence/title.png`, `crate.png`, and `won.png`: independent acceptance
  captures, including the completed crate/ledge/all-12 route.
- `evidence/clock-smoke.txt`: isolated 10,800-tick loss in 3.018 s and
  batch-versus-single fixed-step equivalence for player and crate.
- `evidence/touch-controls.png`: 390×844 canvas after real UI clicks exercised
  Start, Pause/Resume, Sound, Save, and Load.
- `evidence/natural-lost.png`: normal non-agent mode reached the loss screen at
  tick 10,800 in 182.316 s of wall time.
- `evidence/final-v3.json`: independent final acceptance. It passed 9/10:
  imported asset/title, keyboard, fixed movement/jump, pause, wall, dynamic
  crate, reload persistence, the complete crate/ledge victory route, and
  batch-versus-single equivalence including the crate.

The remaining limitation is exact and preserved: isolated `step(10800)`
passes in 3.018 s, while the same request late in the long sequential
acceptance run times out at the adapter's 60 s bound. The following batch case
still passes, proving cancellation prevents the timed-out job from leaking
ticks. The ordinary human game naturally loses at 180 seconds. Because the
sequential controlled-clock case is red after the three-fix limit, this build
is a playable baseline but is not eligible for clock-qualified change trials.

Setup and implementation were recorded separately:

| Work | Measured time |
| --- | ---: |
| Bun dependency install | 0.131 s |
| Godot 4.7.2 export-template download | 18.19 s |
| First complete asset import | 2.64 s |
| Setup subtotal | 20.96 s |
| Source creation to first native proof | 6m 37s |
| Source creation to first web move/save/reload proof | 12m 23s |
| Source creation through independent full victory result | 25m 10s |

The Godot executable and Chromium were already warm on the assigned machine and
are excluded from setup. Browser shared-library extraction was user-local and
not timed, so it is not included in the subtotal.

The web export is about 39 MB raw. Its main Wasm is 39,514,754 bytes raw and
10,114,302 bytes gzip; the game pack is 159,436 bytes raw and 156,500 bytes
gzip. These are direct Godot export artifacts, not network transfer
measurements.
