# Code review: VideoArm's position from the item's timebase round 2 (2e913b294), 2026-10-10 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `c9f8593a4510ae1698284f12d9e269a47e7fca4439c80c255092eff3ba5f1914`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** READY.
- **Disposition:** The NIT is taken: the comment says `currentTime()` holds the start position while the timebase advances to it.

---

- **NIT — [host/apple/videoarm/VideoArm.swift:171](/Users/admin/x2p2/apps/exact2-video/host/apple/videoarm/VideoArm.swift:171):** “runs ahead” reverses the scheduled-start offset. Scheduling item time 5 seconds two seconds into the future produced `currentTime() == 5` while the timebase advanced from 3 toward 5. Say “the timebase advances from an earlier time while `currentTime()` holds the requested start position.” This matches [Apple’s timing documentation](https://developer.apple.com/documentation/avfoundation/avplayer/setrate(_:time:athosttime:)?language=objc).

No BLOCKER or MAJOR findings. The round-1 scheduled-start objection does not justify a guard in this arm: no scheduled-start, custom-clock, or playback-coordination calls exist in the Apple hosts. Now Playing commands route through ordinary play/pause and app seek actions. Read-only macOS probes matched across playback, rate/pitch changes, seeks, end/loop transitions, and AVKit controls’ play/pause/scrubbing paths.

AirPlay remains a coverage limitation: external playback is enabled, but physical route transitions and iOS `AVPlayerViewController` interactions were not exercised here. I found no concrete additional divergence, rather than proving equivalence across those configurations.

The revised lock wording matches the original sample’s 124 main-thread samples waiting inside `AVPlayerItem.currentTime`. [QUEUE.md:3](/Users/admin/x2p2/apps/exact2-video/QUEUE.md:3) appropriately leaves the workload benefit unmeasured and requests representative remeasurement.

READY
