# Code review: LLP 1121 stage 2: scheme=fixed, round 2, 2026-10-14 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `81bb702df1f2d89935be8f65b313950c7ba67360bda89cdf50056c4947782504`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: one high, two medium, two low; Grok: two high, two medium, two low.
- **Disposition:** fixed in round 2 by judging tokens by their fold

---

Found **1 high, 2 medium, and 2 low issues**.

1. **High — repeated references cause exponential traversal.** [warnings.rs:965](/tmp/rv-sf2/contract/lower/src/warnings.rs:965)  
   Eight levels of `color-mix()` each referencing the next token ten times, ending in `#ffffff`, produce **111,111,111 resolved-entry visits** from a 559-byte design. Depth bounds self-reference and mutual cycles, but does not prevent this explosion. Use canonical entry identities with memoization and cycle detection. This also permits removing the depth cutoff, which currently leaves a literal behind nine alias edges unreported.

2. **Medium — valid light/dark pairs receive an impossible repair.** [warnings.rs:965](/tmp/rv-sf2/contract/lower/src/warnings.rs:965)  
   With `base = #ffffff` and `ink = light-dark(base, #000000)`, a row using `ink` now warns that it is one colour in every mode. The traversal finds `base` without respecting the enclosing pair. Adding the suggested `scheme=fixed` then fails validation. Classify the folded light/dark result, or preserve expression context during traversal.

3. **Medium — a fixed dependency hides an unmarked composite colour.** [warnings.rs:958](/tmp/rv-sf2/contract/lower/src/warnings.rs:958)  
   `base = #ffffff scheme=fixed` followed by `Canvas = color-mix(in srgb, base 50%, #000000)` produces no warning. The fixed dependency stops traversal, while `literal_colours()` ignores the whole mix and the recursive lookup ignores its literal `#000000`. Consequently, unmarked `Canvas` is reported nowhere. Inspect literal operands with scheme context, or classify the folded composite while preserving the intended exemption for aliases.

4. **Low — mixed token/literal summaries still contradict token repairs.** [warnings.rs:1283](/tmp/rv-sf2/contract/lower/src/warnings.rs:1283)  
   One unmarked token plus one literal row reaches the old closer: “name a role for each or leave it unsaid.” That conflicts with the token line’s pair-or-mark repair. Give mixed results a closer that preserves both repairs; retain the existing literal-only wording.

5. **Low — “every mode” overstates what is checked.** [warnings.rs:1232](/tmp/rv-sf2/contract/lower/src/warnings.rs:1232)  
   A token can be identical between light and dark yet vary across an ordinary app axis. Its warning nevertheless claims one colour “in every mode” and lists only its base value. Say “the same in light and dark,” qualifying the applicable app modes where needed.

The equality and duplicate-`scheme` fixes look correct. [tamber_scheme.rs](/tmp/rv-sf2/contract/cli/tests/it/tamber_scheme.rs:139) lacks regression coverage for the cases above.

**Validation:** caps and `git diff --check` passed; worktree unchanged. Rust tests were not run because the review is read-only and this worktree has no built test artifacts.
