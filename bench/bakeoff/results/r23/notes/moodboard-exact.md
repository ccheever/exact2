# Moodboard build notes

## Delivered
- Exact iPhone app named Moodboard, bundle ID `dev.bakeoff.moodboard.be5a542`.
- All 600 source records ship offline; the original JSON is also included as `assets/tiles.json`.
- Three-column square tile grids, independent board filters for All Tiles and Liked, native navigation, large details with board/date, horizontal swipes and accessible Previous/Next controls.
- Individual likes, bulk likes, Select All/Deselect All, confirmed bulk deletion, and empty states. Likes and deletions persist across app relaunches.
- Generated transparent emoji symbol assets and a custom app icon.

## What was hard / looked up
- Read Exact's start guide and looked up tabs, navigation, persisted primitive sets, virtual lists, gestures, native selects, confirmation alerts, styling, and simulator driving.
- Exact action names can shadow array methods: an action named `filter` had to be renamed. Route names passed to `path()` must be literals.
- The web and iPhone test hosts differ after reload: iPhone starts on the root screen, so persistence tests explicitly revisit the relevant tab or route.
- Large text required flex wrapping and compact native controls. The native virtual list also needed a clipping container so scrolled tiles could not draw beneath the board controls. A row-height estimate improves the initial scroll extent.
- Emoji are decorative image content. Rendering the supplied symbols to transparent PNGs avoids treating their fixed visual size as UI typography; titles, metadata, and controls use platform text roles.

## Decisions / assumptions
- Same-date tiles retain their source-file order; dates are date-only values displayed without a timezone shift.
- A detail session keeps its neighbour sequence when a tile is unliked, so it does not unexpectedly jump away. Deleted neighbours are removed from that sequence.
- Deletion is local and permanent, with a native confirmation before committing. There is no server or account requirement.
- Filters and browsing ID sets are also saved. All persistence fits comfortably inside Exact's 16 KB limit per primitive-list state.

## Validation
- `bun exact.mjs verify`: clean; all 10 feature tests pass. Light/dark checks and all 18 largest-text screens pass.
- `EXACT_SIM=EEFC0029-1010-45E4-8242-42CF67E16C96 bun exact.mjs verify ios`: clean; all 10 feature tests pass. Light/dark checks, all 12 largest-text screens, and relaunch checks pass.
- Real simulator touches verified board menus and horizontal detail swipes.
- Native scrolling: nothing for the app to fix, with no dropped frames at either tested lift. The busy shared machine prevented a timing judgment; the verifier separately reports an Exact-level status-bar scroll-to-top timing difference.
- Final `bun exact.mjs ios` completed and installed Moodboard on simulator `EEFC0029-1010-45E4-8242-42CF67E16C96`. No other simulator was used.
- Verification screenshots and detailed reports are in `.exact/verify/`.

## Unfinished work
None of the specified features remain unfinished.

## What would have made this faster
- A gallery starter combining a native virtual grid, board picker, and horizontal detail browsing.
- An explicit example of a virtual list below fixed controls, including the clipping boundary required on iOS.
- Documentation commands that do not need a writable lock in the pinned toolkit checkout.
- Clearer up-front documentation of web versus iPhone reload navigation behavior.
