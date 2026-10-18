# Code review: LLP 1121 stage 2: scheme=fixed, round 2, 2026-10-14 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `81bb702df1f2d89935be8f65b313950c7ba67360bda89cdf50056c4947782504`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: one high, two medium, two low; Grok: two high, two medium, two low.
- **Disposition:** fixed in round 2 by judging tokens by their fold

---

## Summary

Round-1 fixes 1, 3, 4, 5, and 6 hold in the resulting code: a fixed token is recognised under both its written name and one `-exact-` spelling (`entry_colours_at` compares the resolved key and `strip_prefix("-exact-")` to `fixed_tokens`); `same` treats equivalent colours as equal (`#ffffff` and `color(srgb 1 1 1)`, and `light-dark` of two equal sides); a second `scheme=` is refused as `tamber-scheme-fixed`; token sites again use the token repair; a token-only closer does not say "literal colours", and a literal-only file still produces "these 25 literal colours" (`contract/cli/tests/it/warnings.rs`). Round-1 fix 2 holds for the case it targets (a one-scheme token inside `color-mix` is reported, including through a role) but the walk that implements it also mis-reports scheme-varying values (Issue 1) and drops an unmarked one-scheme expression whose only token is fixed (Issue 2). An empty `scheme=` is still not metadata (`meta_word` requires a non-empty value, `tamber/src/syntax.rs:406`) and stays in the CSS text, so `fold_at` reports `tamber-syntax`, not `tamber-scheme-fixed`. The walk is capped (`entry_colours` starts at depth 8 and still reads the node at 0) so a cycle terminates without hanging or overflowing; there is no visited set, and a chain longer than that cap is not a realistic theme.

## Issues

### Issue 1 -- Severity: bug
- File: contract/lower/src/warnings.rs:965
- Description: Severity high. A scheme-varying token is warned as one colour in every mode when its default text merely names a one-scheme token. `entry_colours_at` unions `literal_colours` of every nested entry, splitting on parentheses, and never asks whether the value folds to `Folded::Pair`. `ink = light-dark(base, #111111)` with `base = #ffffff`, referenced by a row or used as a role override, produces no top-level literal (`light-dark` is not a colour function in `literal_colour`) and then recurses into `base`, so the warning is `` `ink` is one colour in every mode (`#ffffff`) ``. The fold is a pair (`tamber/src/fold.rs:271`). The previous alias walk split only on whitespace and commas, so `light-dark(base, #000)` did not resolve `base`; the new splitter does. The same walk cites an ingredient rather than the resolved colour when the result really is one colour: `Canvas = color-mix(in srgb, base 50%, #000000)` is warned as `` (`#ffffff`) ``, which is `base`, not the mix. `contract/cli/tests/it/tamber_scheme.rs` locks that color-mix case and does not cover `light-dark`.
- Suggestion: Decide from the folded value (every app tuple `Folded::One`, and the token not fixed) instead of unioning nested literals. Do not descend into `light-dark()`'s sides. If a text walk stays, cite the folded colour, not an ingredient.
- Status: open

### Issue 2 -- Severity: bug
- File: contract/lower/src/warnings.rs:958
- Description: Severity high. Stopping at a fixed token drops an unmarked one-scheme parent whose colours come only from that token. The fixed check returns before `literal_colours`, and raw colour arguments are not entries, so they are never recorded. `white = #ffffff scheme=fixed` and `grey = color-mix(in srgb, white 50%, #000000)` folds to one colour, is not marked fixed, and is named by a row or is a role such as `Canvas`, but `entry_colours("grey")` follows `white`, returns with nothing, and does not see `#000000`. No `lower-colour-one-scheme` line is emitted. A pure alias (`ink = -exact-label` with `label` fixed) staying silent is intended and is what `tamber_scheme.rs` locks; this is the expression case, which that test does not cover (its inner `base` is not fixed).
- Suggestion: Keep suppressing the fixed token's own site, but judge the parent from its folded `Folded::One` result. Collecting the fixed child's literals into the parent would also reintroduce Issue 1 for `light-dark(white, #000)`.
- Status: open

### Issue 3 -- Severity: bug
- File: contract/lower/src/warnings.rs:1232
- Description: Severity medium. The token repair offers `scheme=fixed` for a token that is one colour only in the default text, while another tuple is a pair, so the mark is refused. `theme_decl` keeps only the default value (`contract/lower/src/theme.rs:661`), ignoring `when` lines. `same.white = #ffffff` with `when dark-style=dim: light-dark(#ffffff, #161e27)` on an axis `under scheme=dark` (as in `tamber/tests/corpus/axes.tamber`) is `Folded::One` for the default tuple and `Folded::Pair` for dim (`under_value` in `tamber/src/check.rs:833`). The warning says it is one colour in every mode and tells the author to mark `scheme=fixed`. `Checker::scheme` (`tamber/src/check.rs:195`) then errors `tamber-scheme-fixed` because the dim sides differ. Adding the mark removes the warning (`fixed_tokens` reads the meta) and fails the compile.
- Suggestion: Offer `scheme=fixed` only when every folded tuple is one colour. Otherwise say that a `when` on an `under scheme=dark` axis still changes the dark side, and do not suggest a mark the checker will refuse.
- Status: open

### Issue 4 -- Severity: bug
- File: contract/lower/src/warnings.rs:1283
- Description: Severity medium. A file with both token sites and literal sites still gets the literal closer, which contradicts the token lines. The token-only branch (`sites.iter().all(|s| s.token)`, line 1268) is correct, and a literal-only closer still includes "literal ". If any site is not a token, the closer is "these N colours stay the same … name a role for each or leave it unsaid", with N equal to every site. Token lines say to give a `light-dark` pair or mark `scheme=fixed`. A design that still has a hex on a row plus an unmarked one-scheme token — the usual migration — gets both repairs in one report.
- Suggestion: Use the token closer for the token sites only, or a mixed closer that does not tell the author to name a role for a token. Keep "these N literal colours" when every site is a literal.
- Status: open

### Issue 5 -- Severity: nit
- File: contract/lower/src/warnings.rs:1182
- Description: Severity low. `leaves.named.borrow().clone()` copies every referenced token name only to drop the `Ref` before `entry_colours`. That call does not borrow `named` (only `colours` on a row string does), so the clone is unnecessary.
- Suggestion: Borrow `named` for the `spellings.contains` test and release it before `entry_colours`, or record names in a plain `Vec` owned by `one_scheme`.
- Status: open

### Issue 6 -- Severity: nit
- File: contract/lower/src/warnings.rs:1234
- Description: Severity low. A token line prints at most three colours and, unlike `shown` (line 1215), adds no ellipsis when there are more. A walk that collects four ingredient colours hides the rest with no mark that it did.
- Suggestion: Match the literal line: after three colours, append `…`.
- Status: open
