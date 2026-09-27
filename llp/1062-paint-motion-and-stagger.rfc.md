# LLP 1062: Paint motion and stagger — colour and shadow transitions, computed times, `each` positions

**Type:** RFC
**Status:** Implemented 2026-09-26
**Systems:** Motion (`exact-motion`), Kernel (motion seam, wire), Contract (syntax, types, lowering), Plan (`LoadIndex`), Runner, Web, Apple (UIKit, AppKit), Linux
**Author:** Claude (Opus 5.5) for Seth Webster
**Date:** 2026-09-26
**Related:** LLP 1002 (motion v1: one representation, two executors), LLP 1003 (motion as built), LLP 1055 D5–D7 (keyframes and `animation`), LLP 1055.000 D6 (SVG colour motion, which this shares), LLP 1034 (`light-dark()`), LLP 1064 (`box-shadow`), `rules/DEFERRED.md` §Motion

## Summary

`transition=` animated only the compositor rows; a colour or a shadow snapped
on every native host while the browser eased it (under `all` it already did).
And `animation=` had to be a literal, so a stagger was a ladder of conditions.
This extends motion the way LLP 1002 and 1055 did: CSS's model, the browser as
executor on the web, `exact-motion` everywhere else, held to Chrome.

```
keyframes enter
  from opacity=0 background-color="#f97316"

button background-color=(on ? "#1d4ed8" : "#fde68a") color=(on ? "#ffffff" : "#111827")
    border-color=(on ? "#16a34a" : "#dc2626") box-shadow=(on ? "0 16px 24px #1d4ed8" : "none")
    transition="background-color 200ms cubic-bezier(.32,.72,0,1), color 120ms ease, border-color 1s, box-shadow 320ms linear"
  text "Toggle"
image "symbol:microphone" tint-color=(live ? "#dc2626" : "#16a34a") transition="tint-color 200ms"
column background-color="light-dark(#ffffff, #0b1020)" transition="background-color 1s linear"
each step, i in steps key=step.id
  row animation=`enter 320ms cubic-bezier(.16,1,.3,1) ${i * 70}ms both`
```

## Design

**D1 — Paint properties are engine properties.** `Property` gains
`background-color`, `color`, the four `border-*-color`s, `tint-color`,
`box-shadow` (offset and blur, points) and `box-shadow`'s colour half (named
only through `box-shadow`, never alone). `Value` grows from two components to
four; a colour is **premultiplied sRGB, channels 0–1**, so componentwise
interpolation is CSS Color 4 §12.3's for legacy colours — Chrome's. A fade from
`transparent` keeps its hue; red to half-transparent blue passes through
`rgba(170,0,85,0.75)`, not purple-grey. A shadow's opacity row folds into its
colour's alpha, so `none` → a shadow is CSS's transparent, zero-length padding.
`transition-property` takes each name and CSS's `border-color` shorthand
(wire value 25; the others are 1 + their `Property` code, 18–24 after
main's 1–17). A `currentcolor` border side's computed value is the
keyword, so while it stays `currentcolor` it starts no transition of its own
(even under a faster `border-color` one) and paints the element's animating
`color` frame by frame (Chrome 153: `rgb(128,128,128)` halfway from black to
white); a host settles its engine value at once and paints the view's
presented `color` on it (`Kernel::current_color_sides`). A side that changes
to or from an explicit colour moves under its row (red to `currentcolor` with
`color: blue` is `rgb(128,0,128)` halfway in Chrome and here). *Rejected:*
interpolating straight RGB (disagrees with every browser whenever alpha
changes); eight-component values carrying both appearances (see D4).

**D2 — Only a node that names paint owns it.** `Kernel::paint_sync(receipt,
dark, owners)` adopts, for each created or touched node, the paint properties its
`transition` starts a curve for or its `animation` or `exit-animation`
animates, with their *computed* targets: `light-dark()` resolved (`dark` is
one appearance or one per node, `Appearance`), `currentcolor` borders as the
computed `color`, `color` inherited. An exit's colours are owned while the
node lives, so the engine has the value its exit keyframes play over (LLP
1063). Everything else stays the style row the
presenter already paints, so a list of 1,000 coloured rows costs the engine
nothing. `PaintOwners` (the host's) records what was adopted, so a row that
stops naming paint retires exactly that. The web never calls it.

**D3 — A spring on paint is its curve from rest.** A spring drives the
compositor rows as physics (velocity carries across an interruption; the web
lowers frames, LLP 1002 D2). Paint, an SVG shape's `fill` and `stroke` included, plays it as its curve
from a unit displacement at rest, a CSS `linear()` easing over its settle time
(`SpringConfig::easing`, sixty stops a second at the f32 the text carries,
`Transition::governing`): the web writes it into the `transition` row (`all`
names each such property), the engine plays the same stops, and it interrupts
as CSS does, from where it is. Chrome 153 matches the engine on a colour
spring, overshoot included (`color-spring`, 5 samples). `layout-transition`'s
web spring shares the lowering. *Rejected:* lowering paint springs to frames
on the web — a colour has no velocity a gesture releases, `light-dark()`
would need the page's appearance in Rust, and `box-shadow` is one CSS
property over two engine ones.

**D4 — An appearance change transitions, as in Chrome.** Chrome 153 resolves
`light-dark()` at computed-value time, so flipping `color-scheme` starts a
transition (recorded: `getAnimations()` holds one, 50% is `rgb(128,128,128)`).
Native hosts resolve before the engine hears a target, so the host must know the
appearance: Apple's presenter reports it (`exact_scheme`, from the ExactView's
trait change and after every boot); Linux's is the app's `setScheme`. A change
re-targets every owner (`paint_resync`) and the engine transitions under each
node's row. The first Apple report only corrects boot's light guess, without
motion. A view whose own appearance differs from the session's (a sheet with
an override) is found as a restyle carrying presented paint reaches it and reported after the batch
(`exact_view_scheme`); its node then resolves by it, keyframes included
(`Engine::set_node_dark`). Its first report corrects in place, as the
session's first does; a later change, or agreeing with the session again,
transitions.

**D5 — Presented over the row, then handed back.** Apple (as merged with
main, 2026-09-27, the mechanism main's SVG colours use, LLP 1055.000 D6): a
frame of paint motion re-sends the view's `style` with the presented values
over its rows (`style_json_presented`: `text_color`, `background_color`,
`border_color_*`, `tint_color`, `shadow_color` with opacity 1,
`shadow_offset`/`shadow_radius`), in the dictionary's own units, so the box
layer, border, shadow caster, paragraph and symbol tint paint it unchanged.
When the value reaches its target the plain style is sent and the row shows
again, so nothing goes stale when the row later changes without a
transition. An animating `color` restyles each view that inherits it (no own
row), as a browser's inheriting element shows it; an inline run is no view,
so its paragraph is re-sent with the run's colour. A leaving view (LLP 1063)
has no node: its last style is re-sent with its exit's colours over it. This
branch first had `present`/`unpresent` ops with a paint dictionary on the
view; the merge dropped them for main's single path. Linux:
`Presented.colors`, painted over the captured box and each run's palette
entry, as SVG colours are.

**D6 — Text is re-rastered, not tinted.** A paragraph's pixels carry its colour
(`TextRasterIOS`), so a presented `color` invalidates the raster and the
paragraph is painted as the batch that carries the colour ends, where it shows
(`paintPresentedText`) — the same frame as its value, not a worker's next;
one out of sight waits for its worker. A layer tint (glyphs as a mask under a coloured
layer) would cost nothing per frame but is wrong for colour emoji and for runs
of other colours; measured below, re-rastering is cheap enough to be the one
path. `tint-color` on the web is the registered custom property `--exact-tint`
(`@property … syntax:"<color>"`), which the browser interpolates; symbols read
it directly.

**D7 — An `animation` template computes its times.** `animation=` and
`exit-animation=` may be a template: `` `enter 320ms ${i * 70}ms both` ``.
As merged with main (2026-09-27), names resolve at run time against the
plan's `keyframes` table (LLP 1055 D5), so a template may compute any part,
a name included, and the compiler checks the names of a literal. (This
branch first allowed `${…}` only before `ms` or `s` and resolved names at
compile time; main's table made the restriction unnecessary.) LLP 1055 D5
holds unchanged: an animation still named keeps its start, so a new delay
applies as if it had always had it (engine, and the web's same `@keyframes`
name).
*Rejected:* an `animation-delay` row — a schema row, a kernel merge and web
ordering for one number, and no computed duration.

**D8 — `each item, i in list` names the position.** A number from 0 (plan opcode
`LoadIndex`). A kept row that moves reads its new position; its identity is
still its key, so a stagger re-times without restarting. A windowed `list`
(`item-height`, `virtualized`) names it too: a mounted row carries its
position, and one the records move under it reads the new one.

**D9 — Keyframes take colours, `light-dark()` included, and `box-shadow`.**
`background-color`, `color`, `border-*-color`, `tint-color` and `box-shadow`
(geometry and colour together, one declaration in the rule, its opacity in
the colour's alpha) in a keyframe, written or returned by a function of
literal arguments (`color=accent()` where `fn accent(): string =
"light-dark(#4F6657, #B7C9AC)"`, or `color=tone("strong", 0.4)`), which
lowering folds to its literal through conditions, templates, `let` and other
functions; anything not known when the app compiles is refused. A
`light-dark()` pair keeps both values (`Keyframe.dark`; on the wire a
flag and four more floats); the row's text writes it as
`light-dark(rgba(…),rgba(…))` under the browser's name (`--exact-tint`).
Which one plays is the appearance the animation **starts** under: Chrome 153
resolves a rule's `light-dark()` once, and a playing animation keeps its
colours across a `color-scheme` flip (recorded: at 50% of `lit`, light
`rgb(128,0,0)` before and after the flip; one started dark, `rgb(128,128,255)`).
The engine does the same (`Engine::set_dark(dark, playing)`); only an Apple
host's first appearance report, which corrects boot's light guess, re-resolves
playing animations in place, keeping their start. On Apple an animation
with a `light-dark()` keyframe is sampled by the engine, never lowered to
Core Animation, which could not say which appearance it started under. Re-verified in Chrome 153
(2026-09-27): at 50% light `rgb(128,0,0)`, after the flip `rgb(128,0,0)`, at
75% `rgb(191,0,0)` — the light curve still; one started after it
`rgb(128,128,255)`.

```
fn accent(): string = "light-dark(#4F6657, #B7C9AC)"
fn textTitle(): string = "light-dark(#171B17, #F5F5EC)"
keyframes lit
  from color=accent()
  to color=textTitle()
each w, i in words key=w
  text w animation=`lit 900ms linear ${i * 120}ms both`
```

## Verification

- **Chrome parity.** Six paint cases join `host/web/src/parity.rs`
  (premultiplied, from transparent, `color` under `cubic-bezier(.32,.72,0,1)`,
  the `border-color` shorthand, reversal, a colour keyframe with `alternate`).
  Chrome 153 recorded 29 colour samples; the engine matches every one within a
  channel unit and an alpha step (`COLOR_TOLERANCE`). A seventh, `color-spring`
  (D3), added 5; all 177 samples in all.
- **Hosts, driven.** The same Contract on web (headless Chrome), iOS
  (simulator, UIKit) and macOS through `scripts/agent.mjs` with the clock held:
  at 250 ms and 500 ms into a 1 s linear transition, background and border pixels
  agree with the web's within 1/255; the shadow's blur differs by the platforms'
  rasterizers (≤ 8/255). An appearance flip on iOS transitions the page from
  `(11,16,32)` to white through `(72,76,88)` at 250 ms — the web's value
  exactly. Tests: `motion/tests/it/paint.rs`, `kernel/tests/it/paint.rs`,
  `host/apple/tests/it/paint.rs` (batch ops, scheme, `currentcolor` sides,
  inline runs, exits, a view's own appearance),
  `host/linux/tests/pinned/motion_paint.rs` (pixels, `currentcolor` border),
  `contract/cli/tests/it/keyframes.rs` (templates, positions in plain and
  windowed lists, refusals, palette functions of arguments, `box-shadow`),
  `host/web/src/css.rs` (a paint spring as `linear()`), and the XCTests
  `PaintMotionIOSTests` (text painted in the batch, inline runs, a view's
  appearance) and `PaintMotionMacTests`. grnl's welcome shape (three
  words, `lit` from `accent()` to `textTitle()`, 300 ms apart) driven on web
  and iOS under dark: each word at its own point between the pairs' dark values.
- **Cost (iPhone 17 Pro simulator on an Apple-silicon Mac, 61 real frames of a
  1 s `color` transition).** Worker raster per frame: a 314×24 pt label
  0.13 ms median (p95 0.21); a 354×308 pt, 14-line paragraph 0.57 ms median
  (p95 1.09, max 1.29). Main-thread `ensure` 5 µs median (max 65 µs). A box
  colour is layer properties only. Not measured on a device.

## Known gaps

- None open. Not a gap: a flip mid-animation leaves a playing keyframe's
  colours as they started; Chrome 153 does the same (D9, re-verified).
