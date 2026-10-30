# Code review: web press paint keeps authored transitions, round 3 of 3 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `eb11b4ebd4`.
- **Method:** one brief (sha256 `45149f103501498099f5bd6c538759dce1da8c2467099c02df421cf7a3342df9`), shared by both reviewers, blind to the other review. The author (Claude, dtool3) is not a reviewer.
- **Transcription:** the final message, unedited.

- **MAJOR — [host/web/input-glue.js:315](host/web/input-glue.js:315): Background layers lose their corresponding settings.** Only `background-size` is shifted. The existing grouped-row separator in [index.html:131](host/web/index.html:131) consequently moves from `no-repeat` at its bottom position to `repeat` at `0 0`, changing the row already at progress 0. Short size lists also break: two images sharing `20px 20px` become sizes `auto, 20px 20px, auto`, resizing the second authored image. Lists must be expanded and shifted together. [CSS layer matching](https://drafts.csswg.org/css-backgrounds-3/#layering).

- **MAJOR — [host/web/input-glue.js:326](host/web/input-glue.js:326): Firefox’s absolute opacity target becomes incorrect after a transition.** With sampled opacity `0.8`, an authored transition toward `0.6`, and pressed target `0.4`, the mask retains factor `0.5`. Once the transition finishes, the result is `0.3`, not `0.4`. This branch never calls `later("opacity")`, so completion never recalculates it. The read-only probe confirmed both the wrong result and the missing subscription.

- **MAJOR — [host/web/input-glue.js:293](host/web/input-glue.js:293): `transitioned()` misclassifies valid transition declarations.** `color 0s 1s` returns false despite creating a delayed transition, permitting the unsafe nonlegacy colour rewrite at progress 0. Conversely, `all 1s, color 0s` returns true although the last matching entry disables that transition, unnecessarily suppressing paint. Negative delays are ignored too. CSS uses the last matching entry and its combined duration. [Transition rules](https://drafts.csswg.org/css-transitions-1/#starting).

- **MAJOR — [host/web/input-glue.js:330](host/web/input-glue.js:330): The mask can hide the entire node immediately.** An authored `mask-mode: luminance` also applies to the newly prepended black gradient. Black has zero luminance, including at progress 0, so intersecting it makes the node invisible. The new layer needs its own mask mode; the remaining mask lists also need their original layer correspondence preserved. [Mask processing](https://drafts.csswg.org/css-masking-1/#MaskValues).

- **MINOR — [host/web/input-glue.js:312](host/web/input-glue.js:312): Cancellation never restores deferred paint.** The rejection handler for `a.finished` does nothing. If a deferred colour/opacity transition is cancelled without a watched style/class mutation—for example through `cancel()`—its paint remains absent until another repaint trigger. The state guard already used for successful completion should also govern cancellation.

- **MINOR — [host/web/input-glue.js:352](host/web/input-glue.js:352): `pressShown` can contradict the rendered background.** An inline `background-image: none !important` defeats the generated layer, but the helper still reports the pressed colour. Conversely, without `OffscreenCanvas`, it reports the unpainted base even when the layer renders. Tests consuming this helper can therefore validate intended paint without validating displayed paint.

- **MINOR — [host/web/tests/press.test.mjs:969](host/web/tests/press.test.mjs:969): The transition-preservation assertion is disabled for Chromium’s Lab/OKLCH cases.** Substituting literal `true` permits replacement or cancellation of those transitions, contradicting the new guarantee. `after.every(...)` also passes for an empty list. Additionally, these cross-engine fixtures never exercise pressed text colour or a `color` transition.

- **MINOR — [host/web/tests/press.test.mjs:865](host/web/tests/press.test.mjs:865): The new transition assertion can flake under load.** It requires a one-second transition still to exist after several awaited CDP operations. A correct implementation fails once those operations consume the second. The 60-second test bound does not prevent this; pause the transition before yielding.

Validation: the new renderer test passes and fails against `origin/main`; source probes confirmed the arithmetic and callback defects. Browser tests could not start because `playwright-core` is missing. No files changed.

**Verdict: NOT READY — four major correctness defects remain.**
