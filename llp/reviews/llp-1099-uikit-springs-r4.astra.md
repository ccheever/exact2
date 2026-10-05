# RFC review: LLP 1099, UIKit's springs everywhere (r4, `e240893f4`), 2026-10-05 (astra, round 4)

- **Family:** OpenAI — `gpt-6-astra` via `codex exec` (codex-cli 0.157.1), reasoning effort max, read-only sandbox, `-C` a detached worktree at `e240893f4`.
- **Method:** the round-4 brief (sha256 `74cce9d318c2d698d460f2c7de1cab45f7a9804bbc3ac0c4645546b5fbee4e6b`): check the round-3 dispositions, then review afresh; blind to the other round-4 review. The author (Claude) is not a reviewer.
- **Transcription:** the run's final message (`-o`), unedited but for paths made repo-relative.
- **Verdict:** NEEDS REWORK.
- **Disposition:** LLP 1099 r5 §11 (round 4).

---

NEEDS REWORK

1. **MATERIAL — The revised sampler violates its stated accuracy contract.** [RFC:482](llp/1099-uikit-springs.rfc.md:482) promises move-relative accuracy, but scales tolerance by `S`, which can exceed the move. Recomputing its formula for `spring(400ms, bounce -0.5)` gives `S=4`, a first interval of 2.07842 ms, and midpoint error **0.00188094 moves**—already outside [D6’s 0.001 parity band](llp/1099-uikit-springs.rfc.md:512), without hitting the frame cap. **Fix:** reconcile the sampling budget and conformance normalization; explicitly distinguish any amplitude-relative release tolerance. Add this negative-bounce midpoint regression.

2. **MATERIAL — The 64-component fold does not define a resulting trajectory.** [RFC:380](llp/1099-uikit-springs.rfc.md:380) specifies initial offset and derivative but leaves the replacement’s coefficients, evaluator branch and end time unspecified. Different choices produce different frames and `settle_time()`. Its continuity claim also fails for multiplicative scale: factors `0.5 × 0.5` require replacement factor `0.25`; summing their residuals produces factor `0`. **Fix:** define the replacement parameters and lifetime, use product-rule value/velocity for scale, and test the 65th retarget with unequal coefficients and different cuts.

3. **MATERIAL — The provisional scale rule is undefined when the target is zero.** [RFC:365](llp/1099-uikit-springs.rfc.md:365) uses `old/new` and multiplies the factors by the target. A simple `1 → 0` therefore produces division by zero and `0 × Infinity`, although zero scale is admitted by the [schema](kernel/tables/schema.json:2441) and [CSS’s scale grammar](https://www.w3.org/TR/css-transforms-2/#individual-transforms). **Fix:** define a finite fallback or representation for zero targets, including retargeting through zero; explicitly include those cases in Stage 0 and cross-host tests.