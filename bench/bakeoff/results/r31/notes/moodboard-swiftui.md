# Moodboard build notes

Finished and installed as **Moodboard**, bundle ID `dev.bakeoff.moodboard.b977a41`, on simulator `486FD498-0E3E-4B64-8E4B-152BD8265EE7`. The app is left running with the original 600 tiles and 62 initial likes. Only this simulator was used.

## Implementation

- SwiftUI three-column lazy gallery, board menu with All and all ten boards, and independent Collection/Liked tabs.
- Tiles use the supplied HSL colors and centered emoji symbols. `Resources/tiles.json` is an exact copy of the supplied data and ships in the bundle.
- Detail pages support horizontal swipes, previous/next controls, title, board, date, and like/unlike. Paging follows the currently filtered collection. A Liked detail session keeps its neighbours stable when a tile is unliked; returning to the grid applies the updated filter.
- Select mode supports individual selection, select/deselect all within the current filter, batch likes, and confirmed batch deletion. An Undo action is available immediately after deletion until another action or navigation.
- Likes and deletions are saved synchronously as an atomic JSON file in the app's Application Support directory. Failed saves leave the displayed state unchanged and show an error.
- Light/dark appearances, accessible button labels, scalable text, scrolling detail pages, and vertically stacked batch actions at accessibility sizes. A generated app icon is included.

## What was hard / decisions

- Date-only values needed careful handling. Screenshot review caught SwiftUI's formatted Date text applying the device time zone and displaying the previous day. Dates are now formatted explicitly in UTC before display, with an exact-date UI regression check.
- The largest accessibility size required a simpler decorative header, an uncluttered board menu, a vertical gallery heading, and stacked batch buttons. Enabled and disabled controls were checked visually in dark mode.
- The skeleton had no Xcode project or interaction runner. I added a separate UI-test-only project to drive the already-installed app without changing the app's supplied build workflow.
- Ties in added dates preserve the supplied JSON order. Deletion removes tiles from both tabs. Seed likes initialize only a new library; saved unlikes are retained on relaunch.
- No external packages or web research were needed. Compiler feedback and the installed Xcode command-line tools resolved API/tooling details.

## Verification

- `SIM=486FD498-0E3E-4B64-8E4B-152BD8265EE7 ./build.sh --run` succeeds. Final bundle identity and the bundled dataset were verified.
- Foundation/model checks pass for dataset integrity, stable sorting, all ten board filters, initial likes, bulk likes, unlike, deletion, restoration, relaunch persistence, an empty collection, corrupt input, and save-failure rollback.
- Simulator UI tests: two initial interaction/accessibility tests passed in light mode, three broader tests passed in dark mode, and one targeted accessibility test passed after final visual refinements. No failures.
- UI coverage includes real horizontal swipes, previous/next boundaries, filtered first/last tiles, exact dates, individual and batch likes, selection counts, select/deselect all, confirmed deletion, Undo, deleting/restoring all 600 tiles, empty Liked, and process relaunch persistence.
- Screenshots were reviewed at normal and maximum accessibility text sizes. Logs, result bundles, and screenshots are under `build/`; test instructions are in `Tests/README.md`.
- Temporary UI-runner installation and test mutations were removed. Final app installation restores the supplied initial library and standard text size/light appearance.

## Unfinished / what would make this faster

No specified features remain unfinished. A preconfigured simulator interaction-test target would have saved the most setup time. Explicit date-only/time-zone semantics and tie-breaking rules would also have removed small assumptions.
