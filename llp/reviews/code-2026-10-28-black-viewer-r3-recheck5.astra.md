# Code review: the black viewer: thumbnail under the picture, picture retries and priority, round 3, re-check 5 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `0d02ee25d2`.
- **Method:** one brief (sha256 `d185541722bae6cd059134be5dc6ab69865134f719a5b88f3eb28f3c7d937f84`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. [host/linux/src/host.rs:926](host/linux/src/host.rs:926) — `dispatch_now` computes the later timestamp but never updates `self.now_ms`. `commit_effects` clamps receipt times to that host clock (lines 1257 and 1327), so the commit does **not** necessarily occur at `max(runner, host)`. For example, a preceding live dispatch can leave runner time at 2600 ms and host time at 2000 ms; a load-triggered 200 ms transition then starts at 2000 and finishes immediately on the next frame. Previously, `dispatch_at` promoted the host clock. **Fix:** set `self.now_ms = at_ms` before committing, without advancing the runner or executing due work.

**Non-blocking**

none.

The previous validation race and deferral starvation are resolved. The declared away-and-back behavior is acceptable: the retained bitmap matches the source checked when the handler runs. I found no additional delivery or error-path blocker.

Static review only; no edits, network, tests, or builds.
