# Moodboard build notes

Completed all specified features. The finished app is built, installed, and running on simulator
`EEFC0029-1010-45E4-8242-42CF67E16C96` as `dev.bakeoff.moodboard.b878f6d`.
The final installation starts with all 600 tiles and the dataset's 62 original likes.

## Decisions and difficult parts

- Converted HSL to RGB explicitly; SwiftUI's hue/saturation initializer uses brightness and would produce incorrect colors. All 600 conversions match Python's independent colorsys reference.
- Chose ascending tile ID as a deterministic tie-breaker for equal dates. Boards appear alphabetically, including boards whose tiles have all been deleted.
- Detail navigation uses the filtered grid's sequence when opened. Unliking a tile in Liked keeps the current detail and swipe neighbours stable; returning to the grid removes the unliked tile.
- Saved likes and deleted IDs atomically in Application Support before publishing each change. A failed write leaves the collection unchanged and displays an error. Dates are parsed without time-zone shifts.
- Made the editorial heading scale, provided vertically scrolling detail metadata, and stacked action buttons at accessibility sizes. The grid stays three columns as specified.
- The installed simulator runtime presents confirmation dialogs as popovers without a visible Cancel button. Replaced bulk deletion's confirmation dialog with an explicit Delete/Cancel alert.

## Testing

- Five simulator UI tests passed: detail metadata, first-page boundary, actual swipes and arrow navigation, board filtering, individual likes across relaunch, Liked, bulk likes, cancellation and deletion, deletion persistence, all ten board counts, selection scope, select-all, and empty Liked/collection states.
- Final layout/interaction checks passed in light mode at standard text size and dark mode at the largest accessibility text size. Screenshots were visually reviewed, including scrolled detail metadata and selected tiles.
- XCTest-free model checks passed for HSL colors, ordering, initial likes, like/unlike persistence, batch operations, delete-all, deleted-tile exclusion, corrupt state reporting, and failed-write rollback.
- The bundled JSON is byte-for-byte identical to `data/tiles.json`. Final bundle name and identifier were verified.
- Tests and rerun instructions are in `Tests/` and `README.md`. `Tests/run.sh` resets this app's simulator data intentionally and restores the seed collection afterward.
- The temporary UI test runner was removed. No other simulator was booted, erased, or used.

## Lookups, limitations, and improvements

No web research or third-party dependencies were needed. Checked local simctl help for appearance/text-size controls and xcodebuild help for diagnostic collection. An initial failed UI test triggered slow diagnostic gathering; disabling that gathering made subsequent runs predictable.

Nothing from the specification remains unfinished. Design, tie-breaking, local persistence format, and permanent deletion confirmation were implementation choices. A starter with a ready-to-run UI test target and declared resource-copy/test commands would have made this faster.
