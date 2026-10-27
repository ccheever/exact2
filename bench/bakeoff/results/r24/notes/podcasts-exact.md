# Podcasts build notes

Finished and installed as **Podcasts**, bundle ID `dev.bakeoff.podcasts.bdca48e`, on simulator `CEDFB9C9-FF04-4617-B156-94C56B83CA86`.

## Implemented

- All 420 supplied episodes, bundled offline from `data/episodes.json`, sorted newest first with a deterministic ID tie-breaker.
- Episode search, an unplayed-only switch, and played/unplayed controls directly in each list row.
- All 12 shows, with live episode and unplayed counts, plus each show's episode list.
- Episode details, summary, date, duration, Play/Pause, and a visible playing state.
- Up Next with duplicate prevention, removal, swipe deletion, and accessible Earlier/Later ordering controls. Boundary moves are disabled.
- Played changes, queue order, and the filter preference persist across launches. Original catalog played flags are preserved until changed.
- Native navigation, tabs, search, system colors, dark mode, Dynamic Type, and labeled controls.

## Validation

- `bun exact.mjs verify`: clean; 7 tests passed, light/dark checks passed, and nothing spilled on 15 screens at the largest text size.
- `bun exact.mjs verify ios --sim CEDFB9C9-FF04-4617-B156-94C56B83CA86`: clean; 6 tests passed, light/dark checks passed, and nothing spilled on 8 screens at accessibility-extra-extra-extra-large.
- The iPhone scrolling check reported UIKit-like physics. Machine load was high, so the verifier did not judge timing; it recorded no dropped frames at the two touch lifts.
- Additional simulator drives checked populated queue ordering, playback, and episode details.
- Final `bun exact.mjs ios --sim CEDFB9C9-FF04-4617-B156-94C56B83CA86` completed and installed the finished app.

## What was hard / looked up

Exact's TypeScript-like view language differs from TypeScript in array indexing, flattening, action naming, and control syntax. I looked up its tab pattern, component props, persistent state, switch syntax, list deletion, symbol roles, and native test commands. Compiler diagnostics were useful.

Large-text checks caught a tab label and detail-title spacing issue. The final tab uses “Browse,” while the screen title remains “Episodes.” Episode action buttons have compact, 44-point targets.

Native relaunch returns to the root screen, unlike the web driver's retained route. Persistence tests now explicitly reopen the episode after relaunch. Explicit data settling after search and navigation resolved a native row-activation test failure.

## Decisions and completion

Queue reordering uses visible Earlier/Later controls rather than requiring drag gestures. Starting dates are treated as calendar dates in UTC to avoid timezone shifts. Play marks an episode played; Pause leaves it played. Playing another episode replaces the current playing state.

Nothing in the requested specification remains unfinished. Audio is intentionally simulated as requested.

## What would make this faster

A compact TypeScript-to-Contract syntax comparison, clearer native relaunch documentation, and deterministic native layout readiness after search/navigation would reduce iteration. Starting the native prebuild early helped. Verification should run against unchanged source; editing during it invalidates that run.
