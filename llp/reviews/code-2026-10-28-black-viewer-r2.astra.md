# Code review: the black viewer: thumbnail under the picture, picture retries and priority, round 2 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `b908420960`.
- **Method:** one brief (sha256 `a9fc9d2c628f2cb74321386de551a458b06b96e3843f9827da0f4611d101b150`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. [host/linux/src/image.rs:475](host/linux/src/image.rs:475) — **Rejoining retains the previous source’s retry deadline.** If two on-screen views have staggered deadlines, the first starts a replacement download and the second joins it without clearing `retry_at`. When that old deadline arrives, lines 468–474 evict the replacement—even while it is downloading or already successful. Other views then abandon it too. Check for a missing source before processing its deadline, clear `retry_at` on rejoin without counting a retry, and restrict eviction to the failed source. Add coverage with both views on screen, staggered deadlines, and a slow successful replacement.

2. [host/linux/src/presenter/images.rs:132](host/linux/src/presenter/images.rs:132) — **Missing painted geometry is treated as on-screen.** The image traversal includes descendants of `display:none` containers, but the painter omits their boxes and `route_visibility()` does not check CSS display. Consequently, hidden images can exhaust all six retries; showing them later leaves the exhausted counter intact. Newly mounted off-screen images can also receive urgent priority before their boxes exist. Keep permissive initial loading separate from on-screen classification: reject hidden ancestry and require valid viewport geometry for retries and urgency. Add a presenter-level hidden-to-visible regression test.

**Non-blocking**

None.

Round-1 blockers are therefore only partially resolved. Source-change retry cleanup looks correct. Promotion and dequeue share the queue mutex, which also serializes the shared streak counter and preserves ordinary-job fairness. No additional thumbnail issue found beyond the acknowledged compositing behavior.

Static review only; no files edited, network used, tests or builds run.
