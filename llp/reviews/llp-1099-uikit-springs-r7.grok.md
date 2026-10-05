# RFC review: LLP 1099, UIKit's springs everywhere (r7, `2aff556e5`), 2026-10-05 (grok, round 7)

- **Family:** xAI — Grok 4.7 via `~/.grok/bin/grok` (1.0.41), reasoning effort xhigh, `--always-approve --no-subagents --output-format plain`, `--cwd` a detached worktree at `2aff556e5`, read-only by instruction.
- **Method:** the round-7 brief (sha256 `cdf8556429204b549b99208ca361e81caeaea3a1a8f0a227d12ff2f4477eea37`): check the round-6 dispositions, then review afresh; blind to the other round-7 review. The author (Claude) is not a reviewer.
- **Transcription:** the run's output from the verdict on, unedited (progress and tool-status lines dropped).
- **Verdict:** SOUND WITH CHANGES.
- **Disposition:** LLP 1099 r8 §11 (round 7).

---

SOUND WITH CHANGES

Round-6 dispositions are in the text, and a fresh `bun motion/tests/fixtures/uikit-springs/fit.mjs` matches the mapping claims (9,474 calls, 100 misses all inside the band, 1,916 flagged, 27 refusals none of which UIKit matched, icon left limits 1.7729 against 1.7378, release sample 0.018832 at 50 ms). Q1–Q7 each give the measurement, a recommendation, and the cost of the other answer.

1. **MATERIAL.** D3’s critical end search does not terminate as written. `llp/1099-uikit-springs.rfc.md:276-291` defines `f(T) = |P(T)|·e^(−ωT)`, which is ≥ 0 and equals ε at the root, then doubles until `f(hi) ≤ 0` and keeps the half where `f(lo) > 0`. That `f` is never ≤ 0 except at P’s zero. `fit.mjs:70-72` uses the residual `|P|·e^(−ωT) − ε`, and the RFC says that function implements exactly this procedure. **Fix:** define `f(T) = |P|·e^(−ωT) − ε`, double while `f(hi) > 0`, and state the `v ≤ ω` peak `fit.mjs:80` uses, `max(0, 1/ω − 1/(ω − v))`. Starting at 0 instead matched that peak to 0 ulps on a 98-point grid, so the value is stable, but the cited code does not start at 0.

2. **MATERIAL.** An end time on a physical spring is dropped on paint. D6 (`:550`) keeps `SpringConfig::easing` for physical paint springs. `motion/src/spring.rs:167-182` lowers the settle curve and has no end-time cut, so `200ms spring(k, c, m)` cuts on a compositor property and runs to rest on a colour. End-timed physical springs are the band escape hatch (D3 `:311`, D1 `:150-155`) and are legal on paint (only velocity is refused, `:173-174`). **Fix:** use today’s easing only when the duration is 0. An end time uses the new stop placer and the terminal pair.

3. **MATERIAL.** A delayed `scale` retarget to 0 is a jump at a time the grid treats as continuous. The collapse to 0 fires at delay end (`:328-333`, `:409-416`). D3 `:334-336` says the value is continuous at a component start, and D6 `:562-564` emits a same-offset step only at component ends. Linear interpolation across that instant ramps a nonzero scale to 0 and leaves the `10⁻³·S` band. §8.4 `:1024-1025` never samples across the jump. **Fix:** two frames at the collapse time, the pre-collapse value then 0, and a between-frames sample. The pre-delay value is the still-moving pre-retarget trajectory (`:332-333`), not a frozen snapshot.

4. **MINOR.** §8.3 `:1006` compares `|browser − engine| ≤ 10⁻³·S`. D6 `:637-644` and §8.4 `:1022` add `q`. On scale 1 → 1.001 the recorded 1.00034 against the engine 1.00034355 errs by 3.55·10⁻⁶; `10⁻³·S` is 10⁻⁶ and `q` is 5·10⁻⁶, so the §8.3 oracle rejects the case §8.4 exists to pin. **Fix:** write `≤ 10⁻³·S + q` in §8.3.

5. **MINOR.** “The oldest pair of the same kind” (`:437-441`) does not rank interleaved kinds. Factors begun at t = 0 and t = 3, residuals at t = 1 and t = 2: ranking by the older member folds the factors; ranking by the younger member folds the residuals. §8.4’s mixed 65-pile then has two legal samples. **Fix:** fold the two earliest components of the kind whose second member is earliest.

6. **MINOR.** Three searches that gate a refusal or a frame are specified only as “bisection”: the critical step (`:581-583`), the paint stop placer (`:550`), and clamp crossings (`:658`). Underdamped and overdamped steps are closed forms, so `spring(1s, bounce 0.9, velocity 20000)` is pinned (17,882 frames by the envelope). A critical spring’s parse-time count is not one integer, and two paint placers can disagree on the 64-stop refusal. **Fix:** a fixed iteration (stated bracket, count, and which endpoint) or a closed form, shared with the paint max-error search.

7. **NIT.** `:612` says that on a 1 → 0.1 → 0.01 retarget `|s''|` is 3.8 times the sum of the per-component second derivatives. At `spring(400ms, 0.8)`, 50 ms after the second retarget, that ratio is 0.066 (the product bound still covers, 305 < 1,958). At `spring(400ms, bounce -0.5)`, at the second retarget, it is 14.1. The cross term is load-bearing. The factor is not. **Fix:** name the spring and the time, or drop 3.8.
