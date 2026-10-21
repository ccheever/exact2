# Code review: an idle timer tick only moves the clock (557c19407), 2026-10-04 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, read-only sandbox, `-C` a detached worktree at `557c19407`.
- **Method:** one brief (sha256 `3aac06ca09b60bb101ebd3f43c0abc589b0a335b94b02061535d1162177f9518`), shared with grok. Round 1, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the run's final message, unedited.
- **Verdict:** LAND WITH FIXES.
- **Disposition (r2):**
  - 1 (surface work after a deferred GPU load): taken. The load callback drains the queued work, on iOS and macOS.
  - 2 (sampler `seq`): taken. A skipped tick reports its range to the sampler at zero cost.
  - 3 (a subtree's appearance leaves accents stale): taken. A control node's appearance change requests a coalesced control sync on the next turn, and each control's size, weight and scale traits do too.
  - 4 (geometry replay outside a batch): taken. `applyGeometry` outside `apply` requests the same coalesced sync.
  - 5 (tests): taken in part. The test uses a real deadline and asserts the armed timer and the nil case; a decode test reads `"controls":true`; Rust cases separate face text, option and face swap.

---

Static review of `557c19407`; no files modified or tests run.

1. **Should-fix — Pending surface work can lose its remaining drain opportunity.**  
   [Session.swift:912](/tmp/x11-review/host/apple/Sources/ExactKit/Session.swift:912), [Session.swift:1052](/tmp/x11-review/host/apple/Sources/ExactKit/Session.swift:1052), [GpuIOS.swift:284](/tmp/x11-review/host/apple/Sources/ExactKit/IOS/GpuIOS.swift:284).

   After first paint, a batch can mount a canvas backed by a deferred GPU artifact and enqueue capture/restore work. `drainSurfaceWork` waits because the artifact is not ready. The deferred load callback subsequently loads—or fails—and requests a frame, but never drains the queued work. A failed load provides a particularly clear case: the canvas frame does not produce another session batch. Previously, the next idle timer applied and delivered the request’s failure; now every idle tick can skip it indefinitely. AppKit has the same callback pattern.

   **Fix:** Drain pending surface work explicitly when deferred loading completes, on both success and failure. Add a regression asserting that the request settles without an unrelated batch.

2. **Should-fix — Skipped ticks disappear from frame diagnostics.**  
   [Session.swift:913](/tmp/x11-review/host/apple/Sources/ExactKit/Session.swift:913), [Session.swift:929](/tmp/x11-review/host/apple/Sources/ExactKit/Session.swift:929), [runner/perf.rs:68](/tmp/x11-review/runner/src/runner/perf.rs:68).

   An empty presentation batch can still carry timer transactions in `seq`. Producing its trailer advances `seq_range`’s reported cursor, so later batches will not report those transactions again. The skip omits `sampler.batch`, losing their frame attribution and activity notification. Repeated skipped polls can also let sampling stop despite continued timer work.

   **Fix:** Record consumed sequence ranges and sampler activity independently of presenter application, with zero presentation cost for skipped batches. Test an empty tick carrying `seq` against the next sampled frame.

3. **Should-fix — Subtree appearance changes leave native accents stale.**  
   [Session.swift:912](/tmp/x11-review/host/apple/Sources/ExactKit/Session.swift:912), [NodeViewIOS.swift:487](/tmp/x11-review/host/apple/Sources/ExactKit/IOS/NodeViewIOS.swift:487), [ControlsIOS.swift:131](/tmp/x11-review/host/apple/Sources/ExactKit/IOS/ControlsIOS.swift:131).

   Change only a control subtree’s appearance while leaving `ExactView`’s traits unchanged. The node callback reapplies its style but never synchronizes its control. Control accents are resolved into fixed colours during `controls.sync`; UIKit cannot re-resolve that stored colour. Previously, a timer app recovered on its next empty poll. It now retains the old accent until another applied batch.

   **Fix:** Schedule a coalesced control refresh from the actual node/control trait callback. Include sizing-trait invalidation for intrinsic measurements. Test a descendant-only override without an unrelated operation. Ordinary root appearance changes retain their explicit `session.scheme` application.

4. **Should-fix — Geometry replay outside a presenter pass can leave controls at their old size.**  
   [ModalIOS.swift:243](/tmp/x11-review/host/apple/Sources/ExactKit/IOS/ModalIOS.swift:243), [NavigationIOS.swift:444](/tmp/x11-review/host/apple/Sources/ExactKit/IOS/NavigationIOS.swift:444), [PresenterIOS.swift:972](/tmp/x11-review/host/apple/Sources/ExactKit/IOS/PresenterIOS.swift:972).

   A concrete case is changing a background button’s width beneath two fullscreen presentations, then dismissing both. The first dismissal runs inside the applied batch; the remaining dismissal can run from `modalDidDismiss` through `navigation.sync` alone. Its replay updates the `NodeView` frame, but control sizing happens in `controls.sync`. With unchanged viewport dimensions and covers, no resize batch repairs it. The former empty poll did; the new skip does not.

   **Fix:** Make actual geometry replay schedule control layout synchronization, including replay outside `Presenter.apply`. Test dismissal through native completion callbacks with unchanged viewport size.

5. **Nit — The new tests establish substantially less than the claimed behavior.**  
   [IdleTickTests.swift:9](/tmp/x11-review/host/apple/tests/ExactKitTests/IdleTickTests.swift:9), [controls.rs:35](/tmp/x11-review/host/apple/tests/it/controls.rs:35).

   The Swift test uses an unbooted session and manually constructed batches; it checks entry counts and stored deadlines, not timer delivery or owed work. The Rust test changes button text and option state together, so either producer alone could satisfy its assertion. Neither exercises serialized `controls` through collection feedback or the snapshot shortcut.

   **Fix:** Add focused regressions for the findings above, actual timer rearming, and separate face/option changes delivered through those collection paths.

The remaining audit supports the implementation’s basic structure:

- `changesNothing` **tightens** the old fill/feedback predicates; the additional canvas, frame-task, image and control checks are appropriate. Their deadline comparisons remain intact.
- `scheduleClock` preserves nil deadlines, armed identical deadlines, near-deadline frame wakes, frame tasks and agent-clock behavior. Reentrant application and in-flight owner work fall back to `apply`. Agent and frame-driven advances still use their existing application paths.
- Control create/update/children coverage checks out; destruction touches the surviving parent and/or emits a destroy operation. The snapshot guard now respects `controls`.
- I found no additional definite regression in animated-raster scheduling, lifted-row ordering, first-paint activation, collection continuations, ordinary outermost drains, input-hold checks, transform notifications or macOS region callbacks.

Timerless apps can already exhibit the missing-invalidation cases above. That explains the pre-existing weakness; it does not establish that removing timer apps’ recovery is behavior-preserving.

Verdict: LAND WITH FIXES
