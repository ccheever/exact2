# Code review: the Overrun re-ask redesign, narrow re-check 1, 2026-10-27 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `591ca16814`.
- **Method:** a narrow brief naming the earlier findings and their fixes; 3 rounds (runtime, client-reachable), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Note:** Grok's narration before its verdict is left out.

CLEAN.

Both round-3 findings are fixed, and neither fix adds a new problem.

**Refresh status.** `settle_pass` copies `refresh_next` into `refreshing` before any ask, and `refreshed` stays true for that resource on every later pass after `force.retain` drops it. The overrun arm and `RequestEffect::Later` both publish `refreshed`, so a later pass replaces the standing wait or request and starts the streak at 0. The two new tests cover the pass-1 `Later` then overrun, and the pass-1 overrun then pass-2 `Later`. A `Now` on the first pass is covered by the same set: `Answered` stores no flag, but the index is still in `refreshing`.

A later pass that returns `Later` for a refresh that never overran replaces rather than keeps, which is what that refresh's first ask would have done. The keep path in `enqueue` is `keepable && !forced`, and the first pass records the effect while the resource is still in `force`, so that ask already skips keep. The later pass now does too. Only the surviving effect is enqueued, at `settlement.rs:955`. A re-query drops the earlier `Later` at `settlement.rs:607` before the new ask; that request was never pushed to `self.requests`. The reuse check still uses the per-pass `forced`, so an unforced store re-read can still reuse and still keep its streak.

**Drop-hold gate.** `earliest` is the due re-ask inside timer and wake order, and `ready` uses it. `reask` is that same value with `at < hold` applied, so a hold still keeps the re-ask from dispatching, but a `then`, `next`, or answer later than it no longer runs first. With the deadlines from round 3 (hold 1000, re-ask 1250, `then` 1300) and timers running, `ready(1000)` is true because the hold is not after the re-ask, the hold runs, and the same loop then dispatches the re-ask and the `then`.

While timers are held, that re-ask does not dispatch and the hold does not either (`advance.rs:480` requires `timers`). Work strictly after the re-ask waits with them. The next pass with timers resumed runs the hold, because `hold <= earliest` makes it `ready`, then the re-ask, then the later work. Work at or before the re-ask still runs in the held pass. `advance_steps` stops when nothing is selected; it does not spin.
