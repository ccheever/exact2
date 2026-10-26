# Code review: filesystem reader retry, round 1 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `3b41efe390`.
- **Method:** one brief (sha256 `262e2eb8fc6ffa5e9b7d1e77c67856b5f68831c1578e8292ffe18899696523b0`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited (links made relative).

**Blocking**

1. **[scripts/filesystem.mjs:313](scripts/filesystem.mjs:313): Explicit close can be undone by a queued retry.** If `fail()` clears `reader` and rejects pending reads, then `closeFilesystemReader()` runs before their catch callbacks, close finds nothing to stop. The catch subsequently spawns and references another helper. An in-memory event-order probe reproduced a successful read after close. Track an explicit-close generation and check it before retrying; the new test only covers close occurring before failure.

**Non-blocking**

1. **[scripts/install-page.test.mjs:285](scripts/install-page.test.mjs:285): The assertion assumes close wins over EPIPE.** The supported write-error path produces a different message without `status 42`. Assert `EXACT_FS_READER_EXITED` alongside the existing two-spawn count, or explicitly synchronize the notification order.

Otherwise, each call permits at most two sends. The failure latch, drained pending queue and session identity check prevent duplicate settlement and stale-session reuse. Ref transitions remain balanced, and Rust independently restricts `--serve-reads` to `get`. The earlier digest capture and `a7656a62f3` support removing the QUEUE entry.

The permitted test could not reach its assertions: sandbox restrictions denied `mkdtemp`. Read-only probes of the reader functions passed 100 recovery cases and 100 bounded-failure cases.
