# Code review: stage 4 commit 2: the closure, round 2, 2026-10-15 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `fbf6c3998a579e5c53e8b3f23d645bbeeee934726b49a543265788bddea710cd`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes requested (stale candidates after an early stop; profile filter before recording; strictness).
- **Disposition:** fixed in the second-round commit

---

I found **two blocking P1 issues and one P2 issue**.

1. **P1 — Early compile failures can cache the previous build’s candidates.** [contract/cli/src/tamber_serve.rs:361](/tmp/rv-s4c2b/contract/cli/src/tamber_serve.rs:361)  
   `CANDIDATES` is cleared only inside `declare_theme`. Parsing, type checking, and font/sound loading can fail before that. The previous nonempty list then makes `reached` true and gets cached under the new source key. For example: add a synthetic expression and a font declaration, request a proposal while the font asset is missing, then supply the asset. The cache key remains unchanged, so subsequent edits can return `live` without the new synthetic value. Reset candidate state at compilation entry and distinguish “design lowering reached” from leftover data. Successful inline, external, and no-design lowering paths do clear it correctly.

2. **P1 — Candidate collection still excludes expressions that value edits can make foldable.** [contract/lower/src/theme.rs:175](/tmp/rv-s4c2b/contract/lower/src/theme.rs:175)  
   Candidates are recorded **after** filtering expressions that reference profile colours. That filtering depends on token values, while the cache key includes only token names. Start with a profile-valued token and a loose mix literal in a function/state; a successful compile caches a list without that mix. Changing the token to a literal colour triggers a rebuild, which now retains the mix. The next colour edit returns `live` but omits its synthetic update because the cached list never changes. Record candidates before this filtering, or invalidate on the relevant value changes.

3. **P2 — The planner drops required row expressions as though they were optional candidates.** [tamber/src/session.rs:224](/tmp/rv-s4c2b/tamber/src/session.rs:224)  
   `foldable` drops every failing synthetic expression; the compiler drops only non-strict candidates. With a literal row `color="color-mix(in srgb, brand 60%, white)"`, changing `brand` from `AccentColor` to `CanvasText` drops the expression on both sides and returns `live`, although the compiler refuses it. This matters after a failed rebuild: further edits can advertise live updates while the affected row remains unfoldable. Preserve strictness and require rebuild/refusal for these failures.

The added tests miss these cases. The role check appears to cover transitive aliases, and the revised gain/loss comparison handles both directions.

This was a static, read-only review. Tests were not run because the checkout has no build artifacts. `git diff --check HEAD~2 HEAD` passed; the working tree is unchanged.
