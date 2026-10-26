# Watchlist build notes

Finished and installed as **Watchlist**, bundle ID `dev.bakeoff.watchlist.a4ae81`, on simulator `EF198926-1958-489D-A7A4-9196E454DD3A`. The final build uses UIKit exclusively and excludes the optional validation code.

## Implemented

- All 260 tickers, with the supplied JSON copied unchanged into the app bundle.
- One-second simulation selecting 20 unique tickers. Prices stay positive, use cents, and are clamped so rounding cannot exceed a 1% move.
- Symbol/company search, symbol ascending and price/change descending sorts, All/My List segments, and stars in both list and detail.
- Persistent favorites and sort via UserDefaults.
- Live detail prices, percent and dollar changes since open, original open and volume, and a custom accessible line chart containing up to 60 one-second samples.
- Light/dark appearances, Dynamic Type through the largest accessibility size, VoiceOver row descriptions and favorite actions, keyboard avoidance, empty states, native navigation, and an app icon.

## Hard parts and decisions

- The timer runs in common run-loop modes so scrolling does not pause it. Visible cells update directly; price/change sorting waits until dragging and deceleration finish before reordering rows.
- Large text needed a vertical cell layout with a full-width company name. The optional market-pulse summary is omitted at accessibility sizes to prioritize the stock list. Detail prices scale to fit on one line, with other text wrapping and scrolling.
- iOS 26 places native search at the bottom. Using the keyboard layout guide makes the last search result reachable above the keyboard.
- The shell's nounset behavior treats an empty array as unbound. The build now always supplies the explicit Swift module name, with an optional validation compilation flag.
- Assumed prices are US dollars; price and change sorts put the highest value first. The specification does not provide historical samples, so charts start with the supplied current price and accumulate real simulated samples, including unchanged values. Prices and chart history reset on a fresh launch; only favorites and sort persist, as requested. Simulation pauses while the app is inactive.
- No web research or additional dependencies were needed.

## Validation

Used only the assigned simulator. Simulator checks passed for data count, case-insensitive and whitespace-trimmed symbol search, company search, no matches, favorite filtering/removal, all sort orders, preference restoration, and 80 ticks of movement bounds and history retention. Verified favorites and sort across an actual process relaunch.

UIKit checks exercised star-button actions, segment actions, search filtering and empty states, stable visible row identities while scrolling, current displayed prices, the real timer, timer delivery in UIKit tracking mode, and scrolling the last search result above the keyboard. Inspected screenshots for the main list, detail, empty My List, keyboard search, dark mode, and largest accessibility text. Screenshots are under `build/`.

The final production build compiled without warnings, installed and launched successfully. Its Info.plist validated, its bundled ticker file matched the source byte-for-byte, and its installed bundle was verified with simctl. Test preferences were cleared before the final install.

## Reproducing

Normal build:

```sh
SIM=EF198926-1958-489D-A7A4-9196E454DD3A ./build.sh --run
```

The optional checks in `Sources/Checks.swift` compile only with `WATCHLIST_QA=1`. Their launch switches include `--qa-model`, `--qa-search`, `--qa-detail`, `--qa-empty`, and `--qa-seed` / `--qa-verify` / `--qa-reset`. Reinstall a normal build after validation.

## Unfinished and improvements to the workflow

No specification features remain unfinished. A simulator gesture/accessibility automation tool would make end-to-end interaction testing faster; the provided toolkit required in-app validation hooks and simctl screenshots. An explicit currency and historical-sample policy would remove the small assumptions above.
