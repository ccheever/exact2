# Code review: LLP 1121 stage 2: scheme=fixed, round 3, 2026-10-14 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `5bbf2d9ec97ce37ed65aa24240a476444a794b2942c3ad009ddec167a694fde2`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: four medium, one low; Grok: one high, three medium, one low.
- **Disposition:** applied without re-review except a row's own expression over a fixed token, and two sources at once, named

---

No **high** findings. Four **medium** findings and one **low** remain:

1. **Medium — conditional tokens inherit the default’s mark wholesale.** [theme.rs:713](/tmp/rv-sf3/contract/lower/src/theme.rs:713) checks only `declared.value`, ignoring `whens`. With `base = #fff scheme=fixed`, `ink = base`, and `when mode=b: #222`, `ink` is suppressed even though its override is an unmarked, independent colour. Fix: inherit marks only through unconditional aliases, or track alias provenance per selected tuple.

2. **Medium — mark inheritance misses bare references to prefixed role declarations.** [theme.rs:708](/tmp/rv-sf3/contract/lower/src/theme.rs:708) compares strings rather than role identities. Declaring `-exact-label = #101010 scheme=fixed` and `ink = label` incorrectly warns on `ink`: the checker resolves that alias, but the suppression loop does not. Fix: use the existing `same_name` role normalization. The current test covers only the reverse spelling.

3. **Medium — skipping a profile can silence unrelated token warnings.** [theme.rs:694](/tmp/rv-sf3/contract/lower/src/theme.rs:694) replaces profile values with a differing pair while retaining metadata. For `photo = color(--brand …) scheme=fixed`, normal lowering substitutes `transparent`, but this warning pass creates an invalid fixed pair. `check_design` then fails and returns an empty list, hiding an unrelated `Canvas = #fff`. Fix: share lowering’s profile normalization and exclude profile tokens without invalidating the whole warning fold.

4. **Medium — synthetic row expressions still lose unmarked composite warnings.** [warnings.rs:953](/tmp/rv-sf3/contract/lower/src/warnings.rs:953) records referenced declarations, while [theme.rs:701](/tmp/rv-sf3/contract/lower/src/theme.rs:701) folds no synthetic tokens. With fixed `base`, `color="color-mix(in srgb, base 50%, #000)"` stays silent although its synthetic result is an unmarked `Folded::One`. The equivalent named composite warns correctly. Fix: include synthetic expressions in the folded analysis and report their results at their row spans.

5. **Low — group closers still recommend invalid repairs.** [warnings.rs:1260](/tmp/rv-sf3/contract/lower/src/warnings.rs:1260) and [warnings.rs:1275](/tmp/rv-sf3/contract/lower/src/warnings.rs:1275) offer `scheme=fixed` regardless of each token’s `every` flag. Adding another warning site to the existing “some modes” test restores the recommendation HEAD intended to remove. The mixed closer also omits the inline-theme migration requirement. Fix: preserve eligibility and source restrictions in the closer, or refer to the individual repairs.

[tamber_scheme.rs](/tmp/rv-sf3/contract/cli/tests/it/tamber_scheme.rs) lacks these combinations. Caps and `git diff --check` passed; the worktree is unchanged. Rust tests were inspected but not run: no matching built binaries were available, and building would violate the read-only constraint.
