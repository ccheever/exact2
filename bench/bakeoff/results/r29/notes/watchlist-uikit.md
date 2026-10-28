# Watchlist build notes

Completed as a programmatic UIKit app, with no storyboards or SwiftUI. The final build is installed and running on simulator `498D6D1F-C21C-4B31-90F4-5D857216E256` as `dev.bakeoff.watchlist.b438280` (Watchlist). All 260 original ticker records are shipped unchanged in the app bundle.

## What was hard

- Keeping the one-second simulation running during scrolling requires a timer in the common run-loop modes. Visible quote labels update directly; numeric sorts defer row reordering until a drag/deceleration finishes and preserve the visible ticker when possible.
- The largest accessibility text size needed a different layout: stacked ticker information, vertically arranged quote facts and sort controls, and a scrollable empty-state row. The optional market pulse summary is omitted at accessibility sizes to leave room for stocks.
- UIKit's iOS 26 search presentation places the native search field near the bottom. The list follows the keyboard layout guide so results remain scrollable above the keyboard.
- The skeleton has no Xcode project or UI-test target. I added launch-only model/UI diagnostics and a Python/simctl runner without adding a test framework dependency.

## What I read or decided

- Read the supplied README, build script, and ticker JSON. No external documentation or web research was needed.
- Chose symbol ascending as the initial sort, with price and percentage-change descending when first selected. Both directions are available and persisted.
- Each active second changes exactly 20 distinct random tickers by up to ±1%, then records a sample for every ticker. Charts retain the latest 60 samples, including unchanged prices. History starts with the supplied quote rather than invented historical prices.
- Prices and history are session data; stars, sort field, and sort direction persist. Simulation pauses while the app is inactive. These choices are explained in the app's Simulation sheet.
- Assumed USD for the supplied prices, since the dataset has no currency field.

## Verification

Built and launched with:

```sh
SIM=498D6D1F-C21C-4B31-90F4-5D857216E256 ./build.sh --run
```

Ran `python3 Tests/run_simulator.py`: all 17 simulator scenarios passed. Checks cover all six sort orders; symbol/name search and missing results; native star actions; saved preferences across termination/relaunch; exactly 20 bounded price moves; 60-sample histories; matching live list/detail prices; actual timer execution in tracking mode; keyboard avoidance; light/dark appearance; and maximum accessibility text size, including scrolling the empty-state action into view.

Screenshots and logs are in `build/verification/`. The final normal app screen is `build/verification/final-main.png`. Diagnostics use separate preferences from normal app launches. The final installation was launched without diagnostic arguments, in light mode at the standard text size.

## Unfinished work

None of the requested features remain unfinished. Background market simulation and persisted price history were not requested; the app explicitly treats prices/history as an active-session simulation.

## What would make this faster

A ready-made UIKit scene manifest, asset catalog, and simulator UI-testing target would reduce setup and make tap/swipe verification easier. An explicit currency and a defined history/background policy would remove the few product assumptions above.
