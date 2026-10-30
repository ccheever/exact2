# Code review: the web navigation split's candidate search, round 1 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `a96067411b`.
- **Method:** one brief (sha256 `9e8901bdbeef7385e533be06763f83774d5ae51f6aa260d7f951a18c5ee1592d`), shared by both reviewers, blind to the other review. The author (Claude, x2-perf4's workers) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

none

**Non-blocking**

1. `host/web/tests/split-search.test.mjs:73` and `:81` — `tid` lives for the whole page and `grow` stops at `tid > 100000`. The 390px pass creates 54479 elements, so the 1280px pass dies at seed 352: seeds 352–400 are empty routes. Both sides then return no list, `trees` is still 400, and `found` / `emptyRetained` / `wide` still clear on the earlier seeds, so those 49 desktop seeds pass without a DOM. Reset `tid` at the start of `grow`. Per-tree `size` and `depth > 7` already bound a tree.

2. `host/web/tests/split-search.test.mjs:157` — the counters only require a pushing list, a retained empty list, no list, and a non-zero desktop width. They do not require sideways-by-class, inline flex row, estimated item width, a nested list under an admitted list, a nested list under a sideways list, a `display:none` ancestor, a popover, or `data-exiting`. On these seeds those shapes do show up and do change the answer (dropping the owned check, ancestor reachability, or the sideways skip disagrees with the reference walk; dropping only the estimated-width term disagrees 65 times). A generator change can drop any one of them and stay green. Count each shape while building and expect each count to be greater than 0.

Equivalence holds for every DOM. `route.querySelectorAll("[data-exact-list]")` is the same light-DOM preorder as the old `children` stack (`host/web/navigation.js:57` versus the walk in `HEAD^`). The route itself is in neither: its `navigationKey`, `popover`, `data-exiting`, `display`, and a `data-exact-list` on the route do not affect the answer. A candidate is admitted only when it and every ancestor up to but not including the route are passable, and no ancestor is a non-sideways list (`:59`). That is the old prune: an admitted list owns its descendants even when its rows do not push; a sideways list does not; an unreachable list (including an unreachable non-sideways list wrapped around another list) is not visited, because `!passable(up)` fails before the owned test. The first pushing list returns; otherwise the retained list with zero rows is kept and later non-answers do not clear it (`:62-63`). `excluded()` is the same predicate, read on the same path; `listRows` (`:38-47`) is unchanged and still crosses `navigationKey`. Slotted light children are in both walks; a shadow tree and a frame document are in neither. `querySelectorAll` is static and the old walk already snapshotted `children` before `listRows`; `listRows` does not mutate, so the snapshot cannot diverge.

The memos are the two `Map`s created inside `pushingList` (`:54`), thrown away when the call returns. `passable` and `sideways` return booleans, so a cached `false` is not treated as missing. Nothing in the search writes attributes or style, and `display` / `flex-direction` are fixed once computed style is read, so a fresh read in that call would match.

`splitOf` (`:68-79`) is the old function plus `export`. `project` still calls `splitOf(routes)` (`:435`). The new `retained` argument defaults to `splitLists.get(route)`, which is the old lookup. Exporting `pushingList` and `splitOf` does not change other importers; `glue.js` imports neither.
