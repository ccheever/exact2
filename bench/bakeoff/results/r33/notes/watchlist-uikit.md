# Watchlist build notes

Completed the UIKit app, built it with `build.sh`, and left it installed and launched on simulator `C2188CC6-FBE9-409A-92A8-ABD89AC2C432`. No other simulator was booted, erased, or used.

## Implementation

- Programmatic app delegate, scene delegate, native navigation, searchable table, segmented All / My List control, and sort menu with both directions.
- All 260 records from `data/tickers.json` are shipped unchanged as `Resources/tickers.json` and verified in the final app bundle.
- Each second, 20 distinct random tickers move by at most ±1%. The timer runs in common run-loop modes, including scroll tracking. Visible cells update directly while scrolling; price/change sorting reorders after scrolling stops.
- Details share the live model and show price, percent and dollar change from open, open, compact and exact volume, and a custom chart of the latest 60 one-second samples.
- Stars, sort field, and direction persist through UserDefaults. Semantic colors, Dynamic Type, accessible labels, keyboard dismissal and avoidance, empty states, and a custom app icon are included.

## Hard parts

Keeping live updates independent from table reordering required separating visible-cell updates from snapshot changes. The first fixed-header layout consumed too much space at the largest text size, so the controls now live in an automatically sized table header that scrolls away. The empty state is a measured table footer, so its action remains reachable at maximum text size. Statistics switch to a vertical layout at accessibility sizes.

The skeleton had no UI-test target, so I created a small Xcode UI-test project under `Tests/`. UIKit exposes the standalone UISearchTextField as a text field rather than the search-field query initially used by the tests; the test queries were corrected.

## Decisions and lookups

No external dependencies or web research were needed. I used the supplied build instructions and local UIKit/Xcode tooling. I assumed dollar prices represent USD, symbol sorting starts A–Z, and price/change sorting starts highest first. Sort direction can be reversed. Prices and history restart from bundled prices on launch; only stars and sort were specified to persist. Simulation pauses while the app is inactive. History fills from real session samples rather than inventing historical prices.

## Verification

- Final full native UI suite: 3 tests, 0 failures (`build/UITestFinal.xcresult`, `build/uitest-final.log`). Covers company search, no-match recovery, clear button, favorites, My List, persistence across termination/relaunch, sorting, detail navigation, live sample updates, and scrolling.
- Dark mode at maximum accessibility text: passed, including scrolling and empty-search recovery (`build/UITestFinalAccessibility.xcresult`). Screenshots were exported and visually reviewed.
- All 21 model checks passed (`build/model-verification.txt`), including 80 simulated ticks, exactly 20 unique movements per tick, the ±1% bound, all six sort orders, search, persistence, 60-sample retention, change-from-open math, and timer delivery in the scroll-tracking run-loop mode.
- Final build/install/launch succeeded. Verified bundle ID, display name, UIKit scene manifest, icons, and byte-for-byte ticker resource equality. Final screenshot: `build/final.png`.

Nothing in the specification remains unfinished. A ready-made UIKit UI-test target in the starter would have made the build faster.
