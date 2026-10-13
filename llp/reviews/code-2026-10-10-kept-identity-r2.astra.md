# Code review: a kept answer's identity read from a fallback (LLP 1027.005 D3) round 2 (75eae7437), 2026-10-10 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `7acae572e2d1d8c20e8ecd716337d741b6590da4ca3775ddb68fc75d64c6a062`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** NOT READY.
- **Disposition:** The shared finding is taken in 019b88ead: a refused kept seed equal to the compiled fallback no longer swaps in for it, so the fallback keeps its compiled mark; a test, direct and through a derive, fails without the fix. Grok's NIT (deps.rs's misplaced doc comment) is taken.

---

- **MAJOR — [runner/src/runner/kept.rs:172](/Users/admin/x2p2/apps/exact2-kept/runner/src/runner/kept.rs:172): an equal-valued fallback can bypass the identity check.** Using the existing fixture, give `account` a kept entry for `[1]` with value `"nobody"`, while this launch calls `account(0)`. Its seed is rejected, but [settlement.rs:717](/Users/admin/x2p2/apps/exact2-kept/runner/src/runner/settlement.rs:717) retains the old, noncompiled `Held` because it equals the compiled fallback. The resulting state has `placeholder=false` and `is_compiled()=false`. Consequently, `feeds` admits a kept entry matching `[0,"nobody"]`, although its account identity remains unknown. This violates amended D3. Preserve fallback provenance across equal-value reuse and add this regression case.

The `with` exclusion, status reads, nested derives, later-settling dependencies, refusal restore, and async activation otherwise look correct by inspection. The ellipsis fix and file split are sound.

All 46 existing kept/settlement/carry tests passed using the prebuilt test binary. They do not cover the transition above. No files changed.

NOT READY
