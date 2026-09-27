# LLP 1066: Gradients — CSS `background-image`, one layer, every host

*Numbered 1056 on its branch; renumbered 2026-09-27 when main's LLP 1056
(Canvas 2D) landed first.*

**Type:** RFC
**Status:** Implemented 2026-09-26
**Systems:** Kernel (`schema.json` style bit 144, `kernel/src/gradient.rs`), Contract (`background-image` in `tags.rs`, refusals in `values.rs`), Web host (`css.rs`), Apple host (`style.rs` gradient_json, `Gradient.swift`, `BoxLayerIOS.swift`, the node views' `draw`), Linux host (`paint/gradient.rs`, the tiny-skia and Vello backends)
**Author:** Claude (Opus 5.5) for Seth Webster
**Date:** 2026-09-26
**Related:** LLP 1014 §5 (took the old gradient rows out; this brings one back in CSS's shape), LLP 1034 (`light-dark()` colours, resolved by the host), LLP 1043.000 (`clip-path`, the CSS-text row this copies), LLP 1053 G2 (the border parity page this copies), `rules/DEFERRED.md` (the gradient line, moved off 2026-09-26; Charlie has not ruled on the take).

## Summary

An app ported onto exact2 (grnl, a voice journal) wanted a protection
gradient under a floating Record button: transparent into the page colour, so
the entries scrolling under it fade out. There was no way to say it except a
canvas, so the port used a flat veil. This RFC adds CSS `background-image`
for `linear-gradient()` and `radial-gradient()`. It is one kernel row, carried
as CSS text and painted natively on every host. Chrome is the oracle: a
twelve-case parity page (`scripts/fixtures/gradients.contract`) and Chrome's
pictures of it hold Linux's two painters and both Apple views within the
border page's band.

## Design

**D1. One CSS-valued row, bit 144.** `background_image` has its own codec,
`background-image`, carrying CSS text on the wire as `clip-path` and
`shape-outside` do. The kernel parses the
text into a `BackgroundImage`, validates it, and uses the canonical text as
the wire form. It is not a layout row. Stops are fixed up when parsed
(CSS Images 3 §3.5.3: first 0%, last 100%, positions made nondecreasing, runs
spread evenly), so every host gets the same positions.
*Rejected:* separate type/angle/colours rows, as in the predecessor. That
structure is not CSS, and an author could not write what they know.

**D2. The grammar that is implemented.** It covers `none`;
`linear-gradient([<angle> | to <side-or-corner>,] <stops>)` with deg, grad,
rad and turn; and `radial-gradient([circle | ellipse] [<extent-keyword>]
[at <position>], <stops>)`, where a position has one or two values (keywords,
px or %). A stop is `<color> [<pct> [<pct>]]`, and the colour is anything a
colour row takes, including `light-dark(a, b)`. There are at most 64 stops.
The kernel's parser returns a reason for each refusal. The compiler reports
that reason for a literal, including inactive branches, and names the
function it refuses: `repeating-*`, `conic-gradient()`, `url()`,
`image-set()`, `cross-fade()`, several layers, colour hints, length stop
positions, positions outside 0–100%, explicit radial sizes, and three- or
four-value positions.
*Rejected:* accepting the rest and approximating. A host that paints
something else than Chrome is a parity bug that nothing reports.

**D3. Colours resolve on the host, per appearance.** A stop holds a
`ColorValue` (LLP 1034 D1). The web hands `light-dark()` to the browser
inside the gradient. Apple sends both appearances' stops when any stop is a
pair, and the view picks by its own appearance. iOS re-applies styles on a
trait change. On macOS, `hasSchemeColor` counts a gradient that carries
`dark`. Linux captures the stops with the box for the painter's `dark`, and a
`setScheme` repaints.

**D4. Mix premultiplied, as CSS does.** Transparent into white never passes
through grey. The browser and Vello (`InterpolationAlphaSpace::Premultiplied`,
its default) do this natively. Core Graphics, Core Animation and tiny-skia
mix unpremultiplied, so the kernel's `premultiplied_ramp` rewrites their
stops. A fully transparent stop takes each neighbour's hue on that side, which
is exact. A stretch between two different partial alphas is sampled eight
times in premultiplied space. `resolved()` always starts at 0 and ends at 1,
repeating the end colours, because Vello's ramp starts its first stretch at 0
whatever the first offset is.

**D5. Painted where CSS paints a background image.** It goes over
`background-color` and under the border and the content. Its placement is the
padding box, which is CSS's gradient box under the initial
`background-origin`. It fills the border box and is clipped to its rounded
outline (the initial `background-clip`), and by `clip-path` and overflow as
the background is. Placement is `Gradient::geometry`: a linear line through
the centre of length `|W sin θ| + |H cos θ|`, `to <corner>` perpendicular to
the other diagonal, and the four radial extents, with the ellipse corners
keeping their side form's ratio. A radial shape with no area is its last
colour everywhere. Swift repeats this arithmetic because only the view knows
its box.
*Deviation:* past the padding box, native hosts extend the end colours under
the border, where CSS repeats the image (`background-repeat: repeat`). Only a
translucent border shows the difference.

**D6. Per host.**
- **Web:** `background-image:` plus the kernel's canonical text.
- **Linux:** `BoxPaint` captures the gradient. Both backends gain
  `fill_gradient`: tiny-skia takes linear and radial shaders (an ellipse is
  the unit circle scaled), and Vello takes a `peniko::Gradient` with a brush
  transform. Content-region pictures carry it through the same `BoxPaint`.
- **iOS:** a `CAGradientLayer` at sublayer index 0, over the layer's
  background and under its border, image and children. It takes the layer's
  one radius and is re-placed on every `display`, so resizes and appearance
  changes update it. Core Animation needs two corrections to match Chrome.
  It mixes stops in its own colour space, which put red into blue a mean
  5.5/255 off, so each stretch between two hues is sampled sixteen times in
  sRGB. It also draws bands perpendicular to the line in the unit square,
  which a non-square box skews (8/255 at 30°), so the unit line is aimed
  along `S·d` and sized to end on the CSS end line. A box that `draw(_:)` already paints (different radii or
  sides, or text without a raster) gets the gradient painted there with Core
  Graphics, between the background and the border, because a sublayer would
  cover that bitmap.
- **macOS:** Core Graphics in `draw`, between the background and the border.
  A gradient counts as box paint.
*Rejected:* a `CAGradientLayer` on macOS. The Mac box is drawn, so a sublayer
would cover the drawn border and text.

## Verification

- Kernel: grammar and canonical round trip, the fix-up, refusals by name,
  linear and radial geometry, the premultiplied ramp, scheme resolution, and
  the patch codec (`kernel/src/gradient/tests.rs`).
- Contract: the attribute, a `style`, a conditional, `none`, and named
  refusals (`contract/cli/tests/it/styles.rs`).
- Web: the declaration (`host/web/src/css.rs`).
- Apple: the wire JSON (`host/apple/src/style.rs`), plus `GradientParity` in
  the Mac and iOS border parity classes, compared to Chrome's pictures.
- Linux: `tests/pinned/gradients.rs`, CPU and GPU, compared to Chrome's
  pictures light and dark.
