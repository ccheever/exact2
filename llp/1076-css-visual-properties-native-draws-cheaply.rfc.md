# LLP 1076: The CSS visual properties native hosts draw cheaply

**Type:** RFC
**Status:** Accepted 2026-10-02 (r2; every question in §7 ruled). Stages 1–4 and §5 built 2026-10-02 on lane/visual-1076 (§8); review and landing owed.
**Systems:** Kernel (`schema.json` style rows from bit 154, `kernel/src/gradient.rs`, hit testing), Contract (`tags.rs` attributes, `values.rs` refusals), Web host (CSS from rows), Apple host (iOS layers, macOS `draw`), Linux host (painter: Vello and tiny-skia)
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Implementer:** Claude (Opus 5.5), from 2026-10-02; Astra and Grok review the build before it lands (Charlie, 2026-10-02).
**Date:** 2026-10-02
**Related:** LLP 1001 §1 (the style table; new rows and declared deviations land there), LLP 1053.000 (`backdrop-filter`, `backgroundMaterial`), LLP 1055.000 D10/D14/D19 (SVG `mask`, `filter`, `mix-blend-mode`: the island route these defer to), LLP 1061–1064 (paint motion, `box-shadow`, `text-shadow`'s precedent), LLP 1066 (`background-image`; its grammar is reused and widened), LLP 1035.004 (native affordances by meaning: the precedent for §5), `rules/DEFERRED.md` (the "decorative effects" lines; moved 2026-10-02, §1)

## Summary

These are CSS visual properties that iOS draws with public Core Animation or Core Text API at little or no cost, that the browser draws natively, and that authors otherwise fake or do without:

| | CSS | iOS mechanism | Stage |
|---|---|---|---|
| D1 | `corner-shape` (`squircle`, `superellipse()`, …, and `-apple-continuous`) | a path from one kernel function; `cornerCurve = .continuous` for `-apple-continuous` | 1 |
| D2 | `mask-image` (one gradient) | `layer.mask = CAGradientLayer` | 1 |
| D3 | `text-shadow` (one shadow) | `CGContext.setShadow` around the run | 1 |
| D4 | `box-shadow` lists, `inset`, spread | one `shadowPath` sublayer per shadow | 2 |
| D5 | `conic-gradient()`, stacked `background-image` layers | `CAGradientLayer` `.conic`, one sublayer per layer | 2 |
| D6 | `background-clip: text` | gradient layer masked by the text | 3 |
| D7 | `-webkit-text-stroke` | `CGContext` `.fillStroke` text mode | 3 |
| D8 | `perspective`, `rotate: x\|y …`, `backface-visibility` | `CATransform3D`, `sublayerTransform.m34` | 4 |

One property is **deferred**: `mix-blend-mode` on boxes (D9). iOS has no public per-layer route for it. (`filter` on boxes already exists: LLP 1055.000 §0.) Nine iOS affordances with **no CSS name** are admitted too (§5, D10–D18). Charlie ruled them in, names included.

Every row follows the house rules for a new visual row. CSS's grammar is admitted as a stated subset and the rest is refused by name, at compile time for a literal and at run time for a computed string. The web draws natively and is the oracle, so the native hosts are held to Chrome's pixels. Each feature links only where it is used, so an app that uses none of them pays no web-core bytes.

## Motivation

The visual rows today: `backdrop-filter: blur()`, `backgroundMaterial`, one linear or radial `background-image`, one outer `box-shadow` without spread, 2D `translate`/`scale`/`rotate`, `clip-path`, `opacity`, `tint-color`, `accent-color`. `filter` on a box is drawn natively on the web and as a picture on Apple (LLP 1055.000 §0), and `mix-blend-mode` only on SVG elements. `backdrop-filter`'s other filter functions are refused (`contract/cli/tests/it/styles.rs:504`), and iOS declares blend modes unsupported.

Each item in the table is a single line of CSS on the web and a few lines of public API on iOS. Without them an author falls back to an SVG island, a Canvas surface, or a design change:

- **Continuous corners.** Every UIKit and SwiftUI control and the native Messages bubbles use them. `apps/messages` is held to native iOS pixels with 31 `border-radius` uses, all circular arcs.
- **Edge fades** on a carousel, a horizontally scrolling chip row, or truncated text. A fade toward a known background colour can be faked today with a gradient overlay. A fade toward whatever is behind the box cannot be.
- **Legible text over imagery** (`text-shadow`), and **gradient or outlined headings** (D6, D7).
- **Card flips and tilt** (D8).

## 1. The DEFERRED move

`rules/DEFERRED.md` is binding and says twice that "decorative effects wait behind" named consumers: behind the four interaction workloads (2026-09-16), and behind the sheet and the Shop accordion (2026-09-17/19). Every item here is a decorative effect. Earlier visual rows moved by naming a consumer and a take. LLP 1061–1064 and 1066 were "for grnl", and Charlie later ratified them with the take waived.

**Ruled (Charlie, 2026-10-02): "Waive the take."** He added that ports are starting to ask for almost all of these, so the waiver covers all four stages. The line moved in `rules/DEFERRED.md` the same day. D9 (`mix-blend-mode` on boxes) stays out.

## 2. Shared rules

- **Rows.** Each is a CSS-text row, like `clip-path` and `background-image`. The kernel parses the text once in one Rust function, so a literal, a `style` block, a class choice and a computed string behave alike. New bits start at 154. LLP 1001 §1 gains the rows and any declared deviation.
- **Refusals by name.** Each D lists what is admitted. Anything else in CSS's grammar is refused with the function or keyword named, as LLP 1066 does. Approximating instead is rejected: a host that paints something other than Chrome is a parity bug that nothing reports.
- **Geometry in the kernel, once.** Shapes that more than one native host draws (D1's corner path, D4's spread and inset outlines, D5's conic placement) are kernel functions. Swift and the Linux painter consume their output and do not re-derive it. LLP 1066 repeated its gradient arithmetic in Swift; these rows do not.
- **Size.** The web-core ceilings in `scripts/metrics.mjs` gate the landing. Each feature's web emit and runtime code link only when a plan uses its row.
- **Motion.** A row CSS calls animatable takes `transition` and keyframes through the paint-motion seam (LLP 1061–1064). Each D states how its value interpolates.
- **Performance is measured on a device.** A mask or an extra shadow can cost Core Animation an offscreen pass. Each Apple design below names the trap it avoids. Each stage's landing includes a frame-level check on an iPhone, with the content under the effect scrolling, not just a screenshot.

## 3. Design

### D1. `corner-shape`

CSS Borders 4, shipped in Chrome. Admitted: the four-corner shorthand and the per-corner longhands, with values `round` (the initial value), `squircle`, `square`, `bevel`, `scoop`, `notch` and `superellipse(<number> | infinity | -infinity)`. All of these are one parameter K, which `squircle` sets to 2, `round` to 1 and `bevel` to 0. Refused: nothing in the property's grammar. Like the radius, the shape needs a `border-radius` to show.

One keyword is added outside CSS: **`-apple-continuous`**, Apple's continuous corner curve. Each name means one shape on every host. `squircle` is CSS's superellipse everywhere, and `-apple-continuous` is Apple's curve everywhere (Q2, Charlie's proposal). The `-apple-` prefix follows WebKit's own (`-apple-system`) and cannot collide with a keyword CSS adds later.

- **Row:** four f32 K values, one per corner, with one value reserved for `-apple-continuous`. A box without the row keeps today's arc fast path on every host.
- **Kernel:** `corner_path(box, radii, k[4]) -> path`, the superellipse corner as CSS Borders 4 defines it, together with the inner border edge and the padding-box clip. Borders follow the shape, as in Chrome. For `-apple-continuous` the function returns Apple's curve: three cubic segments per corner reaching about 1.53× the radius along each side, eased back to a circular arc when the box is too small for that, as UIKit does. The constants are fitted to `UIBezierPath(roundedRect:cornerRadius:)` pixels on a simulator, not taken from a third-party write-up.
- **Web:** native for CSS's keywords. `-apple-continuous` emits the nearest `superellipse(K)` with a radius scaled to the same extent. This is a declared approximation in LLP 1001, and its error against Apple's pixels is measured and stated before stage 1 lands. If the error is visible, the web draws the kernel path as a `clip-path: path()` plus a background, and the border loses its shape.
- **Apple:** `-apple-continuous` is `cornerCurve = .continuous` with the layer's own radius, which costs nothing and needs no path. On macOS, where boxes are drawn, it is the kernel path. Every other keyword is the kernel path drawn as the background and border. For `overflow: hidden` it is also the content clip: a `CAShapeLayer` mask on iOS, a clip in `draw` on macOS. A box that does not clip draws the path into its own layer contents and needs no mask, so it costs no offscreen pass. `shadowPath` takes the same path.
- **Linux:** the kernel path through both backends' existing path fill and clip.
- **Motion:** K interpolates as a number, per corner.
- **Why two names:** Apple's `.continuous` is not CSS's `superellipse(2)`. The continuous curve starts about 1.5× the radius back from the corner, while CSS's shape stays inside the radius box. With one name, either iOS would not match native apps or it would not match Chrome. With two, an author picks the look, and both mean the same shape on every host. `-apple-continuous` to `squircle` interpolates discretely; between the CSS keywords, K interpolates.

### D2. `mask-image`

Admitted: `none`, or one `linear-gradient()` or `radial-gradient()` in LLP 1066's grammar, with the same refusals. Initial `mask-mode` (alpha for a gradient), `mask-origin`, `mask-clip`, `mask-size` and `mask-position`: the gradient covers the border box. Refused: `url()`, several layers, `mask-composite`, `mask-mode: luminance`, and every other `mask-*` longhand. On a box, the `mask` shorthand stays refused. `mask: url(#id)` remains SVG-only (the `svg_mask` row, LLP 1055.000 D10).

- **Row:** the gradient, parsed by `kernel/src/gradient.rs` with D4 of LLP 1066 (premultiplied stops) applied unchanged.
- **Web:** native.
- **iOS:** `layer.mask = CAGradientLayer`, with LLP 1066's two Core Animation corrections. On a `backgroundMaterial` or `backdrop-filter` box, the mask goes on the effect view's `mask`, which UIKit supports. A mask forces an offscreen pass for the masked subtree each time it changes. The device check (§2) measures a masked scroll view at 120 Hz before the stage lands.
- **macOS:** the box draws into a transparency layer clipped by the gradient's alpha.
- **Linux:** tiny-skia multiplies a `Mask` (`raster.rs` already composes masks). Vello's route for an alpha mask over a layer is confirmed or built in stage 1; until it is, Vello renders the subtree as an island, as SVG masks do.
- **Progressive blur.** `backdrop-filter: blur()` plus a `mask-image` gradient fades a uniform blur. This is what Chrome draws, so iOS's masked effect view is parity, not an approximation. A blur whose radius varies across the box is not a CSS feature and is not proposed. Apple's `variableBlur` filter is private.
- **Motion:** stops interpolate as LLP 1066's gradients would, when both ends have the same function and stop count. Otherwise the change is discrete.

### D3. `text-shadow`

Admitted: `none`, or one shadow, `<color>? <length>{2,3}`, inherited as in CSS. The shadow also falls under text decorations, as Chrome paints it. Refused: a list, for the reason LLP 1064 D1 refused one for `box-shadow`: a list needs one extra draw per entry on every host, and no one has asked for it. A missing colour is `currentcolor`, which this row can hold because the text colour is in hand when it draws. LLP 1064's shadow rows could not hold it.

- **Web:** native.
- **Apple:** `CGContext.setShadow(offset:blur:color:)` around the run's draw in the text engine, with blur radius = 2σ as in LLP 1064 D2. The host does not use an `NSShadow` attribute, because Core Text drawing ignores it.
- **Linux:** the run's glyph coverage is blurred and drawn offset, under the glyphs, by the existing shadow blur.
- **Motion:** colour, offsets and blur interpolate as `box-shadow`'s do.

### D4. `box-shadow`: lists, `inset`, spread

LLP 1064 D1 refused all three. This admits CSS's full `box-shadow` grammar except a missing colour, which stays refused for the reason LLP 1064 gives.

- **Rows:** one `box_shadow` CSS-text row (a list) **replaces** the four rows at bits 57–60. Under "delete, don't deprecate", the four rows go, and LLP 1001 and LLP 1064's hosts move to the list (Q4, ruled). Authors write the same `box-shadow` text; only the wire changes, and baked bundles get a new compatibility id. Glass groups read the shadow rows (`GlassGroup*Tests.swift`) and move with them.
- **Kernel:** each shadow's outline. Spread grows or shrinks the border-box outline, and its radii follow CSS's spread-radius rule (Backgrounds 3 §7.1.1). An inset shadow is an even-odd path: the padding-box outline with a hole shrunk by the spread.
- **Web:** native.
- **iOS:** outer shadows are one sublayer each, with an explicit `shadowPath` and no contents, kept outside the border box as LLP 1064 D2 does now. With a `shadowPath` Core Animation casts without an offscreen pass. Inset shadows are one sublayer each above the background and under the content, clipped to the padding box. The first outer shadow stays on the box's own layer, so a one-shadow box costs what it costs today.
- **macOS and Linux:** the existing shadow paint loops over the list. Inset is a clip to the padding box and an even-odd fill.
- **Motion:** lists interpolate pairwise, with the shorter list padded by transparent zero shadows, as CSS does. If inset and outer don't match at the same index, the change is discrete.

### D5. `conic-gradient()` and stacked layers

This widens LLP 1066's grammar. Admitted: `conic-gradient([from <angle>]? [at <position>]?, <stops>)` with 1066's stop rules, and a comma list of up to four layers mixing all three functions, painted with the first layer on top as in CSS. Still refused: `repeating-*`, `url()`, colour hints, and everything else 1066 refuses.

- **Web:** native.
- **iOS:** one `CAGradientLayer` per layer, with type `.conic` for a conic gradient. A box that `draw(_:)` paints (LLP 1066 D6) draws a conic gradient per pixel with Core Graphics, as `Canvas2DPaint.swift`'s `drawConic` does today.
- **macOS:** Core Graphics in `draw`. A conic gradient uses the same per-pixel route.
- **Linux:** Vello's sweep gradient. tiny-skia's support for a sweep gradient is confirmed at the start of the stage; if it has none, the gradient is drawn per pixel as Apple's Canvas does.

### D6. `background-clip: text`

Admitted: `border-box` (initial), `padding-box`, `content-box`, `text`. With `text`, the background colour and image are painted only inside the glyphs of the box's own text. The author sets `color: transparent` to see it, as in CSS; the compiler does not insist. Refused: `-webkit-` spellings, which are aliases Chrome keeps only for compatibility.

- **Web:** native.
- **iOS:** the gradient layer's mask is a layer that draws the same runs the text node draws, from the same layout, so the two never disagree. This costs one offscreen pass per text change, not per frame.
- **macOS:** the runs' glyph paths become a clip in `draw`.
- **Linux:** the glyph outlines' union as a clip path in both backends.

### D7. `-webkit-text-stroke`

The prefixed name is the only one CSS has, and the Compat Standard specifies it for every browser. Admitted: `-webkit-text-stroke-width` (a length), `-webkit-text-stroke-color` (initial `currentcolor`), and the shorthand. Inherited, as in Compat. The stroke is centred on the outline and painted over the fill, as Chrome draws it. Under `paint-order: stroke`, which Chrome honours on HTML text, it goes under the fill, using the existing `paint_order` row (bit 126).

- **Web:** native.
- **Apple:** `CGContext.setTextDrawingMode(.fillStroke)` with the line width in points, or two passes when paint order requires it.
- **Linux:** glyph paths stroked after (or before) the fill.

### D8. 3D transforms: `perspective`, axis rotations, `backface-visibility`

Admitted: `perspective` (on the parent) with `perspective-origin`; the individual `rotate` property's axis forms (`x <angle>`, `y <angle>`, `<x> <y> <z> <angle>`); a `z` component on `translate`; and `backface-visibility`. Refused: `transform-style: preserve-3d`, so every box flattens its children into its own plane (CSS's initial `flat`); `perspective()` and `matrix3d()` inside `transform`, since the box rows are the individual properties; and 3D on SVG elements.

- **Rows:** `rotate` widens from one f32 to an axis and an angle. `translate` gains z. There are new rows for `perspective`, `perspective-origin` and `backface-visibility`.
- **Kernel:** hit testing and the agent's `tree` geometry go through the inverse projection. A tap on a tilted card must land on the pixel it shows, and `layout <node>` reports the projected bounds. This is the stage's real kernel cost.
- **Web:** native.
- **Apple:** `CATransform3D` on the layer. The parent's `perspective` becomes `sublayerTransform.m34 = -1/d` about `perspective-origin`. Core Animation flattens sublayers by default, which is CSS's `flat`. `isDoubleSided` is `backface-visibility`. Rotation about x and y animates on the render server like today's 2D transforms.
- **Linux (Q3):** Vello 0.10 and tiny-skia 0.12 draw affine transforms only. Vello's image command carries a 2×2 matrix and a translation, and tiny-skia's `Transform` has six fields; neither has a perspective term. Skia proper has one (`SkMatrix` is 3×3, and `SkM44` exists). Even so, Chrome does not draw CSS 3D by rasterizing in perspective: its compositor draws each 3D layer as a cached texture on a projected quad, and Core Animation does the same. The Linux host follows them:
  - The subtree under a 3D transform renders flat into an island (the LLP 1055.000 route), cached until its content changes.
  - **Vello:** a patch adds a projective image brush. The image command gains a perspective row (two more floats), and the fine shader divides by w when it computes the sample coordinate. The projected quad itself is an ordinary path fill, a four-point polygon. The patch goes on `vendor/vello` (`exact-vello`, which today serves only Canvas 2D), the Linux painter moves onto it so the tree has one Vello, and the change is proposed upstream to Linebender.
  - **tiny-skia:** a CPU inverse-homography warp with bilinear sampling, in `host/raster` beside the other island code.
  - **Fallback:** if the Vello patch is not working after three rounds, Linux draws the orthographic projection instead: the rotation's affine 2×2 without foreshortening, which both backends draw today. Flips, tilts and back faces stay right, only without perspective. This is declared in LLP 1001 until the patch lands.
- **Motion:** axis-angle rotations interpolate by slerp when the axes differ, as CSS does. Perspective interpolates as a length.

## 4. Deferred

### D9. `mix-blend-mode` on boxes

*Corrected 2026-10-02.* r1 and r2 deferred `filter` on boxes too, but it already exists: LLP 1055.000 §0 (2026-09-29) draws it natively on the web and as a picture on Apple (`BoxFilter`: the box drawn unfiltered, the chain run on the GPU, redrawn after each batch). Linux still draws the box unfiltered; QUEUE carries that. Nothing in this RFC changes it.

`mix-blend-mode` on a box stays deferred. iOS documents `compositingFilter` as unsupported, and SwiftUI's `.blendMode()` uses private `CAFilter`, which risks App Store rejection and can break silently in an OS update. The public route would be `BoxFilter`'s: picture the box and what is under it, then blend. But a blend reads the backdrop, so the picture would have to be redrawn whenever anything under the box changes, not only after a batch.

**Return trigger:** a port that blends a static box over static content.

## 5. iOS affordances with no CSS name

*Ruled (Charlie, 2026-10-02): "do all of them."* These follow the `press-scale` and LLP 1035.004 precedent: a declared thing, rendered by the host that has it, with the web's form declared rather than imitated. Where WebKit already has a spelling, it is used. Otherwise the name is a plain non-CSS row or prop, declared as such in LLP 1001. The names below were approved with the rest (Charlie, 2026-10-02: "assume they are fine").

| | Proposed spelling | Apple | Web | Linux |
|---|---|---|---|---|
| D10 | `symbol-rendering: monochrome \| hierarchical \| palette \| multicolor` and `symbol-palette: <color>{1,3}` on a `symbol:` image | `UIImage.SymbolConfiguration` rendering mode and palette colours | monochrome in `color` | empty, as symbols are today |
| D11 | `symbol-value: none \| <number 0–1>` | `UIImage(systemName:variableValue:)` | full symbol | empty |
| D12 | `symbol-effect: none \| bounce \| pulse \| wiggle \| breathe \| rotate \| variable-color \| scale \| replace`, plus `symbolEffectValue=<expr>`: a discrete effect (`bounce`, `wiggle`) plays each time the value changes, an indefinite one runs while set, and `replace` plays when the source changes | `addSymbolEffect` / `setSymbolImage(_:contentTransition:)` | keyframe stand-ins for bounce, pulse and scale; nothing for the rest (declared) | nothing |
| D13 | Vibrancy as colours, in WebKit's spellings: `-apple-system-label`, `-apple-system-secondary-label`, `-apple-system-tertiary-label`, `-apple-system-quaternary-label`, `-apple-system-separator`, and a fill colour checked against WebKit's keyword list before it is named. Under a `backgroundMaterial` they are vibrant; elsewhere they are the plain system colours | `UIVibrancyEffect(blurEffect:style:)` wrapping the node under a material; `UIColor.label` etc. elsewhere | WebKit's own names in Safari; a `light-dark()` pair elsewhere | the `light-dark()` pair |
| D14 | `press-haptic: none \| selection \| impact-light \| impact-medium \| impact-heavy \| impact-soft \| impact-rigid`, a host-owned row like `press-scale` (no runner round trip), and a `haptic("success" \| "warning" \| "error" \| …)` action for app logic | `UIImpactFeedbackGenerator`, `UISelectionFeedbackGenerator`, `UINotificationFeedbackGenerator`, prepared on touch-down; macOS `NSHapticFeedbackManager` on a Force Touch trackpad | `navigator.vibrate` where it exists; nothing on iOS Safari | nothing |
| D15 | `content-transition: none \| numeric \| numeric-countdown` on text: digits roll when the string changes | a per-glyph roll in the text layer, as SwiftUI's `.numericText` looks; UIKit has no public equivalent, so it is built from per-digit layers | a per-digit roll in the runtime, linked by use | a per-digit roll in the painter |
| D16 | `scroll-edge-effect: automatic \| soft \| hard \| none`, per axis edge on a scroll container | `UIScrollEdgeEffect` (iOS 26) | a `mask-image` fade (D2) for `soft`; a hairline for `hard` | the same as the web |
| D17 | `hover-effect: auto \| highlight \| lift \| hover \| none` (iPad pointer) | `UIPointerInteraction` with `UIPointerEffect` | `:hover`-style tint for `highlight`, nothing else (declared) | nothing |
| D18 | `smart-invert: auto \| ignore`. `ignore` is the default on `image`, `video` and `canvas` | `accessibilityIgnoresInvertColors` | nothing (Safari has only the `inverted-colors` media query) | nothing |

- **D14 is the only one with an action.** A haptic on press must fire on touch-down with no runner round trip, as `press-scale` does. A haptic on a state change (a swipe passing its threshold, a save succeeding) belongs to app logic.
- **D15 is the most work.** UIKit has no public numeric content transition, so iOS builds it from per-digit layers. Its look is matched to SwiftUI's by eye, with no parity oracle; the RFC says so.
- **D17 overlaps LLP 1075.003**, which can already attach a `UIPointerInteraction` through a hook. The declared row is the common case without native code; the hook stays the escape hatch.
- **D18's default** (`ignore` on media) is what Apple's own apps do, so it is verified with Smart Invert on in the simulator.

## 6. Verification

Each stage, before it lands:

- **Chrome as oracle.** Every admitted form gets an `fx-*` box in an existing gallery app (no new app). `scripts/parity.mjs` crops each box from the iOS simulator, macOS and Linux screenshots and compares it with Chrome's within LLP 1053 G2's tolerances.
- **Refusals.** Contract CLI tests show each refused form named at compile time, and named at run time for a computed string.
- **On a device.** A frame-level trace on an iPhone with the effect over scrolling content (D1 with clipping, D2, D4, D6), compared against the same screen without it, following the perf program's protocol. A stage that adds late frames does not land.
- **Size.** `bun scripts/metrics.mjs`: a plan without the rows has unchanged web-core bytes.
- **Driven.** `bun scripts/agent.mjs ios tree "screenshot …"` on the gallery page, plus a tap on a D8 tilted card that lands where it shows.

## 8. As built

### Stage 1 (2026-10-02): `corner-shape`, `mask-image`, `text-shadow`

Rows 154 `text_shadow` (inherited), 155 `mask_image`, 156 `corner_shape`, each CSS text parsed once in the kernel (`style/shadow.rs` `TextShadow`, `gradient.rs` reused for the mask, `corner.rs`). Refusals are named at compile time (`contract/cli/tests/it/visual.rs`).

- **Geometry is the kernel's.** `corner::outline` flattens each corner: a superellipse sampled so its steep ends stay smooth, or Apple's curve as three cubics clamped to the box as UIKit clamps it. Apple's Swift reads it through `exact_corner_outline` (`host/apple/src/corner.rs`), and the Linux painter through `border::shape_path`. Every rounded-rect consumer on both platforms (background, border ring, shadow bands, clips, Vello's and tiny-skia's paths) takes the shape.
- **Apple.** `-apple-continuous` over one radius is the layer's `cornerCurve = .continuous` on the box, clip box, fill, border, gradient and image layers. Any other shape draws through `draw(_:)` (`boxDrawn`/`boxPlan.drawn`), and an overflow clip of that shape is a mask (`BoxMaskIOS.swift`, `BoxMaskMac.swift`). `mask-image` is a `CAGradientLayer` mask (which an overflow or `clip-path` mask itself masks), or the effect view's own mask on a material. `text-shadow` is the raster layer's Core Animation shadow, rasterized; the `draw(_:)` path uses a Core Graphics shadow over a transparency layer.
- **Linux.** `mask-image` is a group: Vello composites the gradient `DestIn` inside a layer clipped to the border box, and tiny-skia draws the group through a frame-sized alpha mask (`gpu/mask.rs`, `raster/mask.rs`). `text-shadow` is the paragraph drawn in the shadow's colour into a CPU island, blurred, and placed under the text by either painter (`paint/text_shadow.rs`).
- **Web.** CSS's own properties. `-apple-continuous` becomes `superellipse(1.6)` over the radius × 1.52 (`css.rs`); for a bound value, the JS target's runtime maps the keyword but does not rescale the radius.

**Measured against Chrome**, on pages `scripts/fixtures/visual.contract` and `text-shadow.contract`, light and dark:

- **Linux, both painters, ten corner and mask cases:** mean ≤ 0.85/255 on every case but one. The GPU `spot` case (a radial mask) is 1.63/255, and no case has more than 0.91% of pixels beyond 48 (`host/linux/tests/pinned/visual.rs`).
- **Text shadows on Linux:** asserted by where the shadow's colour lands, since the fonts differ from Chrome's.
- **macOS and the iOS simulator:** every case matches Chrome by eye, light and dark, driven with `scripts/agent.mjs`.
- **Apple's curve on Linux** (the kernel's) against UIKit's on the simulator: mean 0.57/255 filled, 1.76/255 bordered.
- **The web's stand-in** against UIKit: 1.39/255 filled, 6.49/255 bordered (declared in LLP 1001).

**Owed after stage 1:**

- Motion for the three rows: a transition or keyframes on them is not yet carried by the paint-motion seam.
- Apple XCTest parity classes holding these pages to Chrome's pictures (today the check is by the driven screenshots).
- The iPhone frame check (§6) of a masked scroll view and of a shaped clip.
- A dynamic `corner-shape` naming `-apple-continuous` does not rescale the radius on the web.
- `text-shadow` on a text input.

### Stage 2 (2026-10-02): `box-shadow` lists, `inset`, spread; `conic-gradient()` and layers

- **One row.** Row 57 `box_shadow` (`BoxShadows`, at most eight) replaces LLP 1064's four rows; the schema's later bits moved down three. Authors write the same text. A transition moves the first shadow (the engine's two properties are unchanged) and the rest change at once (LLP 1001).
- **Apple.** `ShadowCaster` is a container masked to outside the border box, with one casting sublayer per outer shadow; its shape is the outline grown by the spread, with CSS's spread radii. `InsetShadowCaster` is a container masked to the padding box, one sublayer per inset shadow casting a frame around the hole; the hole runs the other way round, because Core Animation fills a shadow path by the non-zero rule.
- **Linux.** The band stack is shared by outer and inset shadows. An inset shadow's bands are half as wide, since against Chrome a 1.5-point band showed as a 6/255 step across a padding box.
- **Gradients.** `background-image` holds up to four layers of linear, radial or conic gradients. A conic stop may be an angle. Linux uses tiny-skia's and Vello's sweep gradients, turned so CSS's `from` is where the turn starts (a shifted start angle would wrap mid-sweep). Apple draws a conic gradient as 1,800 wedges that replace each other inside a transparency layer, and draws several layers through `draw(_:)`; a conic mask is a picture.

**Measured against Chrome** on `scripts/fixtures/shadows.contract` and `layers.contract`, light and dark:

- **Linux, both painters, every case:** mean ≤ 1.93/255 (shadows) and ≤ 1.18/255 (gradients).
- **iOS simulator:** every gradient case ≤ 0.63/255, including the conic mask at 0.20; every shadow case matches by eye.
- **macOS:** matches every case by eye; numerically it is offset by the window capture's colour profile (5–9/255 uniformly).

**Owed after stage 2:**

- Pairwise interpolation of shadow lists.
- A spread in motion.
- Keyframes naming a shadow list.
- `mask-image` of more than one layer (refused at compile time).
- Linux and iOS failures that origin/main has too, unchanged here: the content-region registration and transform-drag tests (`requires one attached independent clipped owner`, `lower-transform-drag-handlers`) fail on the base commit as well.

### Stage 3 (2026-10-02): `background-clip`, `-webkit-text-stroke`

- **Rows.** Row 156 `background_clip` (an enum) and rows 154–155 `text_stroke_width` and `text_stroke_color` (inherited). The shorthand binds both, each row taking its part (`style/stroke.rs`).
- **Syntax.** Contract's lexer now reads a name starting `-webkit-` or `-apple-` as one identifier, as CSS does, so the Compat Standard's names can be written. Those two prefixes only, and only as an attribute's name (followed by `=`, not `==`), so `-webkit-x` in an expression is still a negation.
- **Apple.** The stroke is Core Text's own: a negative `strokeWidth` (percent of each run's size) and a `strokeColor`, centred and over the fill as Chrome draws it. A non-border-box clip draws through `draw(_:)`. `text` keeps the paragraph off its raster and clips the background colour and gradients to the union of its glyph outlines (`TextEngine.glyphPath`, from the laid-out lines).
- **Linux.** Glyphs are coverage, not outlines, so the stroke is a band island over the normal glyphs: the stroke-coloured paragraph dilated by half the width, less the same eroded by half, using a disc structuring element with a soft edge, so a fractional width draws as wide as it is. `text` paints the background into an island, multiplies it by the paragraph's glyph coverage, and places it under the glyphs.
- **Web.** CSS, with `-webkit-text-stroke-*` named as Chrome spells them.

**Measured against Chrome** on `scripts/fixtures/text-paint.contract`:

- **Linux, padding and content boxes:** ≤ 0.68/255 on both painters. Text is checked by where its colours land.
- **macOS and the iOS simulator:** match Chrome by eye, light and dark, down to the stroke's internal contours, which Core Text strokes as Chrome does.
- **Linux's stroke**, made from coverage, shows no internal contours.

**Owed after stage 3:**

- `background-clip: text` on a box whose text is in its descendants: only a paragraph's own glyphs clip today.
- Motion on the three rows.

### Stage 4 (2026-10-02): 3D transforms

- **Rows.** 157 `rotate_axis`, 158 `translate_z`, 159 `perspective`, 160 `perspective_origin` (the transform-origin codec), 161 `backface_visibility` (an enum).
  - The `rotate` attribute binds `rotate` (the angle the engine animates) and its axis. It now takes CSS's text forms too (`45deg`, `x 30deg`, `1 1 0 10deg`), where it took only a number before. `translate` binds x/y and z.
  - `perspective` makes a containing block, as CSS's does.
- **Apple.** A rotation out of the screen's plane, or a z translation, is the layer's `CATransform3D`, built as CSS orders it (translate, rotate, scale about `transform-origin`). The parent's `perspective` is its child container's `sublayerTransform`, about `perspective-origin`. `backface-visibility` is `isDoubleSided`.
  - **macOS fix.** AppKit rewrites a layer-backed view's layer geometry, transform included, during its layout and display. Every authored transform, 2D ones too, now goes back on in `layout()`, `updateLayer()` and `draw(_:)`. Before this, a static `rotate` or `translate` never showed on macOS. That was an existing bug this stage found.
- **Linux.** The placement route canvas children already took (`placement.rs`): the box is painted flat apart and warped through its plane's homography, its parent's perspective included, and hit-tested through the same map. `warp_soft` gives the warped outline antialiased edges, as a browser's 3D layer has; canvas children keep opaque edges. The Vello patch (§D8) is not needed for correctness and is not built: both painters use this CPU warp. It is owed only if a measured 3D animation is too slow.
- **Web.** CSS: `rotate` written with its axis, `translate` with its z.

**Measured against Chrome** on `scripts/fixtures/space.contract` (turned about y, x and an axis, z, a corner vanishing point, no perspective, hidden and shown backs, a child flattened into its parent's plane), light and dark:

- **Linux, both painters:** every case ≤ 1.44/255, with no pixels beyond the band after soft edges (they were up to 2.5% without them).
- **The iOS simulator and macOS:** match every case by eye.

**Owed after stage 4:**

- `transform-style: preserve-3d` (refused).
- `perspective()` and `matrix3d()` inside `transform`.
- Slerp between different axes in motion (the axis changes at once).
- A device frame check of an animated flip on Linux's CPU warp.

### §5 (2026-10-02): what Apple draws that CSS has no name for

- **Rows.** 162–170 are declared, non-CSS rows; the web writes no declaration for them. Plus the `symbolEffectValue` prop and the `haptic()` host command.
- **D10–D12 (symbols).** `symbol-rendering` (monochrome, hierarchical in the tint, palette, multicolor) and `symbol-palette` build the symbol's configuration on UIKit and AppKit. `symbol-value` is its variable value. `symbol-effect` runs pulse, variable-color, scale, and (on 18/15) breathe and rotate while set. It plays bounce, and wiggle on 18/15, each time `symbolEffectValue` changes. `replace` is the platform's replace transition.
- **D13 (system colours).** `-apple-system-label`, `-secondary-label`, `-tertiary-label`, `-quaternary-label`, `-separator` and `-fill` are colours on every host: `light-dark()` pairs of UIKit's own values, parsed wherever a colour is.
- **D14 (haptics).** `press-haptic` plays at touch-down, host-owned as `press-scale` is. `haptic("success" | "warning" | "error" | "selection" | "impact-…")` plays from app logic. iOS uses the feedback generators, macOS `NSHapticFeedbackManager`, the web `navigator.vibrate` where it exists, and Linux nothing.
- **D15 (numerals).** `content-transition: numeric` (or `numeric-countdown`) rolls a paragraph's raster when its text changes, unless Reduce Motion is on. The whole line rolls, not each digit.
- **D16 (scroll edge).** `scroll-edge-effect` sets a scroll container's four `UIScrollEdgeEffect`s on iOS 26.
- **D17 (pointer hover).** `hover-effect` adds a `UIPointerInteraction` with the highlight, lift or hover style.
- **D18 (Smart Invert).** `smart-invert` sets `accessibilityIgnoresInvertColors`, which `auto` sets on images, video and canvases.

**Seen:** on the iOS simulator and macOS, each symbol mode, the variable value, the effect symbol and the label colours draw as Apple draws them (`scripts/fixtures/affordances.contract`), and a press delivers `haptic`. The compiler test holds the rows and their named refusals.

**Owed after §5:**

- **D13 vibrancy itself.** A label is vibrant only inside a material's effect view, and the node tree does not nest a material's children there; only glass groups do. The colours land; the vibrancy effect does not yet.
- **D15 per-digit roll.** The raster is one picture, so the whole line rolls.
- **Felt and pointed on a device.** Haptics (D14), the scroll edge (D16) and the pointer (D17) are not observable in the simulator's screenshots.
- **Web stand-ins** for the symbol effects.

### Review (2026-10-02): Astra and Grok

Two independent audits of the six commits. Fixed in round 1:

- **macOS.** A layer-drawn box (`draw(_:)`) re-applies its transform too. A captured outer or inset shadow (one drawn into the picture) draws every shadow in the list. A scheme-coloured gradient nested in a list of layers redraws on a scheme change. An Arrange lift records its shift before the frame moves, so `layout()` re-applies the shifted transform.
- **iOS.** The ink and image layers sit above the inset caster, as CSS paints content over an inset shadow. The image fast path declines a corner shape it cannot draw.
- **Masks on Apple.** One composed mask per box: the shaped clip, `clip-path` and `mask-image` multiply, as CSS composes them. A box `filter`'s picture takes the composed mask, and so does a conic gradient on a material.
- **Kernel and Contract.** `rotate`'s axis survives the wire both ways. A −z axis is the angle negated. A dynamic hover effect of `none` removes the interaction. The vendor-prefix lexing is narrowed as stage 3 says.
- **Web JS target.** The stroke shorthand writes one `-webkit-text-stroke`, the `-apple-system-*` names resolve in dynamic colour writes, and a perspective of 0 writes `none`, as the wasm host does.
- **Linux.** A 3D box inside a 3D box composes both projections. A box's island reaches its outer shadow and its visible overflow. The stroke takes fractional widths.

Owed from the review (not fixed):

- **Per-run inline styles.** A `span`'s own `text-shadow` or stroke draws with its paragraph's, not its own.
- **Apple text shadow under transparent text.** Core Text casts no shadow from a clear fill. Chrome does draw it.
- **3D on Apple: hit-testing and back faces.** A turned box is hit-tested in its flat frame. A hidden back face still takes touches.
- **`paint-order: stroke`** is not a row. The stroke always draws over the fill, as Chrome's default does.
- **A material's children under a mask** (declared in LLP 1001).

## 7. Open questions for Charlie

- **Q1.** *Ruled 2026-10-02:* the take is waived.
- **Q2.** *Ruled 2026-10-02 (Charlie's proposal):* two names, `squircle` for CSS's shape and `-apple-continuous` for Apple's, each the same shape on every host (D1).
- **Q3.** 3D on Linux: a Vello patch (a projective image brush) plus a tiny-skia warp, with the orthographic projection as the fallback (D8). *Ruled 2026-10-02.*
- **Q4.** *Ruled 2026-10-02:* one list row replaces the four.
- **Q5.** *Ruled 2026-10-02:* linked in place of 1035.004.000, which is Implemented.
- **Q6.** *Ruled 2026-10-02:* all of them, with §5's names.
- **Q7.** *Ruled 2026-10-02:* Claude implements once the open questions are ruled, then a sanity review by Astra and Grok.
