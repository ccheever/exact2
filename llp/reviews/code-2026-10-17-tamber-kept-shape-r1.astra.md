# Code review: One retained-shape check for propose and the dev loops (theme2's ruling, Tamber stage 5), round 1, 2026-10-17 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** HEAD, the change.
- **Method:** one brief (sha256 `ee0ddb7e0972cdef24db01ad32800a124dd9a5629448caf587cbbbce1d72e4ce`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes requested (the cached retention missed binding inputs; a respelled role override escaped the shape).
- **Disposition:** in the first-round commit.

---

I would block on the cache issue.

- **P2 — Cached retention misses binding inputs:** [contract/cli/src/tamber_serve.rs:377](/tmp/rv-ks-1/contract/cli/src/tamber_serve.rs:377). Populate the cache for a plain app, then add `app.ts` or `data/Cargo.toml` without changing Contract sources. Proposing an unused token addition still returns `live`, although compilation now retains that token and changes `Plan::structure`. Native declarations in `app.json` have the same problem. Include these inputs in cache invalidation; the new tests create them before starting the server, so miss this case.

- **P2 — Role spelling still escapes shape comparison (pre-existing):** [tamber/src/session.rs:342](/tmp/rv-ks-1/tamber/src/session.rs:342). A transaction removing `CanvasText` and adding `canvastext` with the same value returns `live`: `t.name` is canonicalized. Lowering preserves the authored name in the theme row, so `Plan::structure` changes. Compare the name actually emitted into the plan.

No token-retention regression found in the lowering refactor. Rust tests were not run in the read-only sandbox; the worktree and index remain unchanged.
