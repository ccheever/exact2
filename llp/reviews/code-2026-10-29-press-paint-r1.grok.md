# Code review: web press paint keeps authored transitions, round 1 of 3 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `e7b4354db2`.
- **Method:** one brief (sha256 `f642d4cc82265897afdb2f31e6054822a581e1410ec9130ecfab51b3b8e62be1`), shared by both reviewers, blind to the other review. The author (Claude, dtool3) is not a reviewer.
- **Transcription:** the final message, unedited.

## Findings

## MAJOR

**`host/web/input-glue.js:325` — the fallback changes the non-animated color, so it can start a transition.** `channels` (`303–307`) accepts only comma-space `rgb()`/`rgba()` and `color(srgb …)` with a plain numeric alpha. Anything else — `lab()`, `oklch()`, modern `rgb(r g b / a)`, a `none` component, a scientific or percent alpha — takes

```text
color-mix(in srgb, var(--exact-press-color) calc(var(--exact-press-paint) * 100%), <look beneath>) !important
```

With the paint animation ignored, `--exact-press-paint` is its initial `0`, so this is a `0%` mix: the look beneath converted into sRGB. That is a different computed value from `lab()` or `oklch()`, and the comment on line 324 says so. An authored transition on that property therefore starts (and, while it runs, sits above the paint). The mix is also the premultiplied `color-mix`; the channel path is straight alpha.

This is a real style in this app, not a hypothetical space. `host/web/neutral.css:117` sets filled, bordered-prominent, and prominent-glass button `color` to `lab(from var(--exact-accent, …))`, and `:242` does the same for a checked checkbox or radio. `neutral.css:43` sets `--exact-accent` to `oklch(…)`, so a background that is the accent computes as `oklch` and misses `channels` too. The host emits `transition: all` as a normal easing (`host/web/src/css.rs:898` and the fixture at `:1452`), which covers `color` and `background-color`. A press color on those controls changes the value a transition sees. The tested path (legacy `rgb()` beneath, `color(srgb)` pressed copy) does not go through this branch.

## MINOR

**`host/web/input-glue.js:318` — the opacity base is not checked, and a non-numeric press opacity is dropped.** `number()` (`317`) rejects a non-finite press opacity and clamps a finite one to `[0, 1]`; `0` is kept, and `--exact-press-opacity-to` wins over the share (`319–320`). The base is `Number(beneath.opacity)` with no `Number.isFinite` check. A keyframe end that is not a plain number becomes `calc(NaN + …)`, and `insertRule` (`350`) throws out of the pointer handler. `parseFloat` also skips a press opacity that computes as `calc(0.5)` (the old `clamp()` inside `calc()` resolved that) and accepts `0.5px` as `0.5`. `css.rs` writes these as plain numbers, so this is the token and keyframe-format edge.

**`host/web/tests/press.test.mjs:751` — the stall does not cover the press.** The 300 ms busy loop is a `setTimeout(0)` queued inside `evaluate`, and the press/release CDP calls are sent after that evaluate returns, so the loop runs before the press. `shown()` (`765`) still rejects the known bug: opacity outside `[0.399, 0.801]` or a background alpha other than 1, and computed `transparent` is `rgba(0, 0, 0, 0)`, which the alpha regex catches. Two holes remain. `parts()` (`764`) treats a background it cannot parse (the keyword `transparent`) as alpha 1. Sampling stops once five frames have no `data-exact-painting` (`758`), about 80 ms after the attribute drops, so a drift that starts only after the paint rule is gone can slip through. The engine test is not timer-flaky: `finish()` is synchronous inside one `evaluate` (`930–933`).

**`host/web/tests/press.test.mjs:936` — the engine test does not prove the color, and a same-end restart passes.** `pressed === before` plus the property-name check fails on unfixed Chrome/WebKit (the ends become opacity `1` and transparent) and on unfixed Firefox (those transitions were dropped). It does not fail a cancel-and-restart that keeps the same ends. `look()` records `backgroundColor` (`922`) and the expects never read it (`938–939`): painted opacity `0.30` and released opacity `0.60` are the only color checks, so a paint that preserves transitions but never turns the background blue still passes. On an engine whose keyframe keys are not `opacity` / `backgroundColor`, both sides stringify as `"undefined"` and the equality holds without comparing colors.

## NIT

**`host/web/tests/looks.test.mjs:165` and `host/web/tests/hold.test.mjs:69` — the serialization pin was removed.** Both rewrite `color(srgb r g b)` to 8-bit `rgb()` and round. The old expects were the exact string `color(srgb 0 0 1)` (`looks.test.mjs` had it at the press-over-hover assert; `hold.test.mjs:78` too). The 8-bit color is still checked. The rewrite does not match `color(srgb … / a)`, and rounding hides under half a channel, so the tests no longer prove "a painted color now computes as `rgb()`". `looks.test.mjs:182` only parses a legacy `rgb(calc(N) …)` rule, so a correct `color(srgb …)` rule fails `beneath()` and the alpha is unchecked.

**`host/web/input-glue.js:316` — `toFixed(6)` can miss the settled end.** It does not affect p=0: the delta is multiplied by `--exact-press-paint`, whose non-animated value is 0, and `Number(s).toString()` round-trips the decimals these engines emit (`0.8`, `0.6`, `0.5`, `0.4980392156862745`, …). At p=1, `0.4980392156862745 + Number((1 - that).toFixed(6))` is `1.0000002156862746`. Opacity and `rgb` channels clamp, which is why the settled opaque asserts still pass.

**`host/web/input-glue.js:342` and `host/web/src/host/looks.rs:6` — leftover sheet, and two comments.** The last `drop` (`354–358`) deletes the rule and leaves the empty `CSSStyleSheet` adopted; `paintKeys` only increases. `adoptedStyleSheets` is assigned outside the `try` that guards `new Sheet()` (`340`). The new selector is the same `(1, 2, 0)` as the old index.html rules and beats the framework hover (`looks.rs:46` is `(0, 4, 0)`), and an author inline `!important` still wins (`press.test.mjs:877–880`). Because an adopted sheet cascades after the document's sheets, an author `!important` rule of equal specificity that used to win by coming later in the document now loses. `looks.rs:6` still says the paint is "index.html's important rules".

Checked and sound: opacity-to over the share, the `[0, 1]` clamp, alpha left on the 0–1 scale, p=0 equality for legacy `rgb()` and for the opacity strings above, `indexOf` on the rule object after other deletions, integer keys, the sheet built in the node's own realm, cleanup on pointer release and on `letGo` (including remove-then-mouseup, `press.test.mjs:867–874`), repaint on style/class, scheme, head edits, and `exact.repaintPress`, agent mode finishing immediately, reduced motion still easing. `getAnimations()` with no options does not include pseudo-elements (Web Animations, 8 Oct 2026), so a `::after` opacity transition is not read into the element's look beneath. Straight-alpha mixing of translucent colors is the approved change. Shadow roots stay unpainted because `closest("#exact-root")` does not cross a shadow boundary; the old rules were the same.

Verdict: the reported transition-toward-initial-values bug is fixed on the rgb/opacity path and those tests go red without the fix; the `color-mix` fallback still breaks that invariant for `lab()`/`oklch()` paints.
