# LLP 1011: Image v1 — a replaced element, as built

**Type:** Spec
**Status:** Review (llp-review, one round, 2026-08-29 at Charlie's request — `llp/reviews/1011-image-v1.{codex,grok}.md`, both NOT READY on r1; r2 folds them and is unreviewed. The code was reviewed the same day: `llp/reviews/code-2026-08-29-image.{codex,grok}.md`, folded.)
**Systems:** Kernel (measured leaves, intrinsic size, aspect ratio; Taffy patch 5), Contract (`image` tag), Web host, Apple host (C ABI: `exact_intrinsic`), Build (assets)
**Author:** Claude (Fable 5) for Charlie Cheever
**Date:** 2026-08-29
**Revised:** 2026-10-06 (`load`/`error` on the web, macOS and iOS; the remote fetch policy, as built — #121); 2026-09-26 (raster-image `tint-color` on Apple and web); 2026-09-11 (scaled-image scrollable extent and replaced grid sizing; LLP 1035.004 symbol sources, native leaves, tint, units and verification; earlier r2 image decisions retained)
**Implementer:** Claude (Fable 5), image landing 2026-08-29; Codex, symbol integration 2026-09-10 and replaced-content extent 2026-09-11
**Related:** LLP 1001 §1 (the `Image` replaced-element rule and its declared block-flow deviation), §6 (measured leaves), LLP 1007 (the web host: `<img>`), LLP 1008 §5 (the Apple presenter: loading, `object-fit`), LLP 1010 (the sibling spec whose shape this follows), `vendor/taffy/EXACT-PATCHES.md` patch 5, `rules/RULES.md` §The web is the standard


> **Spelling (2026-10-06):** `tint-color` is spelled `-exact-tint-color` since [LLP 1081](1081-names-exact-invents.rfc.md). This document keeps the spelling it was written with, as the record.

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
  scale-down`, default `fill` — paint, not layout), `tint_color` (initially
  `AccentColor`, the platform's dynamic tint, since LLP 1095 stage 2; not inherited; Apple and web apply it to `symbol:` images
  and, since 2026-09-26, to raster images, including `light-dark()` pairs).
  **A tinted raster is a template:** its alpha is the mask, and every
  opaque pixel takes the tint at that pixel's own alpha; the colours of
  the bitmap are ignored. Without the row a raster paints its own pixels.
  `image "assets/wordmark.png" width=120 height=40 object-fit="contain"
  tint-color="light-dark(#000000, #ffffff)"` is a black-on-transparent
  wordmark that turns white in dark mode, live.

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

**App files and `data:` URLs (2026-10-04, recipes F9/F18, gallery F7/F17).**
An `app:/data|cache|tmp/…` source is the app's own file (LLP 1069.002 D7) on
every host, from the first frame and with nothing picked: the web hosts show
the page's file store's entry as an object URL (`picker-glue.js` `appURL`),
Apple and Linux the file under the roots storage will configure, known at
boot. A `data:` source (RFC 2397, base64 or percent-encoded) is admitted on
every host, as a page's `<img>` takes one, up to **1 MiB of URL text**
(`exact_raster::MAX_DATA_URL_BYTES`; Apple's `RasterInput.dataLimit`, the web
hosts' `DATA_LIMIT`): small generated pictures, never a photo, which is an
`app:/` file. Past the bound every host shows nothing (the web hosts and Apple
journal `image refused: a data: source is over 1048576 bytes`; Linux refuses
it as an undecodable source); the browser itself has no such bound,
which is the declared deviation. One source string works on every host, so a
data module never tells hosts apart to choose one.

**`load` and `error` (2026-10-06, #121).** An `image` hears HTML `<img>`'s
two events on the web, macOS and iOS (Linux sends neither yet), each once
per source: `load=` when the picture is ready, with no payload (`iframe`'s `load` is the same event; the compiler admits `load` on
those two tags and on a native module), and `error=` when the source does not
load, with the error's `message`, as a media element's `error` carries one.
The message is the host's reason where it has one (§4) and a fixed text where
the browser gives none (§3). A symbol fires neither: no load resolves it. A
`data:` URL over its bound is an `error` on Apple (once, though each props
op asks for it again) and on the wasm target (whose glue sets an empty
`src`); the JS target refuses only a bound one (it removes the `src` and
fires nothing), and a literal one is the browser's to fail. Natively one bitmap can be decoded again later at another
size; that is not a second `load`.

**Symbol sources (2026-09-10, LLP 1035.004).** `image "symbol:back"
font-size=17 font-weight=600 tint-color="#007aff"` uses one of the seven
schema roles: back, close, compose, add, microphone, send and search. Literal
unknown roles are `lower-attr-value`; a dynamic unknown role paints empty,
clears intrinsic size and logs a refusal. Symbols are decorative: the surrounding
control carries its accessible name, the symbol does not carry another one.
Since 2026-10-04 a symbol its author names (`alt` or `aria-label`) keeps that
name on the web and macOS (LLP 1035.004 D1, amended); iOS still hides every
symbol from VoiceOver (`NodeViewIOS.swift`), and Linux exposes no image names.
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
`alt=""`, or the author's name when it has one (`accessibilityLabel`, written as `alt`); the glue uses a transparent SVG for intrinsic size and a CSS mask
for the glyph. Computed font size/weight update the SVG; `tint-color` supplies
the mask's colour. The mask follows the content box and `object-fit`. These
paths express the roles without copying Apple's artwork.

**Tinted rasters (2026-09-26).** `host_css` (`host/web/src/element.rs`) gives
an `<img>` whose source is not `symbol:` and whose `tint_color` is set the
tint as `background-color: var(--exact-tint)` (so `light-dark()` follows the
page's scheme live), masked by `mask-image: url(<src>)` in the content box
(`mask-origin`/`mask-clip: content-box`, centred), with `mask-size` from
`object-fit`: `fill` → `100% 100%`, `contain`/`cover` → the same keyword,
`none` → `auto`. `scale-down` needs the natural size, which only the page
knows: the glue (`tintFit`) sets `--exact-tint-fit` to `auto` once the image
has loaded and fits the content box, else `contain`. The `<img>` still loads
`src` (its `alt`, load event and broken state are unchanged); its own
picture moves out of the content box (`object-position`), where the replaced
element's clip hides it. Deviations: the mask covers the whole element, so a
tinted image's own background, border and shadow are masked to the picture
(wrap it to give it a box); a cross-origin `mask-image` is a CORS request, so
a remote tinted image whose origin sends no CORS headers paints nothing, and
may be fetched a second time. **Retained after review (2026-09-27):** the
box-paint case remains the declared deviation in LLP 1001 §1. A loaded
`<img>` does not generate `::before` or `::after` boxes; putting the mask on
one cannot isolate the picture. A host wrapper would become the sizing and
flex/grid item instead of the replaced element. No CSS-only replacement
preserving those semantics was found; no wrapper or JS sizing is added.
An author can put background, border, padding and shadow on a surrounding
`view` and tint its child image. The reference pixels in
`scripts/fixtures/tint.web.png` and `tint.web-dark.png` remain the target for
paint on the image itself, not passing web-host snapshots. Investigation and
verification: `issues/closed/20260927-web-tint-masks-the-box.md`.

**Events and the fetch (2026-10-06, #121).** Both web targets (`host/web/glue.js`
`imageEvents`, `host/web-js/rt.js` `imageEvent`) pass the `<img>` element's own
`load` and `error`; the browser gives an error no reason, so its `message` is
`the image did not load`. A tinted raster from another origin paints through
its mask, a CORS fetch, which an origin that sends no
`Access-Control-Allow-Origin` refuses while the `<img>` itself loads: before
its `load`, such an image is fetched once more with `crossOrigin="anonymous"`,
and if that is refused the event is `error`, `a tinted image from another
origin needs CORS (Access-Control-Allow-Origin)`, not `load` — a declared
deviation from `<img>`, which fired `load`, so that the app hears that nothing
painted. The probe runs once per URL, when the image loads: a `tint-color`
added later, or a same-origin source that redirects to another origin
(`currentSrc` is the URL asked for), is not probed. An adopted page's image
that settled before the runtime attached is read from `complete` and
`naturalWidth`. The fetch is the
browser's `<img>`, with no Exact policy over it: credentials and `Referer` by
the page's rules (Chrome, 2026-10-06, sent the page's origin as `Referer` and a
cookie the image origin had set, it being same-site), redirects followed, the
type sniffed from the bytes whatever the `Content-Type`, no size cap, the
browser's HTTP cache; SVG draws.

**Linux raster tint (2026-09-27).** The ordinary tree walk and the retained
content-region picture carry an optional, appearance-resolved tint to both
backends, including motion's presented tint. tiny-skia and Vello draw the
picture in an isolated layer, then fill the tint source-in. The decoded
bitmap stays shared and unchanged; no tinted asset enters the image cache.

## 4. The Apple host

- **The source policy** (`NodeView.resolveSource`): an `http`/`https`
  URL loads as is (what a page's `<img>` may load); a relative path
  resolves under the asset root — `EXACT_ASSETS`, the app's directory
  (`build.mjs --run` and the smoke set it; the current directory
  otherwise) — and must stay inside it after standardization (`..` that
  escapes does not load); `symbol:<role>` resolves the generated role mapping
  locally (below); `symbol:sf/<name>` passes an opaque, possibly empty name
  directly to the running Apple OS (LLP 1035.004.000, Codex 2026-10-02).
  An unresolved symbol paints nothing and supplies a one-font-size square
  as fallback intrinsic size; raw SF sources are also empty on web/Linux,
  never fetched. `layout` reports current resolution and its reason; missing
  raw names produce no warning. Symbol/raster changes clear old image state;
  `app:/` resolves under `AppFiles`' roots, learned from the library at boot
  (`appRoots`); a `data:` URL within its bound is decoded and spooled to a
  temporary file (`RasterInput.dataURL`); any other scheme (`file:` included)
  does not load. A source that does not load is reported as `nil` with a line on
  stderr.
- **Loading** (`NodeView.loadImage`): when an `image` node's
  `imageSource` prop is set or changes, the presenter bumps the view's
  load generation and loads through the session's raster pipeline
  (`RasterLoader`; a remote source is fetched as the next bullet says),
  then **decodes off the main thread** (`CGImageSource` with
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
- **The remote fetch** (`RasterInput.swift` `RasterDownload`, macOS and
  iOS alike), as built (2026-10-06, #121): a `URLSession` with no cookie
  storage and `httpShouldSetCookies` off (no `Cookie` sent, no `Set-Cookie`
  kept), no credential storage, and no `Referer`; its `User-Agent` is
  CFNetwork's default (`ExactMac (unknown version) CFNetwork/… Darwin/…` from
  a development build). Redirects are followed as `URLSession` follows them,
  to any host, within App Transport Security: the build declares no ATS
  exception but iOS's `localNetworking` (`NSAllowsLocalNetworking`), so a
  cleartext `http:` source or redirect to a non-local host is ATS's to
  refuse, and that refusal is the `error` (not driven here; the repro used
  `127.0.0.1`). Any 2xx response is taken
  whatever its `Content-Type` (ImageIO reads the bytes); any other status is
  the `error` `HTTP <status>`. The body is capped at
  `RasterMetadata.encodedLimit` (64 MiB) from `Content-Length` and again as
  it streams; the request times out at 15 s and the whole load at 30 s. The
  body spools to a temporary file read once for its header (256 KiB) and
  once to decode. Responses are cached on disk only (`URLCache`, 64 MiB,
  `Caches/exact-raster-http`, the HTTP cache headers' policy); decoded
  pixels are the session's raster budget. ImageIO decodes rasters only: an
  SVG, remote or under `assets/`, does not draw, and is the `error` `not an
  image format this host decodes`, as is any whole body ImageIO cannot size.
  Linux loads no remote source.
- **Events:** the loader sends a node that hears it the source's `load`
  when its first decode lands, or its `error` with the reason (the HTTP
  status, `URLError`'s description, the raster refusal), each once per
  source and on the next main turn, after the turn's sizes are reported
  (so `clock settle`, which waits for the decodes, can return just before
  the action runs; a drive reads them after `clock +<ms> real`). A decode
  that a resize retries after a refusal and that then lands sends no
  `load`. A decode ImageIO or Core Graphics declines over bytes whose size
  it read (it does on a busy system) is asked for again after 250, 500 and
  1,000 ms, loading all the while, before it is the `error` `decode failed`
  (`RasterLoader.declineDelays`): a page's `<img>` errs for its file, never
  for a busy machine.
- **Symbols** use a noninteractive, decorative `UIImageView` / `NSImageView`
  inside the existing kernel-owned image leaf. Native symbol configuration uses
  computed font size and the nearest of nine CSS weights; native tint updates
  with appearance. Intrinsic reports run on the next main-queue turn and require
  the original view identity and load generation, so a replaced source or retired
  node cannot receive them. `object-fit`, padding and rounded clipping apply to
  the content box; symbols bypass the bitmap drawing path. `layout <node>` reports
  `native.symbol` with renderer class, source, resolved name, `found`, missing `reason`, intrinsic
  size and frame.
- **Tinted rasters** (2026-09-26) use the one decoded bitmap the raster
  pipeline holds — no second decode, no tinted copy. They paint through
  `draw(_:)` on both platforms: `RasterGeometry.draw` draws the bitmap into
  a transparency layer and fills the tint source-in, so the tint keeps each
  pixel's alpha. On iOS this gives up the untinted image's sublayer
  (`applyImageLayer`) and costs a backing store of the view's size: a mask
  layer would say it without one, but a canvas's capture (`render(in:)`,
  LLP 1014) drops masks and showed the tint's whole rectangle.
  (`clip(to:mask:)` was also measured wrong for an image with alpha.) The
  colour is resolved at paint, so an appearance change (which restyles and
  redisplays the view) re-resolves a `light-dark()` pair.
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

**Raster tint (2026-09-26):** `host/web/tests/it/host.rs`
(`a_tinted_raster_is_its_alpha_masking_the_tint`: the mask declarations per
fit, an untinted raster and a tinted symbol without them, the mask following
a changed fit); `RasterImageTests.testATintDrawsTheBitmapsAlphaInTheTint`
(the draw path's pixels at full and half alpha); `BoxLayerIOSTests.
testATintedImageIsItsAlphaInTheTint` (the view's painted pixels: the
opaque half in the tint, the clear half clear, a live light→dark change
re-tinting; removing the row restores the pixel sublayer). Observed, not
asserted: a temporary Caltrain fixture (all five fits, a padded rounded
image, light and dark) on headless Chrome and the iOS Simulator, where
Caltrain's content is a canvas capture. macOS shares the draw path and its
unit test; its window was not driven.

## 6. Not in v1 (each declared here)

**2026-09-14 scope change:** bounded raster loading/cache memory is now
assigned to Codex under LLP 1010 §6.3, after the windowed-list lifetime
slice, with Messages as the consumer. The initial target is 32 MiB per
session including Exact-owned pinned bitmaps and in-flight reservations;
the browser retains its own decoder/cache policy. This is planned, not
implemented: the current Apple loader below still has no size cap or
cancellation. Remote-image API expansion, `srcset` and loading-state
authoring remain outside that slice.

`srcset`/density selection and `image-rendering`; `tint_color` on Linux
symbols (rasters are tinted since 2026-09-27); Linux symbols; a placeholder
of the host's own while an image loads or after it fails (the kernel measures
an unknown axis as 0; macOS paints nothing; the browser paints its own
broken-image icon and the `alt` text — the app draws its own fallback from
`load` and `error`, §2); SVG sources on Apple (§4); a fetch policy the app
sets — HTML's `referrerpolicy`, a refusal of redirects (`redirect="error"`),
a byte cap below the host's (`max-bytes`) — each a vocabulary proposal
(#121), not built; Linux remote sources; a block-flow image at its intrinsic
width (the
declared deviation, §1); `object-position`; animated images on Linux (it
decodes PNG only; Apple plays GIF and WebP, LLP 1011.000); `hint`/`role`/`headingLevel` on macOS images; pixel
or screenshot assertions for `object-fit` (§5).
