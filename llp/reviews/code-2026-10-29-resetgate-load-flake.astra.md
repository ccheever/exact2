# Code review: ResetGateTests admission check load flake, round 1 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `a7d20649ab`.
- **Method:** one brief (sha256 `c305148c36b4c13f3fb710a89efce7641a643d0a7a4501a355eaa55920ddadb8`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

none.

**Non-blocking**

1. [ResetGateTests.swift:372](host/apple/tests/ExactKitTests/ResetGateTests.swift:372) — The restart proves progress through `check`, but does not isolate update admission. `now() + 60_100` passes the timer’s nominal deadline **and its 100 ms fallback cap**. [`release_waits`](runner/src/runner/waits.rs:133) first calls `advance_timed`, which can fire that fallback regardless of the update flag. This limitation already existed with the old `100`/`+200` values. To prove update-specific admission, use the timer’s nominal deadline as the cutoff, below its cap.

2. [reset-gate-admission.contract:2](host/apple/tests/fixtures/reset-gate-admission.contract:2) — “However loaded” overstates the guarantee. A stall long enough to cross the minute deadline before holding can reproduce the original race. The change provides substantial timing margin; soften the comment accordingly, or control the live-clock source for deterministic isolation.

No Foundation timer callback is required: `wait=update` tasks use runner deadlines, and `check(at:)` evaluates those against the supplied cutoff. The held-call assertion verifies that both synchronous and asynchronous checks actually reach the hold; the zero-count and pending-clear assertions remain meaningful.

Static review only; no tests or builds run.
