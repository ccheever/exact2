# Watchlist build notes

Finished and installed as **Watchlist**, bundle ID `dev.bakeoff.watchlist.b0cf4ec`, on simulator `C2188CC6-FBE9-409A-92A8-ABD89AC2C432`. No other simulator was used. The final app is open at its main screen with standard text size and light appearance.

## Implementation

- UIKit app delegate and scene delegate; all interface code is programmatic. Scene manifest and app identity are in `app.env`.
- The original 260-entry `data/tickers.json` is shipped byte-for-byte as `Resources/tickers.json`.
- One shared market model changes 20 distinct random symbols every second, each by at most ±1%, and samples every ticker into a rolling 60-value history. Open and volume remain the supplied values.
- The timer runs in common run-loop modes. Visible quotes update during scrolling; price/change row reordering waits until the gesture and deceleration finish so rows do not move under the user's finger.
- Symbol/company search, All/My List, independent star controls, three sort modes in both directions, and saved favorites/sort work together.
- Details show current price, percent and dollar changes since open, open, full volume, and a live Core Graphics chart with low/high prices.
- Adaptive light/dark colors, Dynamic Type through the largest accessibility size, native navigation/search/keyboard behavior, accessible star/detail actions, chart summaries, and scrollable empty states.
- A custom Core Graphics app icon is included. Its source generator is `Testing/MakeIcon.swift`.

## What was hard

Keeping quotes live during table tracking without making dynamically sorted rows jump required separating quote refreshes from row ordering. Large text required different stack directions for rows, controls, chart headings, and statistics, plus hiding the redundant fixed footer at accessibility sizes. Keeping empty states scrollable avoids crowding when space is limited.

The starter has no Xcode project or UI-driving tool. I created a small standalone XCTest UI-runner project that tests the installed bundle without changing the app's build script.

## Lookups and decisions

No external web research or dependencies were needed. I used the local compiler, simulator, XCTest, and result-export tools to validate platform behavior. The simulator's native search interface uses a bottom search field and a close button rather than a Cancel button; the UI tests account for that.

The data has no currency field, so prices are shown in USD. Symbol defaults to ascending; price and change default to descending when selected. Both directions are available and persisted. History starts with the real supplied price and fills naturally rather than inventing previous prices. Simulation pauses while inactive, retains its in-memory history on return, and resets prices/history on a fresh process launch. Favorites and sort persist.

## Verification

- `SIM=C2188CC6-FBE9-409A-92A8-ABD89AC2C432 ./build.sh --run` succeeds and installs/launches the app.
- `Testing/ModelTests.swift` passes: all 260 records, case-insensitive symbol/name and whitespace search, empty search results, all six sort orders, star/sort restoration, and 80 ticks checking all 260 prices for bounded changes, exactly 20 changes per tick, unchanged open/volume, correct percentages, and 60-sample history limits.
- Final simulator UI suite: **3 tests, 0 failures**. Covers starring/filtering, detail navigation and chart sampling, search by company and symbol, no matches, sort selection, relaunch persistence, removal of the final favorite, and simulation progress throughout a slow scroll gesture.
- A separate dark-mode run at `accessibility-extra-extra-extra-large` passes. Main, detail, statistics, and empty-state screenshots were inspected and layout issues corrected.
- Results are in `build/FinalTests.xcresult` and `build/AccessibilityDarkFinal.xcresult`; logs and exported screenshots are under `build/`.

Model check:

```sh
swiftc Sources/Market.swift Testing/ModelTests.swift -o build/ModelTests
build/ModelTests
```

UI checks (after installing the app):

```sh
xcodebuild -project Testing/WatchlistUITests.xcodeproj \
  -scheme WatchlistUITests \
  -destination 'platform=iOS Simulator,id=C2188CC6-FBE9-409A-92A8-ABD89AC2C432' \
  -derivedDataPath build/UITestDerived \
  -parallel-testing-enabled NO test
```

## Unfinished work and what would help

Nothing in the requested scope remains unfinished. A preconfigured standalone UI-test runner and screenshot export command in the starter would have made simulator verification faster.
