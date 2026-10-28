# Watchlist build notes

Finished the UIKit app and left the normal build installed and running on simulator `C2188CC6-FBE9-409A-92A8-ABD89AC2C432` as `dev.bakeoff.watchlist.be78701`. No other simulator was used.

## Implemented

- All 260 supplied tickers, with the original JSON shipped unchanged in the app bundle.
- Twenty distinct random tickers move by up to ±1% each second. A timer in common run-loop modes keeps prices updating during scrolling. Visible cells update directly; numerical sort reordering waits until dragging/deceleration finishes and preserves the visible anchor.
- Symbol, price and percentage-change sorting, each with ascending/descending order; case-insensitive symbol/company search with whitespace trimming.
- Stars, All/My List filtering, helpful empty states, and saved favorites and sort preferences.
- Native detail navigation with live price, percentage and dollar changes, opening price, exact volume, and a custom line chart holding the latest 60 samples.
- Light/dark colors, Dynamic Type including the largest accessibility size, accessible quote summaries with working VoiceOver activation, and separate accessible star buttons.
- A native simulation-information sheet and a generated app icon.

## What was hard

Keeping a continuously changing sorted table stable while it scrolls required separating visible value updates from row reordering. The large accessibility sizes also needed different row/metric layouts and a shorter count heading. The optional market summary is hidden at accessibility sizes so the controls and tickers remain prominent.

Screenshot review caught an empty-state footer positioned above its intended location; that was fixed. A persistent section-header view avoids moving labels between transient header instances. The optional verification build initially exposed Bash 3.2's empty-array behavior under `set -u`; the build script now uses one nonempty compiler-input array.

## Lookups and decisions

Used the skeleton README, compiler feedback, and `simctl` command help; no external market service or web research was needed. Assumed USD because the dataset specifies no currency. Default sorting is symbol ascending; choosing price/change defaults to descending, with both directions available.

Charts collect real simulation samples from launch, starting with one point and growing to 60 rather than inventing historical data. Simulation pauses while the app is inactive. Only favorites and sorting persist, as requested; prices and history restart from the bundled dataset on a fresh launch.

## Verification

The simulator verification suite passed 44 checks with zero failures, covering data, all six sort combinations, search, favorites, empty states, navigation, detail values, live chart history, price bounds, scrolling updates, VoiceOver row activation, and layout ambiguity. It was exercised in light/dark mode and at the largest accessibility text size. Two additional checks passed after an actual process relaunch, confirming favorite and sort persistence.

UI checks invoke UIKit control actions directly rather than injecting physical touches. Screenshots were visually reviewed. Verification sources are excluded from the final normal app build. See `Tests/README.md` for reproduction and `build/verification-*` for reports and screenshots. The final bundled JSON was compared byte-for-byte with `data/tickers.json`, and the final bundle identifier was verified.

## Unfinished work and faster tooling

No required feature remains unfinished. A provided simulator touch/keyboard automation tool would make testing actual finger gestures and software-keyboard transitions faster. A UIKit starter with a scene manifest and an optional simulator-check target would also reduce setup time.
