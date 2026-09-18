# Code review: LLP 1027.004 slices 1–2, 2026-09-18 (astra)

- **Family:** OpenAI, `codex exec -m gpt-6-astra -c model_reasoning_effort=xhigh -s read-only`. Each round was a fresh session in a detached worktree at the reviewed SHA. Implementation was also Astra, in separate sessions that never reviewed their own code.
- **Method:** mutually blind to the other family (Grok 4.6 xhigh), which failed every code round with `402 Grok Build usage balance exhausted` (see the grok file). Each brief is identical to the one given to Grok.
- **Rounds:** S1 at `76b07bc` (diff 765e369..76b07bc, brief sha256 `16abeef750e8a04d77ae66dbdd0de8f4ba1ca6e6048abcae00b62402653629e5`); S2 at `33f5944` (diff 76b07bc..33f5944, brief sha256 `c1330f3cd4b2bd51ae834058b576545b5d8654eb6129081edae62afd6b97f89f`); final at `2c35b68` (diff 765e369..2c35b68, brief sha256 `807f4f88e243e5d5553f6a5ec085b683208afe4db28906540f0a7e1337183dd5`).
- **Disposition:** after each round.

---

## Round S1 at 76b07bc (verbatim)

1. **HIGH — [host/apple/src/abi.rs:908](/Users/ccheever/projects/exact2-bounded-20260918/review-s1/host/apple/src/abi.rs:908): Async edge requests never reach the executor.** Edge actions can now enqueue resource requests, but this entrypoint still writes the output directly, bypassing `emit()`, which submits requests. On an idle Apple session, scrolling to an edge can leave loading pending indefinitely until an unrelated event invokes `emit()`. **Fix:** submit committed edge requests through the normal executor path; add an ABI-level async edge test that completes without another user event.

2. **HIGH — [runner/src/instance/collection/mod.rs:558](/Users/ccheever/projects/exact2-bounded-20260918/review-s1/runner/src/instance/collection/mod.rs:558): The deferred second edge can remain armed indefinitely.** Populate a list with established geometry with two 32-pixel rows inside a 320-pixel scrollport. Both edges qualify, but a no-op `reachstart` leaves `reachend` awaiting another report. Web (`navigation.js:348`) and Apple (`Collection.swift:284`) suppress that report because geometry, wrappers and epochs are unchanged; revision changes alone are ignored. The list cannot scroll, so `reachend` never fires. The existing test hides this by manually sending another report. **Fix:** ensure a deferred qualifying edge gets a bounded follow-up callback despite unchanged geometry, and test this through both host adapters.

VERDICT: MATERIAL FINDINGS
**Disposition (Claude):**
1. Confirmed: Apple `collection_feedback` bypassed `emit`, while web and Linux drain requests. Fixed in d5b3de5 with an ABI regression test.
2. Confirmed: nothing sends another report once geometry is unchanged. Fixed in 8d6c202: both edges may dispatch in one call when membership is unchanged, and LLP 1027.004 D3 was updated.

## Round S2 at 33f5944 (verbatim)

1. **HIGH — [apps/messages/app.ts:206](/Users/ccheever/projects/exact2-bounded-20260918/review-s2/apps/messages/app.ts:206)**: A non-empty cursor can still follow arrivals. With 1,000 messages, navigate earlier, then later twice. An incoming reply changes the window’s start from row 800 to 801 because `rows.length - windowSize` increases. The first displayed message disappears while the reader is in history mode, violating D2. **Fix:** retain a stable start or normalized center for explicit cursors instead of recalculating it from the growing tail; test arrivals after paging back to the tail.

2. **HIGH — [apps/messages/app.ts:41](/Users/ccheever/projects/exact2-bounded-20260918/review-s2/apps/messages/app.ts:41)**: Equal `order` values break consistency during recovery. Two offline devices can create messages A and B with the same order. With a later unrelated message present, deleting and recovering B inserts it before A in the transcript, but after A in their reply sheet. The `<=` outgoing-index update also assigns the delivery label to B although A is now the last outgoing transcript row. Reproduced with merged device snapshots. **Fix:** use consistent tie ordering across transcript insertion, reply insertion, outgoing indexes, and selection sorting; add a synchronized equal-order recovery test.

3. **MEDIUM — [apps/messages/apple/src/window_tests.rs:256](/Users/ccheever/projects/exact2-bounded-20260918/review-s2/apps/messages/apple/src/window_tests.rs:256)**: The required whole-thread-walk guard is absent. Timings are sorted and printed, but never compared or asserted. An implementation that scans or decorates all 25,000 messages before returning 200 can pass these assertions, leaving S2.1 unproven. **Fix:** retain the minimum repeated warm timing at 1,000 and 25,000 rows and assert the specified scaling bound, while keeping diagnostic reporting separate.

VERDICT: MATERIAL FINDINGS
**Disposition (Claude):**
1. Confirmed: a tail-clamped cursor window dropped its first row on each arrival. Fixed in 2c35b68, and LLP 1027.004 D1 now says a non-empty cursor's window never slides. Messages stress was aligned in 04d9b47.
2. Confirmed: the separate reply and outgoing indexes ordered ties differently from `restore`. Fixed in 2c35b68 with one `(order, id)` comparator.
3. Accepted. Measurements showed a timing ratio separates the cases (whole-thread walk 6.65× against a 3× bound, bounded 0.98×). Added in 2c35b68.

## Round final at 2c35b68 (verbatim)

1. **HIGH — [apps/messages/app.ts:210](/Users/ccheever/projects/exact2-bounded-20260918/review-final/apps/messages/app.ts:210): Cursors omit the ordering tie breaker.** Rows use `(order, id)`, but cursors contain only `order`. Reproduced with 201 messages sharing order `100`: the first window returns rows 0–199; repeatedly requesting its `later` cursor returns exactly the same window with `hasLater=true`. Fix: encode and binary-search the complete `(order, id)` position; test traversal and deleted anchors across ties larger than K.

2. **HIGH — [host/web/navigation.js:459](/Users/ccheever/projects/exact2-bounded-20260918/review-final/host/web/navigation.js:459): Membership changes can exhaust feedback permanently.** Edge-action commits occur while `delivering=true`, so they cannot replenish the collection’s two-pass budget. After two membership-changing reports, the next membership receives no measurements or deferred edge dispatch. Reproduced with the actual controller: two reports, a third unreported membership, and zero pending callbacks. A short, non-scrollable list cannot generate another scroll stimulus. Fix: carry required feedback for new membership into a subsequent bounded callback; test this through the browser controller rather than manually supplying every report.

3. **HIGH — [runner/src/runner/collection.rs:103](/Users/ccheever/projects/exact2-bounded-20260918/review-final/runner/src/runner/collection.rs:103): Startup feedback can consume an edge before its data executor loads.** Browser collection feedback starts before deferred activation. A baked TypeScript list whose edge handler changes resource arguments therefore queries an unloaded executor, refuses the action, and leaves the edge disarmed. Activation with unchanged endpoint keys does not re-arm it, so the initial load is lost until the reader leaves and re-enters the edge. Fix: defer source-dependent edge dispatch without consuming its arming state, and resume it after activation.

4. **MEDIUM — [host/web/src/host.rs:405](/Users/ccheever/projects/exact2-bounded-20260918/review-final/host/web/src/host.rs:405): The browser still treats an action refusal as rejected geometry.** The wrapper puts a post-commit action error in `batch.error`; `glue.js:23` consequently returns `false` to the collection controller. That controller retains its previous pin reservation despite the runner having committed the release. During transfer between collections, this can suppress the replacement pin’s report when no retry remains scheduled. Fix: distinguish feedback acceptance from edge-action failure through the controller boundary, and test a pin transfer with a refused edge action.

5. **MEDIUM — [apps/messages/apple/src/window_tests.rs:33](/Users/ccheever/projects/exact2-bounded-20260918/review-final/apps/messages/apple/src/window_tests.rs:33): The new heavy tests leave neighboring correctness tests exposed to wall-clock failures.** Only `window_tests::Model` disables the production deadline. The existing Messages and Snapback tests share this parallel test binary and retain the 100 ms per-call deadline while the new tests seed multiple 25,000-row models. Under contention, correct calls can become `Unavailable` and fail the blocking suite. The read-only subset passed locally; the contention risk remains. Fix: disable production deadlines in all functional test fixtures or isolate the heavy tests, retaining the explicit interleaved ratio guard.

6. **MEDIUM — [apps/messages/apple/src/window_tests.rs:201](/Users/ccheever/projects/exact2-bounded-20260918/review-final/apps/messages/apple/src/window_tests.rs:201): The 25,000-row decoration acceptance is skipped.** Despite the test’s name, traversal and overlapping-window decoration checks run only when `n <= 1_000`. At 25,000, this test checks a head reply window; the timing test checks tail answers. Incorrect middle-window decoration above 1,000 rows can therefore pass. Fix: exercise overlapping head, middle and tail windows at 25,000 rows and compare their decorated fields against whole-thread expectations.

VERDICT: MATERIAL FINDINGS
**Disposition (Claude):** all six were routed for reproduction and fixing: 1, 5 and 6 to lane S2; 2, 3 and 4 to lane S1. For 3, a declared limitation is the fallback if the fix is not small. The outcomes follow below.
