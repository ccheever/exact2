# Code review: the Overrun re-ask redesign, round 3, 2026-10-27 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `fa34d55045`.
- **Method:** one brief (sha256 `ae375e6fc58dcf4c5fbc90e4a05cf83ffa7310fe0f4e5acecd3b52f807edc8aa`), blind to the other reviewer; 3 rounds (runtime, client-reachable), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Note:** Grok's narration before its verdict is left out.

Round 3 of `x2/overrun-reask` (`d3a58d5e2a..fa34d55045`, 6 commits) finds one blocker and one should. The six round-2 fixes hold on the paths their tests lock. The drop-hold ordering holds by reading.

## BLOCKER

`runner/src/runner/settlement.rs:424` — a forced refresh keeps another invocation's wait when a later pass of the same settlement asks again, unless the first pass itself overran.

`force.retain` at line 690 drops the resource on the first ask. `refreshed` is restored only from `RequestEffect::Wait { forced: true }`. The stale-store loop re-asks without clearing `effects` and without putting the resource back in `force`. If pass 1 returned `Later { forced: true }` (line 845) or `Now` (`Answered` stores no flag), pass 2's overrun calls `overrun_reasks` and `hold_reask` with `forced: false`. The standing wait has the same arguments and the same `reasks`, so `hold_reask` returns at `runner/src/runner/overrun.rs:76` and leaves that ticket, its due time, and its origin in place. The pass-1 `Later` was already discarded.

`a_refresh_asked_again_in_a_later_pass_still_replaces_the_wait` stays green because `refuse` makes every query overrun, so both passes take the `Wait` arm. A store reader that schedules a request on the first turn, then overruns on the re-query after a later resource writes, still loses the refresh while a wait from an earlier invocation is standing.

Remember the ask as forced for every pass of this settlement once it was forced. Include `Later { forced: true }`, and remember a forced `Now` as well: leave the index in `force` until publish and set `refresh_next` from what was never asked, or record a per-resource bit that survives `Answered`. Add a test whose first pass returns `Later` or `Now` and whose second pass overruns, asserting a new due time and the refresh's origin.

## SHOULD

`runner/src/runner/settlement.rs:848` — the `Later` arm publishes the per-pass `forced` flag, not `refreshed`.

After a forced overrun, pass 1 stores `Wait { forced: true }` and line 424 does set `refreshed` on pass 2, but a pass-2 `Later` still records `forced: false`. `enqueue` then runs `overrun_reasks` unforced and copies the standing wait's streak onto the new request. A refresh during the second delay (the standing wait already has `reasks == 1`) whose re-query returns a request keeps one try and a 1 s first delay. The ticket is replaced, so the origin is the refresh's. The retry budget is the previous invocation's.

Pass `refreshed` into `RequestEffect::Later`, as the Overrun arm does for `Wait`.

## Round-2 fixes

1. **Holds.** `overrun.rs:262` and `:286` run `release_failed` and `reask_answered` under `as_invocation` of the wait ticket. The failing commit asks `watch` inside that frame, which is why the test sees Tool/7.
2. **Holds only when every pass overruns.** See the blocker. The all-overrun test locks the path that was fixed.
3. **Holds.** `overrun.rs:51` carries only when the resource and the arguments match. A forced re-ask whose arguments differ falls through to 0. `overrun_reasking` is restored when `commit_again` returns.
4. **Holds.** `overrun.rs:116` reads the live sampler and does not assign `now_ms`. A seek clock (`sample` is `None`) uses `now_ms`. `hold_reask`, `hold_overrun_reply`, and the re-arm all call it. The slow-turn fixture moves the shared clock 400 ms inside the refused turn and expects due at 1650.
5. **Holds.** `host/render/src/lib.rs:823` completes when every `pending()` ticket is refused, busy, or `is_reask_wait`. The wait stays; `pending_res` stays true, so the checkpoint lists `post`. `has_pending` still excludes waits, and Linux, Apple, and web `settle` still chain `reask_due_ms`. Render settle does not advance the clock.
6. **Holds.** `advance.rs:316`. `hold` is the drop deadline when it is already `<= now`, with no dependence on `timers`. The re-ask is a candidate only while `at < hold`, so an equal or later re-ask waits with the hold while timers are held and does not pass it. An earlier re-ask still fires, and it sits on `ready`, so a later hold does not jump ahead of it. With timers running, the hold block uses `ready` and runs before a same-time re-ask. With no due hold, a re-ask still fires while `timers` is false. `reask_at == INFINITY` never matches `reask_due`.
