# Code review: tamber/1.12, the long poll's held revision, round 1, 2026-10-18 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** the held-revision commit.
- **Method:** one brief (sha256 `c5a1b805efad5ab370019951e00ac28e7bfccb968e43534acf480e17c7fed50b`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT (nothing in the code; accept/restore race coverage).
- **Disposition:** tests added in round 1's commit (drop, recompute).

---

Nothing is blocking in HEAD (`ac05e57f0`). I found no implementation defect.

1. **Should-fix — regression coverage for accept/restore races**, [scripts/tamber-server.test.mjs:879](/tmp/rv-hr-1/scripts/tamber-server.test.mjs:879). The new assertions check an unchanged list and completed successful acceptance, but never poll during acceptance. The critical scenario is: accept removes a proposal; a poll caches that empty list and revision; acceptance fails and restores the proposal. Without the restore increment, subsequent polls omit the restored list indefinitely, and these assertions would still pass. **Fix:** suspend the transaction response, poll during removal, fail acceptance, then poll using the intermediate revision and assert that the restored proposal arrives with a newer revision. Add revision assertions for expiry and cap eviction too. HEAD handles these correctly; this is a test coverage finding.

Read-only in-memory probes passed for server mutations, concurrent acceptance, epoch changes, discarded/error polls, retained import operations/reviews, token binding, and swatch refreshes. The full-list stub, `subscribed` event, documentation, and version consumers are consistent. Integration suites were not run because they modify fixtures. No files changed.
