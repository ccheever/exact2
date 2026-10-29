# Moodboard build notes

Finished and installed as **Moodboard** (`dev.bakeoff.moodboard.b0c6802`) on simulator `CEDFB9C9-FF04-4617-B156-94C56B83CA86`. No other simulator was used. The final installation starts with all 600 original tiles and the 62 likes supplied by the data.

## Decisions and tricky parts

- Same-day tiles retain their order in the source file. Dates represent local calendar days so the displayed date does not shift with the time zone.
- Detail paging follows the grid and board filter from which the tile was opened. That sequence stays stable when a tile is unliked, including when opened from Liked. Deleting the last tile returns to the grid.
- Likes and deletions are written atomically to `Library/Application Support/Moodboard/collection.json` in the app sandbox. A failed write leaves the displayed collection unchanged; unreadable saved data is preserved rather than overwritten.
- HSL needed explicit RGB conversion; SwiftUI’s hue/saturation/brightness initializer would produce different colors.
- The hardest UI work was keeping a useful three-column grid at the largest accessibility text size. Supporting copy is omitted there, filters reflow, bulk actions use icons with accessibility labels, and detail metadata scrolls. Native paging buttons supplement horizontal swipes, and Reduce Motion is respected.
- This simulator’s native confirmation dialog appeared as a popover without a visible Cancel control. Explicit native deletion alerts solved that. A native toolbar label also needed its ideal width and background adjusted to avoid clipping.

## Verification

Passed Foundation/Observation model checks and simulator UI tests covering all board filters, seed likes, like/unlike, swiping and paging boundaries, bulk like/delete, cancellation, Select All/Deselect All, clearing selection on filter changes, single-tile and empty-board states, deleting the entire library, relaunch persistence, failed writes, corrupt-state protection, and date handling in extreme time zones. Dark mode and the largest accessibility text size were exercised and screenshots inspected.

Results are in `build/FlowTests.xcresult`, `build/EdgeTests.xcresult`, `build/FinalAppearanceTests.xcresult`, and `build/EmptyLibraryTests.xcresult`. The bundled tile data matches `data/tiles.json` byte for byte. The final build was installed and launched after test data was removed; `build/final-installed.png` records it.

`Tests/run-tests.sh` reproduces the checks on the assigned simulator. **It resets Moodboard’s app data before and after testing**, then leaves a fresh build installed.

## Lookups, omissions, and improvements

Read the starter README and inspected the supplied JSON; used local Swift compiler, Xcode, and simulator tooling to verify APIs and behaviour. No external services, packages, or web research were needed. No specification features remain unfinished.

A supplied simulator UI-test target would have made this faster. The skeleton used direct `swiftc` compilation, so a small standalone UI-test project was added. Disabling automatic failure-diagnostic collection also avoided a stalled Xcode diagnostic collector during an early test failure. Icon asset compilation is now included in `build.sh`.
