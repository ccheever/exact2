# Code review: the Overrun re-ask redesign, round 2, 2026-10-27 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `4f080c8f3b`.
- **Method:** one brief (sha256 `df58f3f8c5f533312c1836fd7959bfa8ffd7c7c674fc6c371712054d216f1381`), blind to the other reviewer; 3 rounds (runtime, client-reachable), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

- **BLOCKER — Failure publication loses the retry’s origin and invocation.** [overrun.rs:242](runner/src/runner/overrun.rs:242). When the second re-ask’s commit is refused, `as_invocation` has already returned before `release_failed` runs. That function performs another settlement: downstream asks driven by `failed(r)` or `pending(r)` therefore run as App/id 0. This can admit person-only module calls and detach work from the originating invocation. The wait-expiry path at line 264 has the same problem. Keep failure publication under the waiting ticket’s provenance.

- **BLOCKER — A second settlement pass defeats the forced-refresh fix.** [settlement.rs:769](runner/src/runner/settlement.rs:769). Suppose a refresh overruns on a store-reading resource, then a later resource writes the store. Settlement repeats, but the first pass consumed `force`, and the second query discards the earlier `Wait`. Its replacement now has `forced=false` and recovers the old pending request’s count. `hold_reask` consequently preserves that old wait—including another invocation’s ticket, deadline and ownership. Preserve the refresh/replacement metadata across settlement passes.

- **SHOULD — Arguments changing at the retry deadline inherit the old streak.** [overrun.rs:51](runner/src/runner/overrun.rs:51). The active re-ask override matches only the resource index, ignoring `args`. For example, arguments derived from `floor(performanceNow()/1000)` can change between retries at 250 and 1250 ms. The first overrun for the new arguments is then treated as the spent second retry and fails immediately. Carry the count only when the arguments still match the waiting ask.

- **SHOULD — A slow synchronous turn consumes its own backoff.** [overrun.rs:105](runner/src/runner/overrun.rs:105). This schedules from `now_ms` sampled at commit start; the live clock is sampled again only in `conclude`. A turn taking 400 ms therefore creates a 250 ms deadline already in the past, causing an immediate retry. A sufficiently slow next turn also consumes the 1 s delay. Arm the wait relative to refusal/completion while retaining deterministic frozen-clock behavior.

Reviewed `d3a58d5e2a..4f080c8f3b` read-only. Findings are code-traced; tests were not run.
