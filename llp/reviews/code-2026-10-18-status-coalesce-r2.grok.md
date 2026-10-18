# Code review: a design-only build-status told once per 150 ms, round 2, 2026-10-18 (grok)

- **Family:** xAI. `grok-4.7` via Grok CLI, reasoning effort xhigh, edits denied, a throwaway worktree.
- **Scope:** the coalescing commit and round 1 (bce9dcd95).
- **Method:** one brief (sha256 `942d7bbaee5230ea3b5058e6a0720440cb767e0a1de00848c107b22d063ac139`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES (timed test assertions under load).
- **Disposition:** fixed in round 2's commit (45a018414: a test clock; see its message).

---

Nothing blocking. The five round-1 fixes hold: a non-status report flushes the waiting design before the event that follows, the timer only tells for the same registration object while it is still connected, mode key order no longer breaks the facts compare, and the doc matches that behavior (trailing from the first waiting status; `hello` and `builds` are not delayed).

Close does not cancel the timer. The callback no-ops once the stream is gone (`now === b && connected(now)`), and a reconnect `untell`s that id before installing a new registration. Apple, web, and Linux all register with the design they are running and send a status on connect; that status is told at once because the comparison was cleared. The last scrub frame is not left untold on a connected build.

1. **should-fix** — `scripts/tamber-builds.test.mjs:642` (also `:647` and `:651–653`). The timed assertions fail on a correct implementation when the event loop is slow. Three `Bun.sleep(40)` calls leave about 30 ms before the 150 ms timer; if that stretch takes 150 ms or more, the assertion compares `['too slow to check']` to `['d0']` and fails instead of skipping. The follow-up only waits until 190 ms after the first waiting status, so a late timer has not fired and the expected `['d0','d4']` fails, or an early fire has already told `d3` and started a new wait for `d4`. The sustained loop’s fixed cap of 4 tells over `12×30 ms + 200 ms` fails as soon as those sleeps run long (about 50–60 ms each is enough to emit a fifth tell). Fix: drive the wait from measured time, assert the count against elapsed/150 with slack that scales, and drop the fixed upper bound and the “too slow” branch that can never pass.
