# Code review: ResetGate's tables after #1141 and #1144, round 1 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `66cee861e6`.
- **Method:** one brief (sha256 `aa645e96b39403ae14797b6e47f953b7c0e1cd8240324f8e2aea01d5b91e7323`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. [ResetGate.swift:125](host/apple/Sources/ExactKit/ResetGate.swift:125) — **Incoming content is classified as teardown noise.** `navigate` and `hostGo` also deliver external URLs through `openIncoming`/`ExactSession.navigate`. `share` receives incoming shares; `ShareReceive.claimInbox()` deletes the inbox directory after delivery even when the batch is held. These belong in `knownLoss` unless their payloads are retained until the replacement tree is ready. The existing `change` entry has the same mixed use: `ExactDocuments.deliver()` sends file paths through it. Correct the classifications and initial baseline, or guard these external entry points before consuming their payloads.

2. [reset_gate.rs:184](host/apple/tests/it/reset_gate.rs:184) — **The scan can miss held calls and accept nonexistent ones.** A valid `return self.on { … }` is explicitly excluded; `on` followed by a newline and parenthesized closure calls are missed. Requiring the enclosing return type to contain `Batch` also misses a method that consumes a Batch internally. Conversely, line 175 scans raw comments, so `// runtime.holds("obsolete()")` can keep a stale key alive. Nested functions’ `on` calls are attributed to enclosing Batch-returning functions too. The `>50` assertion prevents an empty pass, but these omissions still pass. Use scoped token recognition, reject unsupported holding forms, and add fixtures exercising these cases and both set differences.

3. [ResetGateTests.swift:303](host/apple/tests/ExactKitTests/ResetGateTests.swift:303), [ResetGateTests.swift:338](host/apple/tests/ExactKitTests/ResetGateTests.swift:338) — **Condition (2)’s recovery coverage is incomplete.** Both new tests force `asyncFills = false`, leaving asynchronous recovery across restart unproved. The native test proves synchronous queue retry and subsequent generation re-arming, but its held calls occur before `session.apply`. The admission test discards the captured held-call list and manually invokes `check` after restarting; it can succeed without proving that admission was held or automatically rescheduled. Exercise both modes during the real reset, assert each exact signature and absence of replay, then observe recovery through the queue/update callbacks without manually calling `check`.

**Non-blocking**

1. [ResetGate.swift:147](host/apple/Sources/ExactKit/ResetGate.swift:147) — **Several `knownLoss` reasons overlook existing recovery.** `activateData` and `deliverModuleEvents` guard before consuming their work and already request replay. After reset, `presentCommitted` calls `tellTime()` and `view.rebooted()`, which resend preferences, scheme and segments. Tint reports are deferred through `colorsOwed`; list-margin caches are cleared and measured again. Reclassify these entries with their actual recovery reasons, or identify an unguarded caller demonstrating the claimed loss.

2. [ResetGateTests.swift:296](host/apple/tests/ExactKitTests/ResetGateTests.swift:296) — `entered >= 0` proves nothing. The test establishes signature classification, but permits the replay to do no runtime work. Arrange due work and assert its effect, or remove the tautological assertion.

The three required signature re-keys correctly map to `.advance`. No tests or builds were run.
