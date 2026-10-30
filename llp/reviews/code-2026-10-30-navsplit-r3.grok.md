# Code review: the web navigation split's candidate search, round 3 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `8cb7da100d`.
- **Method:** one brief (sha256 `868c1f0ef22be5d0725f5c7928797c1d3fb0544dd21bb4239aaf65394ffef88e`), shared by both reviewers, blind to the other review. The author (Claude, x2-perf4's workers) is not a reviewer.
- **Transcription:** the final message, unedited.

The round-2 items are fixed, and `answeredBy` stops on the same candidate the search returns. The list search matches the old walk for a DOM it does not change.

**Blocking**

none

**Non-blocking**

none

Round 2, all resolved:

- Coverage stops at the list the search returns. `answeredBy` is `pushingList(route, null)?.list` (`host/web/tests/split-search.test.mjs:73`), and `branches` breaks on that element after counting it (`split-search.test.mjs:120`, called at `:163`).
- Open popovers are what the popover decision measures. The generator shows about 60% (`split-search.test.mjs:90`, `:99-100`); the fixed cases call `showPopover()` and throw unless the computed display is not `none` (`:280-281`). The classifier counts only `:popover-open` (`:108`). A closed popover stays `closedPopover`, so the user agent's `display: none` does not inflate `popover` or `classNone`. Deleting `hasAttribute("popover")` from `excluded` (`host/web/navigation.js:33`) makes those shown popovers admissible, so the fixed cases answer A instead of B and the random routes diverge from the walk copy, which still has the clause (`split-search.test.mjs:33`).
- The guarantee is the walk's answer on a DOM the search leaves alone; a node removed during the search is skipped (`navigation.js:49-52`). The removal test detaches a nested list both miss, and states that a direct child taken off the route is outside the guarantee (`split-search.test.mjs:295-314`).
- The live test emulates `prefers-color-scheme: light` before the first assertion (`split-search.test.mjs:205`). Push and pop both expect the root list to stay `L2` (`:219-220`).
- The ownership note is on the ancestor test (`navigation.js:59`).

`answeredBy` matches the stop, including the two cases that look like they might not. A pushing row returns immediately and does not read `retained` (`navigation.js:62`), so `pushingList(route, null)` is the same element as `pushingList(route, retained)` whenever any row pushes. The retained-empty fallback only assigns `empty` and keeps scanning (`navigation.js:63-65`); with `retained === null` that assignment never runs, `answeredBy` is null, and `el === null` never breaks, so candidates after the empty list stay in the tally. That is the set the search actually walked. A null answer is the same full scan. The fallback is not a stop, and the classifier does not treat it as one.

Against the old walk (`git show HEAD~3:host/web/navigation.js`, the `pushingList` at the parent), the search is the same preorder over the light DOM: `querySelectorAll("[data-exact-list]")` in tree order, prune on `navigationKey` / `excluded` on the way up including self, do not enter a non-sideways list, skip a sideways list but still take lists under it, first pushing rows win, otherwise the retained empty list. `passable` and `sideways` are memoized per call (`navigation.js:54-56`); `listRows` still reads `excluded` fresh, as the walk did. `routeMatches` (`host/web-js/router.js:189`) and `exact_route_match` (`host/web/glue.js:1499`) do not change the DOM, so the direct-child hole is not reachable in production. The fixed cases name the answer for each branch, including the retained empty list (`split-search.test.mjs:255-292`); the random run records mismatches and requires all 16 decision counts, both widths, and every answer kind (`:183-193`). Those fail if owned, reachability, or sideways is dropped. Scope (shadow, frame, slotted) checks both sides and the expected ids (`:247-248`).

APPROVE
