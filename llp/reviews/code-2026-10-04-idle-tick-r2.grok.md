# Code review r2: an idle timer tick only moves the clock (557c19407..73afe0df8), 2026-10-04 (grok)

- **Family:** xAI. `grok-4.7`, reasoning effort xhigh, `--no-subagents`, read-only by instruction, `--cwd` a detached worktree at `73afe0df8`.
- **Method:** one brief (sha256 `6a97ff22afe741ddd7213ecae32d559ffb79670692b9507c964c721cf868958c`), shared with astra. Round 2, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the analysis, findings and verdict, unedited (the tool's progress lines before them are omitted).
- **Verdict:** LAND WITH FIXES.
- **Disposition (r3):** finding 1 is taken as suggested: the load callback drains only when `deferred` is empty. Finding 2 is taken in part: the out-of-batch sync and geometry replay have a grouped-list test, and the clock's re-arm and invalidation are asserted. DEFERRED: a host test of the deferred-load drain (it needs a deferred GPU artifact in the test bundle) and of the sampler's seq (the sampler is development-only and absent in tests).

---

## Round 1

**Surface work after a deferred GPU load.** Resolved for one deferred artifact, on iOS and macOS. The load callback removes its key, loads, then calls `drainSurfaceWorkNow`. A lone success leaves `deferred` empty, so `ready` is true and the queued capture or restore runs. A lone failure sets `canvases.failed`, and the same drain settles the ticket. See finding 1 for two artifacts.

**Sampler sequence on a skipped tick.** Resolved. `applyTick` calls `sampler?.batch(batch.seq, ms: 0)` and returns, so the range is recorded once and `apply` does not record it again. Production builds have no sampler, so this stays off the presenter's pass. `FrameSampler.batch` still calls `activity()`, which is the same dev-only display link an applied batch already kept warm.

**Subtree appearance and size traits.** Resolved on iOS. A control node's `UITraitUserInterfaceStyle` callback calls `requestControlsSync`. Each control registers content size, legibility weight, and display scale and does the same. The first call schedules one sync; the rest hit `controlsSyncOwed` and return. `controls.sync` then re-resolves accent and natural size. A system appearance change still goes through `session.scheme` to a full `apply`, and the coalesced sync runs once more afterward.

**Geometry replay outside `apply`.** Resolved. `applyGeometry` requests the sync only when `!applying`. In-batch frames, including the first of two sheet dismissals inside `navigation.sync`, still reach `controls.sync` later in that same `Presenter.apply`. A later `modalDidDismiss` replays frames with `applying` false, and every `applyGeometry` in that replay shares one scheduled sync, which runs after the replay returns.

**Tests.** The round-1 test notes are in. `IdleTickTests` uses `now() + 250`, so the wake is a timeout (the frame threshold is about 133 ms), and it checks `clockDue`, a valid `clockTimer`, and that a nil deadline invalidates the timer. The decode test reads `"controls":true` and the default false. `controls.rs` separates face text, option `disabled`, and a `when` face swap. Finding 2 is what those tests still do not cover.

## Checked, not filed

`requestControlsSync` is safe across teardown and restart. `destroy` and `presentCommitted` call `presenter.reset()` synchronously; trait callbacks during removal only enqueue work. The block holds the presenter weakly, and after reset `controls.sync` sees an empty chrome and does not publish intrinsics. A block that runs after the restart syncs the new tree.

It does not run its sync while `applying` on the paths that schedule it. The block is `DispatchQueue.main.async`. `Presenter.apply` sets `applying` false in the outermost `defer` before it returns, and that stretch does not spin the run loop, so the block observes `applying == false`. In-batch geometry never requests a sync. Coalescing holds: the flag stays set until the block starts, the sync reads live controls after every synchronous `applyGeometry`, and a trait callback during `controls.sync` schedules one later pass because the flag was already cleared.

The `NodeView` trait callback was already registered for every node and already restyles, invalidates text, and repaints. The new line is a `kind == "control"` check. Only controls schedule work, and they share one sync.

`drainSurfaceWorkNow` does not re-enter. `drainSurfaceWork` snapshots `pendingSurfaceWork`, clears it, and performs `surfaceWork` on a later turn, so `completeSurface` → `apply` is not on the load callback's stack. The `if !applying` guard is not taken when the callback runs after the `apply` that queued the load. A second callback for a key already removed from `deferred` returns immediately.

`clockDue` as `private(set)` is safe. The setter stays on `ExactSession`. `scheduleClock` is the only writer. The test reads it; nothing else does.

Nothing else the idle skip used to refresh is still stranded. `changesNothing` still forces a full `apply` for ops, errors, `controls`, canvas images, and motion, spatial, frame-task, and canvas edges. Fills, feedback, the agent clock, and frame ticks never use the skip. System appearance on both platforms still assigns `session.scheme` and applies. macOS has no sheet geometry replay outside `apply`.

## Findings

**1. Should-fix — A failed deferred load drains surface work for an artifact that is still loading.** `host/apple/Sources/ExactKit/IOS/GpuIOS.swift:290`, `host/apple/Sources/ExactKit/Mac/GpuMac.swift:195`, `host/apple/Sources/ExactKit/Session.swift:1058`, `host/apple/Sources/ExactKit/CanvasSeams.swift:180`.

`loadIfNeeded` schedules one main-queue callback per deferred artifact, in sorted key order. Each callback calls `drainSurfaceWorkNow` as soon as that one `load` returns. `drainSurfaceWork` proceeds when `canvases.failed != nil`, even if `deferred` is still non-empty. If the earlier artifact's `load` fails, `failed` is set and the drain runs before the later callback. `surfaceWork` then completes every queued ticket whose module is still nil with kind 3 (`"unavailable"`). That fulfillment is final: the later artifact can load on the next callback and the capture or restore is already settled. One deferred artifact still drains correctly, because removing its key leaves `deferred` empty.

**Fix:** Drain only after this wave has settled. After `load(key)`, call `drainSurfaceWorkNow` only when `deferred.isEmpty`. The last callback then drains, whether its own load succeeded or failed: `ready` is true once every key is in `attempted`, and a failed module's own tickets still complete as unavailable inside `surfaceWork`.

**2. Nit — The new recovery paths have no regression.** `host/apple/tests/ExactKitTests/IdleTickTests.swift:16`, `host/apple/tests/it/controls.rs:48`.

The clock, decode, and split Contract cases are real. Nothing asserts that a skipped tick passes `seq` into `FrameSampler`, that a deferred load settles `pendingSurfaceWork` without a later `apply`, or that a trait callback or an out-of-batch `applyGeometry` ends in `controls.sync`. A destroy of an option is also still untested; that one stays off the skip because the batch carries a destroy op.

**Fix:** Add a host test that queues surface work, completes a deferred load with no further batch, and sees the ticket fulfilled. Assert `sampler.batch` recorded the idle tick's `seq` at 0 ms. A narrow presenter test can call `requestControlsSync` and `applyGeometry` outside `apply` and expect one `controls.sync`.

Verdict: LAND WITH FIXES
