# Code review: Apple picture retries, Retry-After and screen-filling priority, round 2 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `e2f690c71e`.
- **Method:** one brief (sha256 `7f36c6f138860c0f274364c4d705549ed2496766084acb98b3929f2ba10c39b6`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

Round 1’s fixes address the reported cases by inspection. Five remaining issues:

1. **Retries bypass metadata admission.** [RasterLoader.swift:200](host/apple/Sources/ExactKit/RasterLoader.swift:200) clears failure without checking `metadataLimit`. Failed sources free admission slots, then their timers can reactivate all of them simultaneously. With `metadataLimit = 1`, two initially failed URLs can both start slow retries; the production limit of 64 can likewise be exceeded. Linux retries pass through download admission. Require capacity before resetting a source, retaining the held interest without consuming an attempt when admission is full.

2. **Pausing can orphan a parked image.** [RasterLoader.swift:1258](host/apple/Sources/ExactKit/RasterLoader.swift:1258) removes the watch whenever `retrying` is true, but preserves `item.parked`. A successful retry can reach metadata and become budget-parked off screen while still marked `retrying`. After pause/resume, reconciliation skips it, and scrolling has no observer to wake it. Preserve watches for genuinely parked interests, or rebuild them on resume.

3. **Initial downloads miss promotion when scrolled on screen.** [RasterLoader.swift:1142](host/apple/Sources/ExactKit/RasterLoader.swift:1142) observes only parked, held, or retrying interests. A large image downloading off screen on its first attempt belongs to none of these. Scrolling it into view without changing its frame need not call `resized` or trigger reconciliation, so it retains normal priority and the 30-second bound. Observe visibility changes for pending initial downloads and reevaluate `fillsScreen`; cover this with an actual scroll rather than a manual reconciliation.

4. **Late response headers can still overwrite a timeout.** [RasterInput.swift:348](host/apple/Sources/ExactKit/RasterInput.swift:348) unconditionally replaces `failure`; the content-length refusal does likewise. If the bound has cancelled the task but a response callback still arrives, a late 404 or oversized response replaces the transient timeout with a permanent failure, suppressing retries. Round 1 protects late body bytes, but headers need the same protection: when failure already exists, complete with `.cancel` without replacing it.

5. **`Retry-After` cooldowns use wall time.** [RasterInput.swift:428](host/apple/Sources/ExactKit/RasterInput.swift:428) compares a `Date` deadline with `Date()`, and `hold` calculates its delay the same way. A forward clock correction can expire a delta-seconds cooldown early: a newly mounted view then downloads before the server’s requested duration elapsed. Linux converts the duration to `Instant`. Convert the parsed delay to a monotonic deadline at receipt and use it throughout cooldown and retry scheduling.

**Non-blocking**

1. [RasterLoader.swift:971](host/apple/Sources/ExactKit/RasterLoader.swift:971): Terminal decode/refusal paths leave `retrying` true and retain the scroll watch. After a download retry succeeds but decoding permanently fails, scrolling keeps waking reconciliation for a settled image. Retire retry tracking on every terminal outcome.

2. [RasterLoader.swift:921](host/apple/Sources/ExactKit/RasterLoader.swift:921): `testAnnounced` grows without a bound in every DEBUG application and survives loader resets. Make recording opt-in for tests, or bound and clear the history.

Static review only; no files edited, network requests, tests, or builds run.
