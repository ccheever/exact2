# Code review: NativeTaskTimersTests load flake (retry ask waited for), round 1 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `e1ca00033d`.
- **Method:** one brief (sha256 `c4b496a20174bd989626bfc64c57545c36888bcdaef1281613dd4a933cbfc4fd`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. `host/apple/tests/ExactKitTests/NativeTaskTimersTests.swift:206` — Under the load this commit describes, a frame-later ask was still absent after a 0.1 s `RunLoop.run(until:)`. Line 206 is that same call for 0.05 s, and line 205 has already returned on the first sample with `asked >= 2`. A follow-up `resume(after: 1.0 / 60)` armed from the ask (`notification` is cleared before `ready()` in `NativeTaskTimers.resume`) is due about a frame after that sample. With the same lateness, past 0.1 s, it is still queued when line 207 requires `asked == 2`, so the assertion passes with another ask in flight. A single `run(until:)` also returns when one source fires, so a loaded main loop can end the window before that frame elapses. Replace `turns(0.05)` with a pump that keeps turning until a deadline later than the miss already seen: `spin(until: { asked > 2 }, for: 0.5)` (leave the message empty so a quiet window is success), then `XCTAssertEqual(asked, 2, "the retry asks for itself, once")`. A second ask inside the window fails the equal. Half a second covers one frame plus the lateness that already beat 0.1 s.

**Non-blocking**

1. `host/apple/tests/ExactKitTests/NativeTaskTimersTests.swift:204` — "not at once" still means the retry does not call `ready()` on the `complete(retry:)` stack: nothing turns the run loop between `complete` and this assert, so a synchronous ask fails it. A delay of 0 (`DispatchQueue.main.async`) also leaves `asked` at 1 until the next turn, and line 205 accepts that ask on the next 1 ms slice, any time up to 5 s. Nothing requires the gap to be about `1/60`. To pin "a frame later", after line 204 drain blocks already on the main queue with a few `RunLoop.main.run(mode: .default, before: Date())` and assert `asked` is still 1. A delay-0 ask runs on that turn; `asyncAfter(1/60)` stays pending. That check does not depend on the ask arriving on time.
