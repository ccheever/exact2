# Code review: clock timelines, LLP 1055.002, round 3 (253294cd1), 2026-10-04 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, read-only sandbox, `-C` a detached worktree at `253294cd1`.
- **Method:** one brief (sha256 `abeba7672539bc00dd44467499bc95202e973f6bf4e6c0535b569c2c84bb76ed`), shared with grok. Round 3, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the run's final message, unedited.
- **Verdict:** NOT READY.
- **Disposition (r3 fixes, the third and last fix round):**
  - 1 (two commits at one time reuse a dead member): taken; grok raised it too. Pruning is per commit (`commit()`, from `sync` and both agents' `register`), not per timestamp. `clocks.test.mjs` syncs twice at one time.
  - 2 (a finished animation restarted by a clock change): taken. `clocks.test.mjs` moves a finished one.
  - 3 (clock, then none, then the same clock): taken; grok raised it too. `sync` joins any animation on a clock that is not its member. `clocks.test.mjs` takes the page's last clock away and gives it back.
  - 4 (agent removal-only commits retain): taken; grok raised it too. `register` calls `commit()` every commit.
  - 5 (stale indices across two rows in one held commit): not fixed; no kernel path produces it. LLP §5.
  - Test coverage: `kernel/tests/it/clock_sync.rs` drives `MotionSync::apply` and fails without its hold.
  - The agent-clock gaps: not fixed, as in round 2; grok judged them non-blocking. LLP §5.

---

**NOT READY.** The native commit-end fix addresses the main round-2 failures, but web lifecycle errors remain. One can suppress an entire finite animation.

1. **High — Separate commits at the same timestamp reuse dead members.** [host/web/navigation.js:391](host/web/navigation.js:391)  
   `now === pruned` does not mean the commit was already reconciled. Suppose an `800ms infinite alternate` member started at `0`. At `1200ms`, one commit runs `sync`; another commit at that same timestamp replaces it with an `800ms 1 alternate` animation. Pruning is skipped, so the detached member preserves origin `0`. The replacement receives start `0` and has already finished, instead of starting at `1200ms`. This is possible between browser frames and under the agent clock.  
   **Fix:** prune once per actual commit, including empty commits; use an explicit commit boundary rather than timestamp equality.

2. **Medium — Changing a finished web animation’s clock restarts it.** [host/web/navigation.js:424](host/web/navigation.js:424)  
   A one-second forwards-filled animation finishes. At `10.3s`, change its clock to an idle clock. Clock identity now triggers `start()`, and `sync` writes `startTime = 10.3s` because “finished” is not “paused.” The animation plays again. The engine’s new ended-play guard correctly prevents this, so the web change introduces a D6 and cross-host discrepancy.  
   **Fix:** exclude finished/idle animations from joining and start-time writes; preserve completed plays when their clock changes.

3. **Medium — `clock → auto → same clock` fails to rejoin.** [host/web/navigation.js:415](host/web/navigation.js:415)  
   Start the sole clock-A pulse at `0`. Remove its clock at `300ms`: pruning removes its membership, but the early return leaves `clocked` recording A. Restore A at `600ms`: unchanged pause state and apparently unchanged clock cause the animation to be skipped. A new A member at `900ms` establishes origin `900ms`, while the original retains start `0`; both should have joined the origin established at `600ms`.  
   **Fix:** reconcile tracked clock identity even when no clocked elements remain, and do not infer membership solely from the cached clock/pause pair. The round-2 clock-movement finding is therefore only partially fixed.

4. **Medium — Agent removal-only commits still retain detached animations.** [host/web/navigation.js:345](host/web/navigation.js:345), [host/web-js/agent.js:154](host/web-js/agent.js:154)  
   Both adapters call `clocks.start()` only for previously unseen animations. Remove the last animated screen and continue with commits containing no new animations: neither `start` nor `sync` runs, so the member sets retain the removed animations and targets indefinitely. Moving pruning into `start` does not cover this case.  
   **Fix:** run cleanup explicitly at every agent commit, including commits with zero animations. Round-1 retention and round-2 Astra finding 7 remain incorrectly marked “taken.”

5. **Low — Pending row indices become stale after another animation-row write.** [motion/src/engine/clock.rs:169](motion/src/engine/clock.rs:169)  
   Through the new held-join API, start with an existing `pulse` on a clock whose origin is `0`. At `10.3s`, hold joins, set the row to `[spin, pulse]`, then reorder it to `[pulse, spin]`. The first write records newly started index `0`; the second preserves that index without remapping it. Joining then adjusts `pulse`, while the new `spin` remains at `10.3s`, out of phase.  
   **Fix:** carry pending join status through the same play matching used by `set_animations`, dropping cancelled entries and remapping retained ones. I found this through the public batching API; the normal kernel emitter reads final rows, so I did not establish a Contract path producing these differing duplicate rows.

The other targeted native changes look correct by inspection: `MotionSync::apply` releases the hold even on error; removal and clearing a clock erase pending entries; `Joining::Node` correctly takes precedence when a node both moves and resumes. Final-row timing, paused-only origin establishment, ended-play preservation, and rescheduling address their reported failures. Actions and mutations now participate in shadowing.

The added tests improve coverage, with limits:

- The drag-unbind frame assertion detects the missing scheduling fix. Its `quiescent()` assertion alone is masked by the still-running `LOCK`.
- `in_one_commit_a_drag_bound_play_is_no_member` unbinds the node before joining; it does not test exclusion of a node that **remains** drag-bound.
- The commit tests call the engine batching methods directly, so they do not protect the `MotionSync::apply` integration.
- The browser mover test covers ordinary A-to-B movement, but none of the web failures above. No regression test was added for action/mutation shadowing.

Of the explicitly open gaps, **both agent timing gaps should block the stated D1–D6 behavior**. An initially paused animation remains in `held` after resume ([navigation.js:345](host/web/navigation.js:345)); authored pause state needs reconciliation separately from adapter pauses. The JS agent also registers a timer-created joiner before advancing older members ([agent.js:181](host/web-js/agent.js:181)): a one-second member at `0` can incorrectly keep the clock busy for a joiner at `1500ms`. Advance existing members first or evaluate membership at the supplied virtual time. Missing timeline symbol navigation is nonblocking; compiler acceptance is fixed, navigation remains explicitly deferred.

I reproduced the web start assignments and retained membership using the actual functions with in-memory DOM stubs. No builds, repository tests, browsers, native apps, or remote commands were run. Actual host behavior, performance, and backgrounding remain unverified. The checkout is unchanged.