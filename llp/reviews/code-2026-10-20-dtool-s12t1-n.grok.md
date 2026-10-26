# Code review: the design tool's package param levels (stage 12 D follow-up T1), narrow verification round of round 2's post-round fixes, 2026-10-20 (grok)

- **Family:** xAI. Grok via its CLI, reasoning effort xhigh, Edit and Write denied, a throwaway worktree.
- **Method:** one brief (sha256 `0c6fc168d4429e0a91dd065c7093e2b6c0ea7826e3acc5eed07cf4c78e496d35`), quoting round 2's two reviews verbatim, shared by both, each blind to the other's; HEAD `eec0282eb3`. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited (from its findings; its narration before them left out).
- **Verdict:** 1 BLOCKER (A1 still open), the rest closed.
- **Disposition:** taken post-round, with review2's 17:16 early read (levels never block the design read), in the commit named in the sign-off request, and a second narrow round on it:
  - The BLOCKER (both reviewers): B2-levels' walk picks a `when` branch with no scheme side, so a tuple's level is the dark side's where a branch on an axis `under scheme=dark` decides it. The tool no longer leans on it. The level line names no column ("wins: …"). "The app's own value of this param still wins" shows only for a literal app line with no `when` branch, read from describe's `set`, which wins on both sides. The walk itself is asked of x2-perf4 and theme2 (to-x2-perf4.md and to-theme2.md 18:55).


1. **BLOCKER — not closed.** "Still wins" is still taken from the scheme-blind walk. `readDesign` sets `self` only when every tuple is `app-param` for this param (`apps/design-tool/app.ts:152`), and the package note then says the app's own value still wins (`apps/design-tool/app.contract:2315`, via `components.ts:137`). The walker chooses a matching `when` with no scheme (`contract/cli/src/tamber_levels.rs:134-144`). The fold does the opposite: a `when` on an axis `under scheme=dark` becomes a pair whose light side is the base value (`tamber/src/check.rs:966-972` and `1232`).

   This file still gets the false assurance. The package defines blue `accent.soft`, the app overrides it with red, and the app sets:

   ```text
   dark-style = black | dim  under scheme=dark
   Pill.fill = accent.soft
     when dark-style=black: #000000
     when dark-style=dim: #111111
   ```

   Both tuples stop on `app-param` / `Pill.fill`, so `self` is true and the note says the app's own value still wins. Back to the package changes Light from red to blue. A single value, `dark-style = black`, is the same hole: the only tuple is that dark branch, `self` is true, and Light is still the token.

   The review's narrower file (only the `black` branch, so the `dim` tuple walks on to `accent.soft`) does drop that sentence: `self` is false, and the note says the app's line stays. The wins line does not. Tuple 0's column is still labeled Light (`apps/design-tool/branches.ts:15-23`, `app.ts:151`, `app.contract:2308`), so that row still says "wins in Light: set for this app" while Light follows the token.

2. **SHOULD-FIX — closed** (Review A2 and Review B1 are the same defect). Package params are resolved when the file is empty or is the app design (`app.ts:121`, `file === designFile`). Hello reports that same path (`scripts/tamber-server.mjs:726`; the stub sends `app.tamber`), and `loadDesign` already treated both as the app design (`app.ts:749`). Passing that file is the server's own design (`contract/cli/src/tamber_base.rs:34-36`).

3. **SHOULD-FIX — closed.** The fixture (`app.test.contract:2119`) declares `Pill.gap`, set to `16px`, and leaves `Pill.inset` unset with that param as its default. Describe shows the full name (`contract/cli/src/tamber_params.rs:469`); the walk follows the token name and stops on `Pill.gap`'s app line (`tamber_levels.rs:163-164`). `Pill.tone`'s other tuple is `role` / `-exact-label`, which a package line can produce with `app_set` false. `Pill.ring` has no `level`. The refused resolve is its own test (`app.test.contract:2152-2160`): the stub returns 400 before the canned winner (`stub/tamber-stub.ts:320`), and `call` surfaces that message (`wire.ts:15-17`).

4. **NIT — closed.** A successful reply whose first tuple has no `level`, or which has no tuples, is stored with an empty level (`app.ts:152`). The row says "not known (this server says no level)" (`components.ts:136`, `app.contract:2308`). The contract test expects that for `Pill.ring` (`app.test.contract:2132`).

5. **NIT — closed** (Review B2). The package sentence is "with any when branches it has" (`app.contract:2315`), which is true whether that token line has branches or not. The assertions at `app.test.contract:2140-2141` match it.

HEAD does not break the other pieces. A literal app line that wins on every tuple still gets `self` and the still-wins sentence (the fixture's `Pill.fill`). A missing level is shown. Choosing `app.tamber` resolves the package params. The new test strings match the rendered notes.
