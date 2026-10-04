# Code review, round 2: iOS host batch cost (bbe96611b), 2026-10-04 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, read-only sandbox, `-C` a detached worktree at `bbe96611b`.
- **Method:** one brief (sha256 `1cce0a4c559f7ed242bf61d354404d2fa43b7c10303b2ad8f543deab87e708bf`), shared with grok. Round 2, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the run's final message, unedited.
- **Verdict:** DO NOT LAND.
- **Disposition (r3):**
  - 1 (a control change can make a batch with no ops): taken. The Apple host's projection sets `"controls":true` when it suppresses a control's viewless contents: an option part's create or update, or a control's children. The presenter syncs on it. `host/apple/tests/it/controls.rs` checks the flag from real Contract batches.
  - 2 (subtree Dynamic Type): taken. Each control registers for content-size category and display scale and marks the controls stale.

- **Superseded (2026-10-04):** the branch these reviews were of was abandoned after round 3 and not landed. See `code-2026-10-04-batch-cost-r3.*` and `code-2026-10-04-idle-tick.*`.

---

Static review only; no files changed or tests run. **Round 1 is only partly resolved: the viewless-child blocker remains.**

1. **Blocker — A real control change can produce an entirely empty batch.**  
   [PresenterIOS.swift:890](/tmp/x11-review/host/apple/Sources/ExactKit/IOS/PresenterIOS.swift:890) assumes `ops.isEmpty` means no control input changed. But [paragraph.rs:273](/tmp/x11-review/host/apple/src/paragraph.rs:273) suppresses updates for native-button children and select options. Control child lists are also suppressed at [paragraph.rs:327](/tmp/x11-review/host/apple/src/paragraph.rs:327).

   A concrete case is changing only an existing option’s `disabled` state after boot has settled. Its update emits nothing, its child list is unchanged, and no geometry needs to move. The entire batch can therefore contain `ops: []`. `controlsStale` remains false, so the existing `UIMenu` retains the old enabled state. Button child-text changes have the same problem; their natural size is reported only after control synchronization.

   The revised test actually demonstrates the stale result, then supplies an unrelated parent operation to repair it: [NativeButtonsIOSTests.swift:73](/tmp/x11-review/host/apple/tests/ExactKitTests/NativeButtonsIOSTests.swift:73).

   **Fix:** Emit an owning-control invalidation from the Rust projection whenever viewless face/options inputs change. An empty `props` operation on the owner would make this gate work without introducing another operation kind. Until that exists, retain unconditional synchronization. Add a Contract→Rust→presenter regression covering an option-only change and a button-child-only change, without an unrelated operation.

2. **Should-fix — Subtree Dynamic Type changes bypass the new registration.**  
   [ExactViewIOS.swift:77](/tmp/x11-review/host/apple/Sources/ExactKit/IOS/ExactViewIOS.swift:77) observes content-size category only on the outer `ExactView`. [NodeViewIOS.swift:495](/tmp/x11-review/host/apple/Sources/ExactKit/IOS/NodeViewIOS.swift:495) observes only interface style.

   A route or element hook can change a descendant controller/view’s preferred content-size category while leaving `ExactView`’s traits unchanged. Reparenting retained content beneath such an override has the same effect. UIKit’s button intrinsic size changes, but neither registration marks controls stale. Subsequent empty batches skip the measurement, frame assignment, and intrinsic-size report in [ControlsIOS.swift:159](/tmp/x11-review/host/apple/Sources/ExactKit/IOS/ControlsIOS.swift:159), leaving the control’s allocated geometry stale.

   **Fix:** Observe sizing traits on the actual controls or their owning nodes, including content-size category and display scale, and invalidate the presenter there. Test a descendant-only category override followed by an empty batch.

The other round-1 dispositions are addressed:

- Interface-style changes now mark controls stale. Deferred modal geometry uses `applyGeometry`, which marks stale before the post-navigation control sync.
- Removing `HeaderTitleView.update`’s early return restores identifier updates and intrinsic-size invalidation.
- The avatar cache has a 64-entry limit and includes the renderer scale.
- The obsolete per-control `synced` bookkeeping is gone.

For the remaining requested paths, I found no additional regression from this skip. Keyboard/safe-area changes either affect viewport scrolling alone or produce layout operations. Glass-slot creation/removal moves the control itself, including reconciliation after sync. Grouped/swipe-row carrying preserves local sizes; pooled controls are recreated through create operations. Ordinary route reparenting preserves control geometry, subject to finding 2. Navigation and segments read faces from the kernel. Activation callbacks reconcile committed values, pending intrinsic reports remain queued, and `waiting` still drains independently of the skip.

`controlsStale` starts true at [PresenterIOS.swift:19](/tmp/x11-review/host/apple/Sources/ExactKit/IOS/PresenterIOS.swift:19) and clears after synchronization. [reset():318](/tmp/x11-review/host/apple/Sources/ExactKit/IOS/PresenterIOS.swift:318) does **not** restore it to true, but removes every control; normal reboot create operations force synchronization, so I found no restart failure from that omission.

`view.session.presenter` is valid: `ExactView` strongly owns its session, and the session owns a nonoptional `let presenter` at [Session.swift:374](/tmp/x11-review/host/apple/Sources/ExactKit/Session.swift:374). Destruction resets that presenter rather than removing it.

Verdict: DO NOT LAND
