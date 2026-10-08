# Code review: motion starts at the first frame (LLP 1003.001), round 3 (526346b5e), 2026-10-07 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, a detached worktree at `526346b5e`.
- **Method:** one brief (sha256 `ad2a6ec033f586fc18bc55055c8dde703dc516e44d5e8e5c6aeeaac753a5aa9f`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer. The third and last round under AGENTS.md's limit; its fixes are not reviewed again.
- **Transcription:** the final message, unedited.
- **Verdict:** DO NOT LAND.
- **Disposition:** 1 taken: the takeover moves the host's clock (a prepared host's too) to the agent's (tested with an appearance change right after). 2 taken: the node keeps its place with the waiting origin while any of its plays waits (tested with two plays of one node around a late frame); an exit appended to a waiting clock member is not distinguished (noted). 3 taken: `advance` takes its wall first only with the rule on. 4 taken: the takeover runs a tick at its instant (height layout and a reorder's end settle; no timer fires); not tested. 5 not taken beyond rounds 1 and 2: no new XCTest. 6 taken: removal drops the node's pending keys by range.

---

DO NOT LAND

1. **MATERIAL — Takeover leaves the host clock behind the engine.** [first_frame.rs:73](host/apple/src/first_frame.rs:73) advances the engine to the takeover instant without updating `self.now_ms`. After input at 100 ms and takeover at 200 ms, `set_scheme` still [passes 100 ms](host/apple/src/paint.rs:80), triggering [the backwards-clock assertion](kernel/src/motion/paint.rs:213). **Fix:** update the host clock at takeover, including prepared hosts; test an immediate appearance change.

2. **MATERIAL — Pending clock membership is consumed per node, not per play.** [clock.rs:220](motion/src/engine/clock.rs:220) removes the phase marker when [any eligible play starts](motion/src/engine/first_frame.rs:97). Start play A at 0, append B on the same node at 20 ms, then deliver frames 16 and 33 ms: A starts at 16, but B starts at 33 instead of sharing A’s phase. The [new regression test](motion/tests/it/first_frame.rs:296) uses separate nodes and misses this. **Fix:** track pending clock enrollment per matched play, preserving it until that play starts; test staggered plays on one node and an appended exit.

3. **MATERIAL — `advance` changes switch-off refusal timing.** [host.rs:895](host/apple/src/host.rs:895) now stores the requested time before the runner advances. If `advance(1000)` encounters a refusing timer at 100 ms, the runner reports 100, but [the final engine seek](host/apple/src/host.rs:1288) reaches 1000. Motion can finish prematurely and subsequent valid holds can fail. **Fix:** restrict the early wall-clock update to switch-on operation; preserve the runner’s landed time with the switch off. Add a refusing-timer regression.

4. **MATERIAL — Bare takeover skips height layout and reorder completion.** [start_on_frame](host/apple/src/first_frame.rs:64) resamples, then calls only `present`; [presentation discards Height samples](host/apple/src/presence.rs:338). A running height can finish during takeover while its geometry remains at the previous frame. Reorder settlement is likewise bypassed. **Fix:** perform the tick’s layout and completion work before returning the takeover batch, without firing timers; test takeover across a height/reorder endpoint.

5. **MINOR — Promised integration coverage remains absent.** The [input-clock test](host/apple/src/first_frame_tests.rs:108) has no frame task and acquires a hold directly through the engine. The [RFC’s Swift cases](llp/1003.001-motion-starts-at-the-first-frame.rfc.md:434) remain unimplemented. **Fix:** add actual host drag admission, in-tick births, prepared-host takeover, delayed-tick agreement and held-filter pixel tests.

6. **MINOR — Node removal introduces quadratic pending cleanup.** [engine.rs:375](motion/src/engine.rs:375) scans every pending key for each removed node. Removing N pending animated rows now costs O(N²). **Fix:** remove that node’s pending range independently, following the adjacent running-key cleanup.

Formatting and whitespace checks pass; touched sources meet the line cap. Compiled suites were not run: read-only checkout, no build artifacts.
