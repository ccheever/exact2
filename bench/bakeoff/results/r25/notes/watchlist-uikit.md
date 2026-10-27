# Watchlist implementation notes

Finished a programmatic UIKit app named Watchlist, bundle ID `dev.bakeoff.watchlist.b1c7e23`. It is built, installed, and launched on simulator `486FD498-0E3E-4B64-8E4B-152BD8265EE7`. No other simulator was booted or used.

## What was hard

- Keeping live updates smooth while scrolling and sorting by changing values. A common-run-loop timer updates visible cells during tracking. Value-based sort order is reconciled after scrolling, preserving the visible symbol and its position.
- Supporting the largest accessibility text size without squeezing labels into narrow columns. Rows, market statistics, sort controls, and detail cards use stacked layouts at accessibility sizes. The chart heading also stacks. Search uses a native search text field, and the keyboard layout guide keeps results above the keyboard.
- Avoiding double Dynamic Type scaling when constructing weighted fonts, and keeping the detail star button stable while prices update every second.
- The initial sandbox could compile the app but could not reach CoreSimulator. Running the simulator build/test commands with the approved tool access resolved this.

## Decisions and assumptions

- Exactly 20 distinct tickers are chosen each second, each moving independently by a random amount in ±1%. Prices retain full precision internally and display two decimal places. A one-cent floor prevents nonpositive prices.
- Each ticker records its current price once per second, including unchanged prices. Charts begin with the supplied price and collect real simulation samples until their rolling 60-sample history is full. No history is fabricated.
- Symbol sorting defaults to ascending; price and change default to descending when selected. Direction is selectable and persisted along with the selected sort.
- My List stores symbol IDs in UserDefaults. Simulated prices and chart histories restart with the bundled data on a fresh process launch; the specification only requires stars and sort to persist.
- Simulation pauses when the scene is inactive. Volume and open remain the supplied values.
- Currency is displayed as USD. Change is `(price / open - 1) * 100`.

## Lookups

Read the skeleton README/build script and inspected the supplied JSON. No external research or dependencies were needed. Used compiler/SDK feedback to check UIKit API usage.

## Verification

- Built with `SIM=486FD498-0E3E-4B64-8E4B-152BD8265EE7 ./build.sh --run`.
- All 31 opt-in simulator checks pass. Results are in `QA/checks.log`; rerun with `python3 Tools/run_checks.py` after building.
- Checks cover all 260 unique tickers, case-insensitive symbol/company search, whitespace and no-result cases, all six sort/direction combinations, star controls, percent move bounds, distinct update selection, unchanged unselected prices, 60-sample history, shared live detail data, and timer execution in scroll tracking mode.
- A separate termination/relaunch sequence verifies persisted stars and sort, then restores the original preferences.
- Inspected simulator screenshots in light and dark modes, with the keyboard shown, with empty My List, and at the largest accessibility text size. Screenshots are in `QA/`.
- Confirmed the bundled JSON is byte-for-byte identical to `data/tickers.json` and includes all 260 entries.

## Unfinished work

None of the requested features remain unfinished. The app requires no network connection.

## What would have made this faster

A preconfigured UIKit lifecycle and simulator-safe build permissions would have removed setup overhead. A simulator touch/gesture testing tool or a prepared UI-test runner would make navigation and scrolling checks faster; this skeleton instead uses opt-in in-app checks and simctl screenshots.
