# Code review: Harmony's seed refuses to empty a server that holds data unless --force (apps/harmony/seed.mjs), narrow re-check 1, 2026-10-21 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `f541186f33`.
- **Method:** a narrow brief naming the earlier findings and their fixes; 2 rounds (the lead's 20:50Z), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Note:** Grok's re-check 1 was stopped by the author after one line, its worktree having moved to the next fix; its re-check 2 below covers that fix.

**Not clean: one P1 remains.**

- **[P1] A concurrent write can still be erased without force** — [apps/harmony/seed.mjs:276](apps/harmony/seed.mjs:276). The head is checked at line 268, separately from the reset. An empty store can return `seq: 0`, then accept a `setProfile` mutation before `/dev/reset`. That write changes neither `storeId`, `instance`, nor `pid`, so both identity checks pass and the reset erases it. This affects both the default origin and an explicit origin. The pinned server checks the instance header but imposes no head precondition.

  **Fix:** require `--force`/`--reset` for every destructive reset, or add a server-side `head === 0` precondition checked atomically within the reset transaction. Another client-side read cannot close this race.

The three reported cases are addressed: replacement instances receive 409, stores populated before the head read are refused regardless of persistence, and the reported missing/empty identity fields are rejected. They do not establish the broader guarantee against unforced data loss.

Read-only review; no tests, servers, seeds, installs, or network commands run.
