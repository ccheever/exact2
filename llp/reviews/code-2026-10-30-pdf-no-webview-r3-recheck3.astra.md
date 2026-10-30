# Code review: a local-PDF iframe makes no web view it then drops (the ScreenTime KVO red, not proven the cause), round 3, final narrow re-check (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `5ca67a4397`.
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

No blocking findings.

- **Info — [QUEUE.md:8](/tmp/iosfix2-rv-pdf6-astra/QUEUE.md:8):** Revert complete. The requested log and diff confirm HEAD equals `9efb1ada9f` except for the single ScreenTime QUEUE deletion. Working tree is clean.
- **Low, accepted residual — [WebArm.swift:589](/tmp/iosfix2-rv-pdf6-astra/host/apple/webarm/WebArm.swift:589), [WebModule.swift:360](/tmp/iosfix2-rv-pdf6-astra/host/apple/Sources/ExactKit/WebModule.swift:360):** Fairly stated. Clearing `partition` while changing a page source to a local PDF can serve the old source before the asynchronous read completes, building a web view the PDF subsequently drops.
- **Info — [WebArm.swift:252](/tmp/iosfix2-rv-pdf6-astra/host/apple/webarm/WebArm.swift:252), [FramePDF.swift:159](/tmp/iosfix2-rv-pdf6-astra/host/apple/webarm/FramePDF.swift:159):** No additional stranded-view or local-PDF mount build-and-drop path found. Mount creates the PDF view directly; failure falls back to a web view before notifying the presenter. Unconditional scheduling restores progress for failed reads, superseded sources, and already-queued serves.
- **Info — `QUEUE.md:9` (deleted baseline line):** Removal matches the approved disposition: the red remains iosfix2’s, with recurrence filed with Apple. It does not establish ScreenTime causality or resolution.

Source-only re-check; suites not rerun. No files edited.

VERDICT: APPROVE
