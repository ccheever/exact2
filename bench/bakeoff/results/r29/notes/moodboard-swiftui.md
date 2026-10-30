# Moodboard build notes

Completed and installed as `dev.bakeoff.moodboard.b704ddf` on simulator `EF198926-1958-489D-A7A4-9196E454DD3A`. No other simulator was used. The final installation starts with the original 600 tiles and 62 initial likes; destructive test changes were cleared.

## Implementation decisions

- The original JSON ships unchanged in `Resources/tiles.json`. Tiles use explicit HSL-to-RGB conversion, since SwiftUI's hue initializer uses HSB.
- Dates sort newest first. For equal dates, the supplied file order is preserved because no secondary ordering was specified.
- Detail paging follows the grid's current board and Collection/Liked scope. Its order remains stable while viewing, including when the current tile is unliked on the Liked screen.
- Likes and deletions are saved atomically to the app's Application Support directory. Failed saves report an error and leave the current state unchanged.
- Selection supports individual tiles, select/deselect all, bulk like/unlike, and confirmed deletion. Selections are reconciled when visible tiles change.
- The interface uses native SwiftUI navigation, menus, tabs, paging, and alerts, with adaptive colors and an original geometric app icon. At accessibility text sizes, decorative header copy is omitted, labels stack, content scrolls, and bulk controls stay available at the bottom.

## Validation

Created a small standalone UI-test Xcode project in `Tests/` because the supplied app builds directly with `swiftc`.

Passing simulator checks covered:

- Opening tiles, horizontal swipes, previous/next buttons, and both paging boundaries.
- Like/unlike behavior, the Liked grid, and persistence after termination and relaunch.
- Board filtering, individual selection, select/deselect all, and bulk like/unlike.
- Canceling deletion, deleting a tile, persistence of deletion, deleting the entire collection, and empty states after relaunch.
- Dark-mode collection and detail screenshots.
- Selection, bulk controls, scrolling detail metadata, and liking at the largest accessibility text size.

The five functional tests passed together. The dark appearance test and final largest-text test also passed. Screenshots and `.xcresult` bundles are under `build/`. The bundled JSON was also checked byte-for-byte against the supplied data, including all 600 unique IDs and ten boards.

## What was hard / what I looked up

Keeping a Liked detail session stable when its contents change, implementing real HSL colors, and making the largest text size usable required the most care. XCTest reports some LazyVGrid cells behind the bottom action bar as hittable; the accessibility test now selects a cell whose center is visibly inside the viewport instead.

I read the supplied README and consulted `simctl help ui` for appearance and content-size controls. No external research was needed. The compiler's SwiftUI macro plugins and CoreSimulator required running the build/test tools outside the filesystem sandbox.

## Remaining work / ways to speed this up

No requested features remain unfinished. A ready-made simulator UI-test target and preconfigured compiler/CoreSimulator access would have made this faster.

Rebuild and launch with:

```sh
SIM=EF198926-1958-489D-A7A4-9196E454DD3A ./build.sh --run
```
