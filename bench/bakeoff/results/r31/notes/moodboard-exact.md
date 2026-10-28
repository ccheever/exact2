# Moodboard build notes

Finished and installed as **Moodboard**, bundle id `dev.bakeoff.moodboard.b0ff86d`, on simulator `C2188CC6-FBE9-409A-92A8-ABD89AC2C432`. No other simulator was used.

## Delivered

- All 600 supplied records are bundled through the imported `data/tiles.json`. Tiles use their exact HSL values and centered symbol artwork, with three square tiles per row and stable newest-first ordering.
- Ten-board filtering and All are available in the navigation bar's native filter menu. The subtitle identifies the current board and count.
- Native Explore and Liked tabs retain independent detail navigation and browsing sequences. Details show title, board, date, like state, swipe navigation, and accessible previous/next buttons.
- Selection supports individual tiles, select/deselect all, bulk like, bulk unlike, and confirmed permanent deletion.
- Queued mutations save changes atomically to app storage. Original data stays bundled; likes and deletion overrides survive relaunch.
- Empty boards, empty Liked, an empty collection, partial final rows, and a tile deleted while open in the other tab are handled.
- Included generated symbol PNGs and an app icon.

## Validation

`EXACT_SIM=C2188CC6-FBE9-409A-92A8-ABD89AC2C432 bun exact.mjs verify ios` finished **clean**:

- Seven functional tests passed.
- Light and dark mode checks passed.
- Nine screens passed the largest accessibility text size, with nothing spilling.
- Relaunch persistence checks passed.
- Real-touch scrolling physics matched UIKit. The report recorded zero hitches and 16.7 ms longest frame, but declined to judge timing because the machine was busy.

Additional real-touch simulator drives confirmed both swipe directions. A separate full-collection test selected and deleted all 600 tiles and confirmed the empty state persisted after reload.

The final `bun exact.mjs ios --run` rebuilt, installed, and launched the production app plan on the assigned simulator.

## What was hard and what I looked up

Exact's starter, navigation, storage, menu, symbol, pointer, persistence, and simulator-driving documentation were needed. I also inspected its iOS gesture arbitration implementation.

- iOS 26's full-content Back gesture initially took a rightward pointer swipe. Exact's native `swiperight` handler gives tile browsing precedence away from the screen edge while preserving edge Back.
- The standalone native board picker exceeded its reserved height at AX5. Moving filtering into a native navigation menu resolved this. Keeping the menu definition outside the projected header also made its rows addressable by the activation driver.
- Emoji used as text produced misleading contrast and fixed-font warnings. The twelve original symbols are rendered as bundled transparent artwork; accessibility labels preserve tile information.
- A nested collection scan exceeded Contract's iteration budget. Independent browsing snapshots and efficient deletion pruning avoid that scan.
- The initial background build saw a stale starter app id. The app manifest and data module now agree.

## Decisions and remaining limitations

Equal-date tiles retain their supplied order. Detail browsing follows the filtered collection captured when opening a tile, so unliking in Liked does not unexpectedly replace the current tile. Deleted items are removed from those browsing sequences. Bulk deletion asks for confirmation and is permanent.

No requested iPhone features remain unfinished. The web functional tests also pass, but its largest-font audit flags overlap while the long board menu is open. The iPhone uses UIKit's native menu and passes its final native accessibility checks.

## What would have made this faster

A photo-grid starter, direct documentation of right-swipe versus iOS content Back arbitration, and native picker intrinsic-size handling at accessibility sizes would have reduced iteration. Treating decorative emoji as artwork in contrast/type audits would also help. Shared compiler caches and the simulator driver made the subsequent build and verification loop quick.
