# Code review: the dev loop's -dev bake driver rebuild, round recheck2 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `0e7226e893`.
- **Method:** one brief (sha256 `1a2cb717838661f66c9ac4fe4917133b085069b6743b6f1d3c5431e3707797bd`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. [scripts/app.mjs:1188](scripts/app.mjs:1188), reached by the new call at [dev-bin-run.test.mjs:38](host/web/dev-bin-run.test.mjs:38): **Stale takeover still races.** Two waiters can both pass the content comparison before either deletes. One then deletes and acquires; the other deletes that replacement and acquires too. Both tests enter the critical section, and release also unconditionally deletes the path. Fix: use a kernel-held lock covering the entire test, including cleanup; comparing contents before unlinking is not atomic.

2. [scripts/app.mjs:1182](scripts/app.mjs:1182): **Interrupted publication can permanently strand the claim.** `wx` atomically creates the file, but writing its contents is separate. If the process dies between those operations, an empty claim remains. `claimLive` returns `true` for invalid JSON at line 1214, so every subsequent run waits 25 minutes and fails without recovering it. Fix: use the kernel-held lock above, or publish fully written metadata atomically through an exclusive hard link.

**Non-blocking**

none
