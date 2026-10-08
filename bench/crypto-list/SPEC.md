# Crypto list benchmark — spec

A coin list with live prices and a small live chart per row, the "standard crypto exchange"
case (a Skia-embed list in React Native). Every app renders the same rows from the same data,
driven by the same probe (`../heavy-list/probe`), on a 120 Hz iPad Pro (M1, 12.9") and iPhone 13 Pro Max.

## Apps

| App | Bundle id | How the chart and pulse are drawn |
|---|---|---|
| SwiftUI | `dev.exact.cryptobench.swiftui` | `List`/`LazyVStack` as SwiftUI normally does it; `Path` shape with `.trim(from:to:)` draw-in, pulse via `.repeatForever` animation |
| Expo | `dev.exact.cryptobench.expo` | React Native + `@shopify/react-native-skia` `Canvas` per row + LegendList — the Skia-embed case |
| Exact2 (SVG) | `dev.exact.cryptobench.svgi` | inline `svg` + `polyline` mapped from the row's series in the view, CSS `stroke-dasharray`/`stroke-dashoffset` draw-in, CSS `@keyframes` pulse — what a web developer writes (`exact-crypto-svg/`) |
| Exact2 (GPU) | `dev.exact.cryptobench.gpu` | a `canvas` node per row (LLP 1009, wgpu) drawing the line and the pulse (`exact-crypto-gpu/`) |
| UIKit | `dev.exact.cryptobench.uikit` | hand-written UIKit (`uikit/`): `UICollectionView` + compositional layout + diffable data source; `CAShapeLayer` line with a `strokeEnd` draw-in, `CAAnimationGroup` pulse, Core Animation opacity flash |

Each is written the most normal way for its stack, not tuned against the others.

## Data (`gen.py`, seed 49975)

- 5,000 coins. Each: `id` (`c0`…), `name` (from syllable lists, e.g. "Voltaris"), `ticker`
  (3–5 capitals), `color` (icon colour, hex), `price` (log-uniform $0.0001–$60,000), `change24h`
  (−18 %…+18 %), `series` (48 prices, a random walk ending at `price`).
- Written to `data/coins.json`; each app embeds or bundles it (no network).

## Row (64 pt tall, 16 pt side padding, 1 px separator)

Left to right:
1. **Icon**: 32 pt circle of `color`, the ticker's first letter centred, white, 15 pt semibold.
2. **Name block** (flexes): name 16 pt semibold; ticker below, 13 pt, secondary grey.
3. **Chart**: 96 × 32 pt, 12 pt left of the price block. The 48-point series as a polyline scaled
   to the box (min→bottom, max→top), 1.5 pt stroke, round joins, green `#16a34a` if
   `change24h ≥ 0` else red `#dc2626`, no fill.
   - **Appear animation:** each time a row comes on screen (mount or reuse), the line draws
     left to right over 600 ms, ease-out (stroke trim 0 → 1).
   - **Pulse:** a 3 pt dot at the last point, in the line colour, plus a ring around it that
     "breathes": radius 3 → 9 pt and opacity 0.5 → 0, 1,200 ms, ease-out, repeating forever.
     It starts when the draw-in ends.
4. **Price block**, right-aligned, 104 pt wide: the price 16 pt semibold with `tabular-nums`,
   formatted `$61,234.56` (≥ $1: 2 decimals; < $1: 4 significant digits). Below it, a pill with
   the change `+3.42%`, 12 pt semibold white on green/red, 4 pt radius.

## Live ticks

- Every 100 ms, tick `k` updates 40 coins: coin `(k·7919 + j·104729) mod 5000` for `j` in 0..40.
- Each updated coin:
  - gets `price *= 1 + δ`, with `δ = ((k·31 + j·17) mod 201 − 100) / 10,000` (±1 %);
  - appends the new price to `series` and drops the oldest, so the chart scrolls;
  - updates `change24h += δ·100`.
- **The price flashes** on update: its colour turns green (up) or red (down) at once, then goes back
  to the normal text colour over 400 ms (a colour transition).
- A chart that updates while on screen redraws immediately (no draw-in replay); the pulse moves
  to the new last point.
- Ticks run in the live scenarios below, and at rest in the `rest` scenario. The fling
  scenarios run with ticks off unless `BENCH_LIVE=1`, as in the heavy bench.

## Scenarios (the probe's, plus one)

- `fling`, `ladder`, `jump`, `coldstart` — as in the heavy bench (same speeds and segments).
- `fling` with `BENCH_LIVE=1` — ticks on while scrolling.
- **`rest`** (new): no scrolling, ticks on, 10 s measured after the delay. It reports fps, late frames,
  CPU ms/s (all threads and main), busy ms/frame and memory. This is the "idle but everything is
  animating" cost, the one that drains batteries on real exchange apps.

## Parity

Screenshots of the top of the list and after a jump, with ticks off and animations settled
(`BENCH_FREEZE=1`: draw-in complete, pulse at a fixed phase), should match across apps as closely
as the platforms allow. Chrome is the oracle for the Exact2 SVG version (the same Contract on the
web); the others are compared by eye and by row geometry.

## Decisions

Ambiguities resolved by the builder (2026-09-26), the most ordinary choice each time. Every app follows these.

- **Expo: LegendList 3.4.0 (`@legendapp/list`) instead of FlashList, per Charlie 2026-09-27.** Written as its
  README and performance guide say: `keyExtractor = id`, `recycleItems`, `getFixedItemSize` 64 (the row pitch is
  exactly 64 pt), the default `maintainVisibleContentPosition` (scroll-time stabilisation; no data anchoring: a
  tick changes values, never inserts or reorders). The FlashList 2.0.2 build (series `ipad-r2`, 2026-09-26) is not
  kept here: `expo/package.json` is the LegendList app.

**Data**
- `gen.py` writes `data/coins.json` = `{"version":1,"coins":[…]}`; values rounded to 8 significant digits.
- The series is a log-space random walk pinned at both ends (a Brownian bridge) from `price / (1 + change24h/100)`
  to `price`, so a chart's direction agrees with its colour. `series[47] == price`.
- Tickers are unique, 3–5 capitals, first letter = the name's first letter. Icon colours: HSL hue random, S 0.6, L 0.45.

**Screen**
- One vertical list of the 5,000 coins in data order (`c0` at top), plain style, white background, light mode.
- A top bar above the list (not in it), as in the heavy bench: "Markets" 17 pt semibold #000 left, "Live: on"/"Live: off"
  13 pt #8E8E93 right, padding 10 pt vertical / 16 pt horizontal, a 0.5 pt #E5E5EA hairline below it.

**Row geometry** (row pitch exactly 64 pt)
- The 1 px separator is full width (no inset), #E5E5EA, the bottom pixel of each row's 64 pt.
- Horizontal: 16 pad · icon 32 · gap 12 · name block (flex, min width 0) · gap 12 · chart 96 · gap 12 · price block 104 · 16 pad.
  Everything is vertically centred in the row.
- Name block: name 16 pt semibold #000, ticker 13 pt regular #8E8E93, 2 pt between them, single line each, tail ellipsis.
- Price block: right-aligned column, price 16 pt semibold #000 `tabular-nums`, 4 pt gap, then the pill.
- Pill: 12 pt semibold white, background `#16a34a` (change ≥ 0) / `#dc2626`, padding 2 pt vertical / 6 pt horizontal,
  4 pt radius, hugs its text (right-aligned). Text `+3.42%` / `-3.42%` (ASCII hyphen-minus; `+0.00%` for zero), 2 decimals.
- Price text: ≥ $1 → `$61,234.56` (grouping commas, 2 decimals); < $1 → 4 significant digits, no grouping
  (`$0.1234`, `$0.0009697`, trailing zeros kept: `$0.1200`, never exponent notation). Rounding is round-half-even as `printf`/`toFixed`/`toPrecision` give; differences at exact
  halves are accepted.
- Icon letter: the ticker's first letter, 15 pt semibold white, centred in the 32 pt circle.

**Chart**
- Points: `x_i = 96 · i / 47`, `y_i = 32 − 32 · (v_i − min)/(max − min)` (y = 16 for a flat series), in the 96 × 32 box.
  Round joins and round caps. The stroke may overhang the box by half its width.
- Pulse dot: a filled circle of **radius 3 pt** (so the ring starts at the dot's edge) at the last point.
- Ring: a **filled disc** in the line colour behind the dot, radius 3 → 9 pt and opacity 0.5 → 0 over 1,200 ms,
  CSS `ease-out` (cubic-bezier(0, 0, 0.58, 1)), repeating. The dot and the ring appear when the draw-in ends.
- Nothing is clipped to the chart box: the ring may extend up to 9 pt past it (the 12 pt gaps absorb it). An implementation
  that draws into a canvas makes the canvas the chart box outset by 9 pt on every side (114 × 50 pt, at −9 pt margins)
  and draws the line into the inner 96 × 32.
- Draw-in: stroke trim 0 → 1 by path length over 600 ms, CSS `ease-out`, on mount and on reuse (each time a row comes
  on screen). A tick that changes an on-screen chart redraws it at full length; the pulse keeps its phase and moves
  to the new last point.

**Live ticks**
- Ticks are on iff `BENCH_LIVE=1` or `BENCH_SCENARIO=rest`, and `BENCH_FREEZE` is not `1`. Read from the launch
  environment at startup, as the heavy apps read `BENCH_LIVE`.
- A repeating 100 ms timer from launch; the first tick is `k = 1`. Tick `k` updates `j = 0..39` in order.
- `δ = 0` counts as up. The flash: the price colour becomes `#16a34a` (δ ≥ 0) or `#dc2626` at once, then transitions back
  to #000 over 400 ms with CSS `ease` (the default `transition-timing-function`). A new tick on a flashing coin restarts it.
- `change24h += δ·100` (percentage points); the pill and the line colour follow its sign.
- The price text is always formatted from the current `price`.

**Freeze** (`BENCH_FREEZE=1`, parity screenshots): ticks off, the draw-in complete, the ring drawn at radius 6 pt and
opacity 0.25 (static).

**UIKit** (`uikit/`, added 2026-09-29: a fourth competitor, written as a performance-minded iOS engineer
would, doing the same work as the others)
- *List*: `UICollectionView` with a `UICollectionViewCompositionalLayout` of one section whose items are
  `.absolute(64)` tall (the pitch is exactly 64 pt, so no self-sizing pass), a `UICollectionViewDiffableDataSource<Int, Int>`
  over the 5,000 coin indices (one `applySnapshotUsingReloadData` at launch; identities never change), a
  `CellRegistration`, cell reuse and UIKit's default cell prefetching. One `UIScrollView` (the collection view).
- *Row*: hand laid-out in `layoutSubviews` (frames, no Auto Layout in the cell): the icon a 32 pt view with
  `cornerRadius` 16 and a centred `UILabel`; `UILabel`s for name, ticker, price
  (`monospacedDigitSystemFont` 16 semibold = `tabular-nums`) and the pill text; the pill a view with `cornerRadius` 4;
  the separator a 1-px `CALayer` at the row's bottom pixel, full width. Line heights are rounded up to whole points
  (SwiftUI's placement differs by < 1 pt; see Parity).
- *Chart*: a `CAShapeLayer` polyline (1.5 pt, round joins/caps). Draw-in: a `strokeEnd` 0 → 1 `CABasicAnimation`, 600 ms,
  `easeOut` (= CSS ease-out), added in `willDisplay` — every time a row comes on screen (mount or reuse); removed in
  `didEndDisplaying`, as SwiftUI's `onDisappear` resets it. The dot is a radius-3 `CAShapeLayer`, hidden for the 600 ms
  by an opacity animation. The ring is a radius-9 filled `CAShapeLayer` (model opacity 0) with a repeating
  `CAAnimationGroup` of `transform.scale` 1/3 → 1 and `opacity` 0.5 → 0, 1,200 ms `easeOut`, `beginTime` = appearance
  + 600 ms. All of it runs in the render server, as SwiftUI's `repeatForever` does.
- *Ticks*: a 100 ms `Timer` in `.common` mode. Every tick updates the model for all 40 coins (price, series,
  change, a version). A coin whose cell is on screen (`cellForItem(at:)` non-nil) is pushed into that cell directly:
  new texts, new path (implicit actions off, so no draw-in replay; a running draw-in keeps going), new colours, the
  pulse moved to the new last point with its phase kept (the animation is on scale/opacity, not position). This is
  the cheapest idiomatic path; `reconfigureItems` through the snapshot does the same work plus a data-source pass.
  A prefetched (configured, not yet visible) cell whose coin changed is re-applied in `willDisplay` (a version check),
  so no stale value ever shows.
- *Flash*: a second `UILabel` with the same text in the flash colour over the black price, faded by a Core Animation
  `opacity` 1 → 0 animation, 400 ms, CSS `ease` (cubic-bezier(0.25, 0.1, 0.25, 1)); a new tick replaces the running
  animation (restarts it). Glyph colour = black + a·(flash − black), the same interpolation as SwiftUI's
  `flashColor(amount:)`. `UILabel.textColor` is not animatable; a cross-dissolve transition would also fade the text.
- *Env*: as SwiftUI's app (`BENCH_LIVE`, `BENCH_SCENARIO=rest`, `BENCH_FREEZE`: ring static at radius 6 / opacity 0.25,
  no draw-in; `BENCH_START_INDEX`: `scrollToItem(at:, at: .top)` after the first layout).
- *Focus*: the list has nothing UIKit can focus (no selection, no controls), so `allowsFocus = false` and the
  collection view answers `focusItems(in:)` with none. Without it, on the iPad (hardware keyboard attached) UIKit's
  focus system re-mapped every visible cell's subtree about once a second while scrolling: 30–47 ms main-thread
  frames, ~35 late frames/s (Time Profiler: 79% of those frames in
  `-[UIFocusMapSnapshot _capture]` / `_inferredDefaultFocusItemInEnvironment`). exact2's host does the same
  (`FocusSearchIOS.swift`, 2837669f). It removes no content and no work the benchmark asks for.
- *Scene lifecycle*: `UIApplicationSceneManifest` + `@objc(SceneDelegate)` (iOS 27 traps a UIKit app without it).

**Other**
- `BENCH_START_INDEX=n` (optional, as in the heavy bench) scrolls coin `n` to the top at launch, for "after a jump" shots.
- The `rest` scenario leaves the list at the top (offset 0).
- Every app supports all four orientations on iPad (`UISupportedInterfaceOrientations~ipad`), as the heavy apps
  do, so all run in the device's orientation (the bench iPad stands in landscape: 1366 × 1024 pt).

## Cold start measurement (2026-09-29)

Cold start values taken before 2026-09-29 ~20:00 are inflated: the probe's own static initializer (a Metal hook that
scanned every ObjC class) ran pre-main in every app, more so in exact2's larger binary
(`../heavy-list/probe/README.md`). Injecting the probe with `DYLD_INSERT_LIBRARIES` also disables the app's prebuilt
launch closure, so cold start is measured with the probe **linked** into a copy of each app
(`../heavy-list/probe/makecold.sh`, bundle ids `…<app>cold`), launched without insertion (`cold-series.sh`). The
`coldstart` scenario inside `series.sh` still runs with the probe inserted. Scroll fps and CPU from the older
series stand; their memory carries a small fixed probe offset (about +11 MB exact2, +7.5 MB SwiftUI).

## The exact2 SVG app holds the series (2026-09-30)

The first exact2 SVG app (`dev.exact.cryptobench.svg`) formatted every changed coin's 48-point series into a
`points` string in the data source on each tick, off-screen coins included (54 of 117 ms/s CPU at rest on the
iPhone went to that float formatting). The SVG app measured since 2026-09-30, and the one here, is the idiomatic
port (`svgi`): the series held as numbers in the row and mapped to points in the view (LLP 1017.003), as the
SwiftUI and UIKit apps build a row's path only when it shows. The A/B at the same exact2 commit (iPhone 13 Pro
Max, three rounds): rest CPU 92 → 65 ms/s (UIKit 22), late frames 16 → 4, rest footprint 27.2 → 23.5 MB; live
fling unchanged.
