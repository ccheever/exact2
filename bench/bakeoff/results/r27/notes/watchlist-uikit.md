# Watchlist

Finished as a programmatic UIKit iPhone app, built and installed as `dev.bakeoff.watchlist.bda2244` on simulator `EEFC0029-1010-45E4-8242-42CF67E16C96`.

## Implementation

- The original `data/tickers.json` is copied unchanged into `Resources/tickers.json` and ships in the bundle. All 260 symbols appear in All.
- A one-second timer in the common run-loop modes changes 20 distinct random tickers by at most ±1%. Visible cells update directly rather than reloading the table during scrolling. Price/change reordering is deferred during a drag or deceleration, then reconciled while retaining the visible anchor.
- Search matches symbol or company name, ignores case, and trims surrounding whitespace. Symbol sorts A–Z; price and change sort highest first with symbol tie-breaks.
- Stars and the sort choice use UserDefaults. Empty lists and searches have explanatory states.
- Detail charts retain 60 one-second samples per symbol. They begin with the supplied starting price and collect real simulation samples rather than inventing historical values. The timer pauses when the app is inactive. Prices/history reset on a fresh launch; stars/sort remain.
- Prices are presented in USD. These assumptions are explained in the app's information sheet.
- Dynamic Type, including the largest accessibility size, uses stacked row/detail layouts. Semantic colors support both appearances. Search has keyboard insets and drag dismissal. VoiceOver exposes separate star and quote actions and a chart summary.

## What was hard

Keeping live updates smooth during scrolling and preventing a live sort from moving the user's reading position required separating visible quote updates from ordering changes. Self-sizing table headers and readable accessibility layouts also needed simulator inspection, especially when the market summary becomes multiline.

## Verification

Built and launched repeatedly using the provided build script and only the assigned simulator. Simulator checks passed for bundle decoding, unique identities, all 260 rows, empty My List, case-insensitive symbol/company search, whitespace and no-result queries, all sort modes, star removal, 20 distinct changes per tick, ±1% movement bounds, change from open, 60-sample history, and timer operation in the scrolling run-loop mode. Controller checks exercised the segment, search/cancel, sort refresh, actual star button, navigation, and live detail chart. A separate two-process launch test confirmed that stars and the selected sort survive termination and relaunch, then restored the original preferences.

Inspected screenshots of the main list, live detail in dark mode, maximum-accessibility-size rows and detail, keyboard search, and empty My List. Returned the simulator to light appearance and standard text size for the finished app.

The verification harness is in `Sources/AppChecks.swift`; it runs only with explicit `--self-test` or `--persistence write` / `--persistence read` launch arguments. Preview arguments provide repeatable screenshots without adding controls to the app.

## Lookups, assumptions, and limitations

No web research or additional dependencies were needed. Used the skeleton README, UIKit/Foundation, the supplied compiler, and simctl. The spec did not define sort direction, currency, initial chart history, or background behavior; the choices above make those explicit. No requested feature remains unfinished.

A ready-made simulator UI-driving harness would have made touch/scroll and accessibility regression testing faster than building app-level checks and repeatable preview states.
