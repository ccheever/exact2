# Watchlist build notes

Finished and installed as **Watchlist**, bundle ID `dev.bakeoff.watchlist.beb9b18`, on simulator `486FD498-0E3E-4B64-8E4B-152BD8265EE7`. No other simulator was used. The final installed build excludes the simulator test harness.

## Implementation

- Programmatic UIKit app/scene delegates, native navigation and search, segmented All/My List controls, favorite stars and swipe actions, and ascending/descending menus for symbol, price, and percentage change.
- All 260 supplied tickers ship unchanged in `Resources/tickers.json` and the app bundle.
- A one-second timer in the common run-loop modes updates exactly 20 unique random tickers by at most ±1%. Visible cells refresh during scrolling. Price/change sorting defers row reordering during a drag or deceleration, then preserves the visible position when applying the new order.
- Detail screens show price, change, open, volume, and a custom live chart capped at 60 samples. Favorites and sort/direction persist with UserDefaults.
- Adaptive light/dark colors, Dynamic Type layouts, VoiceOver row descriptions and favorite actions, accessible chart descriptions, and a generated native app icon.

## What was hard

Keeping price updates active without moving the rows under a scrolling finger needed separate value updates and order refreshes. The largest accessibility text size required vertical cell/stat layouts and removal of the decorative overview/footer. Initial testing caught double-scaled label fonts and a clipped chart badge; both were fixed. iOS 26's navigation/search presentation also needed actual simulator screenshots, rather than relying on window-layer snapshots. The optional test build initially exposed the system Bash version's empty-array behavior with `set -u`; the build script now uses a nonempty source-file array.

## Lookups and decisions

Read the supplied README/build script and inspected the JSON. No web research or external dependencies were needed. Prices are displayed as USD because no currency was supplied. History begins with the supplied price and collects real simulated samples rather than fabricating historical prices; a collecting-history label explains the first minute. Simulation runs while the app is active and restarts from the bundled prices on a fresh launch. Only favorites and sort were specified to persist.

## Verification

`Tests/MarketChecks.swift` passed checks for the complete dataset, symbol/name search, empty searches, favorites, preference persistence, all six sort orders, 75 ticks with exactly 20 changes each and ±1% bounds, and 60-sample retention.

The optional `WATCHLIST_TESTS=1` build exercised real UIKit star controls, list scopes, empty states, live visible cells, scroll-position retention, sorting, detail navigation, detail favorite actions, and chart samples. A separate terminate/relaunch check confirmed favorites and price/ascending sort survived process death. Simulator screenshots were reviewed in light and dark appearances and at the largest accessibility text size, including scrolled detail statistics and search/keyboard presentation. Screenshots are in `build/`.

Final normal build/install/launch succeeded. Verified the final bundle's identity and exact ticker data, and confirmed test instrumentation is absent.

## Unfinished work

None of the specified features remain unfinished.

## What would have made this faster

A ready-made simulator UI interaction/inspection harness and a starter with the UIKit scene manifest and icon pipeline already configured would have reduced setup and screenshot iteration.
