# Code review: the black viewer: thumbnail under the picture, picture retries and priority, round 3, re-check 1 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `d26449b8b2`.
- **Method:** one brief (sha256 `968ac06ab23f0a01a4ea4bda7f2867be53a77724a9e8471afef8766e (first 58 of 64 hex digits)`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. **[P1] Untrusted Retry-After can crash the app** — [remote.rs:446](host/linux/src/image/remote.rs:446). `Retry-After: 18446744073709551615` parses successfully but overflows `Instant + Duration`. Release builds use `panic = "abort"`. The date parser also uses unchecked arithmetic on unrestricted integers. Use validated date fields and checked arithmetic; represent an unrepresentably distant deadline without panicking or retrying early.

2. **[P1] Queued loads can reach a replacement source** — [presenter/images.rs:59](host/linux/src/presenter/images.rs:59). Events carry only `ViewId`. If two images finish together and the first handler changes the second’s source, the second’s old completion invokes its current handler. Its eventual real completion fires again. Removal/reset also leave queued IDs intact. Queue node identity and source generation, validate immediately before dispatch, and invalidate pending events on replacement/removal/reset.

3. **[P1] Remounting bypasses Retry-After** — [workers.rs:34](host/linux/src/image/workers.rs:34). The deadline belongs only to `SourceOwner`; the backend holds weak references and caches only successful bodies. Removing the last view after a 503 with `Retry-After: 600`, then reopening it, creates a fresh download immediately. Retain and enforce the cooldown by URL independently of mounted views until its deadline expires.

4. **[P2] Cached loads can remain undelivered on Android’s GPU host** — [image.rs:693](host/linux/src/image.rs:693). A cache hit during `sync_images_moved()` queues an event, but that path only applies sizes. Android’s GPU loop calls `poll_images()` only when the image descriptor wakes; a cache hit produces no decoder completion to wake it. Schedule a presenter turn for queued load events and drain them independently of decoder activity.

5. **[P2] The five-minute cutoff does not last for the showing** — [image.rs:533](host/linux/src/image.rs:533). The cutoff compares against the current time on every poll. With `Retry-After: 600`, another poll after five minutes schedules a retry within the same uninterrupted showing, contrary to the stated policy. Record that retries are suspended for this showing and reconsider only upon a later showing.

**Non-blocking**

none.

Urgent FIFO is resolved. The thumbnail condition is correct after a valid load; native event delivery remains blocked above.

Static review only; no edits, network access, tests, or builds.
