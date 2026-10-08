# Code review: motion starts at the first frame (LLP 1003.001), round 2 (222dd14c8), 2026-10-07 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, a detached worktree at `222dd14c8`.
- **Method:** one brief (sha256 `70616010d7513b200a522754860b07dbaa8612dbe694fd5ad2a99ec4880512e1`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** DO NOT LAND.
- **Disposition:** 1 taken: with the rule on, `advanced` no longer copies the runner's clock into the host's, and `advance` takes its wall first; the cap is the host's clock (tested: a timer advance after a display frame, then a hold). 2 taken: the takeover samples at its own instant after forgetting presented frames (tested in the engine and the host). 3 taken: a member keeps its own birth for the frame test and takes the origin's phase when it starts (tested with a late frame). 4 taken: a cancelled curve and a removed node drop their pending keys. 5 taken in part as round 1; no new XCTest.

---

DO NOT LAND

1. **MATERIAL — An advance can still move input time past the wall.** After `frame_at(133,120)`, `advance(125)` receives the runner’s unchanged time, 133. [host.rs:929](host/apple/src/host.rs:929) copies that into `now_ms`, which becomes [engine_time’s cap](host/apple/src/first_frame.rs:91). A valid hold at 126 then fails `ClockWentBackwards`. The round-one clock disposition is incomplete. **Fix:** preserve the supplied wall instant through advance paths; test this sequence followed by drag input.

2. **MATERIAL — Takeover presents sampled keyframes at the old input time.** [first_frame.rs:35](motion/src/engine/first_frame.rs:35) starts pending plays at `at`, then clears `shown` without advancing `now`. The following presentation [samples animations using that old clock](motion/src/engine/animate.rs:288). With input time 150 ms and takeover at 200 ms, a newly started sampled play reads local −50 ms; without backwards fill it shows its underlying value. [Swift reads local zero](host/apple/Sources/ExactKit/SvgScene.swift:178). **Fix:** adopt the effective takeover instant and resample after clearing frame history, without firing runner timers. Assert the immediate sampled and lowered output.

3. **MATERIAL — Clock joins lose their actual birth time.** [clock.rs:137](motion/src/engine/clock.rs:137) assigns the pending origin’s timestamp to each joining play. [Frame eligibility](motion/src/engine/first_frame.rs:59) consequently tests the origin’s birth. With an origin pending since 0, a play added at 20 ms starts on a queued 16 ms frame and immediately samples 4 ms into its curve. D2 requires that newer play to remain pending. **Fix:** track each member’s birth separately from its phase anchor; test a join followed by an older frame.

4. **MINOR — Canceled pending keys still escape removal.** [observe’s cancellation](motion/src/engine.rs:475) removes `running` but leaves `pending`; [remove](motion/src/engine.rs:366) clears pending keys only through `running`. Observe `0→1→0`, then remove the node: its pending key survives while the engine is quiescent. **Fix:** clear pending entries on cancellation and remove a node’s pending range independently.

5. **MINOR — Integration coverage remains incomplete.** The [takeover test](host/apple/src/first_frame_tests.rs:174) checks a lowered start and a later advance, hiding finding 2. The [input test](host/apple/src/first_frame_tests.rs:108) has neither an actual frame task nor host drag input. [§5’s Swift cases](llp/1003.001-motion-starts-at-the-first-frame.rfc.md:431) remain absent. **Fix:** add those cases, prepare/takeover/commit, and completion tests for motion created during a tick.

No additional switch-off regression found by inspection. Formatting and whitespace checks pass; touched source files meet the cap. Compiled suites were not run: read-only checkout, no build artifacts.
