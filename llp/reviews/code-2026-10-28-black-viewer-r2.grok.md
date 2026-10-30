# Code review: the black viewer: thumbnail under the picture, picture retries and priority, round 2 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `b908420960`.
- **Method:** one brief (sha256 `a9fc9d2c628f2cb74321386de551a458b06b96e3843f9827da0f4611d101b150`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. `host/linux/src/image.rs:475` — Rejoin drops `source_id` but leaves `retry_at` armed for the failed source. A second on-screen view of the same URL often has a later deadline (it scrolled in during the other view's longer backoff, so its `RETRY_AFTER` slot differs). When the earlier timer fires it evicts and starts the shared retry; the later view then joins that download and keeps its old deadline. `retry_due_in` still sees that deadline (`source_id` is set again at line 496). When it comes due, line 468 evicts whatever that view holds now — the in-flight retry or a source that already became `Ready` — and `retries` increments. The other view then drops the cancelled download and joins the new one. One backoff can abort the attempt that would have landed, and the last of the six tries can be spent on that cancel. The off-screen test does not catch this: that view's `retry_at` was cleared while it was hidden, and it is shown only after the picture has already landed. Clear `retry_at` in the rejoin branch (do not increment `retries`). Evict only when `prepared` is still `Failed`; if the deadline is due and the source is already a replacement, clear `retry_at` and leave the source.

**Non-blocking**

1. `host/linux/src/presenter/images.rs:133` — If the picture has no painted box, `on_screen` returns true. A `display: none` image, or one not painted yet, is therefore retried and can be treated as urgent (up to the six-try cap). After a box exists, the geometric test takes over and an off-screen sync clears the timer, so this does not spin. Return false when the box is missing; the initial ask still goes through `visible`.
2. `apps/excord/viewer.contract:124` — The proxy stays under the full image after it lands. An opaque photo covers it; a translucent original keeps compositing over the proxy.
