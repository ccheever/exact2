# Watchlist build notes

Finished and installed on simulator `EF198926-1958-489D-A7A4-9196E454DD3A`.

- App name: Watchlist
- Bundle ID: `dev.bakeoff.watchlist.b6d918a`
- Final command: `EXACT_SIM=EF198926-1958-489D-A7A4-9196E454DD3A bun exact.mjs ios --run`
- All six requested features are implemented. No known unfinished app features.
- The original 260-entry `data/tickers.json` is imported into the data module. An identical copy ships in the app bundle as `assets/tickers.json`.

## What was hard

The live model needs stable ticker identities, exactly twenty distinct selections per second, a bounded history, and correctly signed changes from each opening price. A seeded random stream makes this reproducible. Prices use integer-cent movements whose limits are rounded down, keeping every movement within one percent even after rounding. Each ticker gets one history sample per second, including seconds when its price is unchanged. Histories retain sixty samples, and flat or single-sample charts draw without dividing by zero.

Native accessibility needed iteration. Fixed pixel flex bases crowded quote rows at the largest text size; rem bases let the price stack below the company. Ticker links needed explicit minimum touch heights. A segmented control fixed above the table overlapped a row at the largest text size; moving it into the table resolved that. Empty states retain a separate scope control and use the platform's centered presentation. Adaptive green/red tokens give readable gains and losses in both schemes.

## Lookups and assumptions

I used Exact's start guide, list-detail starter, and recipes for persistence, timers, seeded randomness, native search, menus, SVG, typography, colors, asset bundling, and simulator driving. No web research or external dependencies were needed.

The data does not name a currency, so the app labels prices as USD. Symbol sorting starts A–Z; price and change start highest first. The menu can reverse any sort. Favorites, sort key, direction, and selected segment survive relaunch. Simulated prices and chart history restart from the supplied prices on a new app session; the chart builds up to sixty real samples rather than inventing earlier history.

## Validation and toolkit limitations

- Final iOS functional suite: **6 passed, 0 failed**. Tests cover all 260 tickers, symbol/company search, empty results, all sorts and reversal, favorites, relaunch persistence, live updates, trading values, sixty-sample chart retention, and a missing ticker route.
- Final iOS visual checks: no findings across the initial screen, six test endings, and relaunch, in light and dark mode.
- Largest accessibility text size: body 53 pt, eight screens checked, nothing spills.
- Real-touch scrolling: no app-level findings. The machine was heavily loaded, so Exact judged scrolling physics rather than performance timing. It separately reported a toolkit-owned status-bar scroll-to-top timing difference.
- Independent data checks passed for 180 simulated seconds: twenty distinct price moves every second, all within ±1%, positive prices, correct opening-price changes, unchanged open/volume, deterministic replay, flat charts, and capped history.
- Earlier web UI verification also passed with no light/dark or largest-text findings.

Exact's `verify` UI stage reports **clean**, but the supplementary semantic/model checker then exits with status 1: it cannot deliver the test seed, reports unsupported `toLowerCase`, and reports a zero-event divergence at boot. Its runner reports zero findings. This is an unresolved verification-tool limitation; I did not change the toolkit or weaken case-insensitive search to work around it. Native UI tests and the independent simulation checks passed.

## What would have made this faster

A warm native build, a starter that combines a live table with a chart, and an example showing a segmented scope control inside a native table would help. Earlier guidance about literal sort directions and accessible gain/loss colors would save a compilation/check cycle. The semantic checker should distinguish unsupported coverage from an application failure and support seeded resource tests.
