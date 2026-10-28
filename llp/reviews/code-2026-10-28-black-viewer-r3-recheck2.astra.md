# Code review: the black viewer: thumbnail under the picture, picture retries and priority, round 3, re-check 2 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `0b1913511b`.
- **Method:** one brief (sha256 `fa65a1f132dc9e0bf77d370eb33d92da71aa3d72c84d532cd4aa32e64e3db36d`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. **A timer can still invalidate a load after validation** — [presenter/images.rs:51](host/linux/src/presenter/images.rs:51). Android can defer timer advancement at [android.rs:563](host/linux/src/android.rs:563), then call `tick(now)` before `poll_images()`. `tick` advances `host.now()` without running runner timers. Consequently, `dispatch_at(..., host.now())` can run an overdue timer after the identity/source guard, change the image’s source, and deliver the old load to its new handler binding. Advance timers before validation and use a dispatch that cannot advance them again, or validate inside dispatch after timer advancement. Add coverage for this deferred-timer path.

2. **Changing a source A→B→A during delivery can produce duplicate loads** — [presenter/images.rs:36](host/linux/src/presenter/images.rs:36). `mem::take` removes the whole batch from the queue that source changes invalidate. With three queued loads, the first callback can change the third image A→B, and the second restore B→A. The old third event then passes the key/source comparison, while accepting cached A queues another event for the new assignment. Both fire. Track a source-assignment generation in each queued event and validate it, or keep undelivered events accessible to invalidation. Extend the existing callback-order test with this sequence.

3. **Canvas can repeatedly schedule immediate turns without delivering the load** — [presenter/delivery.rs:55](host/linux/src/presenter/delivery.rs:55). Pending loads now force a zero-delay turn, but [canvas.rs:1221](host/linux/src/canvas.rs:1221) skips `poll_images()` while `scrolled` is true. A timer can assign a cached source during such a frame. If that frame paints fully, it clears `moved` without clearing `scrolled`; with no pending refinement or leading scroll, `Native.owed()` is false. The [Canvas timer callback](host/android/canvas/src/main/java/dev/exact/canvas/Owner.kt:438) then repeatedly produces quick frames without polling images or refining. The load remains pending and the loop spins until another wake intervenes. Service due image work during quick frames, and cover this path without an image-fd wake.

**Non-blocking**

none.

The overflow protection, sequential remount cooldown, and per-showing retry cutoff look resolved. No separate Excord thumbnail defect found.

Static review only; no network, tests, builds, or file changes.
