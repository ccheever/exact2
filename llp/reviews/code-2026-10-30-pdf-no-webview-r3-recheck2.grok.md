# Code review: a local-PDF iframe makes no web view it then drops (the ScreenTime KVO red, not proven the cause), round 3, second narrow re-check (grok)

- **Family:** xAI. Grok via the grok CLI, reasoning effort xhigh, edits denied, a throwaway worktree at `cb7539daa3`.
- **Method:** one brief (sha256 `eca923a0265f645bf3292c6d4be2ad437b8eeaae0490cb4d63ced86f634cff20`), shared by both reviewers, blind to the other review. The author (Claude, iosfix2) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes.
- **Disposition:** cb7539daa3 reverted in 7a1bae8732; the same-update partition clear and page-to-PDF src change is a named residual

## Brief

Second narrow re-check (host change: a local-PDF iframe shows the PDF's view from its mount and makes no web view it then drops; the ScreenTime KVO red, not proven to be the cause). HEAD cb7539daa3; the last re-check reviewed 9efb1ada9f. Read `git diff 9efb1ada9f..HEAD` (one commit), with context from `git diff 125ee17e91..HEAD`.

The re-check's finding (Astra P2 and Grok P2, the same one): clearing `partition` in the same update that changes `src` from a page to a local PDF still built a web view. configure took waitingPDF and scheduled serve, but the host sends the new src only after its async read, so that serve saw the old src, closePDF built a web view, and showPDF dropped it.
Fix: in configure's replacement, when startsAsPDF holds, serve is scheduled only if the src the arm has already names a local PDF (WebArm.namesLocalPDF, the same rule as the host's WebViews.localPDF). Otherwise the PDF's own src is on its way and its set_src serves. A partition removal with the PDF src already in place serves at once, as before.
Test: testALocalPDFLosingItsPartitionWithANewSourceMakesNoSecondWebView mounts partitioned on a page (one construction). It configures {"pdf": true}, waits a turn, sends the PDF's bytes and src, and ends on the PDF view with still one construction. It fails on 9efb1ada9f (two constructions). The earlier same-src test still passes.
Full suites on 9efb1ada9f: Mac 306/0; iOS failed NavigationBarIOSTests.testAForwardedCustomTransitionPopsOnceAndACancelledOneNever, which is navigation and unrelated, to recheck. The suites on this tip are running. Caps is green.

Check only:
(a) Is this fix correct and complete?
(b) Can a frame now be left on the empty waiting PDF view with no serve coming? Think of: a hint whose src never arrives, the read failing, the src not changing, and the name rule disagreeing between the host and the arm.
(c) Any new build-and-drop or lost kind 7?
Do NOT edit files. Give findings with file:line and severity. End with one line: VERDICT: APPROVE or VERDICT: CHANGES.

## Review

I'll re-check the one commit since `9efb1ada9f` against the earlier range, and only the three questions: whether the serve gate is correct, whether a frame can sit on the empty waiting PDF view, and whether a web view is still built and dropped or a kind 7 is lost.The commit is one gate on `scheduleServe`. I'll read the host's local-PDF rule and the arm's configure, set-src, and serve paths around it.The reported page-to-PDF sequence is fixed. A later partition clear after a failed `app:/` read still leaves the frame on the empty waiting PDF view, so the fix is not complete.

## P2 — A failed `app:/` read, then clearing `partition`, never serves

`WebArm.swift:594-596`. After `startsAsPDF`, `configure` shows `waitingPDF()` and skips `scheduleServe` unless `namesLocalPDF(src)`. The comment at `WebArm.swift:593` assumes the PDF `src` is sent next. That is false once an `app:/` read has failed.

The host keeps the path on `entry.src` (`WebModule.swift:356`) and tells the arm `src` is nil (`WebModule.swift:410`). `namesLocalPDF(nil)` is false (`WebArm.swift:255`). Two updates then install the empty PDF and schedule nothing:

- Clearing `partition` only. `src` and `sandbox` are unchanged, so `update` returns at `WebModule.swift:363` and never calls `setSrc` or `setSandbox`. `replaceView` (`WebModule.swift:361`) has already swapped in the empty PDF view from `platformView`.
- Clearing `partition` and pointing at another missing `app:/` file. The new read fails and calls `setSrc(nil)` again. `setSrc` returns at `WebArm.swift:646` because `src` is already nil, so it does not serve.

`closePDF` never runs, so kind 7 `{}` is never emitted (`FramePDF.swift:159-164`). `receivedEvent` would still deliver that kind 7 while `fileFailed` is set (`WebModule.swift:564`). D10 says a failed read gets a web view (`llp/1020.002-a-browser-shell.rfc.md:674`). On `9efb1ada9f` this `configure` always called `scheduleServe`, and `serve` with a nil `src` took `closePDF`.

A failed read in the same update as the partition clear still recovers when the arm's `src` is the old page: `setSrc(nil)` changes it and serves. The new test covers the successful page-to-PDF case only.

**(a)** For that successful case the gate is right. `namesLocalPDF` (`WebArm.swift:254-257`) is the same function as `WebViews.localPDF` (`WebModule.swift:496-499`). `browserSource` rewrites only `//` URLs, which both functions reject, so on a string the host actually sent they agree. A partition clear whose `src` is already the local PDF still serves, which is what `testALocalPDFLosingItsPartitionMakesNoSecondWebView` checks.

**(b)** A hint whose read is still in flight does get its `setSrc`: the callback sends the path, or nil, unless a newer `src` superseded it or the frame is gone. The frame that sticks is the one whose read already failed. The name rules do not disagree. The host's stored path and the arm's nil `src` do.

**(c)** No new build-and-drop on the path this gate skips. The following `setSrc` of a real PDF reaches `showPDF` with no web view built. Kind 7 for that PDF still comes from `showPDF`. The lost kind 7 is the failure case above: `{}` never fires, and the empty PDF stays. If the arm's current `src` already names `.pdf` while a different local `src` is in flight and PDFKit rejects the bytes it has, `configure` still serves immediately and `closePDF` can build a web view that the later `showPDF` drops. That serve is the one this fix keeps, and `9efb1ada9f` did it for every replacement.

VERDICT: CHANGES
