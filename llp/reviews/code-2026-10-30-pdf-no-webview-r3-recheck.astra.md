# Code review: a local-PDF iframe makes no web view it then drops (the ScreenTime KVO red, not proven the cause), round 3, narrow re-check (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `9efb1ada9f`.
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

- **P2 — Build-and-drop remains when the PDF source is pending.** At [WebArm.swift:587](/tmp/iosfix2-rv-pdf4-astra/host/apple/webarm/WebArm.swift:587), clearing `partition` while changing `src` from a web page to a local PDF creates `waitingPDF()` and immediately schedules `serve()`. But [WebModule.swift:360](/tmp/iosfix2-rv-pdf4-astra/host/apple/Sources/ExactKit/WebModule.swift:360) configures before asynchronously delivering the new source. The queued serve therefore sees the old URL, closes the waiting PDF and constructs a WKWebView; delivering the PDF then tears that web view down. The replacement needs to wait for the corresponding source. The new test supplies the document before reconfiguration, so it misses this case.

The helper relocation and missing-PDF assertion are correct. No additional issue found with adding a partition, retaining an existing PDF, the pending-sandbox guard, or kind 7 delivery.

Source review only; XCTest was not rerun in this read-only workspace.

VERDICT: CHANGES
