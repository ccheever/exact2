# Grey box

An ordinary exact2 app: a Contract title screen, then one `world(seed, paused)`
canvas with two HUD children. `app.ts` shape-checks the world's JSON message;
`heardMessage → hud → text` is the whole return path. The GPU crate is one macro
invocation, and the web and Apple adapters follow Weatherlight.

From the exact2 repository root:

```sh
export EXACT_APP_DIR="$PWD/game/games/greybox"
export EXACT_UPDATE_TRUST=development
export CARGO_TARGET_DIR="$PWD/game/target"
bun host/web/build.mjs greybox-web
bun game/games/greybox/proof.mjs web
```

`EXACT_APP_DIR` must name this Cargo workspace, not `game/`. The ordinary build
places the result in `host/web/dist/`; `EXACT_WEB_DIST` can isolate it. The proof
serves that build through the driver's own ephemeral server, uses one headless
Chrome session at 1280×720, and closes both. It sets `EXACT_APP_DIR` itself.
Artifacts default to `game/target/greybox-proof/`; `EXACT_GREYBOX_PROOF` overrides
the scratch directory. Any failed assertion marks a nonzero exit immediately;
the run continues to report the remaining failures.

For an interactive dev loop or an individual transcript:

```sh
bun host/web/dev.mjs --app greybox
bun scripts/agent.mjs web --app greybox --size 1280x720 tree 'tap play' \
  'tree world' 'type world key KeyW down' 'clock +1500' 'state world:player' state
```

WASD or the arrow keys move, Space jumps, and E/Enter lights the beacon when near
it. Pause/Resume changes the canvas's live `paused` argument. Sun shadows and HDR
bloom use the scene vocabulary's existing defaults; no renderer switch is added.

The native test now pins `0x70c17d4a69834418` after W for 1,500 ms, with player
position `(0, 0.9, -5.733332)`. Brief B1b's `0x4dcde63de7f70139` is the previous
schema's hash: commit `46fea5b` added `DirectionalLight.shadows` and environment
support and updated the native golden. The smoke asserts the current native pin
and prints both values explicitly; it does not change the simulation to recover
an obsolete hash.

`state.world[0].perf` contains wall-clock observations outside the world hash:
browser first-contentful-paint from navigation, the existing module `gpuMs`,
trusted input time, first render submission, and the following rendering
opportunity. `inputToFirstFrameMs` measures Play to that opportunity, not display
scanout or GPU execution time. Resource Timing entries prove the GPU glue,
bindings, and engine Wasm requests all begin after title paint and Play. The
proof also prints raw and per-file gzip sizes and its full wall time.

Two integration defects surfaced in the first drive. Entity `state` rounded
component floats to four decimals despite an exact native/web hash; the engine's
component/resource JSON now preserves float precision (layout stays rounded).
Outside agent mode, Play queued a surface but never started the GPU fetch; the
surface queue now schedules the existing loader after a rendering opportunity.
That host fix replaces one line in `glue.js`, leaving it at 1,498 lines. Timing
observations live in `gpu-glue.js`; the driver timestamps trusted clicks before a
lazy module can exist. No new operation or renderer switch is needed.

Authoring finding: this HUD takes a single shape, resource and message action,
with the text and button directly inside the canvas. Absolute CSS positioning
expresses the two corners without a layout workaround. The JSON boundary needs
an explicit TypeScript check because the surface's messages are strings.

Measured on this Mac in a fresh headless Chrome profile at 1280×720 (one run,
with other builds active): title first-contentful-paint 396 ms from navigation;
Play at 542 ms; GPU glue/bindings/Wasm requests at 570.6/574.0/579.9 ms; first
world rendering opportunity at 634.6 ms, or 92.6 ms after Play; `gpuMs=31`.
The proof reported 8.814 s; an external process monitor measured 9.114 s through
exit code 0 and confirmed all 18 recorded descendant PIDs were gone. Like the
driver's CLI, the proof explicitly exits after closing its session, invoking the
shared filesystem reader's existing exit hook. No filesystem-tool change is made.

| Dist artifact | Raw bytes | Gzip bytes |
| --- | ---: | ---: |
| `app.wasm` | 734,612 | 315,574 |
| `gpu_bg.wasm` | 479,142 | 213,800 |
| `gpu.js` | 73,003 | 13,334 |
| Rest | 2,155,347 | 753,299 |

Gzip is level 9, summed per file, including every file in `dist/`. The screenshot
shows a sunlit gold capsule, cast shadows, a bright blooming beacon overlapping
the player, and crisp white HUD text on dark panels. The ground has visible fine
banding; this is a rendering-quality finding, not hidden by the smoke.

Validation: web build, all grey box workspace crate checks (including Apple),
engine/logic tests, focused Clippy and formatting, root workspace build,
root Clippy/formatting, caps and boot passed. The full root test run was stopped
after 22 minutes, still in repeated macOS native TypeScript-tool launch stalls;
it is not claimed green. Its partial log is `game/target/greybox-root-test.log`.
The normal-page check separately confirmed that Play starts GPU loading without
an agent request. All processes launched for this work were stopped; no commit.
