# Code review: Linux and Android store writes on a thread of their own, round 2 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `84728e0afa`.
- **Method:** one brief (sha256 `8560dc046a6258fb89b5d487e02ded19ff37142798aa40b6780b6cfb47800abc`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. [host/linux/src/store_writer.rs:202](host/linux/src/store_writer.rs:202) — **Boot can retry forever despite I/O returning.** Each five-second `drain` resets `retried`. If every attempt takes six seconds and fails, the next drain waits for that attempt, then queues another retry; `settle()` never finishes. Preserve the retry allowance across the entire settle operation, using five-second waits only for progress logging.

2. [host/linux/src/store_writer.rs:122](host/linux/src/store_writer.rs:122) — **Empty-commit collapsing ignores captured roots.** An empty retry queued with unavailable roots causes a later empty commit after restoring `HOME` to be dropped. Likewise, a nameless empty commit can disappear behind an earlier retry, skipping its required clearing of pending writes; a later persistent commit can then resurrect them. Resolve roots before deciding to collapse, and collapse only retries with identical roots, including `None` and errors.

3. [host/linux/src/store_writer.rs:182](host/linux/src/store_writer.rs:182) — **Drain retries can change a failed write’s destination.** Calling `queue_commit` here resolves the environment again. A write that failed under scratch store A can reach store B merely because the environment changed before drain or boot; a nameless environment instead silently clears it. Retain each pending app’s last committed roots and use those for automatic retries. Actual app commits can still supply new roots.

**Non-blocking**

1. [host/windows/src/window.rs:720](host/windows/src/window.rs:720) — **Final write failures can go unreported.** Drain returns `true` after a retry fails, but Windows never consumes the remaining app warnings before exiting. Android’s lifecycle drains have the same reporting gap. Collect and log the app’s warnings after draining, as Linux teardown does.

Round-1 A, D and F are addressed. B’s read-ordering fix is present, with the new liveness issue above. C remains incomplete. E fixes warning ownership. STEPS and its call sites are excluded from non-test builds.

Static review only; no files changed, tests/builds run, or network used.
