# Code review: clock timelines, LLP 1055.002 (20431ac5c), 2026-10-04 (grok)

- **Family:** xAI. `grok-4.7`, reasoning effort xhigh, `--no-subagents`, `--sandbox read-only`, `--cwd` a detached worktree at `20431ac5c` (the branch rebased on `6274fbab3`).
- **Method:** one brief (sha256 `9344e5fd477541c4ed5409996184f189a14c0282b24aaaac5f9b08de96df2103`), shared with astra. Round 1, blind to the other review. The author (Claude) is not a reviewer. A first run in `--permission-mode plan` was cancelled when its shell calls were refused; this is the rerun.
- **Transcription:** the findings and verdict, unedited (the opening narration dropped).
- **Verdict:** SHIP WITH FIXES.
- **Disposition (r1 fixes):**
  - High (the sole member's pause or resume resets the origin): taken; astra's 2. A member that moves to another clock is let go too.
  - Medium (a node moved onto an idle timeline keeps its play): taken; the engine rejoins a node's running plays when its clock changes. `a_node_moved_onto_an_idle_timeline_starts_it_over`.
  - Medium (leaving a drag timeline onto a clock): taken; unbinding rejoins. `leaving_a_drag_timeline_onto_a_clock_takes_its_phase`.
  - Medium (a binding replaced): taken as shadowing rather than a diagnostic; astra's 8.
  - Low (schema.json's comment): taken.

---

**SHIP WITH FIXES.** The happy path matches the LLP: a lone animation starts at its first keyframe, a joiner uses the last cycle boundary (two iterations for `alternate` and `alternate-reverse`), a finite play’s end stays `start + delay + n × duration`, and `clock(...)` stays off real CSS properties. The web clock’s membership does not. Pausing or resuming the only animation on a timeline resets the origin, so that indicator does not rejoin and a later one does not share its phase.

## Findings

**High** — Pausing or resuming the only member resets the timeline origin. `host/web/navigation.js:389`

`start` drops the animation being updated, then treats an empty member set as idle and sets `origin = now`. `sync` calls `start` on every pause-state change, including the transition to paused (it only skips writing `startTime` in that case).

1. One 1s infinite pulse joins Pending at t = 0. Origin is 0 and `startTime` is 0.
2. It pauses at t = 0.4. It is removed before the idle check, so the origin becomes 0.4. Its `startTime` stays 0.
3. A second pulse joins at t = 0.9. The paused one still counts as live, so the origin stays 0.4 and the joiner’s `startTime` is 0.4. The paused pulse is still on 0. They are 400ms apart.
4. The first pulse resumes at t = 7.9 with nobody else live. `start` empties the set again, the origin becomes 7.9, and `startTime` is 7.9. It shows the first keyframe. D3 says a paused member keeps the timeline busy, and D6 says resume rejoins the phase already in progress (the engine test leaves the origin at 0 and resumes at 7.0).

`host/web/tests/clocks.test.mjs:69` pauses `engine` while `lock` is still running, so the set is never empty and the test does not reach this case.

Same function, second failure: a member that leaves the clock without a pause change is never re-homed. `sync` returns early when `paused` is unchanged (`navigation.js:401`), and the idle scan never checks `clockOf`. An animation whose `--exact-animation-clock` is removed or renamed stays in the old set and keeps that timeline busy.

Suggested fix: before the idle check, drop only other members that are disconnected, finished, or no longer on this clock. Keep the animation being updated in the set if it was already a member, and reset the origin only when no such member remains. On resume, set `startTime` from that origin. Call this path when the clock name changes, not only when play state flips.

**Medium** — A node that already has a live play does not take an idle timeline’s new origin. `motion/src/engine/clock.rs:67`

`clock_start` treats `also` (the node’s previous plays) as members of whatever clock the node is on now. `set_animations` always passes those plays, and `set_animation_clock` has already moved the node onto the new clock. Origins are never removed, so an idle timeline still has its old origin.

Pending was used, went idle, and its origin is still 0. A node has been running `spin` off that clock since t = 0.2. At t = 10.3 the commit puts it on Pending and replaces `spin` with `pulse 800ms infinite`. The old play is live, so the timeline stays “busy,” the origin stays 0, and the new play starts at 9.6 instead of 10.3. D3 says the next play on an idle timeline sets the origin to now and starts on its first keyframe.

If the animation name is unchanged, `animate.rs:101` keeps the old start and never calls `clock_start`. The play still counts as a live member. A joiner then aligns to the stale origin, which is not that play’s phase.

Suggested fix: count `also` only when this node was already on this clock. When the clock name changes, rejoin the current plays with `clock_start` instead of keeping their old starts.

**Medium** — Leaving a drag timeline onto a clock continues from the held time. `motion/src/engine/timeline.rs:91`

Clocks are applied before animations, and the drag binding is cleared after `set_animations`. In that call the node is still timeline-bound, so the play keeps the drag hold (`animate.rs:93`) and does not join the clock. Unbind then sets `start = now - held`. The play is on the clock and live, on its own phase. Later joiners align to the clock origin and do not match it.

Suggested fix: in the unbind arm, when `animation_clock` is set, set `start` with `clock_start` and clear the hold. Other nodes on that clock keep the origin; if there are none, the origin becomes now.

**Medium** — A timeline name silently replaces a same-named binding. `contract/syntax/src/clock.rs:50`

`resolve_clock_timelines` runs on the merged file and rewrites every element `animation-timeline=Name` whose name is any merged `timeline`. It does not look at state, parameters, or locals. After `use`, a library `timeline Pending` rewrites `animation-timeline=Pending` in every component, including one whose state `Pending` was the value (`"auto"`, a drag timeline name, or a computed string). The binding never reaches types.

Suggested fix: if that name is bound in the component, leave the ident and emit a diagnostic that the timeline and the binding need different names.

**Low** — `kernel/tables/schema.json:2891` still says `animation-timeline` is `auto | <dashed-ident>` and that those animations are never played on the clock. The codec comment is the declaration authority for that row; it should name `clock(<ident>)` and point at LLP 1001.

## What holds up

- Rebase pieces are present: `clocks.sync()` only on the non-agent branch of `applyBatch` (`host/web/glue.js:904`), `followOnSeek` still gates the agent seek, `agent.js` still imports `perf` and keeps `seek(to, sync)`, and `register` takes `clocks.start`. `rt.js` re-exports `animationClocks`. The new modules are in both module lists.
- Kernel `name()` is only a `--` ident, so a clock never enters drag lookup (`kernel/src/timeline/lookup.rs:75`). `css.rs` and the JS style maps write `--exact-animation-clock` and do not put `clock(...)` on `animation-timeline`. A `clock(` value is not timeline-bound, so an infinite animation on a clock is allowed (`contract/lower/src/svg.rs:790`).
- Two files’ `timeline Pending` are refused at `use` even when the text matches (`contract/cli/src/sources.rs:370`). Delay is applied from `start`, not folded into the period. The period is `duration` or `2 × duration`. There is no separate playback rate in `Animation`; direction is the only reverse control.

## What I could not verify

I did not run tests, browsers, or builds. Phase equality on Chromium, WebKit, the agent clock, and Lexy is what the LLP reports; I did not reproduce it. Firefox, the strict-lane fixture, macOS and Linux app runs, and Core Animation across backgrounding are still open, as the LLP says. I also did not measure float boundary drift after a timeline has been busy for a long time. Under the agent, `register` records a start once per `CSSAnimation` and seek keeps writing `currentTime`; I did not check a finite synced animation that is author-paused across a cycle boundary, where the engine holds and then rejoins.
