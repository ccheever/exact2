# Watchlist build notes

Completed and installed as `dev.bakeoff.watchlist.b32b5d8` on simulator `EF198926-1958-489D-A7A4-9196E454DD3A`. The final app was launched with `bun exact.mjs ios --run` using `EXACT_SIM`.

All requested features are implemented. The 260 original records remain in `data/tickers.json`, are imported into the app’s data module, and also ship as the unmodified raw file `assets/tickers.json`.

## Decisions

- Symbol sorts A–Z; price and percentage change sort highest first, with symbol as a stable tie breaker.
- Each second selects 20 distinct random tickers. Prices are rounded to cents and bounded to ±1% of the preceding price. Percentage change always uses the original open.
- Every ticker records one price sample per second, retaining at most 60 samples. History warms up from the initial supplied quote and resets on a new launch; the detail screen explains this.
- Favorites and sort persist, as do the current list selection and search query. Simulated prices restart from the supplied dataset on relaunch.

## What was hard and what I looked up

- Looked up Exact’s list-detail pattern, native search and picker controls, timers, persisted state, seeded randomness, SVG charts, bundled assets, and simulator driving.
- Native Dynamic Type exposed overlap between fixed controls and the scrolling list even after web checks passed. Moving controls into a self-sizing first list section fixed this without reducing text size. Ticker links have a minimum 44-point touch target.
- The default system green was too light for small text on white. Added adaptive gain/loss colors with sufficient contrast.
- Adding assets during the initial background native build invalidated its packaged-input receipt. Rebuilt after finishing the assets.

## Validation

- `bun exact.mjs verify`: clean; all 6 behavior tests passed, including search, sorting, favorites, relaunch persistence, live detail/history, and unknown ticker navigation.
- `EXACT_SIM=… bun exact.mjs verify ios`: clean; all 6 tests passed, light/dark checks passed, and nothing spilled at the largest accessibility text size.
- The iOS real-touch scrolling check found nothing for the app to fix. The machine was heavily loaded, so the toolkit did not judge timing; scrolling physics passed. It reported its own status-bar scroll-to-top timing limitation.
- A separate simulation check covered 3 seeds × 125 seconds across all 260 tickers: price bounds, update counts, percent changes, stable open/volume, rolling histories, and missing symbols passed.
- Real touches opened native search with the software keyboard visible, selected a result, navigated to its detail, and starred it. Light/dark keyboard-layout checks passed.

Nothing remains unfinished in the specification.

## What would have made this faster

A tested Exact example combining a frequently updating list, embedded row buttons, a live chart, and accessibility-size layouts would help. Earlier native large-text checks and a clearer background-build input snapshot indicator would also shorten iteration.
