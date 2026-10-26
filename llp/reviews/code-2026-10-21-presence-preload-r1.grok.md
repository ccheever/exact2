# Code review: The wasm host preloads its presence module where the plan names a presence row, round 1, 2026-10-21 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `b2bb3da1a4`.
- **Method:** one brief (sha256 `031ce7f88732cda87a594cd6d057b134736884eb5951a6679dd12cc444ef10e8`), blind to the other reviewer; 2 rounds (the lead's 23:50Z), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

I'll review the last commit against the presence-preload goal: plan detection, export wiring, timing, agent `pending()` semantics, leftover jump windows, cost, and tests. No files will be edited.The last commit is the presence preload. I'll read that diff and the plan, host, and agent call sites it depends on.The diff is small. Next I’ll check whether `needed()` sees every presence row the plan can name, and how preload and `pending()` interact with boot and the agent.`needed()` matches the plan, and the wasm export is wired correctly. Two jump windows remain, both on paths that apply a batch without waiting for the preload.

## P2 — A gain still jumps when the batch is applied without waiting

`presence.hold` keeps a batch only for an exit while the module is in flight. A create or style that first writes `--exact-layout-transition` returns false and is applied immediately:

```563:569:host/web/navigation.js
    hold(batch) {
      if (live || unavailable) return false;
      const ops = batch.ops ?? [];
      if (ops.some(op => op.op === 'exit' || op.css?.includes('--exact-'))) start();
      if (!held.length && !(loading && ops.some(op => op.op === 'exit'))) return false;
```

`host/web/tests/presence-loader.test.mjs:50` locks that in. `agentSettled` (`host/web/glue.js:1194`) and the start of each clock round (`host/web/glue.js:1232`) do wait, so an agent tap after boot slides. These appliers do not:

- On a dev reload, agent `--plan` swap, or module replacement, `bootNow` calls `preload()` and then, in the same turn, applies place, page, font, and display-scale batches and `activateData()` (`host/web/glue.js:1358`–`1362`). The script cannot have loaded yet. `activateData` applies the data-ready batch at `host/web/glue.js:1269` and only waits for `pieces.pending()` (`host/web/glue.js:1274`). A data batch that flips a bound transition off `"none"` jumps, including every restart after the first activation.
- Inside one clock round, `exact_advance` at `host/web/glue.js:1239` can be the batch that first calls `start()`. That advance is not awaited; `pending()` is checked only at the top of the next round. `presence.live?.tracking()` on `host/web/glue.js:1238` is also still null, so the clock does not stop at the transition's due times for that jump.

**Fix:** Await `presence.pending()` before `activateData`'s `applyBatch`, and in `bootNow` before the follow-up batches at `host/web/glue.js:1358`. In the clock's step loop, if an advance made `pending()` non-null, await it before the next advance. Leave the boot create itself unawaited (it has no previous box). Do not bring back a general hold.

## P2 — A preload retry after failure is invisible to `pending()` and `hold()`

The loader is created once for the page. On failure the catch sets `unavailable` and clears `loading` (`host/web/navigation.js:557`). The next boot's `preload()` starts a new fetch (`loading ??=`), but:

```561:564:host/web/navigation.js
    pending: () => live || unavailable ? null : loading,
    resize(batch) { return { ...batch, presenceSnap: true }; },
    hold(batch) {
      if (live || unavailable) return false;
```

`pending()` stays null for the whole retry, so `agentSettled` and the clock do not wait. `hold()` also drops exits until `live` is set. A restart after a failed `presence-glue.js` load jumps again, and an exit during the retry is removed immediately.

**Fix:** Clear `unavailable` at the start of a new `start()`, and report an in-flight retry: `pending: () => (live ? null : loading)`. `hold()` should keep an exit while that promise is in flight, not while `unavailable` is still latched from the previous attempt.

## P3 — Preload runs before first paint

`loadAfterPaint` (`host/web/glue.js:92`) appends the script immediately; it does not wait for a frame. `bootNow` calls `presence.preload()` at `host/web/glue.js:1324`, before `applyBatch` of the boot batch and before the two animation frames in `main` (`host/web/glue.js:1493`). The comment on that line, and `host/web/src/abi/presence.rs:3`, say this is after first paint.

Synchronous start is what makes `pending()` already set when the driver proceeds: the driver waits on `bootMs`, which is assigned only after `bootNow` returns. Deferring with `afterPaint(..., agentMode)` would leave `pending()` null for a fast agent tap (no timer under the agent), and the gain would jump again.

Plans that do not name either row never call `preload()`. A literal row already fetched during this same boot via `hold()`. The new cost is a pre-paint fetch for a plan whose boot batch declares nothing.

**Fix:** Keep the synchronous `preload()` under `agentMode`. For a normal page, schedule it with `afterPaint`. Correct the comments either way.

## P3 — `pending()` has no deadline if the script never settles

A network error resolves: the catch fulfills the promise, `unavailable` is set, and the next `pending()` is null. `agentSettled` does not hang on that path.

If neither `onload` nor `onerror` fires, the promise returned at `host/web/navigation.js:561` never settles. `agentSettled` (`host/web/glue.js:1194`) and the clock (`host/web/glue.js:1232`) await it with no race against `SETTLE_DEADLINE_MS`, so `clock settle` never returns `settled: false`. Same shape as `pieces.pending()` on the line above, now on every plan that names a presence row.

**Fix:** Race the await against the clock deadline and continue with motion skipped for that call.

## Checked, not filed

`needed()` (`host/web/src/abi/presence.rs:20`) is the right scan. Every style attribute becomes a `BindingsRow` with `StyleId as u16`, literals included (`contract/lower/src/lib.rs` `style_row`, and the literal assertion in the new test). A `style` class is expanded into those attributes before that (`contract/lower/src/nodes.rs:62`, `class.rs`). A component param is rewritten into the row's value; the row name stays `-exact-layout-transition` or `-exact-exit-animation`, so the binding id is unchanged. State looks are appended to the same `plan.bindings` table. An exit that only names keyframes is still an `ExitAnimation` binding; keyframes with no exit and no layout transition correctly do not load the module. `StyleId::from_bit` matches `uses.rs`; bits 142 and 143 are `exit_animation` and `layout_transition`.

`exact_presence` sits inside `host!` (`host/web/src/abi.rs:1207`) and calls public `Bridge::presence`. `host` is private on `Bridge`, and `abi::presence` is a child module, so that read is visible. It runs only after a successful `exact_boot`, `exact_boot_plan`, or `exact_boot_module`, which covers first boot, dev reload, agent plan swap, and module replacement. A refused boot does not replace the runner and does not preload.

No P1.
