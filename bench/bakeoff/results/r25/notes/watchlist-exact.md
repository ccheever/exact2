# Watchlist build notes

Completed and installed as **Watchlist**, bundle ID `dev.bakeoff.watchlist.bd05784`, on simulator `C2188CC6-FBE9-409A-92A8-ABD89AC2C432`. The final command was `bun exact.mjs ios --run --sim C2188CC6-FBE9-409A-92A8-ABD89AC2C432`.

## Implemented

- All 260 supplied tickers, with company names, prices and percent changes from open.
- Every second, 20 distinct symbols are selected for random moves of at most 1%, including cent-rounding safeguards. Stable symbol keys preserve row identity during live updates.
- Search by symbol or company, All/My List segmentation, stars, and symbol/price/change sorting with ascending or descending direction.
- Persistent stars and sort preferences.
- Native detail navigation with live price, change, opening price, volume, and an accessible line chart retaining the most recent 60 one-second samples.
- Adaptive green/red colors, native search and menus, minimum 44-point row links, and layouts that wrap at maximum Dynamic Type.
- The original JSON is shipped unchanged as `assets/tickers.json` and imported into the data module. `data/tickers.json` remains unchanged.

## Decisions and assumptions

- Prices and chart history restart from the supplied prices on relaunch; stars and sort persist as specified.
- History is collected from actual simulation samples, including unchanged prices each second. During the first minute the detail shows how many samples have been collected rather than inventing prior history.
- Symbol defaults to ascending; price and percent change default to descending. The sort menu can reverse either order.

## What was hard / looked up

- Read Exact's start guide and list-detail pattern, then looked up its timer, deterministic randomness, persistence, search, segmented-control, SVG, adaptive-color and iOS-driving APIs.
- Exact requires literal sort directions and scalar/indexed test assertions rather than conditional directions and nonempty array assertions.
- The default system green was insufficiently legible on white. Adaptive market color tokens fixed contrast.
- Native search collapses navigation chrome. Filters outside the list could overlap cells, so populated lists now own their filter row; empty states keep their filters and center the recovery action.
- Nested links needed explicit flex layout for trailing chevrons and a 44-point minimum height. Large text also required wrapping the whole ticker row.
- Adding the icon during the initial background build invalidated its asset receipt; rebuilding reused the compiled artifacts.

## Validation

- `bun exact.mjs verify`: clean; 6 feature tests passed, light/dark and maximum-text layouts clean.
- `bun exact.mjs verify ios --sim C2188CC6-FBE9-409A-92A8-ABD89AC2C432`: clean; 6 tests passed, light/dark and maximum Dynamic Type clean, relaunch checked.
- Real-touch iPhone scrolling: no app issues, no observed hitch time or dropped lift frames in the final run. Exact declined comparative timing judgment because the machine was busy; it noted its own status-bar scroll-to-top timing separately.
- `bun test ./tests/simulation.test.ts`: 2 tests passed, covering 180 simulated seconds, update limits, immutable opening/volume data, percentage calculations, history bounds, request ordering, flat charts and unknown symbols.
- `bun exact.mjs contract types --check app.contract`: current.
- Final native build installed and launched successfully on the assigned simulator.

## Unfinished

None of the requested features remain unfinished.

## What would have made this faster

A stock/live-list starter with deterministic simulation and chart history, a documented native filter-row composition that accounts for search-bar collapse, and an earlier check of nested link touch-target sizes would have reduced iteration. A clearer warning about changing assets during a background build would also help.
