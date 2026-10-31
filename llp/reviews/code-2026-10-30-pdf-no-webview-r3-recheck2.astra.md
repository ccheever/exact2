# Code review: a local-PDF iframe makes no web view it then drops (the ScreenTime KVO red, not proven the cause), round 3, second narrow re-check (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `cb7539daa3`.
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

- **P2 — The waiting PDF can be left permanently empty.** [WebArm.swift:596](/tmp/iosfix2-rv-pdf5-astra/host/apple/webarm/WebArm.swift:596) suppresses serving without guaranteeing another serve. Mount partitioned on page A, remove the partition while changing to PDF B, then restore A before B’s read completes. The host discards B’s stale completion; `setSrc(A)` returns because the arm still holds A. Nothing serves. Similarly, a failed `app:/` read leaves the arm’s source `nil`: removing the partition without changing that PDF URL skips both serving and another source delivery. Failure or cancellation needs a serve path even when the arm’s source compares equal.

- **P2 — An already queued serve still causes build-and-drop.** [WebArm.swift:594](/tmp/iosfix2-rv-pdf5-astra/host/apple/webarm/WebArm.swift:594) creates the waiting view but leaves `servePending` intact. If a preceding page-source update queued a serve, and partition removal plus the PDF-source update happens before it executes, that queued callback serves the old page during the PDF read. `closePDF()` builds another web view; the eventual PDF serve drops it. Skipping a new scheduling call does not defer existing work. The new test waits for the original load, so it misses this ordering.

The host and arm name rules match. No separate kind-7 filtering regression found; the first finding prevents its emission. Static review only; suites not rerun.

VERDICT: CHANGES
