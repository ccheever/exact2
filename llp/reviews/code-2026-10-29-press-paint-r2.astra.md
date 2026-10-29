# Code review: web press paint keeps authored transitions, round 2 of 3 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `5954bd9d8f`.
- **Method:** one brief (sha256 `4e0f6ac07571b5c43fd43bbb3ac57cced4bca068a6b3f8655167b2177a1afab1`), shared by both reviewers, blind to the other review. The author (Claude, dtool3) is not a reviewer.
- **Transcription:** the final message, unedited.

- **MAJOR — [host/web/input-glue.js:303](host/web/input-glue.js:303): The Chromium limit violates the approved intent.** Restarting an oklch/lab transition changes its timing and cancellation events; replacing a paused transition also loses its pause. Matching endpoint pixels does not preserve behaviour. A viable approach is to omit that property’s paint declaration while its authored transition remains active, then install it when the transition finishes. The exception at [press.test.mjs:963](host/web/tests/press.test.mjs:963) also permits dropped transitions: `toAuthored` passes vacuously for an empty `after` array.

- **MAJOR — [host/web/input-glue.js:303](host/web/input-glue.js:303): Running opacity transitions still use lossy serialization.** The keyframe endpoint overwrites the Typed OM value obtained immediately above. An endpoint such as `0.123456789`, serialized as `0.123457`, becomes a different non-animated value at `p=0`, allowing the press to retarget the transition. Tests using only `0.8` and `0.6` miss this case.

- **MAJOR — [host/web/input-glue.js:336](host/web/input-glue.js:336): The wide-gamut guard misses `transition: background …`.** That shorthand includes `background-color`, but `transitioned()` returns false. Consequently, an rgb base with a P3 pressed colour takes the `color(srgb …)` path despite an authored transition observing the representation change. The generated-rule probe confirmed this branch difference. [CSS transition shorthand rules](https://drafts.csswg.org/css-transitions/#transition-property-property).

- **MAJOR — [host/web/input-glue.js:342](host/web/input-glue.js:342): Unsupported spaces retain the transition-triggering fallback.** A resting `color(srgb-linear …)`, `color(rec2020 …)` or XYZ base with `transition: all` becomes an sRGB mix at `p=0`. This can start a transition solely because the paint attribute appeared. Documenting the fallback does not satisfy the approved invariant; these spaces need equivalent baseline-preserving handling.

- **MAJOR — [host/web/input-glue.js:315](host/web/input-glue.js:315): Missing components are incorrectly converted to zero.** Computed colours can retain `none`, which is distinct from numeric zero. This breaks equality at `p=0` and CSS interpolation: mixing `oklch(0.7 0.2 none)` toward hue `120` should borrow that hue; the generated rule instead passes through hue `60` halfway. [CSS colour comparison and interpolation rules](https://drafts.csswg.org/css-color-4/).

- **MAJOR — [host/web/input-glue.js:343](host/web/input-glue.js:343): Copying pressed values freezes changes inherited from ancestors.** While holding a child whose pressed colour uses `var(--accent)` or `light-dark()`, change an ancestor’s token or local `color-scheme`. The new numeric rule remains stale: observers watch the painted node’s attributes, document-element attributes and head contents, but not ancestor attributes. The former live CSS expression updated automatically. Explicit `exact.repaintPress()` covers its callers only.

- **MINOR — [host/web/input-glue.js:355](host/web/input-glue.js:355): The denominator floor breaks exact equality for small nonzero alpha.** For a computed channel `0.5` with alpha `1e-7`, the rule evaluates at `p=0` to `0.05`, because it divides by `1e-6`. This is a baseline change, even though the colour is almost transparent.

- **MINOR — [host/web/input-glue.js:348](host/web/input-glue.js:348): The 180-degree hue tie takes the wrong direction.** From `lch(60 30 180)` to `lch(60 30 0)`, rounding changes the destination to `360`, producing midpoint hue `270`; CSS’s shorter-hue algorithm leaves this tie unchanged and produces `90`. [CSS shorter-hue algorithm](https://drafts.csswg.org/css-color-4/#hue-shorter).

- **MINOR — [host/web/input-glue.js:419](host/web/input-glue.js:419): Removal cleanup misses shadow-tree removals.** Observing `ownerDocument` with `subtree: true` does not observe mutations inside a shadow root. Removing a painting node there without releasing leaves its rule and `paints` entry retained until another relevant event occurs. Observe the painting root as well. [DOM mutation-record scoping](https://dom.spec.whatwg.org/#queue-a-mutation-record).

**Verdict: REQUEST CHANGES — browser tests could not start because this checkout lacks `playwright-core`; in-memory probes ran, and no files changed.**
