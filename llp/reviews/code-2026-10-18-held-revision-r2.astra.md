# Code review: tamber/1.12, the long poll's held revision, round 2, 2026-10-18 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** the held-revision commit and round 1.
- **Method:** one brief (sha256 `d724b20c5b46392832579dd0885ed5af60461de92bc4e59c9cb42e8f436d99b6`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT (a nit: the recompute assertion's baseline).
- **Disposition:** fixed in round 2's commit.

---

Nothing is blocking. One nonblocking finding:

1. **Nit — [scripts/tamber-server.test.mjs:903](/tmp/rv-hr-2/scripts/tamber-server.test.mjs:903): the recompute assertion uses an earlier baseline.** `moved.held` was captured before creating `second`. That creation already advances the revision, so the assertion passes even if recomputation changes the list without advancing it. **Fix:** poll after creating `second`, immediately before accepting it, then use that revision for the post-recompute poll and assertions.

Read-only, in-memory probes of the actual server and client passed for rename, restart, same-epoch `missed`, overlapping polls, swatch refresh, restoration, expiry, eviction, and stale revisions. Other callers remain compatible, and the protocol documentation/version entry match the implementation. The full `files` map still accompanies every poll; that cost predates this change.

Full integration suites were not run because they write fixtures.
