# Code review: the web navigation split's candidate search, round 2 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `6724658dcb`.
- **Method:** one brief (sha256 `62e09e4190f07cccec264f4aaaa177c5ecd482d28d8bc24ed5f1de02e49730e4`), shared by both reviewers, blind to the other review. The author (Claude, x2-perf4's workers) is not a reviewer.
- **Transcription:** the final message, unedited.

The ancestor walk is correct on a stable tree, and the round-1 detached-list bug is fixed. The new search matches the old walk for any DOM the search does not change. Production matchers do not change it.

The loop at `host/web/navigation.js:59` stops when `up === route`, so the route's own attributes are never read (the old walk never read them either). A direct child still under the route has `parentElement === route`, so the loop body does not run and the child is judged only by its own passable and sideways checks. `!up` is tested before `passable(up)` or `hasAttribute`, so a chain that falls off the document skips that candidate and does not throw. `listRows` only reads. `linked` calls the injected matcher: the JS one is `matchRoute(canonical(href))` at `host/web-js/router.js:135` (passed through `document.js:79`) and `routeMatches` at `router.js:189`, and the wasm one is `exact_route_match` at `host/web/glue.js:1499` (`writeIn` plus an `&self` table read in `abi.rs:434`). None of those touch the DOM.

Round 1: the detached-list skip, the per-route element budget (`split-search.test.mjs:75` and `:185`, `empty === 0` at both widths), the iframe list assertion (`:234`), and the push/pop wording are in place. The fixed cases really do force the branch they name, except the popover ones below. The counters do not.

**Blocking**

none

**Non-blocking**

1. `host/web/tests/split-search.test.mjs:103` — `branches` classifies every `data-exact-list` with its own predicates and never observes `pushingList`. The comment at `:100` says this is generator coverage, not the answer. A candidate after one that already returns is still counted, so `> 0` at `:186` can pass without that search branch running. The fixed cases are what actually force the branches. Point the counters at decisions the search took (return the reason from `pushingList`, or stop the classifier at the candidate the search returns).

2. `host/web/tests/split-search.test.mjs:251` and `:256` — a closed popover is `display: none` in Chrome's user-agent stylesheet, and `excluded` (`navigation.js:33`) checks that too. Deleting the `hasAttribute("popover")` clause still leaves both the ancestor and self popover cases, and the `self:popover` / `ancestor:popover` counters, green. That clause is what excludes an open popover. `showPopover()` on the popover element and expect the list inside it to be skipped while its computed display is not `none`.

3. `host/web/navigation.js:59` — the direct-child residue is not "only null". `querySelectorAll` has already listed later siblings, and the old walk has already stacked `route.children`. If an earlier list's `linked` removes a later direct child that would have pushed, and a list after that one also pushes, the search skips the detached child and returns the later live list; the walk returns the detached element and stops. Nested removal is locked at `split-search.test.mjs:290` (both null). This cannot happen with the production matchers above, and skipping a detached node is the behavior you want, so it is acceptable under the guarantee at `navigation.js:52`. The guarantee should say that a removed direct child is skipped even when a later list then answers.

4. `host/web/tests/split-search.test.mjs:213` — the push and pop steps were relabelled, and `same` still only checks that the search and the copied walk agree. It never checks that the list is still `L2`. Assert the list id there the same way the surrounding steps do.

5. `host/web/navigation.js:58` — the ownership comment sits on the self-`passable` `continue`. Ownership is the ancestor test on `:59` (`data-exact-list` and not sideways). Move the comment onto that condition.
