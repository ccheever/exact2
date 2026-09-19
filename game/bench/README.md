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

### JSON float destinations — 2026-09-18

JSON now parses floats at their destination width. Large integral-looking
decimals such as `f32::MAX` no longer fail an intermediate u64 conversion, and
`1.0000000596046448` rounds directly to f32 instead of rounding through f64.
Reader's default f32/f64 methods preserve binary conversions; integer reads still
reject overflow. Unknown fields validate number grammar without imposing a numeric
range on discarded values. Typed float overflow and malformed JSON remain errors.

The isolated production-source pair changes only the three Data files: Beacons
measures **725,274 → 727,915 raw bytes**, **310,111 → 311,423 gzip bytes** with the
selected normal-inline scanner. The 650,000-byte target remains unmet. Three
bounded candidates were tried; forced inlining adds another 5,108 raw bytes and
is not retained. Three rotating run medians, in µs per 1,000 three-float rows:

| destination | before | normal inline | forced inline |
|---|---:|---:|---:|
| f32 | 63.046 | 62.597 | 61.642 |
| f64 | 61.709 | 61.708 | 57.271 |

Earlier rotating runs measured 52.088 → 57.473 µs for f32 and 51.850 → 55.980 µs
for f64 with normal inlining. Shared-machine variance prevents a no-regression or
speedup claim; a quiet comparison remains queued. These are decoding diagnostics,
not frame timings. Native and Bun WebAssembly separately check 100,000 bit-pattern
samples per width, decimal/binary round trips, strict integer ranges and the f32
midpoint case; both produce digest `10542334cfc1939f`. The benchmark's separate
`input_hash` identifies its input, not decoded output.

The regressions fail before the correction. The initial corrected tree passes
549 workspace tests (11 ignored), all-target Clippy, formatting, caps and boot;
Beacons web and Linux's three restore modes pass with six identical save pairs.
After selecting normal inline, all 10 Data tests, the full placement-response
round trip, engine all-target Clippy, Data formatting, caps and boot pass. The
shared-tree web recheck retains the original tick/save pins but fails four
pointer-control assertions. Concurrent render/host edits also leave workspace
formatting and all-target Clippy failing (`bool_assert_comparison` in
`render/tests/core.rs`). Thus the earlier green suite is not a final integrated
green claim. No pins were changed. Sources, binaries, runs and failures are in
`/tmp/exact-game-goal-json-floats/`.

### Shared primitive trigonometry — 2026-09-18

Primitive geometry uses glam's existing portable trig backend. `revolve` computes
one circle and reuses it across rings: 200 temporary bytes at 24 segments, released
when construction finishes. No author API, manifest flag or retained cache changes.
Three alternating native pairs, median of seven batches per run, measure four
primitives (sphere, cylinder and two capsules):

| segments | before (µs) | final (µs) |
|---|---:|---:|
| 24 | 8.710 | 7.249 |
| 64 | 46.143 | 33.351 |
| 256 | 758.549 | 624.803 |

This is 17–28% faster mesh construction on the shared Mac, not a frame-rate claim.
The first portable-trig candidate repeated longitudes and was slower; circle reuse
removes that cost. Portable shadow power was also tried: a cascade fit rose from
0.166 to 0.202 µs, so `shadows.rs` is restored byte-for-byte. Three candidates close
here. The final unchanged shadow calculation measures 0.180/0.168 µs in the pair;
that variation is not an attributed speedup.

Native and Bun WebAssembly produce matching digests of all vertex/index bits for
32 generated meshes (3–257 segments) and 48 shadow fits, both before and after:
`5099ec9f61dd99b9` and `17d18412e74cc909`. The CPU probe compiles the actual geometry
and cascade-fitting source with matching glam/libm versions; unused Wasm host
imports throw if called, and none is called. It is not a GPU pixel-parity claim.

Normal Beacons builds observe 740,610 → 736,397 raw bytes and 316,850 → 315,404 gzip
bytes. Concurrent input/render changes prevent attributing the whole delta.
Pre-opt compiler math attribution falls from 9,787 to 3,021 bytes: the duplicate
trig/range-reduction routines disappear; shadow power remains. The 650,000-byte
target is still unmet by 86,397 bytes. Source inventories, binaries, alternating
runs and link maps are in `/tmp/exact-game-goal-render-math/`.

Workspace build, all-target Clippy, formatting, caps and boot pass. Beacons web and
Linux normal/Save/FreshGame proofs pass; six captured save files agree byte-for-byte.
The physical-pointer capture differs only in carrier contact ID and local origin/
position, with the same tick-6 world hash. The full workspace sweep, including
documentation checks, passed 559 tests without failures (11 ignored). A temporary
probe was sampled at `_dyld_start`
before Rust and later completed; shared Cargo waits and native startup delays make
these wall times unsuitable as edit-loop measurements. All three math candidates
are closed. No pin changed; the goal remains active.

### Target labels only on failure — 2026-09-18

`Target::entity` borrows its target so character, audio, socket and physics helpers
can format the target's name only on refusal. Game calls and error text stay the
same. Socket cycle/stale-pose labels are lazy too. An instrumented release probe
calls the real APIs 10,000 times each after warming their caches:

| Operation, by name or entity | Allocations/call before | After |
|---|---:|---:|
| Character step | 1 | 0 |
| Capsule controller lookup (without the physics step) | 1 | 0 |
| Construct and position an audio play | 2 | 1 |
| Socket matrix query, retained pose | 4 | 2 |

Unpositioned audio play still allocates once for the sound name. Socket lookup
still allocates its owned cache keys. Character and socket world hashes match the
baseline (`6d50cb871af0b48d`, `cae0edfdeefba5ad`). These are allocation counts, not
frame-rate or elapsed-time measurements. No cache or author-facing option is added.

The normal Beacons build is 736,397 → 737,332 raw bytes and 315,404 → 315,392 gzip;
this shared-tree comparison is not isolated attribution. The raw-size target
remains unmet. Two compiler-hint candidates (`cold`, then `inline(never)`) saved
no raw bytes and compressed seven bytes worse; both were removed. Three candidates
are closed. Sources, binaries, per-call counts and receipts are in
`/tmp/exact-game-goal-target-labels/`.

Engine/physics tests pass (328, with 2 ignored), as do the workspace build,
all-target Clippy and formatting. Beacons web/Linux and the animated-model web
proof preserve their pins. Linux's first freshly linked launch timed out before
readiness; its recorded child exited, and a cached retry passed in 5.26 seconds.

### Borrowed socket lookup names — 2026-09-18

The socket cache stores one model identity and a joint map per model. Warm lookups
borrow both names, and the redundant inner `RefCell` is removed. The author API
is unchanged. Successful joint lookups and matrix queries with a retained pose
now allocate zero times (previously twice); a cached missing joint allocates once
for its returned error (previously three times).

Three alternating before/after native runs, each taking the median of seven
100,000-call batches, measured these successive warm query sets. Allocation
counting was disabled during timing; the shared Mac reported load 23.7.

| Queried models × joints/model | Joint lookup, ns before → after | Retained-pose matrix, ns before → after |
|---|---:|---:|
| 1 × 1 | 61.9 → 33.6 | 130.7 → 108.8 |
| 1 × 32 | 98.5 → 59.4 | 165.7 → 132.3 |
| 32 × 8 | 111.5 → 85.1 | 199.0 → 176.5 |

The probe's world hash stays `4102584e342ccf94`; its 91,001-byte save stays unchanged
through the queries. These are CPU query timings, not frame rates. Model redelivery
invalidates cached successes and refusals even while the old model remains alive;
the strengthened regression also checks that warming does not alter a save.

The animated fixture's observed module grows 1,063,949 → 1,065,684 raw bytes and
440,730 → 440,879 gzip. This is a shared-tree comparison, not isolated attribution.
The Beacons product built successfully, but another writer deleted its app and
output for the fifth authoring build before the new bytes could be captured; no
primitive-module size claim follows. Evidence: `/tmp/exact-game-goal-socket-cache/`.

Engine tests pass (296, 2 ignored), including all 38 animation tests. The animated
fixture passes on web and Linux with all four saves byte-identical and its existing
tick/save pins retained. Workspace build, formatting, caps and boot pass. The first
native launch timed out before readiness; its next attempt hit the concurrent
Beacons deletion, and the third passed after its manifest returned. Workspace
Clippy also hit the deletion; its replacement all-target run passed.

### Game workspace resolution — 2026-09-18

Games now have a known generated workspace, so `resolveApp` reads the manifest
before deciding whether to invoke `cargo locate-project`. Only ordinary external
apps need that lookup. A warm external-game probe measured 15 calls before and
after: median **28.97 → 0.531 ms**, range 25.59–62.13 → 0.420–1.104 ms. Each run
kept all 22 generated/app files byte-identical and returned the same app identity,
manifest, workspace and target. This is resolution time, not proof time or FPS.
Evidence: `/Users/ccheever/projects/.exact-game-verification/resolve-probe/`.

Generated-adapter cleanup also preserves Cargo's `target/` directory while
removing old adapter identities. Six starter checks, 54 proof checks and three
ordinary-app resolver checks pass, along with copied-index caps and boot. The
cold new-game integration built both native binaries, then refused the build
receipt: `no matching rustc unit dep-info for equivalent`. It did not reach app
assertions or web; three integration attempts are closed. These results do not
claim a sub-second complete proof.

The receipt failure was rustc's raw output path containing spaces. The concurrent
writer fixed that matcher; the existing real-Cargo regression now exercises a
workspace with spaces. All four focused checks pass in 12.53 s, covering library
and executable receipts, source/environment changes, cache reuse and refusal of
ambiguous dep-info. This verifies the receipt fix, not the complete new-game proof.
Evidence: `/Users/ccheever/projects/.exact-game-verification/dep-info/focused.log`.

### Direct publication values — 2026-09-18

`World::publish` converts scalars and strings directly into its retained value;
owned strings move into it. Updating an existing publication reuses its key and
one map lookup. No author-facing option or cache is added. An isolated source
snapshot, counting 20,000 warmed calls per case, measures:

| Publication | Allocations/call before → after |
|---|---:|
| Unchanged borrowed string | 2 → 1 |
| Unchanged owned string, including caller construction | 3 → 1 |
| Changed scalar, including journal | 9 → 8 |
| Changed borrowed string, including journal | 12.5 → 10.5 |
| Unchanged two-field record | 6 → 6 |

Three alternating native pairs, each using seven timed batches with counting
disabled, put owned-string calls at 48.4–60.2 → 16.5–20.8 ns (65–66% faster).
Changed scalar calls improve 11–24%; these are call timings, not frame rates.
The first candidate's nested-value ownership conversion added two allocations
for shared values and was removed. Shared/nested value allocation counts are
unchanged in the retained candidate. Both probe binaries are 695,536 bytes;
their published output, journal tail, 284-byte world save and hash are identical.

Beacons passes on Linux and web, including HUD delivery and fresh-process restore.
All three captured save files match across hosts; observed tick pins, final world
hashes, publications and journal lines agree. The game's pins file remains empty,
so these are observed comparisons, not a repinned baseline. Native journal chunks
also carry a tick field that the web chunks omit. Evidence and rejected candidate:
`/Users/ccheever/projects/.exact-game-verification/publication/`.

The final no-host-argument proof passes in 47.90 s wall time, including another
rebake after concurrent edits; this does not measure the warm headless loop. The
preceding native attempt timed out before readiness. A live workspace test harness
was independently sampled at `_dyld_start`, before Rust, during these checks.

The full workspace run finished with 564 passing tests, one texture-redelivery
failure and 11 ignored. The concurrent writer's subsequent redelivery fix passes
all 22 asset tests. Workspace build and all-target Clippy passed; formatting passes
after the other writer formatted its host-control regression.

### Guide edits do not rebuild the engine — 2026-09-19

The crate includes the game guide only under `cfg(doc)`. Normal native/Wasm
compilation no longer depends on it, and proof inputs drop their special README
exception. The runnable example remains one of the 21 passing documentation tests.

In the isolated engine-and-consumer Cargo probe, a guide-only edit previously
rebuilt both packages (19.31 s reported by Cargo). With the conditional include,
another guide edit leaves both artifacts fresh: 0.064 s command wall time, with
identical executable bytes and modification time. A subsequent Rust-source edit
still rebuilds both packages. This measures avoided compilation, not the full
game proof or startup. All 54 proof-harness tests pass, including documentation
exclusion and source/asset invalidation. Evidence:
`/Users/ccheever/projects/.exact-game-verification/doc-build/`.

### Attachment maintenance stays with its component — 2026-09-19

SocketFollow installs pose resolution and tick-boundary maintenance together.
The core world's propagation calls that optional executor instead of naming
socket animation directly. Save/load retains both callbacks; dead-follower pruning
and skipped-animation boundaries keep their existing behavior.

The normal paired Beacons web recipe measures **754,607 → 743,249 raw bytes**
(11,358 removed) and **323,209 → 318,687 gzip bytes**. Only the four engine files
implementing this boundary changed among runtime inputs; a concurrent pins update
does not enter the module. The pre-optimization map has no SocketFollow,
FollowerPoses or prune_follower_poses entries after the change. No new public API,
crate, feature or build option is involved. The 550,000-byte target remains open.

Beacons and Skinned proofs pass on web and headless Linux with unchanged tick/save
digests, including the model fixture's stale and restored attachments. All seven
captured save files agree byte-for-byte across hosts. Evidence and paired artifacts:
`/Users/ccheever/projects/.exact-game-verification/primitive-ownership/`.
The full game workspace passes 565 tests (11 ignored), build, all-target Clippy
and formatting; caps and boot pass. The cleanup regression covers a restored world
with its last follower despawned. Browser descendant inventory was unavailable for
Beacons; all recorded carrier children exited. No frame-rate change is claimed.

### Ordinary proof uses one headless run — 2026-09-19

With existing pins, `bun game/prove.mjs beacons` selects Linux and lets that proof
check its own build receipt. It no longer starts a second build-only invocation
or a browser. Explicit multi-host checks retain their serial bake phase;
`--compare-saves`, first-baseline collection and `--repin` still default to Linux
and web. The starter prints this ordinary command, and `prove.mjs .` works from
the game directory. These are routing changes, not reduced gameplay assertions.

The 55 harness tests cover default, repeat, explicit web, cross-host comparison,
external/current-directory entrypoints, failures and all-mode initial pinning.
Three actual default Beacons runs pass with the existing tick/save observations
and all recorded carrier children exited. All three refreshed stale build receipts
as the shared tree changed (5.74, 259.82 and 12.62 s command wall time); no warm-loop
improvement is claimed. The long run was sampled inside Cargo's build-script output
wait. Evidence: `/Users/ccheever/projects/.exact-game-verification/proof-default/`.

### Binary decoding borrows internal names — 2026-09-19

The decoder's name table and per-record duplicate guard borrow validated UTF-8
from the input. Only names returned through `Reader` allocate a string. The
reader API, encoder and save format are unchanged; no new option or dependency.

An isolated source snapshot differs only in `data/bin.rs`. Three alternating
native pairs, each with seven timed batches and allocation counting disabled
during timing, measure:

| Decode | Allocations before → after | Requested bytes before → after | Time reduction |
|---|---:|---:|---:|
| 256 five-field rows | 3,087 → 1,802 | 127,538 → 96,690 | 16–25% |
| 4,096 five-field rows | 49,171 → 28,686 | 2,042,954 → 1,551,306 | 19–21% |
| Restore a 1,024-entity world | 32,854 → 19,507 | 1,746,030 → 1,205,355 | 22–27% |

World-load medians are 0.824–0.876 → 0.639–0.658 ms. Every pair preserves
encoded bytes and hashes. These are native decode/restore measurements, not frame
rates or end-to-end proof latency. The standalone probe grows 64 executable bytes;
no shipped module size reduction is claimed. Regression coverage checks duplicate
literal names at different input offsets, nested skipped names and their later
reuse. Evidence: `/Users/ccheever/projects/.exact-game-verification/codec-names/`.

Beacons and Skinned proofs pass on web and headless Linux with their existing
pins. All seven captured saves match across hosts, including fresh-process
continuation and stale/restored model attachments. All recorded carrier children
exited; browser descendant audits were unavailable on this run.
The full game workspace passes 566 tests (11 ignored), build, all-target Clippy
and formatting; caps and boot pass.

### Shared record traversal rejected — 2026-09-19

Three derive candidates used the same normal Beacons web recipe and isolated
native decode probe, with three alternating pairs of seven timed batches each:

| Candidate | Raw bytes removed | 4,096 mixed Mesh values: decode slowdown |
|---|---:|---:|
| Shared runtime record loop with a field callback | 17,956 | 6–23% |
| One field loop per enum, dispatching by its active variant | 15,129 | 7–11% |
| Error context shared after each field match | 9,346 | 2–8% |

All preserve probe save bytes and allocation counts. **All are reverted**: their
smaller code comes with observed decode costs. The first also slows 256-row
decoding by 6–15%. These are native decode timings, not FPS; the three-candidate
loop is closed. The baseline is 746,976 raw / 320,274 gzip bytes. Further size work
should remove unnecessary linked ownership/execution, with these traversal shapes
left alone. Evidence: `/Users/ccheever/projects/.exact-game-verification/shared-record-read/`.
The rebuilt restored artifact matches the baseline byte-for-byte; 34 focused
data/save tests, caps and boot pass. Restoration explicitly refreshed source
timestamps after the first check caught Cargo reusing an experimental artifact.

### Asset ownership is optional and shared — 2026-09-19

World holds an optional copy-on-write asset owner. Sim rebuilds share its maps
until a mutation; existing declarations no longer cause redundant writes. Reads
stay direct. The first asset write installs destruction, keeping model ownership
code outside primitive modules without a new public API or build option.

The normal Beacons recipe measures **746,976 → 744,771 raw bytes** and
**320,274 → 319,432 gzip bytes**. The inline native World shrinks **1,032 → 760
bytes**; a populated owner uses a separate allocation, so this is not a total
model-world memory claim. The pre-optimization map loses Assets::clone.

Three alternating native pairs, seven batches of 10,000 restores each, measure
256-asset Sim restore at **15.47–18.85 → 2.51–2.95 µs** (84–86% faster).
Eight assets improve 22–27%; 64 improve 55–64%. World hashes agree in every
pair. Empty restores and model layout/picking are noisy; no gain is claimed
there, nor in frame rate. The isolated probe changes only four runtime files.
The web pair also spans the app's concurrent manifest move to its own workspace,
with the same engine dependency. The 550,000-byte target remains open.

The final isolated engine and asset suites pass 109 tests (one ignored), including
copy isolation, failed-load preservation, final-owner release, delivery retirement
and device recovery. The full game workspace passes 534 tests (11 ignored), build,
all-target Clippy and formatting; caps and boot pass. Beacons, Asset and Skinned
proofs pass on web and headless Linux with their existing pins and all nine valid
captured saves equal across hosts. Linux Save-every-tick and FreshGame modes pass
for all three too; their 18 captured saves equal the continuous run. Recorded
carrier children exited; browser descendant inventory was unavailable. Evidence:
`/Users/ccheever/projects/.exact-game-verification/optional-asset-owner/`.

### Restore constructs the game once — 2026-09-19

Creation and restore share one constructor receiving the retained asset owner.
Restore previously constructed a game with no assets, discarded it, then built it
again. The regression reproduces two setup calls on a valid restore before the
change. Afterward it verifies one on ordinary/bound restore, refused world data
and both reconstruction modes. Existing clock tests moved out of sim.rs.

An isolated native snapshot changes only sim.rs at runtime. Three alternating
pairs, then three reversed pairs after our builds finished, each use seven timed
batches with allocation counting disabled during timing:

| Restore | Allocations before → after | Requested bytes before → after | Native time reduction across both passes |
|---|---:|---:|---:|
| Beacons | 1,083 → 935 | 1,095,836 → 771,970 | 11–24% |
| 4,096 named cubes | 168,209 → 151,750 | 12,069,303 → 10,594,485 | 4–15% |

Empty, 16-entity and 256-entity timings are mixed in the first pass on the shared
Mac. In the reversed pass, their times decrease 5–13%, 10–14% and 7–14%
respectively; their allocation counts decrease in both passes. Every probe retains
its world hash and identical save bytes. These are native restore costs, not frame
rates or full proof latency. The normal Beacons module measures
**744,771 → 744,798 raw bytes** and **319,432 → 319,384 gzip bytes**: this removes
work, with essentially unchanged shipped size. The 550,000-byte target remains open.

The full game workspace passes 535 tests (11 ignored), build, all-target Clippy
and formatting; caps and boot pass. Beacons, Asset and Skinned proofs pass on web
and Linux, plus Save-every-tick and FreshGame on Linux. All 27 cross-host/mode save
comparisons are byte-identical, with existing pins and all recorded carrier
children exited. Beacons' browser descendant audit was unavailable. Evidence:
`/Users/ccheever/projects/.exact-game-verification/single-setup/`.

### Compact type directories rejected — 2026-09-19

Two sorted-vector replacements for World's tree maps were measured and removed.
The first replaced registration, component and resource directories; the second
kept registration in its tree. Their normal Beacons modules measured 736,612 and
741,288 raw bytes, respectively, against 744,798 (gzip: 316,690 and 318,378 against
319,384).

Across three alternating-order native pairs, reverse-order setup of 256 types
became 50–58% slower with all directories replaced and 20–26% slower with only
storage directories replaced. Both improved large-directory lookups, but the
all-directory version improved actual Beacons restore time only 1–4% and added
four allocations. Small-scene restore results were mixed. Both candidates retained
identical save bytes and hashes in the type-directory probe; the all-directory
candidate also retained them for Beacons and four scene populations.

The size saving does not justify extra custom collection code and slower type
installation. This experiment is closed; the original maps remain. Evidence:
`/Users/ccheever/projects/.exact-game-verification/type-tables/`.

### Named world bindings across Exact2 — 2026-09-19

Beacons and the starter now bind `world(seed: 7, paused: paused, restart: again)`.
Names may reorder; omitted named fields use Rust Args defaults. The compiler
rejects duplicate or mixed labels. The GPU module resolves names against the
current Args metadata before the existing typed bind, preserving atomic refusal,
pause, restart and saved continuation. Positional calls keep their existing arity.
Host serialization is shared in the runner, with no new GPU ABI export or work
on every frame. Formatter/symbol integration from the landed 1035.005 lane remains.

The normal Beacons recipe measures **744,798 → 749,148 raw bytes** and
**319,384 → 321,433 gzip bytes**: +4,350 raw bytes (0.58%) buys the named authoring
seam. No binding-speed or frame-time gain is claimed; the 550,000-byte target
remains open.

The game workspace passes 536 tests (11 ignored), build, all-target Clippy and
formatting. Focused coverage passes five compiler tests, four render restore tests,
30 Swift tests and 55 browser surface tests. Beacons passes web, Linux, macOS and
iOS proofs, plus Linux Save-every-tick and FreshGame. Asset passes web and Linux
with positional calls. All 24 comparisons of captured saves across hosts, modes
and the preceding positional implementation are byte-identical; existing game
pins are unchanged. A generated external starter fills its first pins only after
all six web/Linux mode collections agree, then passes an ordinary pinned Linux
proof. Caps and boot pass using a private staged index; the shared index is unchanged.

The full root build, Clippy and formatting pass. Root tests initially fail in
three targets: a stale Linux control fixture is repaired (all 34 Linux tests pass),
and the Messages doctest artifact mismatch clears on rebuild. The known real
Keychain fixture still refuses access. Caltrain builds and passes all three web
assertions; its smoke fails solely on Chrome's Keychain/encryption errors. A green
root suite or Caltrain smoke is not claimed. Recorded proof carriers exit; browser
descendant inventory is unavailable on some runs. Evidence:
`/Users/ccheever/projects/.exact-game-verification/named-surface-args/`.

### Record-publication candidates rejected — 2026-09-19

Three candidates tried to remove the temporary HUD tree without changing the
authoring API. Each was compared with the same isolated current-source snapshot,
in three alternating-order native pairs, seven timed batches of 20,000 calls per
case with allocation counting disabled during timing.

| Candidate | Unchanged two-field HUD allocations | Beacons raw module bytes |
|---|---:|---:|
| Original writer | 6 | 749,148 |
| Borrow retained tree before constructing a changed record | 0 | 751,844 |
| Stage changed root fields with borrowed names in each frame | 0 | 755,332 |
| Keep original nested frames; borrow root names only | 1 | 750,139 |

Borrowed comparison sped unchanged flat and nested records by about 80%, but
changed nested cases became up to 18% slower and acquired two allocations.
The other candidates reduced changed-record allocations too, but their timings
did not justify the added code and bytes: the final candidate's nested record
update was 4–13% slower in the last two pairs; its first pair was noisier. There
is no frame-rate claim. The single-frame candidate's smaller preliminary artifact
preceded the repeated-field fix and was not retained.

Published JSON, world hashes and journal tails agree in all 18 paired runs;
each run also verifies that publication leaves its world save bytes unchanged.
The candidates pass nested edits, numeric arrays, signed zero, saved continuation
and atomic-refusal tests; the final two also pass the repeated-field regression.
All three implementations and their temporary test additions are removed. The
original source is restored exactly; this three-candidate experiment is closed.
The restored native probe and shipped Wasm are byte-identical to their baselines.
All 58 focused data, simulation and ergonomics tests pass, as do formatting, caps
and boot. Only these measurements and the queue note remain; the shared index
was unchanged by validation, which used a private staged copy.
Evidence: `/Users/ccheever/projects/.exact-game-verification/record-publication/`.

### Dynamic values stream without an owned copy — 2026-09-19

`Value::write` now emits the existing `Stored` variant framing directly. It no
longer clones strings, options and sequences into another tree before passing
them to the save, hash or JSON writer. The public API and decoder are unchanged.
A conformance test compares all Value variants against Stored's derive, including
unit versus empty containers, nested shared values, signed zero, non-finite
numbers, error paths and binary round trips.

An isolated source snapshot changes only values.rs at runtime. Three alternating
native pairs use seven batches of 2,000 calls each, with allocation counting
disabled during timing. The representative value contains 32 nested records:

| Operation | Allocations before → after | Native time reduction |
|---|---:|---:|
| Value hash | 68 → 0 | 15–25% |
| Value binary encoding | 85 → 17 | 25–38% |
| Value JSON encoding | 481 → 413 | 9–21% |
| World save with a dynamic Value component | 106 → 38 | 27–28% |
| Full simulation save | 141 → 73 | 17–19% |

At 256 nested records, hash allocations fall 516 → 0 and binary allocations
536 → 20. Scalar JSON measured 0.5–6.3% slower in these pairs; no improvement is
claimed there or in frame rate. All 27 paired binary, JSON and complete Sim save files
are byte-identical. The native probe shrinks 679,472 → 679,328 bytes. The normal
Beacons module is effectively unchanged: **749,148 → 749,140 raw bytes**,
**321,433 → 321,466 gzip bytes**. The 550,000-byte target remains open.

Beacons passes web and Linux proofs with existing pins, plus Linux Save-every-tick
and FreshGame. Its 12 comparisons across hosts/modes and the prior accepted
implementation have identical save bytes. A generated external game with a nested
Value component agrees across all six web/Linux reconstruction-mode runs, then
passes pinned proofs on both hosts with identical saves. The full game workspace
passes 537 tests (11 ignored), build, all-target Clippy and formatting. Recorded
carrier children exited; Beacons' browser descendant audit was unavailable.
Evidence:
`/Users/ccheever/projects/.exact-game-verification/value-streaming/`.

### Shared clock preparation rejected — 2026-09-19

Two candidates extracted the clock preparation from `Sim::advance_with`, keeping
the post-tick callback inline. Returning an optional target saved 1,460 raw / 945
gzip bytes; returning the current tick on no work saved 1,580 / 1,041. Both preserved
clock tests and saved state. Short timing samples were noisy; longer whole-probe
pairs were still mixed, so the final comparison ran each workload immediately
before/after, alternating order across five pairs, with seven timed samples per run.

The final candidate slowed the tiny live one-tick plain path in four of five pairs
(2.4–8.3% in those pairs) and the tiny seekable one-tick plain path in four of five
(0.5–8.5%). Across-run medians were 27.5 → 28.9 ns and 189.7 → 201.4 ns respectively.
Beacons timings remained mixed; no frame-rate claim follows. All 132 comparisons
across the three timing rounds produced identical saves, hashes and tick counts.

The size saving does not justify the extra helper and the ordinary-step cost.
The original source and normal Beacons artifact are restored byte-for-byte:
749,140 raw / 321,466 gzip bytes. This experiment is closed. Evidence:
`/Users/ccheever/projects/.exact-game-verification/clock-preparation/`.

### Encoder name storage rejected — 2026-09-19

Three candidates tested the binary encoder's owned name dictionary. Two stored
output offsets behind a fingerprint and exact byte comparison, removing most
name allocations. The first reused the Data hasher; its initial measurements
slowed repeated short names and large reverse-ordered dictionaries. The second
used a cheaper fingerprint: three alternating native pairs measured Beacons
World saves 46–48% faster and Sim saves 43–45% faster, with allocations falling
107 → 24 and 195 → 81. However, 4,096 and 16,384 reverse-ordered names were
16–38% slower, and the normal shipped module grew 868 raw / 65 gzip bytes.

The final candidate retained the standard BTreeMap and replaced String keys with
Box<str>. It added no runtime helper, reduced cumulative requested allocation
bytes by 1,056 per Beacons World save and 1,496 per Sim save, and left allocation
counts unchanged. Native timings were mixed; 64 sorted names were 2.5–6.9%
slower in all three pairs. The module grew 144 raw bytes and shrank 209 gzip bytes.
These small memory savings do not justify a measured time regression or a new
lookup implementation. Requested allocation bytes are not peak resident memory.

All 19 serialized cases agreed byte for byte in each pair for the second and
third candidates, including field counts through 16,384, Unicode names and real
World/Sim saves. Tests also checked shared field/variant encounter-order IDs and,
for the offset candidates, collisions across buffer growth. All three candidates
and their temporary tests are removed; the original encoder is restored exactly.
The fresh restored build is byte-identical to the baseline: 749,140 raw /
321,466 gzip bytes. All 138 focused engine, data, save and simulation tests pass
(1 ignored), as do formatting, caps and boot. The shared index has no staged
content changes; caps used a private staged copy. This experiment is closed. Evidence:
`/Users/ccheever/projects/.exact-game-verification/encoder-names/`.

### Save framing uses one buffer — 2026-09-19

World and Sim saves now start their binary encoder with the existing file header.
They no longer allocate another vector and copy the complete encoded payload just
to prepend that header. The internal constructor starts with 128 bytes for the
fixed save metadata. The ordinary binary writer and the public API are unchanged;
EXGAME v3 and EXSIM v5 bytes remain identical. Sim still owns a temporary encoded
World and copies its other saved fields; this is not a claim that saving allocates
nothing or streams directly to a file.

Three candidates were measured. Starting with only the header reduced copies but
still grew the tiny buffer repeatedly. Shrinking the result at finish preserved
exact capacity but made the 16 MiB World save 151–157% slower on this allocator;
that candidate is removed. The retained version reserves fixed metadata space
and returns the normal growable vector. There is no size-dependent branch or new
caller setting.

The native probe compares 21 cases, including empty worlds, 64/1,024/16,384 named
entities, bulk payloads through 16 MiB and Beacons after 90 ticks. Allocation
counting is disabled during timing. Initial measurements used three alternating
pairs of seven batches; a second comparison increased calls per batch and used
five adjacent before/after pairs for ordinary and entity-heavy saves:

| Save | Allocation calls before → after | Longer paired native result |
|---|---:|---:|
| Empty World | 18 → 12 | 21–28% faster |
| Beacons World | 107 → 101 | mixed: 4.8% slower to 3.3% faster |
| Beacons Sim | 195 → 185 | 0.4–2.5% faster |
| 1,024-entity World | 52 → 46 | 0.2–1.5% slower |
| 16,384-entity World | 56 → 50 | 0.2–4.2% slower |
| 16,384-entity Sim | 4,187 → 4,176 | 6.8–11.2% faster |

These are save measurements, not a frame-rate improvement. In the initial bulk
pairs, 64 KiB–16 MiB World saves were 39–52% faster; the longer table above is the
stronger evidence for ordinary and entity-heavy behavior. The direct World timing
cost is retained in exchange for fewer allocations, faster full simulation saves
and lower temporary buffer usage, without another encoding path.

The memory trade is explicit. At 16,384 entities, maximum live requested allocation
bytes visible to the allocator probe fall 7,738,448 → 4,196,417 for World and
14,668,531 → 11,574,890 for Sim. This excludes allocator internals and is not peak
resident memory. Returned vectors may retain more capacity: Beacons World rises
4,398 → 8,192 bytes, and the 16 MiB payload's World save rises approximately
16 → 32 MiB. A bulk Sim's measured peak is essentially flat. No reduction in
retained save-buffer memory is claimed.

The normal Beacons module measures **749,140 → 749,052 raw bytes** and
**321,466 → 321,397 gzip bytes**. The 550,000-byte target remains open. All 63
initial paired serialized results and all 35 longer paired results are byte-identical;
the sample worlds and simulations also restore to the same world hash.

The full game workspace passes 537 tests (11 ignored), build, all-target Clippy
and formatting; caps and boot pass too. Beacons passes web and Linux proofs with
its existing pins, plus Linux Save-every-tick and FreshGame. Fifteen app-save
comparisons match the previous implementation and each other byte for byte.
The normal web proof's module matches the measured artifact. All recorded proof
processes exited; the browser's full descendant inventory timed out, so a separate
PID audit confirmed every recorded browser/carrier PID was absent. Validation
used a private Git index and changed no staged content.

Evidence: `/Users/ccheever/projects/.exact-game-verification/save-framing/`.
