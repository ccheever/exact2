# Build notes

Finished: Watchlist is built, installed, and running on simulator
`486FD498-0E3E-4B64-8E4B-152BD8265EE7`, with bundle identifier
`dev.bakeoff.watchlist.bc9fa0b`. All requested features are implemented.

## What was hard

- Keeping the live list responsive during scrolling. A common-run-loop-mode timer
  updates visible cells in place; price/change row reordering waits until scrolling
  ends. The same model drives the live detail screen and bounded 60-sample histories.
- Maximum Dynamic Type needed separate arrangements: stacked quote rows and detail
  statistics, shorter control labels, a hidden overview, and scrollable empty states.
- Collapsing the large navigation title when editing produced an iOS 26 navigation
  background overlap at maximum text size. Keeping the native large title stable and
  using a standalone UISearchTextField fixed it. The keyboard layout guide keeps
  results above the software keyboard.
- The simulator requires execution outside the shell sandbox to reach CoreSimulator.
  A Bash 3 empty-array issue in the optional verification flags was also fixed before
  the final production build.

## Lookups and decisions

- Read the provided README/build script and the simulator's `ui help` for appearance
  and text-size commands. No web research or third-party runtime dependencies.
- Assumed USD because the source has no currency field. Symbol sorts A–Z; price and
  percentage change sort high to low, with symbol as a deterministic tie breaker.
- Exactly 20 distinct random symbols move per active second, each by at most ±1%.
  Every symbol records one sample per second, including unchanged prices. History
  begins with the supplied price; it is never fabricated or prefilled.
- The simulation pauses when inactive. Prices/history restart on a new launch;
  favorites and the chosen sort persist, as required.

## Verification

- 29 passing model/UIKit checks in `Tests/verification.txt`: bundled count and
  uniqueness, movement bounds, fixed open/volume, 60-sample retention, percentage
  calculation, search, all sorts, favorite controls, filtering/empty results,
  selection/navigation, live detail updates, and timer execution in tracking mode.
- Two passing terminate/relaunch checks in `Tests/persistence.txt` confirm that
  My List and sort survive a real process restart.
- Visually inspected list, chart, statistics, empty states, and software-keyboard
  presentation, including dark mode and the largest accessibility text setting.
- Confirmed the production bundle identifier and byte-for-byte bundled ticker data.
- `Tests/run.sh` repeats the automated checks, then builds and launches production.
  The verification harness is compiled out of production.

Nothing remains unfinished. A ready-made simulator interaction driver and a UIKit
starter with scene configuration and a test-build flag would have made this faster.
