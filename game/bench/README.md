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

## Feel — Beacons, live clock

```sh
bun game/bench/feel.mjs exact
bun game/bench/feel.mjs exact --hz 120
bun game/bench/feel.mjs compare
# Trust the existing bakes while another lane edits build inputs:
bun game/bench/feel.mjs exact --game beacons --no-build --attempts 1
```

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


#### Sitting #2 — landmark re-score, 2026-09-18T20-36-09-882Z

All 15 attempts satisfy the existing input/focus validity rules; all are provisional
at load 9–19, with nine reviewers and a build sharing the machine. This table is the
literal output of the saved-trace command; it opens no browser and takes no sitting:

```sh
bun game/bench/feel.mjs reanalyze --landmark 8,1,0 game/bench/results/feel-2026-09-18T20-36-09-882Z-*.json.gz
```

The fixed landmark is Beacons' first beacon. The player screen column is retained
only as a diagnostic: a following camera keeps that point nearly stationary.
The three cited trace batches are tracked; uncited captures remain ignored.

| variant | run / trace | status | load1 | raw Hz | raw callback intervals p50/p95/p99/max ms | raw hitches | drawn-clock intervals p50/p95/p99/max ms | drawn hitches | landmark CV (px) | landmark Δ change >0.5 px % | landmark zero % | player screen CV (diagnostic) | world player CV (m) | world player zero % | world camera CV (m) | world camera zero % | event delivery → first drawn pose p50/p95 ms | latency / raw interval p50/p95 | endpoints; injection phase | tick_phase | trials; edges | front/visible |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| exact/120hz | [reanalyzed #1 (2026-09-18T20-36-09-882Z)](results/feel-2026-09-18T20-36-09-882Z-exact-120hz-1-attempt1.json.gz) | PROVISIONAL | 18.9 | 120.5 | 8.30/10.00/10.30/10.50 | 0 (0.00%) | 8.34/8.35/8.36/8.44 | 0 (0.00%) | 0.264 | 0.0 | 0.0 | 1.825 | 0.002 | 0.0 | 0.002 | 0.0 | 6.65/7.80 | 0.80/0.94 | listener → draw pose; CDP between callbacks | 0.3231 | 20/20; 50/50 | yes |
| exact/120hz | [reanalyzed #2 (2026-09-18T20-36-09-882Z)](results/feel-2026-09-18T20-36-09-882Z-exact-120hz-2-attempt1.json.gz) | PROVISIONAL | 10.2 | 120.5 | 8.30/10.00/10.30/10.40 | 0 (0.00%) | 8.34/8.35/8.36/8.41 | 0 (0.00%) | 0.264 | 0.0 | 0.0 | 1.806 | 0.001 | 0.0 | 0.002 | 0.0 | 7.35/8.41 | 0.89/1.01 | listener → draw pose; CDP between callbacks | 0.1869 | 20/20; 50/50 | yes |
| exact/120hz | [reanalyzed #3 (2026-09-18T20-36-09-882Z)](results/feel-2026-09-18T20-36-09-882Z-exact-120hz-3-attempt1.json.gz) | PROVISIONAL | 13.1 | 120.5 | 8.30/9.90/10.30/10.40 | 0 (0.00%) | 8.34/8.35/8.36/8.37 | 0 (0.00%) | 0.264 | 0.0 | 0.0 | 1.896 | 0.001 | 0.0 | 0.001 | 0.0 | 5.10/6.50 | 0.61/0.78 | listener → draw pose; CDP between callbacks | 0.8792 | 20/20; 50/50 | yes |
| exact/120hz | best (#1) | PROVISIONAL | 18.9 | 120.5 | 8.30/10.00/10.30/10.50 | 0 (0.00%) | 8.34/8.35/8.36/8.44 | 0 (0.00%) | 0.264 | 0.0 | 0.0 | 1.825 | 0.002 | 0.0 | 0.002 | 0.0 | 6.65/7.80 | 0.80/0.94 | listener → draw pose; CDP between callbacks | 0.3231 | 20/20; 50/50 | yes |
| exact/120hz | median metrics (n=3) | PROVISIONAL | 13.1 | 120.5 | 8.30/10.00/10.30/10.40 | 0 (0.00%) | 8.34/8.35/8.36/8.41 | 0 (0.00%) | 0.264 | 0.0 | 0.0 | 1.825 | 0.001 | 0.0 | 0.002 | 0.0 | 6.65/7.80 | 0.80/0.94 | listener → draw pose; CDP between callbacks | 0.3231 | 20/20; 50/50 | yes |
| exact/60hz | [reanalyzed #1 (2026-09-18T20-36-09-882Z)](results/feel-2026-09-18T20-36-09-882Z-exact-60hz-1-attempt1.json.gz) | PROVISIONAL | 8.9 | 120.5 | 8.30/10.00/10.30/10.40 | 0 (0.00%) | 8.34/8.35/8.36/16.79 | 4 (0.04%) | 0.266 | 0.0 | 0.0 | 1.815 | 0.003 | 0.0 | 0.004 | 0.0 | 2.50/17.52 | 0.30/2.11 | listener → draw pose; CDP between callbacks | 0.0583 | 20/20; 50/50 | yes |
| exact/60hz | [reanalyzed #2 (2026-09-18T20-36-09-882Z)](results/feel-2026-09-18T20-36-09-882Z-exact-60hz-2-attempt1.json.gz) | PROVISIONAL | 11.0 | 120.5 | 8.30/10.00/10.30/10.50 | 0 (0.00%) | 8.34/8.35/8.36/8.36 | 0 (0.00%) | 0.265 | 0.0 | 0.0 | 1.758 | 0.002 | 0.0 | 0.002 | 0.0 | 6.45/8.13 | 0.78/0.98 | listener → draw pose; CDP between callbacks | 0.4380 | 20/20; 50/50 | yes |
| exact/60hz | [reanalyzed #3 (2026-09-18T20-36-09-882Z)](results/feel-2026-09-18T20-36-09-882Z-exact-60hz-3-attempt1.json.gz) | PROVISIONAL | 16.2 | 120.5 | 8.30/10.00/10.30/11.40 | 0 (0.00%) | 8.34/8.35/8.36/16.75 | 1 (0.01%) | 0.265 | 0.0 | 0.0 | 1.855 | 0.001 | 0.0 | 0.002 | 0.0 | 3.95/17.41 | 0.48/2.10 | listener → draw pose; CDP between callbacks | 0.2875 | 20/20; 50/50 | yes |
| exact/60hz | best (#3) | PROVISIONAL | 16.2 | 120.5 | 8.30/10.00/10.30/11.40 | 0 (0.00%) | 8.34/8.35/8.36/16.75 | 1 (0.01%) | 0.265 | 0.0 | 0.0 | 1.855 | 0.001 | 0.0 | 0.002 | 0.0 | 3.95/17.41 | 0.48/2.10 | listener → draw pose; CDP between callbacks | 0.2875 | 20/20; 50/50 | yes |
| exact/60hz | median metrics (n=3) | PROVISIONAL | 11.0 | 120.5 | 8.30/10.00/10.30/10.50 | 0 (0.00%) | 8.34/8.35/8.36/16.75 | 1 (0.01%) | 0.265 | 0.0 | 0.0 | 1.815 | 0.002 | 0.0 | 0.002 | 0.0 | 3.95/17.41 | 0.48/2.10 | listener → draw pose; CDP between callbacks | 0.2875 | 20/20; 50/50 | yes |
| godot/interpolation | [reanalyzed #1 (2026-09-18T20-36-09-882Z)](results/feel-2026-09-18T20-36-09-882Z-godot-interpolation-1-attempt1.json.gz) | PROVISIONAL | 13.0 | 119.9 | 8.34/14.14/16.38/28.29 | 2788 (29.37%) | 8.34/14.14/16.38/28.29 | 2788 (29.37%) | 0.141 | 0.0 | 0.0 | 0.720 | 6.59e-6 | 0.0 | 0.004 | 0.0 | 9.67/16.73 | 1.16/2.01 | _input → _process pose; after sample | — | 20/20; 50/50 | yes |
| godot/interpolation | [reanalyzed #2 (2026-09-18T20-36-09-882Z)](results/feel-2026-09-18T20-36-09-882Z-godot-interpolation-2-attempt1.json.gz) | PROVISIONAL | 21.0 | 120.1 | 8.32/14.21/14.56/32.93 | 2733 (28.21%) | 8.32/14.21/14.56/32.93 | 2733 (28.21%) | 0.142 | 0.0 | 0.0 | 0.780 | 0.004 | 0.0 | 0.005 | 0.0 | 16.50/22.76 | 1.98/2.73 | _input → _process pose; after sample | — | 20/20; 50/50 | yes |
| godot/interpolation | [reanalyzed #3 (2026-09-18T20-36-09-882Z)](results/feel-2026-09-18T20-36-09-882Z-godot-interpolation-3-attempt1.json.gz) | PROVISIONAL | 11.1 | 120.4 | 8.30/14.00/14.51/46.90 | 2589 (26.75%) | 8.30/14.00/14.51/46.90 | 2589 (26.75%) | 0.170 | 1.1 | 0.0 | 0.785 | 0.061 | 0.0 | 0.062 | 0.0 | 12.02/19.77 | 1.45/2.38 | _input → _process pose; after sample | — | 20/20; 50/50 | yes |
| godot/interpolation | best (#3) | PROVISIONAL | 11.1 | 120.4 | 8.30/14.00/14.51/46.90 | 2589 (26.75%) | 8.30/14.00/14.51/46.90 | 2589 (26.75%) | 0.170 | 1.1 | 0.0 | 0.785 | 0.061 | 0.0 | 0.062 | 0.0 | 12.02/19.77 | 1.45/2.38 | _input → _process pose; after sample | — | 20/20; 50/50 | yes |
| godot/interpolation | median metrics (n=3) | PROVISIONAL | 13.0 | 120.1 | 8.32/14.14/14.56/32.93 | 2733 (28.21%) | 8.32/14.14/14.56/32.93 | 2733 (28.21%) | 0.142 | 0.0 | 0.0 | 0.780 | 0.004 | 0.0 | 0.005 | 0.0 | 12.02/19.77 | 1.45/2.38 | _input → _process pose; after sample | — | 20/20; 50/50 | yes |
| godot/shipped | [reanalyzed #1 (2026-09-18T20-36-09-882Z)](results/feel-2026-09-18T20-36-09-882Z-godot-shipped-1-attempt1.json.gz) | PROVISIONAL | 9.8 | 120.4 | 8.31/13.97/14.54/30.38 | 2642 (27.27%) | 8.31/13.97/14.54/30.38 | 2642 (27.27%) | 1.014 | 99.4 | 49.7 | 1.461 | 0.994 | 49.7 | 0.994 | 49.7 | 8.56/16.56 | 1.03/1.99 | _input → _process pose; after sample | — | 20/20; 50/50 | yes |
| godot/shipped | [reanalyzed #2 (2026-09-18T20-36-09-882Z)](results/feel-2026-09-18T20-36-09-882Z-godot-shipped-2-attempt1.json.gz) | PROVISIONAL | 26.4 | 119.9 | 8.34/14.28/16.46/26.61 | 2879 (30.29%) | 8.34/14.28/16.46/26.61 | 2879 (30.29%) | 1.014 | 100.0 | 49.7 | 1.454 | 0.994 | 49.7 | 0.994 | 49.7 | 10.85/17.11 | 1.30/2.05 | _input → _process pose; after sample | — | 20/20; 50/50 | yes |
| godot/shipped | [reanalyzed #3 (2026-09-18T20-36-09-882Z)](results/feel-2026-09-18T20-36-09-882Z-godot-shipped-3-attempt1.json.gz) | PROVISIONAL | 15.3 | 120.2 | 8.32/13.99/14.44/30.99 | 2584 (26.67%) | 8.32/13.99/14.44/30.99 | 2584 (26.67%) | 1.020 | 100.0 | 50.0 | 1.460 | 1.000 | 50.0 | 1.000 | 50.0 | 16.48/17.05 | 1.98/2.05 | _input → _process pose; after sample | — | 20/20; 50/50 | yes |
| godot/shipped | best (#3) | PROVISIONAL | 15.3 | 120.2 | 8.32/13.99/14.44/30.99 | 2584 (26.67%) | 8.32/13.99/14.44/30.99 | 2584 (26.67%) | 1.020 | 100.0 | 50.0 | 1.460 | 1.000 | 50.0 | 1.000 | 50.0 | 16.48/17.05 | 1.98/2.05 | _input → _process pose; after sample | — | 20/20; 50/50 | yes |
| godot/shipped | median metrics (n=3) | PROVISIONAL | 15.3 | 120.2 | 8.32/13.99/14.54/30.38 | 2642 (27.27%) | 8.32/13.99/14.54/30.38 | 2642 (27.27%) | 1.014 | 100.0 | 49.7 | 1.460 | 0.994 | 49.7 | 0.994 | 49.7 | 10.85/17.05 | 1.30/2.05 | _input → _process pose; after sample | — | 20/20; 50/50 | yes |
| three/shipped | [reanalyzed #1 (2026-09-18T20-36-09-882Z)](results/feel-2026-09-18T20-36-09-882Z-three-shipped-1-attempt1.json.gz) | PROVISIONAL | 15.7 | 120.5 | 8.30/9.90/10.30/10.50 | 0 (0.00%) | 8.30/9.90/10.30/10.50 | 0 (0.00%) | 0.520 | 37.1 | 11.7 | 2.130 | 0.487 | 11.7 | 0.487 | 11.7 | 5.30/12.25 | 0.64/1.48 | listener → draw pose; CDP between callbacks | — | 20/20; 50/50 | yes |
| three/shipped | [reanalyzed #2 (2026-09-18T20-36-09-882Z)](results/feel-2026-09-18T20-36-09-882Z-three-shipped-2-attempt1.json.gz) | PROVISIONAL | 9.3 | 120.5 | 8.30/9.90/10.30/10.40 | 0 (0.00%) | 8.30/9.90/10.30/10.40 | 0 (0.00%) | 0.679 | 60.1 | 20.1 | 1.806 | 0.634 | 20.1 | 0.635 | 20.1 | 5.95/13.44 | 0.72/1.62 | listener → draw pose; CDP between callbacks | — | 20/20; 50/50 | yes |
| three/shipped | [reanalyzed #3 (2026-09-18T20-36-09-882Z)](results/feel-2026-09-18T20-36-09-882Z-three-shipped-3-attempt1.json.gz) | PROVISIONAL | 15.2 | 120.5 | 8.30/9.90/10.30/10.40 | 0 (0.00%) | 8.30/9.90/10.30/10.40 | 0 (0.00%) | 0.404 | 20.8 | 6.7 | 2.048 | 0.372 | 6.7 | 0.371 | 6.7 | 6.85/8.10 | 0.83/0.98 | listener → draw pose; CDP between callbacks | — | 20/20; 50/50 | yes |
| three/shipped | best (#2) | PROVISIONAL | 9.3 | 120.5 | 8.30/9.90/10.30/10.40 | 0 (0.00%) | 8.30/9.90/10.30/10.40 | 0 (0.00%) | 0.679 | 60.1 | 20.1 | 1.806 | 0.634 | 20.1 | 0.635 | 20.1 | 5.95/13.44 | 0.72/1.62 | listener → draw pose; CDP between callbacks | — | 20/20; 50/50 | yes |
| three/shipped | median metrics (n=3) | PROVISIONAL | 15.2 | 120.5 | 8.30/9.90/10.30/10.40 | 0 (0.00%) | 8.30/9.90/10.30/10.40 | 0 (0.00%) | 0.520 | 37.1 | 11.7 | 2.048 | 0.487 | 11.7 | 0.487 | 11.7 | 5.95/12.25 | 0.72/1.48 | listener → draw pose; CDP between callbacks | — | 20/20; 50/50 | yes |

Read adversarially: Exact raw callbacks are now recorded (8.30/10.00/10.30 ms
p50/p95/p99, no raw hitches), essentially Chrome's same rAF distribution as three.js
on this display. Its drawn clock is 8.34/8.35/8.36 ms, with one drawn hitch in roughly
9,700 frames in the 60 Hz rows. World displacement CV remains 0.001–0.002 versus
three.js 0.37–0.63 (12–20% repeats), shipped Godot 0.994–1.0 (50% repeats), and
interpolated Godot 0.004–0.061. **Landmark CV is different:** Exact medians are
0.264–0.265, three.js 0.520, shipped Godot 1.014, interpolated Godot 0.142. Perspective
and camera motion change the displacement of a fixed point even under smooth world
motion; this metric is not pure scheduler jitter. Exact 60 Hz latency is 3.95/17.41 ms
p50/p95 (the p95 includes a tick wait); 120 Hz is 6.65/7.80, higher than sitting #1's
3.5–4.1 ms median under this load. Three.js is 5.95/12–13, Godot 10.9–16.5/17–20.
Godot's 26–28% raw hitch rate at load 11–15 reflects the machine as much as the
engine. The old join is present in these saved captures and cannot be repaired from
missing causal generations; the corrected probe must be used next time. No quiet
sitting exists here, and the perceptual winner remains withdrawn. The table's `best`
selection can hinge on a 1e-12 ms tie-break: that is floating-point noise, not a win.

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
arguments at each draw as `(raw, paced, generation)`, matched one-for-one to ring
rows. Out-of-callback resize redraws inherit the preceding callback's raw stamp
and generation, even at repeated paced time; distinct callbacks always advance
generation. Only repeated generations are dropped in new traces. Overflow or a
mismatched draw count refuses analysis. Arrays are preallocated and serialized
only after sampling. Historical raw traces use raw-stamp deduplication; legacy
Exact traces without raw callbacks retain every row (no deduplication). Old traces
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
| D3 final — exact float spelling + concurrent skeleton edits | 764,322 | 325,721 | +26,554 |

Receipts for the dated table are retained in [results/d3-size.json](results/d3-size.json).
D3 kept std's decimal tie spelling; D4 lets Ryu choose the shortest significand.
Agent rounding is unchanged. The earlier stage attribution and review narrative
have been deleted; the implementation and regression tests are the record.

## D4 — module size, 2026-09-18

The same normal web recipe (`wasm-opt -Oz`, gzip level 9), measured with
`bun game/bench/size.mjs d4-before --app <game>` and `d4-after`.
[The receipt](results/d4-size.json) includes artifact SHA-256s and the complete
crate/module summaries. These are shared-checkout before/after observations;
I1's input-control addition also landed between them.

| artifact | raw before | raw after | raw reduction | gzip before | gzip after |
|---|---:|---:|---:|---:|---:|
| beacons | 886,817 | 776,398 | 110,419 | 371,418 | 333,052 |
| greybox | 1,012,003 | 901,315 | 110,688 | 417,238 | 378,722 |
| skinned-fixture | 1,056,905 | 1,028,137 | 28,768 | 439,379 | 432,001 |

**The 650,000-byte round target is not met**: Beacons is 126,398 bytes over it,
and 226,398 over the longer-term 550,000 target. Playback/pose inspection,
Carry definition reconciliation and FreshGame reconstruction no longer appear in
the ordinary primitive link map. Plain Pose/rig geometry remains in the smaller
data core because spatial bounds can consume it. This is an executor/module split,
not a new crate or Cargo feature: the existing model-capable artifact selects
`ModelPresentation`, and gameplay explicitly links animation by calling it.

A temporary build without particle presentation measured 759,870 raw / 325,992 gzip,
against its paired 775,655 / 332,683 control: only 15,785 raw bytes. The patch was
reverted; untextured emitters remain. Sprite pipelines and initial sprite storage
are absent from the primitive web artifact.

The next structural cut toward 550,000 is to put model asset ownership/discovery
and model-aware spatial bounds entirely in the model executor; primitive `World`
still owns the common asset lifecycle. At this measurement, std float formatting
still links through dynamic float clamp/panic paths (removed in the later
"Compact dynamic diagnostics" measurement below). The final
pre-optimization map attributes 47,536 bytes to Data, 44,129 to World and 37,673 to
Sim; these overlap generic/core allocations
and are not promises of shipped savings. Typed moves, deterministic sampling and
the complete state hash remain intact. No changed compiler flags or stripped
validation are credited as progress.

The [D4 check and deletion receipt](results/d4-checks.json) records the code cuts,
README line counts and validation: 519 game tests passed (11 ignored), 37
GPU/Caltrain tests passed, and 100 Bun tests passed. All seven games pass three
Linux paranoid variants; Beacons, Greybox and Skinned pass web with unchanged
simulation/save pins. Game and GPU all-target clippy, both fmt checks, caps, boot
and the Apple ExactKit build pass. Full root build remains blocked in Weatherlight's
Bun/Rolldown bake, including the bounded serial retry; no root-workspace green is
claimed. Regenerated model SHA-256s and their reasons are in the size receipt.

### Recovery hash reuse — 2026-09-18

A later paired Beacons build removed the four-line model digest reconstruction
from `WorldSurface::device_lost`. GPU loss leaves CPU model content unchanged;
asset delivery already hashes replacements, and asset preparation already fills
missing hashes after retirement/restore. Reusing that path removes work and makes
model serialization unreachable from the primitive-only module.

| Beacons GPU module | raw bytes | gzip bytes (level 9) |
|---|---:|---:|
| before | 794,148 | 339,351 |
| after | 782,915 | 335,367 |
| removed | 11,233 (1.41%) | 3,984 (1.17%) |

Both use the normal web build and `wasm-opt -Oz`. The GPU production-source delta
between captures is the four-line deletion; concurrent edits were host JavaScript,
documentation and the recovery regression. Pre-bindgen attribution confirms the
model serializers and recovery-specific BTreeMap collection/sort are gone. This
is a separate paired measurement from D4 above, not a comparison to its older
baseline. The 650,000-byte target remains unmet by 132,915 bytes.

Artifacts, SHA-256s, attribution and checks are retained locally under
`/tmp/exact-game-goal-asset-boundary/`. The extended existing recovery test covers
both retained and retired hashes and failed on the old implementation at the
unnecessary rehash assertion.

Validation: 128 renderer tests passed (7 ignored); all-target renderer Clippy,
scoped formatting, caps and boot pass. Beacons web retains its simulation/save
pins. Asset web retains its pins but still fails the previously recorded recovered
pixel/model-buffer checks; this size reduction does not close that recovery issue.


### Shared model bounds — 2026-09-18

Imported conservative bounds are computed with delivery and retained beside the
immutable model. Layout, picking and animation share those six floats. Replacing
content replaces its bounds; fresh-game reconstruction recomputes them from the
model bytes. Neither the asset format nor simulation saves change. The trade is
24 bytes per retained model entry and one geometry walk at delivery instead of
repeated walks during layout/picking before a Pose exists.

`churn --model-layout` measures one public layout plus one public pick. Three
alternating before/after runs, each the median of 7 × 100 pairs, used the same
isolated source path, lockfile and release profile. Median across those runs:

| skinned model | before (µs/pair) | after (µs/pair) |
|---|---:|---:|
| 3 vertices | 1.007 | 0.558 |
| 30,000 vertices | 649.049 | 0.420 |

Every run retains world hash `8338e471f999d37a`. These are CPU query timings on a
busy shared Mac, not frame-rate or loading-time measurements. The initial probe
was corrected to include required UVs; its invalid fixture supplied no timing.

Normal Beacons web builds changed from 782,915 to 777,083 raw bytes and 335,367
to 332,730 gzip bytes. This is a shared-tree observation: a concurrent GPU source
edit was also present, so the complete 5,832-byte difference is not attributed
solely to this change. Attribution does verify that `animated_bounds` and its
vertex walk are absent from the primitive artifact. Common asset ownership is
still linked; the 650,000-byte target remains unmet.

The 285 engine tests pass (2 ignored), including explicit replacement, Pose
precedence, read-only query/save and restore coverage. Skinned-fixture Linux
passes all three proof modes, and web passes with unchanged pose/tick/save pins.
Engine/render all-target Clippy, scoped formatting and caps pass. Evidence:
`/tmp/exact-game-goal-model-bounds/`.

The 12 renderer asset/restore tests and boot also pass. The captured tick-120
web/Linux saves match byte-for-byte (14,928 bytes); see `cross-host-save.json`
in that evidence directory. These passes do not close the separate browser
device-loss defect or the root suite's existing Keychain failure.

### Native unchanged bake — 2026-09-18

All selected native units use ordinary `cargo build`; the executable-specific
`--emit` override is removed. Copied products identify unique compiler dep-info by
exact bytes plus source/rule validation. The receipt continues to hash compiler
inputs and compile-time environment, never substituting product identity for them.

Two consecutive Beacons native bakes on this arm64 Mac take 56.130 s and 0.940 s.
The repeat compiles nothing and preserves both GPU/executable bytes, nanosecond
mtimes and the full 183-unit binary-input digest. This is one warm unchanged bake,
not cold-start or edit-to-pixel latency. The real-Cargo cache regression and web /
headless-Linux Beacons proofs pass. Logs, product hashes and timings:
`/tmp/exact-game-goal-executable-cache/real-bake-results.json`.

### Restore transfers asset ownership — 2026-09-18

`World::load` transfers delivery bookkeeping after the replacement world passes
all validation. Assets are not saved state, so cloning their names, states,
dependencies and ownership sets before decoding was redundant. Malformed,
truncated, trailing-byte and cyclic saves leave the current ownership untouched.
The asset format, public API and saved bytes stay the same.

`churn --asset-restore` uses an otherwise empty world with 0–256 delivered models.
Three alternating release before/after runs, each 7 × 100 restores, on this shared
arm64 Mac give these medians (µs/restore):

| delivered assets | World before | World after | full Sim before | full Sim after |
|---|---:|---:|---:|---:|
| 0 | 1.276 | 1.326 | 3.393 | 3.401 |
| 8 | 1.565 | 0.993 | 4.435 | 4.215 |
| 64 | 4.261 | 1.039 | 10.928 | 7.503 |
| 256 | 13.366 | 1.056 | 30.395 | 17.396 |

World hash `754bcd349d7932fb` is identical throughout. These isolate retained asset
bookkeeping, not realistic scene load time or frame rate. World source is the sole
production-code delta across the native pair; the concurrent lockfile change only
removes an unrelated generated `new-proof-*` fixture. Full Sim still validates and
constructs its candidate separately. The engine suite passes 287 tests (2 ignored),
renderer restore tests pass, and Skinned web plus Linux's three reconstruction
modes retain pose/hash/save pins. Evidence: `/tmp/exact-game-goal-restore-ownership/`.

A preceding sorted-vector texture-queue experiment was rejected and fully reverted:
its observed primitive artifact was 5,622 raw bytes smaller, but reverse-order
256-texture delivery/drain rose from 65.974 to 86.358 µs/batch (three alternating
run medians). The bounded upload queue keeps its tree map. `churn --texture-delivery`
retains the workload; replacement, retirement, single-drain ownership and restore
coverage stay in the asset tests. No shipped size saving is claimed for that
experiment. Evidence: `/tmp/exact-game-goal-texture-registry/`.

### Compact dynamic diagnostics — 2026-09-18

The private clock/movement clamp keeps std's comparisons and invalid-bound
rejection, with the existing compact float writer for panic details. Placement
depth uses the same writer. No author API or build flag changes. Attribution
confirms that std's float Debug/Display and `flt2dec` implementation are absent
from the primitive module. An initial assertion before std's clamp did not remove
the formatter under the shipped size profile; that experiment is not retained.

Normal Beacons web builds measure 776,497 → 752,263 raw bytes and 330,448 → 320,319
gzip bytes. Concurrent engine/render edits occurred between these builds, so the
whole 24,234-byte difference is not attributed to this change. The 650,000-byte
target remains unmet by 102,263 bytes. Hashes, source inventories and link maps:
`/tmp/exact-game-goal-clamp-diagnostics/`.

The clamp test compares both float widths bit-for-bit against std across edge
cases and 100,000 random triples each, including invalid bounds and signed zero.
Placement depth round-trips finite edge values through Rust's numeric parser;
the engine's JSON reader separately refuses integral decimals beyond u64, now
queued. Final engine/renderer coverage passes 420 tests (9 ignored). All-target
Clippy, formatting, caps and boot pass. Beacons web and Linux's three restore
modes retain their pins, and all six captured web/Linux save files are identical.
Placement web also passes. Browser descendant audits report `ps` unavailable;
all recorded children are awaited. These checks do not close the existing root
Keychain refusal or establish new authoring/perceptual scores.

### Primitive asset boundary — 2026-09-18

Primitive surfaces reject unsupported models/sprites directly instead of running
the full request, retirement and residency machinery to manufacture a failed
delivery. Initial and late references name the missing `game.assets` setting.
The check reuses component revisions plus the existing world identity, so unchanged
polls do not scan entities and a replaced/restored world is always rechecked.
The model-capable executor retains its delivery path; no author API or flag changes.

Normal Beacons web builds measure **750,672 → 725,274 raw bytes** (25,398 removed)
and **319,803 → 310,111 gzip bytes**. Only `surface.rs` changes in the production
source inventory across this pair. The final link map excludes Sim's asset request,
failure, retirement, device-invalidation and model-readiness paths. Common World
asset ownership remains linked. The 650,000-byte target is still 75,274 bytes away.

A temporary release probe measures `Surface::assets` over primitive-only worlds.
Three alternating pairs, each taking the median of seven batches:

| meshes | unchanged poll before/after (ns) | one Mesh write + poll before/after (µs) |
|---|---:|---:|
| 0 | 8.720 / 3.190 | — |
| 100 | 11.170 / 8.069 | 0.140 / 0.145 |
| 10,000 | 11.414 / 8.027 | 12.233 / 11.819 |

All world hashes agree. The 100-mesh rescan samples overlap; these are CPU polling
diagnostics on the shared Mac, not frame-rate claims. Probe source, binaries,
inventories, digests and raw runs: `/tmp/exact-game-goal-primitive-assets/`.

The late-reference regression fails on the prior implementation, then passes for
models and sprites, clean-save recovery, and replacement worlds with equal revision
counters. Final renderer coverage passes 133 tests (7 ignored), as do all-target
Clippy, formatting, caps and boot. Beacons web/Linux pass on the final cache; Skinned
web/Linux pass on the preceding candidate (its only subsequent runtime change is
the primitive check's identity key). Linux uses all three restore modes. All captured
Beacons and Skinned save pairs agree across hosts. Browser descendant audits report
`ps` unavailable; recorded children were awaited. Existing root/Apple/perceptual
gaps remain separate.
