# Code review: exact-tamber crate, round 2, 2026-10-13 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `8646745a97ff601115082c4db233a1f75295d798106fd62966edcc5ce5ad11ec`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** fix from both (Astra 5 MAJOR, 4 MINOR; Grok 1 MINOR and three edges).
- **Disposition:** all fixed in round 3: typed percentages, more probes, the kernel platform-name rule, unclamped relative components, powerless hues refused, typed legacy commas, hidden under-dark whens, no inert report beside other refusals, Contract escapes, a zero-sum mix, longer hue at -180

---

Fix required. The source review found these remaining issues:

1. **MAJOR — percentages lose their type before arithmetic.** [color_fn.rs:103](/Users/admin/x2p2/apps/exact2-tamber/tamber/src/color_fn.rs:103) converts percentages to `Num::N`, so invalid sums such as `rgb(calc(10 + 50%) 0 0)` are accepted. The new [relative.tamber:19](/Users/admin/x2p2/apps/exact2-tamber/tamber/tests/corpus/relative.tamber:19) and its golden actually bless this error. Percentages must retain their type through the calculation and resolve against the channel afterward. [CSS Values 4](https://www.w3.org/TR/css-values-4/#calc-type-checking)

2. **MAJOR — the influence probes reject working dials.** [check.rs:630](/Users/admin/x2p2/apps/exact2-tamber/tamber/src/check.rs:630) tests only endpoints or opaque black/white. A dial `hue: number = 0 in 0..360` used by `hsl(hue 100% 50%)` is reported inert because both endpoints are red. Similarly, `rgb(from accent 0 0 0 / alpha)` cannot demonstrate dependence when both color probes are opaque. These samples cannot justify a hard constancy diagnosis.

3. **MAJOR — platform validation still emits values the kernel refuses.** [fold.rs:335](/Users/admin/x2p2/apps/exact2-tamber/tamber/src/fold.rs:335) accepts `-exact-platform-color(ios bogus, #000)`, but [roles.rs:351](/Users/admin/x2p2/apps/exact2-tamber/kernel/src/style/roles.rs:351) requires `native_name_ok`. Conversely, the new identifier/function-only filtering rejects valid `-exact-platform-color(web #fff, #000)`. The fallback fix is correct; platform argument validation remains incomplete.

4. **MAJOR — relative colors still clamp intermediate components.** [color_fn.rs:292](/Users/admin/x2p2/apps/exact2-tamber/tamber/src/color_fn.rs:292) clamps Lab/Oklab lightness and polar chroma. Starting with `oklch(.8 .1 20)`, applying relative `calc(l + .5)` and then relative `calc(l - .5)` returns lightness `.5`, rather than `.8`. Relative components must remain unclamped through further folding; alpha is the exception. [CSS Color 5](https://www.w3.org/TR/css-color-5/#relative-colors)

5. **MAJOR — implicitly missing components escape the `none` refusal.** [color_fn.rs:285](/Users/admin/x2p2/apps/exact2-tamber/tamber/src/color_fn.rs:285) returns numerical conversion results without detecting powerless components. For example, `oklch(from white .5 .2 h)` uses a hue produced by floating-point noise instead of preserving—or explicitly refusing—the missing hue. Rejecting the literal word `none` does not cover missing components created by conversion. [CSS Color 4](https://www.w3.org/TR/css-color-4/#powerless)

6. **MINOR — valid legacy percentage calculations are refused.** [color_fn.rs:368](/Users/admin/x2p2/apps/exact2-tamber/tamber/src/color_fn.rs:368) checks whether arguments are literal `Item::Percent`s rather than percentage-valued calculations. Consequently, `hsl(0, calc(100%), 50%)` and `rgb(calc(50%), 0%, 0%)` fail. Classify their evaluated types. [CSS Values 4](https://www.w3.org/TR/css-values-4/#calc-type-checking)

7. **MINOR — hidden `when` validation remains incomplete.** [check.rs:590](/Users/admin/x2p2/apps/exact2-tamber/tamber/src/check.rs:590) only calls `fold_at`, bypassing `under_value`. An under-dark branch with an incorrect light side—or a bare role—passes when more-specific branches exhaustively hide it. Apply the same contextual checks to hidden branches.

8. **MINOR — unrelated errors produce false inert-dial diagnostics.** [check.rs:121](/Users/admin/x2p2/apps/exact2-tamber/tamber/src/check.rs:121) skips influence whenever any error exists, then calls completeness with empty usage sets. Thus a valid `brand = accent` receives an inert `accent` diagnostic merely because another token is invalid.

9. **MINOR — metadata escapes are corrupted.** [lex.rs:146](/Users/admin/x2p2/apps/exact2-tamber/tamber/src/lex.rs:146) strips every backslash. `help="a\nb"` becomes `anb`; Contract decodes the newline and rejects unknown escapes. Escaped quotes now work, but the decoder does not implement Contract strings.

Refusing explicit `none` is defensible as the declared temporary limitation, subject to finding 5. **Zero-sum refusal is a CSS incompatibility:** current CSS Color 5 defines the two-color case as an equal mix with zero alpha. [color_fn.rs:540](/Users/admin/x2p2/apps/exact2-tamber/tamber/src/color_fn.rs:540) should not describe it as invalid CSS. [Specification](https://www.w3.org/TR/css-color-5/#color-mix-result)

The relative sRGB scaling, retained legacy precision, direct sRGB mixing, host-axis/currentcolor refusals, bare-role case handling, and `when`/`under` spacing fixes look correct.

`cargo test -p exact-tamber` was blocked by the read-only Cargo lock. I ran the existing binaries: **all seven tests passed**, including corpus and parity. The additional cases above are source-derived. The worktree remains unchanged.

VERDICT: fix (MAJOR — tamber/src/color_fn.rs:103; tamber/src/check.rs:630; tamber/src/fold.rs:335)
