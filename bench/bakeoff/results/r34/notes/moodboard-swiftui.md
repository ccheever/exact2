# Moodboard build notes

Finished and installed as **Moodboard** (`dev.bakeoff.moodboard.b5190b8`) on simulator `EEFC0029-1010-45E4-8242-42CF67E16C96`. The app is left running on Explore, with light appearance and normal text size. No other simulator was used.

## Delivered

- SwiftUI three-column square grid, newest first, with all 600 supplied tiles.
- Correct HSL-to-RGB conversion and centered emoji symbols.
- Native board menu with All and all ten boards, including current counts.
- Swipeable detail pages with title, board, date, color values, likes, and accessible previous/next controls.
- Liked tab using the same grid and board filtering.
- Multiple selection, select/deselect all, batch like/unlike, and confirmed batch deletion with Undo.
- Atomic on-device persistence of likes and deletion IDs. The original data remains unchanged.
- Light/dark asset colors, a custom app icon, scalable type, scrollable detail content, accessible labels, and layouts that stack at the largest text sizes.

`Resources/tiles.json` is an exact copy of `data/tiles.json` and is included in the built application. Verified 600 unique IDs, ten boards, valid HSL ranges, and identical source/resource/bundle bytes.

## Decisions where the specification left room

- Equal added dates are ordered by ascending ID for deterministic results.
- Detail paging follows the grid and board from which a tile was opened. Its sequence stays stable while liking/unliking; returning to Liked immediately reflects changes.
- The data's 62 initial likes are honored on first launch.
- Changing boards clears selection; leaving a tab exits selection mode.
- A fully liked selection offers Unlike. A mixed selection offers Like, which likes every selected tile.
- Deletion uses persistent tombstones. Undo stays available until another action; it is intentionally an immediate action rather than a separate trash screen.
- Date-only values are formatted without shifting calendar days across time zones.

## What was hard / what I looked up

The main effort was fitting native navigation, three-column tiles, selection actions, and long detail text together at the largest accessibility text size. Decorative header copy is omitted at accessibility sizes; essential controls stack vertically and metadata scrolls.

The simulator runtime's native toolbar sizing initially compressed custom content. Explicit intrinsic sizes and shared-background configuration fixed the brand and selected-count labels. A parent accessibility identifier also propagated into child controls, so identifiers were moved to the individual actionable elements.

I read the starter README and used `simctl ui` help for the exact content-size options. No external libraries, network services, or internet research were needed. The compiler's macro expansion and CoreSimulator needed execution outside the shell sandbox; the authorized build and test commands completed successfully.

A final normal post-install launch exposed symbols that were blank until relaunch. I replaced geometry-derived font sizes with explicit artwork sizes and added a screenshot pixel check across three ordinary launches. All passed. Empty states are also explicitly scrollable so their actions remain reachable at the largest text size.

The starter had only a Swift compiler script. I added an independent XCTest UI-test project, plus asset catalog compilation to `build.sh`. UI tests use a separate state file so destructive test scenarios never modify the normal collection.

## Verification

All interaction testing ran on the assigned simulator:

- Final regression: **4 tests passed, 0 failures** (`build/FinalRegression.xcresult`).
- Final rendering regression: **2 tests passed, 0 failures** (`build/RenderingRegression.xcresult`), including actual symbol pixels across three normal launches.
- Final dark asset/appearance verification: **passed** (`build/FinalAppearance.xcresult`).
- Largest supported Dynamic Type size (`accessibility-extra-extra-extra-large`): **passed** (`build/FinalLargestText.xcresult`), with screenshots visually reviewed.
- Empty-state scrolling and Show all boards at the largest size: **passed** (`build/AccessibleEmptyState.xcresult`).
- Covered square three-column layout, ordering, board counts, swipe/arrow paging, first/last boundaries, detail metadata, likes across relaunch, unlike while viewing Liked, batch actions, canceled deletion, Undo, clearing hidden selection, all tiles deleted, empty Liked boards, and deletion across relaunch.
- Screenshots are in `Screenshots/`; `Screenshots/Explore.png` shows the final production launch.

Build and launch:

```sh
SIM=EEFC0029-1010-45E4-8242-42CF67E16C96 ./build.sh --run
```

Run the interaction suite after installing the app:

```sh
xcodebuild test \
  -project Tests/MoodboardUITests.xcodeproj \
  -scheme MoodboardUITests \
  -destination 'platform=iOS Simulator,id=EEFC0029-1010-45E4-8242-42CF67E16C96' \
  -derivedDataPath build/UITests \
  -parallel-testing-enabled NO CODE_SIGNING_ALLOWED=NO
```

## Unfinished / what would make this faster

No known unfinished specification requirements. A starter with asset catalog support and a ready-to-run simulator UI-test target would have removed setup work and made iteration faster.
