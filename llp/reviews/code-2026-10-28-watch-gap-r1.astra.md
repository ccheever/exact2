# Code review: the design server's watcher: a read after a new watch, round 1 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `b5b39dadab`.
- **Method:** one brief (sha256 `7393086cc4453f21b7d0e982c8676f1650a6f3a811d36686fa068002bb691367`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. [scripts/tamber-watch.test.mjs:55](scripts/tamber-watch.test.mjs:55) — The test can still flake. Startup and each newly installed watch schedule another `hello`; neither `wire.ready` nor observing the latest digest guarantees those reads have finished. A read can snapshot the old source before the swap, then return afterward. Its `watchAll` fills the replacement directory, and subsequent reads see the filled source. No response reports the directory empty, so this assertion times out despite correct recovery. Assert that the injection ran and the expected round digest arrived; control outstanding refreshes before each swap if deterministic failure without the fix is required. Dropping late callbacks cannot cancel reads already queued.

**Non-blocking**

1. [scripts/tamber-server.mjs:461](scripts/tamber-server.mjs:461) — The production fix correctly closes the reported gap. Stable inodes stop the chain; a failed watch alone does not rearm it. Continually changing inodes or flapping directories can sustain rescans indefinitely, but this requires continuing filesystem changes. The settle timer coalesces additions; there is no self-sustaining loop on a stable tree.

2. [scripts/tamber-server.mjs:468](scripts/tamber-server.mjs:468) — I found no new write race or duplicate announcement. `writing` defers rescans through `again`, and the final write reply resumes them after publishing its digests. Earlier reads and writes are processed in reply order. Both `publish` and `packageDesigns` suppress unchanged snapshots. A genuinely observed disappearance and return can produce two legitimate events.

3. [scripts/tamber-server.mjs:406](scripts/tamber-server.mjs:406) — The cost is an additional `hello` per installation batch, potentially followed by another when descendant watches become available. It is **not necessarily another `describe` per watch**: `learnParams` skips matching file keys and plans. Changed files, package invalidation, or a previous failed describe can require one.

4. [scripts/tamber-watch.test.mjs:30](scripts/tamber-watch.test.mjs:30) — Suppressing late parent-directory events is an honest model of those notifications having arrived before the read. The synchronous fill reliably creates the intended gap. The outstanding-read race above limits the test’s determinism.

5. [scripts/async.mjs:116](scripts/async.mjs:116) — The new test is absent from the async lane’s explicit script list. Add `tamber-watch` so the regression runs automatically.

6. [scripts/tamber-package-designs.mjs:30](scripts/tamber-package-designs.mjs:30) — Package watches retain the analogous pre-existing installation gap. Their periodic check observes inode changes, so a fill after that inode was recorded can remain unseen. Treat this as a separate follow-up; this patch covers `watchAll`.

Static review only; no tests or network access.
