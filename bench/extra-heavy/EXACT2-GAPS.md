# What exact2 needs before the Extra Heavy feed

Date: 2026-09-27. Audited against **origin/main `c74615a3`**, read-only. Evidence is `path:line`
at that commit, taken from `git show origin/main:…` and `git grep … origin/main`. A kind counts as
built only when code proves it, through a test, an app in `apps/`, or a host implementation. An LLP
saying a stage is built is not enough. Nothing here was run; these are reading verdicts. The lane
that builds the app should confirm each "yes" by driving it (`scripts/agent.mjs`).

A record (2026-09-27 to 09-30), kept as written. Its `results/…` paths name the original harness's raw
results, which are not in the repository; the summaries it quotes are the evidence that remains.

Row details are in `SPEC.md` (written alongside this audit; the audit was re-read against it:
the feed's SVGs use paths, basic shapes and linear gradients only — no `<image>`, no text — and the
web-view page is an HTML string, which makes `srcdoc` matter).

The feed is a vertical `list virtualized=true` with 3,000 rows in 19 kinds (17 until 2026-09-27,
when kinds 18 `filmstrip` and 19 `inbox` added a nested virtualized list to one row in four). The shape the
feed must take follows from how lists compile:

- **One template per list.** A virtualized list takes exactly one keyed `each` whose body is one
  element (`contract/lower/src/collection.rs:36-46`). The 17 kinds are therefore one `column` holding
  a `match`/`when` per kind, as `../heavy-list/exact/app.contract:92-151` does.
- **No nested virtualized list.** One is refused inside another list's row
  (`contract/lower/src/collection.rs:72-76`). Kinds 18 and 19 are exactly that (gap 2).
- **The container cannot be horizontal.** A virtualized list container refuses `flex-direction`,
  `flex-wrap` and grid templates (`collection.rs:127-131`).
- **The iOS pool reuses only plain rows.** It reuses `view`, `text`, `image`, `button`, `scroll` and
  `svg` rows, 8 per shape and 32 in total (`host/apple/Sources/ExactKit/IOS/NodePoolIOS.swift:59-60`).
  It refuses any row that holds any of these (`NodePoolIOS.swift:152-162`):
  - a live `UIScrollView`;
  - a field or text area;
  - video;
  - a web view;
  - Metal;
  - canvas input;
  - a material;
  - a placement;
  - gesture recognizers;
  - focus.

  So at least 9 of the 17 kinds are rebuilt from scratch every time they scroll in.

## Verdict per kind

| # | Kind | Natural at origin/main? | Blocking gap |
|---|---|---|---|
| 1 | photo | **yes** | none |
| 2 | thumbs | **yes** (grid through flex-wrap) | `grid-template-columns` has no Contract name |
| 3 | shader | **partly** | no worked example of an image-sampling surface; Metal rows not pooled |
| 4 | canvas | **partly** | `fillText`/`drawImage` are LLP 1056 stage 2 (unbuilt) |
| 5 | svg | **yes** (inline `svg`) | the feed's SVGs need nothing unbuilt; no `image src="*.svg"` path was found, so the port inlines `svgs.json`; SVG `<image>` (stage 10a) matters only as a shader fallback |
| 6 | video | **yes**, not pooled | video rows rebuilt per scroll-in on iOS |
| 7 | map | **yes** as an app module, not pooled | native views not pooled; Linux has no map |
| 8 | markdown | **yes** | none on web/Apple; Linux has no `markup` |
| 9 | code | **partly** | no `white-space: pre` (LLP 1053 G5 deferred it) |
| 10 | intl | **partly** | no emoji/RTL/CJK host tests; Apple ignores `lang`; an iOS mixed-script clipping bug is open |
| 11 | typeface | **yes** | none (TTF/OTF only) |
| 12 | carousel | **partly** | inner horizontal list cannot be virtualized; the row is not pooled |
| 13 | motion | **no** (native) | animated GIF/WebP paint their first frame; Lottie is in `DEFERRED.md` |
| 14 | glass | **partly** | `backdrop-filter` has no Contract name, only host-policy `backgroundMaterial`; not pooled |
| 15 | live | **yes** | iOS samples translate/colour keyframes in the engine every frame |
| 16 | thread | **partly** | height-to-`auto` is Apple-only and untested with `line-clamp`; inputs not pooled; draft state must live in the parent |
| 17 | webview | **partly** | the SPEC loads an HTML string: no `srcdoc` attribute (the fallback is one bundled page per row); web views not pooled |
| 18 | filmstrip | **no** | a virtualized list inside a list row is refused, and a virtualized list cannot be horizontal (gap 2); the eager fallback is 2,000 laid-out items per row, and the row is never pooled |
| 19 | inbox | **no** | a virtualized list inside a list row is refused (gap 2); no offset survives the row's eviction; iOS does not honour `overscroll-behavior` |

### 1. photo: yes
- **Avatar.** A circle avatar is `image … border-radius=20 object-fit="cover"`; the heavy port does exactly
  this in a list row (`../heavy-list/exact/app.contract:103`). The pattern is in-repo at
  `apps/realworld/app.contract:358`.
- **Downsampled decode (Apple).** Apple decodes large sources downsampled through
  `CGImageSourceCreateThumbnailAtIndex` with `kCGImageSourceThumbnailMaxPixelSize`
  (`host/apple/Sources/ExactKit/RasterImage.swift:170-175`). A test checks it
  (`host/apple/tests/ExactKitTests/RasterImageTests.swift:62-69`).
- **Relative time.** `every(1000, …)` plus a Contract `fn ago` gives "3m ago"
  (`../heavy-list/exact/app.contract:53,82-83,105`). `now()` is time since launch. Wall-clock time
  comes from the `exactTime` source (`runner/src/time.rs:1-14`, tested in
  `contract/cli/tests/it/time.rs:25,54`), though no app uses it yet. There is no built-in
  relative-time formatter: LLP 1054.000.003 (Draft).

### 2. thumbs: yes
- **Shimmer bar.** The bar gets `background-image: linear-gradient(...)`. LLP 1066 is Implemented on web, iOS,
  macOS and Linux (`contract/lower/src/tags.rs:677`, `kernel/tables/schema.json:2610` bit 144).
- **Sweep.** `background-position`/`-size` do not exist, so the sweep is a gradient child
  moved by an infinite `translate` `@keyframes` inside an `overflow="hidden"` rounded bar. Keyframes
  on boxes are built (`motion/src/property.rs:34-120`; test `host/apple/tests/it/animation.rs:48-103`).
- **The 1.2 s gate.** It needs no timer: the skeleton plays `animation: fade-out … 1200ms forwards` and each
  image plays `fade-in … 1200ms both`. An animation starts when its node mounts, which is when the
  row appears. There is no `load` event on `image` (only iframes send `load`: `host/web/glue.js:461-466`).
- **Grid.** `display="grid"` exists but `grid-template-columns` has no Contract name. Only the list-container
  refusal mentions it (`collection.rs:127`). A 4×3 grid of 70 pt squares is `flex-wrap` with fixed widths
  and `gap`.
- **Image corners.** `border-radius` with `object-fit="cover"` on images is built (`apps/interaction-gallery/app.contract:345,409`).

### 3. shader: partly
- **Continuous frames.** A GPU surface (`canvas surface=name(args)`, LLP 1009, `tags.rs` `AttrTarget::Surface`) renders
  continuous frames when `render` asks for another (`gpu/src/lib.rs:214-223`); the Caltrain sky does
  this every frame (`apps/caltrain/app.contract:158-161`).
- **Two routes to the photo as a texture.**
  - An `image` placed as a canvas child arrives as the children texture (`gpu/src/lib.rs:282-291`, LLP 1014/1014.000).
  - A module can request asset bytes (`gpu/src/lib.rs:188-201`), which only `game/render/src/surface.rs:292,332` uses.
- **No worked example.** No example decodes a photo into a texture, so the wave plus duotone surface is new app code (a gpu crate with WGSL).
- **Off-screen rendering is fixed on main.** Canvases are gated to on-screen rendering on iOS (`host/apple/Sources/ExactKit/IOS/GpuIOS.swift:461,500`; cryptobench GAPS §1 landed).
- **Surface churn is not.** Metal rows are never pooled (`NodePoolIOS.swift:155`), so every scroll-in creates a surface. cryptobench GAPS §2 measured memory
  climbing to 824 MB at 48k–96k pt/s. Nothing owns this yet: no LLP.

### 4. canvas: partly
- **Built: LLP 1056 stage 1** (Accepted), on web, Apple and Linux, with gallery parity against Chrome
  (`apps/canvas-gallery/app.contract:60-80`). Bezier, arc and linear-gradient calls are all there:
  `canvas/src/context.rs:489,525,600,940`.
- **Not built: text and `drawImage`.** `fillText`/`measureText` and `drawImage` are stage 2 (`llp/1056-canvas-2d.rfc.md:603`).
  `canvas/` has no `fill_text` or `draw_image`.
- **Pooling.** Pooling 2D canvas rows on iOS is stage 3 (`:604`).
- **Off-screen drawing.** Each host draws frames whether or not the canvas is on screen (QUEUE.md:5, item 4).

### 5. svg: yes (inline markup)
- **What the feed uses.** Icons (paths, `fill-rule`), charts (`line`, `rect rx`, `polyline`) and illustrations
  (`polygon`, `circle`, `linearGradient`) — all in built stages. The SPEC's source is SVG *files*; exact2 draws inline
  `svg`, and no code decodes an SVG file as an `image` source on Apple (ImageIO cannot), so the port inlines the
  markup from `data/svgs.json` (a mechanical conversion, not a gap in rendering).
- **Built: SVG stages 1–9, 10b and 10c** (`llp/1055.000-completing-svg.rfc.md:504-515`). That covers paths, gradients,
  `use`/`symbol`, clip and mask, filters and markers.
- **Pooled on iOS.** `svg` rows are pooled (`NodePoolIOS.swift:60`).
- **Not built: `image` inside `svg`.** It is refused: "not in exact2's SVG yet: LLP 1055.000 §4 builds it in a later
  stage" (`contract/lower/src/svg.rs:339-340`), which is stage 10a.
- **Text in SVG.** Text inside `svg` is also refused (`svg.rs:336-337`, LLP 1055 D12). An illustration that labels itself needs a `text` placed beside the `svg`.

### 6. video: yes, not pooled
- **Playback.** `video` (`tags.rs:157`) with autoplay, muted, loop and playsinline plays a bundled MP4
  (`apps/video-player/app.contract:26`).
- **Cover.** `cover` maps to `.resizeAspectFill` (`host/apple/videoarm/VideoArm.swift:304`).
- **Rounded corners.** The radius clip is `VideoModule.swift:95-99`.
- **Pausing out of view.** `playbackVisibilityThreshold` can pause a video out of view, but only with an authored `paused`
  binding (`llp/1042-video.spec.md:91-121`).
- **Gaps.**
  - LLP 1042 is Draft.
  - No app puts a video in a list.
  - Video rows are not pooled (`NodePoolIOS.swift:155`).
  - Linux has no decoder (`llp/1042-video.spec.md:194`).

### 7. map: yes as an app-owned native module, not pooled
- **The module.** `apps/map-demo` wraps `MKMapView` (`apps/map-demo/modules/apple/NativeMap.swift:9,37`). It is written
  `native-map lat=… pins=…` (`apps/map-demo/app.contract:21`) and lowers to `NativeView` with
  `nativeViewModuleName` (`contract/lower/src/native.rs:48,121`). LLP 1024 is Accepted with §9 built.
- **Disabling interaction.** This is the module's own Swift: the app writes it.
- **Gaps.**
  - `NativeView` is not in the pool's kinds (`NodePoolIOS.swift:60`), so each map row builds an `MKMapView`.
  - No app has put one in a list.
  - Linux shows the module as unavailable.

### 8. markdown: yes
- **The `markup` prop.** `text body markup="markdown"` is a plain prop (`tags.rs:449-451`, schema prop 111), used on
  ordinary text in `apps/realworld/app.contract:944` and `apps/markdown-stress/app.contract:411-413`.
- **Coverage.** The Apple styler covers headings, bullets, quotes, code, links, bold, italic, mono and strike
  (`host/apple/src/markup.rs:1-40`).
- **Draft status and Linux.** LLP 1045 is Draft, and Linux has no markup code.
- **Alternative.** Hand-built blocks are possible too: nested `text` makes inline runs (`contract/lower/src/lib.rs:901-910`).

### 9. code: partly
- **Built.** Coloured runs are nested `text` children, one per token. A bundled monospace font is a declared `font` (kind 11).
- **Missing: `white-space: pre`.** `WhiteSpace` is only `normal`, `pre-wrap`, `nowrap` and `pre-line` (`schema.json:1349-1357`).
  LLP 1053 says "`pre` and `break-spaces` can wait" (`llp/1053-…rfc.md:108`).
- **Fallback.** Write one `text white-space="pre-wrap"` per source line, give the box a fixed wide width, and clip it with
  `overflow="hidden"`. `nowrap` collapses whitespace, which would destroy indentation.

### 10. intl: partly
- **Direction.** `direction` is a CSS row (`tags.rs:767-768`). Apple sets the RTL base direction and CoreText does
  bidi, shaping and font fallback (`host/apple/Sources/ExactKit/Text.swift:629-637`). The web passes CSS through
  (`host/web/glue.js:944`).
- **Gaps.**
  - Apple and Linux take `ltr` paragraphs' direction from the first strong character (QUEUE.md:51).
  - `lang` is read only on the web (`host/web/src/element.rs:495`).
  - The Linux text-flow walker cannot break inside an RTL run, and Linux RTL is on the to-do list (QUEUE.md:517).
  - "The last line of a mixed-script paragraph (Arabic via font fallback) clipped on iOS" is listed as in flight on
    `fix/ios-text-render` (`llp/1053-…rfc.md:72-76`). I found no commit on main that names the fix.
  - No host test covers emoji ZWJ, flags, CJK or Arabic shaping.
- **No Devanagari entry in QUEUE.** No `QUEUE.md` line at `c74615a3` mentions Devanagari. It appears only in
  `textflow/tests/corpus/breaks.txt:7,29`. The row must be verified by screenshot on each host.

### 11. typeface: yes
- **Declaring fonts.** A face is declared as `font "Bebas Neue" = "assets/BebasNeue-Regular.ttf"` (`apps/typetour/app.contract:15-23`;
  one-file form `contract/syntax/tests/it/fmt.rs:101`).
- **Loading.** The web loads each face with `FontFace` at boot (`host/web/glue.js:823-857`); Apple registers them with CoreText
  (`Text.swift:362`).
- **Limits.**
  - TTF/OTF only; WOFF2 is refused (`contract/lower/src/fonts.rs:81-99`).
  - Paths must be relative to the app directory.
  - No fallback stacks among declared families.
  - There is no count limit, so ten families are fine.
- **Choosing a face per row.** It must be a compile-time literal per arm: a `match` over literal family names (LLP 1053 G7, landed).
- **Linux** lags on `font_family` (QUEUE.md:517).

### 12. carousel: partly
- **Built.** A horizontal `scroll` with a `row` of 10 cards compiles inside a list row. `scroll-snap-type: x mandatory`
  / `start` exist, but on iOS they only set fast deceleration (`NodeViewIOS.swift:1180`).
- **Not virtualized.** The inner list cannot be virtualized: a nested virtualized list is refused (`collection.rs:72-76`), and
  a virtualized container cannot be horizontal (`:127-131`).
- **Not pooled.** A row holding a live `UIScrollView` is never pooled (`NodePoolIOS.swift:154`).
- **Owner.** LLP 1010 §6, "a bounded vertical list with one direct keyed `each`" (`llp/1010-scrolling-v1.spec.md:716-722`).
  With 10 cards a plain scroll is fine; the cost is the unpooled row.

### 13. motion: no on native
- **Animated GIF/WebP.** On native these paint their first frame only. The Apple test is
  `testOrientationAndSupportedFirstFrameFormats` (`RasterImageTests.swift:62`), and LLP 1011 lists
  "animated images on macOS (the first frame paints)" as a deviation (`llp/1011-image-v1.spec.md:304`).
  No code uses `CGAnimateImageAtURLWithBlock` or GIF frame properties. The web animates only because `<img>` does.
  No LLP owns it.
- **Lottie** is refused by name: "No `lottie`, `rive`, …" (`rules/DEFERRED.md:237`). The natural
  exact2 equivalent is an SVG whose shapes carry CSS `@keyframes` on `transform`, `opacity`,
  `stroke-dashoffset`, `fill` and `stroke`. It is built (`apps/sparkline/app.contract:28-30`) and handed to
  Core Animation on iOS for opacity, dashoffset, `r`, fill and stroke (`host/apple/src/svg.rs:27-33`).
  Path morphing (`d`) and SMIL are not animatable (SMIL is LLP 1055.000 stage 11). A Bodymovin file
  must therefore be hand-ported or converted to SVG plus keyframes.

### 14. glass: partly
- **Contract spelling.** `backgroundMaterial="ultra-thin"` (`tags.rs:437`) maps to `UIVisualEffectView`
  `.systemUltraThinMaterial` on iOS (`NodeViewIOS.swift:1017-1047`) and `NSVisualEffectView` on macOS.
  On the web it becomes `backdrop-filter: blur(20px) saturate(…)` (`host/web/index.html:55-56`).
- **The CSS row has no Contract name.** `backdrop_blur` exists (`schema.json:2071`, bit 62), but Contract has no attribute for it, so `backdrop-filter: blur()`
  cannot be written. That contradicts "the web is the standard".
- **Tint.** An rgba `background-color` gives the translucent tint.
- **Gaps.**
  - Rows with a material are not pooled (`NodePoolIOS.swift:156`).
  - Linux paints the material transparent.

### 15. live: yes
- **Countdown.** A countdown is `task … mount` / `every(1000, tick)` (`contract/syntax/src/parser.rs:964`;
  `apps/caltrain/app.contract:118-119`).
- **Rings and waveform.** Progress rings (`stroke-dashoffset` with infinite `@keyframes`, LLP 1055 D5) are built and used in a
  virtualized list (`apps/sparkline/app.contract:28-30,59`). The 48 waveform bars are SVG `rect`s or
  boxes, and the playhead is an infinite `translate` keyframe. Do not use a 50 ms tick: no app proves periods under 250 ms.
- **Cost on iOS.** Keyframes on `translate`, `scale` and colour are sampled by the engine every frame, which keeps the
  display link awake (`host/apple/src/svg.rs:27-33`). A screen of shimmers and playheads costs
  main-thread time. SwiftUI and Expo hand the same animations to Core Animation or Reanimated.

### 16. thread: partly
- **Clamp.** `line-clamp` and `text-overflow` exist (`tags.rs:663-664`) and appear in rows (`apps/messages/app.contract:532,539`).
- **Height to `auto`.** The kernel animates `height` to `auto` under `interpolate-size: allow-keywords` with
  `box-sizing: border-box` (`kernel/src/motion.rs:478-505`). Apple runs it (`host/apple/src/height.rs:161-200`), but only with
  one layout root and no content region (`:185`). No test or app animates a `line-clamp` change, and
  Linux is open.
- **Fallback.** Animate between two numeric heights computed from `line-height × lines`.
- **Inputs.** `input placeholder="Add a comment…"` is built.
- **Recycling rules (LLP 1010).**
  - Rows that leave the window lose component state, so the draft must live in the parent's keyed data
    (`llp/1010-scrolling-v1.spec.md:334-340,729`).
  - A focused row is pinned while off screen. That was checked only on the web, with a `/tmp` fixture (`:423-427`).
  - Input rows are never pooled on iOS.
  - An open QUEUE item covers `press` on a container holding its own input (QUEUE.md:105).

### 17. webview: partly (`srcdoc` unbuilt)
- **What the SPEC asks.** `embed.html` with five placeholders filled per row, loaded as an HTML string with no base
  URL. Without `srcdoc` the port must bundle one generated page per webview row (177 files) and use `src`.
- **Built.** `iframe` becomes `WebView` (`tags.rs:151`) on web, macOS and iOS; Linux is absent by design
  (`llp/1020-webview.rfc.md:271-275`). A bundled local `src` is read and inlined as the inner frame's srcdoc on
  Apple (`:184-192`); Caltrain uses `iframe src="/deck/index.html"` (`apps/caltrain/app.contract:137`).
- **Gaps.**
  - An author-facing `srcdoc` attribute is not built (`llp/1020-webview.rfc.md:302`).
  - Subresources of a multi-file bundle do not resolve.
  - LLP 1020 is Draft.
  - Web views are not pooled, so each row that scrolls in builds a `WKWebView`.
  - A cold simulator has timed out a deck's first load (QUEUE.md:111).

### 18. filmstrip: no
- **Refused.** The ordinary form is `list virtualized=true` over 2,000 items inside the feed row's
  template. Lowering walks the row template, active and inactive branches alike, and rejects it
  (`contract/lower/src/collection.rs:72-74`, `lower-collection-nested`: "a virtualized list cannot
  occur inside another virtualized list's row template until bounded ancestor-row lifetime is
  supported").
- **Not horizontal either.** Even alone, a virtualized container refuses `flex-direction`,
  `flex-wrap` and grid templates, and requires `overflow-x: hidden` (`collection.rs:119-131`,
  `lower-collection-flow`). The collection index is height-only (`runner/src/instance/collection/index.rs:1-12`).
- **Fallback.** A plain horizontal `scroll` with 2,000 eager items: about 6,000 kernel nodes per strip
  and 500 strips in the feed. "Ordinary `scroll`/`each` remains eager" (`llp/1010-scrolling-v1.spec.md:219-220`).
  Each node costs about 1.3 KB of kernel memory and 720 B of `NodeView` (`QUEUE.md:7`), all laid out
  from the root. A row with a live `UIScrollView` is never pooled on iOS
  (`host/apple/Sources/ExactKit/IOS/NodePoolIOS.swift:153-160`). Or page the data (a strip of 40 with
  arrows), which changes the benchmark.
- **Offset.** It is not kept, because row-local state dies on eviction (`llp/1010-scrolling-v1.spec.md:729-730`).
  The app can keep it: the `scroll` event carries `scrollLeft` (`contract/lower/src/lib.rs:1376`), and
  `scrollLeft` is a prop (`contract/lower/src/tags.rs:476-477`). A keyed map in the parent plus a
  bound `scrollLeft` restores it, the pattern SwiftUI and Expo use. It is untested on iOS for a newly
  created view.

### 19. inbox: no
- **Refused** as a nested virtualized list (`collection.rs:72-74`), like the strip. A vertical
  virtualized list alone is the supported shape (`llp/1010-scrolling-v1.spec.md:715-716`), and LLP 1010
  hosts already report "the actual nested scrollport" (`:732`). So the missing piece is the lifetime
  of a collection whose ancestor row can be evicted, not windowing in a box.
- **Fallback.** An eager `scroll` of 1,000 messages (about 7,000 nodes) in a `height=400` box, 250 times
  in the feed, with the same pooling and offset notes as the strip.
- **Nested scrolling.** Contract has `overscroll-behavior`, `-x` and `-y` (`contract/lower/src/tags.rs:786-790`;
  `kernel/tables/schema.json:1374-1380`, default `auto`). It is honoured on macOS
  (`host/apple/Sources/ExactKit/Mac/NodeViewMac.swift:62-68`, `:1159-1162`) and by the browser. iOS
  reads it nowhere: `NodeViewIOS.swift:30-34` says "UIKit does not chain a pan out of a nested scroll
  view at its edge", recorded as not in v1 (`llp/1008-apple-host-v1.spec.md:1219-1220`, `QUEUE.md:506`).
  - For this benchmark that is parity: SwiftUI and Expo on iOS do not chain either (SPEC Decisions).
  - Against the standard it is a gap: the web chains by default.

## Gaps in priority order

**1. Heavy views are never recycled in iOS list rows.**
- **What it blocks.** This touches 9 kinds: shader, canvas, video, map, carousel, glass, thread,
  webview, and every row that holds any of them. `NodePoolIOS.recyclable`
  (`host/apple/Sources/ExactKit/IOS/NodePoolIOS.swift:152-162`) refuses video, web, Metal, canvas
  input, materials, live scroll views and fields. `kinds` (`:60`) leaves out `NativeView`.
- **Capacity.** The pool is 32 views with 8 per shape, and a "shape" is the preorder of kinds and child counts. So 17 row kinds will
  reuse little even among the plain ones.
- **Why it comes first.** Every heavy kind is expressible and none of this breaks correctness. But at fling speed each scroll-in
  builds an `AVPlayerLayer`, `WKWebView`, `MKMapView`, `UIVisualEffectView`, `UIScrollView` or a wgpu surface.
  That is the exact cost this benchmark exists to measure. cryptobench measured it for Metal alone: 375 surface creates a
  second and 824 MB.
- **Owners.** They are split. 2D canvases are LLP 1056 stage 3. The pool itself is LLP 1010 §6. Video, web view,
  native view, material and scroll-view pooling have **no LLP yet**. Each needs a reset contract; the
  heaviest is moving a wgpu surface or an `AVPlayer` between rows.
- **Size.** Large, done per kind. Materials and scroll views are the smallest.
- **Fallback.** Ship unpooled and report the rebuild cost as a finding. Pause video out of view with
  `playbackVisibilityThreshold`, and show a poster image in place of the map or web view until the row rests.

**2. Nested and horizontal virtualization (a separate LLP will design it; LLP 1010 §6 today).**
- **What it blocks.** Kinds 18 and 19, 750 rows (one in four), and the carousel's inner list. It is the
  only gap that stops two kinds from being expressed in their natural form at all.
- **Evidence.**
  - `contract/lower/src/collection.rs:72-74` refuses a `virtualized` list anywhere in another's row
    template (`lower-collection-nested`). The walk covers inactive `when`/`match` arms too
    (`:62-64`, `:78-90`).
  - `collection.rs:119-131` (`lower-collection-flow`): on the container, `overflow-x` must be
    `hidden`, `gap` 0, and `flex-direction`/`flex-wrap`/`grid-template-*` are refused; top and bottom
    padding must be literal 0 (`:109`).
  - The collection index and the legacy windowed list are height-only, with width only as the cross
    axis (`runner/src/instance/collection/index.rs:1-12`, `:101`, `:277`; `runner/src/instance/window.rs:39`,
    `:243`, `:288`).
  - LLP 1010 supports "a bounded vertical list with one direct keyed `each`"
    (`llp/1010-scrolling-v1.spec.md:715-716`). It rejects virtualized descendants of a virtual row
    template, allowing an eager one or a sheet with one collection (`:760-763`). Row-local state dies
    on eviction (`:729-730`).
  - "Bounded ancestor-row lifetime" appears only in the compiler's message. No LLP defines it, and none
    proposes horizontal virtualization. `QUEUE.md:141` notes the same wall from the Messages
    transcript: its horizontal timestamp reveal is "outside the collection's supported shape".
  - `rules/DEFERRED.md:65-66` defers only a *sheet* with a nested collection (LLP 1041 §8.5).
- **What the design has to answer.**
  - The lifetime of an inner collection when its ancestor row is evicted, parked or recycled: its
    item instances, its anchor and its offset.
  - An inline axis for the collection index, so rows can be columns.
  - Where an inner list's offset lives across eviction. SwiftUI and Expo both make it app state keyed
    by row id; exact2 could make it the collection's, keyed by the row's key.
  - `overscroll-behavior` on iOS: honour `auto` by chaining, or declare UIKit's non-chaining as a
    deviation in the kernel spec.
  - Pooling a row that holds a live scroll view (gap 1): 750 rows of the feed hold one.
- **Size.** Large: a lifetime model across two windowing levels, an axis in the index, and
  per-host scrollport reporting for inner lists.
- **Fallback.** Eager inner `scroll`s, about 6,000 nodes per strip and 7,000 per inbox, unpooled, the
  offset kept by the app through `scroll` → keyed map → bound `scrollLeft`/`scrollTop`. That measures
  exact2 without virtualization, which is a finding in itself. Or cut the inner lists to a page
  of items, which changes the benchmark and must be declared.

**3. Canvas 2D text and `drawImage` (LLP 1056 stage 2).**
- **What it blocks.** The canvas kind as specified. Stage 2 (`llp/1056-canvas-2d.rfc.md:603`) also brings `Path2D`,
  patterns and shadows. Accepted with an owner is the path: Caltrain's line map.
- **Size.** Medium. Text needs executor-local measurement on three hosts; images need the
  image pipeline reachable from the recorder.
- **Fallback.** Draw paths, circles and the gradient in the canvas, and lay the label (`text`) and the picture
  (`image`) over it with `position="absolute"`. On screen the result is the same. It is not the same workload.

**4. Animated GIF and WebP on native (no LLP yet; LLP 1011 lists it as a deviation).**
- **What it blocks.** Two of the motion row's three items on iOS and macOS. Images are otherwise mature (downsampled decode,
  off-thread preparation), so this is a decode-and-present loop. ImageIO frame iteration or
  `CGAnimateImageAtURLWithBlock` belongs in `RasterImage`, alongside a frame clock that respects on-screen visibility.
- **Size.** Medium.
- **Fallback.** Convert each animation to a horizontal sprite sheet and play it with
  `animation: … steps(N) infinite` on `translate` inside a clipped box. `steps()` easing is in the
  grammar (`schema.json` `_transitions`). Alternatively, encode the clip as a looping muted MP4 `video`.

**5. Lottie (refused by `rules/DEFERRED.md:237`).**
- **What it would take.** Admitting Lottie means amending DEFERRED with a take. The natural exact2 form is SVG with CSS
  `@keyframes`, which is built, but a Bodymovin player is a runtime.
- **Fallback.** Convert the chosen animation offline into
  SVG plus keyframes. Or pick a Lottie simple enough (transforms, opacity, trim paths as dashoffset) to port by
  hand, and record the conversion as a declared deviation in SPEC.

**6. `backdrop-filter` has no Contract name (no LLP yet).**
- **The problem.** The glass row works through the host-policy prop `backgroundMaterial="ultra-thin"`, which is the right
  iOS parity target. But the CSS row `backdrop_blur` (`schema.json:2071`) is unreachable from Contract, contrary to
  CLAUDE.md's "the web is the standard".
- **Size.** Small: add a table row, then map a numeric blur on Apple to the nearest material. UIKit has no
  public arbitrary-radius backdrop blur.
- **Fallback.** `backgroundMaterial` for now.

**7. `white-space: pre` (LLP 1053 G5, "can wait").**
- **What it blocks.** The code row. `pre-wrap` exists, so this is the `nowrap × preserve` quadrant.
- **Size.** Small to medium: every native text engine needs a no-wrap preserving line mode, and the collapsing step
  landed in `9bd0c225`.
- **Fallback.** One `pre-wrap` `text` per source line, inside a wide fixed-width box clipped by
  `overflow="hidden"`.

**8. Engine-sampled keyframes on iOS (no LLP yet; LLP 1062 is Implemented without it).**
- **What it affects.** The shimmer (kind 2), the playhead (kind 15) and any `translate`, `scale` or colour keyframe. On iOS these
  are sampled per frame by the engine; only opacity, dashoffset, `r`, fill and stroke go to Core
  Animation (`host/apple/src/svg.rs:27-33`). With several of them on screen the display link never sleeps. That
  skews CPU numbers against SwiftUI, whose shimmer runs in the render server.
- **Size.** Medium: hand box `translate`, `scale` and `rotate` keyframes to `CABasicAnimation`/`CAKeyframeAnimation`.
- **Fallback.** Prefer opacity-based effects. For example, the shimmer can be two stacked gradient bars cross-fading
  on opacity.

**9. SVG `image` (LLP 1055.000 stage 10a).**
- **The problem.** It is refused at `contract/lower/src/svg.rs:339-340`, and it needs each host's image pipeline for an element
  with no view.
- **Size.** Medium.
- **Fallback.** Put an `image` absolutely positioned under or over the `svg`, clipped by a
  rounded box in place of an SVG `clipPath`.

**10. An image-sampling GPU surface: the first worked example (LLP 1009/1014, no stage).**
- **The problem.** The pieces exist: the children texture (`gpu/src/lib.rs:282-291`), asset bytes (`:188-201`) and
  continuous frames. But no surface samples a photo, and LLP 1009 is still Review.
- **Size.** Small to medium as app code (a gpu crate with WGSL), plus gap 1's surface churn.
- **Fallback.** An SVG filter chain (`feTurbulence` + `feDisplacementMap` + `feColorMatrix` duotone) over an SVG
  `image` is blocked by gap 9, so the shader row has no cheap fallback. Build the surface.

**11. Height transition to `auto` with `line-clamp`, off Apple (LLP 1003/1063; Linux open).**
- **What to check.** It is expected to work on Apple and the web (`kernel/src/motion.rs:478-505`,
  `host/apple/src/height.rs:185` requires one root and no content region). No test animates a clamp change.
- **Fallback.** Numeric heights from `line-height × lines`, `transition="height 250ms"`, `overflow="hidden"`.

**12. Inputs across recycling (LLP 1010, by design).**
- **The problem.** The row's draft must be keyed state in the parent, a map from row id to text. Focus pins one row. This is
  correct CSS-list semantics, not a bug, but it is verified only by a `/tmp` web fixture
  (`llp/1010-…spec.md:423-427`).
- **What the benchmark should do.** Check it on iOS by typing, flinging away and back, and confirming the draft is kept.

**13. International text checks and small text gaps.**
- **The gaps.**
  - Apple ignores `lang`, so Han unification picks the system's CJK face.
  - `ltr` takes its direction from the first strong character (QUEUE.md:51).
  - The iOS mixed-script clipping bug is still listed as in flight (`llp/1053-…rfc.md:74`).
  - No emoji ZWJ or flag test exists.
- **Owners.** LLP 1053 §2 for the bug. Nothing owns the rest: no LLP yet.
- **Fallback.** None needed to express the row. Verify it with screenshots against the SwiftUI reference.

**14. Minor gaps.**
- **`grid-template-columns` in Contract** (no LLP yet). Fallback: `flex-wrap`.
- **An image `load` event** (LLP 1011). The spec's fixed 1.2 s makes a CSS `animation-delay` sufficient.
- **An author-facing `srcdoc`** (LLP 1020 §302). The SPEC's web view takes an HTML string; fallback: 177 generated
  pages bundled as files, one `src` each.
- **A relative-time formatter** (LLP 1054.000.003). Fallback: a hand-written `fn ago`.
- **Linux coverage.** Linux has no video, iframe, native map, material, `markup` or RTL. The benchmark is Apple-first, so this matters
  only if a Linux run is wanted.

## Found while building the app (2026-09-27, origin/main `6c1c6ca4` → `7123335e`)

The app is `exact-xheavy/` (README there says how each kind is written). What follows was hit while
writing it, each checked by compiling or running (`exact-xheavy/target/release/contract build` on a
one-screen file, or `scripts/agent.mjs ios … layout` on the simulator), not by reading.

**A bug, found here and fixed on origin/main (`54ef9abb`, 2026-09-27).**
- **`canvas`'s 300 × 150 default was an authored size, not a natural one.** `canvas surface=wave(…)
  width="100%" aspect-ratio=1.7777777777777777` in a 600-wide column laid out **600 × 150**
  (`layout shader-canvas`: `height = 150 (authored)`). Chrome lays `<canvas style="width:100%;
  aspect-ratio:16/9">` out 600 × 337.5: 300 × 150 is the replaced element's natural size, which
  `aspect-ratio` and a percentage width override. Minimal repro:
  ```
  component A
    view
      column width=600
        canvas surface=x() width="100%" aspect-ratio=1.7777777777777777
  ```
  The app wrote `height="auto"` until the fix; since `54ef9abb` (canvas and iframe are real replaced
  elements) the shader row is written without it and lays out 600 × 337.5 (verified on the simulator,
  `layout shader-canvas`).

**Not expressible the natural (web) way.**
- **~~No scroll-to-row~~ — built (LLP 1070.000, origin/main `2cfb87f7`).** Until then `BENCH_START_INDEX`
  was served by answering the feed from row n. Now it is `scrollIntoView("feed", key)`; the iPad shots at
  3 / 7 / 500 / 1,500 / 2,500 land row n exactly at the top.
- **~~No list indexing in Contract~~ — closed: `at(list, i)` (the web's `Array.prototype.at`, origin/main
  `60303a913`, in dca79552). The app now reads `match at(pool, mod(k.m0 + t.j * k.mStep, 997))`
  (`exact-xheavy/app.contract:739`, and the per-kind build of the time), as SwiftUI indexes its array; build `dcaat` (dca79552 + this
  line), exact2's 19-kind fling / innerfling / innerkeep rows re-run in `results/dcaat-19-<dev>/` and replace
  the `dca-19-<dev>` rows. Earlier exact2 inner-list rows (inbox 78–80 fps at 48k) paid the scan below.**
  Was: the inbox's message j is `messages[(m0 + j·mStep) mod 997]`; JS and
  Swift index the pool (`POOL[i]`, `pool[i]`). Contract has `first`, `filter`, `map`, `join`, `length`
  but no `at`/subscript, so each built message is `first(filter(pool, p => p.i == …))`, a 997-step scan
  in the VM per item. Per-row item lists would avoid it but are baked into the plan (no dedupe:
  250 inboxes × 1,000 messages is tens of MB), and a resource cannot live in a row component
  (`type-child-resource`).
- **`translate` takes lengths only.** `keyframes k from translate="-100% 0px"` is refused:
  `[lower-keyframes] BadValue("translate: -100% 0px")` (the row is a `vec2` of px,
  `kernel/tables/schema.json` `translate`). CSS percentages are of the element's own box, which is
  how a shimmer band or a playhead window is written. The thumbs shimmer uses px for C = 600 (the
  benchmark's column on the iPad); on a narrower screen the band's travel would be wrong.
- **Keyframes take literal values only; no custom properties.** A ring's arc oscillates around
  the row's own base (`p = base ± 0.2 sin …`); CSS writes one `@keyframes` over `var(--base)`.
  `style="--base: 0.4"` is `[lower-unknown-attr]`, and keyframe values cannot be expressions. The app
  generates one `@keyframes ringNN` per two-decimal base (101 rules, `exact-xheavy/gen-rings.py`) and picks
  it by name with a template: correct, but a generated-CSS workaround.
- **No `srcdoc`** (known, LLP 1020 §302): `[lower-unknown-attr] iframe has no attribute srcdoc`.
  The 132 webview rows are 264 bundled pages (running / paused), `exact-xheavy/gen-assets.py`.
- **`overflow-x="auto"` is refused** (`expected one of "visible", "hidden", "scroll"`). CSS's
  ordinary horizontal scroller is `overflow-x: auto`; the carousel says `scroll` (same behaviour on
  iOS, where scroll bars overlay).
- **`calc()` beyond `percent ± px` and `grid-template-columns`** (`calc((100% - 12px) / 4)` refused;
  `grid-template-columns` unknown). The thumbs grid is three `row`s of four `flex=1 aspect-ratio=1`
  cells — fine, but not the CSS grid a web developer would write.
- **`after(0, …)` is refused** (`lower-timer-interval`: at least 1 ms); `setTimeout(fn, 0)` is the
  web's ordinary "after this turn". The app uses `after(1, boot)`.
- **A computed `font-family` × computed `font-style` is checked per attribute, not per value
  pair.** Runs whose family is `(code ? "Space Mono" : "system-ui")` and whose style is
  `(italic ? "italic" : "normal")` are refused (`lower-font-face`: Space Mono declares no italic)
  even when `code` and `italic` never coincide, and even with `font-style=(code ? "normal" : …)`.
  The Markdown runs split code into its own `when` arm.
- **The GPU surface decodes its own image.** A surface that samples a photo either takes an
  `image` child as the children texture (LLP 1014; Apple only, and a canvas with children keeps
  the row out of the pool) or asks for the asset's bytes and decodes them itself: the shader row
  links the `image` crate (JPEG only) and decodes off the main thread to ≤ 1,200 px, while the
  host has ImageIO's downsampling decoder a surface cannot reach.
- **Animated images cannot be paused or seeked** (as on the web): `BENCH_FREEZE` swaps each GIF /
  WebP for a first-frame PNG (`assets/still/`).
- **Lottie** (rules/DEFERRED.md; Charlie, 2026-09-27): the four files are hand-ported to SVG +
  `@keyframes` (`exact-xheavy/lottie.contract`), a declared difference in SPEC.

**Reserved words and small surprises (not gaps).** `from` is reserved, so a source cannot be named
`from` (`syntax-expected-name`). `line-height=18` is CSS's unitless *multiplier* (18 × 15 px); the
app writes `"18px"`. A height transition needs the computed height to change: `height: auto` →
`auto` does not animate (as in CSS), so the thread box moves between `54px` and `auto` with
`interpolate-size: allow-keywords` — which then animates both ways in a list row on iOS (measured:
55 → 142 → 198 px at 16 / 125 / 1,000 ms after the tap).

**Confirmed working (by driving the simulator):** every row kind above draws; the thread's draft
survives the row leaving the window (it is the data source's, keyed by row id); the web view
loads its bundled page into a rounded, bordered box; the map, video, shader and Canvas 2D (text and
`drawImage`) rows render in list rows; live mode ticks the countdown, `Updated`, the waveform time
and the thread flips; the shader and rings redraw every frame.

**Nested lists, as built (LLP 1070 stage 4, origin/main `50a00477`; smoke on the iPad, `results/smoke/exact2-inner*`).**
- `innerfling 0`: the strip holds 119.4–119.9 fps at every ladder speed to ±96k pt/s; the inbox
  119–120 to ±24k, 106 at ±48k and 109–111 at ±96k (its 63,000 pt clamp the travel to 65 % / 33 %).
- `innerkeep 0`: strip marks 12,345 and 23,456 come back exact after the 3,000 pt and 24,000 pt trips;
  inbox 8,765 exact after the far trip; inbox 5,432 came back **5,424** after the near trip (8 pt: the kept
  anchor is re-measured against its estimate). `sameView` is 0 everywhere: the scroll view is new, the
  runner restored the position (Q1's default), no app code.
- The carousel is a nested row list too (`scroll-restoration="manual"`: SPEC's recycled carousel starts at 0).

**Measured cost of the missing list indexing (2026-09-28).** SwiftUI's inbox indexes the pool
directly (`swiftui/Nested.swift:97`, `pool[(row.m0! + j * row.mStep!) % pool.count]`); Expo builds its
1,000 items per mounted row from `POOL[…]` (`expo/src/nested.tsx:91-92`). exact2's inbox scans the 997
pool entries with `first(filter(pool, …))` per built message. On the iPad the inbox's `innerfling` runs
105–110 fps at ±48k–96k pt/s (SwiftUI 118–120) at 4.47 ms/frame main busy (SwiftUI 3.45);
`results/series19-r1/summary.md`. The scan is the likeliest single cause; not yet isolated by a
build with indexing.

**Found on the iPhone 13 Pro Max (iOS 17.6.1), 2026-09-28 (`results/iphone19-r1/`).**
- **Crash under `renderInContext` — fixed on origin/main `2cf49c21`.** Every probe run with
  `BENCH_RENDER=layer` on a scrolling scenario (`jump 1`, `fling 4`) terminated exact2 with
  `-[NSConcreteValue doubleValue]: unrecognized selector` inside QuartzCore's recursive layer render.
  My guess (a Core Animation keyframe of exact2's) was wrong: the cause was AVKit's gravity animation on
  iOS 17 (`FigVideoContainerLayer` `sublayerTransform.scale.y` carrying an `NSValue(CGSize)`),
  triggered by exact2's VideoArm setting `videoGravity` on every layout. The feed's video is now a
  bare player layer by default anyway (4445620b).
- **The GPU canvas drew nothing — fixed on origin/main `2cf49c21`.** The shader row stayed its #E5E5EA
  placeholder on the iPhone. Not an iOS 17 rendering bug: exact2 rendered GPU canvases only while
  the app was *active*, and a SpringBoard "Edit Home Screen" alert over the iPhone made every app
  inactive for the whole series. Now a canvas renders unless the app is backgrounded. (The alert
  also made SwiftUI and Expo inactive, with unknown effects: the iPhone series r1 is suspect for all three.)
- **Kept inner position, inbox.** `innerkeep` kept 7 of 12 marks: the strip 6/6 exact; the inbox came
  back 8–32 pt short (5432 → 5400; 5432 → 5416 with the mark already read back as 5424 right after
  it was set; 8765 → 8757). On the iPad 12/12 in the series (one smoke run 8 pt short).

**iPhone series r2 (origin/main `2083633d`, 2026-09-28, `results/iphone19-r2/`).** With the two iOS 17
fixes the layer-sampler runs no longer crash and the shader row draws. One harness caveat remains,
against exact2: on iOS 17 the probe's `renderInContext` sampler does not capture exact2's
`CAMetalLayer` content, so a drawn shader row reads as its #E5E5EA background, a 104 pt blank band
(`results/dump/iphone-cold2/cmp.png`: sampler frame beside a real screenshot of the same moment).
Consequences on the iPhone only: exact2's layer-sampler cold start never completes (the
hierarchy sampler sees the row: 191 ms, 3 runs, `results/iphone19-r2/cold-hierarchy/`), and its
`jump` timeouts (18/30) and `fling 4` blank counts include shader rows the screen shows drawn.
SwiftUI's `layerEffect` is Core Animation content the sampler does see. On the iPad (iOS 27) the
same sampler sees the Metal content.
**Fixed in the probe (2026-09-28):** `probe.m` now swizzles `-[CAMetalLayer nextDrawable]` and, after
each layer-sampler render, paints every CAMetalLayer that has vended a drawable and is visible under
the window as a dark/light checker, so it reads as ink; a Metal layer that never drew keeps its
background. Same rule in all three apps. Validated on the iPhone: the cold-start dump
(`results/dump/iphone-cold4/`) shows the drawn shader row marked while a photo still decoding
reads blank. Corrected iPhone rows are `results/iphone19-r2m/`.

**Found by the feature benchmarks (F1–F5, not in this repo, 2026-09-28).** An SVG mask or filter island over
16,777,216 pixels at display scale is dropped silently on Apple (`SvgIsland.swift:80`, `:193`,
`SvgPaint.swift:131`): the island covers the whole mask/filter region, not the visible part. A full-screen
map under `preserveAspectRatio="xMidYMid slice"` with a vignette mask and a 200 % drop-shadow region draws
nothing on the iPad; Chrome draws it. Repro: the feature bench's `filter-island-cap.contract`.
Also: CSS `filter` on an HTML box compiles but no Apple host code applies it (the schema's `filter` row
is SVG's, LLP 1055.000 D14); F3 puts its chain on an SVG `g`, which is supported. Not yet verified by
running a box filter.

**Live Canvas 2D grows without bound on iOS (feature F2, origin/main `dd09a300`, iPhone 13 Pro Max).** A full-screen
`canvas surface=particles` whose `draw_2d` returns `Ok(true)` every frame (3,000 arcs) shows blank (the
live-canvas bug) and its footprint climbs the whole time: 106 MB at 2 s after launch, 223 MB at 5 s, ~600 MB
at 15 s, 1,219 MB at 30 s, 2,169 MB at 60 s.
SwiftUI's `Canvas` doing the same work holds 57 MB. Frames stay at 120 fps; CPU is 1,400 ms/s against
SwiftUI's 420. Likely the same path as the blank (each frame's draw list or surface retained, never
presented); owned by the Canvas 2D live fix.
**Probe correction:** exact2 F1/F2 footprints of 342/223 MB in the first iPhone feature series came from
the older probe's full-scale ink render; the 1/8-scale sampler reads 80/106 MB (same build, delay 2 s or 5 s).

**Update 2026-09-29 (origin/main dca79552).** The live Canvas 2D fix (7911b6a67) holds F2's footprint at
56/57/45 MB on the iPhone (was growing ~35 MB/s to 2.2 GB), and the canvas shows. F2 still costs
1,035 CPU ms/s against SwiftUI's 413, almost all off the main thread (main 32 ms/s).
Web F2 after the base64 fix (faf0c3041): 120 fps, but main busy 686 ms/s against plain Chrome's 80. Trace
(names build): the rAF callback spends it in
`exact_canvas::geom::ellipse` (each `arc` flattened to Bezier segments in wasm, with libm trig), writing and
validating the list (`list::Writer::op`, `Vec` extend, `list::check`), then replaying it op by op from JS
through `Function.prototype.apply` (`fill`, `bezierCurveTo`). The plain page makes 3,000 `arc()` + `fill()`
calls. Candidate fixes: pass `arc` through as an op, and replay without `apply`.
**Probe (2026-09-29).** MapKit on iOS 17 draws into a plain CAMetalLayer under `VKMapView`, which the
probe's `nextDrawable` hook already sees; a loaded map (and MapKit's own loading grid) reads as filled and an
undrawn map box as blank (`results/metaldbg/maps-val.png`). The maps lane's 5 map-row jump timeouts came
from its private probe copy (`exact-maprow/probe/`, Sep 28 11:39), which predates the Metal marking.
Remaining iPhone jump timeouts are blank on the render server too (`screenBlank` = `layerBlank`): a video
box still black, and flat illustration colour >= 60 pt tall (SwiftUI's jump 0 too).

**LLP 1071 text race (origin/main 496ab5c35, 2026-09-29).** exact2 crashes during any scrolling run on both
devices (iPad: fling, live fling, ladder, jump; iPhone: ladder, jump, fling 4, innerfling), 5 crash reports
in 12 minutes, `results/main-19-ipad-496-hang/ExactIOS-*.ips`. `TextEngine` (`host/apple/Sources/ExactKit/Text.swift:388`)
owns an unsynchronized `TextResidency` struct and `measuredBreakCache`; since 1071 the runtime's measure callback
(`TextEngine.measureText` -> `measure` -> `paragraph` -> `TextResidency.retireWidths`, TextResidency.swift:477) runs on
`exact.owner` while main runs `Presenter.refreshVisibleText` -> `NodeView.paragraphLayout` -> `paragraph` ->
`TextResidency.trim/evict/charge` (:646). Faults: a Swift trap in `charge`, `swift_release` in `maintain`, a
Dictionary deinit, and freed CFStrings under ICU line breaking. The comparison runs on dca79552 until fixed.
