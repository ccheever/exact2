# Watchlist implementation notes

The finished app is built, installed, and running on simulator `498D6D1F-C21C-4B31-90F4-5D857216E256` as `dev.bakeoff.watchlist.bc1c1e0` (Watchlist).

## Delivered

- All 260 tickers from `data/tickers.json` are imported into the bundled TypeScript data module, so the app works offline and ships the supplied data.
- Every second, twenty distinct randomly selected tickers move by at most 1%. Prices use whole cents, and rounding cannot push a move beyond that limit.
- Symbol, price, and percentage-change sorting, ascending/descending direction, and case-insensitive symbol/company search.
- Stars, All/My List selection, useful empty states, and native pushed ticker details.
- Live details show price, percentage change since open, open, volume, and a rolling line chart with up to sixty one-second samples.
- Favorites, sort key, and sort direction survive relaunch. Search and list selection are also restored.

## Decisions

- Prices are shown in USD; the source does not provide a currency.
- Symbol sorting starts ascending. Price and change start descending, with a menu action to reverse the direction.
- Each launch begins a fresh simulated session at the supplied prices. History starts with the initial price, fills once per second, and keeps the newest sixty samples, including unchanged prices. Only settings and favorites need to persist.
- The filter and sort controls occupy the first list row and scroll with the list. This avoids native large-text overlap from controls fixed above the list.

## What was hard and what I looked up

- Exact's native navigation, search, segmented controls, menus, persistent state, timers, seeded randomness, SVG charts, numeric formatting, and simulator-driving recipes.
- `formatDecimal` expects integer smallest units. Passing a fractional percentage produced an empty number; formatting rounded hundredths fixed it, and an explicit percentage assertion now covers it.
- The native search controller hides navigation-bar items while active. Putting sorting in the list controls keeps it available during search.
- Native list links measured 42 points high inside taller cells. An explicit 44-point minimum fixed their touch targets.
- Large text needed wrapping rows and a shorter Back label. Invalid ticker routes now show a short fallback title and a recovery action.
- An initial background prebuild overlapped changing the starter's app identity and failed with an app-ID mismatch. Rebuilding after the manifest and data module matched resolved it.

## Validation

- `bun exact.mjs verify`: clean; 7 tests passed, with light/dark, reload, and largest-text checks.
- `bun exact.mjs verify ios --sim 498D6D1F-C21C-4B31-90F4-5D857216E256`: clean; 7 tests passed, no app layout findings, including the largest accessibility text size.
- Simulator scrolling verification found nothing for the app to fix. The machine was busy, so Exact declined to judge overall timing; it separately noted its own status-bar scroll-to-top duration, outside app control.
- A real-touch simulator drive exercised the UIKit sort menu, stars, ticker navigation, and live detail updates. Light and dark checks passed.
- Final installation and launch: `bun exact.mjs ios --run --sim 498D6D1F-C21C-4B31-90F4-5D857216E256`.

## Unfinished work

None of the requested features remain unfinished.

## What would have made this faster

- A documented native pattern combining search, a segmented list filter, sorting, and live updates, including maximum-text behavior.
- A clearer integer-only description of `formatDecimal` in the money recipe.
- Earlier native diagnostics for hidden navigation-bar controls and minimum link targets.
- An app-identity preflight before starting background native builds.
