# Moodboard build notes

Finished and installed as `dev.bakeoff.moodboard.bf3d999` on simulator `498D6D1F-C21C-4B31-90F4-5D857216E256`. The final build excludes the QA-only routes. It ships the original 600-tile JSON unchanged, with its 62 initial likes.

## Implementation

- SwiftUI collection and Liked tabs, three-column lazy grids, eleven-option native board menu, and paged tile details with swipe navigation and accessible previous/next buttons.
- Select mode supports individual selection, select/deselect all within the current filter, bulk liking, and confirmed bulk deletion.
- Likes and deleted IDs are written atomically to Application Support before the UI commits each change. Deletions also remove likes. Failed saves display an alert without changing the visible state.
- HSL values are converted to RGB; the supplied emoji symbols remain centered in square tiles. Includes a custom app icon.
- Dynamic Type, VoiceOver labels, dark appearance, scrollable details and empty states, and adaptive selection controls.

## Hard parts and decisions

- At the largest accessibility size, decorative text and normal horizontal controls consumed too much space. Filters wrap, selection buttons stack, optional captions disappear, and empty states and detail metadata scroll vertically.
- Dates in the dataset are calendar dates rather than timestamps. Both parsing and display use UTC to avoid showing the previous day in Pacific time.
- Equal-date tiles sort by ascending ID, retaining deterministic ordering consistent with the supplied data.
- Detail paging uses a snapshot of the current grid's order. Unliking a tile while browsing Liked keeps that page stable; returning to the grid shows the updated results.
- Deletion is permanent within the app and requires confirmation. Selection clears when changing boards or leaving the collection screen.
- No external documentation or packages were needed. I read the supplied build instructions and inspected the data schema. The simulator service required elevated execution beyond the filesystem sandbox.

## Verification

- `swiftc Sources/Models.swift Tests/ModelTests.swift -o build/model-tests-runner` followed by `TZ=America/Los_Angeles ./build/model-tests-runner` passed.
- Checks cover all 600 records, all ten boards, newest-first ordering and ties, initial likes, bulk liking/unliking, deletion, reload persistence, deletion of the whole collection, failed writes, HSL conversion, and calendar-date display.
- Simulator QA builds exercised collection, Garden filtering, Liked, detail paging, selected tiles, and empty states. Screenshots were inspected in light and dark appearances, at normal and maximum accessibility text sizes. Captures are in `build/`.
- The simulator printed PASS for writing two likes and one deletion, then PASS after terminating and launching again to read those changes.
- QA uses a separate Application Support subdirectory. Build with `QA_BUILD=1` to include those routes; ordinary builds exclude them. `--qa-reset` resets only the QA state, and flags in `Sources/QA.swift` / `Sources/App.swift` provide reproducible screen fixtures.
- Final build/install/launch succeeded using the requested command. The bundled JSON was compared byte-for-byte with `data/tiles.json`.

## Limitations and what would help

No specified feature is left unimplemented. Direct computer-control access to Device Hub was not approved, so simulator checks used compile-time screen fixtures, screenshots, the actual paging action, and on-device persistence assertions; physical tap/swipe automation was not available. A preapproved simulator interaction tool would have made end-to-end gesture testing faster and more thorough.
