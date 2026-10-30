# Code review: Apple picture retries, Retry-After and screen-filling priority, round 3 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `fdc4e0e825`.
- **Method:** one brief (sha256 `12538eabf97a421968ec1d8b88e34c3fda498d40331a5386c77c35a315bd73e6`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. **[P2] Late responses lose `Retry-After`** — [RasterInput.swift:345](host/apple/Sources/ExactKit/RasterInput.swift:345). If the timeout fires before a queued 429/503 response callback, this guard discards its header. `finish()` sees only the timeout, records no cooldown, and the next retry can fetch after one second despite `Retry-After: 600`. Linux records cooldowns even when the timeout already won. Parse and remember the cooldown before this guard, while preserving the timeout as the download’s failure.

2. **[P2] Delayed layout can still miss the first-download watch** — [RasterLoader.swift:759](host/apple/Sources/ExactKit/RasterLoader.swift:759). If an offscreen picture is initially small, then grows to screen-filling size while downloading, `resized()` neither installs the watch nor wakes reconciliation: it is neither held nor retrying, and `offeredPixel` remains zero. Scrolling it onscreen therefore leaves its first download at the 30-second bound. Wake reconciliation or install the watch when layout makes a pending first download eligible. The added test sizes the picture before loading, so misses this sequence.

Round 2’s metadata-cap, parked-watch, failure-preservation, and monotonic-time fixes appear resolved. The first-download scroll fix remains incomplete as described above.

**Non-blocking**

None.

Static review only; no files edited, network used, tests or builds run.
