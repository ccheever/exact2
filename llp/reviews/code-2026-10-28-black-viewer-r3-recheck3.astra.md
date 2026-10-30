# Code review: the black viewer: thumbnail under the picture, picture retries and priority, round 3, re-check 3 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `445212a3d6`.
- **Method:** one brief (sha256 `06ed48a670fd165025731aecf72384bf51f6374d74e734f7f8ab6bb003ea59a1`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. [host/linux/src/presenter/images.rs:45](host/linux/src/presenter/images.rs:45) — **The advance-before-validation fix remains incomplete on the live clock.** `advance(100)` can execute a callback that samples time 110, leaving a timer due at 105 untouched because its cutoff remains 100. After the source check, `Host::dispatch_at(..., 100)` clamps to the updated host time, 110, and runs that timer before the load handler. If it changes the image source, the stale load still fires. Revalidate after dispatch’s final advance, or keep the event queued and defer delivery whenever work remains due at the updated host time.

2. [host/linux/src/presenter/images.rs:44](host/linux/src/presenter/images.rs:44) — **The current event escapes source-change cancellation during `advance`.** It is removed before advancing. That advance’s commit processing can deliver two resize callbacks that change this image’s source A→B→A; their `sync_shown` calls cannot remove the extracted event. The final key/source check passes, delivering an obsolete load, potentially followed by the new assignment’s load. Keep the candidate in the queue throughout advancement, removing it only afterward, or validate a source-assignment generation.

**Non-blocking**

none.

The earlier-load away/back case, Canvas quick-frame delivery, and `cool` self-eviction are addressed. Static review only; no files edited, network, tests, or builds.
