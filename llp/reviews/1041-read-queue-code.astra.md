**Three blockers and three should-fix findings.** I reviewed the requested commit range and both prior reviews without editing files.

1. **Blocker — capacity pressure still permanently fails re-asks, and that refusal bypasses fencing.**  
   With 128 admitted markers and 1,024 waiting records, the 1,153rd `Again` returns an error at [executor_core.rs:661](/private/tmp/bsky4-rv/wt-rq/host/apple/src/executor_core.rs:661). Apple/Linux record that as a terminal refusal; the JS waiting-call path returns `Unavailable` at [lib.rs:900](/private/tmp/bsky4-rv/wt-rq/js/src/lib.rs:900). This recreates permanently failed answers at a larger threshold, contrary to the agreed “capacity-blocked re-asks never fail” rule and [LLP Q7–Q8](/private/tmp/bsky4-rv/wt-rq/llp/1041-graceful-overload.rfc.md:741).

   This error also neither raises `ordered_barrier` nor retains a `Fenced::Refused` position. Consequently, a later POST can execute and settle before this ordered refusal. Capacity exhaustion must retain readiness without taking the terminal-failure path. Add coverage beyond 1,152 re-asks, including a subsequent effect.

2. **Blocker — cancelling a refusal can reorder already-fenced re-asks.**  
   Apple/Linux lift the fence when the runner no longer retains its refusal, including cancellation, without requiring the admitted markers to have drained ([abi.rs:243](/private/tmp/bsky4-rv/wt-rq/host/apple/src/abi.rs:243)). Consider 128 occupied marker slots and a fence containing:

   `Again A → refused B → Again C`

   When the original refusal is cancelled, `resume_ordered` puts A into `full`, returns B’s refusal, and retains C behind B. Its final retry then appends A **after C** ([executor_core.rs:600](/private/tmp/bsky4-rv/wt-rq/host/apple/src/executor_core.rs:600), [executor_core.rs:628](/private/tmp/bsky4-rv/wt-rq/host/apple/src/executor_core.rs:628)). Settlement becomes B, C, A. This violates both held ordering and oldest-first re-ask service.

   The same unchecked transfer can move 128 fenced markers into an already-full 1,024-record waiting queue, producing **1,152 waiting records**, exceeding the stated queue and 8 KiB bounds. Preserve the held prefix when marker placement fails, and test cancellation while the marker window is full.

3. **Blocker — pages enumeration can bypass its deadline indefinitely.**  
   The new `Again` arm immediately supplies a completion. The enumerator checks its deadline only after an unsuccessful drain ([pages.rs:66](/private/tmp/bsky4-rv/wt-rq/host/render/src/pages.rs:66), [pages.rs:71](/private/tmp/bsky4-rv/wt-rq/host/render/src/pages.rs:71)). A source that repeatedly returns another re-ask therefore loops without reaching that check.

   The corresponding fix in the main render settlement loop is correct, but does not cover this second loop. Check the deadline on every enumeration iteration and exercise the existing “forever” fixture through `pages()` too.

4. **Should-fix — the implementation expands the agreed admission envelope.**  
   Real admission subtracts `agains` from the ticket count, while marker admission independently allows 128 markers ([executor_core.rs:369](/private/tmp/bsky4-rv/wt-rq/host/apple/src/executor_core.rs:369), [executor_core.rs:761](/private/tmp/bsky4-rv/wt-rq/host/apple/src/executor_core.rs:761)). This permits **256 admitted ordered tickets**, before fenced and waiting records.

   The [as-built amendment](/private/tmp/bsky4-rv/wt-rq/llp/1041-graceful-overload.rfc.md:607) explicitly documents the deviation and its shared-load rationale. Nevertheless, it differs from the supplied agreed design’s shared 128-ticket backlog. Reconcile that capacity decision explicitly; conformance to the original envelope is not established.

5. **Should-fix — the Bridge repro still masks missing wakes and has timing-dependent pump counts.**  
   [admission.rs:173](/private/tmp/bsky4-rv/wt-rq/js/tests/it/admission.rs:173) pumps unconditionally, and [line 186](/private/tmp/bsky4-rv/wt-rq/js/tests/it/admission.rs:186) ignores a 100 ms receive timeout. Losing every subsequent wake would therefore still allow periodic progress.

   The gate and serialized transport log remove the original startup and cross-test races. The row identity, invocation count, and exact URL assertions are useful. However, “wake-driven” and deterministic **62-pump** evidence are stronger claims than this harness supports. Require a wake before another pump, failing on timeout; use explicit completion barriers if pump counts must be reproducible. The [Linux FD test](/private/tmp/bsky4-rv/wt-rq/host/linux/src/presenter/events_tests.rs:935) does require actual wake readiness.

6. **Should-fix — the terminal-cleanup test does not detect removal of the cleanup.**  
   The assertions at [castle.rs:1209](/private/tmp/bsky4-rv/wt-rq/js/tests/it/castle.rs:1209) check `Module::in_flight()`, which counts only Rust parked calls and streams ([lib.rs:628](/private/tmp/bsky4-rv/wt-rq/js/src/lib.rs:628)). The parked entry is removed before the new `forget_calls` invocation. The fixture’s shared-promise waiter makes no subsequent fetch, and the test never observes its retained JavaScript call.

   Thus, by inspection, deleting the new cleanup call leaves these assertions satisfied. Add an observation of prelude bookkeeping that distinguishes deletion from merely removing the Rust parked entry.

The central accounting change otherwise follows the conservative design: `Again` carries no closure; admitted markers settle through the ordered head; finished reads become `light`; finished effects retain their sixteen-slot charge until drain; and `ordered_idle` remains based on settlement obligations ([executor_core.rs:454](/private/tmp/bsky4-rv/wt-rq/host/apple/src/executor_core.rs:454), [executor_core.rs:579](/private/tmp/bsky4-rv/wt-rq/host/apple/src/executor_core.rs:579), [executor_core.rs:858](/private/tmp/bsky4-rv/wt-rq/host/apple/src/executor_core.rs:858)). Normal drain/forget wake paths and background-refusal handling remain intact. All named host dispatch arms are present. Resumption uses the existing call, with no new source-restart path that would replay earlier effects.

The **512 MiB aggregate is still not strictly enforced for handed-off outcomes**: they reserve no response growth, then add retained bytes at completion. That predates this change and is now expressly documented at [LLP:664](/private/tmp/bsky4-rv/wt-rq/llp/1041-graceful-overload.rfc.md:664).

Verification was limited: `git diff --check` passed, but Cargo failed before running tests because the read-only sandbox denied target-directory creation. I checked the overflow/reordering transitions with a small in-memory model, not the compiled Rust executor. I therefore cannot independently confirm the reported twenty-row result or pump counts.