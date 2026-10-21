# Code review: the transform packet as an owner job (8ad2bc418), 2026-10-03 (grok)

- **Family:** xAI — `~/.grok/bin/grok -m grok-4.7 --reasoning-effort xhigh --always-approve --no-subagents --output-format plain --prompt-file <brief>`, headless, `--cwd` a detached worktree at `8ad2bc418`; read-only by instruction, the worktree clean afterwards.
- **Method:** the shared brief (sha256 `4e24707fd373b5e188b1ece2032e3f7b2c4c2cb7ade39dc62459be02bc8f7bad`); one round; blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** stdout, unedited; the first three sentences are progress narration.
- **Verdict:** LAND.
- **Disposition:** no findings. Astra's dismissal remeasure was added anyway (see the astra artifact).

---

I'll review that one commit read-only: the diagnosis, whether the owner-thread wrap is complete, and whether the modal and test changes actually hold.The review skill would write artifacts; this request is read-only, so I'll inspect the commit and call sites directly and report findings only.The commit is the parent they named. Next I’ll read the owner-job path and every `exact_*` call that takes a runtime handle.The diagnosis is right and the fix is complete. A transform-drag packet from the main thread now runs as an owner job, the same way every other runtime call does, and the modal hook is the missing remeasure after an iOS zoom transition. No other `exact_*` call in `host/apple/Sources` still takes a runtime handle off the owner thread.

## Diagnosis

`exact_create` registers the runtime in the owner thread’s thread-local registry (`host/apple/src/abi.rs:1385`, `with_runtime` at `abi.rs:1425`). A miss writes `runtime N: no such runtime (destroyed, or never created)` into that thread’s refusal buffer (`abi.rs:1394`) and `exact_out` returns that buffer (`abi/exports.rs:129`). Before this commit, `Runtime.transformMotion` called `exact_transform_motion` on the caller. From main the registry is empty, so every geometry report (op 10), begin (op 11), move (op 12), and release (op 13) missed. The refusal JSON has no `batch` key, so `TransformDragReply.init` returns nil (`TransformDragBridge.swift:41`) and the hold never starts (`TransformDragHold.swift:41`). That matches LLP 1072 §8.12 and the photo-viewer failure.

## The owner-job wrap

`transformMotion` now goes through `on(busy: nil)` (`TransformDragBridge.swift:57`), and both `write` (`exact_in`) and `exact_transform_motion` / `exact_out` sit inside that job. Splitting them would leave the packet in main’s empty registry and the owner reading an empty buffer.

Every other `exact_*` call in `host/apple/Sources` that takes a runtime handle is already inside `Runtime.on`, `Owner.shared.sync`, `post`, or `notify` (`Bridge.swift`, and `exact_set_app_module` at `NativeModule.swift:457`). `exact_log` / `exact_app_changed` are notifications. Raster, markup, textflow, and `exact_gesture_constant` do not take a runtime handle. Nothing else in this tree has the old bug.

## Reentrancy and `busy: nil`

`Owner.sync` returns the busy value only when main is inside `callMain` (`serving > 0`, `Owner.swift:88`). `transformMotion` then returns nil, same as a decode failure.

That window is not hit by the photo path:

- `TransformGeometryHost.refresh` runs only from `DispatchQueue.main.async` (`TransformGeometry.swift:35`). The main wait loop does not pump the run loop, so that block runs after `serving` is back to 0.
- `changed()` during `refresh` is swallowed by `delivering` (`TransformGeometry.swift:28`, set at `TransformGeometry.swift:70`).
- Pan and pinch call `TransformDragHold` from the gesture callback (`TransformDragIOS.swift:91`, `MouseTransformDragMac.swift:51`), which is outside `callMain`.
- Ops 11 and 12 do not `runner.dispatch`. Op 10 dispatches only when a geometry handler exists, and op 13 dispatches the release, both still inside the owner job. The Swift apply of the returned batch happens after `transformMotion` returns (`TransformGeometry.swift:73`, `TransformDragHold.swift:51`). Text measure stays on the owner; `readerMeasure` / `native.call` hop to main, but those callbacks do not call `transformMotion`.

If nil ever did come back, callers already treat it as “do not start”: `refresh` leaves `accepted == false` and does not poll (`TransformGeometry.swift:65`, `TransformGeometry.swift:79`); `TransformDragHold.init` returns nil (`TransformDragHold.swift:41`); `move` returns false (`TransformDragHold.swift:105`). A later `changed()` uses a new sequence. No retry is required for the path that was broken.

## ModalIOS

The extra `presenter.transformGeometry.changed()` is needed on iOS and safe.

`transformFacts` returns nil while any ancestor, including the presentation layer, has a non-identity transform or a transform animation (`TransformDragIOS.swift:123`). During an iOS 18 zoom that becomes op 14, which clears `ready`. `current()` then stays nil (`TransformGeometry.swift:42`), so the pan cannot begin. The viewport is moved into the modal at the start of `present` (`ModalIOS.swift:288`), so `didMoveToWindow` / `layoutSubviews` (`NodeViewIOS.swift:516`, `NodeViewIOS.swift:1270`) measure while the transition transforms are still up. Nothing moves the viewport again when the transition ends. `fit()` only lays out if the size changed. The completion call is the settle signal (`ModalIOS.swift:302`).

Dismissal does not have that hole. `closeTop` reparents the viewport back to its home before the dismiss animation, which fires `didMoveToWindow` outside the transitioning view. `changed()` running when the layer is already gone is the same reparent case; `drainRetired()` then animates a controller that no longer contains the viewport. Nested sheets share one presenter; the top presentation’s completion remeasures every binding, including a covered route whose ancestors are identity again. With `ExactEnv.agentFreezes`, `present` is not animated (`ModalIOS.swift:290`), so this is a second pass over unchanged facts and `refresh` no-ops when the prior observation was accepted (`TransformGeometry.swift:61`).

## Test

`TransformMotionOwnerTests` fails without the fix and passes with it. `Runtime()` creates the runtime on the owner (`Bridge.swift:70`). The test calls `transformMotion` from the XCTest main thread. Without the wrap, the reply is nil and `XCTUnwrap` fails (`TransformMotionOwnerTests.swift:16`). With the wrap, an unbooted runtime returns `{"accepted":false,"batch":{…"error":"not booted"}}` (`abi.rs:958`), which decodes, and the error does not contain `no such runtime`. It does not boot a session or present a modal; it locks the registry bug, which is the failure that refused every drag.

## macOS

AppKit uses the same `TransformDragHold` and `transformMotion` (`MouseTransformDragMac.swift:51`, `MouseTransformDragMac.swift:104`). The owner-job wrap is what macOS needed. There is no UIKit zoom transition. `viewDidMoveToWindow` and `layout` already call `changed()` (`NodeViewMac.swift:1174`, `NodeViewMac.swift:1194`), and mouse-down calls it again before the hold (`MouseTransformDragMac.swift:33`). No AppKit counterpart of the `ModalIOS` line is required.

No findings.

Verdict: LAND

