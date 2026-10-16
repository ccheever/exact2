# Code review: exact-tamber color-mix and Rec. 2100 follow-up, round 2, 2026-10-13 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `51cc99572a70f8ab03769e3ebb79414549934e5b5302cc6346bc41b4ba982a1a`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** ship from both.
- **Disposition:** landed

---

I'll review only the Round 2 fix in `3e8b08c03` against the three Round 1 findings, without checking out, restoring, or editing anything.The worktree is already on `3e8b08c03`. I'll read the Round 2 diff and the surrounding mix code, and stay read-only.Round 1 named three defects. I'll read the brief and the mix/HLG code the fix touches so I can check each one against the new behavior.The three Round 1 defects are fixed, and the new code does not add another one. `cargo test -p exact-tamber -p exact-color` passed.

**Hue across a running mix.** `step` keeps the accumulator in the mix space and marks the result's hue powerless only when both ends were (`gray: a.gray && b.gray` in `tamber/src/color_fn.rs`). A transparent pair no longer comes back as achromatic. `color-mix(in oklch, oklch(0.7 0.1 30 / 0), oklch(0.7 0.1 90 / 0), oklch(0.7 0.1 120))` is hue 60, then a third of the way to 120, which is 80. That matches CSS Color 4: a component is missing on the result only when both inputs were missing, and hue is not premultiplied.

**All-zero weights.** `normalise` leaves an all-zero total at 0, and each step uses progress 0.5 when the running total is still 0, which is CSS Color 5's rule. Three opaque 0% colours are repeated even mixes: `color(srgb 0.25 0.25 0.5 / 0)`. Two colours still meet at half. A positive total still scales and applies the alpha multiplier once, after interpolation.

**HLG sign.** The decode and the inverse both keep the sign, including past the 1/12 knee, and they invert each other. Signal 0.75 is still linear 1 via `hlg_linear(0.75)` (the same scale as the spec's 3.7743). In the square-root region this is the Color HDR OETF: ProPhoto green's negative channels come out near `(−0.214, 0.765, −0.180)`. The round-trip test covers a negative HLG channel. Positive channels are unchanged.

VERDICT: ship
