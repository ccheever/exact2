# Exact2 gaps met by the crypto benchmark

A record, as found on 2026-09-26 against origin/main 0fd40388; the line numbers are that commit's. Since then:
gap 1's fix is on main (`GpuIOS.swift` `onScreen`); gap 2 stands (`NodePoolIOS.swift` still refuses to pool a view
with `metal`); for gap 3, colour transitions and CSS `@keyframes` / `animation` have landed (LLP 1055,
1055.000 D6), and the SVG app (`exact-crypto-svg/`) flashes with a keyed `animation`, while the GPU app keeps the
opacity workaround it was measured with.

## 1. Mounted canvases outside the viewport rendered every frame (fixed: `perf/crypto-gpu` 4776fa4a)

LLP 1009 D4 says "a canvas renders when the host judges it on screen". On iOS, the only
judgement was whether the app is active and in a window:

- `host/apple/Sources/ExactKit/IOS/GpuIOS.swift:425` `visible`
- `host/apple/Sources/ExactKit/CanvasSeams.swift:271` `live()`, which checks only `view.window != nil`

A virtualized list keeps one viewport of rows mounted on each side of the screen, and up to
two more in the direction of travel (`runner/src/instance/collection/mod.rs:40-50`). So
every mounted row's canvas rendered and presented on every tick, because the tick loop's
render guard (`GpuIOS.swift:482` on main) had no position test. In this app the sparkline
pulse makes every canvas want every frame.

This cost performance and also broke the spec. An offscreen row played its 600 ms draw-in
where no one could see it, so rows scrolled into view already drawn.

**Fix (18 lines, `GpuIOS.swift`, landed):** `onScreen(_:)` intersects the Metal view's window rect
with the window and every `clipsToBounds` ancestor, including the scroll view's visible
bounds. The tick skips a canvas that is not on screen. The canvas keeps its `wants` flag and
its dirty inputs, so it renders on the first frame it is seen.

| iPad simulator, at rest, 60 Hz | before | after |
|---|---|---|
| renders/s (`EXACT_FPS=1`) | 2,460 (41 canvases per frame) | 1,260 (21, the ones on screen) |
| main-thread busy per frame | 9.1 ms | 4.7 ms |
| CPU | 1,338 ms/s | 665 ms/s |

## 2. Rows holding a canvas are never recycled (not fixed)

`host/apple/Sources/ExactKit/IOS/NodePoolIOS.swift:151` refuses to pool any view whose
`metal != nil`. So every row that scrolls in builds these from scratch:

- a new `MetalView` and `CAMetalLayer`;
- `gpu_create`, which configures a wgpu surface on that layer (`gpu/src/lib.rs:533-570`);
- a first `bind`.

Every row that scrolls out calls `gpu_destroy`. At 24,000 pt/s that is about 375 rows a
second.

The surface keeps its pipeline in a thread-local shared across instances
(`exact-gpu/gpu/src/lib.rs`, `SHARED`), so pipeline compilation is not the cost. Surface
configuration and layer churn are. On the iPad the GPU app holds about 115 fps at 12k pt/s
and falls to about 63 fps at 24k.

On the simulator, footprint jumped 65 → 323 MB during the two 24k segments. On the iPad the
smoke fling peaked at 157 MB and ended at 140 MB.

On the iPad ladder (round 1 of series `ipad-r1`, `gpu-ladder-t-1.json`), footprint holds at 126–152 MB
through 24k pt/s. It then climbs 431 → 614 → 716 → 824 MB over the 48k and 96k segments and does not come
back by the run's end. Nothing else in the app grows with speed, so canvas churn is retaining memory
(likely Metal layers, surfaces or drawables that are released late). This is not yet attributed; a
`BENCH_VM=1` ladder would split it by VM tag.

The fix is not small: it needs pooling of canvas rows, moving a wgpu surface between
instances, or one shared layer for the list. So it is recorded here, not built.

## 3. No colour transition, so the price flash is a workaround (not a canvas gap; not fixed)

`transition` animates only `all | translate | scale | rotate | opacity | height`
(`motion/src/property.rs:17`, parsed in `motion/src/parse.rs:66-80`). `color` is refused,
and `all` never animates colour. `@keyframes`/`animation` are out of motion v1
(`llp/1003-motion-v1.spec.md:239`).

The flash is therefore built from two pieces:

- a second copy of the price, in green or red on white, keyed by the flash's sequence number
  and created at opacity 1;
- the data source's next tick switches it off, and `transition="opacity 400ms ease"` fades it.

**Deviation from SPEC:** the flash holds full colour for one tick (100 ms) before its
400 ms fade. Two extra text nodes per flashing row also exist while the flash lasts.

The Exact2 SVG app's agent is adding CSS `@keyframes`. When that lands, a keyed
`animation` is the direct form.

## 4. A canvas cannot paint outside its box (by design; worked around)

The Metal view is exactly the node's bounds (`IOS/PresenterIOS.swift:771`). The pulse ring
reaches 9 pt past the 96 × 32 chart, so the canvas is 114 × 50 at −9 pt margins, as the
Skia app does. This is recorded in SPEC Decisions and is not counted as a gap against the web
(a `<canvas>` does not overflow either).

## 5. Launch environment needs a data-source round trip (known, heavy-bench gap G9)

`BENCH_LIVE`, `BENCH_SCENARIO` and `BENCH_FREEZE` are read by a `config()` mutation on the
first 100 ms timer tick. For the first ~100 ms, `freeze` is false and "Live: off" shows.
Under `BENCH_FREEZE=1`, rows mounted in that window start a draw-in, then snap to the
settled picture.

## 6. Not implemented in the GPU app

`BENCH_START_INDEX` (optional in SPEC) is not implemented: there is no launch-time initial
scroll position. Parity shots are top-of-list only.
