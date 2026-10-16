# Code review: DOM's `scrollend` round 2, 2026-10-13 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `1f837fefe8f99c20fbad3a66e512eb47319994d0e127c692cdd819038f932573`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited (the Grok CLI's progress lines before it dropped).
- **Verdict:** NOT READY.
- **Disposition:** Every finding taken in round 3's code.

---

- **MAJOR — A stationary touch can still lose the iOS end.** [ScrollEndIOS.swift:15](/Users/admin/x2p2/apps/exact2-comp/host/apple/Sources/ExactKit/IOS/ScrollEndIOS.swift:15) cancels the quiet check whenever `isTracking` is true. If an authored jump or anchor adjustment moves the offset during a touch that never becomes a drag, releasing it supplies no drag/deceleration/animation completion. The end remains owed indefinitely. UIKit explicitly allows [tracking without dragging](https://developer.apple.com/documentation/uikit/uiscrollview/istracking). Round 1’s retry only helps when a quiet check survives. Keep that retry armed for tracking-only movement.

- **MAJOR — Flushing the final scroll can deliver the end to a reused iOS node.** [ScrollEndIOS.swift:29](/Users/admin/x2p2/apps/exact2-comp/host/apple/Sources/ExactKit/IOS/ScrollEndIOS.swift:29) validates identity before `sendScrollEvent()`, then reads `id` again afterward. Outside batch application, `Presenter.send` invokes the handler synchronously. A final animation-frame scroll handler can retire and reuse a pooled inner-list view; the subsequent `scrollend(id)` then targets its new node. Capture the original ID/incarnation and revalidate after flushing. The waiting queue preserves ordinary ordering but does not protect this immediate-delivery path.

- **MAJOR — The JS target can invoke a removed scroller’s handler.** The new admission at [emit.rs:1318](/Users/admin/x2p2/apps/exact2-comp/host/web-js/src/emit.rs:1318) uses the plain listener in [rt.js:888](/Users/admin/x2p2/apps/exact2-comp/host/web-js/rt.js:888). That wrapper advances due tasks before invoking the handler, but checks whether the element/owner survived only for `close`. A due task can remove the scroller, after which its `scrollend` action still runs. An isolated execution of the actual wrapper reproduced this: one callback after removal for `scrollend`, zero for `close`. Apply the same lifetime guard and cleanup to `scrollend`.

The remaining round-1 fixes look correct on inspection, including offset comparison, follow-animation deferral, cancellation across pooling, and Mac observer cleanup. Compiler/ABI wiring, swipe-row activation, and the changed generated plan digest also checked out.

Existing binaries passed both Contract tests, the JS emitter test, and all three Mac tests. The two iOS findings are source-traced; I did not rerun iOS or establish whether its reported suite failures occur on main.

The API serves bsky5’s intended replacement for `calm`/`snap`: set `flinging` when needed, then clear it and snap on `scrollend`. These lifecycle gaps need fixing first. Removing `scrollTick` eliminates timer-reset writes; changing `headerY` can still require presenter work.

NOT READY
