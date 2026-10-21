# Code review: stage 4 commit 6: uses, resolve and scrub sessions, round 3, 2026-10-16 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** the whole range, the round-1 and round-2 fixes first.
- **Method:** one brief (sha256 `c051c8b8687183970cdc1e1cac52b06b5a00b31caac516dd2af51eff74d0bc91`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** P2s (a previous release overwriting the next scrub; a delayed update reopening a cancelled scrub; `uses` missing locals and role spellings).
- **Disposition:** applied without re-review (the last round) in the third-round commit.

---

I would hold this for the scrub races. Four P2 findings remain:

- **P2 — A previous release overwrites the next scrub.** [host/web-js/dev.mjs:155](/tmp/rv-s4-13/host/web-js/dev.mjs:155): release A, preview B before A’s build finishes, then let that build land. B inherits `released=true`, so the build clears B’s preview and broadcasts A while B remains open. The TypeScript and Rust paths retain the same flag across new previews.

- **P2 — Delayed updates resurrect cancelled scrubs.** [scripts/tamber-scrub.mjs:90](/tmp/rv-s4-13/scripts/tamber-scrub.mjs:90): the supplied scrub ID is checked only while a session exists. After cancellation, a delayed update carrying the old ID and unchanged opening digests opens another session and previews again.

- **P2 — `uses` misses tokens assigned through locals.** [contract/cli/src/tamber_uses.rs:151](/tmp/rv-s4-13/contract/cli/src/tamber_uses.rs:151): an action containing `let chosen = "brand"` followed by `ink = chosen` makes `color=ink` a real use. `BindLocal` advances `start` past the token, and `LoadLocal` carries no provenance, so the site disappears. Parameterized inline functions have the same problem.

- **P2 — `uses` misses alternate role spellings.** [contract/cli/src/tamber_uses.rs:203](/tmp/rv-s4-13/contract/cli/src/tamber_uses.rs:203): declaring `secondary-label` and writing `color="secondary-label"` works, but `uses` for its canonical IR name, `-exact-secondary-label`, returns no site. Matching ignores the compiler’s role spelling equivalence and case rules.

Both races reproduced using the current JS in memory. All 12 isolated session tests passed; they miss these sequences. Full Rust/integration suites weren’t run. Worktree and index unchanged.
