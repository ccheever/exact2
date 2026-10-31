# Code review: a local-PDF iframe makes no web view it then drops (the ScreenTime KVO red, not proven the cause), round 3, narrow re-check (grok)

- **Family:** xAI. Grok via the grok CLI, reasoning effort xhigh, edits denied, a throwaway worktree at `9efb1ada9f`.
- **Method:** one brief (sha256 `3d6afdd22e286fae0522b86b14f40185ab57a9c64480f5ff424f79d2266d9e7f`), shared by both reviewers, blind to the other review. The author (Claude, iosfix2) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes.
- **Disposition:** cb7539daa3 tried a src-wait; the next re-check showed it could strand a frame, so it is reverted (7a1bae8732) and the case is a named residual

## Brief

Narrow re-check of round 3's fixes (host change: a local-PDF iframe shows the PDF's view from its mount and makes no web view it then drops; the ScreenTime KVO red, not proven to be the cause). HEAD 9efb1ada9f; round 3 reviewed 70e7123573. Read `git diff 70e7123573..HEAD` (2 commits), with context from `git diff 125ee17e91..HEAD`.

Round 3 findings, and the fixes:
- **Astra P1: iOS test compilation failed.** ConfiguringFn, configured, swapsStore and configuring were inside `#if os(macOS)`. They're now declared outside it, for both platforms. The iOS test build succeeds, and the three new tests pass there.
- **Grok P2: clearing `partition` on an unsandboxed local PDF built a web view in configure that serve then dropped.** configure's replacement now calls waitingPDF() when startsAsPDF holds (the hint, no partition, no sandbox, nothing built), and otherwise build() and closePDF() as before. Test: testALocalPDFLosingItsPartitionMakesNoSecondWebView mounts partitioned (WKWebView, one construction), configures {"pdf": true}, and ends on the PDF view with still one construction. It fails on 9618374f83 (two constructions).
- **Grok note: the failed read's "starts on the PDF view" wasn't asserted.** IframeCaptureSessionTests now asserts the missing-PDF frame's view is a PDFView right after the mount.
Full suites on 70e7123573: Mac failed AgentSocketStartupTests' timing (unrelated; it passed alone after); the iOS build failed (fixed above). The suites on this tip are running. Caps is green.

Check only:
(a) Are these fixes correct and complete?
(b) The configure path: any case where startsAsPDF now holds wrongly (adding a partition; a frame that was showing a PDF; the hint while a sandbox is pending)?
(c) Any new build-and-drop, double view, or lost kind 7?
Do NOT edit files. Give findings with file:line and severity. End with one line: VERDICT: APPROVE or VERDICT: CHANGES.

## Review

I'll re-check the two commits since round 3 against the three specific questions: whether the fixes are complete, whether `startsAsPDF` can fire in the wrong case, and whether any new build-and-drop or lost kind remains.The iOS move, the partition-loss path the new test covers, and the missing-PDF assertion are right. One same-update ordering still builds a web view and drops it.

**P2. `host/apple/webarm/WebArm.swift:587` with `WebArm.swift:788` and `host/apple/Sources/ExactKit/WebModule.swift:360`.** Clearing `partition` in the same update that changes `src` still constructs a `WKWebView` and tears it down. `configure` runs before `setSrc`. With the hint set, `partition` already nil, and `sandbox` nil, `startsAsPDF` is true, so the new branch calls `waitingPDF()` and `scheduleServe()`. That serve is queued before the file read finishes (`WebModule.swift:398` then `417`). It still sees the previous `src`. `serve` at `WebArm.swift:788` does not take `showPDF`, and `closePDF()` at `FramePDF.swift:163` builds a web view and emits kind 7 `{}`. The read completion then `setSrc`s the PDF, and `showPDF` at `FramePDF.swift:144` tears that web view down. The new test never hits this: it mounts `app:/tmp/doc.pdf` and only then sends `{"pdf": true}`.

Checked and holding:

- Adding a partition cannot take `startsAsPDF`. `partition = name` is at `WebArm.swift:584`, before the check, and `WebModule.swift:310` omits the hint when `partition` is set. The branch builds once and `closePDF` emits kind 7 `{}`.
- A frame already showing a PDF does not take it either. With `pdf != nil` and the same nil partition, the guard at `WebArm.swift:578` returns. `waitingPDF()` is not called again.
- A sandbox that is about to be applied does not make the predicate true. The hint is sent only when `props["sandbox"]` is nil (`WebModule.swift:310`), in the same `update` that will call `setSandbox`. The predicate reads the arm's current `sandbox` (`WebArm.swift:253`). A clear that has not reached the arm yet makes `startsAsPDF` false, so this branch builds; that prior state is a partitioned frame that also had a sandbox.
- Kind 7 is still emitted for the path the test covers: `showPDF` sends `{"pdf": pages}` (`FramePDF.swift:150`). The missing-file assertion is on the synchronous mount, before the async failed read: non-list iframes are released in `HeavyLeavesMac.swift:79`, and `node("gone")` is an unsandboxed `app:/` PDF in `scripts/fixtures/iframe-pdf.contract:18`.
- The hook helpers now live before `#if os(macOS)` at `IframeCaptureTests.swift:493`, so the iOS tests that count constructions see one definition.

VERDICT: CHANGES
