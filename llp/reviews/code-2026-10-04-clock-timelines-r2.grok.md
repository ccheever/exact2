# Code review: clock timelines, LLP 1055.002, round 2 (fba216439), 2026-10-04 (grok)

- **Family:** xAI. `grok-4.7`, reasoning effort xhigh, `--no-subagents`, `--sandbox read-only`, `--cwd` a detached worktree at `fba216439`.
- **Method:** one brief (sha256 `9bae271f04fc6eeeb517ce883ba40a0be432a7c391642155626b39b8dc809a20`), shared with astra. Round 2, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the findings and verdict, unedited (the opening narration dropped).
- **Verdict:** SHIP WITH FIXES.
- **Disposition (r2 fixes):**
  - High (an unbind onto a clock is no longer sampled): taken; astra's 2.
  - Medium (an ended play restarted): taken; astra's 6.
  - Medium (a paused-only node keeps a stale origin): taken; astra's 5.
  - Medium (a web animation moved to another clock): taken; astra's 3.
  - Medium (a drag-bound play counts as a member in the same commit): taken by the commit-end join, which skips bound nodes; `in_one_commit_a_drag_bound_play_is_no_member`.
  - Low (an action or mutation of the name): taken; both shadow a timeline.
  - Low (no timeline symbols): not fixed; LLP §5, known gaps.

---

**SHIP WITH FIXES.** The join, boundary, sole-member pause, and listed-binding shadowing fixes do what their tests say. `rejoin_clock` does not. On a sampling host a play that leaves a drag timeline onto a clock freezes, and a finished play moved onto a clock is started again.

## Findings

**High — Leaving a drag timeline onto a clock updates the start and then never samples it.** `motion/src/engine/clock.rs:52`

`rejoin_clock` rewrites `start` and marks properties dirty. It does not put the node back in `animating`. Drag-bound plays are taken out of that set by `seek_timeline` (`motion/src/engine/timeline.rs:176`) and by `set_animations`, because a held play is not `live`. The kernel applies timelines last (`kernel/src/motion.rs:100`), so the unbind at `motion/src/engine/timeline.rs:97` runs after that removal.

Node ENGINE is bound to a drag timeline and also on `Pending`, where LOCK has been running since t = 0. At t = 5.3 the drag binding is cleared. ENGINE’s start becomes 5.0, in phase with LOCK, and one dirty entry is recorded. ENGINE is not in `animating`. The next `advance` dirties only LOCK. Linux paints from `frame()`, so ENGINE stays on the join-frame opacity while LOCK keeps moving. `quiescent()` is true when ENGINE was the only motion, and the display loop stops. `leaving_a_drag_timeline_onto_a_clock_takes_its_phase` reads `animation_plays` directly, so it passes while the frame path is frozen. Apple nodes that stay sampled do the same; a node eligibility then lowers is played by Core Animation from the new start.

After reinserting the plays, update `animating` with the same `live && sampled` test `set_animations` uses (`motion/src/engine/animate.rs:129`).

**Medium — A finished play moved onto a clock is started over, and becomes the member that holds the timeline busy.** `motion/src/engine/clock.rs:57`

The rejoin loop keeps every play whose `hold` is `None`. That includes plays that have already ended. `clock_start` then sets `start` to the current boundary, so local time falls back inside the active interval.

`pulse 1s 1` ran off any clock, ended at t = 1, and is still the node’s animation (fill forwards is showing the end). At t = 10.3 the row changes to `clock(Pending)` on an idle timeline whose stored origin is 0. Rejoin sets `start` to 10.3. Local time is 0, the animation plays a full second again, and `set_animations` (same commit, after the rejoin) sees it as live and inserts `animating`. A joiner in that window shares that new origin. Before this fix the finished play kept its old start and stayed finished.

Rejoin only plays with `hold.is_none()` and `local(now) < end_time()`. Leave an ended play’s start alone, and do not call `clock_start` for it (that call is what sets the origin).

**Medium — A paused-only node moved onto a clock keeps a stale origin and counts as busy.** `motion/src/engine/clock.rs:57`

Paused plays are skipped (`hold` is `Some`). The comment says they rejoin on resume. They are already on the clock, and `clock_start` treats any hold as live (`motion/src/engine/clock.rs:87`). Nothing in the rejoin updates the origin.

Pending went idle at t = 2 with origin 0. A node has `pulse 1s infinite paused` and is not on a clock. At t = 10.3 it is moved onto Pending. Rejoin does not call `clock_start`, so the origin stays 0. A joiner at t = 10.5 sees the hold, treats the timeline as busy, and starts at 10.0. Resume does the same: `set_animations` passes the paused play in `also`, so it counts itself as live and keeps origin 0. D3 says the move onto an idle timeline sets the origin to now; D6 says the resume rejoins that phase. A running sibling in the same rejoin does set the origin, because that play is not skipped. The paused-only case does not.

When every remaining play is paused, still resolve the origin once, ignoring this node’s not-yet-joined plays, and leave the holds unchanged so resume can rejoin.

**Medium — The web drops a member that changed clocks and does not put it on the new one.** `host/web/navigation.js:411`

`sync` rejoins only when `playState` changes. `prune` then removes an animation whose `--exact-animation-clock` no longer matches. The `was === is` continue runs before `start`, so a running animation that gains a clock, or moves from one clock to another, keeps its old `startTime` and is not a member of the new clock.

A pulse has been running on Pending since t = 0. At t = 10.3 its custom property changes to Other, which is idle. Play state stays `running`. Pending no longer counts it. Other’s next joiner sets a new origin at 10.3 and gets that `startTime`. The moved pulse stays on `startTime` 0, so it is about 10.3 s into its own cycle. The engine path for this is `set_animation_clock` → `rejoin_clock`. The pause skip itself is right: a paused member stays in the set and does not get a `startTime` write. Drag-to-clock still rejoins on the web, because dropping the drag’s `animation-play-state: paused` flips play state and `start` runs.

After prune, call `start` when the animation’s clock is non-empty and it is not in that clock’s set, including when play state is unchanged. Keep skipping `startTime` while it is paused.

**Medium — In one commit, drag-held plays count as clock members before the unbind rejoin.** `kernel/src/motion.rs:100`

Clocks are applied, then animations, then timelines. `set_animation_clock` records the clock immediately and skips `rejoin_clock` while the node is still timeline-bound (`motion/src/engine/clock.rs:34`). The busy scan treats a hold as live. The rejoin that would set a fresh origin runs only at unbind, after every other node’s `set_animations`.

Pending is idle, origin still 0, now is 10.3. This commit creates A on Pending and moves B from a drag timeline onto Pending. B is inserted into the clock map with its drag holds still set. A’s `clock_start` sees those holds, keeps origin 0, and starts A at 10.0. B’s unbind then sees A and joins 10.0. The timeline was idle: both should have started at 10.3. B alone is fine, which is what `leaving_a_drag_timeline_onto_a_clock_takes_its_phase` covers.

In the busy scan, skip nodes that are still timeline-bound. A then sees an idle timeline, sets the origin to now, and B’s rejoin follows A.

**Low — An action or mutation of the timeline’s name is still rewritten to a clock.** `contract/syntax/src/clock.rs:43`

The shadow set is props, injects, state, derives, resources, and `each`/`match` binders. The view scope also binds mutations and actions (`contract/types/src/lib.rs:589`). `timeline Pending` plus `mutation Pending` (or `action Pending`) rewrites `animation-timeline=Pending` to `"clock(Pending)"` before types run. A state of the same name is left as the binding; the test covers that.

Treat mutation and action names like the other component bindings.

**Low — `contract symbols` typechecks the clock and still has nowhere to navigate to it.** `contract/cli/src/sources.rs:79`

`resolve_clock_timelines` in `sources::load` is why `symbols_json` and Lean accept `animation-timeline=Pending`. The rewrite turns the ident into a string first, and `declarations` (`contract/cli/src/symbols.rs:277`) never defines a `timeline`. Finding 7’s unknown-value failure is fixed. The symbol graph still has no timeline declaration and no reference from the use. `every_entry_point_sees_the_clock` only checks that the calls return.

Define the timeline from the declaration span, and record the use against that definition before replacing the ident.

## Round-1 items marked taken

| Item | Result |
|---|---|
| Boundary snap (astra 1) | Fixed. At 4.8 s with period 1.6 s, f64 `into` is `1.5999999999999996` (`period - into` ≈ 4.4e-16). The old formula returns 3.2; the 1e-9 s snap returns 4.8. The web uses the same 1 ns threshold in milliseconds. `a_join_on_a_boundary_starts_on_it` fails without the snap. |
| Sole web member pause/resume (astra 2, grok High) | Fixed. A pause no longer removes the member or resets the origin; resume calls `start` and sets `startTime`. The new solo-member assertions fail without that. |
| Node moved onto an idle clock (grok) | Fixed for a running play. `a_node_moved_onto_an_idle_timeline_starts_it_over` fails without `rejoin_clock`. Finished plays are restarted (finding above). A paused-only node does not set the origin. |
| Drag timeline unbound onto a clock (grok) | The start is fixed (`5.0`, not `now - held`). Sampling does not follow it (High above). |
| Retired web members (astra 6) and a member that left its clock (grok) | Prune on every `sync`, before the empty-page return, drops disconnected, finished, and re-clocked members. The re-clocked animation is not joined to the new clock (finding above). |
| Symbols and Lean (astra 7) | Typechecking is fixed via `sources::load` and Lean’s single-file parse. The symbol graph is not. |
| Shadowing (astra 8, grok) | Fixed for the bindings the LLP lists. `a_local_binding_of_the_name_shadows_the_timeline` and the `each` test fail without it. Actions and mutations are outside the set. |
| Hyphenated `clock()` names (astra 9) and the schema comment (grok) | Fixed. `clock(Pending-Work)` parses. The JS style map’s `[^)\\s]+` capture already accepts the hyphen. |

## Known gaps

None of the three should block landing on their own.

The native same-commit swap (created node applied before the touched node that clears the only member) is the one a person hits on a shipping host: the replacement indicator keeps the old origin and starts mid-cycle. The two agent-clock gaps are the dev clock only, and they match the adapter’s existing “record pause once” behavior. They do mean an agent-clock pause/resume, or one seek that jumps a timer past a finite member’s end, will not match the real clock. The LLP already says so.

## What holds up

Several plays rejoined in one call all pass `also: &[]` and the node is absent from the table, so none of them sees its siblings. On an idle timeline every one of them sets the origin to the same `now` and starts there. On a busy timeline every one of them sees the other nodes and keeps that origin. They do not split the phase. The bad cases are a finished sibling, which should not call `clock_start` at all, and a paused-only node, which never calls it.

The web pause skip is the right one: `if (is && was !== undefined) continue` leaves the member in the set and does not write `startTime` (that write would unpause). Resume is not skipped. `prune` walks the member set once per commit, which is the same commit `sync` already spends on `document.getAnimations()`. It is not per frame.

The rebase kept both sides of the import conflicts. `host/web/glue.js` still has `followOnSeek`, the agent `seek`, and `presence.live?.sync()`, and calls `clocks.sync()` only on the non-agent branch. `host/web-js/agent.js` still imports `perf` and keeps `seek(to, sync = true)`.

## What I could not verify

I did not run builds, tests, browsers, or native apps. The boundary number is a Python f64 calculation of the same expression, not `cargo test`. I did not run the Chrome clock test, and I did not watch a Linux or Core Animation frame after an unbind.
