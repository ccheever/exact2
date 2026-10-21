# Code review: exact-tamber crate, round 1, 2026-10-13 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `3053457b2862e5f1d5a9511b0468827b8591f7146ad4c8f579572e1ab359d6b1`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** fix from both (Astra 10 MAJOR, 8 MINOR; Grok 2 MAJOR, 6 MINOR).
- **Disposition:** all fixed in round 2 but two kept as declared limits: missing components (none) refused, not folded; color-mix needs in <space>

---

This needs fixes before shipping. Findings are from source tracing; `cargo test -p exact-tamber` failed because the read-only sandbox prevented Cargo from creating build files. No files changed.

1. **MAJOR — Relative `color(srgb …)` uses RGB’s 0–255 scale.** [fold.rs:661](/Users/admin/x2p2/apps/exact2-tamber/tamber/src/fold.rs:661) scales channels whenever the space is sRGB, including `color(from … srgb …)`, then divides the result by 255. Consequently, `color(from red srgb 1 0 0)` produces red at `1/255` instead of `1`. Scaling must depend on the function, not merely its space. [CSS Color 5](https://drafts.csswg.org/css-color-5/#relative-color-function)

2. **MAJOR — Absolute colour calculations use the wrong percentage reference.** [fold.rs:324](/Users/admin/x2p2/apps/exact2-tamber/tamber/src/fold.rs:324) evaluates every calculation with reference `1.0`. Thus `rgb(calc(50%) 0 0)` becomes `rgb(0.5 0 0)`, ultimately `#010000ff`, instead of `#800000ff`. HSL and Lab calculations suffer analogous errors. References must follow the function and channel. [CSS Color 4](https://drafts.csswg.org/css-color-4/#rgb-functions)

3. **MAJOR — Arithmetic discards CSS types and accepts invalid expressions.** [fold.rs:452](/Users/admin/x2p2/apps/exact2-tamber/tamber/src/fold.rs:452) converts percentages and angles directly to numbers. It accepts angles in RGB channels, percentages as hues, and incompatible additions. The `calc(h + 0.5turn)` input in [relative.tamber:12](/Users/admin/x2p2/apps/exact2-tamber/tamber/tests/corpus/relative.tamber:12), blessed by `relative.json:16`, is invalid: `h` is a number. The corresponding invalid cases are explicitly covered by [WPT](https://raw.githubusercontent.com/web-platform-tests/wpt/master/css/css-color/parsing/color-invalid-relative-color.html).

4. **MAJOR — Legacy inputs are quantized before the final fold.** [fold.rs:331](/Users/admin/x2p2/apps/exact2-tamber/tamber/src/fold.rs:331) immediately stores absolute RGB/HSL/HWB values as `Rgba8`. For example, `rgb(from rgb(0.5 0 0) r g b)` retains `1/255`, losing the original `0.5/255`, although its result is Wide. This violates §3’s requirement to quantize only the final legacy result.

5. **MAJOR — Mixing loses missing-component semantics.** [fold.rs:558](/Users/admin/x2p2/apps/exact2-tamber/tamber/src/fold.rs:558) receives colours whose `none` components have already become zero through `exact_color::parse`. `color-mix(in srgb, color(srgb 1 none 0), lime)` consequently averages green to approximately `0.5`; it should retain green `1` by borrowing the other endpoint’s component. Preserve missing components until interpolation. [CSS Color 4](https://drafts.csswg.org/css-color-4/#interpolation-missing)

6. **MAJOR — Valid `color()` dial defaults are rejected.** [fold.rs:319](/Users/admin/x2p2/apps/exact2-tamber/tamber/src/fold.rs:319) resolves the colour-space identifier through `Names`. For `accent: color = color(display-p3 1 0 0)`, `Literal::named` rejects `display-p3` as a nonliteral name. Colour-space identifiers need syntactic handling, independently of dial/token lookup.

7. **MAJOR — Platform colours bypass checking and lose their fallback.** [fold.rs:272](/Users/admin/x2p2/apps/exact2-tamber/tamber/src/fold.rs:272) accepts even `-exact-platform-color()` without checking arguments, required fallback, or references inside it. Separately, [lib.rs:133](/Users/admin/x2p2/apps/exact2-tamber/tamber/src/lib.rs:133) returns no fallback for valid platform colours. [literals.json:24](/Users/admin/x2p2/apps/exact2-tamber/tamber/tests/corpus/literals.json:24) wrongly blesses `fallback: null` despite an explicit source fallback pair. This violates D7’s existing-form and fallback requirements.

8. **MAJOR — `currentcolor` escapes into an IR with no matching kernel form.** [fold.rs:232](/Users/admin/x2p2/apps/exact2-tamber/tamber/src/fold.rs:232) admits it, but the kernel’s `ColorValue` has no Current variant. The IR also has no fallback. [parity.rs:67](/Users/admin/x2p2/apps/exact2-tamber/tamber/tests/parity.rs:67) explicitly skips this case, masking the incompatibility. It needs a supported lowering or a refusal.

9. **MAJOR — Host traits can be declared as app axes.** [check.rs:167](/Users/admin/x2p2/apps/exact2-tamber/tamber/src/check.rs:167) accepts `scheme = light | dark` and `contrast = normal | more`; their `when` lines then select ordinary app tuples. D5 explicitly excludes both, including authored contrast variants. The existing unknown-axis test does not cover declaring these names.

10. **MAJOR — `tamber-dial-inert` checks mentions, not influence.** [check.rs:642](/Users/admin/x2p2/apps/exact2-tamber/tamber/src/check.rs:642) accepts a colour dial used only in `light-dark(#fff, light-dark(accent, #000))`. The resulting token is always white/black; changing `accent` cannot move it. Reading a discarded branch marks the dial used and defeats D4’s completeness check.

11. **MINOR — The narrowed reserved-name rule admits CSS-wide keywords.** [check.rs:160](/Users/admin/x2p2/apps/exact2-tamber/tamber/src/check.rs:160) permits tokens named `inherit`, `initial`, `unset`, and `revert`. D3 reserves CSS keywords, and these names already have row semantics. The commit’s colour-keywords-only deviation is insufficient.

12. **MINOR — Values hidden by exhaustive `when` coverage are never checked.** [check.rs:534](/Users/admin/x2p2/apps/exact2-tamber/tamber/src/check.rs:534) only folds selected values. With `m = a | b` and valid branches for both values, a token’s default can contain `rgb(` or an unknown name and still pass. Validate every declared value before selecting tuple results.

13. **MINOR — Accepted bare role spellings do not consistently resolve.** [check.rs:714](/Users/admin/x2p2/apps/exact2-tamber/tamber/src/check.rs:714) fails for `Label = #fff` followed by `text.ink = Label`: declaration canonicalizes the role to lowercase aliases, but lookup’s case-insensitive path only permits keys present in `roles`, which contains `-exact-label`, not `label`.

14. **MINOR — The replacement lexer/parser does not preserve Contract’s lexical rules.** [lex.rs:94](/Users/admin/x2p2/apps/exact2-tamber/tamber/src/lex.rs:94) toggles quoting on escaped quotes, so valid metadata such as `label="say \"hello world\""` splits incorrectly. [ir.rs:104](/Users/admin/x2p2/apps/exact2-tamber/tamber/src/ir.rs:104) also strips quotes without decoding escapes. Separately, [syntax.rs:491](/Users/admin/x2p2/apps/exact2-tamber/tamber/src/syntax.rs:491) rejects `when m = a: …`, and axis parsing rejects `under scheme = dark`, despite §3’s token grammar.

15. **MINOR — A malformed dial can disappear without a diagnostic.** [syntax.rs:323](/Users/admin/x2p2/apps/exact2-tamber/tamber/src/syntax.rs:323) returns `None` silently when `=` occurs before `:` but none follows it. A line such as `bad= : color` is dropped and the otherwise valid design succeeds.

16. **MINOR — Refused declarations generate misleading downstream errors.** [check.rs:380](/Users/admin/x2p2/apps/exact2-tamber/tamber/src/check.rs:380) excludes refused tokens from the name table. Referencing a declared `length` token subsequently reports an unknown token. Refused dials behave similarly; [refuse-dials.err:9](/Users/admin/x2p2/apps/exact2-tamber/tamber/tests/corpus/refuse-dials.err:9) blesses that cascade. Preserve a refused-symbol entry and suppress derivative errors.

17. **MINOR — Mix admission follows obsolete/restricted syntax.** [fold.rs:550](/Users/admin/x2p2/apps/exact2-tamber/tamber/src/fold.rs:550) rejects two zero percentages; current CSS Color 5 defines a transparent result. The mandatory `in` and exactly-two-colour restrictions also narrow §3’s claimed support. Record that subset in the RFC or implement the current grammar. [CSS Color 5](https://drafts.csswg.org/css-color-5/#color-mix)

18. **MINOR — The mix golden freezes a rounding error.** [mix.json:9](/Users/admin/x2p2/apps/exact2-tamber/tamber/tests/corpus/mix.json:9) records an equal red/blue mix as `0.49999999999999994` with fallback `#7f007fff`; the exact midpoint’s nearest-byte fallback is `#800080ff`. The conversion through linear sRGB at [fold.rs:558](/Users/admin/x2p2/apps/exact2-tamber/tamber/src/fold.rs:558) introduces the error. This needs an independent expected result, including the dependency’s clipping path.

The positive-total percentage normalization and final alpha multiplier look correct. Nested literal `light-dark()` projection, maximum-specificity selection, and tuple ordering also look sound. I independently verified the six JSON goldens’ FNV digests and tuple-index structure.

The caller-supplied vocabulary, same-tuple aliases, density-preserving light baseline, HSL/HWB mixes remaining Wide, and temporary refusal of role-containing pairs are reasonable deliberate choices. Rejecting the RFC example’s misindented radius is also correct. Expression tests need semantic expectations: the current parity check largely proves that emitted text parses.

VERDICT: fix
