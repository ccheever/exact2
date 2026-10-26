# Moodboard build notes

Finished and installed as **Moodboard** (`dev.bakeoff.moodboard.b12a3da`) on simulator `EEFC0029-1010-45E4-8242-42CF67E16C96`. No other simulator was used. The final first-launch check showed all 600 tiles; its screenshot is `.exact/final.png`.

## Implemented

- Bundled `data/tiles.json`, with exact HSL-to-sRGB artwork, centered symbols, and stable newest-first ordering.
- Full-width three-column grids, a native board picker containing All and all ten boards, and native All Tiles / Liked tabs.
- Large tile views with title, board, date, hearts, real left/right touch swipes, and accessible previous/next controls with correct endpoint behavior.
- Multiple selection, Select All, bulk liking, and confirmed bulk deletion. Selection clears when changing boards or tabs.
- Persistent like overrides and deleted IDs. Existing likes are respected; bulk liking never unlikes an already-liked tile. Detail browsing retains its sequence after unliking. A tile deleted in another tab becomes unavailable in an already-open detail.
- Dark mode, Dynamic Type, descriptive accessibility labels, selection haptics, empty states, and a custom icon.

## Validation

`bun exact.mjs verify`: nine tests passed; no light/dark, relaunch, or largest-text findings at phone and wide widths.

`EXACT_SIM=EEFC0029-1010-45E4-8242-42CF67E16C96 bun exact.mjs verify ios`: nine tests passed; no light/dark layout findings; nothing spilled on eleven screens at accessibility-extra-extra-extra-large (53-point body). Actual simulator touches verified both swipe directions.

The scrolling comparison recorded no hitches, no dropped frames at fling release, no blank samples, and a 4.6 ms main-thread p95. Its one finding was explicitly attributed to Exact: status-bar scroll-to-top took 1.08 seconds versus UIKit's 0.93–0.94 seconds. See `.exact/verify/ios-feel/feel.txt` and `feel.json`. This toolkit behavior remains; no specified app feature is unfinished.

## What was hard / looked up

- Exact's Contract syntax is narrower than TypeScript: indexed access uses options, helper names cannot shadow array methods, and arithmetic and symbol names have prescribed vocabularies. Compiler diagnostics and `contract vocab` supplied the correct forms.
- A filter outside the native navigation scroller overlapped the first tile row. Putting it inside the scroller restored correct large-title insets.
- iOS 26's full-content back gesture initially captured right swipes. Registering Exact's `swiperight` handler gave image browsing the correct precedence while leaving native edge-back behavior available.
- A hand-authored menu produced web overlap findings at the largest text size. A native `select` provides the board menu more reliably and passes both platform checks.
- Source changes during verification invalidate its built copy. Final verification ran against an unchanged, installed build.

## Choices made

Dates are date-only values, displayed without timezone shifts. Tiles sharing an added date retain their supplied order. Board choice and browsing position persist in addition to the required likes and deletions. Deleted tiles are permanently removed, so bulk deletion asks first.

## What would make this faster

A tested image-grid/gallery pattern with paging gestures and bulk selection, clearer documentation of iOS 26 gesture precedence, and a concise Contract-versus-TypeScript syntax reference would reduce iteration. Exact's typed data seam, compiler replacement suggestions, native controls, and automated accessibility checks were useful.
