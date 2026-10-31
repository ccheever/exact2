# Code review: a local-PDF iframe makes no web view it then drops (the ScreenTime KVO red, not proven the cause), round 3, final narrow re-check (grok)

- **Family:** xAI. Grok via the grok CLI, reasoning effort xhigh, edits denied, a throwaway worktree at `5ca67a4397`.
- **Method:** one brief (sha256 `35abd1bde235b43a189e9e9af5f7690c5811f205085b2e972adbc2d985167f9d`), shared by both reviewers, blind to the other review. The author (Claude, iosfix2) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** clean.
- **Disposition:** none needed. The full Mac and iOS suites ran on the final tip.

## Brief

Final narrow re-check (host change: a local-PDF iframe shows the PDF's view from its mount and makes no web view it then drops; the ScreenTime KVO red, not proven to be the cause). HEAD 5ca67a4397. Read `git log --oneline 9efb1ada9f..HEAD` and `git diff 9efb1ada9f..HEAD`: it should be only QUEUE.md, because cb7539daa3 is reverted.

The last re-check found that cb7539daa3 (serve only when the arm's src already names a local PDF) could leave a frame on an empty waiting PDF view with no serve coming: after a failed read (src nil), a superseded src, or a serve already queued. It is reverted, so configure always schedules serve after a partition change, as at 9efb1ada9f, which both of you reviewed in the narrow re-check.
Named residual, for theme2: clearing `partition` in the SAME update that switches src from a page to a local PDF still builds one web view that the PDF then drops. The host configures before its async read, so the first serve sees the old src. It's rare (a runtime partition clear plus a src change at once), and the mount path, where the red was seen, has no build-and-drop.
The QUEUE line for the ScreenTime exception is removed, as the approved plan says; the red stays iosfix2's, and a recurrence is filed with Apple.
Full suites on 9efb1ada9f's tree: Mac 306/0; iOS had one unrelated navigation flake. The suites on this tip are running. Caps is green.

Check only:
(a) Is the revert complete, so the tree equals 9efb1ada9f apart from QUEUE.md?
(b) Is the residual fairly stated, and is there any other path at 9efb1ada9f that leaves a frame without a view, or builds and drops at mount?
(c) Is the QUEUE removal right?
Do NOT edit files. Give findings with file:line and severity. End with one line: VERDICT: APPROVE or VERDICT: CHANGES.

## Review

I'll re-check only the revert, the named residual, and the QUEUE line against `9efb1ada9f` and this tip. No files will be edited.The range is three commits and the net diff is only `QUEUE.md`. Next I’ll compare the revert to `cb7539daa3` and read the serve path those commits touch.The revert is complete, the residual matches the code at `9efb1ada9f`, and the queue edit removes only the Screen Time line. No findings.

**(a)** `9efb1ada9f..HEAD` is `cb7539daa3`, its revert `7a1bae8732`, and `5ca67a4397`. `git diff-tree -r` shows one change, `QUEUE.md`. `host/apple/webarm/WebArm.swift` and `host/apple/tests/ExactKitTests/IframeCaptureTests.swift` have the same blobs at `9efb1ada9f`, `7a1bae8732`, and `HEAD`. The working tree is clean. `cb7539daa3` had added `namesLocalPDF` and the configure gate (`if !startsAsPDF || pdf == nil || Self.namesLocalPDF(src)`); both are gone. Configure on a partition change always reaches `scheduleServe()` again.

**(b)** The residual is accurate. On a partition change, configure tears the old web view down, and if the new configuration says `pdf` with no partition and no sandbox it shows a waiting PDF view and still schedules a serve:

```585:589:host/apple/webarm/WebArm.swift
        if replacing {
            // A local PDF's frame losing its partition shows the PDF's view, and makes no web view for `serve` to drop.
            if startsAsPDF { _ = waitingPDF() } else { _ = build(); closePDF() }
            markLoading()
            scheduleServe()
```

The host calls that configure before the file read. The new `src` is sent only after the read, on a later main-queue turn (`WebModule.swift:360`, then `398–419`). The serve queued at `WebArm.swift:766` therefore still sees the previous page `src`. `serve` misses the PDF check at `788`, and `closePDF` at `159–165` builds a web view. When the PDF `src` arrives, `showPDF` at `144` tears that web view down. That needs a partition clear and a src change to a local PDF in one update.

The mount path does not do this. The create op already carries `src` (`paragraph.rs:366–367`, `PresenterMac.swift:1098–1099`, `PresenterIOS.swift:874–875`). With no partition and no sandbox, `WebModule.swift:310` sets `pdf`, the first configure is not a replacement so it does not serve, and `platformView` at `WebArm.swift:252–253` returns the waiting PDF view. The read’s `setSrc` then serves that same view. `testALocalPDFFrameMakesNoWebView` (`IframeCaptureTests.swift:300–316`) is that path. A partitioned or sandboxed local PDF stays a web view from the mount (`319–331`), which is what `1020.002` D10 specifies. A failed read or a non-PDF file replaces the waiting PDF view with a web view and keeps it (`closePDF`, D10’s “gets a web view in its place”). Every replacing configure and every `closePDF` / `showPDF` installs a PDF view or a web view, and kind 7 swaps it in (`WebModule.swift:603–607`). No path leaves the frame with neither.

**(c)** The queue diff is the one Screen Time bullet, the `IframeCaptureTests.testALocalPDFIsAPDFView` / `WKScreenTimeConfigurationObserver` line that sat between the `IframeWindowsTests` bullet and the `Proxy` bullet. Those neighbors are still `QUEUE.md:8` and `QUEUE.md:9`. Nothing else in `QUEUE.md` mentions that exception. Removing it matches the plan: the mount churn that line was about is gone, the red stays iosfix2’s, and a recurrence goes to Apple.

VERDICT: APPROVE
