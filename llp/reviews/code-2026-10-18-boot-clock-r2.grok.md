# Code review: a fresh boot on a live clock starts the runner's clock at its reading (StoreRefreshTests), round 2, 2026-10-18 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `cb1c4451b`.
- **Method:** one brief (sha256 `a176cfe43ca93b52a48f0b21def202ca0527de658b37c9d6ceaa9f4d6e53948a`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocking; the bracket is right and neither load race escapes it.
- **Disposition:** nothing to take.

Nothing is blocking. The round-1 bracket is right, and neither load case flakes.

The long task is armed at the refresh commit’s start sample. `store_changed` checkpoints before it settles, and that checkpoint calls `start_commit`, which samples the live clock. `gate_step` then sets the deadline to that `now_ms + 20000`. The later sample in `conclude` does not move an already-armed gate. In `StoreRefreshTests.swift`, `refreshing` is taken before the foreground post, and the commit runs only after that post (the main-queue observer, then `refreshSharedStore`). `refreshed` is taken after `wait` returns. The start sample sits between those two wall readings, so `start + 20000` is inside `[refreshing + 20000, refreshed + 20000]`. The range is closed, so an unchanged clock still matches. The short task’s deadline is about 10 s after boot, which is below `refreshing + 20000`.

The expectation cannot win the race with `observeBatch`. `observeBatch` runs inside `read()` during `storeChanged`, and `refreshStore` does not schedule the reload until every session’s callback has returned. That reload is `DispatchQueue.main.async`, which is what fulfills the expectation. A reload left over from the previous step is not still queued: the previous `wait` returns on its own reload, and the only thing it can leave behind is a follow-up `check`, which itself refreshes (and hits `observeBatch`) before it queues the reload this `wait` is waiting on.

Reader 1 is inside the same window. `whenIdle` runs the work immediately when no fill is in flight, and this fixture has no collection fill, so both readers finish `storeChanged` and `apply` before the aggregate completion queues the reload. Reader 1’s predicate only checks `!= due` and `>= refreshing + 20000`, not the upper bound, but nothing can move its deadline after `refreshed` is sampled: the assertion runs on the same turn, before the run loop.
