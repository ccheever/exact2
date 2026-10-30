# Code review: web press paint keeps authored transitions, round 2 of 3 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `5954bd9d8f`.
- **Method:** one brief (sha256 `4e0f6ac07571b5c43fd43bbb3ac57cced4bca068a6b3f8655167b2177a1afab1`), shared by both reviewers, blind to the other review. The author (Claude, dtool3) is not a reviewer.
- **Transcription:** the final message, unedited.

## Findings

**MAJOR** — A computed colour that is not a short literal does not round-trip, so the rule at progress 0 starts an authored transition. `host/web/input-glue.js:310`

`colour` parses `getComputedStyle` and `paintRule` writes those numbers back. In Chrome that string is not the internal colour. Assigning `color(srgb 0 0 1 / 0.12)` (what `color-mix(in srgb, blue 12%, transparent)` computes to — the neutral tinted fill) starts a `background-color` transition. So does `lab(29.5653 68.3015 -112.035)` from `lab(from rgb(0, 0, 255) l a b)`. `computedStyleMap()` stringifies to the same rounded text, unlike opacity. The transition sits above the paint until it ends, so the press does not show for that long. `lab(90 10 10)`, `oklch(0.9 0.05 200)`, and `rgb(255, 0, 0)` do round-trip, which is why the engine test stays green.

**MAJOR** — The premultiplied channel calc is not the channel beneath when alpha is neither 0 nor 1. `host/web/input-glue.js:355`

`calc((b * a + …) / max(alpha, 0.000001))` bakes `b * a` in JavaScript. For `rgba(3, 0, 0, 0.1)`, that is `0.30000000000000004 / 0.1`, which is not 3. Reapplying the plain `rgba(3, 0, 0, 0.1)` starts no transition; the emitted calc starts one. 245 of 254 8-bit alphas fail some channel in 0–255 (0.1, 0.12, 0.2, 0.4, …). Alpha 0 and the both-opaque path are special-cased and do round-trip. Same effect: the new transition hides the paint for its duration. `b + pressedAlpha * (pressed − b) * p / alpha` is the same mix and is exactly `b` at progress 0.

**MINOR** — The new tests do not cover either hole, and one assertion is only “not transparent.” `host/web/tests/press.test.mjs:949`

The engine matrix is opaque `rgb()` / `oklch(0.9 0.05 200)` / `lab(90 10 10)`. On Chromium the identity check is replaced with `true`, and the colour check only rejects a fully transparent end. The stall window (opacity 0.399–0.801, alpha 1) also passes if a same-looking transition simply holds the unpressed colour. Both tests still fail on main for the original bug (identity, and opacity 1 / alpha 0). They do not fail for these two.

The Chromium limit is acceptable. A running `oklch()` / `lab()` transition’s keyframes come back as `oklab(…)`, so the rule at 0 can retarget toward an end that paints the same, never toward the initial values. Omitting that property from the rule until the transition finishes would avoid the retarget. It would not fix the two holes above. Sheet scoping, rule identity on delete, removal, opacity-to over opacity, the wide-gamut clip when a transition would see `color(srgb …)`, agent mode, and reduced motion (the press still eases; the looks test requires that) hold. The rec2020 / xyz `color-mix(in srgb)` fallback is the one the comment already owns.

Verdict: Not clean — progress 0 is still not the look beneath for a 12% srgb mix, a converted lab(), or a translucent rgb() through the premultiplied calc, so those presses still start an authored transition and hide the paint.
