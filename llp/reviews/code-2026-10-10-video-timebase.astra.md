# Code review: VideoArm's position from the item's timebase (1a71f1cfb), 2026-10-10 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `61a0394ecfc7437f1bb92e3d97e7b90d869bb1f3c664995a7e0365a8fd062afb`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** NOT READY.
- **Disposition:** The MAJOR (the clocks differ after a scheduled start) is answered in the comment, not code: no `setRate(_:time:atHostTime:)` exists in the Apple hosts, which round 2 verified. The comment now says what the sample showed and that the timebase read takes locks too.

---

- **MAJOR — [host/apple/videoarm/VideoArm.swift:172](/Users/admin/x2p2/apps/exact2-video/host/apple/videoarm/VideoArm.swift:172): The two APIs are not universally equivalent.** A read-only macOS 27 probe using the repository’s `motion.mp4`, scheduling playback at media time **2.0 s** for **400 ms in the future**, reported `currentTime() == 2.0` while the timebase advanced from approximately **1.6 to 2.0**. This agrees with Apple’s documented [scheduled-start behavior](https://developer.apple.com/documentation/avfoundation/avplayer/setrate(_:time:athosttime:)). With no `seekTarget`, the new getter exposes that earlier position through snapshots, `timeupdate`, and potentially Now Playing; the finite-value check does not correct it. There is no explicit scheduled-start call in this repository, so this establishes an API mismatch, **not a reproduced AirPlay/fullscreen regression**. The unconditional replacement needs either a demonstrated restriction to equivalent states or preservation of the canonical position path for other states.

Other checks:

- **Local macOS behavior:** probes matched before readiness, immediately after replacement, while paused/playing, after successful paused/playing seeks, across rate changes, at both end-of-item modes, and after a loop-style restart. Removing the item preserved the previous normalized zero.
- **Events and zero fallback:** event scheduling and the seek-target override are unchanged. Any discrepancy changes reported values, not whether `seeked` fires. I found no zero-fallback regression in the exercised lifecycle states; nil timebase alone is not documented proof that the canonical position is zero.
- **Main-thread safety and blocking:** reading the public timebase is supported; the Swift expression introduces no apparent lifetime hazard. It is **not lock-free**: the inspected macOS implementation of `CMTimebaseGetTime` reaches `FigReentrantMutexLock`. This does not establish contention or negate a possible improvement over the old path.
- **Platform coverage:** fullscreen retains the same AVPlayer, but native-controller interactions, live streams, AirPlay/external playback, iOS and tvOS were not exercised. Their equivalence remains unverified.
- **Comment and QUEUE:** [QUEUE.md:3](/Users/admin/x2p2/apps/exact2-video/QUEUE.md:3) correctly labels the change unmeasured. The comment should describe the observed wait without implying universal timing equivalence or attributing the lock owner beyond what the sample established.

Repository unchanged.

**NOT READY**
