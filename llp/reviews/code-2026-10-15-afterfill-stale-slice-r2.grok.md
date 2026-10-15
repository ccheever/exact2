# Code review: a stale slice frees what waited for it, round 2, 2026-10-15 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `e9f19841e507b4f0453f537068331ec33ee298ea1d2b7ec5a192892ea020a7a9`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes requested (both P1: reset-time cover clears bound to the new generation; Astra P2: binding dropped on re-queue).
- **Disposition:** both fixed in the second-round commit (viewsEpoch, idle)

---

I'll review the last two commits read-only, starting with the review guidance and the exact diffs.The request is a read-only review of `HEAD~1` and `HEAD`, with a specific checklist. I'll inspect those commits and the surrounding call sites.The second commit defers stale-slice work to the next main-queue turn. I'll read the landing path and every caller that assumes that work already ran.The stale-slice deferral keeps one piece of bound work that belongs to the old tree and runs it after the new tree has published its covers. That is a blocking bug.

[P1] A stale landing reapplies the old tree's cover clear after the new tree — `host/apple/Sources/ExactKit/Session.swift:660`

`generation` increments in `commit` and `finishBoot` before `presenter.reset()`. On iOS, `NavigationHost.reset()` (`host/apple/Sources/ExactKit/IOS/NavigationIOS.swift:1056`) still sees the old views and calls `onCovers` with `(id, nil)`. That closure is `whenIdle(bound: true)`, and the fill is still in flight, so it is stored under the new generation. The filter at `Session.swift:660` keeps it. `landFill` then defers it with `whenIdle(work)` (`Session.swift:665`), which drops the generation. When the slice is already published, `apply` lands it before `presenter.apply`, the new navigation reports its covers synchronously (`fillInFlight` is already false), and the deferred clear runs on the next turn. `set_host_cover` keys a bare `ViewId` (`kernel/src/id.rs:44`), so a reused id clears the new node. The host's `delivered` map is not updated, so it does not send the covers again.

Unbound work (the wake's pump, a timer's advance, the native intrinsic flush) is deferred only, still runs on the next turn, and is queued again if a fill is in flight then. A destroyed session drops it. `drainFill` returning before that work is what `testWorkWaitingForAStaleSliceStillRuns` asserts; under the agent, fills are synchronous, so `clock settle` does not take this path. A current-generation slice still runs its waiting work inline, inside `landFill`, so it does not re-enter `landing`.

The test does fail on the pre-fix code: the free closure stays at 0 because `afterFill` was discarded. It does not catch this bug. `commitForTests` only bumps `generation`, and the bound closure is queued before that bump, so the filter drops it. The slip is a bound closure queued after the bump, which this test never does.
