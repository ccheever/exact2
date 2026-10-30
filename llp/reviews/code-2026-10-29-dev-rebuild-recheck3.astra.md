# Code review: the dev loop's -dev bake driver rebuild, round recheck3 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `9182a5cfa7`.
- **Method:** one brief (sha256 `cbe734e45bf5911cac42b1d6eac7541510ec33fe0aa2cf65c47ea98bb590ec44`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. [host/web/dev-bin-run.test.mjs:40](host/web/dev-bin-run.test.mjs:40): Cold runs cannot acquire the lock. `claimBuildOutput` created the parent directory; `takeLock` does not. `apps/sparkline/target` is absent in this checkout, and `resolveApp` does not create it. Consequently, `openSync` throws `ENOENT`, and the catch retries continuously for 25 minutes before failing. Create the target directory with `mkdirSync(..., { recursive: true })` before the loop, and retry only contention timeouts.

**Non-blocking**

none
