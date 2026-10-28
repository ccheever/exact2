# Code review: Apple picture retries, Retry-After and screen-filling priority, round 3 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `fdc4e0e825`.
- **Method:** one brief (sha256 `12538eabf97a421968ec1d8b88e34c3fda498d40331a5386c77c35a315bd73e6`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. `host/apple/Sources/ExactKit/RasterLoader.swift:757` — A first download whose box grows to screen-filling size while it is still off screen is never hurried when it later scrolls on. `resized` only calls `urge` when `fillsScreen` is true, and that requires the view to be on screen already. For a remote picture with no metadata yet, `offeredPixel` is still 0, so `resized` returns at line 762 without waking a pass. `watching` is armed only inside `reconcile` (lines 964–969), and a scroll wakes a pass only for views already in `parked` (line 1166). The in-flight task keeps the 30s bound. Arm the watch from `resized`: if the picture is not delivered, is remote, has no metadata yet, and `fillsScreen(view, anywhere: true)`, call `backend.wake()` so `reconcile` sets `watching` while it is off screen and `urge`s it once a scroll brings it on.

**Non-blocking**

1. `llp/1011-image-v1.spec.md:424` — The spec says a download already under way moves to the 120s bound on every native host. Linux `Downloads::promote` (`host/linux/src/image/remote.rs:297`) only does that while the job is still queued; a thread that has taken it keeps the original deadline. Apple extends the in-flight task, which matches this spec sentence and the scroll test. The “Linux’s rules” line overstates Linux.
2. `host/apple/Sources/ExactKit/RasterLoader.swift:1273` — Pause cancels the in-flight download but does not clear `RasterSource.urgent`. The task started on resume still reads that flag, so a picture that no longer fills the screen keeps the 120s bound and high priority. Clear `urgent` when pausing and let the next pass `urge` only if it still fills.
