# Code review: clock timelines, LLP 1055.002, round 3 (253294cd1), 2026-10-04 (grok)

- **Family:** xAI. `grok-4.7`, reasoning effort xhigh, `--no-subagents`, `--sandbox read-only`, `--cwd` a detached worktree at `253294cd1`.
- **Method:** one brief (sha256 `abeba7672539bc00dd44467499bc95202e973f6bf4e6c0535b569c2c84bb76ed`), shared with astra. Round 3, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the findings and verdict, unedited (the opening narration dropped).
- **Verdict:** SHIP WITH FIXES.
- **Disposition (r3 fixes):**
  - Medium (a second sync in one frame keeps dead members): taken; astra's 1.
  - Medium (the last clock taken away and given back): taken; astra's 3.
  - Medium (agent retention on a removal-only commit): taken; astra's 4.
  - Tests: `MotionSync::apply`'s hold now has a kernel test.

---

**SHIP WITH FIXES.** The commit-end join in the engine matches D6 and holds up on the cases you listed. The web clock fix is incomplete in two ways, and the agent-retention item marked taken in round 2 is still open. None of the three gaps left in LLP §5 should block landing.

## Correctness

**Medium — A second clock sync in the same frame keeps dead members and hands their origin to the next joiner.** `host/web/navigation.js:390`

`prune` returns immediately when `now === pruned`. `document.timeline.currentTime` does not move inside a frame, and `applyBatch` calls `clocks.sync()` once per batch (`host/web/glue.js:905`). Boot already does several batches back to back (`glue.js:1354`), and a click plus the action batch it produces does the same.

At t = 0 a pulse on `Pending` sets the origin to 0. In a later frame the first batch leaves that pulse in place, so `prune` runs and records `pruned = now`. A second batch in that frame removes the pulse and adds another on `Pending`. The second `prune` returns at once, the detached animation stays in the set, and the new one sees a non-empty set, keeps origin 0, and writes that `startTime`. The detached member is dropped on the next frame, but the new `startTime` is never rewritten (`sync` skips an animation whose pause state and clock name are unchanged). A finite joiner can therefore start up to a full cycle early and finish before it shows, which is the failure D4's boundary exists to prevent.

The timestamp guard is there so `start` does not rescan every member for every animation inside one commit. Key it off a counter incremented at the top of each `sync` and each `register`, and let `prune` run when that counter changes. Calls to `start` inside the same commit then still skip the scan.

**Medium — Taking the last clock off and putting the same name back does not rejoin.** `host/web/navigation.js:415`

`sync` prunes, then returns when no element has `--exact-animation-clock`, before `clocked` or `paused` are written.

A running animation on `Pending` is synced, so `clocked` is `Pending` and it is a member. A later commit clears that property and nothing else on the page carries it. `prune` drops the member because `clockOf` is now `''`, then `sync` returns, and the WeakMap still says `Pending`. A following commit sets `--exact-animation-clock: Pending` on the same `CSSAnimation` (a custom-property change does not replace the object). Pause state and clock name both match the map, so line 418 continues. The animation is not a member and keeps its old `startTime`. A new joiner finds an empty set, sets the origin to now, and the two stay out of phase. D6 says removing the name detaches the play and adding it back joins again.

The new Chrome case moves `Before` to `After` while other clocked elements remain, so the loop runs and the map is updated. This path is the one where the loop does not run. On the empty-document return, set `clocked` to `''` for every current animation (or delete the entries). And treat "this clock is non-empty and this animation is not in its set" as a join even when the name matches, including the paused continue at line 423. A paused animation pruned out of the set is skipped there too.

**Medium — Round 2 marked agent retention taken; a removal-only commit still retains detached targets.** `host/web/navigation.js:344`, `host/web-js/agent.js:154`

Round 2's disposition says Astra's item 7 was taken because "`start` prunes every clock once per commit time." The comment at `navigation.js:388` says the same. `register` only calls `start` for animations absent from `starts`:

```344:346:host/web/navigation.js
    register(t) {
      for (const a of document.getAnimations()) if (!starts.has(a)) { starts.set(a, clocks.start(a, t) ?? t); if (a.playState === 'paused') held.add(a); }
    },
```

`host/web-js/agent.js:154` is the same loop. A commit that only removes clocked animations never calls `start`, so `prune` never runs. Those targets stay in the strong member set for the rest of the page if that clock name is never used again, which is the case the earlier review named. A later commit that registers some new animation does prune first, so a following joiner's phase is usually right. The leak, and a same-timestamp replacement (the first finding), are what remain.

Call `prune` unconditionally at the start of both `register` functions, on the same per-commit counter as `sync`.

## What round 2 got right

These were the cases to re-check. They behave as D6 states.

`MotionSync::apply` (`kernel/src/motion.rs:84`) holds joins, applies every row, then joins, and the join runs on the error path too. `join_clocks` clears `held` before it looks at the map (`motion/src/engine/clock.rs:78`). A failure before the clock rows leaves that map empty. The only fallible step after a join is recorded is a later `set_animations`, which validates before it mutates and which kernel rows do not fail. Joining on that error is the useful outcome: a resumed play's placeholder start is `now` until the boundary is written, and skipping the join would leave that placeholder in place.

A node destroyed in the commit is in `removed` first, and `Engine::remove` forgets the clock and the joining entry before any join. `display: none` pushes an empty animation row; `set_animations` drops the plays before `join_clocks`, and the busy scan then ignores that node. The other joiner treats the clock as idle.

Clocks are applied before animations. `set_animation_clock(None)` and `forget_clock` both drop `joining`, and `join_clock` does nothing for a node that is no longer on a clock. A node cannot record a join and then leave the clock later in the same commit.

`set_animations` collects indices while walking the new list backwards, then reverses them, so they match the plays the row ends with. A second animation row for the same node, from `display_changed`, is built from the same post-commit `hidden()` and `style.animation`, so the indices still name those plays. `Joining::Node` (a clock change) ignores indices and reads whatever plays are present at join time, which is what `in_one_commit_a_join_reads_the_row_it_ends_with` asserts: a 1s cycle held across a retiming to 2s joins at 4.0, not 5.0.

A move and a resume in one commit leave `Joining::Node` in the map. `join_clock` does not turn that into play indices, so the resumed play is not treated as an old member of the new clock. A resume on the same clock stays `Plays { resumed }`, still counts as live, and keeps the origin. An ended play is skipped (`hold` is empty and `local >= end_time`). A paused-only move is `Node`, so those plays do not count as busy, an idle clock's origin becomes now, and the holds stay for the later resume.

Unbinding a drag timeline rewrites `start`, calls `join_clock_node`, and `join_clocks` calls `schedule_animations`, which puts the node back in `animating` and dirties the keyframe properties. Drag-bound nodes are excluded from the busy scan, including inside the commit.

On the web, a single `sync` does the right thing for an animation first seen paused (it is added, and `startTime` is left alone so it stays paused) and for one first seen with no clock (`clocked` is stored as `''`, so a later clock name is a change and joins).

Actions and mutations are in the shadow set beside the other view-scope names (`contract/syntax/src/clock.rs`), matching `component_scope`. Tasks and provides are outside that scope, so leaving them out is consistent.

## Taken items

Round 1's boundary snap, sole-member pause, move onto an idle clock, drag unbind, listed shadowing, hyphenated names, and the schema comment are in the tree and covered by tests that assert the corrected numbers. Round 2's same-commit cancellation, retiming, ended play, paused-only origin, and drag-bound membership are implemented as D6 describes. The native ordering gap round 1 left open is closed.

Two "taken" claims are short of what they say. Agent retention, above, still fails on a removal-only commit. The web clock-move fix is real when some other clocked element keeps `sync` in the loop, and the new test covers that. It does not cover the last clocked element leaving and coming back.

## Gaps that should not block

The three items LLP §5 already leaves open should land as documented:

- The agent clock records an author's pause once, at registration, so a later resume does not rejoin. That is main's behavior for every animation (LLP 1055 D10). The page clock goes through `sync` and does rejoin.
- One JS-agent seek can register a timer's joiner before older finite members are sought, so a member that ended inside the jump can still count as busy.
- `contract symbols` has no timeline definition or reference. That is navigation-only.

## Tests, by inspection

No tests were run. From the assertions and the code they drive:

- `in_one_commit_a_member_ended_elsewhere_is_gone`, `in_one_commit_a_join_reads_the_row_it_ends_with`, and `in_one_commit_a_drag_bound_play_is_no_member` fail if the corresponding join rule is reverted. They call `hold_clock_joins` / `join_clocks` on the engine directly. Dropping those two calls from `MotionSync::apply` would not fail them. The wiring itself is the five lines at `kernel/src/motion.rs:84`.
- `leaving_a_drag_timeline_onto_a_clock_takes_its_phase` asserts a painted frame after the unbind. That fails without `schedule_animations`. `!quiescent()` stays true anyway, because the other pulse is still running.
- The Chrome mover assertion fails without the `clocked` map. It does not cover a second sync at the same time, the last clock being removed, an animation first seen paused, or `register`.
- Nothing fails if the two new `.chain` calls that shadow actions and mutations are removed.

## Not verified

Cargo tests, the Chrome clock test, Firefox, a macOS or Linux app drive, and Core Animation across backgrounding. LLP §5 already lists the last four. The join arithmetic for the new engine tests was checked by hand against the asserted times, not by running them.

