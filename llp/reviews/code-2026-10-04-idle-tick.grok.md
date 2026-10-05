# Code review: an idle timer tick only moves the clock (557c19407), 2026-10-04 (grok)

- **Family:** xAI. `grok-4.7`, reasoning effort xhigh, `--no-subagents`, read-only by instruction, `--cwd` a detached worktree at `557c19407`.
- **Method:** one brief (sha256 `3aac06ca09b60bb101ebd3f43c0abc589b0a335b94b02061535d1162177f9518`), shared with astra. Round 1, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the analysis, findings and verdict, unedited.
- **Verdict:** LAND WITH FIXES.
- **Disposition (r2):** the single finding (the test arms no timer; the decode is unasserted; Contract cases are combined) is taken as suggested.

---

### 1. What the skip omits

`applyTick` (`Session.swift:911`) falls through to `apply` while `applying` is set, while any of `fillInFlight`, `tickInFlight`, or `canvasInFlight` is set, and whenever `changesNothing` is false. A nothing-tick therefore runs only while the session is idle. The empty-batch work in `apply` (`Session.swift:920`) is not owed in that state:

- **landFill.** The flight guard matches the condition under which `apply` calls `landFill` (`:924`). `whenIdle` (`:555`) defers a firing timer only for `fillInFlight`. `afterFill` (`:636`) runs that deferred tick after the fill's `apply` and after `if canvasOwed { askCanvasDraw() }` (`:633`), still inside `landing`. A queued owner batch keeps its flight flag until `landFill` clears it, so that deferred tick takes the full path.
- **Sampler `seq`.** Skipped ticks stay out of `FrameSampler`. That is the measured win (empty batches 313 → 46) and is dev-only.
- **`AnimatedRasters.poke`.** GIFs have their own timer. The agent clock calls `evaluate` from `Agent.advanceStepped` and never uses `applyTick`. An idle wall-clock tick changes no raster.
- **`raiseLifted`.** An empty tick applies no style, so it cannot restore a lifted z-index. A batch that changes style is not skipped.
- **`frames.run` / `timerSoon`.** The skip calls the same `scheduleClock(due:)` as `apply` (`:914`, `:958`). `changesNothing` requires frame tasks, motion, spatial, and the canvas flags to match, so `timerSoon` comes out the same.
- **`loadIfNeeded` / `drainSurfaceWork` before first paint.** Boot goes through `apply`, which schedules the async load (`:956`). The gate opens from `firstDrawn` or that post-`firstDrawMs` path. Later native views attach on `update` / `laidOut` once the gate is open. Canvas `surfaceWork` that arrives before `canvases.ready` returns from `drain`, and the GPU completion calls `frames.requestCanvas`; the next real `apply` drains it. A GPU frame tick does not call `session.apply`. An app with no timer already waits the same way.
- **`pendingSurfaceRecords` / `pendingViewDark` / `pendingCommands`.** These are queued inside `presenter.apply` and drained by the outermost `apply` that queued them (`:959`). A later skipped tick does not own them.
- **`collections.flush`.** Dirty paths (`changed`, `userIntent`, `animationEnded`, `dataReady`, `pinsChanged`) call `schedule()` or flush themselves. `landFill` still calls `landed` when the fill batch is skipped (`:635`).
- **Input-hold eligibility.** Rechecked on move. Handler changes are ops. The old feedback skip already returned before this check.
- **`transformGeometry.changed`.** Also runs from layout, window moves, scroll, and modal present/dismiss. An empty tick was not a geometry change.
- **macOS `regions` prepare/flush.** An answer that arrives outside `apply` is received directly. The per-tick flush of an empty batch is the accidental refresh this amendment drops.

### 2. Timer cases

`scheduleClock` (`:1112`) is what both paths call.

- Agent mode (`ExactEnv.agentMode || clock != nil`) yields `Wake.none` (`:1321`). No `Timer` is armed. `timerSoon` stays false unless frame tasks want the link, and those are suppressed for the agent (`:1116`). The agent advances with `session.apply`.
- Frame tasks keep the link: `timerSoon = wake == .frame || (frames.tasks && !agent)`, including when the deadline is far or nil.
- `wake == .frame` (delay ≤ 8×1000/60 ms, `:1324`) does not arm a `Timer`. `Frames.tick` (`:1378`) still full-applies. A 250 ms poll stays on `.timeout` and hits `applyTick`.
- A nil deadline is `.none`: the timer is invalidated and `clockDue` cleared (`:1124`).
- The same deadline already armed returns early (`:1120`) and still calls `frames.run`. The fire callback nils `clockTimer` before `applyTick` (`:1131`), so that early return does not swallow the re-arm of the deadline that just fired.
- A tick that fires during `applying` fails the guard and nests `apply`, which is what the callback did before. Navigation `pendingSync` is retried from the UIKit show and dismiss callbacks.

### 3. `changesNothing` on the fill and feedback paths

The old inline checks were empty ops, a nil error, motion, spatial, `timerDue`, and `canvasOwed`. Fill (`:629`) and feedback (`:667`) still require `batch.timerDueMs == timerDue` on top of `changesNothing`. The shared predicate (`:901`) also requires `!controls`, empty `canvasImages`, and matching canvas and frame-task flags. Those additions keep option-only batches, canvas decodes, and frame-task or canvas-wants-frame edges on the full path. `batch.pending`, `batch.clock`, and `batch.timers` are not read by `apply`. `timers` is set exactly when `timerDueMs` is present, and `clock` is the value the agent reads from the returned batch.

`emit_children` sets `controls` for every touched or created `Control` even when the child list is unchanged (`paragraph.rs:333`). An otherwise empty touch of a control therefore takes a full apply. That is the conservative reading of the round-3 hole, and the Signal measurement still skips the idle polls.

### 4. `controls` coverage

- Create of an option part or button face sets the flag and returns (`paragraph.rs:210`).
- Update sets it for the same nodes (`:277`), after SVG has returned without the flag (`:274`).
- A control's children, including a `when` swap, set it in `emit_children` (`:333`).
- Destroy of a viewless id emits `batch.destroy` unless `exit_holds` or `native_selected_id` swallows it (`host.rs:1168`). The kernel touches the parent, so a control parent hits `emit_children` and a text parent hits the option-part update. The op or the flag keeps the batch out of the skip.
- `PresenterIOS.applySnapshots` requires `!batch.controls` (`PresenterIOS.swift:664`). A collections batch that also changed a face falls through to full `apply`, which still calls `controls.sync()` (`:902`).

### 5. Tests

See finding 1. They pin the predicate and one combined Contract flip. They leave clock arming and the other flag edges unasserted.

### 6. Accidental per-tick refresh

The amendment's claim holds. System appearance on iOS (`reportScheme`) and macOS (`viewDidChangeEffectiveAppearance`) assigns `session.scheme`, which calls `apply` directly, so `controls.sync` still reads the accent. Sheet geometry is replayed inside the dismissal apply (`ModalIOS.releaseBackground` → `applyGeometry` during `navigation.sync`), and `controls.sync` runs later in that same `presenter.apply`. A local sheet trait callback reapplies the node style and does not sync controls. That window already existed for an app with no timer, and the RFC accepts it. Collection reports, animated rasters, region answers, and SVG `seek` (a no-op while `session.clock` is unchanged) do not depend on the idle tick.

### Findings

1. **Nit** — `IdleTickTests` never arms the timer it claims to move — `host/apple/tests/ExactKitTests/IdleTickTests.swift:13`

`timerDueMs: 250` is an absolute deadline on `ExactEnv.wall()`, milliseconds since process start (`Session.swift:87`). By the time the test runs that deadline is in the past, so `SessionClockTimer.wake` (`Session.swift:1324`) returns `.frame` and `scheduleClock` never installs a `Timer`. The assertions only read `timerDue` and `appliedBatches`, so a break in the `.timeout` re-arm (`Session.swift:1127`), in JSON decoding of `controls` (`BatchShapes.swift:34`), or in the fill and feedback call sites would still pass. `controls.rs:35` only checks one press that changes a button title and an option's `disabled` together, against an unrelated text toggle.

Fix: build the idle batch with `timerDueMs: session.now() + 250`, and a second case with `nil`. Assert `clockDue` and that `clockTimer` is valid, then invalidated for nil. Decode a batch whose JSON contains `"controls":true` and assert the field. Add Contract cases that separate option or face create, a `when` children swap on a control, and destroy of an option, each asserting the flag or a destroy op.

Verdict: LAND WITH FIXES
