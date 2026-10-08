# Code review: motion starts at the first frame (LLP 1003.001), round 1 (34795f7d1), 2026-10-07 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, a detached worktree at `34795f7d1`.
- **Method:** one brief (sha256 `5cd508737c99c761c25a6f8c20d7a1ae6b4a8738583a91703849bdf02dc490e9`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** DO NOT LAND.
- **Disposition:** 1 taken: the tick starts its frame right after its seek, before a reorder's end or height layout reads the engine. 2 and 3 taken: no persistent cap; after a display frame the host's clock goes back to the wall and caps the engine's input clock (with the rule on), so the scheme, reorder, data and stepped-advance paths all hear wall times, and the takeover leaves nothing behind. 4 taken: the takeover starts what waits at its instant (or a later begin) and the engine forgets presented frames, so it samples on the agent's clock as Swift does. 5 taken: the render instant is the scene's (`SvgScene.offscreen`), not a global. 6 taken: the engine starts every curve at the takeover. 7 taken: the spring exports report a pending start. 8 taken in part: tests for boot's latch through the bridge, input behind a display frame with a real hold, a takeover before a begin, an exit's destroy; no new XCTest (the Swift changes are covered by the iOS XCTest run and the recordings).

---

DO NOT LAND

1. **MATERIAL — Frame sampling happens after layout and cleanup.** [first_frame.rs:18](host/apple/src/first_frame.rs:18) checks reorder completion and computes height layout before [presence.rs:317](host/apple/src/presence.rs:317) samples the target frame. If the last curve ends between wall and target, that sample retires it and reports no motion, stopping the link with stale height geometry or an unfinished reorder. **Fix:** sample the frame before these consumers; retain the presentation hook for curves born later in the tick. Test the final tick without other activity.

2. **MATERIAL — Clock writers bypass the input cap.** After `frame_at(133, 120)`, [paint.rs:80](host/apple/src/paint.rs:80) passes 133 to [PaintMotion’s engine advance](kernel/src/motion/paint.rs:213). [arrange_now](host/apple/src/arrange.rs:198) similarly promotes a reorder input to the runner’s future time. Subsequent valid input at 126 can be refused. **Fix:** route appearance updates through `engine_time`; preserve the actual input instant throughout reorder holds and releases.

3. **MATERIAL — A stale cap corrupts subsequent timing, including switch-off timing.** [set_start_on_frame](host/apple/src/first_frame.rs:83) leaves `input_cap` intact, [engine_time](host/apple/src/first_frame.rs:96) still applies it, and [advance_until_request](host/apple/src/host.rs:903) never refreshes it. After frame `(133,120)` and takeover at 200, a stepped advance’s timer at 400 can start its curve at 200. [fulfill_all](host/apple/src/host.rs:771) also omits the wall update, allowing later data receipts to capture stale animation phases. **Fix:** clear the cap on takeover and refresh it at every wall-bearing receipt entry, including data completion and collection feedback.

4. **MATERIAL — Rust and Swift disagree on the takeover instant.** [Rust floors takeover to `sample_time`](host/apple/src/first_frame.rs:84), while [Session.clock](host/apple/Sources/ExactKit/Session.swift:398) retains the earlier agent time. With `shown=133` and takeover at 125, a newly started lowered spec has `s=133`; [CssAnimations.make](host/apple/Sources/ExactKit/SvgScene.swift:181) computes negative local time and can omit the animation, while Rust presents its start. **Fix:** return and adopt one effective takeover instant before applying or seeking either executor.

5. **MATERIAL — The render timestamp is shared mutable state across renderers.** [SvgFilterLive.swift:290](host/apple/Sources/ExactKit/SvgFilterLive.swift:290) writes global `CssAnimations.renderTime`, but [text draws run on main and other draws on a worker](host/apple/Sources/ExactKit/SvgFilterLive.swift:187). Overlapping renders can place held animations against another render’s timestamp; nested renders also clear the outer timestamp. This affects authored pauses and agent sessions with the switch off. **Fix:** pass the instant through a per-render context to animation construction.

6. **MINOR — The engine switch can strand pending curves.** [Engine::set_start_on_frame](motion/src/engine/first_frame.rs:29) accepts an instant before a pending curve’s begin, leaves that curve pending, then disables the rule. Ordinary advances never start it. **Fix:** enforce D5’s takeover floor inside the engine API, rather than only in Apple’s wrapper.

7. **MINOR — D1’s spring exports remain unimplemented.** [spring_descriptor](motion/src/engine.rs:627) and [spring_frames](motion/src/engine.rs:643) export the original start while pending. **Fix:** apply the specified `sample_time − begin` adjustment and test exports after a pending advance.

8. **MINOR — Tests miss the integration boundaries.** [The fixture enables the switch after boot](host/apple/src/first_frame_tests.rs:34); the “frame task” test has no frame task, and [its hold assertion](host/apple/src/first_frame_tests.rs:122) accepts `Ok(None)`. No Swift tests were added. **Fix:** cover preboot/prepare/commit, actual drag admission, takeover behind `shown`, delayed async ticks, and held filtered-SVG pixels as specified in §5.

Formatting passed; reviewed source files meet the 1,500-line cap. Compiled suites were not run: the checkout is read-only and has no build artifacts.
