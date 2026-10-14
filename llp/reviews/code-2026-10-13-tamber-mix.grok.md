# Code review: exact-tamber color-mix and Rec. 2100 follow-up, round 1, 2026-10-13 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `a7512b613c4482a0c79399fd1cb643217607c961f24c5f715cd773f8e1ea1817`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** fix from both (Astra 2 MAJOR, 1 MINOR; Grok 1 MAJOR, 1 MINOR, the same findings).
- **Disposition:** all fixed in round 2: the running mix keeps its hue, HLG signed both ways, all-0% weights stepped evenly

---

I'll review commit `0bedaad3c` against CSS Color 5's `color-mix` rules and the Rec. 2100 inverse transfers, then run the crate tests. First I'm reading the review guidance and the commit itself.The request is a defect review of one commit, so I’ll inspect that diff and the Color 5 / Rec. 2100 rules it claims to follow. Git stays read-only.The diff is in. Next I’ll check the mix algorithm and the Rec. 2100 inverses against the current Color 5 text and the crate’s existing decode.The crate tests passed. I’ll probe the n-ary zero-percent mix and the HSL round-trip against the spec, since those are the places the new loop can diverge.`color-mix` with three or more colours at 0% stores the wrong channels. CSS Color 5 leaves a zero total at 0% and, on each step, uses progress 0.5 when the combined percentage is 0. That is a repeated 50/50 mix, so later colours weigh more. `normalise` in `tamber/src/color_fn.rs:702` replaces that total with equal weights, and the running mix at `tamber/src/color_fn.rs:659` uses progress 0 whenever the weight so far is 0.

Two colours still match the spec, which is why `color-mix(in srgb, red 0%, blue 0%)` is `color(srgb 0.5 0 0.5 / 0)`. Three do not. `color-mix(in hsl, red 0%, lime 0%, blue 0%)` folds to `color(srgb 0 1 0 / 0)` (hue 120). The spec’s two 50/50 steps land on hue 150, `color(srgb 0 1 0.5 / 0)`. The same split in sRGB is `(1/4, 1/4, 1/2)` against the equal thirds this code stores. Alpha is 0 in both, and that part is right. The channels are what the token keeps.

The Rec. 2100 inverse is right on the positive side and on the reference white. PQ of linear 1 is 0.580689, the 203 cd/m² media white. HLG signal 0.75 maps to linear 1 through `hlg_linear(0.75)`, the exact form of the sample’s 3.7743 scale. Negative HLG scene light is not. `color/src/lib.rs:653` clamps with `.max(0.0)` and takes an unsigned square root. CSS Color HDR’s OETF keeps the sign at or below 1/12. ProPhoto green is about `(−0.058, 1.081, −0.041)` in linear Rec. 2020, so the HLG signal should be about `(−0.214, 0.765, −0.180)`. The fold stores `(0, 0.765, 0)`. PQ’s clamp matches that spec’s `Math.max`.

The other mix rules I checked match the current draft: omitted percentages, a sum over 100%, the alpha multiplier (`rgb(100% 0% 0% / 0.7) 20%` with `rgb(0% 100% 0% / 0.2) 60%` is alpha 0.26), and positive-weight hue order (`oklch` 10°, 200°, 300° → 290°). The hand-written expectations that are present are right, including `#800080ff`, `color(srgb 0.6 0.6 0.6 / 0.5)`, `#555555ff`, and the `0.627` Rec. 2100 prefix (the value is 0.627404). `cargo test -p exact-tamber -p exact-color` passed.

VERDICT: fix (MAJOR tamber/src/color_fn.rs:702; MINOR color/src/lib.rs:653)
