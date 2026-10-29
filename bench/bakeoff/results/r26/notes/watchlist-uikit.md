# Watchlist — build notes

Finished and installed as `dev.bakeoff.watchlist.b74af95` on simulator `EF198926-1958-489D-A7A4-9196E454DD3A`. The app is left running normally. No other simulator was used.

## Implementation

- Programmatic UIKit app delegate, scene delegate, navigation controller, searchable watchlist, native segmented control and sort menus.
- All 260 tickers ship in `Resources/tickers.json`, byte-for-byte identical to `data/tickers.json`.
- Every active second, 20 distinct random tickers move by up to ±1%. A common-mode timer keeps updates running during scrolling.
- Visible quotes update directly. When sorting by price or change, reordering waits until scrolling ends and preserves the visible ticker's position.
- Stars, sort field and sort direction persist in UserDefaults. Both list and detail screens share the same model.
- Detail screens display live prices, change from open, supplied open and volume, and an accessible custom line chart.
- Semantic colors support light/dark appearance. Accessibility-sized rows stack their contents, detail content scrolls, and header fonts have readable caps so controls leave room for results. VoiceOver has separate quote and favorite actions and a chart summary.
- Search results resize above the keyboard; dragging or Cancel dismisses it.

## What was hard

Keeping live updates smooth without disrupting a finger scrolling the table, especially when the current sort order changes. Dynamic Type also needed explicit layout changes: the largest size initially overwhelmed the header, so controls were refined while keeping full scaling for company and quote content. An initial badge constraint bug was caught on the simulator and fixed before final validation.

The sandbox initially blocked CoreSimulator access. Running the authorized build and simulator commands with elevated tool permissions resolved it.

## Lookups and decisions

Used the supplied README/build script and simulator commands; no external web research or dependencies were needed.

The specification provides no historical series, so charts begin with the supplied price and collect actual simulation samples rather than inventing past prices. Every ticker gets one sample each active second, including unchanged prices, and retains the latest 60. Simulation pauses when inactive. Prices/history restart on a new app process; the required favorites and sort settings survive.

Default sorting is symbol A–Z. Price and change initially sort highest first; the menu also allows reversing any sort. Currency is USD. Volume remains the dataset's supplied value.

## Verification

Built and installed using:

```sh
SIM=EF198926-1958-489D-A7A4-9196E454DD3A ./build.sh --run
```

The explicit `--self-test` launch argument runs 26 model and UIKit checks inside the simulator. All passed, including 75 simulated ticks, all six sort/direction combinations, searches, empty states, favorite actions, visible-position preservation, detail updates, bounded chart history, and the actual timer firing in the scroll-tracking run-loop mode. Results: `Tests/self-test.txt`.

A separate two-process check seeded an isolated preferences suite, terminated the app, then verified favorite/sort/direction persistence on relaunch. Passed: `Tests/relaunch-test.txt`. Test preferences were cleaned up.

Reviewed simulator screenshots of the list and details in light/dark mode and at accessibility-extra-extra-extra-large, plus the search keyboard layout. Screenshots are in `Tests/`. Final simulator logs showed no constraint failures or crashes. Verification is programmatic UIKit interaction plus screenshot review, not an external gesture-driving UI test suite.

## Unfinished / what would help

No known unfinished specification features. A ready-made simulator gesture/accessibility test driver would have made end-to-end interaction testing faster; the provided toolkit only included simctl, so explicit in-app verification hooks were used.
