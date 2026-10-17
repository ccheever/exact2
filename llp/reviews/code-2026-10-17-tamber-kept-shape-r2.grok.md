# Code review: One retained-shape check for propose and the dev loops (theme2's ruling, Tamber stage 5), round 2, 2026-10-17 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree.
- **Scope:** HEAD, the first round over the change.
- **Method:** one brief (sha256 `b8904bd566eba767daa2bd56e4702f44c4d8a8dc361f5f81680c4fde976905a2`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the CLI's output, unedited (its progress lines run into the first paragraph).
- **Verdict:** nothing blocking.
- **Disposition:** in the second-round commit.

---

I'll review the round-2 fixes read-only: the cache key, session shape, and theme retention, plus any live-class edit that still diverges from the plan structure.The round-1 patch is small. I'll compare `shape` with `Plan::structure` so a `live` class can't hide a structure change.No findings. Nothing blocking.

The three round-1 fixes hold, and no edit `propose` would class `live` while `Plan::structure` or `design::shape` changes.

- The cache key in `contract/cli/src/tamber_serve.rs` now includes `code_beside` (`app.ts`, `data/Cargo.toml`, `app.json` modules or hatches), the same bit `DesignSource::bindable` uses. A data crate added under a running server misses the cache and the next `propose` keeps every token. Source hashes already cover the other half of `names_tokens_outside_views` (a non-runner source, a module tag).
- `shape` in `tamber/src/session.rs` uses the name as written, which is what the plan row stores. `CanvasText` to `canvastext`, or `secondary-label` to `-exact-secondary-label`, changes that row and is `rebuild`. `keeps` still short-circuits on `role.is_some()`, so retention does not move with the spelling.
- `contract/lower/src/theme.rs` publishes `Retention` before the fold. `words`, `computed`, and `bindable` do not depend on it. A design that fails to fold no longer looks like "keep every token." When the file itself does not fold, `plan` already returns `rebuild` because one side is missing.

`shape` still carries an axis's `under scheme=dark`, a dial's kind, a token's kind, and `scheme=fixed`, which `Plan::structure` and `design::shape` do not. Those edits stay `rebuild` when the loops could batch: slower, and the running plan does not change.
