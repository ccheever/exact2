# Code review: LLP 1121 stage 2, commit 2, round 3 (last), 2026-10-14 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, a throwaway worktree at the reviewed commit, read-only by instruction.
- **Method:** one brief (sha256 `6d3c1bf64e47ce7b8c6bc777078c25a5aee3ebd19804ced748117e8179eaa522`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra fix; Grok ship.
- **Disposition:** Astra's findings applied without re-review in the round 3 commit

---

The round-2 fixes hold. Each one does what the commit says, and the surrounding theme, scan, and web map tests still pass.

**Profile tokens stay in the check.** `profiles` rewrites a `color(--…)` default to `transparent` and restores the authored string when the entry is built, matched on the token’s own name. Reserved names, duplicates, and exports still run: `red = color(--brand …)` is `tamber-name-reserved`. On a successful check every token is present, so the restored colour is that token’s, not the stand-in.

**Role spellings match.** `same_name` treats two writings of one role as the same token, for a whole-value alias and for a use inside an expression. `Canvas` / `canvas` and `-exact-secondary-label` / `secondary-label` both carry `color(--brand 0.1 0.2 0.3 0.4)`. A use that is not the whole value is still `lower-theme-value`.

**A profile may have `when`.** The default is what the plan stores. The `when` values stay in the design and are folded. `ink` with `when dark-style=dim` publishes the profile default, and `other` is kept. Axes in the plan remain commit 3’s work, as this commit says.

**`Stmt::Call` is walked.** Arguments and the expanded body are both scanned. `set("light-dark(brand, card)")` reaches the plan table. The call arm replaces the old wildcard, and `Refresh` is the only statement with no expression.

**Strictness follows the value.** Through `?:`, `match`, and `let`, only the branches that are the colour are strict. A condition string and a call argument are loose. The ternary is `(condition, yes, no)`, so `label == "light-dark(brand, CanvasText)" ? "brand" : "card"` compiles, while a branch that names no token is still `tamber-unknown-token`.

**`url()` and quotes are copied whole.** The web map matches those before a colour function, and a match that is not a function call is kept as written. The prefix is still rewritten, so `url("…/light-dark(brand, surface.card).png") brand` keeps the URL and writes the colour for the trailing name. The existing synthetic, role, and gradient cases still pass.

**Refusals use the token’s column.** A file token’s span takes the name’s line and column. `top` under a two-space indent is `(5, 3)`.

`tamber::` and `theme::` (21 tests) and the web-js style tests (13) pass, including the new ones.

VERDICT: ship
