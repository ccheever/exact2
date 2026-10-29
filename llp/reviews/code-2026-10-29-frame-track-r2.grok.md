# Code review: the Apple agent's bounded scroll track, round 2 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `f2a3c445d0`.
- **Method:** one brief (sha256 `49b39f05c82473813027b6510ec1ad02ee94d786ee17988c1a69a228e611562b`), shared by both reviewers, blind to the other review. The author (Claude, x2-perf4's workers) is not a reviewer.
- **Transcription:** the final message, unedited.

Round-1 items are fixed and covered by tests. The carrier skips `ExactSession.drainAll()` for a string `track` and `testCarrierTrackRequestsDoNotDrainTheOwner` holds the owner with a tick in flight. A cursorless read starts at row 0 and counts `dropped` (904 in the ring test); the driver keeps a per-session cursor and resets it on a successful start. Stop joins one tail page, and a mid-read `no-track` is returned with `rowsBeforeEnd`. `renderPerf` renders start, read, stop, refusal, and an ended lease. Node ids go through `UInt32(exactly:)`, with the negative, 2^32, non-integer, and string cases refused on iOS without replacing the track. `destroy()` clears the track, and a feed after teardown appends nothing. Centipoints clamp at both signs. The lease ends on the next feed, and reset, clock change, and teardown drop it. Mac phase comes from the wheel handler (`began`/`changed` only, then cleared), and the resize-at-rest case is tested after a wheel has already written a row.

No new scheduler, timer, or observer. One 4096-row ring. Pages stay inside 4000 bytes with the newline. Start is gated on `enabled && !virtual`, same as capture.

VERDICT: CHANGES

1. **MAJOR** — `host/apple/Sources/ExactKit/FrameHealthTrack.swift:150` — Mac axis selection treats “sideways” as “document wider than the clip and not taller,” and ignores `ChainingScrollView.scrollsX` / `scrollsY`. A horizontal scroller (`overflow_x: scroll`, `overflow_y` left visible) whose document is taller than the clip is read on Y. That happens when the content is taller than the viewport, and also when a legacy scroller shrinks the clip below a document sized to the frame. Y does not move, the 0.001 filter then drops later events, and the horizontal scroll is missing. `testAMacSidewaysScrollerIsReadAlongX` uses equal content and frame height, so it stays green under overlay scrollers. Prefer the overflow flags when only one axis scrolls.

2. **MINOR** — `host/apple/Sources/ExactKit/FrameHealthTrack.swift:152` — The resize filter writes whenever `last` is nil. The origin is not sampled at start, so the first bounds change after `start` is stored even when it did not move (a resize or `fitScroll` at rest). The Mac test only resizes after a wheel has set `last`, so that first event is untested. Sample the clip origin into `last` when the track starts, and write only when it changes.
