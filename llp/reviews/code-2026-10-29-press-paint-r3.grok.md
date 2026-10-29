# Code review: web press paint keeps authored transitions, round 3 of 3 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `eb11b4ebd4`.
- **Method:** one brief (sha256 `45149f103501498099f5bd6c538759dce1da8c2467099c02df421cf7a3342df9`), shared by both reviewers, blind to the other review. The author (Claude, dtool3) is not a reviewer.
- **Transcription:** the final message, unedited.

## Findings

## BLOCKER

**Release and repaint start an opacity or color transition and then withhold the paint** — `host/web/input-glue.js:271` (`removeAttribute`), flushed at `host/web/input-glue.js:309`, then `later()` at `host/web/input-glue.js:312`.

`paint()` drops `data-exact-painting` and calls `getComputedStyle` before the new rule is installed. Press-down is safe: the attribute was absent, and the new opacity or color value at `--exact-press-paint: 0` matches the look beneath, so nothing starts. Release, scheme change, and any `style`/`class`/`head` mutation call `paint()` again while progress is not 0. That flush resolves opacity and color back to the author value. If the node declares `transition: opacity`, `transition: color`, or `transition: all` (the pattern in LLP 1061), the engine starts a transition from the pressed value to the author value. `later()` sees it and leaves that property out of the rule, so the press curve never runs; the transition the paint just started does. When it finishes, `repaint` puts the press back, so a held control fades to the resting look and then jumps. `endPaint` does not cancel that transition, so it keeps running after the press is gone. Re-press before it finishes hits `later()` again and shows no press opacity or color. Nothing in `press.test.mjs` samples the transition list after pointer-up or after a mid-press repaint; `settle()` finishes whatever was started.

## MAJOR

**Prepended layer lists phase-shift the author's layers at progress 0** — `host/web/input-glue.js:315` and `host/web/input-glue.js:330`.

Only some longhands are shifted, and only when a layer already exists. `background-size` gets one `auto` prepended; `background-position`, `background-repeat`, `background-origin`, `background-clip`, and `background-attachment` are left as the author's list. Masks prepend `mask-clip`, `mask-size`, and `mask-composite`, and not `mask-position`, `mask-repeat`, `mask-origin`, or `mask-mode`. A list shorter than the new layer count cycles. Two images with `background-position: left, right` become paint-at-left, first image at right, second at left, while the paint layer is still transparent. The same shift hits a second mask. One image with one size is fine (`auto, 20px 20px` keeps the author's size), which is the only case `press.test.mjs:1008` builds. With `background-image: none`, size and position are not set at all, so an authored `background-size` shrinks the press colour to that box. An authored `mask-mode: luminance` is also cycled onto the black press mask, which hides the element at progress 0. LLP 1061 (`llp/1061-press-feel-and-reduced-motion.rfc.md:282`) says a property the paint writes is exact at rest or not painted. `background-size` is neither, under `transition: all` or `transition: background`.

## MINOR

**Transparent pressed text divides by zero at progress 1** — `host/web/input-glue.js:338`. For an opaque `rgb()` base and a pressed `color(srgb … / 0)`, the channel formula divides by `1+(a-1)*p`. At progress 1 that is `0/0`, the `color` declaration is invalid, and the text snaps off the fade. `color-mix` on the background layer does not have this hole. The exact path is only taken when both regexes match (`host/web/input-glue.js:334`); a `color(srgb)` copy serialized with `none` components or a percentage alpha skips it.

**`pressShown` is not the painted pixels** — `host/web/input-glue.js:347`. The canvas fills `background-color` only, then the sRGB copy at `globalAlpha = paint`, and reads 8-bit premultiplied pixels (alpha to 3 decimals). Author images and masks are not in that composite, so a row wash is reported as flat colour. A translucent base is flattened against transparent before the press is applied, including at progress 0. No `OffscreenCanvas` leaves `backgroundColor` as the unpainted computed colour. A null 2D context throws. `host/web-js/agent.js:244` calls `pressShown` twice (the `globalThis` check, then `exact.pressShown`). `glue.js:1055` calls it once. The halfway assertion (`press.test.mjs:981`) reads this canvas string, not a screenshot, so a CSS layer that disagreed with the canvas would still pass. `rgba(0, 0, 255, 0.5)` does not match that regex; only the 8-bit `0.498`/`0.502` forms do.

**The adopted sheet is never removed, and a shadow root cannot match** — `host/web/input-glue.js:370` and `host/web/input-glue.js:308`. `drop` deletes rules; the `CSSStyleSheet` stays on `adoptedStyleSheets` for the life of the document. `paintRule` returns `""` unless `closest("#exact-root")` hits, and the rule selector is `#exact-root …`, which does not match a node inside a shadow tree. `getRootNode()` would attach the sheet to that shadow root anyway. Header back buttons are shadow nodes (`host/web/navigation.js`).

**LLP overstates which `!important` wins** — `llp/1061-press-feel-and-reduced-motion.rfc.md:280`. The paint rule is `#exact-root [data-exact-painting="…"][data-exact-painting]` (`host/web/input-glue.js:381`). Inline `opacity: … !important` beats it (the Chrome check at `press.test.mjs:891`). A stylesheet `opacity: … !important` does not. Firefox's mask is a different property, so that half of the sentence is true. The comment above the Chrome check says the engine test covers the Firefox `!important` case; `press.test.mjs:1005` never sets `!important`.

**Tests that do not prove the claim** — `press.test.mjs:969`, `press.test.mjs:772`, `press.test.mjs:495`.

- Chromium plus `oklch()`/`lab()` forces `seen.same` to `true`, so that cell is green on this branch and on main. `toAuthored` only rejects a transparent end, not a different opaque one. The comment points at a limit "said in the glue"; the glue does not say it. Background colour is no longer rewritten, so this waiver is leftover.
- The stall check accepts any opacity in `[0.399, 0.801]` and any opaque background. White or yellow with no press layer passes. It does reject opacity 1 and transparent, which is the old bug.
- No test declares `transition: color`, a non-`rgb()` text colour, a transparent press colour, a second background layer, or a release while `transition: opacity` is declared. Y1 can be wrong and every current colour assertion still passes through the `color-mix` fallback (`host/web/input-glue.js:340`), because those nodes do not declare a colour transition.
- `liftedSoon` (`press.test.mjs:495`, used at `510` and `520`) skips the quick-tap asserts when the round trip exceeds 150ms, so under load those checks pass without running.
- The 60s figures (`press.test.mjs:313`, `581`, `680`, `919`, `990`, `1042`) are the test-function budgets around a per-engine browser, not animation bounds. The paint-end polls are 2s of frames (`720`, `742`, `871`) and can go red if the finish event is late. The stall waiter stops at 900 frames. `suiteBrowser` shares the process browser; the empty `finally` at `1041` does not leak it.

`transitioned()` (`host/web/input-glue.js:291`) matches `all`, the longhand, and a cycled duration, and treats `0s`/`0ms` as not declared. `running()` (`296`) sees `running` and `paused`. `later()`'s `finished` handler is ordered correctly for one live transition, including a re-press of the same animation object and `endPaint` before it resolves. The opacity number path is exact at progress 0 for a finite `computedStyleMap` value; base 0 does not divide; a raise with no engine float and a declared transition is not painted; the mask is only used when it cannot need to increase opacity. Those parts hold.

## NIT

**Dead shorthand table and a swallowed cancel** — `host/web/input-glue.js:290` and `host/web/input-glue.js:312`. `HOLDS["background-color"]` is never consulted: `transitioned()` is only called for `opacity` and `color`, which have no shorthand besides `all`. The `finished` rejection handler is empty, so a transition cancelled without a style, class, or head mutation never gets a new listener (`state.waits` still holds the old object).

Verdict: do not take this round — press-down is exact, but a declared opacity or color transition is started again on every release and repaint, and multi-layer background and mask lists are wrong at progress 0.
