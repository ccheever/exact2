# Watchlist build notes

Completed a programmatic UIKit app named Watchlist with bundle ID `dev.bakeoff.watchlist.b5aa998`. The finished build is installed and running on simulator `486FD498-0E3E-4B64-8E4B-152BD8265EE7`. No other simulator was used.

## Implementation

- All 260 supplied tickers ship in the app. `Resources/tickers.json` and the installed bundle's JSON were compared with the original and are identical.
- Twenty distinct random tickers move by at most ±1% every second. The timer runs in common run-loop modes so prices continue updating during scrolling.
- Visible cells update directly. Numerical sorting defers reordering during dragging and deceleration, then preserves the visible anchor.
- Search matches symbols and company names without case sensitivity. All/My List filtering composes with search and sorting.
- Stars work in the list and detail screen. Favorites and the selected sort persist in UserDefaults.
- Detail screens show live prices, percentage change from open, opening price, volume, and a custom gradient line chart with up to 60 real session observations.
- Includes native navigation, keyboard avoidance, empty states, VoiceOver labels and activation, haptics, light/dark colors, Dynamic Type, and a custom app icon.

## Difficult parts and decisions

The main challenges were keeping live updates active during scrolling without replacing rows, preserving scroll position when numerical ranks change, and remeasuring table headers and empty states when text size changes. At accessibility sizes the optional overview card becomes a compact live indicator, quote rows become vertical, and sort controls stack. During accessibility-size keyboard editing, secondary header information is hidden to make room for results.

The specification leaves sort direction and chart sampling details open. Symbol sorts A–Z; price and change sort highest first, with symbol as the tie breaker. Prices are USD. One observation per ticker is recorded each second, including unchanged prices. History starts with the supplied price, fills naturally, and retains 60 observations; historical data is never invented. Simulation pauses in the background and prices/history reset after process relaunch. Only favorites and sorting persist, as requested.

## Verification

Used the supplied README and build script; no external dependencies or web research were needed. CoreSimulator required execution outside the workspace sandbox, which was approved automatically.

The in-app `--self-test` harness passed 165/165 checks at standard text size and at the largest accessibility size in dark mode. Checks cover bundled data, searching, all sort orders, favorites, 70 bounded price ticks, the 60-sample history limit, scrolled cells, detail updates, and timer delivery in tracking mode. Separate `--persistence-write` and `--persistence-read` launches confirmed persistence across an actual process restart and restored the original preferences afterward.

Reviewed simulator screenshots of the main screen, details, keyboard search, and empty My List in light/dark mode and maximum Dynamic Type, including changing text size while running. Fixed a startup timing issue in the test-only empty-state preview. Final integration logs and screenshots are in `build/`. The simulator was returned to standard text size, light mode, and the normal main screen.

## Unfinished work

None of the specified features remain unfinished. Validation used simctl and in-app integration checks; no external touch-automation framework was available.

## What would have made this faster

A supplied simulator touch/accessibility automation harness and a starter UIKit scene manifest would have reduced setup and UI-verification work. Explicit sort directions and a definition of whether history means price updates or one-second observations would have removed the small specification choices above.
