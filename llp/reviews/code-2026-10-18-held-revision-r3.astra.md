# Code review: tamber/1.12, the long poll's held revision, round 3, 2026-10-18 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** the held-revision commit and rounds 1 and 2.
- **Method:** one brief (sha256 `5597f47847187ac5849e339ac9fc6af9a4dbc67e9bb05e07912eae447becb178`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT (nits: a malformed `held` accepted; QUEUE names 1.11).
- **Disposition:** fixed in round 3's commit.

---

Nothing is blocking. Two nits:

1. **Nit — [scripts/tamber-server.mjs:521](/tmp/rv-hr-3/scripts/tamber-server.mjs:521): malformed `held` silently disables caching.** Sending `held: "0"`, `true`, `{}`, or `[]` returns 200 and the full list. This contradicts [docs/tamber-protocol.md:118](/tmp/rv-hr-3/docs/tamber-protocol.md:118), which promises an error for incorrectly typed fields. A client accidentally stringifying its revision therefore keeps downloading every proposal. **Fix:** validate non-null `held` as a nonnegative safe integer, or explicitly document malformed hints as cache misses.

2. **Nit — [QUEUE.md:7](/tmp/rv-hr-3/QUEUE.md:7): future work still targets 1.11.** The proposed batch `resolve` is described as an addition “under tamber/1.11,” although this change opens 1.12. **Fix:** update that reference to 1.12 or remove the hardcoded version. The runtime bump is complete; remaining host and RFC §10 references describe historical additions.

Read-only, in-memory checks passed for parked polls, accept removal/restoration/recomputation, eviction/expiry, down/503, omitted-list retention, superseded polls, client renames, epoch changes, and snapshot refreshes. No files changed; full integration suites were not run.
