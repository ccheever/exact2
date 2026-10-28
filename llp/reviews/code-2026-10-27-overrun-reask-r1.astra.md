# Code review: the Overrun re-ask (first build), round 1, 2026-10-27 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `f9d20916a9`.
- **Method:** one brief (sha256 `8623845a3b6f4008c4dcde75faec8c2d50bfa21db98fe949d9b6ffbf716f8b68`), blind to the other reviewer; 3 rounds (runtime, client-reachable), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Note:** Found the first build's model wrong; theme2 approved a redesign at 18:55 and restarted the round count.

- **BLOCKER — Retries lose origin and invocation ownership.** [overrun.rs:125](runner/src/runner/overrun.rs:125)  
  `Wait` retains neither origin nor invocation ID, and the timed commit runs without `as_invocation` or `reasking`. A resource originally asked by a tool/wake is subsequently asked as `App`, with invocation ID 0. Its module calls can consequently pass `person-required` admission checks, and the original invocation can finish while its retry remains outstanding. Preserve both provenance and ownership through the wait and retry.

- **BLOCKER — A refused retry commit strands the resource pending forever.** [overrun.rs:115](runner/src/runner/overrun.rs:115)  
  The deadline becomes infinity **before** `commit_again` checkpoints. If the retry answers successfully but a downstream derive, resource, or gate refuses the commit, rollback restores that infinite wait. These retries are not registered as reconciling work, so `give_up` does not clear it. The resulting state has `pending(r) == true`, no request, no failure, and no future wake. Handle this refusal with a terminal failure or a bounded, finite reschedule.

- **BLOCKER — Unrelated commits bypass the backoff.** [settlement.rs:752](runner/src/runner/settlement.rs:752)  
  After an argument change overruns, the retained state still has the old arguments. Settlement’s `asked_args` considers actual tickets, but never `Wait.args`; therefore its reuse check fails at the next unrelated commit and queries immediately. Using the existing fixture, `change(1)` followed by two `tick` actions can exhaust all attempts without advancing the clock. The reply path can similarly re-ask immediately inside `release_overrun`’s publication commit. Waiting arguments must participate in settlement’s outstanding-ask logic.

- **BLOCKER — A downstream overrun can retry the wrong resource.** [overrun.rs:162](runner/src/runner/overrun.rs:162)  
  `overran_reply` checks the error class but ignores which resource failed. For example, A’s valid reply changes B’s arguments; B already has a delivered stream and its new synchronous turn overruns. B’s error escapes settlement because of the streaming guard, then this code schedules A again. A’s own turn succeeded. Classify the originating parse failure separately from failures elsewhere in its commit.

- **SHOULD — Clock jumps skip retry deadlines and ordering.** [advance.rs:695](runner/src/runner/advance.rs:695)  
  Retries run only after the clock reaches the requested endpoint and ordinary timers have fired. With continuous overruns starting at 0, `advance(1250)` performs the first retry at 1250 and schedules another at 2250; stepping through 250 and 1250 instead exhausts the streak. An intervening timer also observes the pre-retry value. Integrate retries into deadline selection and `advance_until_request` stopping.

- **SHOULD — Frozen settle reports success while a retry waits.** [commit.rs:914](runner/src/runner/commit.rs:914)  
  Only expression pending flags include waits; `has_pending()` and the agent’s pending list do not. Apple/Linux settle selects motion endpoints, so with no motion it returns `settled: true` before the 250 ms retry, leaving the resource pending. The live wake works, but does not drive a frozen clock. Settle needs explicit handling of these bounded waits; merely adding them to `has_pending()` would instead wait on an unmoving clock.

- **SHOULD — The mixed test never crosses the mixed envelope.** [overrun.rs:142](js/tests/it/overrun.rs:142)  
  Both children use `Placement::Main`, so `Mixed` creates no ordered set and calls the child directly. This does not establish condition 4’s mixed-envelope coverage. Exercise a Main child ordered with a worker through overlapping grants, or another configuration that demonstrably traverses that envelope.

Reviewed HEAD `f9d20916a9`; it is currently **two**, not three, commits ahead of `origin/main`. Tests were not run: the sandbox is read-only and this checkout has no build artifacts.
