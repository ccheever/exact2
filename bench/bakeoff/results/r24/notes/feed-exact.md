# Feed — build notes

Finished and installed as **Feed** (`dev.bakeoff.feed.baab765`) on simulator `C2188CC6-FBE9-409A-92A8-ABD89AC2C432`. The final command was `EXACT_SIM=C2188CC6-FBE9-409A-92A8-ABD89AC2C432 bun exact.mjs ios --run`; the app is left installed and launched normally.

## Delivered

- All 400 supplied posts, sorted by age rather than assuming file order.
- Full post text, correctly proportioned rounded photos, horizontal image/title carousels, post detail navigation, and a full-screen photo viewer with swipe-down dismissal.
- Likes and counts shared between feed and detail, reply counts, pull-to-refresh adding three local posts, and a text composer that rejects whitespace-only submissions.
- Atomic local file persistence for likes, composed posts, and refreshed posts. Writes share a queue to preserve rapid changes. Unsaved composer drafts also survive relaunch.
- Native navigation bars, compose sheet, controls, haptics, semantic colors, Dynamic Type, and an app icon.
- Original data and all images shipped under `assets/data/`. Validated 400 unique post IDs and all 659 image references against the 120 bundled image files and their actual dimensions.

## Validation

- `bun exact.mjs verify`: **clean**, 9 tests passed. Light/dark layouts and the largest text size passed, including the auxiliary wide viewport.
- `EXACT_SIM=C2188CC6-FBE9-409A-92A8-ABD89AC2C432 bun exact.mjs verify ios`: **clean**, 9 tests passed. No layout, contrast, overlap, or touch-target findings. Eleven screens checked at accessibility-extra-extra-extra-large (53-point body text), with no spills. Relaunch persistence passed.
- Real simulator touches separately confirmed pull-to-refresh (400 → 403 posts), horizontal carousel scrolling, and photo swipe dismissal.
- The iOS verifier exercised scrolling with real touches for 32 seconds. It reported nothing for the app to fix. Frame timing was not judged because machine load was 25.3 on 18 cores. It identified two Exact-owned physics differences: rubber band ratio 0.66 versus UIKit 0.47, and status-bar scroll-to-top about 1.1 seconds versus UIKit 0.93–0.94 seconds. Timing under an unloaded machine remains unqualified. Detailed evidence is in `.exact/verify/ios-feel/`.

## What was hard / looked up

- Read Exact’s start guide and list-detail pattern, then looked up asset bundling, local files/grants, timestamps, native controls, full-screen navigation, gestures, accessibility, and verification.
- Native button tint belongs on the symbol child, and the compiler restricts native button styling. Some hints required an additional lookup to find the correct placement.
- Full-screen route presentation was supported but was easier to discover in the pitfalls guide and host implementation than in the main modal recipe.
- Pan callbacks deliver incremental deltas, so swipe dismissal accumulates displacement before applying its threshold.
- Horizontal carousels worked best with cards directly inside a flex-row scroll container. An extra row wrapper obscured the horizontal scroll extent from layout checking.
- Largest text required wrapping author metadata, flexible avatar sizing, and normal emergency wrapping for words longer than a line. Pressable one-line prose needed a separate minimum-44-point target.
- Adding the icon during a background build invalidated its packaged-asset receipt. A cached rebuild fixed it. Editing source during an iOS verification also caused a stale-build rejection; the final verification used a stable packaged build.

## Decisions and remaining limits

- No network, accounts, or server are used. Refresh rotates locally authored text and assigns unique persistent IDs.
- With only `minutesAgo` in the source data, seed timestamps are anchored to the local snapshot when initialized. Composed and refreshed posts receive local timestamps; labels update every minute.
- Reply counts are display-only, as specified. Card photos use a consistent cropped preview; full-screen photos show the entire original image.
- No requested feature is unfinished. The remaining qualification is scrolling timing on an unloaded machine and the toolkit-owned physics differences described above.

## What would have made this faster

A tested feed pattern with variable-height virtualized rows, nested horizontal media, full-screen swipe dismissal, and persistence would have removed several lookups. Clearer native tint examples, a more prominent full-screen recipe, a horizontal-scroll example without a wrapper, and prebuild guidance about asset-receipt changes would also help.
