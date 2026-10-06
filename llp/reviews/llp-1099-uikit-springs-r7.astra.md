# RFC review: LLP 1099, UIKit's springs everywhere (r7, `2aff556e5`), 2026-10-05 (astra, round 7)

- **Family:** OpenAI — `gpt-6-astra` via `codex exec` (codex-cli 0.157.1), reasoning effort max, read-only sandbox, `-C` a detached worktree at `2aff556e5`.
- **Method:** the round-7 brief (sha256 `cdf8556429204b549b99208ca361e81caeaea3a1a8f0a227d12ff2f4477eea37`): check the round-6 dispositions, then review afresh; blind to the other round-7 review. The author (Claude) is not a reviewer.
- **Transcription:** the run's final message (`-o`), unedited but for paths made repo-relative.
- **Verdict:** NEEDS REWORK.
- **Disposition:** LLP 1099 r8 §11 (round 7).

---

NEEDS REWORK

1. **MATERIAL — Delayed scale-to-zero needs an explicit discontinuity.** [D3](llp/1099-uikit-springs.rfc.md:328) holds the presented scale through the delay, then [D5](llp/1099-uikit-springs.rfc.md:409) collapses it to zero. But D3 calls starts continuous, and [D6](llp/1099-uikit-springs.rfc.md:558) requires paired frames only at ends. Scale 1→0 with a 100 ms delay must jump at 100 ms; a single start frame can ramp into zero beforehand. **Fix:** specify the pending collapse, its paired frames and completion time, and what supersedes it when another retarget or hold arrives during the delay. Test both boundary samples and supersession.

2. **MATERIAL — The revised paint tolerance cannot determine acceptance at parse time.** [D6](llp/1099-uikit-springs.rfc.md:550) makes colour tolerance depend on the largest channel move, while requiring refusal at parse. [Parsing](motion/src/parse.rs:43) and [CSS lowering](host/web/src/css.rs:606) receive no colour endpoints. My numerical greedy fit of `spring(100ms, .01)` needed 47 stops for a one-level channel move; its 73 extrema preclude 64 stops at the full-range colour tolerance. The same declaration therefore has different acceptance outcomes. **Fix:** define a fixed normalized tolerance and share its lowered stops across hosts, or explicitly design endpoint-dependent runtime fitting and refusal.

3. **MINOR — D3 bisects the wrong function as written.** [D3](llp/1099-uikit-springs.rfc.md:276) defines `f(T)=|P(T)|·exp(−ωT)`, then brackets using `f(hi)≤0`. For ω=1, v=0, a literal Bun implementation returns approximately **745.133**, versus **9.233413** from [`criticalSettle`](motion/tests/fixtures/uikit-springs/fit.mjs:70). **Fix:** define the signed residual `F=f−ε` and use it throughout the bracket and bisection instructions.

4. **MINOR — Opacity rows need the same replay qualification as scale-to-zero.** [§8.4](llp/1099-uikit-springs.rfc.md:1013) exempts only scale-to-zero’s rewritten history. Yet [row `alpha-v1-bfcs`](motion/tests/fixtures/uikit-springs/ios-27.0.txt:9660) records **0.40130639 at t=0**, whereas the first animation starts at opacity **1**. **Fix:** compare opacity recordings only from the retarget onward, reconstruct earlier samples from the first animation, and explicitly select the `bfcs` rows for Contract’s replacement semantics.

5. **MINOR — The serialization allowance remains inconsistent.** [D6](llp/1099-uikit-springs.rfc.md:636) now requires `10⁻³·S + q`, but [§8.3](llp/1099-uikit-springs.rfc.md:1004) still specifies `10⁻³·S`. **Fix:** reference the D6 comparator consistently throughout §8, including the overdamped midpoint regression.