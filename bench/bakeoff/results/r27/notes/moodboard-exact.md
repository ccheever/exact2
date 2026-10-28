# Moodboard build notes

Finished and installed as **Moodboard**, bundle ID `dev.bakeoff.moodboard.b64f6ae`, on simulator `EF198926-1958-489D-A7A4-9196E454DD3A`.

## Completed

- Bundled all 600 tiles by importing the supplied, unchanged `data/tiles.json` into the Exact data module.
- Three-column, full-width, virtualized square grid, ordered newest first. Each tile uses its HSL color and centered symbol.
- Native navigation-bar menu for All and all ten boards, with independent filters in Library and Liked.
- Large tile detail with title, board, date, horizontal swipes, accessible Previous/Next controls, and boundary handling.
- Like/unlike, initial likes from the dataset, and a Liked grid.
- Multi-selection, Select All within the current filter, batch likes, and confirmed batch deletion.
- Persistent liked/deleted ID sets using Exact's persistent state. Deleting also removes a tile from Liked and closes its stale detail in the other tab.
- Native navigation, tabs, menus, alerts, haptics, dark mode, empty states, accessible tile labels, and a custom app icon.

## Validation

`EXACT_SIM=EF198926-1958-489D-A7A4-9196E454DD3A bun exact.mjs verify ios` finished **clean**:

- 13 tests passed, 0 failed.
- First screen, 13 test ends, and relaunch passed light/dark checks.
- Largest accessibility text size (53-point body), 15 screens: nothing spills.
- Scrolling reported “like UIKit,” with no dropped lift frames or measured hitches. The host was busy, so the tool qualified its timing judgment and judged physics.
- Real simulator touches also confirmed opening a tile, swiping to its neighbor, and opening the native board picker during development.

The two fixed emoji sizes are explicitly accepted in `app.json`: they are artwork within squares, not reading text. Interface labels and metadata retain Dynamic Type. The final `bun exact.mjs ios --run` rebuilt the installed plan and launched the finished app on the required simulator.

## Hard parts and lookups

- Read Exact's start-here guide and began with the tabs pattern. Looked up native menus, virtualized lists, pan/panrelease, persistent sets, asset packaging, navigation, and simulator-driving commands.
- The shared compiler cache initially needed sandbox escalation. The first native runtime build took several minutes; later plan updates and builds were quick.
- Adding an icon during the first background build invalidated its asset receipt. Exact detected this and a subsequent build repaired it.
- Exact action names cannot shadow array methods, and route names passed to `path` must be literals. Compiler messages identified both fixes.
- iOS test reload returns to Library; virtualized rows must be revealed by row key before targeting their nested tiles. Tests were adjusted accordingly.
- Largest-text verification caught an overflowing batch toolbar and a board picker that overlapped scrolled tiles. The toolbar now wraps; the board menu moved into the native navigation bar.
- Emoji are rendered as artwork, but contrast checking initially treated them as inherited white text. The data module now supplies a calculated contrasting ink, which also supports monochrome symbol rendering.
- Automatic approval review rejected an optional web preview because testing was restricted to the specified simulator. The default web verification was stopped; final validation used the required iPhone simulator.

## Decisions and assumptions

- Tiles with equal added dates preserve their order in the supplied JSON.
- Neighboring tiles follow the filtered grid from which the detail was opened. Unliking the currently displayed tile does not abruptly dismiss it; it disappears from Liked when returning to the grid.
- Dates are displayed as calendar dates without timezone shifts.
- Changing boards or tabs clears the current selection. Batch actions finish selection mode.
- Deletions are permanent after confirmation, as the specification provides no trash or restore flow.

## Unfinished

No specified features remain unfinished.

## What would have made this faster

- An Exact starter for a virtualized three-column image grid with selection and a paged detail.
- A first-build warning that changing packaged assets during prebuild invalidates the receipt.
- A dedicated artwork-symbol primitive that does not need fixed-font verification annotations.
- More explicit documentation for iOS reload navigation and revealing nested virtualized targets.
