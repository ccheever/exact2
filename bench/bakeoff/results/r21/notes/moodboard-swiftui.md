# Moodboard build notes

Implemented the complete specification in SwiftUI. The final app is built, installed, and launched on simulator C2188CC6-FBE9-409A-92A8-ABD89AC2C432 with bundle identifier dev.bakeoff.moodboard.526288 and display name Moodboard. No other simulator was booted, erased, or used.

## Implementation decisions

- Bundled an exact copy of data/tiles.json as Resources/tiles.json: all 600 tiles and their original 62 likes are present in the clean installation.
- Sort dates newest first and preserve input order for ties because the data has dates without times.
- Convert HSL to sRGB explicitly; SwiftUI's hue/saturation/brightness initializer uses a different color space.
- Keep each detail browsing session's tile order fixed. Unliking a tile while browsing Liked leaves the current page in place; the grid updates on returning. Previous/next controls supplement swipe gestures and disable at the ends.
- Persist complete liked-ID and deleted-ID sets atomically in Application Support. A failed save leaves the visible collection unchanged; unreadable saved state is protected from overwriting.
- Selection is scoped to the current board and clears when changing boards or tabs. Bulk Like becomes Unlike when every selected tile is already liked. Deletion has explicit confirmation and immediate Undo.
- The three-column grid remains three columns at accessibility sizes. Larger text uses stacked bulk buttons, a simpler decorative header, full-width board controls, scrollable detail metadata, and improved dark-mode button contrast.
- Included a custom app icon generated locally from vector shapes.

## What was hard / what I checked

The most involved work was combining a swipe pager with changing liked membership, making bulk controls usable at the largest text size, and validating persistence without leaving test mutations in the delivered app. UI testing exposed duplicate accessibility titles from neighboring pager pages; offscreen pages now stay out of accessibility. Visual review also caught a wrapping Delete label at the largest size, which was fixed with full-width stacked actions.

I consulted the local simctl and xcresulttool command help. I assumed date ties should retain source order, that board filters apply to each tab independently, and that bulk liking mixed selections should like every selected tile. No web research or external assets were necessary.

## Validation

- Built and launched with the required SIM=... ./build.sh --run command.
- Tests/StoreChecks.swift passed checks for the full dataset, ten board filters, ordering, HSL, individual and bulk likes, deletion and undo, all-deleted and no-liked states, relaunch persistence, failed writes, and corrupt-state protection.
- Simulator UI tests passed the complete browsing/persistence flow: filtering, horizontal swiping, first-page boundary, liking/unliking, relaunch, bulk likes, deletion cancellation, deletion, undo, and deletion persistence.
- A separate UI flow passed for bulk unlike, empty Liked, a one-tile Liked detail with both navigation buttons disabled, unliking the final liked tile, and persistence of the empty state.
- Dark mode at accessibility-extra-extra-extra-large passed simulator interaction checks and screenshot review for Explore, detail, scrolled metadata, and selection actions.
- Screenshots and test logs are under build/. The optional UI-test project is under Tests/; the production build continues to use the provided build.sh.
- Removed the test app data and test runner, restored light appearance and standard Large text, then built and installed the final clean app. Verified the bundled JSON matches the supplied data.

## Unfinished work

None of the requested features remain unfinished. Undo is intentionally immediate and session-only; the deletion itself survives relaunch.

## What would have made this faster

A ready-made simulator UI-test target or touch-driving tool would have reduced test setup. The starter has only a direct swiftc build, so I added a small optional Xcode UI-test project for actual interaction checks. An explicit tie-break rule for same-day tiles and a defined behavior for unliking while viewing the Liked pager would also remove minor product decisions.
