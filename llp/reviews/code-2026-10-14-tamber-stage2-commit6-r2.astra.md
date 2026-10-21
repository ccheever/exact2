# Code review: LLP 1121 stage 2, commit 6 (the web's app modes), round 2, 2026-10-14 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `b6347d338183ffb0096f9c8792abc419ef3befe8ecc767f84a731af74b035cb7`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: seven medium, one low; Grok: one high, one medium (not a defect: the match includes the closing parenthesis), two low.
- **Disposition:** fixed in the round-2 commit; the RFC diff was main having moved, answered by a rebase

---

No high-severity findings. I found **seven medium-severity issues and one low-severity issue**.

1. **Medium — Literal clears still block inherited token rows.** [host/web-js/src/modes.rs:190](/tmp/rv-s2c6r2/host/web-js/src/modes.rs:190)  
   Static `color="unset"`, `"inherit"` and `"currentcolor"` project to `color:unset;`. This loop nevertheless sets the ownership bit and records channel `-1`. A pinned boundary then keeps its parent’s computed colour instead of re-selecting the token; an intermediate cleared ancestor also blocks re-specification below it.  
   **Fix:** Exclude declarations that `StyleValue::text_unsets` treats as absent from both ownership bits and channel metadata. Test clears on boundaries and intervening ancestors.

2. **Medium — Changing between cleared values loses the implied row.** [host/web-js/modes.js:79](/tmp/rv-s2c6r2/host/web-js/modes.js:79), [host/web-js/modes.js:120](/tmp/rv-s2c6r2/host/web-js/modes.js:120)  
   After a boundary’s bound colour becomes `unset`, the runtime correctly writes its inherited token. Changing that binding to `inherit` or `null` overwrites/removes that inline declaration, but `$xi` remains `undefined` and `$xs` still says the implied text is installed. Subsequent passes skip restoration. The in-memory probe reproduced this.  
   **Fix:** Invalidate the implied-row cache whenever a binding writes over it, including transitions between absent values.

3. **Medium — Server rendering restores cleared rows before the binding finishes.** [host/web-js/modes.js:102](/tmp/rv-s2c6r2/host/web-js/modes.js:102)  
   `Sx` calls `schedule()` while evaluating the value passed to `S`. During rendering, `schedule()` immediately runs `apply()`. On an own-colour → `unset` transition, `apply()` writes the inherited token, then `S` overwrites it with `unset`. The later `After` pass trusts `$xs` and leaves the wrong rendered colour. This also reproduced in memory.  
   **Fix:** Resolve render boundaries after style writes finish, through the commit’s `After` pass or an equivalent deferred flush.

4. **Medium — The accent cycle guards discard valid accent values.** [host/web-js/modes.js:96](/tmp/rv-s2c6r2/host/web-js/modes.js:96), [host/web-js/src/rows.rs:369](/tmp/rv-s2c6r2/host/web-js/src/rows.rs:369), [host/web-js/src/modes.rs:216](/tmp/rv-s2c6r2/host/web-js/src/modes.rs:216)  
   A varying `brand` token containing only literal colours creates no cycle. Nevertheless, its re-specified accent omits `--exact-accent`, and a bound accent on a boundary explicitly drops that companion. Thus a pin can change `accent-color` while descendant `AccentColor` uses keep the ancestor’s colour. The static guard also drops the companion in literal-valued tuples merely because another tuple names the accent.  
   **Fix:** Suppress actual self-dependencies for the effective tuple; retain companion writes for safe token values. Test descendants explicitly using `AccentColor`, rather than the same brand token.

5. **Medium — Replaying the stroke shorthand overwrites authored widths.** [host/web-js/src/rows.rs:365](/tmp/rv-s2c6r2/host/web-js/src/rows.rs:365), [host/web-js/modes.js:95](/tmp/rv-s2c6r2/host/web-js/modes.js:95)  
   An ancestor’s bound `-webkit-text-stroke="2px ink"` is recorded and replayed whole for channel 6. A boundary with its own `-webkit-text-stroke-width="5px"` consequently receives an inline shorthand resetting its width to `2px`. The wasm implied row writes only `TextStrokeColor`.  
   **Fix:** Track and re-specify the shorthand’s colour longhand separately. Add a boundary test with an explicitly different width.

6. **Medium — Rejected computed colours are recorded as owning the row.** [host/web-js/modes.js:117](/tmp/rv-s2c6r2/host/web-js/modes.js:117)  
   A bound value such as `"not-a-colour"` receives channel `-1` before the style writer validates it. The writer subsequently falls back to `unset`, but the channel still suppresses the boundary’s inherited token. The runner treats a refused binding as setting no row.  
   **Fix:** Determine ownership from the accepted style result; rejected values must follow the absent-row path.

7. **Medium — Dynamic themed texts accumulate without bound.** [host/web-js/modes.js:118](/tmp/rv-s2c6r2/host/web-js/modes.js:118)  
   Each distinct computed themed shadow or stroke string is permanently appended to `Texts`, with a linear `indexOf` search on every update. A shadow containing a changing offset and a token therefore consumes increasing memory and lookup time. A probe retained 2,048 texts after removing its only element.  
   **Fix:** Store dynamic text on the live element/channel, retaining only its current value; keep the shared table for compiled texts.

8. **Low — The RFC regresses accepted documentation.** [llp/1121-tamber.rfc.md:499](/tmp/rv-s2c6r2/llp/1121-tamber.rfc.md:499)  
   The diff restores the rejected `axis:value` spelling, regresses the status line, and removes the stage 2 commits 2–5 implementation notes.  
   **Fix:** Restore those sections and the accepted `axis=value` examples from `origin/main`.

The added Bun test passes: **1 test, 17 assertions**. I found no additional issue in tuple ordering, sentinel substitution, marker/name collision handling, or the no-mode import gate. Browser/Cargo conformance was not run because it would write artifacts. No files changed.
