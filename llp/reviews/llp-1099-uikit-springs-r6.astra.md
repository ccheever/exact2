# RFC review: LLP 1099, UIKit's springs everywhere (r6, `0d82410db`), 2026-10-05 (astra, round 6)

- **Family:** OpenAI — `gpt-6-astra` via `codex exec` (codex-cli 0.157.1), reasoning effort max, read-only sandbox, `-C` a detached worktree at `0d82410db`.
- **Method:** the round-6 brief (sha256 `1bf268a1a199f88f639f6b517365468fe1291bbf52086067b5a2c43b12ec8413`): check the round-5 dispositions, then review afresh; blind to the other round-6 review. The author (Claude) is not a reviewer.
- **Transcription:** the run's final message (`-o`), unedited but for paths made repo-relative.
- **Verdict:** NEEDS REWORK.
- **Disposition:** LLP 1099 r7 §11 (round 6).

---

NEEDS REWORK

1. **MATERIAL — The revised scale envelope still underestimates curvature.** [RFC:573](llp/1099-uikit-springs.rfc.md:573) uses `K=2` for every overdamped factor, but its fast exponent can exceed `2ω`. Bun computation for scale 1→2 with `spring(400ms, bounce -0.5)` gives initial curvature **3701.10**, versus the claimed bound **1139.64**. The prescribed first interval produces midpoint error **0.001537**, exceeding the **0.001** parity allowance. Round-5 G5 therefore remains unresolved. **Fix:** derive separate first- and second-derivative envelopes from each branch’s coefficients and exponents; use those in the product rule. Add a negative-bounce scale regression.

2. **MATERIAL — The parity allowance ignores browser serialization precision.** [RFC:605](llp/1099-uikit-springs.rfc.md:605) requires `10⁻³·S`, while [parity.rs:35](host/web/src/parity.rs:35) acknowledges six significant digits; [Chromium’s serializer](https://raw.githubusercontent.com/chromium/chromium/main/third_party/blink/renderer/core/css/css_numeric_literal_value.cc) uses `%.6g`. For scale 1→1.001 with `spring(400ms, 0.8)`, my Bun reproduction at 50 ms gives engine **1.0003435500**, interpolated frames **1.0003435400**, and serialized **1.00034**. Serialization alone creates **3.55·10⁻⁶** error against an allowance of approximately **10⁻⁶**. **Fix:** make the recorded-value comparator account for browser quantization separately from interpolation error; add a small-distance fixture.

3. **MATERIAL — Retargeting a released scale to zero has conflicting outcomes.** [RFC:396](llp/1099-uikit-springs.rfc.md:396) requires immediate zero and collapses live scale components; [RFC:474](llp/1099-uikit-springs.rfc.md:474) says released residuals survive later retargets and continue adding. Release scale 1 toward 2 at zero velocity under `spring(400ms, 0.8)`, then retarget to zero after 50 ms: the two rules produce **0** and **−0.656466**, respectively. **Fix:** explicitly make zero-target retargets cancel both component kinds, qualify the residual-continuation rule, and test release→zero→nonzero.