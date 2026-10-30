# Code review: the Apple agent's bounded scroll track, round 2 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `f2a3c445d0`.
- **Method:** one brief (sha256 `49b39f05c82473813027b6510ec1ad02ee94d786ee17988c1a69a228e611562b`), shared by both reviewers, blind to the other review. The author (Claude, x2-perf4's workers) is not a reviewer.
- **Transcription:** the final message, unedited.

Reviewed the full range and fixups. The two TRACK driver tests pass. Swift regression tests were inspected but not rerun: this read-only checkout has no build artifacts.

The dispatcher exemption, row-0/drop accounting, stop-tail merge, mid-read `no-track`, rendering, ID bounds, teardown, clamping, expiry, reset/clock change, and Mac axis fixes have coverage. Remaining issues:

VERDICT: CHANGES

1. **MAJOR — Cursors leak between embedded sessions.** [scripts/agent-inspect.mjs:469](scripts/agent-inspect.mjs:469) keys the cursor only by driver object `s`, although `s.session` can change. Reproduced: start A and B, read A’s five rows, then read B’s two rows; B receives cursor `"5"` and returns nothing with zero dropped. Keep cursors by session label within each driver, including start/stop cleanup, and test switching sessions.

2. **MAJOR — Driver start still drains the owner.** [scripts/agent-inspect.mjs:517](scripts/agent-inspect.mjs:517) calls `s.target()`, which always sends `tree`, even for numeric IDs. That request reaches `ExactSession.drainAll()` before the exempt TRACK request. Confirmed the request sequence is `tree` → TRACK start. Starting or replacing a track mid-glide can therefore stall it. Provide a numeric-ID path without tree resolution and cover the public driver path with the owner held.

3. **MINOR — The first resize at rest still records a row.** [FrameHealthTrack.swift:152](host/apple/Sources/ExactKit/FrameHealthTrack.swift:152) accepts any first callback because `last` starts nil. Start tracking a stationary scroller, then resize without moving its origin: the filter records a false scroll sample. Initialize the baseline at start; test resizing before any wheel movement. The existing test resizes after six wheels.

4. **MINOR — Mac phase attribution lacks regression coverage.** [FrameHealthTrackTests.swift:319](host/apple/tests/ExactKitTests/FrameHealthTrackTests.swift:319) checks only plain-wheel `"-"`. Removing the new phase assignment would still pass. Add handler-level coverage for dragging and momentum began/changed events, ended events, and subsequent layout movement, asserting `t`, `d`, and `-` respectively.
