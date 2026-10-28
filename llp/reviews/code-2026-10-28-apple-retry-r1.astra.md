# Code review: Apple picture retries, Retry-After and screen-filling priority, round 1 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `d67cc6e4c9`.
- **Method:** one brief (sha256 `ba6c9daa4a40a3dc708f64e2c26131782c5a442e6ffb0660c7dd2ad5871f91e3`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. [RasterLoader.swift:740](host/apple/Sources/ExactKit/RasterLoader.swift:740) — **Held images can stall after layout brings them on screen.** An off-screen transient failure has `held == true`, `parked == false`, no retry timer, and `offeredPixel == 0`. `resized` therefore returns without waking reconciliation. Without another scroll or backend completion, it never retries or emits its eventual error. Wake held interests when layout changes visibility; test re-entry without explicitly calling `reconcile`.

2. [RasterLoader.swift:859](host/apple/Sources/ExactKit/RasterLoader.swift:859) — **Leaving during a running retry does not reset the showing’s count.** Starting a retry clears `held` and removes the scroll watch; only `hold` resets `retries`. Leaving and returning before that download finishes therefore preserves the previous showing’s count, even with intervening reconciliations. `setPaused` likewise preserves it across window detachment. A new showing can receive an error before its six retries. Track visibility throughout an unsettled assignment and reset the count on departure, including pause.

3. [RasterInput.swift:238](host/apple/Sources/ExactKit/RasterInput.swift:238) — **Startup and promotion race.** Promotion between the `urged` lookup and task registration sees no matching download; startup then creates an ordinary task with the 30-second bound. The source is already marked urgent, so subsequent calls cannot repair it. Registration also publishes the download before `self.task` and `asked` are assigned, allowing queue-side promotion to race those writes. Serialize initialization, registration and promotion, with the urgency check inside that operation.

4. [RasterLoader.swift:828](host/apple/Sources/ExactKit/RasterLoader.swift:828) — **Separate backend reads can emit an error prematurely.** After another view resets the shared source, `transient` can return nil while it downloads. If a worker publishes another transient failure before the following `metadata` read, that read sees a failure and `hold` returns false, ending this view’s retries. Read failure and retry deadline together under one lock; also update the held failure message from that snapshot.

5. [RasterInput.swift:254](host/apple/Sources/ExactKit/RasterInput.swift:254) — **An obsolete timer can cancel a promoted download.** Canceling the dispatch source cannot withdraw a cancellation operation its handler already submitted. That operation checks only `done`, so it can execute after promotion rearms the timer for 120 seconds and cancel at the old deadline. Validate a timer generation or the current deadline on the sessions’ queue before canceling.

6. [RasterInput.swift:257](host/apple/Sources/ExactKit/RasterInput.swift:257) — **Whole-load timeouts can become permanent encoded-limit failures.** The timer sets `.timedOut` and cancels the task, but URLSession may still deliver data before acknowledging cancellation. The guard at line 371 treats an existing failure as a size violation and overwrites it with `.encodedLimit`, suppressing retries and sending an immediate error. Preserve an existing failure; check the byte cap separately.

7. [RasterInput.swift:412](host/apple/Sources/ExactKit/RasterInput.swift:412) — **`urgedURLs` can grow without bound.** `source.inspecting` includes metadata inspection after a download has finished. Promotion then inserts a URL with no remaining task to remove it. Cooling and cached-response exits also bypass `finish`. Repeated URLs retain process-lifetime markers and incorrectly promote later requests. Give pending promotion an attempt-scoped owner and clean up every exit, or eliminate the global markers through atomic startup.

8. [RasterRetry.swift:37](host/apple/Sources/ExactKit/RasterRetry.swift:37) — **Spool creation failures are not consistently transient.** `RasterDownload.init` throws `RasterFailure.decode` when `createFile` fails, and this classifier rejects every `RasterFailure`. Temporary disk exhaustion therefore sends an immediate error instead of retrying, unlike Linux’s spool handling. Preserve the filesystem error or introduce a distinct transient spool error.

9. [RasterLoader.swift:203](host/apple/Sources/ExactKit/RasterLoader.swift:203) — **Urgency persists across subsequent attempts.** Resetting the source leaves `source.urgent` true forever. After a full-screen view shrinks or disappears, retries serving only a thumbnail still receive high priority and 120 seconds. Linux recomputes urgency when acquiring the next download. Recompute it from current view interests for each new attempt, while retaining promotion for the running attempt.

**Non-blocking**

1. [RasterLoader.swift:706](host/apple/Sources/ExactKit/RasterLoader.swift:706) — Held images have a failure string, so `loadingOnScreen` excludes them and screenshots can report zero pending images during retry waits. The aggregate `parked` diagnostic also counts held views whose individual `parked` flag is false. Count unsettled held images as loading and distinguish watched views from genuinely parked views.

2. [RasterRetryTests.swift:148](host/apple/tests/ExactKitTests/RasterRetryTests.swift:148) — The fixture has no session or event handlers, so these tests cannot establish error timing, exactly-once delivery, or absence of `load` after `error`. Add event-level coverage, shared-source retries, and actual scroll/layout transitions. The 404 case is an unchanged-behavior regression test; it need not fail before this change.

3. [contract-for-agents.md:1721](docs/contract-for-agents.md:1721) — “Up to about a minute later” understates the delay: 61 seconds is only the backoff sum, excluding downloads and accepted `Retry-After` waits. Describe it as backoff time, not an overall bound.

Static review only; no files edited, network used, tests or builds run. Red-before behavior was not executed.
