# Moodboard build notes

Finished and installed as **Moodboard**, bundle ID `dev.bakeoff.moodboard.b31f139`, on simulator `CEDFB9C9-FF04-4617-B156-94C56B83CA86`. The final app is launched with the original 600 tiles and 62 initial likes. No specified features are unfinished.

## Implementation

- SwiftUI grid, native navigation, Browse/Liked tabs, and a menu containing All plus all ten boards.
- Correct HSL-to-sRGB conversion and centred source emoji symbols. The original JSON is bundled unchanged in `Resources/tiles.json`.
- Detail paging follows the board or Liked collection that opened it. That sequence stays stable when a tile is unliked, so the current page does not jump away.
- Multi-selection, Select all/Deselect all, bulk liking, confirmed bulk deletion, and Undo for the latest deletion during the session.
- Likes and deletion IDs are written atomically to Application Support. Failed writes report an error and do not change the visible saved state.
- Adaptive colours, VoiceOver labels and selection values, scrollable details, and vertically stacked bulk controls at accessibility text sizes. A small app icon is included.

## What was hard / decisions

HSL is not the HSB colour initializer supplied by SwiftUI, so the conversion needed to be explicit. Date-only values are parsed and displayed in UTC to prevent timezone shifts. Equal-date tiles retain source order; the spec did not define a tie breaker. Board filters are independent between the two tabs. Selection clears when changing boards or leaving a grid.

The largest text size required real layout changes: filter controls reflow, decorative toolbar text disappears, and selection actions stack vertically. Simulator UI tests also needed to tap tile centres that were clear of both the pinned filter and the expanded action panel.

The starter had no Xcode project. A test-only project in `Tests/` provides repeatable UI automation; the shipped app still builds with the original `build.sh` workflow. Failed UI runs initially spent a long time gathering system diagnostics; `-collect-test-diagnostics never` avoids that delay. The available computer-use connection did not expose this local simulator, so testing used local simulator/Xcode tools.

## Validation

- `Tests/ModelChecks.swift` passed: data counts, newest-first ordering, board filtering, likes/unlikes, batch deletion, restoring all 600 tiles, persistence, empty collections, and HSL reference colours.
- The normal-size simulator interaction suite passed: filtering, metadata, horizontal swipes, previous-page navigation, liking/unliking, Liked, bulk liking, deletion, Undo, and likes/deletions after termination and relaunch.
- Dark mode at `accessibility-extra-extra-extra-large` passed: deleting an entire board, the empty state and Undo, selecting a visible tile, accessible Like/Delete targets, opening detail, and scrolling to the Like control.
- Exported screenshots were visually reviewed. That review caught and fixed mid-word wrapping in the original bulk-action layout.
- Final `SIM=CEDFB9C9-FF04-4617-B156-94C56B83CA86 ./build.sh --run` succeeded. The installed bundle ID, display name, icon metadata, and bundled count were checked. `build/final.png` shows the restored 600-tile collection. The test runner was removed, and simulator appearance/text size were restored to light/large.

Passing reports: `build/UITests-verified.xcresult`, `build/Accessibility-verified.xcresult`, and `build/Accessibility-layout.xcresult`. The last report covers the final stacked selection layout.

## Lookups and what would make this faster

Consulted the starter README and local `simctl` / `xcodebuild` help. No external dependencies or web research were needed. A ready-made simulator UI-test target, disabled failure diagnostics by default, and explicit choices for date ties and paging after unliking would have shortened the work.
