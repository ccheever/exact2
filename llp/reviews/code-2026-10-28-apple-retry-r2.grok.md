# Code review: Apple picture retries, Retry-After and screen-filling priority, round 2 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `e2f690c71e`.
- **Method:** one brief (sha256 `7f36c6f138860c0f274364c4d705549ed2496766084acb98b3929f2ba10c39b6`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. `host/apple/Sources/ExactKit/RasterInput.swift:348` — `didReceive response` always assigns `failure`. The data callback now keeps an existing failure (`:382`), and `didCompleteWithError` only fills a nil one (`:390`), but this callback does not. The whole-load timer sets `URLError.timedOut` and cancels on the session queue; URLSession can still deliver the response afterward. A late 404, a non-HTTP response (`HTTP 0`), or a `Content-Length` over the cap replaces the timeout with a permanent failure, `hold` sees no retry time, and the `error` goes out with retries left. A late 2xx takes the other branch and calls `completionHandler(.allow)`, so the body continues after the bound. Linux keeps the first outcome (`Load::settle`). At the top of the response callback, if `failure` is already set, call `completionHandler(.cancel)` and return. Leave the status and encoded-limit assignments for a response that arrives first.

**Non-blocking**

1. `host/apple/Sources/ExactKit/RasterLoader.swift:706` — `sampleBlank` still treats any `failure` as finished. A held picture keeps its failure string for the whole wait (about a minute), so the blank clock stops between attempts. `loadingOnScreen` already counts `held`. Use the same condition (`failure == nil || held`).

2. `host/apple/Sources/ExactKit/RasterInput.swift:348` and `host/apple/Sources/ExactKit/RasterLoader.swift:862` — a 429/503 wait is stored as a wall-clock `Date`, and cooling is compared to `Date()`, while the sleep is a monotonic `asyncAfter`. A backward clock step lets the timer fire while the URL is still cooling, so `fetch` fails immediately, `retries` increments, and a large step makes `wait > most` and sends `error` without another request. Linux turns the header into a duration and stores an `Instant`. Store a `DispatchTime` taken when the header is read, and test cooling against that.
