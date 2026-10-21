# Code review: clock timelines, LLP 1055.002, round 2 (fba216439), 2026-10-04 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, read-only sandbox, `-C` a detached worktree at `fba216439` (the branch with round 1's fixes, rebased on `880ae483b`).
- **Method:** one brief (sha256 `9bae271f04fc6eeeb517ce883ba40a0be432a7c391642155626b39b8dc809a20`), shared with grok. Round 2, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the run's final message, unedited.
- **Verdict:** NOT READY.
- **Disposition (r2 fixes):** the engine now joins at the end of a commit (`hold_clock_joins` / `join_clocks`, LLP 1055.002 D6 "Within a commit").
  - 1 (a same-commit swap discards the new animation): taken by that; `in_one_commit_a_member_ended_elsewhere_is_gone`.
  - 2 (an unbind onto a clock is no longer sampled): taken; grok raised it too. A join reschedules the node (`schedule_animations`); the drag test now asserts frames and `quiescent`.
  - 3 (a web animation moved to another clock does not join it): taken; grok raised it too. `sync` tracks each animation's clock; `clocks.test.mjs` moves one.
  - 4 (a clock and timing change in one commit join with the old timing): taken by the commit-end join; `in_one_commit_a_join_reads_the_row_it_ends_with`.
  - 5 (a paused-only node moved onto an idle clock keeps a stale origin): taken; grok raised it too. `a_paused_node_moved_onto_an_idle_timeline_sets_its_origin`.
  - 6 (an ended play restarted by the move): taken; grok raised it too. `moving_onto_a_clock_leaves_an_ended_play_ended`.
  - 7 (retention under the agent adapters): taken; `start` prunes every clock once per commit time.
  - 8 (no timeline symbols): not fixed; LLP §5, known gaps.
  - The two agent-clock gaps: not fixed. Grok judged neither blocking; the author-pause tracking is main's for every animation (LLP 1055 D10). LLP §5.

---

**NOT READY.** Several “taken” findings remain incomplete, and the native ordering gap can suppress an entire finite animation.

1. **High — Same-commit replacement can discard the new animation.** [kernel/src/motion.rs:103](kernel/src/motion.rs:103)  
   This acknowledged round-1 gap should block landing. Suppose the sole member started at `0`. At `1.2s`, one commit clears it and creates an `800ms 1 alternate` member. Created nodes are processed first, so the new member inherits origin `0`, starts at `0`, and has already ended at `0.8s`. The web starts it at `1.2s`. This is more than an origin difference. **Fix:** reconcile cancellations, clock membership, and visibility for the complete commit before calculating joins.

2. **Medium — Drag-to-clock rejoining does not restore frame scheduling.** [motion/src/engine/clock.rs:63](motion/src/engine/clock.rs:63)  
   Binding a drag timeline removes the node from `animating`. Unbinding now clears its hold, recalculates its start, and marks it dirty, but never reinserts it. `MotionSync` already called `set_animations` while it was bound. Consequently, Linux receives one corrected frame and then stops updating that animation; the engine can also incorrectly report quiescence. The new test directly samples the play, bypassing this bookkeeping. **Fix:** recompute sampled-animation scheduling after rejoining, and test subsequent `advance`/`frame` output and `quiescent()`.

3. **Medium — Existing web animations still do not join a changed clock.** [host/web/navigation.js:411](host/web/navigation.js:411)  
   `sync` skips an animation whenever its paused status is unchanged. Change a running member from clock A to B at `300ms`: pruning removes it from A, but it never joins B or changes start. A second B member appearing at `600ms` establishes a separate origin. Evaluating the actual function with DOM stubs produced starts `[0, 600]`, where `[300, 300]` is required. Paused clock changes are skipped too. **Fix:** track clock identity separately from pause state and reconcile membership before either skip.

4. **Medium — A simultaneous clock and timing change joins using the old timing.** [motion/src/engine/clock.rs:35](motion/src/engine/clock.rs:35)  
   The kernel applies clocks before animations, so `rejoin_clock` sees the previous duration/direction. At `5.3s`, move an existing `pulse 1s infinite` onto a busy clock with origin `0`, while changing it to `pulse 2s infinite`. Rejoining chooses start `5`; `set_animations` matches the name and preserves that start. The required boundary is `4`, leaving it one second out of phase. **Fix:** calculate joins from the final animation rows after matching retained plays.

5. **Medium — Moving only paused plays onto an idle clock preserves a stale origin.** [motion/src/engine/clock.rs:57](motion/src/engine/clock.rs:57)  
   If Pending previously had origin `0` and became idle, moving a paused node onto it at `10.3s` makes it busy but never initializes its origin: every play is filtered out. A one-second joiner at `10.5s` sees the paused member as live and starts at `10`, rather than `10.3`. **Fix:** establish membership and the idle-to-busy origin independently of whether a play’s held time should change.

6. **Medium — Clock changes restart completed finite plays.** [motion/src/engine/clock.rs:58](motion/src/engine/clock.rs:58)  
   `hold.is_none()` also admits finished plays. A one-second forwards-filled fade completed long ago restarts when its node moves onto an idle clock, visibly replacing its final value. D6 specifies rejoining running plays; completion should not become a restart merely through this setter. **Fix:** distinguish completed plays from running ones before rewriting starts, including nodes containing both.

7. **Medium — Detached-animation retention remains under both web agent adapters.** [host/web/navigation.js:345](host/web/navigation.js:345), [host/web-js/agent.js:154](host/web-js/agent.js:154)  
   Global pruning happens only in `sync()`. Agent mode instead calls `start()` for newly registered animations, which prunes only that animation’s clock. Remove a screen and never reuse its clock: its animations and detached targets remain strongly retained. The in-memory check confirmed that removal remains retained until `sync` runs. **Fix:** expose membership cleanup separately from real-clock synchronization and call it on agent commits, including empty ones.

8. **Low — Timeline symbols still lack definitions and references.** [contract/cli/src/symbols.rs:257](contract/cli/src/symbols.rs:257)  
   Resolution now prevents the type-checking failure, but the symbol graph never defines timelines, imports search only component/shape/style/function kinds, and rewritten string expressions produce no reference. Querying `Pending` therefore cannot navigate its declaration, import, or use. The new test only calls `.unwrap()`. **Fix:** emit timeline definitions and authored reference edges, and assert their source spans.

The other two acknowledged agent gaps should also block the claimed D1–D6 behavior:

- **Medium — Author pause/resume remains untracked**, at the two adapter locations above. An initially paused animation remains in `held` after resume; an existing running animation never recalculates its shared start. Track authored pause state separately from the adapter’s own pauses.
- **Medium — JS-agent seeks can reuse an expired origin**, at [host/web-js/agent.js:181](host/web-js/agent.js:181). Jump past a finite member’s end to a timer-created joiner: registration precedes seeking the old member. Advance existing members first, or calculate liveness at the supplied virtual time.

The boundary correction, listed lexical-shadowing cases, hyphenated-name grammar, schema comment, and Lean entry-point resolution look correct by inspection. Their new tests appear sensitive to reverting the corresponding fixes. Coverage remains incomplete: the drag test bypasses frame delivery, the symbols test checks acceptance only, and there are no added tests for clock changes, cleanup, or the lifecycle cases above. Thus the “taken” drag-rejoin, retention, symbols, and clock-movement findings are not fully closed.

The rebase preserves both sides of the conflicting imports and main’s `followOnSeek`, `seek(to, sync)`, and `perf` integration. Pruning costs a scan of retained members per sync, plus repeated group scans for new joins—quadratic for a large batch joining one clock. I did not measure browser performance.

The checkout remains unchanged. No builds, repository tests, browsers, native apps, or remote commands were run. Verification was source inspection, arithmetic, and in-memory execution of `animationClocks` with stubs; actual host behavior and performance remain unverified.