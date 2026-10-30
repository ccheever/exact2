# Watchlist build notes

Finished and installed as **Watchlist** (`dev.bakeoff.watchlist.b957013`) on simulator `498D6D1F-C21C-4B31-90F4-5D857216E256`. The app is left running normally, in light mode at standard text size. No other simulator was used.

## Implementation

- Programmatic UIKit app/scene delegates, navigation controller, live diffable table, native search, segmented list selection, and sort menu.
- All 260 original tickers ship unchanged in `Resources/tickers.json`; the built bundle was compared against the source data.
- Every second, 20 distinct random tickers move within ±1%. The timer runs in common run-loop modes so scrolling does not stop prices. Visible rows update directly; numeric reordering waits until an active scroll ends to keep touch targets stable.
- Each ticker retains up to 60 one-second price observations. The detail chart, quote, percentage, and absolute change update together. Open and volume come from the supplied data.
- My List, sort key, and sort direction are stored in UserDefaults.
- Adaptive layouts support dark mode and the largest accessibility text size. Search uses the keyboard layout guide so all results remain reachable. Empty states are scrollable at large text sizes.
- A small code-drawn app icon ships with the app.

## What was hard

Keeping live values updating without disturbing an active scroll required separating value refreshes from numeric reordering. The largest text size exposed oversized fixed controls and awkward horizontal layouts; the final layout hides decorative copy, uses a compact navigation title, and stacks controls and quote values. Keyboard testing also identified the need to constrain the list above the keyboard.

CoreSimulator was unavailable inside the default sandbox. The requested simulator build/install/test commands succeeded with approved simulator access.

## Lookups and decisions

I read the skeleton's local README and build script; no web research was needed. The specification does not prescribe numeric sort direction, historical initialization, currency, or background behavior. Price/change default to highest first; symbol defaults to A–Z, and either direction can be selected. Dollar formatting is used. Charts start with the supplied price and build genuine simulated history instead of inventing earlier data. Simulation pauses while inactive and starts fresh after process relaunch; favorites and sort settings persist.

## Validation

- Built and launched with `SIM=498D6D1F-C21C-4B31-90F4-5D857216E256 ./build.sh --run`.
- Foundation model checks pass for all 260 records, case-insensitive and whitespace-trimmed search, no results, favorites, all six sort orders, saved settings, malformed data, and 120 ticks validating 20 unique changes, ±1% limits, percentage calculations, and bounded histories.
- XCTest-free UIKit checks ran in the actual simulator using `--ui-check`: row star actions, segmented selection, search, empty results, scrolled live refresh, detail chart updates, and detail favorite actions all passed. The actual one-second timer was also exercised in scroll-tracking run-loop mode.
- A separate process termination/relaunch verified favorite, sort key, and direction persistence.
- Simulator screenshots were inspected in light/dark modes, at standard/largest text sizes, with search keyboard visible, and on detail/empty screens. Evidence is in `build/`.
- Bundled JSON matches the supplied source byte for byte, and the final Info.plist passes `plutil -lint`.

Re-run the model checks from this directory:

```sh
swiftc -module-cache-path build/ModuleCache Sources/Market.swift Tests/MarketChecks.swift -o build/MarketChecks
./build/MarketChecks
```

Re-run simulator interaction checks:

```sh
xcrun simctl launch --terminate-running-process --console 498D6D1F-C21C-4B31-90F4-5D857216E256 dev.bakeoff.watchlist.b957013 --ui-check
```

## Unfinished work and improvements to the toolkit

No specification features remain unfinished. A supplied simulator interaction driver and screenshot/assertion harness would have made UI verification faster. A native icon resource scaffold would also reduce setup work.
