# Moodboard build notes

Completed and installed as **Moodboard** (`dev.bakeoff.moodboard.b13528b`) on simulator `EF198926-1958-489D-A7A4-9196E454DD3A`.

## Finished

- All 600 supplied tiles, drawn from their HSL colours and centred symbols, in a full-width three-column grid ordered newest first.
- All ten board filters, independent Browse and Liked tabs, native navigation, large tile details, dates, and neighbouring-tile swipes.
- Persistent likes, unlikes, and batch deletions; selection, Select All, cancellation, and destructive confirmation.
- Independent detail sequences for both tabs. Deleting a tile also closes its detail if it was open in the other tab.
- The original `data/tiles.json` is imported into the offline data module and also shipped unchanged as `assets/tiles.json`. A custom app icon is included.

## Validation

- `bun exact.mjs verify`: all 10 feature tests pass; normal-size light/dark checks pass. The web-only largest-font checker reports nine menu/background overlaps while a popup is open.
- `EXACT_SIM=EF198926-1958-489D-A7A4-9196E454DD3A bun exact.mjs verify ios`: **clean**, 10 tests pass, no light/dark findings, and nothing spills at the largest accessibility text size across 12 screens.
- Real-touch tests exercised both swipe directions. Persistence was checked through relaunches.
- Native scrolling was reported “like UIKit.” The machine was heavily loaded, so the toolkit judged physics rather than timing.
- The final `bun exact.mjs ios` build completed and installed the embedded app on the specified simulator.

## What was hard and what I looked up

- iOS 26's content-back gesture initially competed with rightward tile swipes. Exact's native `swiperight` handler gives the tile the correct gesture ownership. Leftward swipes use pointer events because there is no corresponding `swipeleft` attribute. I consulted the toolkit's gesture documentation and native gesture-precedence implementation.
- Exact's native bars, menus, navigation stacks, persistence, asset bundling, and simulator-driving syntax required documentation lookups.
- Scalable emoji needed enough line height at the largest text size. Multicolour emoji also triggered foreground contrast checks, resolved by supplying contrast-safe ink while preserving the original HSL fills.
- Native relaunch returns to the root screen, unlike a web reload that retains its route. The test now explicitly reopens the detail after relaunch.

## Decisions

- Tiles added on the same date retain their supplied order.
- The 62 initially liked tiles in the source data are honoured.
- A detail sequence stays stable when its tile is unliked, allowing continued browsing until returning to the grid.
- Batch deletion is permanent and asks for confirmation.

## Remaining work and speedups

No requested iPhone features remain unfinished. A documented pair of native left/right swipe handlers, clearer popup-check exclusions on the web, and a warm native build cache would have made this faster. The initial native toolkit build was the longest wait.
