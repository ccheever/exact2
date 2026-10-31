# Code review: a local-PDF iframe makes no web view it then drops (the ScreenTime KVO red, not proven the cause), round 2 (grok)

- **Family:** xAI. Grok via the grok CLI, reasoning effort xhigh, edits denied, a throwaway worktree at `3e5b6f9eee`.
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

I'll review the whole change from `125ee17e91` to HEAD and the round-1 fixes from `d9f87da63c`, focusing on the kind-7 failure path, `makeUI`, and whether the tests actually prove the claims.The tree is clean at `3e5b6f9eee`. I'll review the full range from `125ee17e91` and the round-1 fixes from `d9f87da63c` against the failure-guard, `makeUI`, and test claims.The diff is six files. Next I’ll read the failure path, `FrameUI`, and the tests around the new kind-7 and mount-PDF behavior.[P2] A sandboxed local PDF is mounted as a PDF view — `host/apple/webarm/WebArm.swift:253`

`startsAsPDF` refuses a PDF view when `sandbox != nil`, but that field is still nil the first time the view is taken. `WebModule.create` configures the arm and calls `platformView` before any `setSandbox`. The `pdf` hint is set from the file name alone (`WebModule.swift:309`), and `sandbox` is not in that configuration. It is sent only after the async read (`WebModule.swift:408` and `WebModule.swift:417`). Until that callback, a sandboxed `app:/…pdf` frame is a `FramePDF`: the node shows it, a hatch's first platform object can be that `PDFView`, and `pdf != nil` makes capture, snapshot, `postMessage`, and script treat it as a PDF document. `serve` does recheck `sandbox` before `showPDF` (`WebArm.swift:788`), so the bytes are not drawn in PDFKit, and the end view is WebKit. `testASandboxedPDFStaysWebKits` (`IframeCaptureTests.swift:433`) mounts with `configuration: [:]` and never sets `pdf`, so it does not take this path.

The kind-7 failure exception is safe. A missing `app:/` file sets `fileFailed` and then `setSrc(nil)`; `serve` calls `closePDF`, which emits kind 7 as `{}`. That clears `pdfPages` and runs `replaceView`. Kinds 0, 2, 4, and 5 stay dropped, so the fallback page does not publish a load or a navigation. `makeUI` runs after `configure` has set `agent`, and `FrameUI` / `FrameUIAgent` do not touch a web view in `init`. `confirmExternal` uses the view it is given. The mailto test shows the sheet when the PDF delegate fires; the no-web-view test checks the window after mount and after the document arrives. The session test's missing-file frame has no `error` handler, so the host does not apply an error event, and the capture, tree, and `changed` assertions stay on `doc`.

VERDICT: CHANGES
