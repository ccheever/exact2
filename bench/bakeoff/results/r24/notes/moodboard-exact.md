# Moodboard build notes

Finished and installed as **Moodboard** (`dev.bakeoff.moodboard.b1bf12c`) on simulator `EEFC0029-1010-45E4-8242-42CF67E16C96` with `bun exact.mjs ios`.

## Delivered

- All 600 supplied tiles are shipped through the bundled `data/tiles.json` import. Their HSL colors are converted to square color tiles with centered symbols.
- Three-column, virtualized grids; newest-first ordering; All and all ten board filters; separate All Tiles and Liked tabs.
- Large tile view with title, board, date, left/right swipes, previous/next controls, and like/unlike.
- Multiple selection, Select All/Deselect All, bulk liking, and confirmed permanent bulk deletion.
- Likes and deletions are saved atomically in the app's private `app:/data/library.json` file and survive relaunches. The 62 seed likes are respected.
- Native navigation, menus, tabs, alerts, dark mode, Dynamic Type, accessibility labels, and a custom app icon.

## Validation

- `bun exact.mjs verify`: clean; six tests passed; light/dark and largest-text checks clean.
- `EXACT_SIM=EEFC0029-1010-45E4-8242-42CF67E16C96 bun exact.mjs verify ios`: clean; six tests passed; eight screens checked at the largest iOS text size with nothing spilling.
- Real simulator touches exercised board menus, horizontal swipes, liking, multiple selection, and the destructive confirmation.
- Relaunch tests cover both likes and deletions; additional coverage includes unliking while viewing Liked, selection toggling, filter changes, first-item paging boundaries, and an empty Liked collection.
- The native scrolling report says there is nothing for the app to fix. The machine was busy, so the toolkit did not judge timing; it separately attributes a slightly slower status-bar scroll-to-top animation to Exact itself. Detailed results are in `.exact/verify/ios-feel/`.

## What was hard / looked up

- I used Exact's start guide and tabs pattern, then looked up virtualized collections, menus, file grants, navigation, pan gestures, confirmation alerts, and simulator driving.
- Exact pan events report incremental movement. Accumulating those deltas and paging on release was necessary for reliable physical swipes.
- Derived expressions can be evaluated inside callbacks. An initial nested collection lookup exceeded Exact's iteration budget; keeping a snapshot of the active grid's IDs removed that nested work.
- Fixed board controls need an inline navigation title and the correct `navigationScroll` target. Getting that combination right prevented grid offsets from shifting after returning from detail and enabled native status-bar scrolling.
- The filter and bulk-action surfaces needed opaque system backgrounds and the grid needed clipping to keep colorful tiles from interfering with controls at the largest text size.
- Keying the grid by board resets its scroll position when a different board is chosen.

## Decisions and remaining limitations

- Equal added dates retain their order in the supplied file, since the data contains dates without times.
- Dates are formatted in UTC so a tile's calendar date cannot shift with the device's time zone.
- Paging follows the grid snapshot used to open a tile, so unliking the current tile does not abruptly remove it from view.
- Board filters and selection are session state. Likes and deletions persist as requested.
- No requested feature is unfinished. Deletion is permanent and explicitly confirmed.

## What would have made this faster

A tested Exact image-grid/pager starter with bulk selection, explicit documentation about callback evaluation costs, and an example combining a fixed filter with native navigation scrolling would have saved the most iteration. A short pan-delta example and a faster focused iOS layout check would also help.
