# Code review: the web navigation split's candidate search, round 1 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `a96067411b`.
- **Method:** one brief (sha256 `9e8901bdbeef7385e533be06763f83774d5ae51f6aa260d7f951a18c5ee1592d`), shared by both reviewers, blind to the other review. The author (Claude, x2-perf4's workers) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. [host/web/navigation.js:57](host/web/navigation.js:57): **The unconditional equivalence claim fails under synchronous mutation.** Consider root children `A` and `W`: `A` is a list containing a link; `W` contains pushing list `B`. Have the `matchesRoute` callback invoked for A’s link remove `B` and return false. The old walk subsequently visits `W`, sees no children, and returns null. The new static NodeList still contains `B`; its null `parentElement` passes the ancestor loop, so the search returns the detached list.

   The current production matchers appear pure, and native `listRows` does not yield to timers or MutationObservers. This blocks the stated “every DOM” guarantee, rather than demonstrating ordinary commits interrupting a search. Resolve this by explicitly restricting the guarantee to nonmutating searches, or preserving the old enumeration timing and adding mutation comparisons on independent fixtures. Merely checking containment would not address newly inserted candidates.

**Non-blocking**

1. [host/web/tests/split-search.test.mjs:81](host/web/tests/split-search.test.mjs:81), [line 158](host/web/tests/split-search.test.mjs:158): **Coverage assertions establish answer categories, not every prune/admit/ownership case.** The generator can construct the requested cases, but an earlier winner can prevent their examination. Also, `tid` spans both widths; if its global cap is reached, subsequent empty routes still increment `trees`. Add deterministic cases or exercised-case counters, and use a per-route generation budget.

2. [host/web/tests/split-search.test.mjs:187](host/web/tests/split-search.test.mjs:187): **The push/pop comparisons are effectively vacuous as navigation coverage.** They append/remove a later route, while both compared functions destructure only `[route]`. Drive selection and projection, as the existing wide tests do, or describe these assertions as confirming independence from later routes.

3. [host/web/navigation.js:52](host/web/navigation.js:52): **“Once an element a search” overstates the style-read reduction.** `passable` reads computed style through `excluded`, and `sideways` reads it again; `listRows` also performs its own reads. Describe this as once per memoized predicate during candidate discovery.

Otherwise, by inspection:

- For a stable DOM, root exclusion, nested-list ownership—including unreachable outer lists—sideways traversal, ordering, and retained-empty fallback match the old walk.
- The Maps are strictly per-call. Their values can differ from fresh reads after synchronous mutation; they cannot become stale across projections.
- Comparisons preserve unique element identity through test IDs, row order, `press`, `href`, and width. The shadow/iframe/slotted-child scope fixture is sound; asserting the iframe’s list exists would strengthen its negative assertion.
- `splitOf` and its callers retain their behavior; the exports introduce no execution changes.

No files edited, network used, or tests/builds run.
