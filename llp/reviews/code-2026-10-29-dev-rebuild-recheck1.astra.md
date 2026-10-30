# Code review: the dev loop's -dev bake driver rebuild, round recheck1 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `1c92a20820`.
- **Method:** one brief (sha256 `5dddf096a3b006956b8a726f2c0c457a8fb0fedf0f1601882a5c8196f29c0f81`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. [host/web/dev-bin-run.test.mjs:39](host/web/dev-bin-run.test.mjs:39) — **PID publication races with takeover.** Between A’s `mkdirSync` and PID write, B can encounter `EEXIST`, create/read an empty PID file through `'a+'`, and delete A’s lock. A can then write into B’s replacement directory, allowing both runs into the test. Publish ownership atomically—for example, exclusively link a prewritten claim—and do not treat an unpublished PID as proof of death.

2. [host/web/dev-bin-run.test.mjs:42](host/web/dev-bin-run.test.mjs:42) — **Two stale-owner observers can delete a replacement lock.** A and B both read the dead PID; A removes the directory and acquires a new lock; B then removes A’s live lock using its earlier observation. Both can manipulate the same `deck/`, and cleanup can remove another run’s lock. Serialize reclamation with acquisition and release, preferably using a process-held advisory lock. Also handle a disappearing directory during the PID read: its `ENOENT` currently escapes before cleanup.

**Non-blocking**

none. `EPERM` correctly means “alive.” The compaction preserves `bakeWatchEnv`, `devBinRun`, and spawn behavior for existing callers.

Static review only; no files edited, network accessed, tests or builds run.
