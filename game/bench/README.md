# bench — the same scenes here, in Godot and in three.js

Diagnostic, never a check (LLP 1041.000 §6a). `bun game/bench/run.mjs <engine> <scene>
<n> [mode] [variant]` prints one JSON line. Every engine presents to the display at
2560×1440 pixels, 4× MSAA, vsync on, so the honest question is **the largest N that
still holds the refresh rate**, with the script and render milliseconds beside it.
A run records the machine's load; this machine is shared, so compare runs taken together.

Each twin is given its best idiomatic effort and its best optimized one — a strawman
twin measures nothing. Here there is one mode: you spawn entities.

## Scenes

**`cubes`** — N unit cubes on a cubic grid (side ⌈∛N⌉, spacing 2, centred), cube *i*
turning about axis normalize(frac(i·0.3719)−½, frac(i·0.7331)−½, frac(i·0.1913)−½) at
0.5 + (i mod 7)·0.25 rad/s, coloured by hue frac(i·0.61803). One directional light, no
shadows, flat ambient. A 60° camera orbits at radius 1.8·side + 6, height 0.35·radius,
period 20 s. It measures what it costs to move many things: per-entity update, the
transform path to the GPU, the draw.

Modes — Godot: `nodes` (a `MeshInstance3D` each), `multimesh` (one `MultiMesh`, set from
GDScript each frame), `shader` (the rotation in the vertex shader; no CPU work). three.js:
`meshes` (a `Mesh` each), `instanced` (one `InstancedMesh`, set from JS each frame).

## Baselines — 2026-09-17, M5 Max, macOS, 120 Hz, machine under other load

| engine | mode | N | fps | p95 ms | script ms |
|---|---|---|---|---|---|
| Godot 4.7.2 Forward+ | nodes | 2,000 | 119 | 9.3 | — |
| | nodes | 10,000 | 18 | 56.1 | — |
| | multimesh | 100,000 | 118 | 8.8 | 5.9 |
| | shader | 1,000,000 | 119 | 8.6 | 0 |
| three.js r186 WebGL | meshes | 10,000 | 57 | 19.7 | 2.3 |
| | instanced | 100,000 | 120 | 9.9 | 2.2 |
| | instanced | 200,000 | 120 | 9.9 | 4.1 |
| three.js r186 WebGPU | meshes | 10,000 | 20 | 58.8 | 2.3 |
| | instanced | 100,000 | 120 | 10.0 | 2.3 |

`bun game/bench/run.mjs sweep <engine> cubes <mode> [variant]` finds the largest N that
holds the refresh rate (≥ 96% of 120 fps and p95 under one and a half intervals):

| engine | mode | holds | breaks | what gives out | load |
|---|---|---|---|---|---|
| Godot 4.7.2 Forward+ (native) | nodes | 1,500 | 2,000 | draw calls | 72 |
| | multimesh | 50,000 | 53,500 | GDScript (5 ms) | 111 |
| Godot 4.7.2 web (WebGL2) | multimesh | 66,000 | 69,000 | GDScript in wasm | 16 |
| three.js r186 WebGL | instanced | 225,000 | 237,500 | the JS loop (8 ms) | 30 |

(The native Godot MultiMesh run held 100,000 at 118 fps when the machine was quieter;
its sweep was taken under a load average of 111. Compare runs taken together.) Godot's
web export is 39.5 MB of wasm — 10.1 MB gzipped, 7.9 MB brotli — before a game is in it.

The bar these set: the idiomatic path here — one entity per cube — should hold 120 Hz
past where both twins' *optimized* paths stop: past 225,000 in a browser.

## Cubes entry — 2026-09-17, display locked; comparison pending

**No new refresh-rate results or sweeps were taken.** `ioreg` reported
`CGSSessionScreenIsLocked=Yes` before validation and again afterward. A headed
window cannot present in that state. In particular, whether Exact holds 120 Hz
past **225,000 browser cubes remains unanswered**. The older twin numbers above
are not substitutes for the requested interleaved sitting.

| N | exact-web | three instanced (webgl) | godot-web multimesh | exact-macos | godot multimesh (native) |
|---|---|---|---|---|---|
| 10,000 | pending | pending | pending | pending | pending |
| 100,000 | pending | pending | pending | pending | pending |
| 200,000 | pending | pending | pending | pending | pending |
| 500,000 | pending | pending | pending | pending | pending |

Cells will be **fps / p95 ms / script or tick ms**, with each run's `load1`.
The five sweep hold/break results are also **pending**, for the same reason.

Run this one command with the display unlocked and available to the bench:

```sh
bun game/bench/run.mjs compare
```

It runs all five engines sequentially at each N, retakes the whole group when
its highest load exceeds twice its lowest (three attempts maximum), then runs
all five sweeps. Raw JSON lines, including rejected groups, go to
`game/bench/results/cubes-<timestamp>.jsonl`; its sibling Markdown file contains
the comparison and sweep tables. Every sweep trial retains its load and Exact's
complete phase distributions. Keep the measured window frontmost; the runner
refuses a locked display. Do not run multiple bench builds concurrently: baking
N temporarily changes this app's Contract state, restoring it in `finally`.

Individual runs and build/sanity commands:

```sh
bun game/bench/run.mjs exact-web cubes 100000
bun game/bench/run.mjs exact-macos cubes 100000
bun game/bench/run.mjs sweep exact-web cubes
bun game/bench/run.mjs sweep exact-macos cubes
BENCH_BUILD_ONLY=1 bun game/bench/run.mjs exact-web cubes 100000
BENCH_BUILD_ONLY=1 bun game/bench/run.mjs exact-macos cubes 100000
bun game/bench/cubes/proof.mjs
```

Both app builds passed locally, including the ad-hoc signed macOS executable.
The temporary Swift wrapper supplies `--build-system native` and is removed
when the build ends. The web runner completed a live-clock **headless sanity**
at 100,000 cubes: 100,000 rendered instances, 100,002 entities, 2560×1440 pixels,
phase samples through CDP, and clean Chrome exit. The checked-in proof uses
1,000 cubes for a short repeatable sanity run. The existing three.js path also
completed a short headless sanity. Headless lines carry `measurement: "sanity-only"`
and cannot be swept. Their FPS values are not display measurements. A read-only macOS `EXACT_AGENT=live` launch also answered readiness, tree and
world/perf state (100,002 entities) without a clock command and exited cleanly.
Its drawable size was zero under the lock, so live presentation remains unverified.

`cubes/` was generated with `bun game/new.mjs bench-cubes`, moved here, and
registered in the shared game workspace. Its only update path is one entity per
cube and two ordinary queries: precomputed quaternion step multiplication at
60 Hz, then the camera orbit. Grid, axes (fractions evaluated in double precision
as in the twins), speeds, 20-second orbit, 60° camera, clipping planes, roughness,
no shadows, no bloom, 4× MSAA, viewport and scale are explicit. The twins themselves
differ: three.js uses HSL(h, .6, .6) and light position (.38, .77, .51); Godot uses
HSV(h, .6, .9) and Euler angles (-50°, -30°, 0°). Exact follows **three.js** for
those authored values. Each engine retains its own material/tonemapping pipeline;
Exact's two draws are cubes plus its normal HDR resolve/tonemap pass.

Contract's `searchParam` returns text, and its stdlib has no numeric conversion;
`#[derive(Args)]` requires a number for `u32`. Thus the runner sets `BENCH_N` and
bakes the requested N into `state n`, restoring the source default **100000**
afterward. The host shells are the generator's. Bakes are reused only when the
source fingerprint, N, and output artifact hashes match. The normal repository
builders receive `EXACT_APP_DIR`, development trust and the CommandLineTools
`DEVELOPER_DIR`; web output is the normal `host/web/dist`.

Small changes outside `bench/`: workspace membership/lockfile; optional
`Environment.background` so a dark flat background does not also darken ambient
lighting (defaults preserve existing scenes); `WorldSurface` perf ring counts,
means, dimensions and a `state` request with `perf_reset: true`; and macOS's
`EXACT_AGENT=live` enabling its **existing stdio** agent reader without setting
the seekable clock. macOS uses stdio, not the iOS agent socket. No clock command
is sent. No engine optimizations were made from headless timings.

The 16,384-sample phase rings are reset after two seconds of warm-up and read
once after `BENCH_SECONDS` (default eight). `fps_avg` is 1000 / `frameMs.mean`;
percentiles/max come directly from that ring. `tick_ms` is mean time per 60 Hz
simulation tick, `feed_ms` per history feed, and `encode_ms` per rendered frame
(including CPU upload/encoding work); these have different sample counts and
must not simply be added as a per-frame total. `ticksPerFrame` is retained for
interpretation. GPU execution/compositor time is **not** measured by encode time.
Readback/hash work happens at the window boundaries, not on each measured frame.

Validation: cubes logic plus renderer tests passed (63 passed, four ignored),
including local Metal pixel tests; targeted Clippy and formatting passed. The
root build, Clippy/format, caps and boot checks passed. Root tests were stopped
when two TypeScript subprocesses stalled beyond a minute; the provided Linux
builder fallback could not complete them (its trimmed workspace lacks
`exact-filesystem`, and the TypeScript executable is absent). These unrelated
verification limitations were not repaired in this change. No commits were made.

There is no honest new bottleneck conclusion for any of the five engines while
the display is locked. The older twin runs suggest CPU script work limits their
instanced/MultiMesh paths, but changing load already moved those limits. At
Exact's first failing browser N, compare tick and feed costs (scaled by tick/feed
counts per frame), encode cost, and frame cadence; high residual time calls for
a GPU/upload trace, not a claim that the GPU is slow. The interleaved rerun and
that phase breakdown are required before choosing an optimization or declaring
the 225k bar passed or failed.

## This engine, so far — the simulation alone (no renderer yet)

`cargo run --release --example churn -p exact-game`, 2026-09-17:

| | M5 Max (arm64 macOS) | EPYC 9454 (x86-64 Linux) |
|---|---|---|
| turn every cube, per entity per tick (500,000 entities) | 3.6 ns | 5.8 ns |
| so one 60 Hz tick of 500,000 turning cubes | 1.8 ms | 2.9 ms |
| despawn and respawn 1,000 of 100,000 | 0.49 ms | 0.30 ms |
| a query over 12 entities in a world of 100,000 | 1.7 µs | 1.1 µs |
| world hash after 200 ticks × 500,000 entities | `fbe3a8fa56f19d95` | `fbe3a8fa56f19d95` |

The last row is the determinism contract's first evidence: two architectures, two
operating systems, the same bits.

## Feel — Beacons, live clock

```sh
bun game/bench/feel.mjs exact
bun game/bench/feel.mjs exact --hz 120
bun game/bench/feel.mjs compare
# Trust the existing bakes while another lane edits build inputs:
bun game/bench/feel.mjs exact --game beacons --no-build --attempts 1
```

One command does **preflight → prepare → warm → three attempts → rows/table**.
Before any preparation or launch, macOS `ioreg` must report `IOConsoleLocked=No`
and `HIDIdleTime` ≥ 30 seconds. Locked, active or unreadable console state refuses
the command immediately; it never waits and retries that refusal. Preflight is
checked again after preparation, at every attempt, every five seconds during
capture, and at completion. Leave the keyboard/mouse untouched and the display
unlocked throughout. High load does not refuse a capture: any sampled `load1 > 8`
marks its row **PROVISIONAL**, including otherwise valid rows.

Ordinary 60 Hz preparation uses `game/proof.mjs`'s existing content/artifact-digest
build gate with an empty proof callback: it builds only when stale, then the
bench warms and measures. **`--no-build` skips the staleness/build path**, trusts
existing artifacts, fingerprints them, and records the skipped freshness check.
`--hz 120` selects the existing `games/<game>/target/feel120` bake and labels the
row **exact/120hz**; the ordinary dist is **exact/60hz**. The actual world tick rate
must match the label in every trace, even with `--no-build`.

`--game` defaults to `beacons`; Exact's observer is bench-owned and accepts
`--entity player` and `--play '[data-testid="play"]'`. Other games need the same
live-host trace protocol. The twins and `compare` currently support Beacons only.
`compare` interleaves exact/60hz, exact/120hz, three/shipped, godot/shipped and
godot/interpolation, three times. Individual `three` and `godot` commands also
remain available. Exactly **three attempts per variant** are taken by default;
invalid input/focus/latency rows are retained and count toward that budget. There
are no automatic retakes. `--attempts 1` permits a single diagnostic attempt,
including when it fails; high load never adds an attempt.

JSON lines go to stdout and are also appended to
`results/feel-YYYY-MM-DD.jsonl` (UTC date); progress and a Markdown table go to stderr. The table is also saved as
`results/feel-<batch>.md`, including completed rows if a later preflight aborts.
Each row links a gzipped JSON trace containing **every frame and delivered event**.
There is no browser package or new dependency: Bun serves the existing local three.js,
plain `fetch` and `WebSocket` speak CDP, and a fresh **headed** Chrome gets a unique
`game/bench/.feel-*/exact2-feel-chrome` profile. Every launched PID is printed; cleanup
kills only that recorded child if needed, awaits exit, checks `ps`, and removes the
temporary profile. Godot exits after writing its one result file. SIGINT/SIGTERM,
startup failures, CDP errors and timeouts also pass through cleanup.

`--seconds` is the **minimum main-script window**, not a shortened play script or a
trial count. Default 12 seconds plus latency trials takes about **81 seconds per
run**, excluding startup: about 4½ minutes for three.js and 8½ for both Godot rows.
The minimum accepted value is 7.02; a 10.02-second floor before trials lets movement
stop. Larger values add idle time before the same 20 trials. `CHROME` and `GODOT` can
override executable paths. The default Godot is the cached 4.7.2 app. Foreground and
display-mode confirmation currently use macOS AppKit/CoreGraphics through JXA.
Keep the display awake and let the measured window retain focus during the command.

### Method and boundaries

The runner presses the focused **Play** button with Enter, waits 0.5 seconds, holds
W 2.5 seconds, D 1.5 seconds, taps Space for 20 ms, holds S 1.5 seconds, then idles
at least 1 second. Releases precede the next press. All scheduling uses the live
monotonic clock: no `?proof`, `--fixed-fps`, manual advance, virtual time, or disabled
vsync. Startup and shader warmup occur before the measured Play event. Delivered
event times, actual W duration and each event's deviation from the target schedule
are retained, so shared-machine scheduling delays are visible.

The game source is unchanged. The HTML conditionally loads the adjacent `feel.js`
before `game.js` only for `?feel=1`. The Godot scene has an adjacent `Feel` observer
which disables processing unless `-- --feel` is present; the runner supplies its
schedule via `--feel-config`. Probes preallocate Float64Array / PackedFloat64Array
buffers (capacity 1,000 frames/second plus headroom) and write numeric slots. They
do not clone state, allocate sample buffers, log, perform file I/O, or serialize on
the measured per-frame path. Exact caches its rAF wrappers by callback identity;
a newly seen callback allocates one wrapper, then reuses it. InputEventKey objects are also prepared before the
Godot run. Results are read once after sampling ends. Instrumentation has a small,
unsubtracted observer cost; the games' own allocation and UI work remain included.

- **Frame pacing:** every probe records the **raw callback clock** (rAF's argument
  in both browsers; `Time.get_ticks_usec()` on entry to Godot `_process`) and the
  **drawn clock** (Exact's paced `Frame::now_ms`, three.js's rAF argument, Godot's
  process time). `frame_ms`, refresh interval/Hz, p50/p95/p99/max and raw hitches
  use only raw callbacks. A separate drawn-clock interval set uses only drawn
  clocks, including zero intervals when two distinct callbacks draw the same slot.
  Same-raw-time redraws are dropped, keeping the first; backwards raw time refuses
  the trace. The whole script, including idle gaps, is included. Quantiles linearly
  interpolate sorted values. Each clock's hitch is strictly greater than 1.5× its
  own median; percentages divide by recorded frames (the first has no interval).
  A steady half-rate renderer can have zero hitches: compare raw cadence to the
  recorded display mode. These are offered draws, not presentation fences.
- **Screen-space displacement:** in the W delivery + 1 second through release
  window, project the drawn player's world position through that draw's camera
  view-projection into **physical canvas pixels**, with both displacement endpoints
  inside the window. `screen_player.judder` is population stddev(Δ)/mean(Δ), with
  Δ the Euclidean pixel displacement. Report exact Δ = 0 repeats separately, and
  the fraction of adjacent displacement pairs with `abs(Δ[i] - Δ[i-1]) > 0.5 px`;
  the first displacement has no predecessor. `DISPLACEMENT_CHANGE_PX = 0.5` is the
  half-pixel reporting choice specified by **Brief F3, finding 2**: a perceptual
  diagnostic threshold, not a validated universal human detection threshold.
  Equality at the boundary does not count. A wholly stationary screen position
  has undefined CV (`—`), all repeats and zero changes. Missing camera projection
  (or a point behind the camera) yields `—`, never an inferred camera or zero.
  Camera follow/rotation, authored motion and hitches all contribute; this measures
  the player's screen center, not every visible feature or photographed judder.
- **World-space continuity:** the old player/camera translation CV and repeats
  remain, explicitly marked **world (metres)**. Their formula and window are
  unchanged. three.js samples world matrices in the actual mesh `onBeforeRender`;
  Godot samples the player and camera transforms in `_process`, using interpolated
  transforms when enabled. All probes retain that camera's view-projection and
  physical canvas dimensions per draw. The mapping is the usual homogeneous divide
  and NDC-to-canvas conversion ([three.js camera matrices](https://threejs.org/docs/pages/Camera.html),
  [Godot camera projection](https://docs.godotengine.org/en/4.7/classes/class_camera3d.html)).
- **Input latency:** 20 alternating W/S presses, each held 100 ms and separated by
  3.5 seconds after release. No teleport, save restore, velocity reset or paused
  simulation is used. Before each event the last 100 ms must contain at least three
  frames with exactly unchanged player position; otherwise the trial is invalid,
  not a misleading zero-latency success. The first subsequent frame with any changed
  player coordinate ends the trial. Start is `performance.now()` inside a capturing
  `keydown` listener or `Time.get_ticks_usec()` inside the probe's `_input`.
  End is `performance.now()` at the mesh draw callback or the Godot `_process` sample.
  We retain rAF's timestamp separately because it can precede an event delivered
  during that frame; subtracting it could yield negative latency. Reported median
  and p95 are milliseconds and multiples of the run's **raw** median refresh interval.
  Failed/moving-baseline trials are explicit; fewer than 20 valid trials makes the
  row invalid. The command exits nonzero if any attempt is invalid.

**This measures the engine's pipeline from event delivery to presented state, not
the OS/USB path, GPU completion, compositor queue or scanout.** Chrome events use
CDP `Input.dispatchKeyEvent`; Godot uses `Input.parse_input_event` plus the proof's
`Input.flush_buffered_events`. Each table row names its listener/`_input` → first
**drawn pose change** endpoints and its injection phase in a column note.
No claim of equal physical input-to-photon latency follows from these numbers.

Godot shipped uses its 60 Hz physics and Compatibility renderer. Its additional row
sets `SceneTree.physics_interpolation = true` before the game's nodes are built —
the runtime equivalent of the sole project switch
`physics/common/physics_interpolation=true`. The transform history is warmed before
measurement. See Godot's [setting documentation](https://docs.godotengine.org/en/stable/classes/class_scenetree.html#class-scenetree-property-physics-interpolation)
and [displayed-transform accessor](https://docs.godotengine.org/en/stable/classes/class_node3d.html#class-node3d-method-get-global-transform-interpolated).
three.js ships a 120 Hz fixed-step accumulator without interpolation; it has **no
one-line interpolation switch**. Implementing interpolation would change the game,
so only its shipped row is included.

Every row records engine/browser version, display refresh and resolution, window
content pixels, observed cadence, focus/visibility, and load. These macOS runs use
**2200×1520 physical content pixels**: 1100×760 CSS pixels at DPR 2 in Chrome and
`--resolution 2200x1520` in Godot (whose authored 1100×760 UI stretches normally).
Rendering options and simulation rates otherwise stay as shipped. `load1` is the **maximum
one-minute load average sampled once per second** through that run; start/end values
are also saved. Any sample above 8 marks the row **provisional**. AppKit confirms the
recorded PID is frontmost before measurement; the probe checks focus/visibility
every sampled frame, and Chrome gets an additional end confirmation. Loss of either
makes the row invalid, even if metrics are still printable. Invalid attempts remain
in the JSONL and raw traces but are excluded from best/median summaries. This is focus/visibility
confirmation, not a pixel occlusion analysis. “Best” selects the actual run with the
lowest **raw** hitch percentage, then raw p99 interval (unavailable when raw time was not saved). “Median metrics” takes each column's
median across three runs; it is not a synthetic fourth trace or a pooled latency
distribution. Neither summary removes the provisional flags.

`bun test ./game/bench/feel.test.mjs` checks known smooth and alternating-frame motion,
known latency with distinct frame/sample clocks, residual-motion rejection, schedule
durations, quantiles and corrupt buffers. F3 verifies saved traces and unit fixtures; Godot also passes a headless script
parser check. The orchestrator owns the next live sitting.

### Measurements — 2026-09-17, provisional

Apple M5 Max, macOS 26.6.2, 120 Hz display mode (3456×2234), **2200×1520 content
pixels in every run**. three.js r186 / Chrome 153.0.8010.52; Godot
4.7.2.stable.official.ed1daf0bf, Compatibility renderer. All rows below represent
valid foreground/visible captures with 20/20 latency trials. **Every attempt is
provisional**: valid-run peak load1 ranged from 22.0 to 40.9.

[The JSONL](results/feel-2026-09-17.jsonl) contains 14 complete attempts and links
their raw traces: nine valid runs and five rejected focus-loss attempts (two of
those also lacked sufficient stationary baseline samples for one latency trial).
No failed attempt is silently discarded. Accepted slots are three.js #1/#2/#3,
Godot shipped #1.3/#2.3/#3, and Godot interpolation #1.2/#2/#3; a suffix denotes
the retake attempt. Each best row is selected by pacing, so it need not have the
lowest judder or latency. Median rows take the median of each metric across the
three accepted runs. The median interval column is also frame p50.

| engine / summary | load1 | median interval ms (Hz seen) | p95 ms | p99 ms | max ms | hitches / % |
|---|---:|---:|---:|---:|---:|---:|
| three.js shipped — best #3 | 31.9 | 8.30 (120.5) | 9.98 | 10.30 | 11.90 | 0 / 0.00% |
| three.js shipped — median | 24.0 | 8.30 (120.5) | 9.98 | 10.30 | 13.00 | 1 / 0.01% |
| Godot shipped — best #2.3 | 22.6 | 8.28 (120.8) | 11.45 | 12.48 | 38.49 | 98 / 1.02% |
| Godot shipped — median | 26.0 | 8.28 (120.8) | 11.07 | 12.83 | 49.90 | 108 / 1.13% |
| Godot interpolation — best #2 | 35.8 | 8.20 (122.0) | 11.53 | 11.99 | 32.04 | 41 / 0.42% |
| Godot interpolation — median | 35.8 | 8.25 (121.2) | 11.14 | 11.99 | 33.85 | 51 / 0.53% |

| engine / summary | player judder | player Δ=0 | camera judder | camera Δ=0 | input median / p95 ms | input median / p95 intervals |
|---|---:|---:|---:|---:|---:|---:|
| three.js shipped — best #3 | 0.432 | 9.5% | 0.431 | 9.5% | 6.85 / 15.95 | 0.83 / 1.92 |
| three.js shipped — median | 0.432 | 9.5% | 0.431 | 9.5% | 5.75 / 13.86 | 0.69 / 1.67 |
| Godot shipped — best #2.3 | 1.000 | 49.4% | 0.999 | 49.4% | 8.14 / 17.73 | 0.98 / 2.14 |
| Godot shipped — median | 1.000 | 49.4% | 0.999 | 49.4% | 10.40 / 17.36 | 1.26 / 2.10 |
| Godot interpolation — best #2 | 6.85e-6 | 0.0% | 0.004 | 0.0% | 10.87 / 16.82 | 1.33 / 2.05 |
| Godot interpolation — median | 6.85e-6 | 0.0% | 0.004 | 0.0% | 10.87 / 17.22 | 1.33 / 2.09 |

**What a player would notice:** on this 120 Hz display, shipped Godot draws roughly
half its motion frames at an unchanged player and camera position: the resulting
60 Hz stepping should be visible while walking or panning. Its one interpolation
switch removes those repeated motion frames and makes the follow camera much
smoother. three.js moves more often, but its unsmoothed 120 Hz accumulator still
repeats frames when tick and render phases differ: player judder ranged 0.129–0.660
across the three runs, despite steady frame cadence. Interpolated Godot's player
judder ranged 0.0000067–0.062; interpolation cannot eliminate a loaded machine's
hitches. The event-to-drawn-state medians here are about 6 ms for three.js and
10–11 ms for Godot, but the injection-phase difference and missing compositor/scanout
time prevent a physical responsiveness ranking. Re-take on a quiet machine before
attributing the pacing differences or rare stalls to the engines.

The awkward parts were observing the drawn transform without changing either game,
using Enter's CDP text payload to activate the real Play button, distinguishing rAF
frame time from event time, matching Retina framebuffer sizes, and refusing brief
focus losses as valid measurements. Bun also retained driver handles after Chrome
cleanup; the CLI now exits explicitly only after child exit and process audit.
All 14 traces were re-analyzed successfully, all nine accepted traces include the
W/D/jump/S path, and the numerical tests and repository caps check pass.

### Exact's observer

`exact` serves the selected built host and explicitly installs
[`probes/exact.mjs`](probes/exact.mjs) only into this benchmark page. No per-game
probe file is required; concurrent/repeated installation shares one promise. It first opens
Play to warm the actual GPU module/pipelines, fresh-boots the authored title with
`exact.reload()`, then focuses Play for the unchanged Enter/W/D/jump/S script.
There is no `?agent` parameter, host clock override, alternate simulation or renderer.
The existing `exact.gpu.agent(view, request)` is available in live mode; no new host
handle was needed. `exact.agent` and `exact.now` are absent there. Input delivery's
capturing listener reads the same page's `performance.now()` as the wasm trace's
`window.performance.now()`. The host forwards `event.timeStamp` and rAF timestamps
without subtracting its UI clock origin. Raw `event_stamps_ms` retain the forwarded
stamps separately from listener delivery times.

The surface accepts `state` with `trace: {entity: "player", frames: 4096}` to arm a
bounded, preallocated ring, `trace: "read"` to consume it and disarm, or
`trace: "stop"` to discard it. Trace storage is absent by default. It retains just
one entity's previous/current global translations, following the same last-two-tick
feeds, generation resets and teleport/parent snaps as the GPU. If armed during
motion it waits for an observed tick to establish a valid history pair;
`initial_frames_skipped` makes this explicit. The active camera uses the feed's
existing two poses. Both translations are mixed in f64 using the submitted alpha;
they are not rounded agent-state coordinates or GPU readback. This observes submitted
state, with the same presentation-fence limitation as the twins.

The trace's flat 14-number rows are `[frame_ms, sample_ms, player_xyz, camera_xyz,
alpha, ticks_this_frame, completed_tick, tick_ms, feed_ms, encode_ms]` (xyz expands
to three numbers). `frame_ms` is `Frame::now_ms`; browser `sample_ms` is
`performance.now()` just after draw encoding, while native uses elapsed `Instant`
since arming. The last three numbers sum existing wall timings for that frame;
feed time includes page upload encoding, not GPU completion. Ring overwrite sets
`overflow`, and disappearance of the entity sets `missing`; either invalidates the
probe. The adapter copies the first eight numbers only at the final bulk read,
retaining the full trace and perf summary alongside them. Scheduling, judder,
latency, quantiles and the provisional threshold use the twins' unchanged functions.

Exact and three.js both use [`probes/input.mjs`](probes/input.mjs): one recorder,
one scheduled item per key edge, preserving delivery time and `event.timeStamp`.
The schedule uses the same key definition and passes its accepted codes to Godot.
A press and its release are already two schedule items: 10 main-script edges plus
40 latency-trial edges equals **50 delivered events**, not 100. Extra accepted
edges invalidate the attempt; the analyzer never deduplicates them.

**What was wrong in the first real Exact run:** the saved
`feel-2026-09-18T07-35-27-317Z-exact-shipped-1-attempt1.json.gz` contains 100 events
for 50 scheduled edges, with no adjacent equal event timestamps. Enter has exactly
one down/up pair. There are unscheduled A edges and extra W/D/S edges at distinct
times (the first W arrives about 348 ms after Enter, before its scheduled 500 ms).
This is extra delivered input contaminating the capture, not a down/up counting
mistake or a listener recording each event twice. The trace cannot identify who
sent the extra input. The shared recorder, explicit idempotent installation and
idle-console preflight make the method consistent and reject contamination;
they do not erase evidence to force the expected count.

`FEEL_EXACT_DIST` can select another prebuilt directory; `FEEL_EXACT_HZ` supplies
the default rate label, overridden by `--hz`. A 120 Hz run reuses the provisioned
experiment without editing game source or changing interpolation. Preparation of
a new experiment is outside this command. A mismatched actual rate is invalid.

Implementation validation includes a 144 Hz presentation/60 Hz tick fixture, tick
bursts, ring wrapping/full precision, read-once state semantics, unchanged simulation
time, and an allocation counter covering the armed trace. Adapter tests preserve
both timestamps and produce byte-for-byte identical input to the common analyzer.

### F1d single attempt — 2026-09-18, provisional

Command: `bun game/bench/feel.mjs exact --hz 60 --no-build --attempts 1`.
Preflight passed unlocked, with 586.4 s console idle at capture start and 673.9 s
at completion. Actual simulation rate was 60 Hz; the display was 120 Hz, content
2200×1520 pixels, Chrome 153.0.8010.52. The shared recorder delivered **50/50
scheduled edges**, and the common analyzer accepted **20/20 latency trials**.
No focus/visibility loss occurred. Chrome PID 83949 was killed, its exit awaited,
and the process audit found no remaining process for that PID/profile.

| variant | status | peak load1 | frame p50 / p95 / p99 / max ms | hitches | player / camera judder | repeated positions | input p50 / p95 ms |
|---|---|---:|---|---:|---|---|---|
| exact/60hz | valid, PROVISIONAL | 25.7 | 8.30 / 10.00 / 10.30 / 10.80 | 0 | 0.10560 / 0.10559 | 0% / 0% | 6.90 / 10.03 |

[JSONL row](results/feel-2026-09-18.jsonl),
[table](results/feel-2026-09-18T08-02-42-873Z.md),
[raw trace](results/feel-2026-09-18T08-02-42-873Z-exact-60hz-1-attempt1.json.gz).

The count is now analyzable, but **judder is not approximately zero**: player and
camera are both about 0.106 despite no repeated positions or threshold-defined
hitches. Read against the raw trace (the exact trace keeps both the frame's
`requestAnimationFrame` timestamp and the wall clock), the number is exactly the
frame clock's own jitter: the rAF intervals in the judder window have a CV of 0.1056
(7.8–8.9 ms on an 8.33 ms display), the drawn displacements a CV of 0.1056, and
displacement divided by the rAF interval a CV of **0.0000**. The engine draws
precisely where the host's clock says the frame is; Chrome's callback clock wanders
around the vsync grid under this load while the frames land on it. Godot with
interpolation scores 6.9e-6 on the same display because its process delta is
smoothed to whole refresh intervals; the Apple host here already renders at
`CADisplayLink.targetTimestamp`, a presentation-aligned clock. So the finding is a
web-host frame-pacing gap, not an interpolation gap: `host/web/gpu-glue.js` must
pace the frame clock it hands the module (snap each delta to whole refresh periods
within a tolerance, keep the paced clock within half a period of the callback's).
The schedule also records a maximum late edge of 58.6 ms, so zero render hitches does
not mean the loaded runner delivered every edge precisely on time. These are
one provisional attempt's event-to-drawn-state numbers, with no new twin runs
in the same sitting; they cannot establish a latency ranking. No retake was made.

### After frame pacing — one provisional attempt, same build, load 74

`host/web/pace.js` (commit 05cbe640) hands the module a clock snapped to the
display's lattice. Same bake, same command, the working-tree glue copied into the
dist by hand (a first copy carried the assets builder's half-done `gpu_assets`
call and was invalid: build what you measure).

| variant | status | peak load1 | frame p50 / p95 / p99 / max ms | hitches | player / camera judder | repeated positions | input p50 / p95 ms |
|---|---|---:|---|---:|---|---|---|
| exact/60hz, paced | valid, PROVISIONAL | 74.0 | 8.34 / 8.36 / 8.36 / 10.79 | 0 | 0.042 / 0.042 | 0% / 0% | 18.00 / 19.50 |

The frame columns now describe the paced clock (uniform by construction). The
displacement was identical on 176 of the 180 frames in the judder window; the four
that differ are real late callbacks (10.8 ms) at load 74, which the pacer of that
run passed through — the committed pacer snaps them, and replaying the recorded
callbacks through it gives a delta CV of 0.002. Judder 0.106 → 0.042 in this run,
→ ~0.002 with the committed rule, against Godot-with-interpolation's 6.9e-6 and
three.js's 0.43.

The latency column is the other lesson: 6.9 ms in the unpaced run, 18.0 ms here,
on the same code. The raw traces show a **tick-phase lottery**: the 60 Hz tick
deadlines fell just before the frames that ran them in the first run (alpha 0.02)
and 7.5 ms before them in this one (alpha 0.45), so every input waited most of a
frame period for a tick that was already due. Godot has the same lottery; three.js
ticks every frame. A tick that is due before the next frame should run now (brief
F2b); until then a single latency row from any 60 Hz fixed-step engine is one draw
from that lottery and three attempts are the minimum.

### First full sitting — 2026-09-18, 04:56–05:36, display unlocked, console idle throughout

`bun game/bench/feel.mjs compare --attempts 3` (exact 60/120 Hz, three.js, Godot ×2)
at commit 98dd3b3a plus the fixes below, then `godot --attempts 3` again after its
probe was repaired. Every row is **provisional**: the machine ran two builders and
four reviewers at load 12–29 throughout. The exact rows were rejected by the runner
as written ("Nonmonotonic frame timestamps": a redraw at an unchanged frame time —
the resize path draws at the last paced time — put two rows at one timestamp) and are
scored from their saved raw traces with the repaired analyzer
(`bun game/bench/feel.mjs reanalyze <trace.json.gz>…`); the Godot rows first failed
on a GDScript type error the probe refactor introduced (`code := … else keycode`) and
on JSON floats never matching integer key codes, both fixed here. The exact runs'
first attempts are contaminated by the orchestrator (a proof run stole the window's
focus during the 120 Hz attempt: `front/visible NO`, the player stood still for 138
frames; the 60 Hz attempt overlapped a build) and are shown but not used.

| engine / variant | player judder (3 runs) | repeated positions | event → submitted pose, p50 (3 runs) | p95 | hitches |
|---|---:|---:|---:|---:|---:|
| **exact / 60 Hz world** | 0.074†, **0.002**, **0.002** | 0 % | 4.25†, **5.15**, **6.50** ms | 6.3–13.7 | 0 |
| **exact / 120 Hz world** | 2.06†, **0.002**, **0.002** | 0 % | 4.95†, **4.10**, **3.50** ms | 5.3–8.2 | 0–2 |
| three.js r186 (120 Hz accumulator, no interpolation) | 0.725, 0.579, 0.484 | 12–26 % | 1.45, 4.65, 4.50 ms | 6.9–12.5 | 0 |
| Godot 4.7 as shipped | 0.994, 1.000, 0.994 | 49.7 % | 16.10, 16.31, 14.58 ms | 17.1 | 12.3 % |
| Godot 4.7 with physics interpolation | 0.032, 0.068, 0.370 | 0 % | 16.49, 10.99, 16.07 ms | 17.3 | 12.4 % |

† contaminated attempt, see above.

Original reading (superseded by the F3 re-score below): judder here is the displacement between consecutive
*submitted* poses on the paced clock, not a photographed frame — Exact's frame column
is its own paced clock while three.js's is raw rAF, so the pacing column favours Exact
by construction and only the displacement columns compare like with like; latency is
event-to-submitted-pose (a CPU `f64` pose, the first floating-point change, no scanout),
so call it that; Godot injects its keys after its frame sample while CDP injects between
callbacks — a different phase distribution; the twins move by different rules (three.js
a 120 Hz exponential-velocity step, Godot its own controller, Exact the 60 Hz
`Character`), so these are authored experiences, not one workload; and a 12 % hitch
rate for Godot on a machine at load 15 says as much about the machine as about Godot.

Original conclusion, limited to world-space motion: on this display Exact draws the walking player with a
displacement that varies by 0.2 % frame to frame — three.js's varies by 50–70 % and
repeats a frame every fifth or sixth, shipped Godot repeats every other frame, Godot
with interpolation on lands at 3–37 % — and its event-to-pose latency at 60 Hz ticks
(5–6.5 ms) sits with three.js's per-frame stepping (4.5 ms) and three times under
Godot's (16 ms); at 120 Hz ticks it is 3.5–4.1 ms. The first-attempt numbers under
disturbance (0.074 judder, a stalled player) are a reminder that a sitting is a sitting:
nothing else may touch the display while it runs.

#### F3 — re-scored from the same traces

The historical table above stays intact. The following is the literal output of
this command, run from the repository root; every run links its input trace:

```sh
bun game/bench/feel.mjs reanalyze game/bench/results/feel-2026-09-18T11-56-12-978Z-*.json.gz game/bench/results/feel-2026-09-18T12-27-35-105Z-*.json.gz
```

All valid traces of this sitting are included, together with the unfocused Exact
attempt (now correctly excluded from summaries). The intermediate
`game/bench/results/feel-2026-09-18T12-16-13-541Z-*.json.gz` Godot attempts have
no delivered edges and cannot be scored. Load/provisional metadata comes from
`game/bench/results/feel-2026-09-18.jsonl`; no fresh measurement was taken.
The build-overlapping Exact 60 Hz first attempt remains flagged by the historical
footnote: it passes the unchanged automatic input/focus rules, so the mechanical
median below includes it. Ranking claims still use the undisturbed attempts.

**Archive limitation:** despite their old `timestamp_source` label saying rAF,
Exact's `frames` slot 0 and `exact_trace` slot 0 are the **same paced clock**;
slot 1 is the draw-boundary `performance.now()` sample. Neither is the raw rAF
argument. Thus Exact's raw cadence, raw hitches and latency/raw-interval cells
are `—`. Substituting wall time would repeat the incomparable-clock mistake.
The twins' two clock interval sets are identical by definition. No saved trace
contains the drawn camera projection, so every screen-pixel cell is `—`.
**The next sitting supplies these fields once Exact’s ring extension lands.**
The ring now lives in `game/render/src/trace.rs`, outside the brief’s named scope;
that small row extension and its `surface.rs` call-site hunk are prepared in
`/tmp/f3-trace-row.patch`, pending scope approval. The browser adapter already
accepts the extended row; Exact camera capture is not yet complete.
No camera or callback timestamps
are reconstructed from the authored game or the pacer.

| variant | run / trace | status | load1 | raw Hz | raw callback intervals p50/p95/p99/max ms | raw hitches | drawn-clock intervals p50/p95/p99/max ms | drawn hitches | screen player CV (px) | Δ change >0.5 px % | screen zero % | world player CV (m) | world player zero % | world camera CV (m) | world camera zero % | event delivery → first drawn pose p50/p95 ms | latency / raw interval p50/p95 | endpoints; injection phase | tick_phase | trials; edges | front/visible |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| exact/120hz | [reanalyzed #1 (2026-09-18T11-56-12-978Z)](results/feel-2026-09-18T11-56-12-978Z-exact-120hz-1-attempt1.json.gz) | INVALID PROVISIONAL | 25.4 | — | — | — | 8.34/8.35/8.36/12.27 | 0 (0.00%) | — | — | — | 2.058 | 77.1 | 1.432 | 0.0 | 4.95/6.71 | —/— | listener → draw pose; CDP between callbacks | 0.9045 | 20/20; 50/50 | NO |
| exact/120hz | [reanalyzed #2 (2026-09-18T11-56-12-978Z)](results/feel-2026-09-18T11-56-12-978Z-exact-120hz-2-attempt1.json.gz) | PROVISIONAL | 29.0 | — | — | — | 8.34/8.36/8.36/12.82 | 2 (0.02%) | — | — | — | 0.002 | 0.0 | 0.002 | 0.0 | 4.10/8.15 | —/— | listener → draw pose; CDP between callbacks | 0.7952 | 20/20; 50/50 | yes |
| exact/120hz | [reanalyzed #3 (2026-09-18T11-56-12-978Z)](results/feel-2026-09-18T11-56-12-978Z-exact-120hz-3-attempt1.json.gz) | PROVISIONAL | 14.6 | — | — | — | 8.34/8.36/8.36/8.37 | 0 (0.00%) | — | — | — | 0.002 | 0.0 | 0.002 | 0.0 | 3.50/5.25 | —/— | listener → draw pose; CDP between callbacks | 0.7242 | 20/20; 50/50 | yes |
| exact/120hz | median metrics (n=2) | PROVISIONAL | 21.8 | — | — | — | 8.34/8.36/8.36/10.59 | 1 (0.01%) | — | — | — | 0.002 | 0.0 | 0.002 | 0.0 | 3.80/6.70 | —/— | listener → draw pose; CDP between callbacks | 0.7597 | 20/20; 50/50 | yes |
| exact/60hz | [reanalyzed #1 (2026-09-18T11-56-12-978Z)](results/feel-2026-09-18T11-56-12-978Z-exact-60hz-1-attempt1.json.gz) | PROVISIONAL | 24.8 | — | — | — | 8.34/8.35/8.36/10.00 | 0 (0.00%) | — | — | — | 0.074 | 0.0 | 0.074 | 0.0 | 4.25/6.34 | —/— | listener → draw pose; CDP between callbacks | 0.4550 | 20/20; 50/50 | yes |
| exact/60hz | [reanalyzed #2 (2026-09-18T11-56-12-978Z)](results/feel-2026-09-18T11-56-12-978Z-exact-60hz-2-attempt1.json.gz) | PROVISIONAL | 19.7 | — | — | — | 8.34/8.36/8.36/8.37 | 0 (0.00%) | — | — | — | 0.002 | 0.0 | 0.002 | 0.0 | 5.15/13.66 | —/— | listener → draw pose; CDP between callbacks | 0.3833 | 20/20; 50/50 | yes |
| exact/60hz | [reanalyzed #3 (2026-09-18T11-56-12-978Z)](results/feel-2026-09-18T11-56-12-978Z-exact-60hz-3-attempt1.json.gz) | PROVISIONAL | 12.6 | — | — | — | 8.34/8.35/8.36/8.37 | 0 (0.00%) | — | — | — | 0.002 | 0.0 | 0.002 | 0.0 | 6.50/7.70 | —/— | listener → draw pose; CDP between callbacks | 0.4621 | 20/20; 50/50 | yes |
| exact/60hz | median metrics (n=3) | PROVISIONAL | 19.7 | — | — | — | 8.34/8.35/8.36/8.37 | 0 (0.00%) | — | — | — | 0.002 | 0.0 | 0.002 | 0.0 | 5.15/7.70 | —/— | listener → draw pose; CDP between callbacks | 0.4550 | 20/20; 50/50 | yes |
| three/shipped | [reanalyzed #1 (2026-09-18T11-56-12-978Z)](results/feel-2026-09-18T11-56-12-978Z-three-shipped-1-attempt1.json.gz) | PROVISIONAL | 22.2 | 120.5 | 8.30/10.10/10.30/10.40 | 0 (0.00%) | 8.30/10.10/10.30/10.40 | 0 (0.00%) | — | — | — | 0.725 | 26.3 | 0.725 | 26.3 | 1.45/8.83 | 0.17/1.06 | listener → draw pose; CDP between callbacks | — | 20/20; 50/50 | yes |
| three/shipped | [reanalyzed #2 (2026-09-18T11-56-12-978Z)](results/feel-2026-09-18T11-56-12-978Z-three-shipped-2-attempt1.json.gz) | PROVISIONAL | 27.4 | 120.5 | 8.30/10.00/10.30/10.50 | 0 (0.00%) | 8.30/10.00/10.30/10.50 | 0 (0.00%) | — | — | — | 0.579 | 16.8 | 0.579 | 16.8 | 4.65/12.52 | 0.56/1.51 | listener → draw pose; CDP between callbacks | — | 20/20; 50/50 | yes |
| three/shipped | [reanalyzed #3 (2026-09-18T11-56-12-978Z)](results/feel-2026-09-18T11-56-12-978Z-three-shipped-3-attempt1.json.gz) | PROVISIONAL | 16.6 | 120.5 | 8.30/10.00/10.30/10.40 | 0 (0.00%) | 8.30/10.00/10.30/10.40 | 0 (0.00%) | — | — | — | 0.484 | 11.7 | 0.484 | 11.7 | 4.50/6.91 | 0.54/0.83 | listener → draw pose; CDP between callbacks | — | 20/20; 50/50 | yes |
| three/shipped | best (#2) | PROVISIONAL | 27.4 | 120.5 | 8.30/10.00/10.30/10.50 | 0 (0.00%) | 8.30/10.00/10.30/10.50 | 0 (0.00%) | — | — | — | 0.579 | 16.8 | 0.579 | 16.8 | 4.65/12.52 | 0.56/1.51 | listener → draw pose; CDP between callbacks | — | 20/20; 50/50 | yes |
| three/shipped | median metrics (n=3) | PROVISIONAL | 22.2 | 120.5 | 8.30/10.00/10.30/10.40 | 0 (0.00%) | 8.30/10.00/10.30/10.40 | 0 (0.00%) | — | — | — | 0.579 | 16.8 | 0.579 | 16.8 | 4.50/8.83 | 0.54/1.06 | listener → draw pose; CDP between callbacks | — | 20/20; 50/50 | yes |
| godot/interpolation | [reanalyzed #1 (2026-09-18T12-27-35-105Z)](results/feel-2026-09-18T12-27-35-105Z-godot-interpolation-1-attempt1.json.gz) | PROVISIONAL | 14.4 | 120.5 | 8.30/13.14/13.69/20.27 | 1200 (12.39%) | 8.30/13.14/13.69/20.27 | 1200 (12.39%) | — | — | — | 0.032 | 0.0 | 0.032 | 0.0 | 16.49/17.29 | 1.99/2.08 | _input → _process pose; after sample | — | 20/20; 50/50 | yes |
| godot/interpolation | [reanalyzed #2 (2026-09-18T12-27-35-105Z)](results/feel-2026-09-18T12-27-35-105Z-godot-interpolation-2-attempt1.json.gz) | PROVISIONAL | 14.2 | 119.7 | 8.36/13.25/14.52/31.61 | 1282 (13.31%) | 8.36/13.25/14.52/31.61 | 1282 (13.31%) | — | — | — | 0.068 | 0.0 | 0.069 | 0.0 | 10.99/17.02 | 1.32/2.04 | _input → _process pose; after sample | — | 20/20; 50/50 | yes |
| godot/interpolation | [reanalyzed #3 (2026-09-18T12-27-35-105Z)](results/feel-2026-09-18T12-27-35-105Z-godot-interpolation-3-attempt1.json.gz) | PROVISIONAL | 20.9 | 112.7 | 8.87/13.18/14.63/46.50 | 332 (3.47%) | 8.87/13.18/14.63/46.50 | 332 (3.47%) | — | — | — | 0.370 | 0.0 | 0.368 | 0.0 | 16.07/17.88 | 1.81/2.01 | _input → _process pose; after sample | — | 20/20; 50/50 | yes |
| godot/interpolation | best (#3) | PROVISIONAL | 20.9 | 112.7 | 8.87/13.18/14.63/46.50 | 332 (3.47%) | 8.87/13.18/14.63/46.50 | 332 (3.47%) | — | — | — | 0.370 | 0.0 | 0.368 | 0.0 | 16.07/17.88 | 1.81/2.01 | _input → _process pose; after sample | — | 20/20; 50/50 | yes |
| godot/interpolation | median metrics (n=3) | PROVISIONAL | 14.4 | 119.7 | 8.36/13.18/14.52/31.61 | 1200 (12.39%) | 8.36/13.18/14.52/31.61 | 1200 (12.39%) | — | — | — | 0.068 | 0.0 | 0.069 | 0.0 | 16.07/17.29 | 1.81/2.04 | _input → _process pose; after sample | — | 20/20; 50/50 | yes |
| godot/shipped | [reanalyzed #1 (2026-09-18T12-27-35-105Z)](results/feel-2026-09-18T12-27-35-105Z-godot-shipped-1-attempt1.json.gz) | PROVISIONAL | 15.5 | 120.8 | 8.28/13.17/13.69/21.45 | 1191 (12.29%) | 8.28/13.17/13.69/21.45 | 1191 (12.29%) | — | — | — | 0.994 | 49.7 | 0.994 | 49.7 | 16.10/17.06 | 1.95/2.06 | _input → _process pose; after sample | — | 20/20; 50/50 | yes |
| godot/shipped | [reanalyzed #2 (2026-09-18T12-27-35-105Z)](results/feel-2026-09-18T12-27-35-105Z-godot-shipped-2-attempt1.json.gz) | PROVISIONAL | 13.9 | 120.6 | 8.29/13.21/13.70/23.37 | 1365 (14.10%) | 8.29/13.21/13.70/23.37 | 1365 (14.10%) | — | — | — | 1.000 | 50.0 | 1.000 | 50.0 | 16.31/17.00 | 1.97/2.05 | _input → _process pose; after sample | — | 20/20; 50/50 | yes |
| godot/shipped | [reanalyzed #3 (2026-09-18T12-27-35-105Z)](results/feel-2026-09-18T12-27-35-105Z-godot-shipped-3-attempt1.json.gz) | PROVISIONAL | 16.5 | 116.8 | 8.56/13.20/14.84/33.72 | 877 (9.15%) | 8.56/13.20/14.84/33.72 | 877 (9.15%) | — | — | — | 0.994 | 49.7 | 0.994 | 49.7 | 14.58/19.24 | 1.70/2.25 | _input → _process pose; after sample | — | 20/20; 50/50 | yes |
| godot/shipped | best (#3) | PROVISIONAL | 16.5 | 116.8 | 8.56/13.20/14.84/33.72 | 877 (9.15%) | 8.56/13.20/14.84/33.72 | 877 (9.15%) | — | — | — | 0.994 | 49.7 | 0.994 | 49.7 | 14.58/19.24 | 1.70/2.25 | _input → _process pose; after sample | — | 20/20; 50/50 | yes |
| godot/shipped | median metrics (n=3) | PROVISIONAL | 15.5 | 120.6 | 8.29/13.20/13.70/23.37 | 1191 (12.29%) | 8.29/13.20/13.70/23.37 | 1191 (12.29%) | — | — | — | 0.994 | 49.7 | 0.994 | 49.7 | 16.10/17.06 | 1.95/2.06 | _input → _process pose; after sample | — | 20/20; 50/50 | yes |


The world-space ordering survives: undisturbed Exact has the lowest displacement
CV, followed by interpolated Godot, three.js, then shipped Godot. Event-to-drawn-pose
latency in milliseconds is unchanged, with the per-engine input phase now beside
it. The claim that Exact wins raw frame pacing does **not** survive: its raw clock
was not recorded. No screen-space ranking can be made from this archive, so the
old world-space result cannot establish a perceptual Feel winner. All rows remain
provisional; the games' different motion rules and the sitting's load still limit
cross-engine conclusions.

**Full measurement pending:** the orchestrator must run `compare` in a quiet
sitting for three attempts of all five variants. The 120 Hz bake was selected and
fingerprinted in tests, but not launched here. The normal stale-build path was
not executed in F1d; existing bakes were trusted with `--no-build`, and no Cargo
command ran. Earlier twin rows above remain historical, not evidence about Exact.

### Trace protocol

Use the same trace protocol, without changing scheduling or metric logic. Register
a web adapter in `feel.mjs` with a local `root`/`page` or an already-served `url`, plus
an optional readiness expression. The page starts with Play focused and exposes
`window.feel.begin(durationMs)`, `arm(trialId)` (labels the next captured key event),
and `end()` (stops capture and returns one plain record). The probe needs the **drawn
player and camera xyz each frame, a frame timestamp, a same-clock draw/sample time,
and key-delivery time**. Do not expose only the last physics tick's positions.

Schema 1 retains `stride: 8` and flat `frames` rows of
`[drawn_clock_ms, sample_ms, player_x, player_y, player_z, camera_x, camera_y, camera_z]`.
New captures also supply parallel `raw_callback_ms` and `drawn_clock_ms` arrays,
one entry per row. `sample_ms` is the latency endpoint, **not** either frame clock.
`camera_projection` has 18 values per row: column-major world-to-clip matrix (16),
then physical canvas width and height. The analyzer's projection is a homogeneous
divide, without camera reconstruction or smoothing. Exact's observer captures rAF
arguments without changing callbacks, then joins them to ring rows using callback
entry wall times and draw-boundary wall times; resize draws inherit the last raw
callback and duplicate stamps are dropped. Arrays are preallocated and serialized
only after sampling. Old Exact traces explicitly lack raw callbacks; old traces
of every engine lack camera projection, and those metrics stay unavailable.
Flat `events` rows are `[delivered_ms, VK_code, down_0_or_1, trial_id_or_minus_1]`;
all times share one monotonic origin. Include `engine_version`, `interpolation`,
`timestamp_source`, `window_pixels`, `overflow`, `hidden_frames` and
`unfocused_frames` (and `viewport_css` or `viewport_pixels`). The same `script()` and
`analyze()` apply without an engine-specific threshold or metric smoothing.



## D3 — measured module size, 2026-09-18

Run `bun game/bench/size.mjs` from the repository root. It builds Beacons with the
normal web recipe and unchanged `wasm-opt -Oz`, uses gzip level 9, and aggregates
`twiggy top -n 20000 -f json` by the first Rust crate token (including Rust's crate
hash annotations). Both tools read the same per-game target. The command prints
shipped bytes, crate/section attribution and engine/render module attribution;
`[label] --no-build` measures existing outputs. Full JSON and twiggy rows are in
`game/games/beacons/target/d3-size/`. No GPU shader, model or host split was widened.

All sizes below are **bytes**, measured after each cumulative cut. B is a fresh
`dfdb5003` build, not the earlier 758,561 / 301,524 artifact quoted in the brief.
The final row includes concurrent skeleton production changes, so its delta from
2c is **not an isolated D3 saving**. Initial whole-workspace-target measurements
were discarded when the per-game link-map path mismatch was found.

| stage | shipped raw | gzip | raw delta |
|---|---:|---:|---:|
| B — fresh baseline | 829,372 | 329,049 | — |
| 1 — float/Debug callers | 848,305 | 345,159 | +18,933 |
| 2 — erased component pages | 800,660 | 337,065 | -47,645 |
| 3 — packed WGSL | 797,108 | 335,927 | -3,552 |
| 4 — asset vectors | 784,576 | 332,879 | -12,532 |
| 5 — streamed formatting | 781,272 | 331,803 | -3,304 |
| 1b — Ryu small tables | 772,094 | 321,462 | -9,178 |
| 2b — storage-kind factories | 739,020 | 315,971 | -33,074 |
| 2c — array-length sharing | 737,768 | 315,604 | -1,252 |
| Current — exact float spelling + concurrent skeleton edits | 764,322 | 325,721 | +26,554 |

**The <550,000-byte target was not met.** The current measured output is
214,322 bytes over it. Ryu is now shared by JSON, perf/trace and
audio diagnostics, with exact rational tie handling to preserve std's shortest
spelling and the existing four-place agent rounding. The tests compare 100,000
f32/f64 samples and fixed audio decimals against the old formatter; no D3 JSON
pin was edited. Publication journal lines deliberately change from Rust Debug
(e.g. `Number(0.0)`) to the Data field walk (`{"Number":[0.0]}`). Journals are outside
the world hash but those diagnostic strings also appear in simulation saves.

The float cut alone initially grew the module: Rust's dragon/grisu formatter
still links through dynamic `clamp` panic messages, including protected animation
code. Ryu's small tables recover 9,178 raw bytes. Registry factories formerly
linked **both component and singleton storage for every registered type**; now
only the declared kinds link, and loading a type in an undeclared kind refuses.
Aligned byte pages share allocation, removal and Data traversals; typed queries
retain constant pointer strides and the same leases, masks and write generations.
Over-aligned owned values, ZSTs, panicking destructors, load budgets, ordering,
page uploads and save/carry coverage pass. Resource ambient/presentation policy
stays in registration and does not change saves or observations.

Asset states, models and dependency lookups use sorted vectors. Entity names
already used a scan. Sounds, publications and world storage maps retain their
ordered Data contracts. JSON publications, storage reads and input replies now
stream into one string; common error/journal formatting avoids repeated generic
container code. Array Data walks share by element type while retaining scalar
sequence tags, separate from bulk-vector encoding.

WGSL comments (including nested block comments) and indentation are removed at
build time for pipeline shaders. Every newline survives, retaining concatenated
source line numbers; columns change. `skin.wgsl` is untouched. The shader cut
removed 3,552 bytes from `.rodata` and the shipped module. Extra convergence,
function merging and GUFA wasm-opt experiments each saved less than 1 KB; the
normal flags remain unchanged.

Pre-bindgen shallow attribution, **not shipped-size attribution**: debug/custom
name sections are omitted; `other` includes bindgen export strings, Wasm structural
entries and symbols without a crate. Array/primitive implementations use the
first named crate in their symbol. Module subrows overlap their parent crate.

| crate / section | B | 1 | 2 | 3 | 4 | 5 | 1b | 2b | 2c | current |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| exact_game | 372,418 | 376,071 | 315,948 | 315,948 | 316,869 | 315,667 | 315,657 | 276,932 | 275,386 | 286,093 |
| other | 353,873 | 354,950 | 354,755 | 354,755 | 354,755 | 354,755 | 354,757 | 354,353 | 354,353 | 354,789 |
| alloc | 176,978 | 176,801 | 174,023 | 174,023 | 157,560 | 155,977 | 156,016 | 155,788 | 155,670 | 172,307 |
| core | 132,789 | 133,361 | 130,921 | 130,921 | 131,357 | 129,291 | 129,291 | 129,025 | 129,025 | 135,907 |
| .rodata | 98,874 | 109,930 | 109,722 | 106,170 | 106,266 | 106,330 | 96,458 | 94,858 | 94,858 | 95,338 |
| exact_game_render | 59,121 | 59,141 | 59,141 | 59,141 | 59,141 | 59,147 | 59,147 | 59,147 | 59,147 | 59,190 |
| wgpu | 45,376 | 45,376 | 45,376 | 45,376 | 45,376 | 45,376 | 45,376 | 45,376 | 45,376 | 45,378 |
| js_sys | 31,708 | 31,708 | 31,708 | 31,708 | 31,708 | 31,708 | 31,708 | 31,708 | 31,708 | 31,708 |
| exact_gpu | 21,477 | 21,477 | 21,477 | 21,477 | 21,477 | 21,477 | 21,477 | 21,477 | 21,477 | 21,477 |
| std | 11,182 | 11,182 | 11,182 | 11,182 | 11,182 | 11,182 | 11,182 | 11,182 | 11,182 | 11,182 |
| wgpu_types | 11,016 | 11,017 | 11,017 | 11,017 | 11,017 | 11,017 | 11,017 | 11,017 | 11,017 | 11,017 |
| compiler_builtins | 10,874 | 10,874 | 10,874 | 10,874 | 10,874 | 10,874 | 10,874 | 10,874 | 10,874 | 10,874 |
| wasm_bindgen | 10,376 | 10,380 | 10,380 | 10,380 | 10,380 | 10,380 | 10,380 | 10,380 | 10,380 | 10,380 |
| glam | 8,686 | 8,551 | 8,551 | 8,551 | 8,551 | 8,551 | 8,551 | 8,551 | 8,551 | 8,551 |
| hashbrown | 7,932 | 7,932 | 7,932 | 7,932 | 7,932 | 7,932 | 7,932 | 7,932 | 7,932 | 7,932 |
| dlmalloc | 7,861 | 7,861 | 7,861 | 7,861 | 7,861 | 7,861 | 7,861 | 7,861 | 7,861 | 7,861 |
| libm | 6,440 | 6,440 | 6,440 | 6,440 | 6,440 | 6,440 | 6,440 | 6,440 | 6,440 | 6,440 |
| beacons_logic | 6,102 | 6,102 | 6,102 | 6,102 | 6,102 | 6,102 | 6,102 | 6,100 | 6,100 | 6,100 |
| web_sys | 3,821 | 3,821 | 3,821 | 3,821 | 3,821 | 3,821 | 3,821 | 3,821 | 3,821 | 3,821 |
| bitflags | 1,485 | 1,485 | 1,485 | 1,485 | 1,485 | 1,485 | 1,485 | 1,485 | 1,485 | 1,485 |
| exact_plan | 1,366 | 931 | 931 | 931 | 931 | 875 | 875 | 875 | 875 | 875 |
| __rustc | 1,354 | 1,354 | 1,354 | 1,354 | 1,354 | 1,354 | 1,354 | 1,354 | 1,354 | 1,354 |
| once_cell | 939 | 939 | 939 | 939 | 939 | 939 | 939 | 939 | 939 | 939 |
| log | 365 | 365 | 365 | 365 | 365 | 365 | 365 | 365 | 365 | 365 |
| .data | 349 | 349 | 349 | 349 | 349 | 349 | 349 | 349 | 349 | 349 |
| beacons_gpu | 144 | 144 | 144 | 144 | 144 | 144 | 144 | 144 | 144 | 144 |
| raw_window_handle | 76 | 76 | 76 | 76 | 76 | 76 | 76 | 76 | 76 | 76 |
| ryu | 0 | 4,746 | 4,746 | 4,746 | 4,746 | 4,746 | 5,479 | 5,479 | 5,479 | 5,479 |
| ↳ exact_game::storage | 126,466 | 126,466 | 68,389 | 68,389 | 68,389 | 68,389 | 68,389 | 33,560 | 33,560 | 34,240 |
| ↳ exact_game::data | 46,440 | 50,643 | 48,715 | 48,715 | 48,715 | 48,249 | 48,249 | 44,871 | 45,568 | 49,614 |
| ↳ exact_game::animation | 31,772 | 31,772 | 31,772 | 31,772 | 31,780 | 31,780 | 31,780 | 31,780 | 31,780 | 36,986 |

Three interleaved native 200k-cube pairs, M5 Max/Metal, 2560×1440, 4× MSAA,
60 warm-up + 240 measured frames per run. AB / BA / AB; load1 8.28–9.18. These
are offscreen CPU phase timings, not refresh-rate claims. Before/after **ranges
overlap in every mode and phase**; no speedup or regression beyond noise. The
after executable includes the storage, registry, vector, shader and array cuts;
subsequent edits affect diagnostic float/journal spelling only.

| moving | median tick before → after ms | median feed before → after ms | median encode before → after ms | feed ranges before / after ms |
|---|---:|---:|---:|---|
| all | 0.4273 → 0.4214 | 0.9675 → 0.9537 | 0.0653 → 0.0664 | 0.9210–1.0042 / 0.9417–0.9651 |
| 1% | 0.0047 → 0.0050 | 0.0493 → 0.0489 | 0.0400 → 0.0389 | 0.0478–0.0494 / 0.0477–0.0501 |
| still | 0.0004 → 0.0005 | 0.0282 → 0.0274 | 0.0387 → 0.0365 | 0.0263–0.0320 / 0.0265–0.0294 |

The workspace rerun passed 446 tests (10 ignored); all-target clippy passed.
Beacons, Greybox and asset-fixture web proofs passed unchanged pins. The first
skinned web run preserved fresh-process continuation but failed the starting
tick-120 hash; the concurrent skeleton slice subsequently changed tick-60 JSON
and the tick-120 test pin from `0xb05ce95a6c799acf` to `0xcae264dd3df5267b`,
alongside controller/root-motion and fixture changes. D3 edited none of those
files or pins. All four final web proofs pass, including byte-identical fresh-process
continuations; the skinned proof uses the revised skeleton pins. Whole-workspace
fmt, boot and caps pass. Caps staged only D3 files/hunks in a temporary index,
then unstaged them; the shared index stayed untouched.
Measurements and validation logs: `/tmp/d3-*.log`,
paired raw output `/tmp/d3-pairs.jsonl`; retained executables `/tmp/d3-before-cubes`
and `/tmp/d3-after-cubes`. No commit, clone, stash, sub-agent or shared-index edit.


R2 storage-only remeasurement at `284ea691` (2026-09-18), using the same normal
Beacons build with successful `wasm-opt` and gzip-9: **765,274 / 325,962 →
766,003 / 326,394** raw/gzip bytes (+729 / +432, 0.095% / 0.133%). The before and
after `--no-build` receipts are `target/d3-size/r2-before.json` and
`r2-after-storage.json` under Beacons; this isolates typed descriptor moves before
any animation production edits. Crate/module attribution is **pre-bindgen and
pre-wasm-opt**, not a breakdown of the shipped bytes printed beside it.

The float comparison now checks exact Display as well as Debug against std for
100,000 f32/f64 bit samples, plus both signs of zero, NaN/infinities, subnormals,
notation boundaries and decimal ties. **Std Display stays decimal; Debug uses
scientific notation outside exponents [-4, 16).** `Float` follows Display, while
unrounded Data JSON follows Debug. Rounded agent JSON follows Display after the
existing four-place rounding. Special-value tests also check the two existing
JSON policies: the Data encoder refuses nonfinite numbers, while Contract values
emit null. No float spelling or pin needed changing; the review's claim that std
Display shares Debug's exponent window was disproved by these comparisons.
