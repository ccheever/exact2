# Feed build notes

## Delivered

- Native iPhone feed with all 400 supplied posts, sorted by time, full post text, original image aspect ratios, and horizontally scrolling titled cards.
- Post detail, full screen photos with swipe-down dismissal, native compose sheet and keyboard, pull-to-refresh, and persistent likes and new posts.
- All 120 original JPEGs, their dimensions, and the original posts JSON are shipped in `assets/data/`. Their bytes were checked against the supplied files.
- Bundle identifier: `dev.bakeoff.feed.b90c5a7`. Simulator: `EF198926-1958-489D-A7A4-9196E454DD3A`.

## Decisions

The supplied relative times have no absolute date, so they are anchored to the first launch and saved. Refresh creates three posts from rotating local templates, each with a new durable ID. Composed posts use the author “You.” All writes are serialized and saved atomically to protect likes and posts when actions overlap.

## What was hard

Exact's native layout needed explicit horizontal strip sizing to keep large carousel text and offscreen cards correctly bounded. `max-content` and rem units inside CSS math were unavailable; viewport widths and an explicit strip extent worked. `overflow-wrap="break-word"` handles an unusually long word at the largest accessibility size while preserving normal word wrapping.

A zero-like button needed an explicit 44-point minimum width. Real pull gestures also needed to start below the navigation bar: Exact's list frame includes the area beneath that bar.

The early prebuild raced the media copy and rejected its asset receipt. Rebuilding after the assets settled fixed it. Agent file operations require a named storage sandbox on web.

## Validation

The contract and TypeScript compile. Six interaction tests cover feed order, like toggling and relaunch, compose validation/cancellation/persistence, post detail, photo closing/swiping, and carousel image navigation. Additional data checks cover rapid concurrent likes, initially liked posts, overlapping saves, refresh counts/unique IDs, and composing immediately after refresh. The last case caught and fixed a timestamp ordering edge case.

Web verification passes cleanly at phone and wide widths, in light/dark mode and at 53-point equivalent text. Final `verify ios` is clean: six tests pass, native light/dark checks have no findings, all eight largest-text screens fit, and the real-touch scrolling probe reports “like UIKit.” Real simulator pulls add exactly three posts, and real downward photo swipes dismiss the viewer.

## Toolkit limitations and improvements

Earlier strip layouts produced momentum/gesture hand-off warnings. After the explicit strip sizing fix, the final probe reports “like UIKit.” The machine was heavily loaded, so the probe declined to judge frame timing; its physics checks passed.

A documented, tested media-feed pattern, clearer horizontal intrinsic-size guidance, CSS math with rem support, and a read-only documentation command without a Cargo lock would make this faster. No requested app feature was omitted.

Final build reinstalled with `bun exact.mjs ios --sim EF198926-1958-489D-A7A4-9196E454DD3A` and opened on the default local store with 400 posts. Nothing remains unfinished.
