# Moodboard build notes

Finished and installed as `dev.bakeoff.moodboard.b79f692` on simulator `C2188CC6-FBE9-409A-92A8-ABD89AC2C432`. The app is launched in its original state: 600 tiles, including the 62 likes supplied by the dataset. Simulator appearance and text size were restored to light / standard after testing. No other simulator was used.

## Implementation

- SwiftUI navigation, three-column lazy grids, independent board menus on Collection and Liked, and a horizontal paged detail view.
- Tiles use the exact supplied HSL colors converted to RGB, with the supplied emoji centered. `Resources/tiles.json` is identical to `data/tiles.json` and is copied into the built app.
- Individual likes, bulk like/unlike, selection of all tiles in the current filter, confirmed bulk deletion, and a seven-second Undo action.
- Likes and deleted IDs are written atomically to the app’s Application Support directory on every change. A failed write reports an error and does not change the visible state.
- Native Dynamic Type, dark colors, accessibility labels and selected values, and explicit previous/next controls as an alternative to swiping. Accessibility text sizes omit decorative introductory copy and stack bulk actions vertically.
- A custom generated app icon ships with the app.

## Decisions and difficult parts

The specification does not define ordering for equal dates. Source order is preserved for ties. Neighbors follow the filtered grid from which a tile was opened. The detail pager keeps that sequence stable when unliking a tile in Liked; returning to the grid reflects the updated likes. Dates are parsed as calendar dates in UTC so a local time zone cannot shift them to the previous day.

The important color pitfall was HSL versus SwiftUI’s HSB initializer. Explicit HSL conversion avoids incorrect tile colors. The largest accessibility size initially gave decorative copy too much space; simulator screenshots led to a simpler layout with the grid and actions immediately available.

The starter had no test project or simulator tap utility, so a small independent XCTest UI-test target was created under `QA/`. Native confirmation dismissal updates the grid asynchronously; tests wait for the empty count instead of reading during that transition. Local `simctl`, `xcodebuild`, and `xcresulttool` help were consulted. No external packages or web lookups were needed.

## Verification

Passed model checks in `Tests/StoreChecks.swift`: all 600 unique tiles, ten boards, seeded likes, stable newest-first order, board filtering, like/unlike persistence, batch deletion persistence, undo, deleting the entire collection, known HSL values, and rollback on a failed save.

Passed simulator UI tests:

- `testCompleteFlow`: details, swipe navigation, page boundaries at the beginning, individual like/unlike, Liked updates, board filtering, select all, bulk likes, deletion, Undo, and likes/deletions surviving termination and relaunch.
- `testEmptyAndEdges`: deleting every liked tile, empty state, Undo, reaching the last tile of a filtered board, and swiping in both directions at the end without wrapping.
- `testLargeText`: dark mode at `accessibility-extra-extra-extra-large`, actual tile selection, enabled bulk controls, bulk liking, detail scrolling, and individual unliking.

Passing result bundles, exported screenshots, and logs remain in `QA/`. `QA/final.png` shows the final installed app. Build caches and the temporary simulator test runner were removed.

Build again with:

```sh
SIM=C2188CC6-FBE9-409A-92A8-ABD89AC2C432 ./build.sh --run
```

Model checks can be repeated with:

```sh
swiftc Sources/TileStore.swift Tests/StoreChecks.swift -o QA/store-checks-runner
QA/store-checks-runner
```

## Unfinished work and improvements to the setup

No known unfinished specification features. A preconfigured UI-test target and direct simulator interaction commands would have reduced setup time. Clearer tie-order and filtered-neighbor rules would have removed the small behavior choices noted above.
