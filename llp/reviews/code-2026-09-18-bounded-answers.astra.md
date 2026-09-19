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
**Disposition (Claude):** all six reproduced and were fixed. 1, 5 and 6 went to lane S2 (3448148: `(order, id)` cursors; heavy tests moved to their own test binary; 604 decorated-row comparisons at 25,000). 2, 3 and 4 went to lane S1 (beec42b: feedback continues after edge-action commits, refused edges stay armed and are re-evaluated after activation, acceptance is reported separately from the action error).

## Round final-2 at beec42b (verbatim; brief sha256 `c7154bf5a18c41d7af6f1a9c5c061f696d69cd5ec2c4c06f88f7624b2af90a3a`, diff 765e369..beec42b)

1. **HIGH — [runner/src/instance/collection/mod.rs:234](/Users/ccheever/projects/exact2-bounded-20260918/review-final2/runner/src/instance/collection/mod.rs:234): Bidirectional pagination can loop indefinitely without scrolling.** When both endpoints fit inside the geometric window, reaching rows `0..199` makes `reachstart` a no-op; `reachend` then loads `99..298`. Changed endpoints re-arm both handlers, so the next report loads `0..199` again. Fresh measurement epochs keep host callbacks running indefinitely. The tiny-row tests only advance membership in one direction and miss this cycle. **Fix:** prevent an automatic window replacement from immediately requesting the inverse window while both edges remain visible; add a bidirectional tiny-row test that reaches an endpoint and becomes idle.

2. **HIGH — [runner/src/runner/collection.rs:93](/Users/ccheever/projects/exact2-bounded-20260918/review-final2/runner/src/runner/collection.rs:93): Deferred answers let the second edge supersede the first window request.** With both edges eligible and an asynchronous window source, `reachstart` changes the cursor and queues an earlier request, but the kept answer leaves membership unchanged. The membership check therefore permits `reachend`, which reads the old answer’s `later` cursor and queues the opposite request. Both requests reach the host, but the earlier ticket has already been superseded and its answer is discarded. **Fix:** also defer the second edge when the first changes the collection’s resource arguments or starts a pending window request; reconsider it after settlement. Test both handlers together with an `Answer::Later` source.

VERDICT: MATERIAL FINDINGS
**Disposition (Claude):** both findings are confirmed as defects in the D3 arming rules the orchestrator specified. Re-arming on a key change oscillates when the whole window is visible, and the "membership unchanged" test lets a second edge supersede an async request. They went to lane S1 as its third and last review-fix round: re-arm by geometry only, dispatch the second edge only after a pure no-op, and guarantee a deferred edge a follow-up evaluation.

Outcome of round final-2: both fixed in 9f4498c. Edges re-arm by geometry only, and the second edge dispatches only after a no-op, with one follow-up after settlement.

## Round final-3 at e08e25a (verbatim; brief sha256 `d6fdd7ff513a63d4cebcad051982215289af704366387fffc5688e9d8190a539`, diff 765e369..e08e25a)

1. **HIGH — [runner/src/instance/collection/mod.rs:234](/Users/ccheever/projects/exact2-bounded-20260918/review-final3/runner/src/instance/collection/mod.rs:234): Provisional heights can sustain an edge-dispatch storm.** With 200 one-pixel rows in a 320px scrollport, a cursor shift introduces unmeasured rows estimated at 32px. Anchor restoration and `geometric_edges()` interpret that temporary expansion as an edge exit and re-arm it. Subsequent measurements fire the opposite edge, whose replacement repeats the process. Earlier/later windows can oscillate without scrolling, violating D3’s all-fitting-window idle requirement. The existing two/eight-row tests miss this because even their estimates fit. **Fix:** prevent replacement estimates from manufacturing re-arming; require an established geometric exit, and add a 200-row regression using measured feedback through settlement.

2. **HIGH — [apps/messages/app.ts:212](/Users/ccheever/projects/exact2-bounded-20260918/review-final3/apps/messages/app.ts:212): Arrivals slide a non-empty cursor window after its anchor becomes past-end.** Retain a cursor, delete its anchor and all later messages, then receive an incoming reply. The fallback initially centers on the last surviving row; the next answer centers on the new arrival instead. Reproducing this in the producer changed a 101-row window’s first message and discarded it, rather than appending a 102nd row. This breaks D1’s stable history window. **Fix:** retain the resolved surviving anchor when falling back, so later arrivals cannot recenter that cursor; test deletion followed by `advanceReplies`, asserting unchanged first row and appended arrival.

VERDICT: MATERIAL FINDINGS
**Disposition (Claude):**
1. Confirmed, and **declared rather than fixed**. Lane S1 had used its three review-fix rounds (RULES: fix loops get three rounds). The oscillation needs a fully visible window of rows far below the 32 px estimate, and no current consumer comes near that. It is recorded in LLP 1027.004 D5 and `QUEUE.md` (e61369b) as a question for Charlie: authorize one more round or leave it queued.
2. Confirmed. It went to lane S2 as its third and last fix round: a cursor resolves to the last surviving row at or before its position, which arrivals can never capture.

Outcome of round final-3, finding 2: fixed in 574955e (cursor resolves to the predecessor; reproduced first: an arrival had changed the first row from `sent-711` to `sent-712`).

## Round delta at 04a08d0 (verbatim; brief sha256 `fdbb2a529cccc41c895b93e418037f6cc53ab3b3789d1185fc6af0f5bfe7d45e`, diff e08e25a..04a08d0, code paths only)

VERDICT: NO MATERIAL FINDING
**Summary (Claude):** the last code review, of the delta at 04a08d0, found nothing material. The only open review finding is final-3 #1, declared in LLP 1027.004 D5 and `QUEUE.md`. Grok never completed a code review (see the grok file), so these slices have single-family (Astra) code review only.
