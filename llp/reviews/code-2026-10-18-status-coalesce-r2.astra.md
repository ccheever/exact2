# Code review: a design-only build-status told once per 150 ms, round 2, 2026-10-18 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** the coalescing commit and round 1 (bce9dcd95).
- **Method:** one brief (sha256 `942d7bbaee5230ea3b5058e6a0720440cb767e0a1de00848c107b22d063ac139`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES (timed test assertions under load; the re-registration test had no waiting design).
- **Disposition:** fixed in round 2's commit (45a018414: a test clock; see its message).

---

No blocking findings. The runtime fixes correctly preserve status/selection ordering and deliver the final design after reconnect.

1. **Should-fix — [scripts/tamber-builds.test.mjs:642](/tmp/rv-cs-2/scripts/tamber-builds.test.mjs:642): timing assertions fail under load.** Three 40 ms sleeps leave under 30 ms of scheduling tolerance; exceeding it deliberately fails with `too slow to check`. The sustained-input count at line 653 also assumes sleeps stay short. I reproduced both failures with correct production code: one additional 45 ms delay, and 30 ms sleeps taking 85 ms. **Fix:** use controlled timers to assert no delivery at 149 ms, latest-design delivery at 150 ms, and unchanged deadlines during sustained input.

2. **Should-fix — [scripts/tamber-builds.test.mjs:668](/tmp/rv-cs-2/scripts/tamber-builds.test.mjs:668): the re-registration test has no waiting design.** The preceding `unsupported` report changes the comparison facts, so `d9` publishes immediately. Consequently, this test still passes with `untell(r.id)` removed—I verified that mutation in memory—even though removing it can lose the first status after re-registration. **Fix:** isolate this case, establish matching facts, queue a design-only change, assert it remains unpublished, then re-register and verify immediate first-status delivery and no later stale emission.

Validation: 19 channel tests passed; additional in-memory ordering, refresh, and reconnect checks passed. No files changed.
