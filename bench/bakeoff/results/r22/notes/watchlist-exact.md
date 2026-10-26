# Watchlist build notes

Finished, built, installed, and launched on simulator `C2188CC6-FBE9-409A-92A8-ABD89AC2C432` with bundle ID `dev.bakeoff.watchlist.b387854`.

## Implemented

- All 260 supplied tickers, symbol/company search, symbol ascending and price/change descending sorts.
- Twenty distinct randomly selected prices move each second, within ±1% of their previous values. Full precision is retained internally; currency is displayed to two decimals.
- Favorites, sort, and the All/My List selection survive relaunch.
- Native navigation, search, segmented selection, sort menu, haptics, empty states, adaptive gain/loss colors, and live detail charts.
- The original dataset is imported into the data module and an unchanged copy ships at `assets/tickers.json`.

## Verification

- `bun exact.mjs verify`: five tests pass; light/dark, relaunch, and largest-text checks clean.
- `EXACT_SIM=C2188CC6-FBE9-409A-92A8-ABD89AC2C432 bun exact.mjs verify ios`: five tests pass; light/dark and all seven largest-text screens clean; persisted settings verified on relaunch.
- `bun test ./.exact/simulation.test.ts`: three tests pass, checking 260 unique symbols, 20 distinct updates per second, the ±1% bound, positive finite prices, deterministic replay, and sixty-point history for every ticker.
- Real simulator touches exercised scrolling. A separate held-contact check confirmed the feed counter advanced during a 1.5-second real finger hold.
- Final `bun exact.mjs ios --run` succeeded and launched the normal installed app.

## What was hard / looked up

Exact's list-detail pattern, task timers, seeded randomness, persistence, resource identity (`with`), SVG paths, semantic colors, segmented controls, and simulator driving required reference lookups.

Web checks missed native 42-point tap targets and large-text collisions. Minimum 44-point navigation targets, wrapping whole rows, and placing controls inside the populated list fixed those findings. Empty states remain outside the list. System green was too faint on white, so gain/loss colors have separate accessible light and dark values.

Building chart strings for every ticker and repeatedly formatting static values wasted work. Charts are now generated only for the displayed detail, static formatting happens once, and sorting/filtering happen in the data module. Unchanged list quotes retain identical values between ticks. Favorites and navigation use sibling controls because a button nested in a link also navigated on the web.

## Assumptions

Prices are displayed as US dollars; volume is the supplied static volume. Symbol sorting is A–Z, while numeric sorts are highest first. Chart history starts with the supplied price and warms up to sixty actual one-second samples. The simulated session restarts on launch; the requested preferences persist.

## Remaining limitation

No requested functional feature is omitted. Exact's final native scrolling benchmark still reports six timing findings: 10.9 ms/s hitch ratio, 37.8 ms longest frame, 12.1 ms main-thread p95, three frames of touch lag, four deceleration offset jumps, and a 1.11-second status-bar scroll-to-top. Layout, touch-target, contrast, and feature checks pass. These remaining host/list performance findings are recorded in `.exact/verify/ios-feel/feel.txt` and `feel.json`; the app uses simple text/symbol rows, without per-scroll state handlers, shadows, or blur. I did not change the pinned toolkit.

## What would make this faster

A ready-made live financial-list pattern, clearer guidance about resource identity for transient filters, consistent nested-control behavior across hosts, and faster/lighter native list rendering would help. Native large-text verification early in the loop is essential; web-only verification did not expose the same layout issues. The first Rust/Swift build took several minutes; later builds and plan updates were much faster.
