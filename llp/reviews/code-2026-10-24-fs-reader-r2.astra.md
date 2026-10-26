# Code review: filesystem reader retry, round 2 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `59bd54153a`.
- **Method:** one brief (sha256 `353613aa9f78fd81787305889e924117bfd04ab7309ea438566c6ab4bfe18893`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

none.

The generation fix closes the reported race, including when `fail()` has already cleared `reader`. Each read gets at most two sends. Failure handling remains idempotent, stale callbacks cannot clear a replacement session, and liveness references stay balanced. Both the bridge and helper restrict this path to `get`. QUEUE removal is justified by this fix and the earlier capture/liveness changes.

**Non-blocking**

1. [scripts/install-page.test.mjs:288](scripts/install-page.test.mjs:288) — The shutdown assertion does not cover the round-1 race: it closes synchronously before the helper’s failure callback runs, so it also passes without the generation guard. Add a controlled `fail → closeFilesystemReader → retry callback` case and assert no replacement spawns. The 200 ms delay cannot exercise that ordering. The revised error-code assertion correctly tolerates close versus EPIPE.

In-memory event-order checks reproduced the round-1 bug and passed on HEAD, including concurrent retries, stale callbacks, second failures, and reference transitions. The permitted Bun test stopped before assertions because the sandbox denied `mkdtemp` with `EPERM`.
