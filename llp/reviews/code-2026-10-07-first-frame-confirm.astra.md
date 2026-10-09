# Confirmation pass: motion starts at the first frame (LLP 1003.001), round-3 findings at e200397ec, 2026-10-07 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, a detached worktree at `e200397ec`.
- **Method:** a scoped brief (sha256 `360bcf80d660bfa8ae898d7e810493a3862ce1e4c3aaba497380c3d68be4f277`), at the lead's request: are round 3's findings resolved, and did their fixes break anything. Astra only; not an open review.
- **Transcription:** the final message, unedited.
- **Verdict:** NOT CONFIRMED.
- **Disposition:** the regression taken in `0f3ccfe39`: the clock's wait is per play (`AnimationPlay::clock_wait`), set only by a join to a waiting origin, so an appended exit starts at its own frame (tested). Finding 5 taken: host-level hold admission behind a frame, a flight landing, a curve a tick begins, the takeover settling a height, a prepared host taken over before its commit, and a Swift test of a held animation's offscreen placement. Not added: a Swift test of a tick in flight across dropped callbacks.

---

NOT CONFIRMED

1. **RESOLVED** — Takeover updates the host clock, including prepared hosts: [first_frame.rs:79](host/apple/src/first_frame.rs:79), [abi.rs:1363](host/apple/src/abi.rs:1363).
2. **PARTLY** — Staggered plays retain membership, but enrollment remains per node; pending exits also inherit it: [first_frame.rs:119](motion/src/engine/first_frame.rs:119). See regression below.
3. **RESOLVED** — Early clock assignment is conditional on the rule; off-mode retains the runner’s landed time: [host.rs:889](host/apple/src/host.rs:889), [host.rs:930](host/apple/src/host.rs:930).
4. **RESOLVED** — Takeover now invokes `tick`, including reorder settlement and height layout: [first_frame.rs:70](host/apple/src/first_frame.rs:70), [first_frame.rs:20](host/apple/src/first_frame.rs:20).
5. **NOT** — The input test still acquires a hold directly through the engine; the promised integration cases remain absent: [first_frame_tests.rs:123](host/apple/src/first_frame_tests.rs:123), [RFC:413](llp/1003.001-motion-starts-at-the-first-frame.rfc.md:413).
6. **RESOLVED** — Removal visits only that node’s pending-key range: [engine.rs:375](motion/src/engine.rs:375).

**MATERIAL — New rule-on exit regression from `d1b0176eb`, within finding 2.** A clock member begins at 0 ms; append a 300 ms exit at 20 ms; deliver frames 16 and 33 ms. The new reenrollment preserves the node’s origin for the exit, making its start **16 ms instead of 33 ms** and its end **316 instead of 333 ms**. Exits never join the clock ([animate.rs:313](motion/src/engine/animate.rs:313)), but receive its origin at [first_frame.rs:109](motion/src/engine/first_frame.rs:109). **Fix:** preserve enrollment per matched play, exclude appended exits, and test this sequence.

No additional rule-off regression identified. `cbd06e5ac` correctly supplies `false` to the test boot: [control_text_tests.rs:92](host/apple/src/control_text_tests.rs:92). Static inspection only; compiled suites were not run—read-only checkout, no build artifacts.
