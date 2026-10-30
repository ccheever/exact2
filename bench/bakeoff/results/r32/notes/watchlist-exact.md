# Watchlist — implementation notes

Finished and installed as `dev.bakeoff.watchlist.ba905a4` on simulator `EF198926-1958-489D-A7A4-9196E454DD3A` with `bun exact.mjs ios`.

## What works

- All 260 records from `data/tickers.json` are embedded in the app through its JSON import; no network connection is needed.
- Every second, twenty distinct, randomly selected tickers move by up to ±1%. Stable symbol keys keep the list updating in place.
- Symbol/company search; numeric price/change sorting; ascending/descending ordering; All/My List selection; stars on rows and details.
- Live details show price, change from the supplied open, open, volume, and a rolling line chart with up to sixty one-second samples.
- Favorites, sort key, sort direction, search, and list selection persist across relaunches.
- Native navigation, search, segmented controls, menus, haptics, dark mode, accessible labels, and layouts that accommodate the largest text size.

## What was hard

The main work was fitting changing numeric columns beside company names at large text sizes while keeping separate star and navigation actions. The iOS verifier caught 41-point navigation targets inside otherwise larger rows; explicit 44-point minimum link heights fixed them. A fixed filter above the list overlapped content at the largest text size, so the populated list now contains the filter in its first row. Empty states retain their own filter and recovery action.

System green was too pale for readable small text in light mode. Semantic light/dark market colors now provide stronger contrast. The first compiler lookup also needed permission to write the Exact toolkit's compiler cache outside the workspace.

## Lookups and assumptions

Used Exact's start guide, list-detail pattern, and focused documentation for timers, seeded randomness, resources, persistence, search, menus, numeric readouts, SVG, themes, and iOS driving. No external market APIs were used.

History begins with the supplied price at launch and collects one observation per second, including unchanged prices. It retains the newest sixty observations. Prices/history restart on relaunch; favorites and ordering persist as specified. Simulation uses full precision and formats prices and changes to two decimals. Symbol defaults to ascending; price and change default to descending, with a menu option to reverse either order.

## Validation

- `bun exact.mjs verify`: clean; five functional tests pass; light/dark, relaunch, narrow/wide, and largest-text checks pass.
- `EXACT_SIM=EF198926-1958-489D-A7A4-9196E454DD3A bun exact.mjs verify ios`: clean; five tests pass; light/dark, relaunch, and largest-text checks pass.
- Simulator scrolling verification used about 33 seconds of real touches and reported nothing for the app to fix. The machine was heavily loaded, so Exact did not judge overall timing. It identified a toolkit-owned status-bar scroll-to-top difference.
- Additional simulation checks verified all 260 original records, twenty unique bounded changes per tick, percentage calculations, finite chart coordinates, the sixty-sample limit, matching list/detail prices, and seed reproducibility.
- A simulator detail drive advanced 61 seconds and confirmed a live sixty-sample chart.
- Final `bun exact.mjs ios` completed successfully and installed the production plan on the specified simulator.

## Unfinished work and improvements to the process

No specification features remain unfinished. A tested Exact example combining live prices, separate row actions, filtering, and accessibility text sizes would have shortened layout iteration. A workspace-local compiler cache and faster native verification would also help.
