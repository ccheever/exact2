# Code review: the Inspector's folded initial rows and the base rows across the grid, round 1 (astra)

- **Family:** OpenAI. GPT-6 Astra via codex exec, reasoning effort max, read-only sandbox, a throwaway worktree.
- **Method:** one brief (sha256 `0385b4092bf85fca47afb1796a73308a457a3c22d798ce7687d3999149897e7c`), shared by both reviewers, each blind to the other's review; HEAD `00f7457370`. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocks; 2 SHOULD-FIX, 2 NIT
- **Disposition:**
  Neither reviewer found anything blocking. Taken in the round 1 commit:
  - Astra 1 (an expansion survived a round trip: shown on A, B selected, A again reopened): a task folds them again once the selection moves to another view (`task refold … key=inspected.key`, the timer a commit later). The folded rows and the control both read one derive (`foldedRows`, `foldedHeld`). Tested in Astra's order. Removing the task fails the test.
  - Astra 2, Grok 2 (the before and after screenshots were the same bytes; my capture script reused file names): recaptured. The before shots come from a tool built at main (c20466ca79), the after shots from this branch, on the same throwaway copy of the clone, signed out: ~/x2theme/shots/dtool2-finding3/{inspector,base-rows}-{before,after}.png.
  - Astra 3 (layout coverage): the cells are asserted not to overlap (each starts at or after the last one's right edge, and the controls after the last cell). A narrow width and the expanded, package and override variants are left untested; they share the markup.
  - Astra 4 (the edited-row fixture is impossible on the real server, which makes initial rows uneditable): kept as the defensive case the plan asked for, and said here.
  - Grok 1 (the comment cited review history): reworded to the reason.
  Astra also confirmed the counts match what is hidden, keyboard access and labelled counts, DOM order equal to visual order, and that no uses jump targets a row today.

No BLOCKER found. There are two SHOULD-FIX items and two test-quality nits.

1. **SHOULD-FIX — [app.contract:1120](/tmp/rv-dt-rows1-astra/apps/design-tool/app.contract:1120): expansion survives a selection round trip.** Expand A, select B without expanding it, then select A again: A reopens because `initialFor` still contains A’s key. That does not reset on every selection change. Consecutive reports of the same node correctly preserve expansion; switching nodes should clear the previous expansion.

2. **SHOULD-FIX — [app.contract:1747](/tmp/rv-dt-rows1-astra/apps/design-tool/app.contract:1747), [app.contract:1949](/tmp/rv-dt-rows1-astra/apps/design-tool/app.contract:1949): the cited screenshots do not demonstrate before/after.** `/tmp/dt-rv/i2.png` and `after-i2.png` have identical SHA-256 hashes; so do `s8.png` and `after-s8.png`. Both pairs show the new UI. Preserve distinct parent/HEAD captures to satisfy condition 4.

3. **NIT — [design-tool-scale.test.mjs:75](/tmp/rv-dt-rows1-astra/scripts/design-tool-scale.test.mjs:75): layout coverage is narrower than the acceptance cases.** The sole layout snapshot checks collapsed `space.md` at the default viewport. Expanded platforms, package rows, overrides/Back, and narrow scrolling receive no geometry assertions. Increasing left edges also permit overlapping cells. Assert nonoverlapping bounds and grid overflow at a narrow width. Stretch provenance is untested; the expected-value helper’s `from()` also lacks a stretch case.

4. **NIT — [app.test.contract:2333](/tmp/rv-dt-rows1-astra/apps/design-tool/app.test.contract:2333): the edit-preservation fixture is impossible through the real server.** It combines `source: "initial"` with an editable style rung. [tamber-inspect.mjs:364](/tmp/rv-dt-rows1-astra/scripts/tamber-inspect.mjs:364) always makes initial rows uneditable. The test exercises the defensive comparison, but does not establish preservation through real set-row, set-state, token, or held-state flows.

The remaining checks look correct by inspection:

- Counts exactly match each list’s hidden rows. Native buttons provide keyboard access, and their explicit accessible labels include the counts.
- `r.edit` and `r.state` match the set-row and set-state target formats. The predicate is **not a complete active-edit predicate**: cancellation leaves `editTarget` populated, and `editFromSelection` selects a token through `selToken`. This causes no demonstrated production folding defect because those editable rows are noninitial.
- Uses currently offers text sites and an Outline action; there is no jump targeting an Inspector row. Held-state rows carry source locations and remain visible.
- Grid DOM order follows visual order. Package values, base values, overrides/Back, expanded platforms, and stretch descriptions are outside the name column; identifying metadata appropriately remains inside it. Fixed-width, nonshrinking cells should overflow the grid without wrapping into that column.

Read-only review; no builds or test suites run.
astra exit 0
