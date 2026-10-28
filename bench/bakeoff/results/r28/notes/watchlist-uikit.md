# Watchlist build notes

Finished and installed as **Watchlist** (`dev.bakeoff.watchlist.bd549f2`) on simulator `EF198926-1958-489D-A7A4-9196E454DD3A`. The final normal build was launched successfully. No other simulator was used.

## Implementation

- UIKit app and scene delegates, programmatic navigation, native search and sort menus, self-sizing stock rows, and a Core Graphics line chart. No SwiftUI or storyboards.
- The complete, unchanged 260-ticker dataset ships as `Resources/tickers.json`.
- A shared main-thread market store changes 20 distinct random stocks each second, by at most 1% each. Its timer runs in common run-loop modes so scrolling does not pause updates.
- Visible stock cells update directly. Value-based row reordering waits until scrolling settles and preserves the visible anchor.
- Every stock retains at most 60 one-second price samples, including unchanged prices. Detail screens observe the same live store as the list.
- Favorites and the selected sort persist in UserDefaults. The installed app starts with an empty My List and symbol sorting after test preferences were restored.
- Adaptive light/dark colors, Dynamic Type layouts, explicit VoiceOver actions, keyboard-safe native search, and scrollable empty states are included.

## Difficult parts

Keeping live price updates smooth while rows move under price/change sorting required separating cell updates from list reordering. Maximum Dynamic Type also exposed two issues that were corrected: weighted fonts being scaled twice and empty-state content exceeding the available viewport. Large-text layouts now stack fields vertically and allow the complete empty state to scroll.

## Lookups and decisions

I read the skeleton README and build script and used the installed UIKit SDK/compiler and simulator tooling. No external data or web research was needed.

The specification does not define currency or sort direction. Prices are displayed in USD; symbols sort A–Z, while price and change sort high to low, stated in the sort menu. Live price history starts with the bundled price and fills naturally rather than inventing historical samples. Simulation pauses while the app is inactive; prices and history restart on process relaunch, while the required preferences persist.

## Verification

Simulator-only checks in `Tests/SimulatorChecks.swift` drove actual UIKit controls and verified:

- All 260 bundled stocks, percent-change calculations, symbol/company searching, whitespace/case handling, and no-results states.
- Empty My List, adding/removing stars in rows and detail, and removing the last saved stock.
- All three sort orders and native menu selection state.
- Seventy simulated ticks with exactly 20 changed stocks per tick, the 1% movement bound, and 60-sample history limits.
- Visible-cell reuse, current prices after scrolling, and the timer firing in the scrolling tracking run-loop mode.
- Correct detail navigation, shared live chart data, keyboard focus, and VoiceOver row activation.
- Favorites and sorting surviving actual termination and relaunch.

Screenshots in `artifacts/` were inspected in light/dark mode and at the largest accessibility text size, including detail metrics, native keyboard search, and the scrolled empty-state action. UIKit correctly adjusts the list's safe area above the search keyboard. The final Info.plist was validated and the bundled JSON was compared with the source.

Test hooks compile only when `WATCHLIST_TESTING=1`; they are absent from the finished installed build. The normal build command remains:

```sh
SIM=EF198926-1958-489D-A7A4-9196E454DD3A ./build.sh --run
```

## Unfinished work and workflow improvements

No specification features remain unfinished. A ready-made simulator interaction and accessibility inspection harness would have reduced the time spent building test drivers and checking maximum-text layouts. A bundled app-icon generation step would also make the custom icon easier to revise.
