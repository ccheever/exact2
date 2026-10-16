# Code review: LLP 1121 stage 3, commit 2 (edit operations and inverses), 2026-10-14 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `885dee64ffe5a0eccb48f38867ec1b3b6eedef2396a2d94e2bc3abf8570e10e5`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: five high, four medium, two low; Grok: three high, one medium, three low.
- **Disposition:** fixed in round 1 (the inverse became a restore of the changed lines)

---

Found 5 high, 4 medium, and 2 low findings in `8b3591cac`.

1. **High — Rename changes text that is not a token reference.** [tamber/src/edit.rs:383](/tmp/rv-s3c2/tamber/src/edit.rs:383)  
   Renaming `fff` to `abc` also changes literal `#fff` to `#abc`. Renaming a token named `hard` to `soft` changes `when tone=hard` conditions. Both can pass checking while changing unrelated colors. The scanner also mishandles escaped quotes, allowing replacements inside quoted metadata. **Fix:** replace only declaration names and resolved reference spans; use the lexer’s string/escape handling.

2. **High — Role renames miss aliases and can produce unusable inverses.** [tamber/src/edit.rs:363](/tmp/rv-s3c2/tamber/src/edit.rs:363)  
   Renaming a declared `label` to `brand.ink` leaves references spelled `-exact-label`; those silently fall back to the platform role. Renaming `-exact-label` to `brand.ink` succeeds, but its inverse rejects `-exact-label` because `is_name` excludes the prefix. **Fix:** use the checker’s role identity and accepted spellings for validation and reference replacement.

3. **High — Operation fields can introduce additional declarations.** [tamber/src/edit.rs:225](/tmp/rv-s3c2/tamber/src/edit.rs:225), [edit.rs:414](/tmp/rv-s3c2/tamber/src/edit.rs:414)  
   A `SetToken.value` of `"#000\n  extra = #fff"` creates another token; checking succeeds, and undo leaves `extra` behind. Similarly, `AddAxisValue.value = "dim | oled"` adds two values, but its inverse cannot find that combined name. **Fix:** validate identifiers, CSS values, metadata and comments as their respective field types before interpolation. Whole-file checking cannot enforce the JSON operation’s scope.

4. **High — Appending a token can take another token’s `when`.** [tamber/src/edit.rs:454](/tmp/rv-s3c2/tamber/src/edit.rs:454)  
   An unindented comment is legal between a token and its indented `when`, including in formatted output. `block_end` stops at that comment, so `AddToken` inserts before the existing `when`, attaching it to the new token. Undo can then delete that original condition. **Fix:** determine block boundaries from parsed code lines, not comment indentation.

5. **High — Deletion inverses lose comments and source positions.** [tamber/src/edit.rs:349](/tmp/rv-s3c2/tamber/src/edit.rs:349), [edit.rs:278](/tmp/rv-s3c2/tamber/src/edit.rs:278)  
   `RemoveToken` drains comments and blank lines between the declaration and its last `when`, but records none of them. Its `before` anchor also moves restoration across intervening comments/blanks. `UnsetTokenWhen` records only a sibling index, so undo inserts before a surviving comment that originally preceded the removed condition. **Fix:** retain the exact removed text and insertion position in the inverse.

6. **Medium — Metadata reconstruction breaks even formatted-file undo.** [tamber/src/edit.rs:505](/tmp/rv-s3c2/tamber/src/edit.rs:505)  
   Removing and restoring `label="Ink"` produces `label=Ink`. `label=""` produces invalid `label=`, and an escaped `\n` becomes a physical newline. **Fix:** preserve metadata’s original spelling in the preimage; any serializer must also handle empty strings and control-character escapes.

7. **Medium — Creating the first theme block is not undone.** [tamber/src/edit.rs:324](/tmp/rv-s3c2/tamber/src/edit.rs:324)  
   Adding a token to `tamber t\n` creates a blank line and `theme` header. The inverse removes only the token, leaving the new block. Original trailing blanks and final-newline state are also discarded. **Fix:** include block creation and the original ending in the inverse.

8. **Medium — Unordered condition matching loses the original order.** [tamber/src/edit.rs:241](/tmp/rv-s3c2/tamber/src/edit.rs:241)  
   Updating `when x=a y=b` with conditions supplied as `y=b, x=a` finds the correct line, but both the replacement and inverse use the caller’s order. Undo therefore fails byte equality on formatted input. **Fix:** retain `w.conds` for an existing condition line.

9. **Medium — Accepted unformatted and CRLF input cannot round-trip.** [tamber/src/edit.rs:170](/tmp/rv-s3c2/tamber/src/edit.rs:170)  
   `parts` discards carriage returns and spacing; setters reconstruct the line. Undo restores the value but leaves normalized spacing and LF endings, potentially producing mixed line endings. **Fix:** retain raw touched-line preimages, including terminators.

10. **Low — Edit refusals always point to `1:1`.** [tamber/src/edit.rs:140](/tmp/rv-s3c2/tamber/src/edit.rs:140)  
    Duplicate-token and missing-condition/value errors have a relevant declaration, but discard its location. **Fix:** pass that declaration’s span into `refuse`; reserve `1:1` for errors without a source location.

11. **Low — The dial test never changes a dial’s value.** [tamber/tests/edit.rs:196](/tmp/rv-s3c2/tamber/tests/edit.rs:196)  
    It appends a space that `apply` trims, so an implementation ignoring the requested value would pass. **Fix:** set a different valid value and assert the changed line and inverse. Add regressions for the cases above and a multi-operation `apply_all` sequence containing insertions.

On inspection, `apply_all` reparses between operations, avoiding stale line numbers, and ordinary dangling token references are refused by the checker.

Caps, the pinned Rust formatter check, and commit whitespace checks passed. Rust tests were not run because this read-only worktree has no build artifacts. No files were changed.
