# Code review: web clock settle waits for a glide, narrow re-check 1, 2026-10-28 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `fd493229d1`.
- **Method:** a narrow brief naming the earlier findings and their fixes; 2 rounds (agent tooling on the web hosts, theme2's 03:42 ruling), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Note:** Grok's narration before its verdict is left out.

**BLOCKER** `host/web/glue.js:1252` — A smooth scroll started by this round's `then` still answers `settled: true` on the wasm host.

`scrollsSeen()` is taken at `glue.js:1232` and compared at `glue.js:1252` with no rendering step between them. The `then` runs inside `exact_advance` (`glue.js:1239`). `scrollIntoView` runs synchronously at the end of that batch (`navigation.js:847`, from `glue.js:876`). The `scroll` event for that call is delivered at the next rendering step, the same timing `input-glue.js` records for a scroll written in a turn. `fireOwedScrolls` (`glue.js:1249`) waits for frames only while a virtualized list owes a scroll; a non-virtualized scroller owes nothing, so that `await` is a microtask. `textflow.settle` yields a frame only when flow is dirty. With the clock already at a fixed point, line 1252 succeeds and the host returns `settled: true` before the first event. A `behavior: "smooth"` scroll then keeps firing — Astra's case, one event by the time the caller looks and more on the following frames.

The list fixture stays green because that smooth scroll is the collection's own motion, which already held settle before this fix.

On the JS host a `then` is its own commit, so `clock.epoch` moves (`rt.js:210`) after the snapshot at `agent.js:486`. The quiet check at `agent.js:495` fails, and `agent.js:497` waits one frame before the next `glidesSettled`, which then sees the events.

The other round-2 checks hold:

- A frozen page cannot stack the 250 ms floor across rounds. The timer resolving `frame()` false makes `glidesSettled` return false at `input-glue.js:1111`, and both callers return `reason: 'scroll'` on that call (`agent.js:462`, `glue.js:1232`). Once the budget is spent, a page that never paints pays one 250 ms wait and answers false. Returning true from those timeouts would be required to reach 16 rounds × 2 frames × 250 ms, and a timeout is what returns false.
- An owed list scroll does not make every later round non-quiet. `fired` or `owed` `continue`s before the quiet check (`agent.js:477`, `glue.js:1249`). The next round runs `glidesSettled` and only then samples `scrollsSeen`, so the event already delivered is the new baseline. Motion that event started is waited out inside that call, which answers false at the budget (`input-glue.js:1113`) rather than walking the 16-round and 15-round caps. A layout read does not dispatch `scroll`.
- Both JS `continue`s sit above the quiet check and leave `quiet` false. Each one starts another round whose first act is `glidesSettled`. Falling out of round 16 still fails a last-round list event as `scroll-event` (`agent.js:505`), and a round that continued with a request in flight still answers from the inflight count.

The budget change itself is right: only time inside `glidesSettled` reduces `budget.ms` (`input-glue.js:1116`), a quiet pair returns true before the deadline check (`input-glue.js:1112`) even when `budget.ms` is already 0, and a frame that never arrives returns false. The auto-case upper bound stays the guard from round 1.
