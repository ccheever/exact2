# Code review: LLP 1121 stage 3, commit 2, round 3, 2026-10-14 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `aa2f460c8e58f4277c3bc7d96e8163da3bbc305590fa97de6f468044165c0db9`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: four medium, one low; Grok: two medium, one low.
- **Disposition:** applied without re-review

---

No **high** findings. Four **medium** findings and one **low** remain.

1. **Medium — Equal current folds do not make a rename safe.** [tamber/src/edit.rs:181](/tmp/rv-s3c2c/tamber/src/edit.rs:181)  
   With `seed: color = #808080`, `r = #ff0000`, and `ink = rgb(from seed r g b)`, renaming `r` to `b` passes: the current channels are equal. It nevertheless changes the expression to `rgb(from seed b g b)`, corrupting its behavior when `seed` later changes. Unselected defaults and `when` values are also absent from this comparison. **Fix:** rename actual reference positions, or refuse ambiguous keyword matches regardless of their current folded values.

2. **Medium — Rename protection still fails open inside transactions.** [tamber/src/edit.rs:183](/tmp/rv-s3c2c/tamber/src/edit.rs:183)  
   Starting with `r = #ff0000` and `ink = rgb(from #123456 r g b)`, a transaction can add `pending = future`, rename `r` to `b`, then add `future = #ffffff`. The unresolved forward reference makes `folded(src)` return `None`, skipping protection. The final check succeeds with `ink` unintentionally changed. **Fix:** refuse a rename when either safety check fails, or use structural reference rewriting that works independently of intermediate checker errors.

3. **Medium — Role lookup both misses valid spellings and aliases ordinary tokens.** [tamber/src/edit.rs:782](/tmp/rv-s3c2c/tamber/src/edit.rs:782)  
   A valid declaration `canvas = #ffffff` produces IR name `Canvas`, but editing `Canvas` reports “no token.” Likewise, canonical `-exact-label` cannot find a declaration spelled `-exact-LABEL`. Conversely, `RemoveToken("-exact-ink")` deletes ordinary token `ink`, although no such role exists. **Fix:** resolve role identities through the checker’s vocabulary and canonicalization; keep ordinary token matching exact.

4. **Medium — Successful repairs can return unusable inverses.** [tamber/src/edit.rs:330](/tmp/rv-s3c2c/tamber/src/edit.rs:330)  
   Given `ink = missing`, setting `ink` to `#000000` succeeds. Applying its returned inverse restores `ink = missing`, then fails the final checker, so `undo` never returns the original text. **Fix:** either require a valid initial design before accepting edits, or allow trusted inverse restoration of the original preimage even when it contains checker errors.

5. **Low — Token removal can break canonical formatting.** [tamber/src/edit.rs:606](/tmp/rv-s3c2c/tamber/src/edit.rs:606)  
   In a formatted theme with a blank line between its first and second tokens, removing the first leaves a blank immediately after `theme`; removing a middle token can leave consecutive blanks. Consequently, `format(result) != result`. **Fix:** remove redundant neighboring blank lines during the splice; the existing Restore capture can preserve them for undo.

The 11 existing edit tests passed using a prebuilt binary with matching Tamber sources and fixtures. These additional cases were traced in source. No files were changed.
