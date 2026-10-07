# RFC review: LLP 1099, UIKit's springs everywhere (r5, `5afd8bd8f`), 2026-10-05 (grok, round 5)

- **Family:** xAI — Grok 4.7 via `~/.grok/bin/grok` (1.0.41), reasoning effort xhigh, `--always-approve --no-subagents --output-format plain`, `--cwd` a detached worktree at `5afd8bd8f`, read-only by instruction.
- **Method:** the round-5 brief (sha256 `9edf0f4367cf89e854493f25bcedeed58bfb20e3795c349b2ac0511f5806a1d3`): check the round-4 dispositions, then review afresh; blind to the other round-5 review. The author (Claude) is not a reviewer.
- **Transcription:** the run's output from the verdict on, unedited (progress and tool-status lines dropped).
- **Verdict:** NEEDS REWORK.
- **Disposition:** LLP 1099 r6 §11 (round 5).

---

NEEDS REWORK

Round-4 dispositions hold in the text, aside from the two called out below. Q1–Q7 are fairly posed: each gives the measurement, a recommendation, and the cost of the alternative. Recomputed figures hold: icon left limits 1.7729 and 1.7378, menu response gap 0.2056, textbook-versus-quirk gap 0.874 on the ζ = 2 spring (and ~0 against the actual velocity v + 2ζω), the 60 s crossing at b = 0.944600 (b = 0.95 ends at 66.431 s; b = 0.9, d = 1 s ends at 11.146 s), and the lobe values 5.421646815 and 5.454495022. The smooth-curve step stays inside 5·10⁻⁴·S (measured chord 4.94·10⁻⁴ on 0.1 s at ζ 0.01).

1. **MATERIAL.** §8.3 still holds `uikit-duration`, `uikit-cut`, `uikit-bounce-overdamped` and `uikit-retarget` to an absolute 10⁻³ (`llp/1099-uikit-springs.rfc.md:909-911`). D6 and §8.4 hold the new cases, including that same overdamped midpoint, to 10⁻³·S (`:564-569`, `:932-933`). §11:1131 marks round-4 A1/G4 taken. On the 300 pt center retarget, S = 300, so the sampler is only guaranteed to 5·10⁻⁴·S = 0.15 pt; a test written from §8.3 fails and one written from D6 passes. **Fix:** make §8.3 say |browser − engine| ≤ 10⁻³·S with S stored in the fixture.

2. **MATERIAL.** That band is unsatisfiable for opacity and height through the harness §8 names. `host/web/parity.html:15` records `getComputedStyle`. Computed opacity is clamped to [0, 1]; the icon curve peaks at 1.7729 (k = 184.95167507412762, c = 1.6319632719725765, v = 0.8, t = 0.2), so an opacity 0→1 case is off by 0.773 against a band of 10⁻³. `host/web/motion-glue.js:96-98` clamps height with `Math.max(0, value)` and says the displayed length leaves the engine curve. A 100→30 shrink on that icon spring reaches −24.1. §8.4:936 requires a shrinking height on the absolute track. **Fix:** state that the band is what Chrome can return for `translate`, `scale` and `rotate`; for height, the displayed value is max(0, engine) and §8.4 includes a shrink that crosses 0; for opacity, keep the parity case inside [0, 1] or record the unclamped keyframe.

3. **MATERIAL.** D3:307 says the 16,384-frame bound is sized so every accepted spring fits. `spring(1s, bounce 0.9, velocity 20000)` passes the duration, bounce, ζ and 60 s gates (end = 23.839 s) and the D6 step needs 17,882 frames. `velocity 100000` needs 39,991 frames and ends at 26.400 s. §8.5 says to refuse a spring over the frame bound and gives no input that trips it. **Fix:** delete the sizing sentence and add `spring(1s, bounce 0.9, velocity 20000)` to §8.5 as the parse-time refusal.

4. **MINOR.** D6:503 says an interior duplicate stop would still interpolate, citing `easing.rs:334`. The terminal pair snaps because an input at or past the last stop returns the last output (`motion/src/easing.rs:334-335`). An interior pair with equal inputs returns the later stop (`:340-341`). **Fix:** cite `:340` and drop the interpolate claim.

5. **MINOR.** Scale’s step size is not a formula. D6:526 defines Δᵢ as factor distance times the target, D6:528 sets M = Σᵢ Mᵢ, and D6:540-541 says each scale Mᵢ comes from the product rule on the factors’ envelopes. On a duration spring ζ = 0.8, d = 0.4, scale 1→0.1 retargeted to 0.01 at 0.05 s, |s''| reaches 3.78× the sum of the per-component envelopes |Δᵢ|·ω²Re^(−ζωt). That chord is about 1.9·10⁻³·S, past the parity band. The folded scale component is also missing its textbook initial condition: the state is the factor, whose equilibrium is 1, so the displacement passed to `spring.rs` is f₁·f₂ − 1. **Fix:** write the product envelope (|f−1|, |f'| and |f''| bounded per factor, then the second derivative of T·∏fᵢ) and set the folded displacement to f₁·f₂ − 1. Pin the post-fold sample at a stated time.

6. **MINOR.** D5:369-370 says the combination rules are all measured in rows R, then includes `height` and layout. Rows R (`ios-27.0.txt:9657-9674`) and the catalogue at `:647` are `center`, opacity, `scale`, `scale-up`, `scale-to-zero` and `rotate`. **Fix:** say addition is the rule for `height` and layout, and that rows R measure `center`, `scale` and `rotate`.

7. **MINOR.** §8.4:924-928 names an exact zero-distance release and states assertions only for the over-bound release. **Fix:** assert that the zero-distance release plays, is not refused, and uses S = max(|v₀|/ω, 10⁻³).

8. **NIT.** Three stale numbers. The f32 delayed cut is at 0.300000011920929 if the add is done in f32, or 0.300000004470348 as an f64 sum of the two f32 values (`:582`); the icon there is still 1.7729. The v = 0 response gap at ζ 0.01, 0.1 s is 1.965 (`:114-115`); the menu figure 0.206 is right. §11:1102-1106 still says round 3 is unreviewed and was the last round; the status line (`:2`) is Draft (r5).
