# Code review: Harmony's web tests against a Snapback server of their own (scripts/memory-backend-test.mjs, the async lane), narrow re-check, 2026-10-21 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `a2d00c73fa`.
- **Method:** one brief (sha256 `ae395cac0e95f5afed143f37c7574677ee57d4ed40244154ff8660b36e7e8649`), blind to the other reviewer; 2 rounds (the lead's 20:50Z), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

Two gaps remain before I’d call these fixes complete:

1. **Cleanup can leave the actual builder running.** [`stop()`](scripts/memory-backend-test.mjs:58) signals only the direct child, but [`host/web/build.mjs`](host/web/build.mjs:71) spawns another builder. Cancellation can kill the tracked wrapper and remove `dir` while its descendant continues writing or recreates the staging directory. Cleanup needs to terminate and wait for the owned build tree.

2. **Failed server spawn can still produce an unhandled rejection.** [`createInterface()`](scripts/memory-backend-test.mjs:109) runs before a rejection handler is attached to `serverDone`. Bun can expose null stdout on failed spawn—the [existing driver handles this explicitly](scripts/agent.mjs:131). Readline then throws, leaving the subsequent `serverDone` rejection unobserved. Await successful spawning or handle rejection before accessing stdout.

The checked-dist fix is correct: [`webDist()`](scripts/app.mjs:442) honors `EXACT_WEB_DIST`, and both resets and serving use it. This invocation does **not rebuild the web app**; the [default Chrome path rejects stale builds](scripts/agent.mjs:82). It still invokes Cargo for the test compiler. The cancellation guards, shared cleanup promise, and supervisor PID are otherwise correctly wired.

Static review only; no edits, tests, servers, installs, or network commands.
