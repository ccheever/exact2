# Code review: web press paint keeps authored transitions, round 1 of 3 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `e7b4354db2`.
- **Method:** one brief (sha256 `f642d4cc82265897afdb2f31e6054822a581e1410ec9130ecfab51b3b8e62be1`), shared by both reviewers, blind to the other review. The author (Claude, dtool3) is not a reviewer.
- **Transcription:** the final message, unedited.

- **MAJOR — [host/web/input-glue.js:298](host/web/input-glue.js:298): Serialized computed values do not guarantee equality at `p=0`.** Chrome serializes numbers to six significant digits: authored `opacity:0.33333334` becomes `"0.333333"`. Writing that back changes the transition baseline. With `transition:opacity 1s`, pressing can therefore start an otherwise unnecessary transition that hides the dim for a second. Rounding only the deltas does not prevent this. [Chromium’s serializer](https://raw.githubusercontent.com/chromium/chromium/main/third_party/blink/renderer/core/css/css_numeric_literal_value.cc)

- **MAJOR — [host/web/input-glue.js:299](host/web/input-glue.js:299): Paused transitions lose their authored endpoint.** A transition from `0.8` to `0.6`, paused at `0.7`, produces a baseline of `0.7` because only `"running"` animations qualify. Applying the rule consequently changes the target and can cancel the transition. This affects the framework’s agent clock, which pauses animations in [agent.js:284](host/web-js/agent.js:284). The endpoint must also be retained for paused, unfinished transitions.

- **MAJOR — [host/web/input-glue.js:325](host/web/input-glue.js:325): The fallback violates the unchanged-baseline requirement.** An `oklch(...)` or `color(display-p3 ...)` baseline becomes an sRGB `color-mix()` result even at zero progress. Chromium’s color equality includes the color-space tag, so this can start or retarget an authored transition. The comment’s “differ … by spelling” understates a functional difference. [Chromium color equality](https://raw.githubusercontent.com/chromium/chromium/main/third_party/blink/renderer/platform/graphics/color.h)

- **MAJOR — [host/web/input-glue.js:326](host/web/input-glue.js:326): Translucent interpolation now produces different colors.** From transparent black to opaque blue, `p=0.5` generates approximately `rgba(0,0,128,0.5)`; the previous `color-mix()` produces `rgba(0,0,255,0.5)`. Interpolating RGB and alpha independently introduces visible darkening during ordinary transparent-background presses. Preserve CSS’s premultiplied-alpha interpolation. [CSS Color interpolation](https://www.w3.org/TR/css-color-4/#interpolation-alpha)

- **MAJOR — [host/web/input-glue.js:328](host/web/input-glue.js:328): An RGB baseline clips wide-gamut pressed colors.** Pressing from white toward `color(display-p3 1 0 0)` generates RGB channels approximately `(278.73, -57.82, -38.28)` at `p=1`. The emitted `rgb()` clamps those to ordinary sRGB red, losing the authored P3 color. The previous `color(srgb …)` result retained extended channels. [CSS RGB clamping](https://www.w3.org/TR/css-color-4/#resolving-sRGB-values)

- **MINOR — [host/web/input-glue.js:279](host/web/input-glue.js:279): Removal alone does not sweep a paint.** Cleanup runs only during another `paint()` and excludes its current element. Removing the sole pressed node leaves its rule, map entry and observer retained until another interaction; even an explicit repaint leaves its map entry. The removal test sends `mouseReleased` before checking, so it proves release cleanup rather than removal cleanup.

- **MINOR — [host/web/tests/press.test.mjs:751](host/web/tests/press.test.mjs:751): The stall regression can pass without observing the failure window.** Sampling starts after both CDP input commands complete. Under load, the first sample can arrive after the erroneous one-second transition has finished; six settled, unpainted samples then pass. Nothing asserts that the stall overlapped the press or that an active transition was sampled. Start observation before input and assert that the relevant interval was observed.

Browser tests could not start because `playwright-core` is missing; findings use source review and in-memory probes. No files changed.

**Verdict: Changes required; the implementation does not yet preserve authored transitions across the supported cases.**
