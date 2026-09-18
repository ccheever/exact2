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
do not clone state, allocate buffers/objects, log, perform file I/O, or serialize on
the measured per-frame path. InputEventKey objects are also prepared before the
Godot run. Results are read once after sampling ends. Instrumentation has a small,
unsubtracted observer cost; the games' own allocation and UI work remain included.

- **Frame pacing:** consecutive rAF timestamps in three.js; consecutive
  `Time.get_ticks_usec()` samples in Godot `_process` after physics, with vsync on.
  The entire script, including the idle gaps between latency trials, is included.
  `refresh_interval_ms` is their median and `observed_refresh_hz` its reciprocal.
  p50/p95/p99 use linear interpolation between sorted values; max is the largest
  interval. A hitch is strictly greater than 1.5× that run's median. Hitch percentage
  divides by the number of recorded frames (the first has no preceding interval).
  A steady half-rate renderer can have zero hitches: compare observed cadence with
  the separately recorded **display mode's refresh rate**. These callbacks observe
  frames offered to the renderer, **not a hardware presentation fence or a guarantee
  that the compositor displayed every submission**.
- **Judder:** only frames from W key delivery + 1.0 seconds up to its release;
  both endpoints of a displacement must be in that window. Δ is the Euclidean
  world-space distance between consecutive **drawn** positions, not velocity divided
  by elapsed time. `judder = population_stddev(Δ) / mean(Δ)`; `repeated_fraction`
  counts exact Δ = 0. Player and camera translation are reported independently.
  three.js observes the capsule's mesh position and render camera in its actual
  `onBeforeRender`; Godot reads `global_position` in `_process`, or
  `get_global_transform_interpolated().origin` when interpolation is enabled. Camera
  rotation, screen-space projection and perceptual judder are not measured. A hitch
  legitimately increases this displacement metric even with interpolation; the
  camera's remaining follow-easing transient also contributes to its metric.
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
  and p95 are milliseconds and multiples of the run's median refresh interval.
  Failed/moving-baseline trials are explicit; fewer than 20 valid trials makes the
  row invalid. The command exits nonzero if any attempt is invalid.

**This measures the engine's pipeline from event delivery to presented state, not
the OS/USB path, GPU completion, compositor queue or scanout.** Chrome events use
CDP `Input.dispatchKeyEvent`; Godot uses `Input.parse_input_event` plus the proof's
`Input.flush_buffered_events`. Godot injects due events after the probe's frame sample,
so its deliveries are quantized to frames; CDP delivery can occur between callbacks.
This phase difference limits cross-engine interpretation of sub-frame latency. The
raw delivered timestamps and first changed frames make that limitation inspectable.
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
lowest hitch percentage, then p99 interval. “Median metrics” takes each column's
median across three runs; it is not a synthetic fourth trace or a pooled latency
distribution. Neither summary removes the provisional flags.

`bun test game/bench/feel.test.mjs` checks known smooth and alternating-frame motion,
known latency with distinct frame/sample clocks, residual-motion rejection, schedule
durations, quantiles and corrupt buffers. The integration verification is the live
three-run commands above; it does not build the home engine or the core/host crates.

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

Schema 1 uses `stride: 8` and flat `frames` rows of
`[frame_ms, sample_ms, player_x, player_y, player_z, camera_x, camera_y, camera_z]`.
Flat `events` rows are `[delivered_ms, VK_code, down_0_or_1, trial_id_or_minus_1]`;
all times share one monotonic origin. Include `engine_version`, `interpolation`,
`timestamp_source`, `window_pixels`, `overflow`, `hidden_frames` and
`unfocused_frames` (and `viewport_css` or `viewport_pixels`). Preallocate before
capture. The same `script()` and `analyze()` functions then apply without an
engine-specific threshold, smoothing filter, clock adjustment, or metric branch.
