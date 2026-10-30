# Code review: a local-PDF iframe makes no web view it then drops (the ScreenTime KVO red, not proven the cause), round 1 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `d9f87da63c`.
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

- **P2 — PDF links to other apps silently stop working.** [WebArm.swift:252](/tmp/iosfix2-rv-pdf1-astra/host/apple/webarm/WebArm.swift:252) bypasses `build()`, the only place that initializes `ui`. After loading a hinted PDF, `ui` remains nil, so [FramePDF.swift:114](/tmp/iosfix2-rv-pdf1-astra/host/apple/webarm/FramePDF.swift:114) silently drops `mailto:` and other external links. Initialize the confirmation handler independently of WKWebView construction. The existing link test uses no PDF hint and checks only HTTPS links.

- **P2 — Failed reads lose the mounted view instead of completing the fallback.** The new hint at [WebModule.swift:309](/tmp/iosfix2-rv-pdf1-astra/host/apple/Sources/ExactKit/WebModule.swift:309) creates a PDF placeholder even for a missing `app:/…pdf`. The failure handler sets `fileFailed = true`; subsequent `closePDF()` removes that placeholder and builds a web view, but [WebModule.swift:562](/tmp/iosfix2-rv-pdf1-astra/host/apple/Sources/ExactKit/WebModule.swift:562) discards its kind-7 event. The replacement remains detached, while `owner.web` references the removed PDF view. Allow structural replacement events through the failure guard. The new fallback test cannot catch this because it supplies bytes directly and manually mounts the replacement through `current()`.

The original crashing test also still mounts without `"pdf": true` at [IframeCaptureTests.swift:341](/tmp/iosfix2-rv-pdf1-astra/host/apple/tests/ExactKitTests/IframeCaptureTests.swift:341), retaining the same WKWebView construction and immediate teardown.

Native tests were not rerun in this read-only worktree; `git diff --check` passed.

VERDICT: CHANGES
