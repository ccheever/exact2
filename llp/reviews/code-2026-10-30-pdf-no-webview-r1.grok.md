# Code review: a local-PDF iframe makes no web view it then drops (the ScreenTime KVO red, not proven the cause), round 1 (grok)

- **Family:** xAI. Grok via the grok CLI, reasoning effort xhigh, edits denied, a throwaway worktree at `d9f87da63c`.
- **Method:** one brief (sha256 `7f7af0439b3791844f16567a7cfdcfef1134c4ed8f843d0bd724fac198340b9c`), shared by both reviewers, blind to the other review. The author (Claude, iosfix2) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes.
- **Disposition:** fixed in 3e5b6f9eee, each with a test that fails before the fix

## Brief

Code review, round 1 of 3 (host change: the Apple web arm and WebModule). Branch tip d9f87da63c (HEAD of this worktree); base 125ee17e91 (origin/main). Review `git diff 125ee17e91..HEAD`: 2 commits, the red tests first.

Approved plan (theme2, 2026-10-30 13:54Z): ~/x2theme/iosfix2-screentime-plan.md, if readable.
The red: IframeCaptureTests.testALocalPDFIsAPDFView failed in 2 of about 27 full parallel Mac runs with WebKit's own NSInternalInconsistencyException ("Cannot remove an observer <WKScreenTimeConfigurationObserver> for the key path configuration.enforcesChildRestrictions from <STScreenTimeConfigurationObserver> ..."). It can't be caught from Swift, so in an app it would crash.
Finding: WebModule (and the test) ask the arm for its platform view before src reaches it. So a local-PDF frame built a WKWebView (platformView = pdf?.view ?? webView), showed it, and showPDF tore it down at once. NOT PROVEN to be the cause:
- a scratch probe of 1,200 plain WKWebView build-and-drops didn't reproduce it;
- theme2's condition (1): the REAL arm (configure, data store, content controller; mount, then set_src of a local PDF) looped 200 times beside a cargo build, twice on main (load 24-31) and twice on this branch (load 14-16), gave 0 occurrences either way.

The change:
- WebViews.configuration adds "pdf": true when the frame's src is a local file (app:/, or a bundled path with no scheme) named .pdf, with the fragment and query aside: WebViews.localPDF. It's a hint; the file decides.
- WebArm.platformView, when nothing is built, the hint is set and there's no partition or sandbox, returns a fresh FramePDF view (waitingPDF) and builds no web view. showPDF then reuses it and has no web view to tear down. A file that isn't a PDF, a sandbox or partition arriving, a failed read, or a src change goes through serve's existing closePDF (which builds the web view and emits kind 7), and the presenter's replaceView swaps the view.
- Tests (IframeCaptureTests, Mac and iOS):
  - testALocalPDFFrameMakesNoWebView: a PDF view from the mount, and no WKWebView in the window at the mount or after the load;
  - testAPDFNamedFileThatIsNotOneShowsAWebView: a .pdf-named HTML document ends up a WKWebView with its page loaded, after a kind 7 with no pdf;
  - testWhichSourcesAreHintedAsLocalPDFs: the hint's table.
  On main's arm the first fails (a web view at the mount), and the second fails too (no kind 7; the web view was there from the start). The existing PDF, zoom, page, capture and link tests pass. Mac 12/12; iOS: the new 3 pass bare, and the PDF test needs a scene, so it runs hosted in the full suite. Full suites are running.

Check hard:
- anything that treats `pdf != nil` as "a PDF is showing": capture, command ("a PDF frame has no document to script"), checkOwned, configure's partition check, the agent's tree, the hatch's `changed` before the file comes;
- a hint on a frame whose sandbox or partition arrives later (set_sandbox comes after platformView in mount);
- the hint's src rules against WebModule.update's own local-file test;
- the iOS presenter's replaceView and the kind 7 path;
- the waitingPDF view's lifetime and its delegate (FramePDF(arm:) sets view.delegate);
- whether the tests prove what they claim.
Do NOT edit files. Give findings with file:line and severity. End with one line: VERDICT: APPROVE or VERDICT: CHANGES.

## Review

I'll review the branch tip against `125ee17e91` without editing anything, starting with the approved plan and the diff so the red tests and the WebModule/Apple-web change are checked against the same rules.The plan is a hint-only local-PDF path so the arm never builds a web view it immediately tears down. I'll read the two-commit diff and the call sites that treat a PDF view as already showing.The diff is three files. Next I’ll read the arm, WebModule, and the places that treat a non-nil PDF as already showing.A missing `app:/*.pdf` removes the frame's only view, and the swap that should put a web view back is dropped. That path is the one the plan said `closePDF` and kind 7 already cover.

[P1] A failed `app:/` read of a hinted PDF detaches the frame and leaves it empty — `host/apple/webarm/WebArm.swift:252`

`platformView` now installs a `FramePDF` before the file is read whenever the hint is set. For `app:/…pdf` that is missing or unreadable, `WebModule.update` sets `fileFailed` and sends a nil `src` (`WebModule.swift:403`). `serve` then misses the PDF branch and `closePDF` (`FramePDF.swift:159`) removes that view from the hierarchy, builds a `WKWebView`, and emits kind 7. `receivedEvent` returns first on `fileFailed` (`WebModule.swift:562`), so the kind 7 handler never calls `replaceView`. `node.web` and `entry.platformView` keep pointing at the detached PDF view, and nothing inserts the web view until a later `update`. A frame that had already shown a PDF also keeps `pdfPages`, so the agent's tree still reports that PDF. `testAppFilePDFPixelsAndMissingFileError` only waits for the error string in agent state, and the new tests feed a document straight to the arm, so both stay green. Let kind 7 through that guard (still drop the empty document's load), or swap the view before the failure is marked.

The other checks hold. After a real PDF, `pdf != nil` matches a document on screen: capture draws that view, evaluate's "no document to script" is right, `checkOwned` sees the delegate `FramePDF.init` set, and the tree's `pdf` flag waits for kind 7's page count. A partition is in the first `configure`, so `startsAsPDF` does not build a waiting view; a sandbox arrives with the bytes and, when the read succeeds, `closePDF` plus kind 7 swaps in a `WKWebView`. `localPDF` and `update`'s local-file test agree on `app:/`, a schemeless path, `//`, and query or fragment. The iOS and Mac presenters share `replaceView`, and a delivered kind 7 copies the frame, replaces the subview, and tells the hatch. The new tests do prove the arm's claim: no `WKWebView` in the window at the mount or after a real PDF, and a non-PDF `.pdf` name ends on a web view after an empty kind 7.

VERDICT: CHANGES
