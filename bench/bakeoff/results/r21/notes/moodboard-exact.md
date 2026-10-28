# Moodboard build notes

Completed with Exact and installed as `dev.bakeoff.moodboard.d939e9` on simulator `486FD498-0E3E-4B64-8E4B-152BD8265EE7`. The final app is left on the main grid.

## What works

- All 600 bundled records from `data/tiles.json`, displayed as square HSL-colour tiles with centred symbols, three per row, newest first.
- All Boards plus each of the ten boards through a native chooser, independently in Browse and Liked.
- Native detail navigation with title, board, date, left/right swiping, and accessible previous/next controls. Browsing respects the active board and Liked filter.
- Like/unlike, original liked records, and the Liked grid.
- Multiple selection, select/deselect all, bulk like, and permanent bulk deletion with confirmation.
- Likes and deletions saved atomically in the app library file and retained across relaunch, including an entirely deleted library.
- Native tabs, navigation bars, dark mode, accessible tile labels, scalable symbols, and scrollable selection controls at large text sizes.

## Validation

`bun exact.mjs verify`: seven tests passed, no layout findings in light/dark mode or at the largest text size.

`EXACT_SIM=486FD498-0E3E-4B64-8E4B-152BD8265EE7 bun exact.mjs verify ios`: seven tests passed, no light/dark findings, and nothing spilled at accessibility-extra-extra-extra-large (53-point body).

After the last badge-tint polish, rebuilt and installed with `bun exact.mjs ios`. A final real UIKit touch drive selected a tile, checked selection contrast in both appearances, changed boards, swiped from tile 113 to its neighbour 140, and returned to the main grid. Final screenshots are in `.exact/final-*.png`.

## What was hard and what I looked up

- Exact's guide, tabs starter, grid sizing, native chooser, persistence, navigation, pan events, and iOS driving documentation were needed.
- An initial quadratic neighbour-list intersection hit Exact's iteration limit at 600 records. A single-pass filtered list resolved it.
- Pan events provide incremental movement. The gesture now accumulates movement and navigates on release. The web touch surface also needed `touch-action="pan-y"`.
- Large text exposed header compression, a custom menu overlap, and oversized toolbar content. Wrapping headers, using the native chooser, and scrolling constrained toolbar content resolved these without shrinking text.
- The native test harness starts at Browse after relaunch; persistence tests explicitly reopen Liked.
- Updated plans can run in the agent before they are in the standalone app. The final installation was rebuilt to ensure normal launches use the finished plan.
- Screenshot review caught selection marks inheriting a tile's foreground colour. Their badges now use the system link tint consistently.

## Decisions and assumptions

- Same-date records use ascending ID as a stable tie-breaker.
- The supplied 62 liked tiles are liked on first launch.
- Swipes follow the current filtered collection. An unliked tile remains visible while its detail is open, allowing continued browsing.
- Deletion is permanent and removes the tile from both tabs. Board filters and transient selections reset on a fresh launch; likes and deletions persist.

## Unfinished

None of the requested features remain unfinished.

## What would have made this faster

An Exact starter for a three-column collection with selection, a pan example that includes release handling and touch-action, and an iteration-limit diagnostic that points at the costly expression would have shortened the work. A clearer distinction between an agent's updated plan and the installed standalone plan would also help.
