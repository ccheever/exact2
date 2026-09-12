# LLP 1011: Image v1 — a replaced element, as built

**Type:** Spec
**Status:** Review (llp-review, one round, 2026-08-29 at Charlie's request — `llp/reviews/1011-image-v1.{codex,grok}.md`, both NOT READY on r1; r2 folds them and is unreviewed. The code was reviewed the same day: `llp/reviews/code-2026-08-29-image.{codex,grok}.md`, folded.)
**Systems:** Kernel (measured leaves, intrinsic size, aspect ratio; Taffy patch 5), Contract (`image` tag), Web host, Apple host (C ABI: `exact_intrinsic`), Build (assets)
**Author:** Claude (Fable 5) for Charlie Cheever
**Date:** 2026-08-29
**Revised:** 2026-09-11 (scaled-image scrollable extent and replaced grid sizing; LLP 1035.004 symbol sources, native leaves, tint, units and verification; earlier r2 image decisions retained)
**Implementer:** Claude (Fable 5), image landing 2026-08-29; Codex, symbol integration 2026-09-10 and replaced-content extent 2026-09-11
**Related:** LLP 1001 §1 (the `Image` replaced-element rule and its declared block-flow deviation), §6 (measured leaves), LLP 1007 (the web host: `<img>`), LLP 1008 §5 (the Apple presenter: loading, `object-fit`), LLP 1010 (the sibling spec whose shape this follows), `vendor/taffy/EXACT-PATCHES.md` patch 5, `rules/RULES.md` §The web is the standard

## Summary

An `image` is CSS's **replaced element**: its box comes from the picture
unless the style rows say otherwise. The kernel had `Image` as a leaf
sized by rows alone; it now has one seam — **the host reports the image's
natural size once it has loaded** — and lays the node out from it as CSS
lays out `<img>`: an unknown axis measures 0 until it loads, then the
natural size, keeping its ratio when one dimension is set and when
min/max bind, unless an `aspect_ratio` row wins. On the web the browser
does all of this itself. On macOS the presenter loads and decodes the
source off the main thread, tells the kernel the size through the C ABI,
applies the frames that moved, and paints with CSS `object-fit` in the
content box. Where this document and the code disagree, the code and its
tests are the authority.

## 1. The kernel: a measured leaf with an intrinsic size

- **`Image`** (node type id 2) is a **measured leaf** — `NodeType::
  is_measured_leaf` is `is_text_leaf || Image` — so its Taffy node carries
  its slot as the measure context, as a text leaf's does. It holds no
  children (`can_hold_children` is false).
- **The intrinsic size** is a per-node column (`NodeArena::intrinsic`,
  `Option<(f32, f32)>`), `None` until the host reports one. **The unit
  is the bitmap's pixel counts, taken one-for-one as layout units** —
  never divided by a backing scale, so a 2× asset lays out (and paints) at
  half its pixels per point, as the web does without `srcset`. That is the
  contract every host implements: report `pixelsWide × pixelsHigh`, not a
  platform image's point size. **Configured symbols** (LLP 1035.004) instead
  report the native symbol's point size at its computed font size and weight;
  they are generated glyphs, not density-unaware bitmap assets. The browser's
  generic SVG fallback has a square intrinsic size equal to `font-size`.
- **`Kernel::set_intrinsic_size(view, Option<(w, h)>)`** sets it. It
  refuses an unknown view (`LayoutError::UnknownView`), a node that is not
  an `Image` (`NotAnImage`), and a size that is not finite and positive on
  both axes (`InvalidIntrinsicSize`); it does nothing when the value is
  unchanged, and otherwise re-derives the node's engine style and marks it
  dirty. The column is **host state, not plan state and not wire state**:
  no style row carries it, the export omits it, and a replay of the
  batches does not restore it — the host, which loaded the image, reports
  it again. `Kernel::rehydrate` clones the arena's columns, so a rehydrated
  kernel keeps it; the equality gate holds either way (its images are
  unreported on both sides, pinned by `kernel/tests/image.rs`).
- **The measure.** In the layout pass's measure closure, an `Image` with
  no intrinsic size measures `0×0` (each *unknown* axis is 0 — a `width`
  row still sizes the box, so the Caltrain logo is `96×0` before it
  loads); with one, each unknown dimension is the intrinsic one.
- **Scrollable extent** (Codex, 2026-09-11; patch 5 extension). The image's
  natural pixels size unknown axes; once its box is sized, replaced content
  cannot enlarge scrollable overflow. Taffy's existing replaced-item marker is
  now set for `Image`, and its leaf reports the used padding box as content.
  A 132×132 Tapback displayed at 32×32 previously enlarged a short reply's
  scroll extent and clipped its top when following the end. Source replacement,
  both scaling directions, padding/borders and all five `object-fit` values are
  covered by `kernel/tests/image.rs`; text retains its measured overflow.
  The same marker permits CSS's compressed automatic grid minimum for images.
  [CSS Overflow §2.1](https://www.w3.org/TR/css-overflow-3/#ink-overflow)
  distinguishes replaced-content ink overflow from scrollable overflow.
- **The ratio.** `taffy_style` gives an `Image` with an intrinsic size and
  no `aspect_ratio` row a Taffy `aspect_ratio` of `w / h`; a set row wins.
  That is what makes `width=96` yield `96×36` for a 320×120 picture and an
  `aspect_ratio: 1` row yield `96×96`.
- **Min/max keep the ratio** (Taffy patch 5, `vendor/taffy/EXACT-PATCHES.md`):
  a leaf with a ratio takes a tentative size — the set dimension and the
  other by ratio, or the natural size — and resolves it against
  `min_*`/`max_*` by CSS 2.1 §10.4's constraint table for replaced elements,
  never axis by axis: `max_width: 100` gives 100×37.5, `max_height: 40`
  gives 106.67×40, `width: 96; max_height: 20` gives 53.33×20. The ratio is
  also kept when a flex container measures the item's content-based flex
  basis at a stretched cross size (css-flexbox §9.2 rule B), so an
  auto-width image in a stretching flex column is 390×146.25 under a 390
  offer — what the web gives.
- **Declared deviation (LLP 1001 §1):** in *block* flow Taffy stretches an
  auto-width child to its container, so a bare `image` under a block
  parent lays out container-wide with its height by ratio (390×146 for
  320×120 under a 390 offer, on the point grid), where CSS would use the
  intrinsic width. Pinned by `kernel/tests/image.rs`.
- The rows an image uses: `width`/`height`/`min_*`/`max_*` (its box),
  `aspect_ratio` (the ratio, over the intrinsic one), `object_fit` (how
  the picture fills the content box: `fill | contain | cover | none |
  scale-down`, default `fill` — paint, not layout), `tint_color` (opaque
  black initially, not inherited; Apple and web apply it to `symbol:` images,
  including `light-dark()` pairs; raster-image tint remains unsupported).

## 2. Contract

`image "src" width=… height=… fit=… label=… testId=…`: the positional
argument is `imageSource`; `fit`/`objectFit` set `object_fit`; `label` is
`accessibilityLabel`. The tag also admits `hint`, `role`, and
`headingLevel` (the generic accessibility attrs) — §3/§4 say what each
host does with them. A relative source is resolved by the host against
the app's asset root (§3, §4). The Caltrain app: `image
"assets/caltrain.png" width=96 fit="contain" label="A Caltrain train"` at
the top of the header; `apps/caltrain/assets/caltrain.png` is a generated
320×120 PNG (a train), the only asset today.

**Symbol sources (2026-09-10, LLP 1035.004).** `image "symbol:back"
font-size=17 font-weight=600 tint-color="#007aff"` uses one of the seven
schema roles: back, close, compose, add, microphone, send and search. Literal
unknown roles are `lower-attr-value`; a dynamic unknown role paints empty,
clears intrinsic size and logs a refusal. Symbols are decorative: the surrounding
control carries its accessible name, the symbol does not carry another one.
No network or asset load resolves a symbol. Definite dimensions still size its
box; inherited font size/weight configure its natural glyph dimensions.

## 3. The web host

An `Image` is `<img>`; `imageSource` is `src`; `accessibilityLabel` is
**`alt`** (the replaced element's text alternative, shown by the browser
when the image does not load — on an image, never `aria-label`);
`accessibilityHint`/`Role`/`HeadingLevel` lower to `aria-description`/
`role`/`aria-level` as on any node; `object_fit` lowers by name to
`object-fit`; the page's stylesheet sets `img { display: block }` (a bare
`<img>` is inline, which the kernel has no notion of). The browser loads,
sizes, and paints — including its own broken-image presentation (the icon
and the `alt` text), which the kernel and macOS do not reproduce (§6). The
host never learns the intrinsic size and does not need to, since it never
lays out. `host/web/build.mjs` replaces `dist/assets/` with
`apps/<app>/assets/` on every build (a deleted asset does not linger), so
a relative source resolves against the page's URL, as `src` does; the
servers (`serve`, `dev`, `smoke`, `metrics`) know `image/png`.

For `symbol:` sources, the host supplies the schema's generic SVG path and
`alt=""`; the glue uses a transparent SVG for intrinsic size and a CSS mask
for the glyph. Computed font size/weight update the SVG; `tint-color` supplies
the mask's colour. The mask follows the content box and `object-fit`. These
paths express the roles without copying Apple's artwork.

## 4. The Apple host

- **The source policy** (`NodeView.resolveSource`): an `http`/`https`
  URL loads as is (what a page's `<img>` may load); a relative path
  resolves under the asset root — `EXACT_ASSETS`, the app's directory
  (`build.mjs --run` and the smoke set it; the current directory
  otherwise) — and must stay inside it after standardization (`..` that
  escapes does not load); `symbol:` resolves the generated role mapping
  locally (below); any other scheme (`file:` included) does not
  load. A source that does not load is reported as `nil` with a line on
  stderr.
- **Loading** (`NodeView.loadImage`): when an `image` node's
  `imageSource` prop is set or changes, the presenter bumps the view's
  load generation and loads on a background queue: `Data(contentsOf:)`
  (the platform's default timeout for URLs; no size cap — the web has
  none either), then **decodes there** (`CGImageSource` with
  `kCGImageSourceShouldCacheImmediately`), so `draw` never decodes on the
  main thread. On the main queue the completion is dropped unless it is
  the view's current generation *and* the view is still the presenter's
  view for that id (a destroyed or reused id never receives a stale
  size); otherwise it keeps the bitmap as an `NSImage`, repaints, and
  reports the pixel counts through the presenter's `intrinsic` callback.
  While a new source loads the old picture and its size stay, as a
  browser keeps showing the old `src`; a failed load clears the picture
  and reports `nil`; a cleared source clears both. `destroy` and `reset`
  call `forget()` (generation bumped, source and picture dropped), and
  `reset` clears the smoke's `imagesLoaded` list.
- **Symbols** use a noninteractive, decorative `UIImageView` / `NSImageView`
  inside the existing kernel-owned image leaf. Native symbol configuration uses
  computed font size and the nearest of nine CSS weights; native tint updates
  with appearance. Intrinsic reports run on the next main-queue turn and require
  the original view identity and load generation, so a replaced source or retired
  node cannot receive them. `object-fit`, padding and rounded clipping apply to
  the content box; symbols bypass the bitmap drawing path. `layout <node>` reports
  `native.symbol` with renderer class, generated name, intrinsic size and frame.
- **The ABI** (`exact_intrinsic(view, width, height)`, `exact.h`): a
  finite size with either dimension ≤ 0 clears; a non-finite value reaches
  the kernel and comes back as an `error` (`InvalidIntrinsicSize`), as does
  a non-image view (`NotAnImage`) or an unknown one. Otherwise the host
  calls `Kernel::set_intrinsic_size`, lays out again, and returns a batch
  with every frame that moved — the image's and everything its size
  pushed. The same size twice returns an empty batch.
- **Painting** (`NodeView.draw`): the picture is drawn in the node's
  **content box** — the frame inset by the border widths and padding —
  by `object_fit` from the style dictionary: `fill` stretches to the
  content box, `contain` and `cover` scale by the smaller/larger ratio,
  `none` is the natural size, `scale-down` the smaller of `none` and
  `contain`, and any other value is the initial `fill`; centered, clipped
  to the content box and to the border box's rounded path (`border_radius`
  clips the picture as on the web).
- **Accessibility:** the label becomes the view's accessibility label;
  `hint`/`role`/`headingLevel` are not applied to images (nor to any node
  yet) — declared in §6.
- **Reported** in smoke mode: `images: <source> WxH…; first image frame
  WxH` (the lowest image id).

## 5. Held by

Asserted, under the five checks: `kernel/tests/image.rs` (8): the measure
is 0×0 until a size is reported, then the intrinsic size, then 0×0 when
forgotten; one dimension set gives the other by ratio; a set ratio row
wins; an unknown view, a non-image, and a non-finite or non-positive size
are refused and store nothing; the §10.4 table (height only, both set,
`max_width`, `max_height`, `min_width`, width with `max_height`, both
maxima); the stretching flex column (390×146.25); the block-flow deviation
(390×146); a reported size survives `rehydrate` and not a replay.
`kernel/src/node.rs`: `Image` is a measured, non-text leaf that holds no
children. `host/web/tests/host.rs`: the header image's create op is an
`<img>` with `src`, `alt` (and no `aria-label`), `object-fit`, and width.
`host/apple/tests/host.rs`: before the report the image's frame is 96×0;
after `set_intrinsic(320×120)` it is 96×36 and the station name below it
got a frame; the same size again moves nothing; `None` makes it 96×0
again; a non-image and an infinite size come back as errors with no
frames.

Observed, not blocking (both smokes say so in their first lines; they need
Chrome / a window server): `host/apple/smoke.mjs` greps the app's report
for `caltrain.png 320x120` and `first image frame 96x36` — a real
`loadImage` through the ABI, not a painted pixel. No harness exercises
`draw`, the fit values other than `contain`, padding/border/radius on an
image, a changed source, or the broken-image presentation; the author
viewed both hosts' screenshots on 2026-08-29 and found the train in the
same place — an observation, not a check.

**Symbol verification (2026-09-10):** the compiler corpus and Apple/web host
tests cover literal refusal, generated mappings, inherited font configuration,
decorative images and scheme-aware tint. Temporary iOS/macOS/browser drives
cover natural/fixed sizing, changed size/weight/source, unknown/cleared/restored
sources and a symbol button. The iOS native-leaf prototype matches 210
`UIImageView` raster comparisons. Production Messages passes public-XCTest
button taps in light/dark; Fieldnotes uses Add and Close through the same
boundary. AppKit geometry and interaction pass, but its saved captures are
transparent, so AppKit pixels remain unverified. Receipt and limits:
`/tmp/messages-symbol-integration/verification.json`.

## 6. Not in v1 (each declared here)

`srcset`/density selection and `image-rendering`; raster-image `tint_color`
(symbol tint is implemented on Apple/web); Linux symbols; loading states and errors visible to the app (the
kernel measures an unknown axis as 0; macOS paints nothing and writes a
line on stderr; the browser paints its own broken-image icon and the
`alt` text — no `onError`, no placeholder); a size cap or a timeout of
our own on macOS loads, and cancellation (a plan reload re-creates the
node and loads again; the in-flight `Data(contentsOf:)` runs to completion
and is then dropped by the generation check); remote images on macOS
beyond that blocking fetch on a background queue (no `URLSession`, no
headers, no cache); a block-flow image at its intrinsic width (the
declared deviation, §1); `object-position`; animated images on macOS (the
first frame paints); `hint`/`role`/`headingLevel` on macOS images; pixel
or screenshot assertions for `object-fit` (§5).
