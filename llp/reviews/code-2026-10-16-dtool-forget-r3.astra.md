# Code review: the design tool's undo stacks after a server restart (main red), round 3, 2026-10-16 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `6dbceca3f`.
- **Method:** one brief (sha256 `60db83b7d9bfd5016b1e431671778feb955664f1c699bc6851906b5856f9f89e`), a quick review under the main-red standing permission. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** needs changes (3 medium).
- **Disposition:** taken. The poll passes the token of the run its place was read from (`placeRun`, published with `place`, not `heldToken`). The runs forgotten are a set, so a late refusal from an earlier run forgets nothing of a later one. A run already forgotten is forgotten again when the stacks hold another run's entries (an unpolled run between restarts). Left, said in the commit: an in-flight operation of a run found gone only by a refusal, with nothing on the stacks, is not fenced. Its token is that run's, so it cannot write to another run.

---

- **Medium — [app.ts:1085](/tmp/rv-dt-forget3/apps/design-tool/app.ts:1085): an unpolled intervening run escapes invalidation.** Poll last saw A; an A refusal calls `forget(A)`; a fresh B write succeeds; C starts before any poll observes B. C’s poll passes `was=A`, so forgetting is skipped. B’s invalid undo entries and displayed answer survive; delayed B operations also pass the unchanged-session guard. **Fix:** compare the poll’s new `used` token against the runs owning retained and in-flight work, rather than invalidating only `was`. Keep invalidation independent of stack depth.

- **Medium — [app.ts:1069](/tmp/rv-dt-forget3/apps/design-tool/app.ts:1069): the previous token and epoch can describe different runs.** A B poll updates `heldToken`, then awaits resync `hello` at line 1122 before updating `place`. Changing the client name starts another poll. Its B answer sees `was=B` but `place.epoch=A`, so it calls `forget(B)` while B is live. This can hide the refusal again and makes the subsequent genuine B→C reset a no-op. **Fix:** publish the poll’s token and epoch together, with replacement-poll checks covering their publication.

- **Medium — [app.ts:489](/tmp/rv-dt-forget3/apps/design-tool/app.ts:489): deduplication remembers only the most recently forgotten run.** After A→B→C, `forgotRun=B`. A delayed A-bound commit refusal calls `forget(A)` again, clearing valid C undo entries and incrementing the session. This remains an incomplete fix for overlapping refusals across restarts. **Fix:** make retirement idempotent across intervening runs and invalidate only the named run’s state, including its in-flight operations.

HEAD has no shared `refusedBy`/`restarted` hand-off or predicate reset. Request-local `used`, accepted-proposal tokens, and redo token preservation look correct. An actual forget still fences earlier operations with empty stacks.

Source review only; no tests run or files changed.

**Verdict: Needs changes—restart identity and deduplication still fail under delayed or overlapping observations.**