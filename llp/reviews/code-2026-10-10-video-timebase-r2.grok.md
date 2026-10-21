# Code review: VideoArm's position from the item's timebase round 2 (2e913b294), 2026-10-10 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `c9f8593a4510ae1698284f12d9e269a47e7fca4439c80c255092eff3ba5f1914`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** READY.
- **Disposition:** The NIT is taken (the same as Astra's).

---

**NIT** — The scheduled-start sentence has the clocks backwards. `host/apple/videoarm/VideoArm.swift:171`

`setRate(_:time:atHostTime:)` with a future host time holds `currentTime()` at the scheduled item time while the timebase runs up to it from earlier. On a local file here, scheduling item time 1.0 s at host+0.8 s left `currentTime()` at 1.0 while `CMTimebaseGetTime` read 0.40, then 0.60, then 0.80. Scheduling item time 0 left `currentTime()` at 0 and the timebase negative (−0.60, then −0.40, then −0.20) until they met. That is the same split `AVPlayer.h` describes: the timebase starts from an earlier time, and the item time sits at `itemTime` until the timebase reaches it. The comment says the timebase runs ahead of `currentTime()`.

No code guard is required. The split is that call, and nothing in the Apple hosts makes it. The only mention of `setRate(_:time:atHostTime:)` in the tree is this comment. The arm starts and stops with `play()`, `pause()`, and `rate` (`VideoArm.swift:339`, `VideoArm.swift:442`, `VideoArm.swift:447`). Now Playing play/pause go through those (`NowPlaying.swift:203-205` → `sessionPlay` / `sessionPause`). The six transport actions are events to the app (`VideoArm.swift:248`); a seek that comes back still returns `seekTarget` (`VideoArm.swift:174`, `VideoArm.swift:450-454`). `AVPlayerView` / `AVPlayerViewController` expose play, pause, seek, and `defaultRate` / `selectSpeed`, not a scheduled start, and `setRate:time:atHostTime:` raises when `automaticallyWaitsToMinimizeStalling` is YES, which this arm sets by default (`VideoArm.swift:323`). AirPlay is the player's external-playback mode on that same item. It does not add a second call.

Those paths agree. With stall-waiting left on, `play()` in `.waitingToPlayAtSpecifiedRate`, playback at 1× and at 1.5× with the arm's default spectral pitch, a paused seek, and the end of the item, the two reads stayed within a single read of each other (about 16 µs). A guard on `.waitingToPlayAtSpecifiedRate` would miss the real split: during the scheduled start above, `timeControlStatus` was already `.playing`. It would also send the stall-wait, which is the common case, back through `-[AVPlayerItem currentTime]`.

The rest of the comment matches the sample and the code. It says the 2026-10-09 Bluesky sample showed main waiting on a lock inside `-[AVPlayerItem currentTime]`, that the timebase read takes locks too, and that the shorter path is unmeasured. No item still reports 0: `currentTime()` is invalid then, and the old `isFinite` fallback was 0 as well. `QUEUE.md:3` matches that record (the clone's diary describes that Discover fling as `AVPlayerItem.currentTime` under a MediaToolbox lock; `tools/perf/measure.py --feed-fixture videos` is the 48-video fixture) and still asks for the re-measure.

READY
