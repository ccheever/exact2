# Code review: VideoArm's position from the item's timebase (1a71f1cfb), 2026-10-10 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `61a0394ecfc7437f1bb92e3d97e7b90d869bb1f3c664995a7e0365a8fd062afb`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** READY.
- **Disposition:** Nothing to take.

---

No findings. **READY**

`seconds` in `host/apple/videoarm/VideoArm.swift:170` still returns `seekTarget` while a seek is in flight. Otherwise it reads `CMTimebaseGetTime` on `currentItem.timebase`, or `0` when that timebase is missing or the result is not finite. On this Mac that time matches `-[AVPlayer currentTime]` in every state exercised below. A seek still in flight leaves both clocks at the pre-seek time, so the `seekTarget` override is still what makes the report jump.

The public `timebase` is the proxy folded timebase (`_copyProxyFoldedTimebase`), not the internal folded one. `-[AVPlayerItem currentTime]` copies that internal timebase and reads it after `_copyFigPlaybackItem`. The proxy’s time still matched, including with an `AVPlayerLayer` attached.

Measured with a 6s AIFF and a 3s H.264 clip (30 fps, audio), local files only:

| State | Result |
|---|---|
| No item | `currentTime` is invalid (`NaN` → old code returned `0`). Timebase is nil → `0`. |
| `replaceCurrentItem`, status still unknown, and again once ready | Both `0`. The timebase already exists. |
| Playing, paused, and still paused 0.3s later | Paused values are identical and do not drift. While playing, the gap is a few microseconds, the time between the two calls. |
| `setRate` 2 and 0.5 | Same, within one read of each other. The timebase rate follows the player rate. |
| Zero-tolerance seek, in the completion handler | Both land on the target (`2.25` and `1.0`). |
| End, `actionAtItemEnd = .pause` | Both equal the duration (`6` and `3`) in the end notification and after it settles. |
| End, `actionAtItemEnd = .none` (VideoArm’s loop) | Both run past the duration together (`6.69` and `3.51`). |
| Failed URL | Both `0`, and the timebase is non-nil. |

`timeupdate`, `seeked`, Now Playing elapsed time (`sessionElapsed` at `VideoArm.swift:234`, published only on the events in `moves`), and `state.currentTime` all go through `seconds`. The nil-timebase `0` is the no-item case, which the old `isFinite` check already reported as `0`. An attached item, including one not ready and one whose file fails, already has a timebase at `0`.

`timebase` is `NS_SWIFT_NONISOLATED`, and `CMTimebaseGetTime` is the supported read from any thread, including main. It is not a pure anchor load: it tail-calls `figTimebaseGetTime_MaybeUpdatingAnchorTimeFromLoopiness`, which can enter Fig on the loopiness path. The getter also takes the observation registrar (`accessWithKey:on:`) and copies the proxy. With 12 local players playing, that path was faster than `currentTime` (about 0.007 ms average, 0.048 ms max, versus 0.030 ms and 1.3 ms). It did not show the MediaToolbox wait from the sample.

The comment at `VideoArm.swift:165` matches the old stack: `send` → `snapshot` → `seconds` → `-[AVPlayer currentTime]` → `-[AVPlayerItem currentTime]`, which does call into Fig. “Not yet timed is at zero” matches the nil-timebase result. `QUEUE.md:3` is right that this shipped unmeasured and that a local repro did not show the stall. The 124 ms and 12 ms figures are that entry’s report; nothing in the tree contradicts them. Live streams, AirPlay, and tvOS were not run. Full screen uses this same `AVPlayer` (`VideoArm.swift:666`), so it has this same timebase. The re-measure in the queue is the right check on whether the Fig path inside `CMTimebaseGetTime` can still stall under a real autoplay fling.

READY
