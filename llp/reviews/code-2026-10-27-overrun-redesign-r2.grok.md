# Code review: the Overrun re-ask redesign, round 2, 2026-10-27 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `4f080c8f3b`.
- **Method:** one brief (sha256 `df58f3f8c5f533312c1836fd7959bfa8ffd7c7c674fc6c371712054d216f1381`), blind to the other reviewer; 3 rounds (runtime, client-reachable), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Note:** Grok's narration before its verdict is left out.

Round 2 of the redesign is not clean. Fixes 1–5 hold, and the same-loop deadline order holds while timers run. Two holes remain.

## BLOCKER

`host/render/src/lib.rs:818` — a runner-held re-ask stalls page render until the wall deadline, and the page comes back 503.

`Runner::pending` (`runner/src/runner/commit.rs:1005`) lists every entry in `self.pending`, including a wait (`reask_at: Some`). That wait is never handed to the host: `hold_reask` pushes no `RequestOut`, and `take_requests` (`commit.rs:927`) only drains `self.requests`. `has_pending` / `in_flight` (`commit.rs:1027`, `commit.rs:1082`) correctly leave it out via `waited()` (`runner/src/runner.rs:372`). The Linux, Swift, and web agent settles follow that split and seek `reask_due_ms`.

The page renderer does not. `settle` (`host/render/src/lib.rs:785`) treats every `pending()` ticket as executor work:

```817:831:host/render/src/lib.rs
        if runner
            .pending()
            .iter()
            .all(|(_, t)| held.refused.contains(t) || held.busy.contains(t))
        {
            return Ok(if held.busy.is_empty() {
                Settled::Complete
            } else {
                Settled::Busy
            });
        }
        if Instant::now() >= deadline {
            return Ok(Settled::Deadline);
        }
        executor.wait(deadline);
```

A resource that overruns with a kept value or a placeholder takes the `Wait` arm (`runner/src/runner/settlement.rs:751`) and the commit stands. Render then blocks on an executor that will never answer that ticket. `Settled::Deadline` is HTTP 503 (`host/render/src/lib.rs:88`). This is the boot and refresh path for any rendered page whose data-module turn hits the CPU budget or the wall backstop.

`settle` should wait the way `has_pending` does, and advance the runner to `reask_due_ms` so the re-ask actually runs inside the render deadline. `pending()` can keep reporting the wait for visible pending (`pending_res`, the boot document in `host/render/src/direct.rs:60`).

## SHOULD

`runner/src/runner/advance.rs:474` — with timers held, a re-ask still lands a drop that should already have timed out.

Fix 6 is real while `timers` is true: `ready` (`advance.rs:317`) requires every candidate at or before the earliest re-ask, and the re-ask block (`advance.rs:499`) sits after the reorder block (`advance.rs:472`). Answer, `then`, and `next` pass that gate.

The reorder candidate is still `timers &&`. The re-ask candidate is not (`advance.rs:312`). `advance_within` (`advance.rs:220`) passes `timers && !unseen`, and `land_then` (`advance.rs:207`) passes `timers = false`. An unseen wake or a hidden tool call (`wake_stops_timers`, `call_stops_timers`) therefore advances across a hold deadline without calling `reorder_timeout`. `timer_due_ms` still reports both times (`runner/src/runner/gates.rs:112` and `gates.rs:115`), so the host does wake.

The re-ask's `commit_again` ends in `apply` → `reconcile_reorder` (`runner/src/runner/apply.rs:50`). A hold whose row has moved or vanished ends there as Landed or Gone (`runner/src/runner/reorder_group.rs:425`), which clears `deadline_ms`. The timeout (`reorder_group.rs:462`, ending `Timeout`, "the move did not show within 1 s") never runs. A drag of a list whose source overran, overlapping a hidden wake or tool call, can commit the drop after the 1s hold has already passed.

Gate the reorder deadline on the same `ready` path as the re-ask whenever the clock is moving, including the timers-held window. Astra's note stands: a reorder hold is not constructible in the runner's unit fixtures, so this is from reading.

## Fixes that hold

1. A spent streak of refused re-asks returns `release_failed`'s receipt (`overrun.rs:242`).
2. A re-ask is chosen beside `timer_at`, not behind `timers` (`advance.rs:312`). The test's `land_then` path is the one that fires it.
3. `hold_reask` keeps a standing wait only when `!forced` and the args and streak match (`overrun.rs:75`).
4. The LLP 1054 keep path sets `reasks` to 0 (`commit.rs:825`). A wait has `keepable: None`, so an argument change replaces it; `overrun_reasks` then sees different args and starts at 0.
5. `enqueue` stamps `overrun_reasks` before the old ask is removed (`commit.rs:839`). The store-reader copy in `fulfill_inner` (`commit.rs:1373`) writes that same count back onto the request just pushed.

`hear_forgotten` omits waits (`commit.rs:221`), so a Mixed reservation is released when the reply's turn is over. The streak cannot replenish except on a forced refresh, which is specified to start at 0. A re-ask that overruns again moves `reask_at` forward, and `TIMER_FIRE_LIMIT` is checked before the re-ask fires, so one `advance_steps` does not spin.
