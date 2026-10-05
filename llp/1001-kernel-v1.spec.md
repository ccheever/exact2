# LLP 1001: Kernel v1 — what `exact-kernel` is, as built

**Type:** Spec
**Status:** Draft
**Systems:** Kernel, Wire, Layout, Text, Export
**Author:** Claude (Fable 5) for Charlie Cheever
**Date:** 2026-08-28
**Implementer:** Claude (Fable 5), landing 2026-08-28 (this document transcribes the landing)
**Related:** LLP 1000, RFC 0491, LLP 0507, LLP 0486, LLP 0487, LLP 0488, LLP 0297, LLP 0382

## Summary

`kernel/` is RFC 0491's end state built fresh, scoped by `rules/DEFERRED.md`: a
copied-in, borrow-parsed binary command stream with transactional apply, Taffy
layout, and columnar binary exports. Nine workstreams in exact1; here they are the
shape of one small crate. This document records the decisions the code embodies so
a reader can tell a choice from an accident. Where this document and the code
disagree, the code and its tests are the authority and this document is stale.

Under `rules/RULES.md` this spec is legitimate because the implementer and date
are on it and the code exists. It is not a promise about anything not yet built.

## 1. One declaration authority (WS-B)

`kernel/tables/schema.json` is the only place node types, props, style rows, enum
vocabularies, symbol roles and their host mappings, and opcodes are declared. `kernel/build.rs` generates from it — into
`OUT_DIR`, never committed — the Rust enums, `StyleProps` and its mask, the
patch/clear operations, the wire codec for style rows, and `SCHEMA_DIGEST`
(domain-separated SHA-256 of the canonical JSON and the production wire-codec
sources, first 8 bytes, little-endian). Hashing both makes any handwritten or
generated codec edit a protocol rotation even when its schema row is unchanged;
the codec byte/digest snapshot fails if that coupling is removed.
Every EXWF frame carries the digest; a producer generated from a different table is
refused at decode (`DecodeError::SchemaDigestMismatch`).

The generator fails closed: a row it cannot validate is a build error. A second
copy of any table anywhere is a defect — the TypeScript encoder, when it exists,
reads the same file.

**Defaults live in the table, once, and they are CSS's.** The style table's
`default` column is both the storage initializer and the semantic default; there is
no separate profile authority yet (0486's layout profile becomes relevant when a
second engine exists). Per `rules/RULES.md` §Scope the web is the standard: a bare
node is `display: block`, `box-sizing: content-box`, `flex-direction: row`,
`flex-shrink: 1`, `align-items: normal` — what a bare `<div>` does — and rows
carry CSS names and vocabularies (`object_fit: fill|contain|cover|none|scale-down`,
`text_overflow: clip|ellipsis`, `line_clamp`). Block layout brings CSS margin
collapsing with it; flex and grid are opt-in per node. (An earlier draft chose
React Native's defaults; Charlie reversed that on 2026-08-28.) A root is a
block formatting context, as CSS's root element is: a first child's top
margin stays inside it. On the web a root is an element inside
`#exact-root`, where that margin would collapse through to the page, so the
web host lowers a block root to `display: flow-root` (2026-09-23; a root at
y 0 with its child at 30, where the page had both at 30).
A `button` is Chrome's `<button>` under the web reset (`all: unset;
display: block`; Charlie, 2026-10-04, reversing 2026-09-23's "One native
button, flex column", which three app diaries hit: kanban F16, onboarding F3,
recipes F5, about 25 buttons written around it). Its automatic width shrinks
to fit, as HTML sizes a button (`item_is_table`, Taffy patch 21). A block
button's content lies in HTML's anonymous button content box, a flow-root
centred safely in the block axis, which no `align-content` moves (Chrome 154:
a 20px child at 40 in 100px, two at 30 and 50, a 150px one at 0); the kernel
lays its children out with block `align-content: safe center` (`style.rs`
`taffy_style`; patch 26 moves absolute children's static positions with
them). The UA sheet's `text-align: center` is a fixed row. An authored `display:
flex` or `grid` makes an ordinary container, a row unless `flex-direction`
says otherwise, which Chrome does not centre. Literal Chrome cases:
`browser_cases::a_button_lays_out_its_content_as_chrome_does`.
Declared: its outer display is the reset's `block`, not the UA's
`inline-block`. The kernel has no inline formatting context, so in a block
parent two buttons stack where Chrome sets them on one line box; in a flex or
grid parent the two displays are the same (an item is blockified).

**Native tab-bar projection (LLP 1059; Charlie, 2026-09-27):** an iOS
symbol-and-label tablist can report its native control size through the host
intrinsic seam. Its height supplies `min-height: auto` in native layout, so
siblings reserve the control's space; explicit CSS `min-height` still wins.
This is a declared native presentation deviation: the browser keeps authored
HTML/ARIA/CSS. No aspect ratio is inferred, and ending projection clears the size.

`justify-content`, `align-content`, `align-items` and `justify-items` accept
and default to `normal` (2026-09-23). It lowers to Taffy's unset alignment,
which each algorithm resolves per CSS Box Alignment: `flex-start`/`stretch`
in flex; stretched auto tracks, and items stretched unless sized, in grid;
`start` with no new formatting context in block. The old `flex-start`
default left grid auto columns at content width (`auto auto` in 400px: 0
wide, Chrome 200 each). `kernel/tests/it/browser_cases.rs` holds the
literal-Chrome cases. `align-self` keeps `auto` and has no `normal`.

**Corner percentages (2026-10-02):** the four `border_radius_*` rows store
lengths or percentages. A single `border-radius="50%"` sets each corner;
percentages resolve independently against the border box's width and height,
then CSS's common overlap reduction applies. Web keeps the authored percentage;
Apple and Linux paint elliptical corners, including after a resize. Negative
literal lengths and percentages and `auto` are refused. Each corner still takes
one length or percentage; paired horizontal/vertical radii and the slash-separated
`border-radius` shorthand are not implemented. Percentage ellipses do not require
that separate value-pair syntax.

**Border semantics (Codex, 2026-09-11):** four `border_style_*` rows
(bits 91–94) accept `none | hidden | solid | inset`, initially `none`. Contract's
single-value `border-style` sets all four; `border-<side>-style` sets one.
Other line styles are refused until a consumer needs their painting. `inset`
came with HTML's `hr` (LLP 1021 D1, 2026-10-04): the browser draws it, and
`border_colors()` gives the native hosts Chrome's two shades, measured from its
pixels and Blink's `CalculateInsetOutsetColor` — the top and left
`Color::Dark()`, the bottom and right `Color::Light()`, a colour whose relative
luminance is no more than `#202020`'s lightened instead (once, and twice for the
lit side), one brighter than `#ebebeb` its own lit side, and a `currentcolor`
side — unwritten or written — shaded from `#eeeeee`, not from `color` (WebKit's
`colorIncludingFallback`, which Blink keeps). Measured 2026-10-04 in Chrome 154
(`--headless --screenshot`, 4px `inset`/`outset` boxes): `color` red, gray,
`#000040`, black and `#eeeeee`, with no `border-color` or with `border-color:
currentcolor`, all paint `#9a9a9a` over `#eeeeee` (`outset` the reverse), while
`getComputedStyle` reports the border colour as the resolved `color`; an
explicit `border-color: red` under `color: blue` paints `#ab0000` over red. Chrome's
UA sheet gives `hr` `color: gray` and no `border-color`, so a bare `hr` is
`#9a9a9a` over `#eeeeee` whatever its `color`. Firefox shades differently;
Chrome is the oracle. Widths
retain their authored values, initially 3 (`medium`), while `border_widths()`
returns zero for `none`/`hidden`. Layout, Apple content insets and Linux paint
consume those effective widths. Border colours initially use `currentcolor`:
the row's `current-color` codec is an optional colour, sharing the caret codec's
wire representation but preserving its own keyword. The native hosts resolve
it against the node's computed text colour, retaining light/dark pairs; CSS
receives the authored values. Explicit transparent colour still occupies space.
These are [CSS Backgrounds §3.2–3.3](https://www.w3.org/TR/css-backgrounds-3/#border-style)'s rules.

The prior native host painted every declared width: a 100×60 content box
with padding 10 and border width 8 was 136×96 versus CSS's default 120×80,
with child inset 18 versus 10 (`/tmp/messages-border-geometry/`). The corrected
native fixture switches between those sizes with `solid` and `none`/`hidden`,
and follows inherited colour through appearance and explicit colour changes.
Existing app borders now declare solid, including Messages and Fieldnotes.
The Messages iOS comparison is pixel-identical and the browser badge gains its
declared one-point border (`/tmp/messages-border-semantics/`). Fractional device
pixel snapping, patterned borders and compound shorthand values remain unverified
or unsupported; these checks do not establish all CSS border painting.

**Motion rows (2026-08-28, LLP 1002/1003).** The animatable rows carry CSS's
individual transform property names — `translate` (vec2), `scale`, `rotate`
(degrees) — beside `opacity`, and a `transition` row (codec `transitions`, bit 82)
carries CSS `transition` declarations. The row's type is `exact_motion::Transitions`;
the kernel owns its bytes (`wire/codec.rs`) and depends on `exact-motion` for the
type, which is the only dependency edge between the two crates. `Kernel::motion_sync`
restates a commit for the engine (LLP 1003 §6).

`caret-color` (bit 61; Messages, 2026-09-10) follows CSS `auto | <color>`:
initially `auto`, inherited, and independent of layout and text measurement.
It replaces the unused `tint_color` row. The `auto-color` codec keeps auto
separate from explicit transparent paint; clearing an override resumes
inheritance. The browser receives CSS; Apple receives auto or the existing fixed/
light-dark colour representation. UIKit's editor tint colours both insertion
carets and native selection handles/highlights; this platform coupling also
applies to a read-only selection. AppKit colours its insertion point; Linux
colours the focused input's painted caret. Auto retains UIKit's default tint and
uses the text colour on AppKit/Linux. Messages explicitly requests white on its
outgoing read-only selection and auto on incoming selections. See
[CSS UI's caret rules](https://www.w3.org/TR/css-ui-4/#caret-color): user agents
may also apply them to selection mechanisms outside editable text.

`tint-color` (bit 89; LLP 1035.004, 2026-09-10) colours a `symbol:` image
on Apple and the web. Initially opaque black, not inherited, it accepts fixed
and `light-dark()` colours. Computed inherited `font-size` and `font-weight`
configure the symbol independently of its CSS box. The schema's `symbols` rows
hold each portable role, its Apple name and its generic SVG path; the generator
emits the compiler vocabulary and both host mappings. An explicitly set row also tints raster images on web, Apple and Linux
(LLP 1011 §3–4; 2026-09-27): the image alpha masks the tint. On Apple and
Linux the box's background and border still paint; on the web the mask still
covers the whole element (declared below). Linux symbol rendering remains unsupported. This
is separate from `caret-color`. `symbol:sf/<name>` passes the opaque name
straight to Apple's running OS (LLP 1035.004.000; Codex, 2026-10-02).
Empty/unavailable names and symbols on hosts without their renderer paint
nothing, with a one-computed-font-size square as fallback intrinsic size.
This font-sized image source is a declared extension to the web's schemes;
CSS dimensions, constraints and aspect ratio still determine its box.

`overflow-wrap` (bit 90; implementer Codex, 2026-09-11) is inherited text
layout intent with CSS's `normal | break-word | anywhere` vocabulary and
`normal` default. Both emergency modes break otherwise unbreakable text;
only `anywhere` includes those breaks in min-content sizing. The paragraph
request carries the policy to the host measurer; its cache and painted
paragraph use the same policy. The web receives the CSS declaration directly.
The compiler's `textarea` tag supplies `break-word`, matching its browser
user-agent rule; Messages also declares it on bubble text. This repairs a
45-character unbroken message that occupied one overflowing web line while
UIKit wrapped three. It does not tighten a bubble around its wrapped lines.
See [CSS Text's overflow wrapping](https://www.w3.org/TR/css-text-3/#overflow-wrap-property).

`scrollLeft` (prop 59, float) is the horizontal counterpart of `scrollTop`: a
changed binding sets that axis after layout, clamped by the host’s content
extent; unrelated updates preserve the reader’s offset.

`autocapitalize` and `autocorrect` (props 57/58, text) carry HTML editing
hints. They remain strings so HTML's case-insensitive aliases and missing,
empty and invalid-value behavior stay the host's. The browser receives real
attributes; native editors apply them without transforming stored text.

Explicit application policies (Messages, 2026-09-09) have no CSS
equivalent: `scrollFollowEnd` (boolean, absent/false by default) keeps a
scroll container at its trailing vertical edge across content/viewport changes
only while the reader is already there. Above the end, web leaves CSS scroll
anchoring to the browser; iOS records a visible descendant and adjusts the offset
by its movement after layout. It keeps that choice between batches until the
reader scrolls; if the node disappears, a surviving visible candidate can hold
the position. With none left, iOS clamps the old offset to the new extent.
An inactive iOS route retains an unpinned offset across a temporary viewport
clamp, restoring it when space permits; a new drag or explicit scroll write wins.
macOS still preserves the numeric offset in all cases. An explicit `scrollTop` assignment wins. Web uses
ResizeObserver and commit boundaries; Apple snapshots before each batch and
restores after layout. The opt-in policy is not a complete native implementation
of CSS `overflow-anchor` selection and suppression rules.
`inert` (prop 20, boolean, absent/false by default) is now authorable through
Contract. It preserves layout while requesting subtree input, focus and
accessibility exclusion. The browser uses HTML inertness; iOS enforces the
subtree boundary and its modal-confirmation exception (LLP 1035.001 D3).
AppKit/Linux subtree enforcement remains open; the declaration alone is not
cross-host support.
`retainFocus` (prop 55, boolean, absent/false by default) lets a button
retain the existing editing session on a tap, including a button with gesture
handlers and no press action. Messages uses it on bubbles and reaction badges
that open Tapbacks: the panel does not exist yet at pointer down. Web prevents
the pointer’s default focus change; iOS skips resigning the current responder.
It neither focuses a field nor opens a keyboard. Messages also declares it on
both Send controls: sending from a retained composer keeps its editing session.
Other hosts currently ignore it.
`swipeIndicator` (prop 56, boolean, absent/false by default) marks a direct
child of a `swiperight` target as authored gesture feedback. Web and iOS hold
its opacity and scale between their authored values and 1 as the rightward
offset grows from 0 to the 64-point action threshold, clamped thereafter.
Reversal follows the offset; release/cancellation restores the authored values
through their transitions. Its subtree does not take pointer hits, including at
zero opacity. It has no CSS equivalent and creates no host artwork
or runner gesture state. Other hosts ignore it.
`keyboardDismissMode` (`none`, `on-drag`, `interactive`; absent means `none`)
opts an iOS scroll container into UIKit's keyboard dismissal behavior. Other
hosts retain their platform behavior. Linux does not yet implement either policy.

`scrollbar-width` (bit 88; Messages, 2026-09-09) follows
[CSS Scrollbars](https://www.w3.org/TR/css-scrollbars-1/#scrollbar-width):
`auto` initially, `thin`, or `none`, not inherited. It changes scrollbar
presentation, not overflow, scroll position, snap, or layout. Web emits the CSS
property. iOS uses its already thin native indicators for `auto`/`thin` and
hides both for `none`; macOS uses regular/small scrollers and hides both for
`none`. Clearing the row restores platform defaults. Linux has no scrollbar
painting yet and ignores this row. Messages hides the horizontal timestamp track.

`clip-path` (bit 87) clips painting and platform hit-testing without changing
layout. Its initial value is `none`. The implemented CSS subset is
`path([nonzero | evenodd,] "...")`: SVG path data in full, in CSS pixels,
parsed by the SVG path parser (`svg/path.rs`, LLP 1055.000; `parse_d_whole`)
and refused whole on any error or when it draws nothing; `url(#id)` names an
SVG `clipPath` on an SVG element. Other shapes are rejected. The parsed
commands are absolute `M`, `L`, `C` and `Z` (quadratics and arcs arrive as
cubics). `clip.rs` validates and canonicalizes the value;
the wire carries that CSS string and validates it on decode. Apple receives the
parsed commands and fill rule (`{"rule", "commands"}`), applies a layer mask to the entire subtree, and
tests the same path for pointer hits. Web emits CSS. Linux masks by it.
Messages uses it for transparent curved tails over the focused reply material.

`navigationKey` and `navigationBack` declare a host navigation container on the
first root. Its direct children carry unique `navigationKey` values; the root's
key selects one, with preceding children retained as its back stack.
`navigationBack` names the HTML `id` of a press control in the active route.
UIKit presents those existing Contract views through `UINavigationController`
and presses that control after a completed native pop; cancellation changes no
Contract state. The browser hides and makes inactive routes inert. This is an
explicit platform navigation policy, not a CSS style or an engine-owned gesture
or interactive transition model. Other hosts currently retain their ordinary
stacked rendering; callers supply opaque, absolutely positioned route surfaces.

`swipeContent` (65), `swipeLeading` (66) and `swipeTrailing` (67) are
explicit native row presentation requests (2026-09-10, Messages). The first
names a descendant's HTML `id`; the others are whitespace-separated control
ids, ordered from the outer action inward. These props do not change CSS
layout, inheritance or scroll semantics. UIKit supplies
the row's swipe surface (LLP 1008 §9); other hosts keep the authored content and
controls. A full swipe performs the first configured action when it is enabled.
Missing/ambiguous references or invalid row geometry leave the authored fallback
in place and produce a host diagnostic. All references must be within the owning
scroll node, and its content must have that node's width and height.

`destructive` (68, boolean, absent/false) declares a control's destructive
presentation role. The former `swipeDestructive` row is renamed in place, with
no alias: UIKit uses it for swipe actions, menu items and confirmation actions
(LLP 1021 D2/D3). Messages' measured black Block/Discard actions earn back the
shared hint under LLP 1021 §5. This is an explicit deviation: CSS, HTML and ARIA
have no equivalent action role. It changes no dispatch, inheritance or layout;
the web emits `data-destructive` and retains authored CSS. AppKit and Linux do
not yet map the role to native presentation. A red colour or action label never
implies the flag.

`commandfor` (69) and `command` (70) are HTML string attributes. A `dialog`
semantic tag defaults to absolute positioning; `show-modal` and `close` name
browser-owned presentation commands, not runner state. UIKit supports the narrow
confirmation grammar and `closedby="any"`; AppKit/Linux do not present dialogs
(LLP 1021 D2). No arbitrary dialog or additional command capability is implied.

`emojiPicker` (prop 63, boolean, absent/false by default) is an explicit
selection-input policy on `input`, not an HTML `inputmode` value. A single emoji
grapheme emits `change`; ordinary text and multiple graphemes are refused and
the field is cleared. iOS prefers an enabled emoji keyboard, including its
native search; web and macOS filter entered characters without opening a system
picker. Linux's driver reports unsupported before changing focus or state.
The app keeps `value=""`; its draft belongs to a separate editor. The selection
gate accepts emoji-presentation scalars, or emoji scalars accompanied by VS16
or the keycap mark, within one grapheme. This is a bounded input filter, not a
complete Unicode emoji-sequence validator. It is not inherited.

`contextTarget` on a context-preview node names its source's HTML `id`.
The preview's nearest enclosing absolutely positioned panel aligns that preview
with the source's visible position, clamped inside the viewport and safe areas.
Web and Apple also intersect that vertical range with the panel's containing
region, allowing authored content to reserve space above or below the menu.
The region's bounds do not change the preview's source or text measurement.
The app supplies the preview's matching content and the surrounding controls;
the host supplies placement over scrolled content. This is a context-preview
presentation policy, not CSS anchor positioning. Web and iOS also magnify the
preview by 15%, capped at 26 added points on its larger dimension, preserving its
text layout, source-facing outside edge and vertical center before viewport clamping. Later
content counteracts the panel's half-height shift so a receipt keeps its
source-relative position. Top-aligned immediate side siblings translate to the
enlarged preview's left or right edge, preserving their authored horizontal gap
without changing the preview's percentage-width basis. Clamping includes the farther extent of the enlarged
preview or following controls. If the group is taller than the available region,
following controls shift upward to keep their bottom inside it, overlapping the
preview where necessary; the receipt retains its source-relative position.
The kernel's layout and authored transform rows do not change. This follows
measured iPhone 17 / iOS 26.5 preview geometry, not a recovered UIKit rounding/animation policy.
`contextMagnify=false` on that preview disables only this host magnification;
alignment, containment and the authored CSS transform still apply. Absent or
true keeps the 15%/26-point rule. The prop is a non-inherited boolean and has
no effect without `contextTarget`. Messages uses false for badge/double-tap
entry and true for long-press entry, including their respective emoji pickers.
macOS implements alignment without magnification; Linux currently retains the
panel's authored position. Messages supplies its outside-dismiss backdrop as an
ordinary press control.

**Additional deviations as built (2026-09-27; recorded by Astra).** The
following declarations collect the host carve-outs and non-CSS rows introduced
with PR #47, with Charlie's rulings of 2026-09-27 where he made them; the
`box-shadow` subset is still unruled.

- **`press-scale`**, bit 147 ([LLP 1061 D1–D3](1061-press-feel-and-reduced-motion.rfc.md)),
  is not CSS. It supplies platform press feedback without a runner round trip
  or app-owned pressed state. Ruled 2026-09-27: it composes through CSS's
  `scale` property and never writes `transform`, and it is kept under reduced
  motion, as a native button's highlight is.
- **`layout-transition`**, bit 146 ([LLP 1063 D1, D6](1063-presence-and-layout-motion.rfc.md)),
  is not CSS. It moves the presentation after committed layout, sizing only
  the box's surface while content keeps its final geometry. The separate row
  prevents `transition: all` from silently starting to animate layout.
  Ruled 2026-09-27 (with `exit-animation`, option a): the web runs it by FLIP,
  as web layout-animation libraries do; no browser feature is its oracle, so
  parity is one recorded timeline compared across hosts.
- **`exit-animation`**, bit 145 ([LLP 1063 D1–D5, D8](1063-presence-and-layout-motion.rfc.md)),
  is not CSS: it holds a destroyed view as an inert ghost until its finite
  animation ends. CSS's `animation` does not itself defer destruction. Linux
  refuses it and removes the node immediately, with a journal entry, because
  its painter walks the live kernel tree and has no retained destroyed subtree.
- **`box-shadow`** ([LLP 1064 D1](1064-box-shadow-and-text-transform.rfc.md),
  [LLP 1077 D4](1077-css-visual-properties-native-draws-cheaply.rfc.md)) is
  one row holding CSS's list (at most eight), outer and inset, with spread. A
  missing colour is still refused instead of using CSS `currentcolor`. A
  transition or keyframes move the list's first shadow's offset, blur and
  colour; the other shadows, and a spread, change at once, where CSS
  interpolates the lists pairwise.
- **`background-image` layers** ([LLP 1077 D5](1077-css-visual-properties-native-draws-cheaply.rfc.md))
  are at most four. Apple draws a conic gradient, and more than one layer,
  through the box's `draw(_:)` (Core Animation's conic gradient bends CSS's
  angles in a box that is not square), and a conic `mask-image` as pixels.
- **Gradient paint under borders** ([LLP 1066 D5](1066-gradients.rfc.md)):
  native hosts extend end colours outside the padding box instead of repeating
  the gradient image as CSS's initial `background-repeat` does. Their gradient
  shaders/layers extend one gradient rather than tiling a padding-box image;
  translucent borders expose the difference.
- **`corner-shape: -apple-continuous`**, bit 156 ([LLP 1077 D1](1077-css-visual-properties-native-draws-cheaply.rfc.md)),
  is not a CSS keyword. It names Apple's continuous corner curve, one shape on
  every host: UIKit and AppKit draw it with `cornerCurve = .continuous` where
  the box has one radius, and the kernel's outline (`corner::outline`, fitted
  to UIKit's curve; Linux against the iOS simulator: mean 0.57/255) everywhere
  else. The web draws `superellipse(1.6)` over the radius scaled by 1.52, the
  closest CSS shape (2.5% of the radius at worst; a bordered box measured mean
  6.5/255 against UIKit), and a bound (dynamic) `corner-shape` is not rescaled.
  CSS's own keywords are CSS's on every host. The inner border edge of any
  shaped corner is the same shape over the padding box's radii, CSS's rule for
  round corners.
- **`mask-image` on a material** ([LLP 1077 D2](1077-css-visual-properties-native-draws-cheaply.rfc.md)):
  UIKit and AppKit mask the effect view itself (`mask` / `maskImage`), as they
  require of a visual effect view, so the blur fades and the node's children
  do not; CSS masks the element and its children together.
- **`text-shadow`** ([LLP 1077 D3](1077-css-visual-properties-native-draws-cheaply.rfc.md))
  takes one shadow, not a list, and no spread (CSS has none). On Apple a
  paragraph drawn without a raster clips its shadow to the view's bounds.
  Adjacent inline runs with the same shadow and stroke paint as one inline box
  (all their shadows, then their glyphs). CSS paints each box's shadow over
  the boxes before it, so a shadow cast back across another run with the same
  shadow is covered by that run's glyphs here. Runs that differ paint in
  CSS's order. On Apple a mixed-run shadow reaches at most 512 points past
  its paragraph's box.
- **`-webkit-text-stroke`** ([LLP 1077 D7](1077-css-visual-properties-native-draws-cheaply.rfc.md))
  on Linux is a band of the glyphs' coverage (dilated less eroded), not a
  stroke of their outlines, so a glyph's overlapping contours show no inner
  lines as Chrome's and Core Text's do. `background-clip: text` clips to the
  node's own paragraph, not to text in its descendants.
- **3D transforms** ([LLP 1077 D8](1077-css-visual-properties-native-draws-cheaply.rfc.md))
  flatten every box into its parent's plane (`transform-style: preserve-3d`
  is refused), and a transition between two different `rotate` axes changes
  the axis at once where CSS slerps. Linux draws a 3D box as a picture warped
  on the CPU.
- **Apple's affordances** ([LLP 1077 §5](1077-css-visual-properties-native-draws-cheaply.rfc.md)),
  rows 162–170, are not CSS: `symbol-rendering`, `symbol-palette`,
  `symbol-value`, `symbol-effect` (with the `symbolEffectValue` prop),
  `press-haptic` (host-owned as `press-scale`), `content-transition`,
  `scroll-edge-effect`, `hover-effect` and `smart-invert`. Each draws on the
  platform that has it; the web writes no declaration for them and draws a
  symbol monochrome. `press-haptic` alone has a web arm: the pressed
  element's `--exact-press-haptic`, which the input glue plays at the press
  as `navigator.vibrate` where the browser has it, as `haptic()` does. The `-apple-system-*` label, fill and separator colours
  are WebKit's names. The kernel keeps them as themselves (`ColorValue::System`),
  and each paints as a `light-dark()` pair of UIKit's values. Inside a blur
  material, Apple draws them vibrantly, blended with what the material blurs:
  - On macOS, any view in that colour inside the material.
  - On iOS, only in a material whose box clips its children on both axes, which then hosts them in its effect view.
    In that material, a paragraph's text and a plain fill (no border, gradient,
    image, corner shape or `background-clip`) are vibrant.
  - Everything else, and glass, draws the pair.
- **Raster `tint-color`** ([LLP 1011 §3](1011-image-v1.spec.md)) is a template
  image operation without a CSS property of that name. On the web the tint is
  a `mask-image` on the `<img>` itself, so it also masks the element's own
  background, border and shadow (declared limitation: `issues/closed/20260927-web-tint-masks-the-box.md`),
  and a remote source needs CORS or paints nothing, which is the browser's mask
  security policy. Apple and both Linux painters apply a source-alpha tint to
  the picture alone.
- **Native `text-transform`** ([LLP 1064 D5 and “Not done”](1064-box-shadow-and-text-transform.rfc.md))
  uses Unicode's root case mapping, without language tailoring: the native
  paragraph path has no language-specific mapping input. Native Markdown
  bypasses the transform because hosts expand the source themselves and
  transforming that source would rewrite URLs; web CSS transforms rendered
  Markdown. This is a native coverage gap, not CSS's behavior.
- **Projected iOS tab-bar height** ([LLP 1059 D2](1059-tab-bar-projection.rfc.md)):
  `UITabBar` reports its intrinsic height through the kernel measurement seam
  and fills the resulting box; a segmented control (`UISegmentedControl`,
  macOS's `NSSegmentedControl`) reports its own the same way and fills the
  content box (LLP 1059 D2a). The former overflow deviation
  is removed (`issues/closed/20260927-tab-bar-height-to-layout.md`).

**Clock timelines (2026-10-03, [LLP 1055.002](1055.002-synced-animations.rfc.md)
D2; not reviewed).** `animation-timeline` takes a third value, `clock(<ident>)`,
which CSS has no form of. Its animations stay on the clock (it is `auto` to
every other reader, the name lookup included) and each starts at the last
cycle boundary of the timeline it names, so every animation on one timeline is
in phase. The reason: indicators that appear at different moments (two pending
commands, skeletons) should pulse together, and CSS can only do that with a
script setting each animation's `startTime`; the web host does exactly that.

**Drag timelines (2026-09-27, [LLP 1057.003](1057.003-gesture-timelines.rfc.md)
D1, accepted by Charlie).** `drag-timeline`, bit 150, is not CSS. CSS names a
timeline on a scroller (`scroll-timeline`) or on a box's visibility
(`view-timeline`), and has none a gesture drives. This row takes `none |
<dashed-ident> [x | y]?` and names a timeline whose position is the node's
presented `translate` on the axis (`y` if unsaid): the held value while a drag
holds it, then the spring the drag hands off to. The reason: a follower, such
as a backdrop fading with a dismiss drag, must be a function of its source as
presented, in the frame the source moves, with no app code per frame, and no
CSS timeline has a drag as its source (LLP 1057.002 §6.10).
- **The consumer's rows are CSS's**, `animation-timeline` (bit 151) and
  `animation-range` (bit 152), in a subset: `auto | <dashed-ident>`, and
  `normal | <length> <length>`. A drag has no scroll range for `cover`,
  `contain` or percentages to name. `normal` leaves the node on the clock.
- **The mapping is a scroll timeline's.** The range spans the animation's
  delay and active interval together, unclamped, so its fill decides outside
  the range. The parity fixture holds this to Chrome's CSS scroll timelines
  (`host/web/tests/fixtures/browser-motion.txt`, the `tl-` cases).
- **Paint rows only (Q1).** The compiler refuses a bound animation whose
  keyframes animate a layout or geometry row (`lower-timeline-row`). An SVG
  element takes no timeline yet.
- **An endless animation is refused** (`lower-timeline-endless`). CSS gives
  it no duration on a progress-based timeline and shows its end; a computed
  one that reaches a host holds its start.
- **Names scope as CSS scopes them** (D4): `timeline-scope`, bit 153, is
  CSS's row, and a consumer's name resolves to the nearest ancestor-or-self
  that declares or scopes it, in the kernel and in the web's glue alike.
  `all` follows the specification's text; Chrome 154 does not parse it.
- **A name that finds no single source is Chrome's.** An inactive timeline
  (a scope with no declaring descendant, or several) leaves the animation
  without effect; with no timeline in scope, the animation keeps the time
  it has, 0 when new. Declared: when a lost timeline is found again, the
  animation follows it. Chrome leaves it paused (at 0%, where Web
  Animations 2 holds its last progress), which would freeze a follower
  whose source a commit replaced.
- **The web lowers no timeline.** The consumer's CSS animation is paused and
  seeked where the drag writes its held value, and once per frame while a
  release spring runs; phase 2 (D3) removes that per-frame seek.
  `timeline-scope` is emitted as CSS too, for the timelines the browser
  will resolve (D5), and as `--exact-timeline-scope` for the glue's lookup.

Declared deviations, and beside each what is CSS's own:
**CSS grid's supported grammar follows Chrome's.** Track templates preserve
named lines and `repeat()` (fixed, `auto-fill` and `auto-fit`) and accept px,
percent, fr, auto, min/max-content, `fit-content()` and flexible `minmax()`
maxima. Placement accepts numeric and named lines and spans. Keywords and units
are ASCII-case-insensitive; nested `repeat()` is refused as invalid CSS.
All six rows are parsed as CSS tokens, so escapes and comments have their CSS
meaning; canonical output uses the CSS serializer for identifiers and retains a
shorthand's trailing placement whenever omission would change that meaning.
The following Chrome-valid forms are the complete declared grid grammar gaps:

- `subgrid`; Taffy has no subgrid layout algorithm;
- track lengths in units other than px, and calculated/custom/environment
  track or placement values (`calc()`, `min()`, `max()`, `clamp()`, `var()`,
  `env()`); Taffy's portable value has no browser unit context, cascade or
  calculation resolver;
- CSS-wide keywords (`inherit`, `initial`, `unset`, `revert`, `revert-layer`)
  on all six rows, plus custom/environment values on `justify-items`; kernel
  style rows store specified values without a CSS cascade;
- `justify-items: last baseline` and its `legacy` modes; Taffy exposes neither
  alignment mode;
- more than 10,000 explicit tracks, or a placement index/span whose magnitude
  exceeds 10,000. Chrome 154 accepts larger specified values and clamps used
  grid coordinates at 1,000,000; Taffy's layout engine clamps at 10,000, so
  Exact refuses the larger portable value instead of laying it out differently.

Literal declarations in those gaps are compile errors. The JS target filters
the same forms after asking Chrome for its specified-value serialization
(escapes resolved, comments removed), so it cannot lay out a value native
cleared. A value Chrome rejects clears the row on both paths. Other invalid
dynamic grammar is dropped exactly as the kernel parser drops it.

`position` is CSS's (LLP 1074 T1, 2026-09-30): `static | relative | absolute`,
`static` initially. An absolutely positioned box is placed against its nearest
positioned ancestor, or the root, and sits at its static position on an axis
with neither inset (vendored Taffy patches 18 and 20, held by 79 literal-Chrome
cases). A static box's insets do nothing, and its `z-index` applies only when
it is a flex or grid item. Declared, as what stays of the old rule that every
box was a containing block:
- **A box that clips, scrolls, transforms or animates is `position: relative`
  unless it names a position.** The Contract compiler lowers it
  (`contract/lower/src/tags.rs`, `CONTAINS_ABSOLUTE`), and refuses an authored
  `position: static` there. A transform or a filter makes a containing block in
  CSS too. `overflow` and motion at rest do not: a native host clips and
  scrolls a box's view subtree, so a descendant placed against a box outside it
  would still be clipped and scrolled by it, and a browser makes a box a
  containing block while a transform runs on it. The kernel's own rule is
  position alone; a producer other than the compiler sets the row itself.
  This includes context-preview structural recipients (side/trailing siblings,
  repeated flow roots and the source scroll content), and Runner-generated
  row wrappers in a list with `reorderdrop` (LLP 1074 D1).
- **The kernel paints in tree order.** A page paints its positioned boxes and
  stacking contexts after its in-flow boxes, so a web host makes a static box
  that follows one of those in tree order `isolation: isolate`
  (`host/web/src/layers.rs`): it then paints in tree order with them and is no
  containing block. This rule replaced `position: relative` there, which had
  replaced the page's blanket `#exact-root * { position: relative }`
  (2026-09-29): a positioned box is a paint layer, and with every node one, a
  10k-row grid spent 50–70 ms per interaction in hit-testing and compositor
  commit (select a row: 150 ms input→paint, 63 ms after).
  The promoted box is a paint group, including its descendants: its child's
  `z-index` cannot escape above an earlier sibling whose `z-index` places it
  above that group. This is the sibling scope below, not an exception for a
  static wrapper. For example, an earlier absolute sibling at `z-index: 1`
  stays above a later static wrapper's absolute child at `z-index: 2`.
  The group's blend backdrop excludes earlier siblings outside the group.
  Authored `mix-blend-mode` and `isolation` are SVG-only rows (LLP 1055.000
  stage 10c); the compiler refuses them on boxes. An SVG island's multiply
  child blends with earlier SVG paint inside that island, not a blue box
  behind its static wrapper. `host/web/parity.mjs --paint` holds these
  nested-z and internal/external SVG backdrop cases on web and macOS
  (2026-09-30); the latter allows the declared display-colour-space difference.
- **`z-index` orders siblings.** Apple's presenters give it to the layer
  (`usedZIndex`); the Linux painter stacks siblings by the same rule (LLP
  1083 D6). CSS orders a whole stacking context.
- **`order` lays out, and painting stays in tree order** (feed F19). A flex
  or grid container hands the layout engine its children in order-modified
  document order (`kernel/src/layout/order.rs`), so items are placed as CSS
  places them. CSS also paints flex and grid items in that order; the kernel
  paints them in tree order, so two reordered items that overlap stack as
  their tree order says. Focus and accessibility order are tree order, as
  on the web.
- **`position: fixed` is not a row.** `sticky` lays out as `relative` with no
  offset; each native host moves it as its scroller scrolls (LLP 1083).
A text field (`input`, `textarea`) lays out as the web's (2026-09-30): it keeps
its own width in a block container, where a `<div>` stretches, and stretches in
flex, under insets and at a percentage. At `field-sizing: fixed` its width is
HTML's 20 characters (a textarea's 20 columns). How wide a character is, the
web leaves to the browser, and browsers differ: Blink takes the font's OS/2
average width plus a margin, or the width of `0` for fonts on its own list.
Declared: the kernel takes the width of `0` in the field's font, for every
font, since no two hosts share font metrics (Caltrain's search field, in the
same face, is 175 px in Chrome on macOS and 195 px on the macOS host). The web
is the standard, not one browser's metrics (Charlie, 2026-09-30); an app that
needs a field's width sets it
(`issues/closed/20260930-field-character-width.md`).
`text_align` is CSS's (`start` initially;
`start` and `end` resolve against the paragraph's `direction` in
`Paragraph::from_style`, so hosts see only left, center, right or justify; LLP
1053). CSS `direction` orders flex rows and places blocks in the kernel, and
`direction: rtl` is a paragraph's base direction on every host. Declared: under
the initial `ltr`, the Apple and Linux engines still take a paragraph's base
direction from its first strong character (as `unicode-bidi: plaintext`), so
an `ltr` paragraph that opens with Hebrew or Arabic orders mixed text as an
`rtl` one would. Its alignment is still `start` = left, and the web uses CSS.
The Linux text-flow walker cannot yet break an RTL run inside an LTR
paragraph. Font matching stops at the nearest
real declared face and never synthesizes weight or style, rather than CSS's initial
`font-synthesis: weight style small-caps`; the compiler diagnoses a literal
weight/style whose declared family lacks the needed face, and the web host emits
`font-synthesis: none` (LLP 1019 §5). A `ScrollView`/`List` scrolls on its
block axis unless the producer sets `overflow_y` — the only per-tag default,
applied in `StyleProps::to_taffy` (a scroll container is `overflow: auto` on the
web). A `NativeView` may report a preferred content-size pair through the same
`Kernel::set_intrinsic_size` seam as a `Control` (LLP 1024 D4, 2026-10-01).
It is a measured leaf without an inferred ratio or a projected tablist's
minimum; CSS still determines the outer box. Without a report its content
measures zero, and a block still stretches to available width. Reports are
finite positive pairs or `None` to clear, not constrained measurements.

An `Image` is a replaced element: the host reports its intrinsic size
(`Kernel::set_intrinsic_size`, the bitmap's pixel counts one-for-one as points,
after the image loads; before that each unknown axis measures 0, so a `width`
row still sizes the box), the node is measured from it, and it keeps its
natural ratio under `aspect-ratio: auto`. `auto <ratio>` uses the natural ratio
once known and the given one before; a plain `<ratio>` overrides it (LLP 1053
G1). One dimension given, the other follows by the ratio and each is clamped
on its own; with neither given, min/max resolve by CSS 2.1 §10.4's table
(Taffy patches 5 and 12). Any box takes CSS `aspect-ratio` (`auto || <ratio>`,
stored as authored). A non-replaced box's derived height is a floor its
content can pass unless `min-height` is set or it scrolls; min/max transfer
through the ratio only into an axis the box does not size. A width derived
from a definite height takes its content-based automatic minimum (unless
`min-width` is explicit or the box scrolls). Percentage children resolve
against the ratio's preferred height even when the content minimum enlarges
the used box. Absolutely positioned boxes,
grid items, a flex or grid container's own ratio and the root size through the
ratio as block and flex items do (LLP 1074 T2; `vendor/taffy/EXACT-PATCHES.md`
patches 12, 18 and 19). In block flow an auto-width image keeps its intrinsic
width, as CSS has it; in a stretching flex column it stretches, by ratio
(`kernel/tests/it/image.rs`; LLP 1011).
A `Canvas` is replaced too: before any row its natural size is its bitmap's
default, 300×150, so it has the natural ratio 2:1, which a plain
`aspect-ratio` overrides. An `iframe` (`WebView`), a `Video` before its
metadata and an `svg` without a view box have only CSS Images 3 §5's
300×150 default object size and no natural ratio: a width leaves the height
150, an authored ratio alone stretches the box to the offered width, and a
flex row may shrink a bare `svg` to nothing, as Chrome does
(`kernel/tests/it/browser_replaced.rs`). A canvas's children are laid out in
the box its measure gives and never size it (Taffy patch 16). No replaced
element stretches to a grid area by default or between an absolute box's
insets (Taffy patch 17). Declared: the
web draws a canvas as a `div` that CSS sizes as a `<canvas>`
(`host/web/src/element.rs`, `canvas_css`), except that in a flex row its
automatic minimum width is a `div`'s: two bare canvases in a 400px row are
200 wide there, and 300 in the kernel and on a `<canvas>`.
`white-space` is a paragraph row: the measure interface carries one mode per
paragraph, so an inline run inside a paragraph takes its paragraph's collapsing
and wrapping, where CSS lets each inline element choose its own. A text input's
value is never collapsed, as the web's `<input>`/`<textarea>` (the kernel measures
a collapsing row there as `pre-wrap`). Native `text-overflow: ellipsis` truncates
`nowrap` lines on Linux; a wrapped line that overflows with an unbreakable word is
ellipsized on Apple and the web but not yet on Linux. `overflow: hidden` on a
box with four equal radii clips its children to the rounded border box on
every host. Declared: with unequal radii, UIKit and AppKit clip to the unrounded
box, since a layer's single corner radius cannot carry four and `layer.mask`
already carries `clip-path` (LLP 1054.000 R7). Web flowed text with
`tabular-nums` keeps ordinary layout: canvas cannot measure the feature
(LLP 1053 §0 G4, G5). A preserved tab stops every eight spaces (`tab-size`'s
initial value; no row sets another). Declared: under `system-ui` Apple's stops
are up to 2% narrower than Chrome's, which sizes them from San Francisco's
untracked space where CoreText's space is tracked (LLP 1053 §0.1).
`backdrop-filter` is `none` or one `blur()` (LLP 1053.000). Declared: iOS draws
it as `UIBlurEffect.Style.light`, whose blur and tint are its own (no public
arbitrary-radius backdrop blur exists; Charlie's ruling, 2026-09-27); macOS's Core Image blur
matches Chrome's σ but clips the node's children to its border box and sees
only its parent's subtree as the backdrop; the GPU painter on Linux renders
the frame once more per backdrop node. The measured bounds are LLP 1053.000 §3.

## 2. The data model (WS-A)

- **Typed props.** `PropId` is a generated `#[repr(u16)]` enum; every prop declares
  a `PropKind` (`str | bool | int | float`). A boolean is a boolean on the wire and
  in storage; `"true"` is unrepresentable. A kind mismatch is a decode rejection
  (`PropKindMismatch`) or, for in-process ops, an apply rejection.
- **Spelling-check hint** (2026-09-09, Messages). HTML's enumerated `spellcheck`
  is a string prop, preserving the authored spelling. `NodeRef::spellcheck()`
  returns the nearest explicit hint through logical ancestors: ASCII-case-insensitive
  `true`/`false`, empty true, invalid/missing values inherited without trimming.
  `None` leaves the editor's default in charge. The getter does not modify props;
  native hosts project its result to editors and the web emits the authored attribute.
- **Columnar arena** (`arena.rs`). Nodes are slots; every attribute is a column;
  topology is index-based (`parents`, `children`); destroyed slots go on a free
  list. Frames and Taffy handles are *derived* columns — rehydration is
  columns-plus-rebuild (`LayoutTree::rebuild`, `Kernel::rehydrate`), never
  serialized engine state.
- **Identity.** A producer names nodes by a wire-local `ViewId` (u32, unique among
  live nodes in one kernel). Inside, a `NodeKey { index, generation }` names one
  allocation; generations start at 1 and bump on slot reuse, so a stale key never
  resolves (`arena::tests::keys_fail_closed_after_reuse`). Receipts and agent refs
  carry keys, never bare ids. `CreateView` on a live id of the same type is a
  no-op (0507 §5.4 `LiveNoop`); of another type, a rejection; after a destroy in
  the same batch, allocate-after-destroy with a fresh generation.

  *Deliberately not yet built:* the six-field raw address of 0507 §5.1
  (`ProducerId`, `ExecutionGeneration`, `rootId`, `rootIncarnation`, …). v1 has one
  producer per kernel and one incarnation counter (`Kernel::incarnation`, bumped by
  `reset`). The fields are added when a second producer or HMR exists to need them.

- **Selector index.** `testId` is a kernel citizen: an exact-value multimap,
  maintained on set/clear/destroy/reset, returned in structural tree order
  (`Kernel::find_by_test_id`). `find_first_by_test_id` uses that same order:
  unique names resolve directly from the index; repeated names stop at the first
  structural match, with detached slots last. Targeted agent reads separately
  verify attachment and recover depth from the current ancestors.
- **The environment** (2026-08-30). A dimension row takes a fourth kind beside
  `auto`, points, and percent: an `env()` length — CSS's
  `env(safe-area-inset-<edge>)` and `calc(env(safe-area-inset-<edge>) ± <n>px)`,
  or `calc(<n>px + env(…))`, since addition commutes (2026-10-03: the Signal
  Clone's `calc(300px + env(…))` was refused while the other order compiled;
  `<n>px - env(…)` negates the inset, which no dimension holds, and is still
  refused), parsed once in `style.rs` (`Dimension::parse_env`; text on a dimension row is
  that or a rejection; wire kinds 3–6, one per edge, the `f32` the added points;
  no fallback argument, since the host always defines the four). The kernel
  holds one `Env` — the four insets in points, the host's, set with the viewport
  (`Kernel::set_env`; a `reset` keeps it, a rehydration carries it) — and
  resolves every `env()` length against it where the engine style is derived
  (`taffy_style`); `set_env` re-derives and dirties exactly the nodes whose style
  reads an inset (`uses_env`) and says whether any did, so a host lays out only
  when something can move. Zero until the host says otherwise, as a browser
  reports the insets for a page without `viewport-fit=cover` (LLP 1008 §9).
  `tests/env.rs`.

## 3. One write path (WS-D, WS-F, 0507 §4)

Two ingress forms, one engine: EXWF byte frames (`Kernel::apply_frame`) and
in-process structured apply (`Kernel::apply(root_id, batch, &[Op])`). There is no
direct setter. Both enter `txn::apply`, which **validates the whole batch against a
staged view** (arena plus the batch's own earlier ops) **before writing anything**.
A rejection is a typed `ApplyError` and the arena, the layout engine, the selector
index, the epoch, and every receipt are exactly as they were —
`tests/apply.rs::every_rejection_class_leaves_the_kernel_untouched` proves it per
class by comparing the EXNODE export before and after.

The closed op list (revision 1): `CreateView`, `DestroyView` (**subtree** — the
WS-I correction, from day one), `SetProp`, `ClearProp`, `SetStyle` (a **masked
patch**), `ClearStyle` (a mask), `SetChildren`, `AttachRoot`. There is no
`ComputeLayout` op: layout is a host call (`Kernel::compute_layout`), because the
host owns the frame clock. A root with `width: auto` fills the width it is
offered — CSS's block rule, which Taffy does not apply to a root — and stays
as tall as its content: the page a viewport scrolls (added 2026-08-29 when the
Apple host's first bare-root fixture laid out 89 pt wide; a root's engine
style is re-derived on `AttachRoot`).

`SetChildren` rejects: duplicate children, self-child, a root as a child, a cycle
(child is an ancestor of the parent), children on a leaf type, a non-`Text`
child under a `Text` (a text node's children are its inline runs; anything else
would be a node the engine never lays out — `InlineRunNotText`), and a node
more than `MAX_DEPTH` (128) levels below the top of its tree, inline runs
included (`TooDeep`, 2026-09-23). Layout recurses once per level: release
Taffy takes up to ~3.5 KiB of stack a level (flex; block 1.9, inline runs
0.7) and hosts lay out on their main thread, 1 MiB on iOS; a 20,000-deep
tree overflowed it and aborted the process. The deepest app tree is 9
(Caltrain). The bound stays 128, not the 1,024 first asked for. The coordinator
settled it on Charlie's word, 2026-09-23: "tackle everything still open from
the core lane". A child listed
under a new parent is reparented; children dropped from a list become detached
(live, no parent, in no root's layout) — not destroyed. An old parent is
pruned once per op, or once per run of consecutive `DestroyView`s, in one pass
and one engine update: moving N children, or destroying them one op each, is
O(N), where it was O(N²) (8,000 of either took 0.7–0.8 s; 2026-09-23,
counted by `txn::tests`). Every number in a style
patch must be finite on both ingress paths (`DecodeError::NonFinite`,
`ApplyError::NonFiniteStyle`); NaN never reaches a frame.

The apply phase assumes only what validation established. If that assumption ever
fails it stops and returns `ApplyError::Internal` — typed and loud, never a silent
skip and never a panic. That error names a kernel defect: earlier ops in the batch
were applied and no receipt was published, so a host `reset()`s and re-snapshots.
There is no `unwrap`/`expect` on the production path.

## 4. EXWF frame revision 1 (`wire/frame.rs`)

40-byte header: magic `EXWF`, revision u16, header_len u16, frame_len u32, root_id
u32, batch u64, schema digest u64, flags u32, reserved u32. 16-byte op header,
8-aligned: opcode u16, flags u16, view_id u32, payload_len u32, reserved u32; the
payload is zero-padded to 8. **There is no op count on the wire**; a consumer
iterates by validated lengths until `frame_len`. Reserved fields must be zero.
Every malformation names a `DecodeError`; a frame that fails anywhere applies
nothing (`tests/wire.rs::a_malformed_frame_applies_nothing`).

Value grammars: dimension = kind byte (0 auto, 1 points, 2 percent) + f32;
**percent is authored 0–100** on the wire and in storage and converted to Taffy's
fraction exactly once (`style.rs`); `auto` is admitted per row (`admitsAuto`) and is
a rejection elsewhere (`AutoNotAdmitted`); colors are `0xRRGGBBAA`; grid templates
and placements are length-prefixed canonical CSS strings, reparsed and domain-
validated at decode; portable grid templates, indexes and spans above Taffy's
10,000-track limit are refused rather than clamped. Enum bytes outside
their vocabulary are rejected, never defaulted.

## 5. Layout proportional to change (WS-H)

Per-node flags (`STYLE_DIRTY`, `TEXT_DIRTY`, `CHILDREN_DIRTY`, `PROPS_DIRTY`,
`PAINT_DIRTY`, `GEOMETRY_CHANGED`, `CREATED`), one published epoch
(`Kernel::epoch`, bumped only by a commit that changed something), and a
`CommitReceipt` per batch (`created`, `destroyed`, `touched`, `layout_invalidated`)
retained in a 64-deep ring. During apply, touched generation-checked keys collect
in a vector. Adjacent duplicates collapse before sorting; publication removes
remaining duplicates, destroyed generations and newly created nodes, in slot
order. The retained vector releases excess scratch capacity. The same-batch
reuse case is held by `tests/apply.rs::touched_receipt_is_unique_ordered_and_excludes_destroyed_or_created_generations`.
`compute_layout(root, offer)` runs Taffy over that root, publishes absolute frames,
and returns a `LayoutReceipt` naming exactly the nodes whose frame bits changed —
the changed-geometry receipt.

Frames retain fractional CSS pixel geometry (Messages, 2026-09-09). Taffy's
whole-point rounding is disabled when constructing the layout tree, including
rebuilds and rehydration. A half-point height edit must move the following row
by half a point, not by a whole point or zero; nested fractional offsets and
intrinsic image ratios must survive publication too. Kernel regressions and a
browser/iPhone fixture cover those cases. Rasterization belongs to the host;
this does not remove rounding inside an injected text measurer or promise
identical floating-point quantization in every browser engine.

Intrinsic flex contributions exclude the containing flexbox's padding and border
(Codex, 2026-09-11; Taffy patch 6). Those insets are added once, after summing
the items. Flooring each item's contribution by its parent's inset made a
48-point Messages bubble containing a text column 56 points wide; columns also
overstated intrinsic height. The kernel regression covers both parent and child
directions and independent parent/child padding; the original implementation
fails it. Browser measurements supply the expected dimensions. This changes
neither the app's minimum width nor its padding.

Percentage padding and border widths resolve against the containing block's
width on all four sides (2026-09-23; Taffy patch 10). Taffy's block algorithm
sized a child with its vertical sides resolved against the parent's height:
`height: 50px; padding-top: 5%` under a 400px-wide auto-height parent was 50px
tall where Chrome makes it 70px (`kernel/tests/it/browser_cases.rs`).

**The result-equality gate is a test, from the first commit.**
`tests/layout_equality.rs` mutates a random tree for hundreds of rounds and asserts
the incremental frames equal, bit for bit, both a kernel rehydrated from the
columns and a kernel that replayed every batch from scratch. This is RFC 0491's
Phase-5 exit conjunct (1), moved to day one where it costs nothing.

An engine fault is never a panic (`#![forbid(unsafe_code)]`, no `expect` on the
production path): `LayoutTree` records it, `compute_layout` reports
`LayoutError::Engine`, rebuilds the engine tree from the columns, and retries once.

## 6. Text (WS-E, WS-I)

**Book typography** (the reader diary, 2026-10-04). `text-align: justify`
fills every line but a paragraph's last and one before a forced break, on
every host; a line with no justification opportunity stays at the start, as
Chrome's does. `text_indent` (bit 176, `f32` points, inherited; a number,
`rem` or `em`) insets a paragraph's first line from its start edge and takes
that room from it; a negative value hangs it. A percentage and `hanging`/
`each-line` are refused. `hyphens` (bit 177, inherited, initial `manual`): a
soft hyphen (U+00AD) is a break that shows the face's hyphen, which must fit
the line too; under `none` the kernel's runs carry U+034F in its place (no
break, the same length in UTF-8 and UTF-16, so no offset moves); `auto` adds
the document language's hyphenation points (no `lang`: the web page's `en`).
Declared: **Linux has no hyphenation dictionary**, so `auto` breaks there only
at soft hyphens, logging it once; Apple hyphenates by the document's language,
not an element's `lang`; text flowed around exclusions and a content region
take no `text-indent` and no `auto` points. What needs fragmentation, a
paragraph continuing from one box into the next, is refused by name: `widows`,
`orphans`, the `break-*` properties and multi-column layout (`columns`,
`column-count`, ...); Taffy has no fragmentation and there is no paged
context, so they could change nothing. LLP 1093 (admitted 2026-10-05, not
yet built) is the planned home of multi-column and its break rules; the
refusals stay until its stages land.

**CSS line height** (LLP 1035.000.000, 2026-09-11). Bit 72 uses the
`line-height` codec: `Normal` (schema default), `Number(ratio)`, or
`Length(px)`. The wire tag is 0 for normal, 1 plus f32 for a ratio, 2 plus
f32 for a length. Negative/nonfinite values are refused. Percentages and
font-relative length strings are unsupported. Inheritance retains the kind;
`TextStyle::from_style` resolves a ratio using the receiving font size and
returns `Option<f32>`: `None` is natural metrics, `Some(0)` is zero. A
`Paragraph` also carries its own `TextStyle` strut so flattening smaller
inline children cannot erase its minimum line box. The existing schema
digest refuses old plans/frames; there is no numeric-points compatibility mode.

**Inheritance** (LLP 1035.000 D1–D4, landed 2026-09-09). The schema marks the
rows CSS inherits with `inherited: true` — `text_color`, `font_family`,
`font_size`, `font_weight`, `font_style`, `line_height`, `letter_spacing`,
`font_variant_numeric`, `direction`, `white_space`, `text_align` — and the
generator emits `StyleMask::INHERITED` and `StyleId::inherited()`. One
mechanism serves them all: `NodeRef::computed(id)` is the own row, else the
nearest logical ancestor's for an inherited row, else the initial value;
`source_of(id)` names the node that supplied it; `computed_style(rows)`
resolves a set of rows in one walk; `text_color()` and `text_style()` are
instances. Authored presence stays in the own mask — a computed value is
never written back. A run measures with its computed style (`arena.text_runs`),
so a `text` child without a `font_size` takes its paragraph's, as a `<span>` in
a `<div>`; a paragraph's `direction` and `text_align` inherit into its
measurement too. Invalidation is the kernel's: a write to an inherited row
marks and touches every logical descendant that does not set the row itself
(text rows remeasure its paragraph, the rest repaint), stopping under an
override. Attaching or moving a child propagates only rows whose computed
value differs, including an orphan's own/default values before attachment.
The transient before/after snapshot holds only the schema's inherited rows;
it shares the ancestor walk with full computed styles and retains no cache.
Ancestry changes still invalidate paragraph paint/source metadata and a moved
text node's measure ownership. The receipt names what an inherited change
reached; no host re-derives descendants per frame. A light/dark pair is
preserved for the host to resolve.
`kernel/tests/apply.rs` holds colour (reparenting, cleared overrides) and the
text rows (a bare, a bold and a small run; the touched set after an ancestor
change, a move and a clear; an identical write touching nothing).

Text measurement is a **per-kernel injected trait object** (`Box<dyn TextMeasurer>`),
never a process-global callback. The kernel hands the measurer a paragraph as
ordered `TextRun`s: a `Text` with its own `text` prop is one run; otherwise its
`Text` children are its runs, flattened in order — one structural traversal
(`NodeArena::text_runs`), the WS-I `text_fragments()` IR. Inline runs are measured
with their owning paragraph and have no geometry of their own; editing a run marks
the owner dirty (`measure_owner`). Content-sized `TextInput` measures its `value`
or `placeholder`; fixed fields measure 20 `0`s, a textarea two rows of them (declared above). A
content-sized textarea ending in a newline includes its final caret line
(2026-09-10): a zero-width measurement run preserves the empty paragraph that
CoreText omits. The stored value is untouched; ordinary text keeps the shaper's
terminal-break behavior. `kernel/tests/review_fixes.rs` covers repeated Returns,
shortening and unchanged values. `MonospaceMeasurer` is the deterministic reference measurer for
tests and headless hosts.

**Paragraph input identity (Tuft / Carson, 2026-09-17).** Independent Text and
TextInput measurement owners expose `NodeRef::paragraph_stamp()`. Inline Text
returns `None`: its own run subset must not share an identified key with its
owner's full paragraph. `ParagraphStamp` is an opaque retained, payload-free
namespace plus the owner's generational key and current metric/paint-source
revisions. Full equality compares both revisions; `same_metrics` excludes
paint/source. The namespace uses allocation identity, not value equality or a
serialized address. Stamps retain neither the arena nor text.

Layout calls `TextMeasurer::measure_identified(stamp, request)` with the same
owner stamp exposed by reads. Its default calls the existing synchronous
`measure`; requests and canonical run construction are unchanged. Offers,
font-catalog identity, host appearance/resolved paint, attachment eligibility
and accepted publication remain separate inputs. This is an input proof, not
an asynchronous result or a promise that equal stamps produce equal pixels.

Each arena slot stores two current revisions; storage follows arena high-water,
not edit history. Source/topology invalidation updates old/new paragraph owners
and affected inherited inputs; temporary work is bounded by the traversed
subtree and distinct owners. Free clears revisions. New, reset, cloned and
rehydrated arenas get fresh namespaces; derived Taffy rebuilds preserve them.
Counter exhaustion rotates the namespace inside a validated successful apply,
without a fallible partially applied edit. Rejected batches and exact no-ops
preserve identity. Revisions may conservatively change for masked inputs.
Alignment/overflow invalidation and textarea semantic-tag measurement belong to
this proof; their schema changes require rebaked plans. Public-arena fork,
rollover, topology, lifetime and callback/read agreement regressions live in
`kernel/tests/paragraph_stamp.rs` and the arena unit tests.

## 7. EXNODE export (WS-C)

One crossing per sync. `Kernel::rows` is the typed in-process projection; `Kernel::
export` is the binary envelope: 40-byte header (magic `EXNO`, version, total length,
root id, epoch, node count), a section directory with an FNV-1a-32 checksum per
section, and three 8-aligned sections — 32-byte node rows (id, parent, generation,
type, flags, depth, frame), per-node masked style patches, per-node typed props.
`export::decode` validates every length and checksum before adopting anything, and
never allocates from a count (sections, rows, children) it has not first bounded
by the bytes actually present — with the arithmetic in `u64`, so a 32-bit wasm
target cannot overflow on a hostile length either.

`Kernel::row(id)` returns the same typed metadata for one live node, including
detached nodes, with subtree-relative depth zero. It allocates no traversal and
does not visit descendants. Agent shallow reads check live-root attachment and
supply the absolute tree depth before serializing that one row.

## 8. Errors

One convention (`error.rs`): `Result` with a typed error naming the exact
condition. `DecodeError` for the wire, `ApplyError` for rejected batches,
`LayoutError` for layout, `KernelError` as the union. No status integers.

## 9. Not in v1 (and where each is declared)

Selection, semantics/accessibility tree, islands, SVG rasterization, crash
capsules, a module registry, virtualized lists, portals, choice layout (0487's
operator — the corpus stays research until a producer needs it), the direction
truth table (RTL box layout), the 0507 raw-address namespace, EXWF extension
chunks, event frames, the wasm host interface. Each is either on
`rules/DEFERRED.md` or waits for the consumer that would make its spec
transcription rather than speculation. (The C ABI found its consumer on
2026-08-29: the Apple host, LLP 1008 §4.)

## 10. Checks that hold this

`cargo test -p exact-kernel` (unit + `tests/{apply,wire,export,layout_equality}.rs`),
`cargo clippy -p exact-kernel --all-targets -- -D warnings`, `cargo fmt --check`,
`cargo build -p exact-kernel --target wasm32-unknown-unknown`, `node scripts/caps.mjs`.
Every source file is under 1,500 lines; the largest is the generator.

### Platform background materials

`backgroundMaterial` (prop 54) requests a system material, not a CSS blur
radius. `ultra-thin` is consumed by the Messages focused reply thread. UIKit uses
`UIVisualEffectView` with `.systemUltraThinMaterial`, including the platform's
appearance and accessibility adaptation; web approximates the material with
`backdrop-filter: blur(20px) saturate(180%)` and an appearance-aware translucent
fill. `glass` uses `UIGlassEffect(style: .regular)` on iOS 26, falling back
to ultra-thin blur on earlier iOS. Its corner configuration follows the node’s
uniform border radius. Web uses translucent fill, blur, and a light shadow.
Messages uses glass for its composer, header controls, and inbox search. Authored
children go in the effect’s `contentView`; enabled nodes with a press handler use
`UIGlassEffect.isInteractive`. Changing or clearing the material preserves those
children. AppKit uses `NSGlassEffectView` with regular style on macOS 26 for
`glass`; earlier macOS and `ultra-thin` use `NSVisualEffectView(.popover)` with
within-window blending and window-active-state tracking. AppKit has no ultra-thin
material; this is a semantic floating-surface fallback, not pixel parity. Authored
children use the glass content view unless a scroll/canvas already owns their
container. AppKit supplies appearance and accessibility adaptation. Glass grouping
is the `glassGroup` prop (LLP 1053.000.000): its value is the spacing in
points at which the subtree's glass merges (or `"auto"`, the element's gap
along its main axis, LLP 1053.000.000.000), through `UIGlassContainerEffect`
or `NSGlassEffectContainerView` as the node's innermost view; it is
layout-neutral and draws nothing on the web or Linux, and is refused beside a
material, on a scroll or on a canvas. Declared deviations, measured: inside a
group the platform draws all its glass in one layer, beneath the group's
other content on iOS and above it on macOS, whatever the CSS order; glass
overlapping or inside glass fuses into one shape; and because the platform
ignores the opacity, masks and clipping between a group and its glass, the
host isolates a glass whose path fades, masks or clips in a container of its
own, where it is faded and clipped as CSS says and merges with nothing. Since LLP 1053.000 D4 `backgroundMaterial` names every UIKit
and AppKit material (the schema's `materials` table); a platform without the named
one draws its stand-in and logs once, and the web and Linux draw the table's stated
approximation (a blur and a tint; declared approximate). This explicit
host policy stays the Apple-policy spelling beside CSS `backdrop-filter` (LLP
1053.000 D3): it is not sugar for a blur, and where a node has both the material
wins on every host (the web's material rule is `!important` over the inline
blur).

### Native buttons

Exact's `button` is the author's box: its UA sheet is `appearance: none`
(a fixed row, which `layout` reports), where a browser's is `auto`. An
`appearance` that is the literal `auto` after class merging makes it the
platform's own button (LLP 1069.011): a `Control` of type `button`, UIKit's
`UIButton` with the `UIButton.Configuration` its `buttonStyle` names,
AppKit's `NSButton`, the browser's own `<button>`, a painted button on Linux.
Its `text` and symbol `image` children are its title and image, read from the
kernel, never laid out. Declared: its box is `border-box` on every host with
the platform's chrome inside it; its box refuses `padding`, `border`,
`background`, `box-shadow`, `filter`, `overflow`, colour and typography,
which LLP 1069.001 D6 lets other controls' boxes take, because a browser
drops a native button's look under them; it refuses `direction` and
`pointer-events` too (its face's order is its children's, and the platform
hit-tests its own control); `accent-color` colours what the platform colours
with its tint, as iOS does on the web and Linux; inherited typography is
reset on the web's native face; Linux draws no symbol. Its size is the
platform's, as any control's is.

### Window toolbars

Charlie requested native window-toolbar presentation for Interview on 2026-09-15;
implementer: Codex, same date. `toolbarPlacement="window"` (string prop 74) on
`role="toolbar"` explicitly requests window chrome. A toolbar role alone does
not. AppKit projects one visible declaration with direct pressable buttons and
at most one direct `role="heading"` text child. The heading supplies the window
title and a flexible spacer at its position among the buttons. Button accessible
names, images, visible text, disabled/inert state and existing press actions supply
standard `NSToolbarItem`s; AppKit owns their sizing, overflow and appearance.
An action with `toolbarPlacement="navigation"` uses AppKit's leading navigation
placement (`isNavigational`), so Back does not migrate into trailing actions.
All toolbar actions also appear in the native menu, even without a shortcut.
Customization is not enabled in this first slice.

The containing app must call `ExactView.attachWindowToolbar(to:)`; mounting an
embedded view never claims the containing window. An existing foreign toolbar is
not replaced. Detach, unmount and session reset release only Exact's toolbar;
route updates retain the toolbar and item identities. Unsupported or ambiguous
declarations retain their authored rendering. Web, iOS and Linux retain authored
layout. No CSS row changes meaning and no general NativeView loader is introduced.
The standalone Mac adapter opts in and uses unified compact window chrome.
With `viewport-fit=cover`, the actual window safe area includes the toolbar;
the app pads content with `env(safe-area-inset-top)`, not a fixed toolbar height.
Geometry callbacks caused by window chrome during presentation or reset are
coalesced until the incoming batch finishes, before queued input. Otherwise a
reset can update the new runner's insets before its older boot snapshot is drawn,
leaving native frames behind the kernel even though the inset value is correct.
Window-chrome declarations should be outside content flow; applications targeting
an embedder without attachment must provide an appropriate content layout.

The agent's existing `tap` reports `host-activation` through the native target/action;
`layout` identifies `NSToolbar`/`NSToolbarItem`, enabled state and overflow. Standard
items have no public frame API: native `space` is explicitly system-owned, while
the kernel `frame` remains authored fallback geometry. Held pointer injection at
that logical item is refused, not delivered to the fallback box. Window screenshots
capture the actual chrome. This is not a physical-click or manual VoiceOver claim.

### Touch panning directions

`touch-action` (style bit 86, initial `auto`) admits the keyword combinations
listed in `schema.json`. Web emits the CSS declaration unchanged. UIKit tests
the initial pan direction against the hit node's and ancestors' declarations,
through the scroll container, leaving permitted scrolling to `UIScrollView`.
The directional names describe scrolling: a leftward finger movement scrolls
right, so Messages uses `pan-right pan-y` on a replyable bubble. This mapping
was driven against Chrome touch input. Changing the row after recognition does
not change that gesture. Native pinch zoom is not added by this row.

## Video (LLP 1042, 2026-09-18)

`Video` is a replaced measured leaf alongside `Image`; native metadata uses the
same intrinsic-size seam. Without metadata its fallback is 300×150. The Contract
tag carries the browser video UA rule `object-fit: contain`; an explicit CSS row
wins. Playback, controls and the media clock belong to the browser or the optional
AVKit artifact. Keyboard layout uses the existing viewport policy, not media state.

Viewport dimensions (2026-10-04): dimension rows admit `vw`, `vh`, `vmin`,
`vmax`, `svw`, `svh`, `lvw`, `lvh`, `dvw`, `dvh`. The web retains the CSS
unit; native resolves it from the root layout offer and re-derives it on
resize. Native windows have no retractable browser chrome, so their small,
large and dynamic viewports coincide. Viewport units on scalar length rows
(font size, spacing, border widths) remain unsupported; their diagnostic
names the length forms the row accepts.

`cursor` (2026-10-04) admits CSS's predefined keyword set, inherited. Web emits
the keyword. macOS uses NSCursor; unavailable artwork (help/wait/progress,
diagonal resize, zoom before macOS 15) uses an arrow/crosshair stand-in. iOS,
tvOS and Linux ignore this presentation hint: their presenters expose no
CSS cursor executor. Cursor image URLs remain unsupported.

Font lists (2026-10-04) are CSS family lists, one to 64 members; a plan family
with no faces names an installed local family. Web uses the full CSS list;
Apple matches available families and gives CoreText the ordered cascade.
Linux matches the first available family, with its existing cosmic-text
glyph fallback. It logs `font-stack-fallback` for a multi-member list: an
authored per-glyph cascade needs a shaping API beyond cosmic-text's one
`Attrs.family`, and remains owed rather than silently claimed as CSS parity.

CSS form and text vocabulary (app diaries kanban F11, ledger F1/F4,
minesweeper F4, pomodoro F2): `rows` is HTML intrinsic line count and maxlength
limits user UTF-16 edits, leaving authored values intact. Native interactive
CSS resize handles are not implemented; Contract admits none and diagnoses
other CSS values. Portable user-select admits auto/none: macOS selection omits
none subtrees; iOS/Linux have no ordinary selectable text executor, so
text/all/contain are diagnosed at compile time. Border shorthands reset and
lower to existing side rows. Native border painters support none/hidden/solid;
other line styles are diagnosed. Text-decoration lowers solid currentcolor
underline/line-through to the existing line row; other color/style/thickness
components are diagnosed. Linux draws its UA lines from font size and glyph
advances; underline skip-ink:auto is not implemented and its driver log names
that deviation once. Web and Apple use their text systems' decoration metrics.

CSS flex shorthand admits grow/shrink factors and any admitted dimension basis,
with the factors adjacent as CSS requires. Intrinsic flex-basis keywords
content/min-content/max-content/fit-content are not represented by v1 dimension
rows; the compiler names their intrinsic sizing requirement and refuses them,
rather than describing valid CSS as a malformed shorthand.

Font-family's other CSS generics (cursive/fantasy/math/emoji/fangsong) have no
native mapping in the current catalog, and CSS-wide values are not represented
by plan stack ids. Contract diagnoses both limits; quoted names are local
families even when their spelling is a CSS generic.
