# Code review: LLP 1121 stage 3, commit 2, round 3, 2026-10-14 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `aa2f460c8e58f4277c3bc7d96e8163da3bbc305590fa97de6f468044165c0db9`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: four medium, one low; Grok: two medium, one low.
- **Disposition:** applied without re-review

---

## Medium

**Role lookup is not the checker's spelling — `tamber/src/edit.rs:773`**

`token_index` only case-folds when one side starts with `-exact-`. The checker does not work that way. A system role's row spelling is the CSS name (`Highlight`, `Canvas`), and any case of that name is the same override. A design that declares `highlight` is that role, and the IR calls it `Highlight`, but `SetToken`, `RemoveToken`, `UnsetTokenWhen`, and `before` all report no token.

The same function also aliases too much in the other direction. `-exact-canvas` is not a spelling of `Canvas` (`role_key` rejects it as an unknown role), and `-exact-ink` is not a spelling of a normal token `ink`. Both still resolve and edit that token, and the result checks.

Resolve names with the checker's rule (exact declared name, otherwise the same role under `role_key`, case-insensitive, and only then). Do not strip `-exact-` off an arbitrary token. The rename refusal at `edit.rs:153` has the same gap in reverse: `-exact-label` on a token declared `label` is refused, but the diagnostic is line 1 because that lookup is exact.

**A transaction can still rename a channel, space, or platform selector — `tamber/src/edit.rs:177`**

The fold comparison runs only when both the text before the rename and the text after it check. A failed check is treated as "nothing changed," and `run` does not check until every op has been applied. This transaction gets through:

1. `add-token` of a value the checker refuses (`not-a-color` passes `valid`).
2. `rename-token` `r` to `b` inside `rgb(from #112233 r g b)`. The preimage no longer checks, so the guard skips. The channel word still changes.
3. `remove-token` of the temporary token.

The final file checks, and the folded color is different. The same shape works for a colour space (`oklab`) or a platform selector (`ios`, `systemRedColor`). One rename on a file that already checks is still refused.

Treat a rename as unproved unless both sides check and the folded values are equal. A later op must not be able to repair the check after the rename has already been applied to a text that did not check.

## Low

**Duplicate `when` conditions match the wrong line — `tamber/src/edit.rs:796`**

`same_conds` requires equal length and then `contains`. An op whose conditions are `[(tone, soft), (tone, soft)]` matches a real line `tone=soft` plus any second condition. `set-token` rewrites that line's value, and `unset-token-when` removes it. The file still checks. Compare them as sets (reject a repeated axis in `valid`, which the checker already refuses on a real line).

## What holds

`apply` and `apply_all` refuse `Restore`. `undo` accepts only restores. The restore range uses `checked_add` and rejects a zero line. A value cannot carry a trailing comment, a trailing `name=value`, or a top-level `in` (a `color-mix(in …)` stays one word, so it is still allowed). Metadata must be a single `name=value` word, and `label="two words"` still applies. Role renames are refused from the vocabulary even when the design does not check. A token named `when` is renamed, including its uses. A single rename on a checking file that would change a folded channel is refused.
