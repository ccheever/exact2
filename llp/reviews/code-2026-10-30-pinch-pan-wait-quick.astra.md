# Code review: IframeCaptureTests pinch-pan reads the page until it has the zoom and the pan, quick review (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `9457140d27`.
- **Method:** one brief (sha256 `33b5a1024b205a0950dfc7b5f39b84fbd8477467f7cd856df121e79948109699`), shared by both reviewers, blind to the other review. The author (Claude, iosfix2) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** approve.
- **Disposition:** none needed (Astra's soft-deadline note: the loop still ends; kept as is)

## Brief

Quick review (test-only flake hardening, one round). Tip 9457140d27 is HEAD of this worktree; review `git show HEAD` (IframeCaptureTests.swift and one QUEUE.md line removed).
IframeCaptureTests.testAClipAfterAPinchAndAPanCapturesItsElement failed in two full parallel iOS runs (load up to 245); the failure message was not kept. After zooming the WKWebView's scroll view to 1.5 and offsetting it (iOS) or setMagnification + scrollBy (macOS), the test paused a fixed 0.8 s and read visualViewport once. Now it reads every 0.2 s for up to 10 s until the page reports scale 1.5 and a positive page offset, then runs the same assertions and the capture. Mac 1/1, iOS hosted 3/3 green.
Check: the loop is correct on both platforms (it cannot pass vacuously, it ends, the evaluateJavaScript completion and spin are used correctly, the captured read is the last one); the change does not hide a real failure; whether the capture step that follows also needs a wait (the snapshot under load) and whether removing the QUEUE line is fair. Do NOT edit files. Give findings with file:line and severity. End with one line: VERDICT: APPROVE or VERDICT: CHANGES.

## Review

One nonblocking finding:

- **P3 — [IframeCaptureTests.swift:248](/tmp/iosfix2-rv-pp-astra/host/apple/tests/ExactKitTests/IframeCaptureTests.swift:248):** The 10-second deadline is soft: `spin` independently allows 15 seconds, so a late evaluation can extend polling to roughly 25 seconds. It still terminates. Pass the remaining budget to `spin` if 10 seconds must be a strict cap.

Otherwise, the loop is correct on both platforms. Each iteration has its own `read`; `measured` receives the final iteration’s result, and the capture uses that rectangle. Pumping the main run loop permits the [main-thread JavaScript completion](https://developer.apple.com/documentation/webkit/wkwebview/evaluatejavascript%28_%3Acompletionhandler%3A%29) to execute. Missing results, wrong scale, and absent pan still fail the unchanged assertions; there is no vacuous success.

No additional capture wait is warranted: [IframeCaptureTests.swift:161](/tmp/iosfix2-rv-pp-astra/host/apple/tests/ExactKitTests/IframeCaptureTests.swift:161) already waits for the capture record, emitted after writing finishes. [FrameCapture.swift:81](/tmp/iosfix2-rv-pp-astra/host/apple/webarm/FrameCapture.swift:81) requests `afterScreenUpdates`; snapshot failures get bounded retries, while incorrect pixels still fail. This change waits for setup without retrying a failed capture assertion.

Removing the QUEUE entry is reasonable as completed hardening, but the lost failure messages mean the original cause remains unproven. The commit records that limitation.

Static review only; no files edited or tests rerun.

VERDICT: APPROVE
