# Watchlist build notes

Finished and installed as **Watchlist**, bundle ID `dev.bakeoff.watchlist.b2590b5`, on simulator `498D6D1F-C21C-4B31-90F4-5D857216E256`. The app is open on its main screen. The simulator was restored to light appearance and standard Large text size after accessibility checks. No other simulator was used.

## Implementation

Programmatic UIKit throughout: app and scene delegates, navigation controller, a diffable table, native search and segmented controls, and a Core Graphics chart. All 260 records are shipped in `Resources/tickers.json`, copied unchanged from `data/tickers.json`.

Twenty distinct, randomly selected stocks move every second by at most ±1%. A timer in common run-loop modes keeps visible quotes updating during scrolling. Price/change sort reordering waits until scrolling finishes. Favorites and all six sort choices are saved in UserDefaults. Detail screens share the same live data and include price, percentage and dollar change, open, volume, and the most recent 60 samples.

## What was hard

- Live updates must update existing cells without interrupting scrolling or favorite taps. Sorting also needed to preserve stock identity and return to the top when the user chooses a new order.
- Recreating a button's menu on every price tick interfered with an open sort menu. Menus now update only when the sort preference changes.
- The largest accessibility text size required vertically arranged stock information, a full-width sort control, bounded chart annotation fonts, and hiding the supplemental market-pulse card and row sparklines. Primary stock information remains enlarged and fully readable.
- Search needed explicit keyboard-layout-guide constraints and a smaller header while editing so the last result remains reachable above the keyboard.
- The simulator's accessibility cache can retain duplicate virtual cell entries after sorting. The UI tests use stable identifiers and first-match queries for those controls.

## Lookups and assumptions

Used local simulator/Xcode tool help for appearance, text size, and screenshot export. No external libraries or web services were needed. Assumed USD pricing, symbol A–Z as the initial sort, and descending price/change as their first menu options. Charts collect one sample per active second, including unchanged quotes, rather than inventing prior history. Prices reset from the bundled data on process launch; favorites and sorting persist. Simulation pauses when the scene is inactive. These choices are explained in the app's information dialog.

## Validation

- Standalone model checks passed for all 260 records, symbol uniqueness, exactly 20 distinct bounded moves, positive prices, 60-sample history limits, all six sorts, case-insensitive/whitespace-trimmed search, favorites, and zero-open/unchanged-price handling.
- Simulator UI tests passed for navigation, list and detail favorites, empty My List, company and symbol search, no results and clearing search, sorting, favorites/sort surviving termination and relaunch, scrolling, and live chart sample updates.
- Dark appearance and the largest accessibility text category were tested and screenshots visually reviewed. Final standard-size light appearance and detail favorite toggling also passed.
- The final Info.plist validates, the icon is 1024×1024, and the bundled JSON matches its source. Passing test archives and screenshots are in `build/qa/`. The temporary simulator UI-test runner was removed.

Build again with:

```sh
SIM=498D6D1F-C21C-4B31-90F4-5D857216E256 ./build.sh --run
```

Model checks are in `Tests/ModelTests.swift`; the UI test project is `Tests/WatchlistUITests.xcodeproj`. The icon generator is `Scripts/draw-icon.swift`.

## Unfinished work and ways to go faster

No specification features remain unfinished. A UIKit starter with a scene manifest and a ready-to-run simulator UI test target would have saved the most setup time. Explicit currency and chart-history semantics in the specification would also have removed small assumptions.
