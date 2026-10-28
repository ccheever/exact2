# Code review: the black viewer: thumbnail under the picture, picture retries and priority, round 3, re-check 5 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `0d02ee25d2`.
- **Method:** one brief (sha256 `d185541722bae6cd059134be5dc6ab69865134f719a5b88f3eb28f3c7d937f84`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. `host/linux/src/host.rs:926` — The load is committed at the host clock from before the handler, so a transition or numeric roll it starts is born late and then skipped. `at_ms` is taken before `Runner::dispatch`. On a live clock that call samples inside `arrived` / `start_commit`, and `performanceNow()` is that sample (wall time). `self.now_ms` is never assigned. `commit_effects` seeks only to `at_ms.min(self.now_ms)` (`host.rs:1327`) and then to `self.now_ms` (`host.rs:1341`), so the `max(runner, host)` stamp is discarded whenever the runner is ahead, and it never includes the sample when the clocks were equal. A static page waiting on a download does not `advance` or `tick` (nothing is due, no frame task), so both clocks sit at the last turn while the handler runs at wall time. The curve is born at the old host time. The next `tick` or `advance` seeks to wall and jumps the gap: a fade or a numeric roll (`row_dirty` uses the same clamped time, `host.rs:1257`) finishes between two frames. The `Err` arm has the same hole: a refusal still samples the runner, then `commit(&[], Some(...))` leaves the host clock behind, and the next success is clamped back to it. Set the time after `dispatch` returns, on both arms, and move the host clock there before `commit` so the clamp cannot rewind it:

```rust
let at_ms = self.runner.now_ms().max(self.now_ms);
self.now_ms = at_ms;
self.commit(&[Timed { at_ms, receipt }], None)
```

That is still after the handler, so nothing the clock owes runs first. An empty error commit then seeks to that same time; a transition is in flight only while the loop has been ticking, so the catch-up is a frame.

**Non-blocking**

none

The earlier items:

- The check and the handler are adjacent. `dispatch_now` calls `Runner::dispatch`, which does not advance. For `Event::Load`, `document_value` returns the event unchanged. `sample()` only stores the clock. Due timers, `then`s, and the next load stay after this handler, at the loop's next advance. The per-load `after_commit` still drops a later waiting load whose source an earlier handler changed (`Images::sync_shown` bumps `assignment` and removes that id).
- The drain is gone. The loop is bounded by the queue length at entry. Each waiting load is removed and either sent or dropped. A handler that keeps changing sources cannot extend the turn. A load queued during delivery is sent in this turn only when sync has already removed an earlier one, and never past that count. The comment matches the bound.
- A source that changes and changes back inside one advance is an ordering rule, and it is sound for an app. Those commits are one `advance` / `animation_frame`: several runner commits, then a single `sync_images`. The intermediate source is never synced, so it is never fetched, never assigned, and never painted; the bitmap stays the original source's. At send time the node, the source string, and `assignment` are that same source, and the handler runs on them. What the app can see and what `load` reports are the same picture. A refetch happens when the other source is still in the tree at a later sync, which is the first moment that source would have been shown. Two load handlers in one delivery still drop the old load, because each has its own `after_commit` sync; the test `a_load_waiting_is_dropped_when_actions_change_its_source_away_and_back` covers that path.
