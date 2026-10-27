# Code review: the Overrun re-ask redesign, round 1, 2026-10-27 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `a0b6e4c1b3`.
- **Method:** one brief (sha256 `77528edec53e8cfb8854f2d862885eda327f87e6a63d42e7712457f574374625`), blind to the other reviewer; 3 rounds (runtime, client-reachable), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

- **BLOCKER — [overrun.rs:243](runner/src/runner/overrun.rs:243): Final failure receipt is discarded.** If both re-ask commits refuse—for example, their new value triggers a downstream shape error—the second calls `release_failed`, which can successfully commit `pending=false` and the failure. Because `result` is still `Err`, that receipt is discarded. Web/Apple can retain the pending UI indefinitely while runner state says failed. Propagate the successful cleanup receipt even when reporting the original refusal.

- **BLOCKER — [advance.rs:465](runner/src/runner/advance.rs:465): Hidden invocations cannot dispatch their re-asks.** `advance_within` passes `timers=false` during hidden wakes/tool calls, blocking this branch. The wait still owns invocation work and remains advertised by `timer_due_ms`, so it keeps the invocation running without progressing until expiry/timeout. Frozen Linux/web settlement can also report settled with that wait outstanding. Runner-held request continuations need to progress independently of the authored-timer suspension.

- **BLOCKER — [overrun.rs:81](runner/src/runner/overrun.rs:81): An explicit refresh can retain the previous invocation’s wait.** Before the first re-ask, the standing wait has `reasks=0`. A forced refresh with identical arguments that overruns also produces `reasks=0`, so this equality check preserves the old ticket, deadline, origin, and invocation. For example, an App refresh of a Tool-owned wait neither supersedes the tool invocation nor retries under App provenance. Carry the forced/replacement distinction into `Wait`.

- **SHOULD — [commit.rs:823](runner/src/runner/commit.rs:823): Keeping a request across an argument change retains the old streak.** The LLP 1054 keep path updates `p.args` but leaves `p.reasks` unchanged. After retrying arguments A once, switching to B while keeping an identical GET makes B’s first overrun wait 1 second instead of 250 ms; with two retries already counted, B fails immediately. Reset the streak when the retained ticket’s arguments change.

- **SHOULD — [overrun.rs:60](runner/src/runner/overrun.rs:60): An ordinary store-reader re-query can replenish retries.** A store revision can re-query a waiting resource with unchanged arguments and without forcing it. If that query returns `Later`, `enqueue` replaces the wait and this helper assigns `reasks=0`, losing its existing count. A subsequent overrun regains both retries; repeated store commits can extend the streak indefinitely. Preserve the count for an unforced replacement with matching arguments.

- **SHOULD — [advance.rs:463](runner/src/runner/advance.rs:463): Re-asks overtake earlier reorder deadlines.** `ready(at)` compares against authored timers and transport deadlines, but not `reorder_deadline`. With a drop timeout at 1,000 ms and a second re-ask at 1,250 ms, one advance across both runs the re-ask first. Its answer can complete the move and remove a hold that should already have timed out. Select the earliest deadline before applying equal-time priority.

Read-only review; no files changed. These findings are code-traced. Runtime tests were not run because this tree has no built test artifacts.
