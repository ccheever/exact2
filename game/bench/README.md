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
# Build the home game's ordinary web host first (repository root):
EXACT_APP_DIR="$PWD/game/games/beacons" EXACT_WEB_DIST="$PWD/game/games/beacons/dist" bun host/web/build.mjs
bun game/bench/feel.mjs exact
bun game/bench/feel.mjs three
bun game/bench/feel.mjs godot --seconds 12
```

Each command takes **three sequential runs per variant**. Godot alternates shipped /
interpolation-on, three times each. An invalid focus/latency attempt is retained and
retaken, up to three attempts per slot; high load alone never causes a retry.
JSON lines go to stdout and are also appended to
`results/feel-YYYY-MM-DD.jsonl` (UTC date); progress and a Markdown table go to stderr.
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
  row invalid. The command exits nonzero if bounded retakes cannot fill three valid
  slots per variant.

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

`exact` serves the built Beacons host and injects the adjacent
[`feel.mjs`](../games/beacons/feel.mjs) only into this benchmark page. It first opens
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

`FEEL_EXACT_DIST` selects another built directory. `FEEL_EXACT_HZ=120` labels an
explicitly built 120 Hz experiment; the runner checks the actual world Hz and refuses
a mislabeled result. To prepare that build, temporarily add `const HZ: u32 = 120;`
to Beacons' `impl Game`, build into a separate `EXACT_WEB_DIST`, then restore the
source before measuring. This changes the game's tick constant only, with no
interpolation setting. Both commands still take three valid runs.

Implementation validation includes a 144 Hz presentation/60 Hz tick fixture, tick
bursts, ring wrapping/full precision, read-once state semantics, unchanged simulation
time, and an allocation counter covering the armed trace. Adapter tests preserve
both timestamps and produce byte-for-byte identical input to the common analyzer.

**Measurement pending:** the first local attempts reached the live page but AppKit
reported `loginwindow` (PID 415) as frontmost: the display was locked even though
Chrome reported `document.hasFocus() === true` and visibility `visible`. The strict
foreground gate refused measurement and every launched Chrome was killed and awaited.
No exact timing, judder or latency row has yet been accepted; the twins' numbers
above are not evidence about this engine. Unlocking the display is required for the
three valid 60 Hz runs and the labeled 120 Hz experiment.

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
