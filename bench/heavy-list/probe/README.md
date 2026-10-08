# The scroll probe

An injected (or linked) dylib that drives the app's one large `UIScrollView` and records every frame. It began as
the easy list benchmark's probe (2026-09-25) and was extended for this benchmark, the extra-heavy feed's nested
lists and the features benchmark; any app with exactly one tall scroll view can be measured with it.

| File | What it is |
|---|---|
| `probe.m`, `build.sh` | the probe; `build.sh` writes `probe-sim.dylib` and `probe-ios.dylib` (`@rpath/probe.dylib`) beside itself, from any directory |
| `resign.sh` | sets a bundle id, embeds `probe-ios.dylib` and a profile, signs for a device |
| `devrun.sh` | one run on a device (`BENCH_DEVICE`), result copied back from the app's Documents |
| `run.sh` | one run on a simulator (`BENCH_SIM`) |
| `linkprobe.py`, `makecold.sh`, `devrun-cold.sh` | cold-start copies: the probe linked by a load command instead of inserted |
| `devclean.sh`, `crashcheck.sh` | terminate leftover bench apps; collect bench crash logs |
| `analyze.py`, `agg.py`, `summarize.py` | per file; per series per speed; the per-app table |
| `tptrace.sh`, `tpagg.py`, `tpcallee.py`, `tpcaller.py` | Time Profiler around one run, and three readers of its export |

Another benchmark uses it by path: `bench/heavy-list/probe/build.sh`, then
`bench/heavy-list/probe/resign.sh <App.app> <bundle-id>` and `bench/heavy-list/probe/devrun.sh …`.

## Scenarios (`BENCH_SCENARIO`)

- `fling`: ±1k–24k pt/s, 2 s segments after a 3k warm-up. `ladder`: ±3k/6k/12k/24k/48k/96k, each down then up.
  `jump`: 10 absolute jumps, time to a blank-free viewport. `coldstart`: process start → first display-link frame
  with the list present and blank-free; every frame sampled; `BENCH_DELAY` ignored.
- `rest`: no scrolling, one 10 s segment at the list's launch offset after `BENCH_DELAY`; the apps run their live
  ticks in it. analyze.py and agg.py print it as a speed-0 fling row.
- `innerfling` / `innerkeep` (the extra-heavy feed's nested lists). An inner list is found by shape: a descendant
  of the feed with content width >= 20,000 pt and height < 300 (a strip), or 300–600 pt tall, narrower than the
  feed, content height >= 20,000 (an inbox). `innerfling` places one fully in view and drives the inner list's
  own offset at the ladder speeds (strip first, then inbox); with `BENCH_SAMPLE`, blank bands are columns/rows
  >= 60 pt inside the inner list. `innerkeep` marks an inner offset, flings the feed 3,000 pt and 24,000 pt away
  and back, and records `keep`; its primary result is `keptAnchor` (same content at the same place in the box,
  ±1 pt; see probe.m's header), with raw `kept`/`err` secondary. Every step is a segment; unmeasured ones are
  `warm`, and segments may carry `dur`.
- `still` (the features benchmark): no scroll view; a 3 s warm-up then one 10 s measured segment.

## What a result holds

- Per segment (`segStats`) and whole run (`run`): main run-loop busy ms (AfterWaiting→BeforeWaiting observer),
  process CPU (getrusage), main-thread CPU (thread_info), travelled pt, footprint. `busyFrameMs` is busy ms
  between consecutive ticks. `mem` is phys_footprint at load/start/peak/end plus the kernel's lifetime peak.
  `live` says `BENCH_LIVE` was set. `BenchBuild` (from `resign.sh`'s stamp) names the build.
- Blank bands are measured over the scroll view's bounds minus `adjustedContentInset` top/bottom, from a layer
  render when `BENCH_RENDER=layer` and `BENCH_SAMPLE` is non-zero. Coldstart ignores a leading band < 120 pt when
  the list is at its top (its own top margin) and records it as `leadingGapPt`.
- Thermal state and Low Power Mode (since 2026-09-30, probe sha1 `6b10a15abb37`): top-level `thermalStart`,
  `lowPowerStart` (when measuring begins), `thermalEnd`, `lowPowerEnd` (when the result is written),
  `thermalChanges` (`[ms since process start, state]` per change notification), and `thermal`, `lowPower` in each
  `segStats` entry. States: 0 nominal, 1 fair, 2 serious, 3 critical. A throttled iPhone is otherwise
  indistinguishable from a regression; summarize.py prints the worst state per app and scenario. On an iPad the
  observer cost nothing measurable (cold start median 164 ms with and without it).
- Ladder breaking point: the first speed with fps < 110 (scaled to the screen: 55 at 60 Hz) or a blank frame.

## Metal under the layer sampler

`renderInContext` does not draw a `CAMetalLayer`'s presented content on iOS 17. The probe swizzles
`-[CAMetalLayer nextDrawable]` (and `-[CAMetalDisplayLinkUpdate drawable]`), remembers every layer that vended a
drawable, and after a `BENCH_RENDER=layer` render paints each such visible layer as a 2×2 dark/light checker, so
it reads as ink. A subclass that overrides `-nextDrawable` is hooked lazily when the sampler's layer walk (at most
once a second) meets it. `BENCH_METALDBG=N` dumps Metal and MapKit layers every Nth sample. `BENCH_JUMPDUMP=<dir>`
writes, for each timed-out jump, the layer sample and a `drawViewHierarchyInRect` image of the same rect, both
scored (`layerBlank`, `screenBlank`).

## Pre-main inflation in launch numbers (2026-09-29)

From about 04:00 on 2026-09-29 until a fix that evening, the probe's constructor registered an image-load
callback that realized every Objective-C class of every loaded image to find `CAMetalLayer` subclasses. It ran before `main`, cost
~317 ms on exact2, and scaled with the frameworks an app links. Every cold-start and first-ink number taken with
a probe built in that window is inflated and not comparable across apps; scroll metrics start 5–15 s after
launch and are not affected. The fix dropped the scan.

Inserting the probe with `DYLD_INSERT_LIBRARIES` also disables an app's prebuilt launch closure. For that reason
cold start is measured with the probe linked (`makecold.sh`, `devrun-cold.sh`, `../cold-series.sh`). The
`coldstart` scenario of `../series.sh` still runs with the probe inserted.
