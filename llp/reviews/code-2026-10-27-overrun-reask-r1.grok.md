# Code review: the Overrun re-ask (first build), round 1, 2026-10-27 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `f9d20916a9`.
- **Method:** one brief (sha256 `8623845a3b6f4008c4dcde75faec8c2d50bfa21db98fe949d9b6ffbf716f8b68`), blind to the other reviewer; 3 rounds (runtime, client-reachable), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Note:** Found the first build's model wrong; theme2 approved a redesign at 18:55 and restarted the round count. Grok's narration before its verdict is left out.

Reviewed `d3a58d5e2a..HEAD` on `x2/overrun-reask` (2 commits, 25 files). `origin/main` is 5 unrelated commits ahead of that merge-base, so this is not a raw `git diff origin/main HEAD`.

## BLOCKER

### 1. A refused re-ask leaves the resource pending forever
`runner/src/runner/overrun.rs:114`

`reask_overruns` sets `due_ms = f64::INFINITY` and `force_refresh`s before `commit_again`. The checkpoint is taken inside `commit_again`, so it already stores the infinite due (`runner/src/runner/commit.rs:92`, restored at `commit.rs:166`).

Infinity means "the ask has gone out; wait for its answer" (`overrun.rs:25`). If that commit is refused, the ask never goes out, and the rollback puts the infinite due back anyway. `overrun_due_ms` drops non-finite times (`overrun.rs:94`), so `timer_due_ms` does not wake. The next `advance` skips the wait because `due_ms > now_ms` (`overrun.rs:105`). `sync_pending_flags` still forces `pending_res` (`commit.rs:914`). `has_pending` only looks at tickets (`commit.rs:1025`), so the agent's settle does not wait either.

That refusal is a normal settlement error, not a special case. The forced query returns `Unavailable` or `BadArguments`, or a later derive or gate traps (`gates.rs:21` runs on this commit). `commit_again` only calls `give_up` when `reconciling` is non-empty (`admission.rs:52`). A timer re-ask never `owe`s the resource, so the streak is not ended and the resource is not failed.

On screen the earlier commit already showed pending and the last value. Nothing after that answers, fails, or arms a timer.

Set the infinite due only after that commit stands, or take the checkpoint before mutating it and, on refusal, fail the resource the way `give_up` does.

### 2. A later pass of the same settlement both burns a try and clears `pending`
`runner/src/runner/settlement.rs:761`

The overrun arm always does `effects[i] = RequestEffect::Answered` and then `overran()`, which increments `tries` (`overrun.rs:45`). `Answered` means "this ask is finished": the next pass of the fixpoint clears `pending` and `awaiting` before anyone is re-evaluated (`settlement.rs:246`).

`settle_pass` repeats when any store-reading resource's revision is stale (`settlement.rs:899`). A later resource whose query writes the store is enough. That is the case the loop is for.

On that next pass:

- The overrun resource's `pending` is forced false. A `Later` ask is set true again (`settlement.rs:251`); an overrun is not, unless it enters the overrun arm again. Derives that read `pending(...)` therefore publish false. A direct `pending()` call in the view still sees `self.pending_res` after `sync_pending_flags`. The same commit can show "not loading" in a derive and "loading" in the view.
- If this resource is itself a stale store reader, reuse fails and it is queried again immediately. `overran` treats that as the next try. One extra pass consumes the 250 ms slot (`tries` goes 1 → 2, the only delay left is 1000 ms). A second extra pass exhausts `REASK_MS` and fails the resource in the original commit: last value, failure alert, `pending` false. The action still stands. Theme2's conditions 1 and 2 do not.

The 250 ms gap exists so a load spike is not asked again on the next tick (`overrun.rs:13`). This fixpoint is that tick.

Count a try only when a re-ask is actually dispatched, and do not use `RequestEffect::Answered` for a wait that is still pending. A `Later`-style effect keeps `pending` true across passes; forgetting the old ticket can happen once, when the pass that scheduled the wait is published.

## Checked, not filed

- Single-pass sync overrun and the reply path match the ruling: last value kept, `pending_res` set, no `failed_args` until the streak is spent, checkpoint rolls the wait back with a refused action, argument changes drop the wait in `overrun_args` and again in `reask_overruns`, mutations and delivered stream messages are not re-asked (`streaming` at `commit.rs:1113`, `release_overrun` at `overrun.rs:138`, `message` goes to `release_failed`).
- `timer_due_ms` includes `overrun_due_ms` (`gates.rs:112`). The agent seeks with that (`host/linux/src/agent.rs:1298`), so a frozen agent's clock still fires the wait when it is seeked across. It does not fire if the due is infinite.
- `has_pending` ignores the wait, so the gap between refusal and the 250 ms re-ask does not wedge settle. The hang is blocker 1, where there is also no ticket.
- There is no 250 ms forever loop on the intended path: two delays, then the same failure as any other refusal. Blocker 2 spends those delays inside one commit instead.
- The class is preserved on the hops that produce it: in-process `DataError`, the worker envelope (`data/src/envelope.rs:136` and `:151`), and the mixed composer via `envelope::encode`. Older peers and `logic/abi` tag 4 still decode as `Unavailable`, which is the intended seam. `rt.js` is not in the diff. Budget refusals in `begin`, `resume`, `message`, and `overlay` return `Overrun`.
