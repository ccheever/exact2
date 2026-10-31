# Code review: IframeCaptureTests pinch-pan reads the page until it has the zoom and the pan, quick review (grok)

- **Family:** xAI. Grok via the grok CLI, reasoning effort xhigh, edits denied, a throwaway worktree at `9457140d27`.
- **Method:** one brief (sha256 `33b5a1024b205a0950dfc7b5f39b84fbd8477467f7cd856df121e79948109699`), shared by both reviewers, blind to the other review. The author (Claude, iosfix2) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** approve.
- **Disposition:** none needed (Astra's soft-deadline note: the loop still ends; kept as is)

## Brief

Quick review (test-only flake hardening, one round). Tip 9457140d27 is HEAD of this worktree; review `git show HEAD` (IframeCaptureTests.swift and one QUEUE.md line removed).
IframeCaptureTests.testAClipAfterAPinchAndAPanCapturesItsElement failed in two full parallel iOS runs (load up to 245); the failure message was not kept. After zooming the WKWebView's scroll view to 1.5 and offsetting it (iOS) or setMagnification + scrollBy (macOS), the test paused a fixed 0.8 s and read visualViewport once. Now it reads every 0.2 s for up to 10 s until the page reports scale 1.5 and a positive page offset, then runs the same assertions and the capture. Mac 1/1, iOS hosted 3/3 green.
Check: the loop is correct on both platforms (it cannot pass vacuously, it ends, the evaluateJavaScript completion and spin are used correctly, the captured read is the last one); the change does not hide a real failure; whether the capture step that follows also needs a wait (the snapshot under load) and whether removing the QUEUE line is fair. Do NOT edit files. Give findings with file:line and severity. End with one line: VERDICT: APPROVE or VERDICT: CHANGES.

## Review

I'll review the HEAD commit only: the iframe capture test wait and the removed QUEUE line. No files will be edited.The commit is the pinch-pan wait plus one QUEUE line. I'll read the test around that loop, including how `pause`, `spin`, and the capture that follows actually work.The wait loop is sound on both platforms, and the assertions after it still fail the test when the zoom or the pan never shows up. The snapshot path already waits, and dropping that QUEUE line matches this repo's flake notes.

**Loop.** `IframeCaptureTests.swift:242-250` is a `repeat`/`while`, so it always takes a sample, and it stops when the 10s deadline passes or when one read has 11 numbers, `visualViewport.scale` within 0.01 of 1.5, and `pageLeft`/`pageTop` (indices 6 and 7) both positive. That is the same predicate as the asserts at `254-255`. A timeout, a script error, or a non-array result stores `[]` (`245-249`: the completion ignores the error and uses `?? []`, and `spin` treats that as arrived). `count == 11` then fails at `251`, and the `guard` returns. An empty or early read cannot take the success path.

It ends. `pause` (`82`) runs the main run loop for 0.2s and returns. `spin` (`77-79`) returns when `read != nil` or after its own 15s cap. The `while` then sees the deadline. One in-flight `spin` can run past 10s if the callback never comes; the next check still exits and the count assert fails.

The completion and `spin` match the rest of this file. The test is on the main thread, `WKWebView`'s callback is delivered there, and `RunLoop.main.run` in `spin` is what lets that callback set `read`. Each iteration has its own `read`. `measured = read ?? []` runs only after `spin` returns, and a success exits without another call, so the rect passed to `capture` (`257`) is that last read. macOS queues `scrollBy(60, 40)` (`233`) before the loop; the first `pause` turns the run loop, WebKit runs that script before the later read, and the predicate waits for its `pageLeft`/`pageTop`. iOS has already called `setZoomScale(1.5, animated: false)` and `setContentOffset(90, 60)` (`235-236`); the loop waits until the page reports both.

**It does not hide a real failure.** The scale, offset, capture-error, size, and red-pixel asserts are unchanged. If the page never reaches that viewport, the test fails at `251` or `254-255` with the last sample in the message. The 10s bound is only how long WebKit may take to publish a zoom the test already requested.

**Capture does not need another wait.** Once the predicate passes, `capture` (`156-162`) blocks until the `capture` record. `FrameCapture.snap` (`78-82`) sets `afterScreenUpdates` and retries a nil image twice, 120ms apart. On iOS the scroll view is zoomed before the page reports it, so the view is already ahead of the sample the loop waits for. The old 0.8s was time before the read, not a paint delay after a good read. A stale non-nil bitmap still fails the red samples at `262-264`.

**QUEUE.** The removed line recorded one unreproduced failure of the fixed 0.8s test and did not keep a failure message. This commit is the follow-up, and its message keeps both timestamps (03:28Z and ~19:00Z) and that the message was lost. The MIDI note above it stays because that test was not changed. A new failure of this loop is iosfix2's again.

No findings.

VERDICT: APPROVE
