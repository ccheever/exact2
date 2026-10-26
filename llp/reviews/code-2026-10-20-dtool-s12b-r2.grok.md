# Code review: the design tool's "Set for this app" on a package component's params (stage 12 D (b)), round 2 of 2, 2026-10-20 (grok)

- **Family:** xAI. Grok via its CLI, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `13312a7783`.
- **Method:** one brief (sha256 `253632cdd92b60d0ff4fd279bf7560f13f1e0953641a34417d70c5d384442e3a`), shared by both reviewers, each blind to the other's review; commits 848102632c and 13312a7783 (round 1's fixes and part (a)'s post-round fix). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited (from its findings; its narration before them left out).
- **Verdict:** nothing blocks (1 SHOULD-FIX).
- **Disposition:** taken post-round, in the commit named in the sign-off request:
  - (1) as Astra's 1.

Nothing blocks. The four recorded part (b) fixes are in the tree and hold, and the shared parts counter does not let a late read put parts back or leave them unset.

The role spelling, the package label, the column check, the stub restart, and the param edits all match the round-1 notes:

- A default is resolved with `tokenOfIn` (`components.ts:99`, called from `app.ts:739`). `label`, `-exact-label`, and `-Exact-Label` all become `-exact-label`. A param name such as `app.contract#Card.tone` still does not. `tokenPackage` is describe's `package` on that token's row (`app.ts:739`), and the button says "the package token" only when that string is non-empty (`app.contract:2290`). The fixture covers an app token (`accent.fill`), a package token (`accent.soft`), and `-Exact-Label`.
- The panel uses the selected column only when `design.columns.map(c => c.id).includes(selColumn)`, otherwise `design.firstColumn` (`app.contract:1835` and `:1841`). A `3-dark` selection whose axis is gone opens the first column instead of calling `loadBranch` with an id it will not find.
- Restart assigns `FILES` back to the object captured before the first package post (`tamber-stub.ts:444`, `:454`). A second post no longer leaves the first post's keys behind, and an overwritten `app.tamber` digest comes back.
- The package test types Badge.fill and expects `set-param` on `@acme/cards/badge.contract#Badge.fill`, then presses Badge.edge's reset and expects `unset-param` (`app.test.contract:2092-2102`).

The parts counter is safe. `partsReads` moves only for an app describe: `loadDesign` increments when `!file || file === designFile`, and otherwise stores `partsAt = -1` (`app.ts:645`). `loadAppAxes` increments only when a file is open (`app.ts:1360-1366`). Both write `libraryParts` only if their captured ticket is still `partsReads`. A `brand.tamber` read does not bump the counter and does not write parts, so it cannot retire the in-flight app read and then refuse to replace it. A late app describe loses to whichever read incremented later. Switching back to the app design bumps the counter inside `loadDesign` before the older `loadAppAxes` can resume.

1. **SHOULD-FIX.** An unset base name no longer opens from a package param. `components.ts:99` keeps a default only when `tokenOfIn` returns it, and that helper only sees declared tokens and role spellings (`rescope.ts:29-33`). Unset scale names are not in `ir.tokens`. They are added to the grid's name set at `app.ts:682` (`radius.md`, `space.md`), and that set is still passed into `componentsOf`, but the call at `app.ts:739` never consults it. Scenario: a package `Badge.pad` defaults to `radius.md`, and the app has not overridden it. The grid still lists `radius.md`. `tokenOf("radius.md")` is null, so `r.token` is `""` and `app.contract:2288` draws no button. Before this commit the same default matched the name set and offered the button (mislabeled as the package's). Describe does not mark a package on it, so the label should be "Edit the token radius.md instead", and the button should still call `selectCell`. Use `tokenOfIn` first, then the grid name set.
