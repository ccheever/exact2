# Code review: a local-PDF iframe makes no web view it then drops (the ScreenTime KVO red, not proven the cause), round 2 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `3e5b6f9eee`.
- **Method:** one brief (sha256 `3ee88c11c3ed7a4f520adde09e02ced538bb77c8ceca5a3249e81c4a0a3e8548`), shared by both reviewers, blind to the other review. The author (Claude, iosfix2) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes.
- **Disposition:** fixed in 70e7123573, the sandbox test failing before the fix

## Brief

Code review, round 2 of 3 (host change: a local-PDF iframe shows the PDF's view from its mount and makes no web view it then drops; the ScreenTime KVO red, NOT proven to be the cause). Tip 3e5b6f9eee is HEAD of this worktree; base 125ee17e91. Review the whole change with `git diff 125ee17e91..HEAD`, and round 1's fixes with `git diff d9f87da63c..HEAD`.

Round 1 findings (Astra and Grok both CHANGES), and the fixes:
- **Astra P2 and Grok P1: a failed app:/ read of a hinted PDF left the frame with a detached PDF view.** The fallback's kind 7 was dropped by the fileFailed guard. Now WebModule.receivedEvent lets kind 7 through that guard (loads and the rest still drop), so replaceView shows the web view and pdfPages updates. Test: a second frame in scripts/fixtures/iframe-pdf.contract names a missing PDF, and IframeCaptureSessionTests waits for its WKWebView in the window. That fails on d9f87da63c (node.web stays the ExactPDFView).
- **Astra P2: a PDF shown from the mount had no `ui`, so a link to another app was silently dropped.** `ui` is now made by makeUI(), from build() and from waitingPDF(). Test (Mac): testALinkToAnotherAppInAPDFShownFromTheMountAsks calls the PDF view's link delegate with mailto: and finds the confirmation sheet on the window (skipped if no app opens mailto:). It fails on d9f87da63c.
- **Astra note: the original crashing test still built a web view.** testALocalPDFIsAPDFView now mounts with "pdf": true, as WebModule configures it.
- **IframeCaptureSessionTests' expectations** follow the new order: the hatch is built with the PDF view, so there's no `changed` until PDF -> HTML, then one. LLP 1020.002 §9 already says the hatch hands "the PDFView of a local PDF", and `changed` when the arm replaces the view.
- **LLP 1020.002 D10** has a new row for the first view.
Condition (1) results, stated: the real arm looped 200 times beside a cargo build, 2 runs each on main and on the branch, gave 0 occurrences on both.
Full suites on d9f87da63c: Mac failed IframeCaptureSessionTests (fixed above); iOS failed NavigationTabsIOSTests.testATabFollowedAfterItsPopStartsCompletesNavigation, which is unrelated and being rechecked. The suites on this tip are running. Caps is green.

Check hard:
- the kind 7 exception in the failure guard (any event kind-7 carries that a failed frame shouldn't publish);
- makeUI before any web view (FrameUI's assumptions about having a web view; iOS's FrameUIAgent);
- the fixture change (the missing-file frame's error, other assertions in the session test);
- anything else treating `pdf != nil` as a document showing;
- whether the tests prove what they claim.
Do NOT edit files. Give findings with file:line and severity. End with one line: VERDICT: APPROVE or VERDICT: CHANGES.

## Review

- **P2 — Honor the sandbox before selecting the initial view.** [WebModule.swift:309](/tmp/iosfix2-rv-pdf2-astra/host/apple/Sources/ExactKit/WebModule.swift:309) sets the PDF hint for sandboxed sources too. `create()` requests `platformView` before `update()` supplies the sandbox, so `startsAsPDF` sees `sandbox == nil`. A sandboxed local PDF therefore mounts—and reaches its hatch—as a `PDFView`, then replaces it with a `WKWebView` after the read. This changes its initial platform object and hatch ordering despite D10 requiring sandboxed frames to stay WebKit’s. Exclude authored sandboxes from the hint. The existing sandbox test uses `configuration: [:]`, so it misses this production path.

- **P3 — The allocation test only observes attached views.** [IframeCaptureTests.swift:305](/tmp/iosfix2-rv-pdf2-astra/host/apple/tests/ExactKitTests/IframeCaptureTests.swift:305) counts the window’s current subviews. A web view constructed and discarded between observations would escape both assertions. Count the existing `exact_web_set_configuring` callback through PDF loading to establish that the arm never constructs a web view.

The round-1 failure guard and `makeUI()` fixes look sound, including `FrameUIAgent`. Kind 7 only updates view ownership and PDF metadata.

Reviewed both requested diffs; `git diff --check` passes. Runtime tests were not run: this checkout is read-only and has no build artifacts. ScreenTime causality remains unproven.

VERDICT: CHANGES
