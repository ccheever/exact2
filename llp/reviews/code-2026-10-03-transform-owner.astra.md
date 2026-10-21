# Code review: the transform packet as an owner job (8ad2bc418), 2026-10-03 (astra)

- **Family:** OpenAI — `gpt-6-astra` via `codex exec`, reasoning effort xhigh, read-only sandbox, `-C` a detached worktree at `8ad2bc418`.
- **Method:** one brief (sha256 `4e24707fd373b5e188b1ece2032e3f7b2c4c2cb7ade39dc62459be02bc8f7bad`), shared with grok; one round; blind to the other review. Requested by Charlie through the coordinator. The author (Claude) is not a reviewer.
- **Transcription:** the run's final message, unedited.
- **Verdict:** LAND WITH FIXES.
- **Disposition:** the one finding is fixed in the commit after `8ad2bc418`. A dismissal's completion also calls `transformGeometry.changed()`, and LLP 1072 §8.12 says so. Grok argues dismissal reparents the viewport before its animation and so is covered already. Both may be right; the second measurement is one no-op pass when nothing changed.

---

1. **Should-fix — Refresh geometry after dismissal too.** The new invalidation runs only after presentation ([ModalIOS.swift:302](/tmp/rv-tf/host/apple/Sources/ExactKit/IOS/ModalIOS.swift:302)). When dismissing a nested sheet above a surviving photo viewer, an early measurement can see the underlying controller’s transition transform and reject its geometry ([TransformDragIOS.swift:126](/tmp/rv-tf/host/apple/Sources/ExactKit/IOS/TransformDragIOS.swift:126)). Dismissal completion only synchronizes navigation and calls `fit()` ([ModalIOS.swift:345](/tmp/rv-tf/host/apple/Sources/ExactKit/IOS/ModalIOS.swift:345)); unchanged size and insets need not produce another batch. The viewer can consequently remain undraggable until an unrelated layout or update. Add `transformGeometry.changed()` after dismissal completion and verify an animated nested-sheet round trip with `EXACT_AGENT_TIMING=platform`; default agent mode suppresses the transition that exposes this gap.

The owner-thread diagnosis is correct: the registry is thread-local, and missing entries return the named refusal ([exports.rs:26](/tmp/rv-tf/host/apple/src/abi/exports.rs:26), [abi.rs:1439](/tmp/rv-tf/host/apple/src/abi.rs:1439)). The fix correctly encloses input write, operation, and output decoding in one owner job. I found no other runtime-handle `exact_*` call outside owner routing under `host/apple/Sources`. AppKit mouse and trackpad paths use this same bridge.

Reentrant calls are refused without waiting ([Owner.swift:88](/tmp/rv-tf/host/apple/Sources/ExactKit/Owner.swift:88)). `TransformDragHold.init` aborts on `nil` before acquiring tokens; geometry refresh leaves its observation unaccepted. Ordinary refresh runs asynchronously on main, outside the owner’s callback service. The added presentation invalidation reads current bindings, so a retired layer’s completion does not replay stale geometry.

The test is meaningful: without the fix, the top-level registry refusal lacks the nested `batch` required by `TransformDragReply`, so [XCTUnwrap fails at line 16](/tmp/rv-tf/host/apple/tests/ExactKitTests/TransformMotionOwnerTests.swift:16). It proves routing, but does not cover modal geometry recovery.

Static review only; no files modified or tests run.

Verdict: LAND WITH FIXES
