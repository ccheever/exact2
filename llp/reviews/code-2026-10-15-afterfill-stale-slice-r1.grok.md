# Code review: a stale slice frees what waited for it, round 1, 2026-10-15 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `dc1c29ae683793d72e37fefa9b32b8ca402a432ece7fb57ff4fe4236dd5d67db`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes requested (Astra P1 ordering, both: the test).
- **Disposition:** both fixed in the first-round commit (a392b16bc)

---

I'll review the HEAD commit read-only, starting with the diff and the idle-queue paths you called out.The HEAD commit is the Apple host idle-queue fix. I'll read that diff and the generation, landing, and wake paths around it.The diff is small. Next I’ll read landing, generation, and collection reset so the six checks are against the actual control flow.The production change is sound. One bug: the new test fails, so it does not lock the fix.

## Bug

`host/apple/tests/ExactKitTests/IdleTickTests.swift:105` — `testWorkWaitingForAStaleSliceStillRuns` fails on this commit.

Bound work is dropped only when `ExactSession.generation` changes after the work was queued (`Session.swift:571` stores that generation; `Session.swift:663` keeps the item when it still matches). The test never changes it. `makeSession` leaves `generation` at 0, and `publishFillForTests(..., generation: session.generation - 1)` only tags the slice. The bound closure was stored as `0`, the session is still at `0`, so it runs and `bound` becomes 1. `XCTAssertEqual(bound, 0)` fails. The `free == 1` check above it does pass, which is the real fix.

On the old `landFill`, that same situation cleared `afterFill` entirely, so `free` would stay 0. The new test does not compile against that code (`whenIdle(bound:)` and `publishFillForTests` are new), and it does not pass on the new code either. It also never resets the collection host, never bumps the session generation, and never calls `runtime.pump`, so it does not cover a commit, `collections.landed`, or `begin_pump`.

A test that matches the implementation queues both closures, then advances `session.generation` the way `commit` / `finishBoot` / `destroy` do, then lands a slice captured at the old generation. Unbound work runs; bound work does not.

## Checks

**1. Stale `collections.landed` after `reset` is safe.** `CollectionHost.reset` (`Collection.swift:363`) drops `entries`, `dirty`, `fillSent`, `fillLimits`, `retireOwed`, and `fillPending` before the new tree is applied. `landed` (`Collection.swift:550`) only retires a list when `fillLimits[view]` is set and `entries[view]` exists. `fillLimits` is written only when an off-main fill is sent (`Collection.swift:700`), and `flush` returns immediately while `fillInFlight` is true. `landFill` clears `fillInFlight` and then calls `landedFill` with no flush in between (`Session.swift:636`–`643`), so a reused view id cannot pick up the old limit or the old send time. If the new tree is dirty, `landed` calls `schedule()`, which flushes every dirty list, not the stale id. That is the held-report reschedule, and the continuation captures the post-reset collection generation.

**2. Unbound work that must run still runs; view-tied work is still dropped.** Every `whenIdle` in `host/apple/Sources`:

| Call | Bound? | After a commit |
|---|---|---|
| Wake pump (`Session.swift:558`) | no | runs, so `begin_pump` can clear `notified` |
| Timer advance (`Session.swift:1257`) | no | runs; the timer has already cleared `clockTimer` |
| Native intrinsic flush (`NativeModule.swift:949`) | no | runs and clears `intrinsicFlushPending` |
| `onIntrinsic` / `onCovers` (`Session.swift:728`, `731`) | yes | dropped |
| `BlockHost.tell` (`BlockHostMac.swift:188`) | yes | dropped |

The flush applies sizes only through `onIntrinsic`. At landing time `fillInFlight` is already false, so that `whenIdle(bound:)` runs immediately, but `flushIntrinsicSizes` keeps a size only when the nonce, incarnation, and `NodeView` identity still match. `reset` has already forgotten those views.

**3. Ordering.** `landedFill` runs inside `landFill`'s loop with `landing == true`, in the same slot the fresh path already used: this slice, then what waited, then later published items. A waiter's `apply` does not re-enter `landFill` (`Session.swift:1020`), so it cannot pull a later tick ahead of itself; the outer loop applies that tick after the waiter. `schedule()` is `main.async`, and `sendFill` publishes from the owner thread, also onto a later main turn. A stale landing after `reset` does not call `requestFill`, because `fillLimits` is empty, so it does not start another fill in that loop.

**4. Destroyed state.** `landedFill` drops the queued work and skips `collections.landed` when `state == .destroyed` (`Session.swift:661`). `destroy` sets that state, bumps the generation, resets the presenter, and retires the runtime before the in-flight slice can land. A pump that never runs is not stuck: the executor is already retired.

**5. No other dropped pump.** `fillInFlight` is cleared only for a `.fill` item, and both the fresh and stale arms call `landedFill`. A stale tick or canvas does not drain `afterFill`; the pump stays queued until the fill lands. The wake closure is unbound, so a generation change does not drop it. If the session is gone from `live` before the wake's main hop runs, that hop returns without queueing; `destroy` has already retired the executor.

**6. The test does not exercise the fix.** See the bug above.
