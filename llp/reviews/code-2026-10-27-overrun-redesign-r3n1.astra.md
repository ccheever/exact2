# Code review: the Overrun re-ask redesign, narrow re-check 1, 2026-10-27 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `591ca16814`.
- **Method:** a narrow brief naming the earlier findings and their fixes; 3 rounds (runtime, client-reachable), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

**SHOULD — [runner/src/runner/advance.rs:326](runner/src/runner/advance.rs:326): hidden invocations can stall until expiry.** With timers held, hold=1000, an unrelated App-owned re-ask=1250, and the hidden invocation’s `then`=1300, the new gate blocks the `then`. Line 480 prevents the hold from expiring. The pending `then` keeps the invocation open, which keeps timers stopped. Timeout prevents an infinite wait, but valid calls time out and wake expiry discards the blocked `then`. The earlier hold needs to resolve while authored timers remain stopped.

The refresh fixes correctly preserve forced status across passes, including for resources that never overran. Only the final request is dispatched.

Source-traced; tests were not run because this read-only checkout has no build artifacts.
