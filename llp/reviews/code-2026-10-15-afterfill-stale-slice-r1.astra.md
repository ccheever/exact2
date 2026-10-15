# Code review: a stale slice frees what waited for it, round 1, 2026-10-15 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `dc1c29ae683793d72e37fefa9b32b8ca402a432ece7fb57ff4fe4236dd5d67db`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes requested (Astra P1 ordering, both: the test).
- **Disposition:** both fixed in the first-round commit (a392b16bc)

---

I found two issues. **The ordering bug is blocking.**

1. **[P1] Deferred work can overtake the replacement tree — [Session.swift:643](/tmp/rv-af1/host/apple/Sources/ExactKit/Session.swift:643).**  
   `presentCommitted` resets the presenter, then calls `apply(replacementBatch)`. Before presenting that batch, `apply` drains published slices. A stale fill now calls `landedFill`, which immediately runs queued callbacks while `landing == true`. A queued pump or timer can produce and apply a newer batch before the replacement batch creates its views; recursive `apply` bypasses draining because `landing` is true. For example, the pump can dispatch the new plan’s initial `intersect` handler, whose UI changes are then lost or overwritten by the replacement batch. Release waiting work after both the published batches and the enclosing synchronous batch have applied.

2. **[P2] The regression test fails on HEAD — [IdleTickTests.swift:101](/tmp/rv-af1/host/apple/tests/ExactKitTests/IdleTickTests.swift:101).**  
   The bound callback captures `session.generation` when queued, but the test never changes that generation. Publishing a batch tagged `generation - 1` only makes the batch stale. Consequently, the bound callback runs: `bound` becomes **1**, contradicting line 106, and later **2**, contradicting line 114. Queue the callbacks before an actual generation transition and publish the fill with its captured generation. The free-callback assertion would detect the old dropping behavior, but the current test also rejects HEAD.

The other requested checks revealed no additional blocker:

- `Collection.reset()` clears `fillLimits` and `fillSent`; `fillInFlight` prevents a new fill before landing. Reusing the view ID therefore does not retire the new list; current dirty reports are merely rescheduled.
- The production `whenIdle` callers are classified appropriately. Native intrinsic flushing validates incarnation IDs and must survive to clear its pending flag.
- Collection landing schedules later work rather than synchronously sending another fill. The ordering problem comes from executing the waiting callbacks inline.
- A session already destroyed drops the captured queue. I found no additional live-session path that discards a queued wake pump.
- `rest()` is unchanged apart from relocation.

Source review only; I did not build or run XCTest under the read-only constraint. No files were changed.
