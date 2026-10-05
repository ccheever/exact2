# Code review: a spring flight lands as UIKit's spring animators finish, 2026-10-05 (grok)

- **Family:** xAI. `grok-4.7` via `~/.grok/bin/grok`, reasoning effort xhigh, `--no-subagents`, `--cwd /tmp/x20`, plain output.
- **Method:** one brief (sha256 `257c3c154bc65b38924bc209356b4909cb9a85eacdf48e170237ed865e9cf61d`), shared with astra. Round 1, blind to the other review. Reviewed the uncommitted diff in a worktree at origin/main. The author (Claude) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** READY.
- **Disposition (r2):** P2 taken: `land_after` is now the first grid time from which the unit spring stays within the bounds (scanning back from the 10 s cap; `None` if it is not settled there), so a slow crossing is not its end; a regression seeks `spring(1, 1, 1)` past its crossing near 6 s and checks it has not landed. P3 taken: `cargo fmt`. DEFERRED: `clock settle` still asks the engine's original rest time (it may advance farther than a flight needs).

---

**READY.** No findings.

The spring flight lands on the unit curve from rest, and that is the curve the engine actually runs. `start_flights` drops any flight of the arriver, then observes `0` and `END` with no velocity, so the new spring is released from displacement 1000 at rest (`host/apple/src/flights.rs:84-98`, `motion/src/engine.rs:410-436`). A spring is linear, so the time to a fractional band matches `land_after`'s sample from displacement 1. An interrupt does the same thing: `end_flight_of` removes the node before that pair of observes, and the presenter captures the current rect as the new flight's start (`host/apple/Sources/ExactKit/IOS/FlightsIOS.swift:70-87`). Inherited velocity never enters the estimate.

A slow underdamped crossing can meet both thresholds while the envelope is still larger than `1e-3`. Future displacement stays within `hypot(x, (v + αx) / ωd)`. With `|v| < 0.05`, that leftover is about `0.05 / ωd` of the travel. For UI springs it is a few thousandths (the photo spring, slightly under critical, peaks at about `10⁻³¹`). A fast crossing stays above `LAND_SPEED` while it is inside the band, so the speed test still rejects it. The photo spring's first grid hit is `t = 0.3708 s`, with displacement `9.2e-4` and speed `0.021`.

`engine.now()` is the right clock. `start_flights` runs after `advance` to `max(receipt time, engine.now())` (`host/apple/src/host.rs:1242-1262`). `observe` starts the spring at that same `now + delay` (`motion/src/transition.rs:275-286`). `lands_at` adds the same delay. The agent seeks this engine clock (`Agent.swift` `advance` → `engine.advance`); `land_flights` compares against `engine.now()` after that seek (`flights.rs:136-138`). A seek that passes `lands_at` lands in that batch. A seek that stops short keeps the flight up.

Negative delay does not apply. A spring with `delay < 0` is rejected in `Transition::validate` (`motion/src/transition.rs:136-138`), and `set_layout_transition` validates before the flight is armed. `.max(0.0)` does not change a spring that got that far. Easings leave `lands_at` as `None` and still end at their duration.

`land` puts the view back in its slot (`FlightsIOS.swift:114-131`). At the land instant the remaining travel is under `1e-3` by the predicate (`9.2e-4` for this spring). One 60 Hz frame earlier it is about `1.3e-3`. `end_flight_of` still removes the node and does not send `land` (`flights.rs:111-119`). Destroy runs that before `present`, so a destroyed arriver still does not land even if `lands_at` has passed (`host.rs:1192`).

The test matches the curve. The corpus has one `layout-transition`, on `Photo`, which is the leaver's curve when the viewer opens (`kernel/src/handoff.rs:92`). Dispatch at `100 ms`, then `tick(440)` is `0.34 s` of spring time and `tick(520)` is `0.42 s`. The closed form at `0.34 s` is progress `0.9981`, inside `0.99..0.9995`, and the grid land `0.3708 s` falls in the open interval the asserts bracket. I checked that analytically and did not execute the test.
