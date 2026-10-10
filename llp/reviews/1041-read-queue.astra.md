**The diagnosis is correct. I recommend `Dispatch::Again`, the existing bounded read backlog, and separate accounting for outstanding native work and undrained settlements. I would revise the amendment before implementation: its overflow retry mechanism, refusal ordering, and memory accounting are not yet sufficiently specified.**

I reviewed checkout `6be444f8c` without editing. I attempted the requested Cargo command, but the read-only sandbox rejected creation of Cargo’s target directory before tests ran. The findings below are code analysis; I did not independently reproduce the reported rows or timings.

The failure chain is supported by the code:

- A shared-promise waiter becomes `WAITING` in [js/src/lib.rs:687](/private/tmp/bsky4-rv/wt-rq/js/src/lib.rs:687). `Dispatch::Held` parks its request in the host, outside executor admission ([source.rs:48](/private/tmp/bsky4-rv/wt-rq/js/src/source.rs:48), [abi.rs:302](/private/tmp/bsky4-rv/wt-rq/host/apple/src/abi.rs:302)).
- A re-ask is currently an opaque `Work::Now` closure returning an empty response. `wake` releases it after module progress; `release` considers every waiter ([turns.rs:10](/private/tmp/bsky4-rv/wt-rq/js/src/turns.rs:10), [source.rs:69](/private/tmp/bsky4-rv/wt-rq/js/src/source.rs:69)). New answers advance progress; a `WAITING` re-ask itself does not ([lib.rs:774](/private/tmp/bsky4-rv/wt-rq/js/src/lib.rs:774), [lib.rs:900](/private/tmp/bsky4-rv/wt-rq/js/src/lib.rs:900)).
- Opaque work receives the 16-ticket admission threshold. Plain GET/HEAD already receive 128. Refusal raises the ordered barrier ([executor_core.rs:333](/private/tmp/bsky4-rv/wt-rq/host/apple/src/executor_core.rs:333)).
- A refused re-ask removes the parked call and returns an error. The runner clears pending and retains the resource’s last value, without automatically invoking it again ([lib.rs:896](/private/tmp/bsky4-rv/wt-rq/js/src/lib.rs:896), [admission.rs:68](/private/tmp/bsky4-rv/wt-rq/runner/src/runner/admission.rs:68)).
- The subsequent shared-load fetch can wait behind the refusal and resume after settlement. That supports the draft’s diagnosis of permanently failed answers, rather than executor-slot deadlock ([executor_core.rs:290](/private/tmp/bsky4-rv/wt-rq/host/apple/src/executor_core.rs:290), [abi.rs:244](/private/tmp/bsky4-rv/wt-rq/host/apple/src/abi.rs:244)).

The description “counted nowhere” should say **“not charged to executor admission.”** Parked calls still retain runner state, host requests, arguments, and JavaScript objects.

My recommended design is:

1. Keep the current GET/HEAD classification and FIFO backlog. Introduce `Again` specifically for an existing call’s settlement check.
2. When a waiter becomes eligible, register its existing runner ticket as an already-complete ordered marker. Give it no worker job or native-work reservation. Fulfill it through the ordinary bounded pump.
3. Separate **outstanding native work** from **retained ordered tickets**. Writes encounter the 16 threshold based on uncompleted native work, including queued reads; `Again` and completed results consume only settlement-window and memory capacity. Actual reads retain the larger bounded backlog.
4. At settlement-window exhaustion, keep the same call pending with one capacity-blocked readiness record. Retry that record on capacity release, without invoking the source anew or redispatching a consumed continuation.
5. Release native-work accounting at actual completion, but retain result accounting until drain/drop. Preserve refusal settlement barriers independently of that accounting.

This follows the draft’s “queued or running” interpretation of a slot. The 16 threshold is an admission-pressure limit, not sixteen HTTP workers.

There are three **blockers** in the proposed text.

1. **The accounting definitions conflict, and changing the existing counter would break the refusal fence.**

   The draft says both “sixteen counts all ordered tickets” and “completed results/re-asks no longer count.” Those are different policies ([RFC:613](/private/tmp/bsky4-rv/wt-rq/llp/1041-graceful-overload.rfc.md:613), [RFC:648](/private/tmp/bsky4-rv/wt-rq/llp/1041-graceful-overload.rfc.md:648)).

   More critically, `ordered_idle()` currently means `counts[0] == 0`. Apple and Linux use that to permit refusal settlement; render uses it to lift the fence ([core:537](/private/tmp/bsky4-rv/wt-rq/host/apple/src/executor_core.rs:537), [Apple pump:325](/private/tmp/bsky4-rv/wt-rq/host/apple/src/abi.rs:325), [render lift:840](/private/tmp/bsky4-rv/wt-rq/host/render/src/lib.rs:840)).

   If completion decrements that counter, earlier results can remain undrained while the predicate becomes true. A refusal could then parse before them, or render could start work behind the fence prematurely.

   **Required rule:** completion frees work capacity; only settlement/forgetting clears the corresponding settlement obligation. Preserve existing treatment of forgotten but still-running effects too. Do not derive the settlement predicate solely from active-work count.

2. **“Leave it parked and dispatch again at the next release” does not work through today’s interfaces.**

   `Module::release` removes emitted tokens from `waiters`. A composer removes its continuation mapping when dispatch returns anything other than `Held` ([source.rs:73](/private/tmp/bsky4-rv/wt-rq/js/src/source.rs:73), [mixed.rs:631](/private/tmp/bsky4-rv/wt-rq/data/src/mixed.rs:631), [mixed.rs:655](/private/tmp/bsky4-rv/wt-rq/data/src/mixed.rs:655)). Merely putting the request back into the host’s parked map does not arrange another source release.

   **Required rule:** distinguish source-held from capacity-held work. Once `Again` has been emitted, the host owns that readiness record and retries marker admission directly. Wake on drain, forgetting, and fence release; deduplicate records and service older eligible records before newer arrivals.

   Also define ordering as **entry into the ordered settlement sequence**, not numeric runner-ticket order. Source-held requests currently enter later, after release. Reserving a blocking ordered position at their original `Held` stage could obstruct the very delivery they await.

3. **The stated bounds do not cover the proposed retained population.**

   There are already separate normal and fenced pools: the fence permits another 128 held requests and 64 MiB. Refused fence entries are additional metadata ([core:295](/private/tmp/bsky4-rv/wt-rq/host/apple/src/executor_core.rs:295)). Adding unlimited parked overflow markers would not make “128 tickets in all” true.

   There is also a concrete result-accounting gap: handed-off work reserves no response growth before starting, but completion adds its retained result without an aggregate budget check ([core:660](/private/tmp/bsky4-rv/wt-rq/host/apple/src/executor_core.rs:660), [core:711](/private/tmp/bsky4-rv/wt-rq/host/apple/src/executor_core.rs:711)). Several individually permitted outcomes can exceed 512 MiB. Releasing work slots earlier increases the possible accumulation.

   **Required rule:** explicitly budget normal, fenced, completed, and capacity-held states. Keep the existing pool sizes if desired, but state their aggregate envelope. Reserve bounded outcome capacity before handing off work, or provide an equally bounded ownership/backpressure mechanism. Account for parked-call storage separately; “no I/O” does not mean “no memory.” Arbitrary closure captures remain the existing documented limitation.

The following are **should-fix** concerns.

- **Classify the current operation, not the whole answer.** `Again` means “this dispatch carries no executor work,” not “this answer has never performed an effect.” Earlier turns may have written; settlement can produce another request. GET/HEAD classification is a conservative queueing permission, not a promise that settlement commutes. Keep ordered settlement.

- **Terminal failure needs deliberate JavaScript cleanup.** The failed-waiter path removes the Rust parked record before calling any `forget_calls`, so later reconciliation cannot discover it ([lib.rs:896](/private/tmp/bsky4-rv/wt-rq/js/src/lib.rs:896)). However, calling `forget_calls` alone does **not** prove that an arbitrary shared-promise continuation cannot issue a later fetch: the prelude attributes such fetches to the currently delivering call and supports reassignment ([prelude.js:1176](/private/tmp/bsky4-rv/wt-rq/js/src/prelude.js:1176), [prelude.js:1383](/private/tmp/bsky4-rv/wt-rq/js/src/prelude.js:1383)). Separate bookkeeping cleanup from a stronger cancellation guarantee.

- **The host and effect-class claims need narrowing.** Long native requests default to the independent lane, and auth/surface requests have host-specific paths ([request.rs:137](/private/tmp/bsky4-rv/wt-rq/runner/src/request.rs:137), [abi.rs:259](/private/tmp/bsky4-rv/wt-rq/host/apple/src/abi.rs:259)). “Seventeenth native call is refused” is true only for an explicitly ordered case. The existing core test forcibly sets that scheduling ([executor_tests.rs:834](/private/tmp/bsky4-rv/wt-rq/host/apple/src/executor_tests.rs:834)). Preserve these existing distinctions.

- **Cancellation must cover every new state.** Drop a queued read unsent; abort a running safe read; retain ownership of noncancellable work until it ends; discard `Again` without parsing a stale answer. Prune host parked/capacity-held records by current ticket, including supersession and teardown. Existing core cancellation and worker-side destruction are the model ([core:470](/private/tmp/bsky4-rv/wt-rq/host/apple/src/executor_core.rs:470)). Background refusals must continue through `background_landed`, including their wake/fence behavior ([admission.rs:13](/private/tmp/bsky4-rv/wt-rq/runner/src/runner/admission.rs:13)).

My answers to Q1–Q8 are:

| Question | Recommendation |
|---|---|
| **Q1 — POST reads** | Defer `exactRead`. Treat POST conservatively unless a future explicit contract establishes queueable read semantics. Do not infer from endpoint names or “resource” versus “mutation.” |
| **Q2 — storage reads** | Keep opaque storage continuations conservative for this change. Later, classify a specifically identified operation or wholly read-only batch, not an arbitrary closure. The current JS continuation waits on storage already issued through the prelude; it is not simply an unexecuted storage request ([storage.rs:106](/private/tmp/bsky4-rv/wt-rq/js/src/storage.rs:106)). Test storage-backed startup separately. |
| **Q3 — sixteen counts what?** | Uncompleted ordered native work, including queued reads. Exclude completed results and `Again`; retain their independent window/byte charges. Counting only writes would weaken the approved refusal behavior under read pressure. |
| **Q4 — release at completion** | Yes, with separate settlement accounting and enforceable result reservations. A 128-ticket window alone is insufficient. Preserve the independent lane’s accounting. |
| **Q5 — forget failed calls** | Yes, unlink terminally failed calls from prelude bookkeeping. Do not claim that this cancels arbitrary promise continuations or prevents every orphan fetch without additional ownership evidence. |
| **Q6 — herd** | Coalesce to one outstanding readiness record per call immediately. Selective wakeups should follow with tests for completed calls, unclaimed fetches, storage-head changes, and final “pending on nothing” checks. “Only promises whose final value changed” is insufficient. |
| **Q7 — window overflow** | Keep the existing call pending; no permanent failure for an empty re-ask. Require explicit capacity wakeup, fair retry order, cancellation, and accounted storage. |
| **Q8 — pending versus failed** | Keep a capacity-blocked re-ask pending. Resuming the same call does not replay its earlier effects. A generic `refresh` or fresh `answer()` invocation would, and remains prohibited. |

The simpler alternatives compare as follows:

| Alternative | Assessment |
|---|---|
| **Existing read queue + ordered `Again` marker** | Recommended. Removes pointless worker work while retaining runner tickets, ordering, and bounded settlement. |
| **Classify the current no-op as a read** | Small interim fix, but merely moves permanent failure to the larger bound unless overflow remains pending. Still incurs worker scheduling and response reservations. |
| **Retry only the blocked re-ask on the same call** | Sound backpressure. The draft incorrectly groups this with restarting a source. It needs the same capacity-wake and ownership rules as `Again`. |
| **Fulfill re-asks immediately without an ordered gate** | Unsafe for native settlement order. Retaining the runner ticket is possible, but retaining the ordering gate makes this essentially `Again`. |
| **Coalesce re-asks only** | Useful performance work; insufficient admission policy. A real shared completion can legitimately make many calls eligible together. |
| **Raise sixteen** | Moves the failure threshold and weakens effect admission protection. |

Two **nits** are worth correcting: plain ordered GET/HEAD already queue to 128, so the “(b) alone still refuses any seventeenth read” alternative is inaccurate ([executor_tests.rs:725](/private/tmp/bsky4-rv/wt-rq/host/apple/src/executor_tests.rs:725)). Also, the named completed-results test primarily protects the **independent** lane; keep it and add ordered-specific tests rather than weakening it ([executor_tests.rs:204](/private/tmp/bsky4-rv/wt-rq/host/apple/src/executor_tests.rs:204)).

Across hosts, Apple/Linux/render share the core, but need dispatch and wake-path coverage independently. Render must preserve `Busy` and deadline behavior. The wasm web host can put `Again` into its existing immediate-outcome path; that preserves its current behavior, not a new cross-host total-order guarantee ([host.rs:1019](/private/tmp/bsky4-rv/wt-rq/host/web/src/host.rs:1019)). Web-js already uses browser promises directly ([ts-data.js:327](/private/tmp/bsky4-rv/wt-rq/host/web-js/ts-data.js:327)). Terminal still needs an explicit new dispatch arm, despite having no executor bound; render’s pages enumerator does too ([terminal requests:78](/private/tmp/bsky4-rv/wt-rq/host/terminal/src/requests.rs:78), [render pages:62](/private/tmp/bsky4-rv/wt-rq/host/render/src/pages.rs:62)).

The implementation should test:

- **Admission boundaries:** 12/20 shared-load rows; 16 genuinely outstanding jobs followed by GET, HEAD, POST, opaque work, storage, and explicitly ordered native work; count and byte overflow separately.
- **Overflow liveness:** more than 128 eligible re-asks requiring no further I/O; last-capacity release with no new module delivery; composed sources whose dispatch mappings were consumed; exactly one retained readiness record per call.
- **Ordering:** earlier work complete but undrained, then refusal, then `Again`, then an effect. Verify both settlement order and that the later effect cannot execute before refusal settlement. Include held-overflow refusals and the refused background round from `2eca0ad80`.
- **Memory and lifecycle:** large handed-off outcomes behind a slow head; completed results while pumping is paused; forgotten queued/running/completed/fenced/capacity-held work; replacement, stale replies, and worker-side destruction. Assert counters and retained capacities after each transition.
- **No replay:** a call writes once, then waits on a shared promise and encounters capacity pressure. It must finish without another initial invocation. Test terminal cleanup separately from promise cancellation.
- **Host wakes and agent drives:** Linux must progress using executor wakes alone; render must not report completion with pending markers; `clock settle` must observe parked, queued, completed, and background obligations.

Finally, bound the performance claim. Removing worker jobs does not eliminate the re-ask storm: JavaScript work remains roughly O(deliveries × waiters), and repeated linear searches in `wake` can add quadratic waiter bookkeeping. FIFO prevents overtaking among admitted work, but cannot promise progress behind a hung predecessor or admission for writes under continuous read saturation.

Releasing slots on worker completion also makes write-admission decisions sensitive to completion timing. Preserve deterministic ordering, but test capacity decisions with explicit completion barriers rather than sleeps. The current repro uses sleeps and a process-global `ASKED` log shared by both tests; isolate that log and drive completion explicitly before treating its diagnostics as deterministic evidence ([admission.rs:41](/private/tmp/bsky4-rv/wt-rq/js/tests/it/admission.rs:41)).