# Moodboard build notes

Moodboard is built with Exact and ships all 600 records from `data/tiles.json` through the imported data module. The app uses the requested bundle identifier, `dev.bakeoff.moodboard.b854896`.

## Implementation choices

- Tiles are sorted by date descending, with ID ascending as a deterministic tie breaker for tiles added on the same day.
- The board menu applies to Library and Liked. Its choice survives relaunch.
- Detail browsing follows the grid order at the moment a tile is opened. Unliking a tile in Liked keeps its detail visible until returning to the grid, so navigation does not jump unexpectedly.
- Likes and deletions are stored atomically in a private changes file; the original bundled dataset remains intact. Initial likes come from the supplied records.
- Bulk deletion requires confirmation. Select All applies to the current filtered grid.
- Tile symbols are centered artwork and scale with text size. Small grid symbols have a ceiling so they stay inside the three-column squares. Titles, dates, menus and other readable labels use platform typography.

## What was hard or required lookup

- Exact's native list supplies row insets automatically; explicit zero row padding was needed for a grid that fills the screen width.
- A repeated neighbor membership filter exceeded Exact’s interpreted iteration limit at 600 tiles. Reconciliation now happens once after deletion.
- Pan callbacks report incremental movement. Reliable one-tile swipes require accumulating the movement and choosing a neighbor on release rather than checking each delta independently.
- Exact's color role names and its special capped font syntax required documentation lookups. The compiler gave actionable messages for unsupported values.
- Restoring an open detail needed persisted IDs and neighbor order as well as persisted edits.
- A navigation-bar button using a dialog command did not activate through the native bar. An explicit action calling `showModal` made the Delete button behave correctly.

## What would make this faster

A gallery/grid starter with selection badges, accessible square artwork, and a paging-detail example would remove much of the initial layout and gesture research. Native navigation-bar subtitles and controls worked better at accessibility sizes than separate fixed rows above the grid. A documented no-op action statement would also simplify optional branches. The generated TypeScript source interface, platform tabs/navigation, simulator driver, and verification reports were useful.

## Completion and verification

All specified iPhone features are complete. The finished app was built, installed, and launched with `EXACT_SIM=EF198926-1958-489D-A7A4-9196E454DD3A bun exact.mjs ios --run`. No other simulator was used.

- Final `verify ios`: **clean**, 7 tests passed, 0 failed.
- Light and dark mode: no layout or contrast findings.
- Largest accessibility text size (53-point body): nothing spills on any checked screen. Browsing and selection also passed focused real-touch checks at this size.
- Relaunch checks confirm likes, unlikes, deletions, and board selection persist.
- Tests cover full-board deletion and its empty state, canceling deletion, toggling selection, and selecting/liking all 600 tiles at once.
- Real-touch swipes move forward and backward through neighbors; Previous is disabled at the first tile.
- Scrolling physics matched UIKit, with no dropped-frame lifts. The verifier declined to judge timing because the shared machine was busy.
- Web verification: all 7 functional tests pass and normal light/dark layouts are clear. Its largest-text inspection reports overlaps while the board popover is open. This remaining web-only result does not occur with the native iPhone menu; the shipping iOS verification is clean.

No requested iOS feature was left unfinished. Screenshots and detailed verifier output are under `.exact/verify/`.
