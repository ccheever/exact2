# Review: LLP 1056 r1, 2026-09-27 (grok)

- **Family:** xAI — `grok` CLI (grok-4.6), `--reasoning-effort xhigh --permission-mode plan --no-subagents --output-format streaming-json`, `--cwd` a detached review worktree at 2a81868d (r1); 2026-09-27T07:26:54Z start; `stopReason: end_turn`; 40,997 output tokens.
- **Method:** the shared brief (sha256 `9c0768ecfeb328ba8bbb48c2908761fadf653e073be7acb7f5fa02e4d7332559`) verbatim; one round; blind to the other review.
- **Reviewed revision:** LLP 1056 r1.
- **Redacted:** three lines of opening narration ("I'll audit…", "Next I'll read…", "I'll check…") before the heading.
- **Verdict:** build with named changes.
- **Disposition:** LLP 1056 §0 (r2). Every finding below is accepted and answered there, in the section named, with these notes:
  - §1 point 3 (`arcTo` radius 0 "not a CG divergence"): accepted. r2 drops the claim and lists `arcTo`'s specified cases in §3.
  - §1 point 1 ("dropping non-finite ops before replay is the right host-side translation"): superseded. Astra showed the rules must apply at the author's call; D3 moves them into the recorder, and the runner's check is structural only.
  - §3 point 1 (state persistence, option A or B): option A, the web's. The context persists (D3).
  - §3 point 8 (`ImageData` space): device pixels, as on the web. `Frame` gains `pixelWidth`, `pixelHeight` and `scale`, and pixel payloads have their own bound (D4, D6).
  - §3 point 11 (overlay versus pooling): 2D children are ordinary subviews over the view's own layer contents; there is no overlay view (D7, D10).
  - §6 (text is the long pole of stage 1): accepted. Text moves to stage 2.
  - §9 (take 1 only): accepted.

---

## Review (verbatim)

# Audit of LLP 1056 r1 (`2a81868d`)

Document: `llp/1056-canvas-2d.rfc.md`. Binding: the web is the standard; Chrome is the oracle. No builds, no network, no files outside this checkout. Probe numbers that live only under `~/bench/cryptobench` and `metrics/ibex2-speed.jsonl` are **unverifiable here**.

---

## 1. HTML Canvas 2D spec claims

**Agree, with these defaults and behaviours stated correctly**

- Defaults quoted in §3 (`fillStyle`/`strokeStyle` `#000000`, `lineWidth` 1, `lineCap` `butt`, `lineJoin` `miter`, `miterLimit` 10, `font` `10px sans-serif`) match the spec.
- Non-finite **drawing** arguments are a no-op. Dropping those ops before replay is the right host-side translation.
- Bitmap persistence, transparent-black clear on create, and “setting `width` clears” are the right web rules to analogize.
- `source-in` / `source-out` / `destination-in` / `destination-atop` / `copy` really do affect the whole canvas; CG blends only under the shape. A transparency layer over the clip is how WebKit closes that gap.
- `shadowBlur` is twice the Gaussian σ. CG’s blur parameter is a different quantity. Conversion belongs in the replayer.
- Incomplete images: `drawImage` is a no-op. Auto-redraw on decode is the web’s `onload = draw`, made automatic (a platform adaptation, not a spec misread).
- `reset()` resets state **and** clears the bitmap, including the clip stack.
- `getTransform()` after `resetTransform()` is identity in author space. Hiding device scale from that matrix is a real deviation; D6 names it.
- `measureText`’s `width` plus the six bounding-box fields is the right v1 cut; baselines/`emHeight*` can wait.

**Disagree — wrong or incomplete vs the spec**

1. **Non-finite is not the only silent-ignore rule.** Negative `lineWidth`, `miterLimit` ≤ 0, `globalAlpha` outside `[0,1]`, negative `shadowBlur` are finite and must be ignored, not applied. `setLineDash` with a negative or non-finite element is a no-op for the **whole** call. Change: validator/recorder tables for “ignore assignment” vs “no-op method” vs “throw”.

2. **Throws are missing.** Negative radius on `arc` / `arcTo` / `ellipse`, `createRadialGradient` with r < 0, `addColorStop` offset outside `[0,1]` → `IndexSizeError`. Unparseable `addColorStop` colour → `SyntaxError`. The RFC’s “ignore invalid assignment as the spec does” (`llp/1056-canvas-2d.rfc.md:109`) is true for `fillStyle`/`strokeStyle`/`font`, **false** for `addColorStop`.

3. **`arcTo` radius 0 is specified, not a CG mystery.** The spec: line to `(x1,y1)` and return. `CGContextAddArcToPoint` does the same. Keep non-finite as a difference; drop zero-radius as a claimed CG divergence unless measured.

4. **Y-flip inverts `arc`/`ellipse` clockwise.** The RFC names the origin flip (`:49`) and points at `TextRaster.swift`. That file does `translateBy(x: 0, y: height); scaleBy(x: scale, y: -scale)` (`host/apple/Sources/ExactKit/TextRaster.swift:150–152`). After that CTM, canvas clockwise ≠ CG clockwise unless the replayer inverts the flag. This is the highest-probability parity bug in stage 1 and is not in the CG-difference list.

5. **`xor` is not a whole-canvas clearer.** §1’s list of whole-canvas operators is right and does not include `xor`. Stage 2 then dumps `xor` in with them (`:233`). `xor` only affects coverage of the source. It can share the transparency-layer implementation, but it is not the same class as `source-in`.

6. **Porter-Duff operators that only touch covered pixels are omitted from both stages:** `destination-over`, `source-atop`, `destination-out`, `lighter` (and `plus-lighter`). They map to CG blend modes without a transparency layer — the same justification stage 1 uses for `multiply`…`luminosity`. Change: put them in v1, or refuse them by name.

7. **`drawImage` does not take `ImageData`.** `CanvasImageSource` is images, canvases, video, `ImageBitmap`, `OffscreenCanvas`, `VideoFrame`. Pixels go through `putImageData`. D9’s “`ImageData` is also accepted” under `drawImage` (`:201`) is wrong.

8. **`putImageData` is not scaled and ignores smoothing, transform, clip, alpha, and compositing.** Stage 1’s “`imageSmoothingEnabled` … for `putImageData` scaling” (`:226`) is a spec error unless D6 secretly redefines `ImageData` in CSS pixels (see point 3 below). Dirty-rect overload is unspecified.

9. **`CanvasGradient` is live.** Stops added after `fillStyle = g` still affect later paints. A recorder that snapshots the gradient at assignment is wrong. Need handles: `create*` → id, `addColorStop(id, …)`, style references id.

10. **Colour vocabulary vs the kernel parser.** RFC: parse with “the kernel’s CSS parsers”. `Color::parse` accepts hex, `rgb()`/`rgba()`, and `transparent` only; **`"red"` and `hsl(...)` are explicitly `None`** (`kernel/src/style/tests.rs:140–153`, `kernel/src/style.rs:723–732`). Canvas `fillStyle = "red"` must work. Same for `currentColor` (the canvas node’s `color`). Change: extend the parser, or declare a colour subset and put named colours in v1.

11. **There is no kernel CSS `font` shorthand parser.** Font is separate rows (`font-size`, `font-weight`, `font-family`). D8’s “parsed once by the kernel’s parser” (`:191`) is not true of this tree. Someone has to write that parser (CSS Fonts shorthand, including the `10px sans-serif` default).

12. **`setLineDash` odd-length lists are duplicated to even.** Not mentioned.

13. **`direction: inherit`** must resolve against the canvas **node’s** CSS direction. `textAlign: start|end` depend on it. Not specified.

---

## 2. D1 (who draws) and D2 (one tag, roster picks context)

**Agree on the direction.**

- Imperative drawing belongs in the data module. Contract has no loops (`llp/1027-typescript-data-sources.rfc.md` §2). A chart is a loop.
- SVG stays the declarative model (`llp/1055-svg-shapes-and-css-animations.rfc.md`). A second drawing language in Contract would be a second bug farm.
- One `canvas` tag already exists, default 300×150 (`contract/lower/src/tags.rs:137–140`). GPU vs 2D as `getContext` via **which roster registered the name** matches the web (no `context=` attribute).
- Compiler refusal of a name in both rosters, or neither, is the right static check.
- A 2D canvas must not load the GPU module. Children still composite **over** the surface (`llp/1014-canvas-children.rfc.md` D2; web wrapper in `host/web/src/document.rs:183–184, 427–428`).
- Not taking `wants_children` for 2D (no `drawElementImage`) is consistent with the §3 refusal.

**Disagree / change**

1. **`draw` / `surfaces` are a new data-module ABI.** Today the TS seam is `appId`, `grants`, `answer`, `parse` (`js/src/lib.rs:20–23, 78–79`, `ABI = 1`). `Module::inspect` only reads `appId` and `grants` (`js/src/lib.rs:480–494`). This is ABI 2, plus a compiler roster beside `.shells/surfaces.json` (`contract/cli/src/surface.rs:9`). Write that amendment, including how `tsc` sees `Ctx2D` without copying IDL.

2. **The TS recorder cannot be “one more export” on the existing Hermes call.** `exact_js_call` is three **strings** in, one **string** out (`js/src/shim.cc:261–281`). A `Float64Array` “crossing as bytes” needs a binary return (or a documented JSON envelope and its cost). The prelude wrapper, the recorder object, and `measureText` host ops are part of D1, not stage-1 folklore.

3. **`exact-canvas` cannot be both “no dependencies” and the colour/font validator.** D1 (`:79`) vs D3 (`:109`). Kernel colour lives in `kernel/src/style.rs`. Change: `exact-canvas` may depend **down** on kernel parsers, or validation stays in the runner. “Leaf, depends on nothing” is false either way if strings are validated.

4. **“A web-sys routine ports by changing the type”** is optimistic. web-sys is overload-heavy (`JsValue` vs `_str`). Pin the exact method set (`set_stroke_style_str`, etc.) as the portable surface.

5. **GPU `Frame` and 2D `Frame` are different types.** GPU has `width`, `height`, `scale`, `now_ms`, `seekable`, `period_ms`, … (`gpu/src/lib.rs:40–67`). 2D is `{ time, mounted, width, height }`. That is fine (D6 hides scale) but say so, and say whether `period_ms` is intentionally absent (pacing).

---

## 3. D3 / D4 / D5 — list, when `draw` runs, clock, workers

**Agree**

- Kept bitmap + recorded list is the right cross-host model. Chrome then paints the **same list** natives do.
- Draw after layout so `frame.width/height` are the content box. GPU already splits **bind** (args at commit, `runner/src/instance.rs:783–813`) from **render** (size + time at present). 2D `draw` is that render, with the list produced in the executor.
- On-screen-only **frame requests**, first paint of a new/resized/arg-changed canvas even in overscan: learned from the GPU list (`QUEUE.md:8`).
- Worker placement is at least one turn later; first frame may be transparent. That matches 1027.002 (worker calls are `Later` even when the function is sync, `llp/1027.002-optional-worker-execution.rfc.md:140–148`) and should stay declared.
- `clock settle` must not wait on a canvas that may never stop asking. Same as infinite CSS animations (`llp/1055-svg-shapes-and-css-animations.rfc.md:237`). Seeking still draws once at the new time for canvases that asked.
- Accumulating trails are frame-rate dependent; declared, not “fixed”.
- Reload: 1007 rebuilds the tree, carries slots/resources/clock, not DOM identity (`llp/1007-web-host-v1.spec.md:452–469`). New canvases, new `mounted`, lost module locals. Correct.
- 1 MiB list bound with `state` reporting a refusal is the right shape of limit (number TBD).
- Throw in `draw` reported per canvas is right. **Keeping the previous bitmap** is **not** the web (ops before the throw have already hit the bitmap). Declare that deviation, and say whether recorder state rolls back.

**Disagree — holes an implementer would have to invent**

1. **Does drawing state persist across `draw()` calls?** D3 “the canvas model is kept” says yes (one `ctx`, clip/path/transform/style survive). D5 “depend only on arguments and frame” plus the example that restyles every time say no. The host `CGContext` / real `CanvasRenderingContext2D` **keep** state by default. `reset()` **clears pixels**, so it cannot be used to default state between replays. Need an explicit rule, e.g. at canvas birth `save()` the defaults; before each replay `restore(); save();` then replay — bitmap kept, author state either (A) persisted in the recorder and **not** reset, or (B) reset each `draw` as a **named** deviation. On throw, define recorder vs bitmap together.

2. **Web TypeScript `draw` is not “same turn” today.** Native Hermes `answer` is synchronous on the runner thread (`llp/1027.002-optional-worker-execution.rfc.md:36–40`). Browser TS is **already** `Later` through the iframe realm (`:43–50`). D4’s “on `main` placement a canvas is never presented without its first drawing” (`llp/1056-canvas-2d.rfc.md:137`) is true for native TS/Rust and **false for web TS** unless a synchronous draw path is specified. Caltrain (Rust data crate) would be sync in wasm; a TS sparkline on the web would flash transparent. Declare it or add a sync draw door.

3. **`measureText` as a synchronous host call fights 1027.002 D4.** “A service that needs the UI thread is an explicit asynchronous host request; no synchronous callback into the UI may deadlock against the caller” (`llp/1027.002-optional-worker-execution.rfc.md:288–290`). Worker-placed `draw` + sync measureText is that deadlock. Change: measure on the worker with a local text engine; or refuse `measureText` on worker; or make it async (breaks the API). Also: 45 ns is **call overhead**, not metrics, and `metrics/ibex2-speed.jsonl` is **not in this tree** (cited from LLP 1027/1028). Core Text measure is orders of magnitude slower.

4. **Web measureText fonts.** Scratch `OffscreenCanvas` in the **module iframe** does not see the page’s LLP 1019 `@font-face`. Drawing happens on the presented canvas. Labels will not fit unless the measure context shares the document’s faces.

5. **`getTransform` / `DOMMatrix` on Hermes.** Lean VM is not a browser. Need a matrix object with `a…f` (and `setTransform`’s `DOMMatrix2DInit` overload). Unspecified.

6. **Resize / scale**
   - Content-box or DPR change: bitmap cleared (D3). **Does context state reset?** On the web, assigning `canvas.width` resets **everything**. Say yes or name the deviation.
   - Pixel size: GPU uses `round` then `max(1)` (`gpu/src/lib.rs:70–74`). 2D must use the same rule (fractional CSS px, 0×0).
   - Scale-only change (move between 2× and 3× displays) clearing the bitmap is **not** what a web page does on `devicePixelRatio` change. D6 should list it next to the `getTransform` deviation.

7. **No bitmap dimension cap.** Memory is “O(mounted canvases)” with **no max**. A 10k×10k CSS box at 3× is gigabytes, not charged to the image budget (`:211`). Add a max dimension (and a `state` refusal), analogue of `WorldCarrier.limit` (`host/apple/Sources/ExactKit/CanvasSeams.swift:132–133`).

8. **`ImageData` coordinate space vs D6.** On the web, `ImageData` is **bitmap** pixels even after `ctx.scale(dpr,dpr)`. exact2 hides dpr. Then `createImageData(frame.width, frame.height)` is either CSS (deviation, implies upscale — the mistaken smoothing sentence) or device pixels (authors using `frame.width` are wrong). **Must be a declared deviation.** Arithmetic: default 300×150 at 3× is 900×450×4 = 1 620 000 bytes > 1 MiB list bound, so a full-canvas `putImageData` in device pixels **refuses on a 3× phone**. Either raise the bound, exclude pixel payloads, or define `ImageData` in CSS pixels.

9. **Fill-policy cost is not “known”.** D10 says first-draw cost counts toward 1050.000 D3 (“a row known to cost more than a frame”). 1050 only skips rows whose cost is **already known** (`llp/1050.000-choosing-the-fill-tradeoff.rfc.md:30–32, 80`). First canvas draw has no prior measurement. Change: estimate from bitmap bytes, or admit first draw always runs.

10. **`frame.mounted` on pool reuse.** D10 reuses the bitmap if size matches and starts transparent. If `mounted` is not reset, a recycled row skips the 600 ms draw-in. Reset `mounted` (and recorder state) on rebind. `NodePoolIOS` already resets SVG motion on release (`host/apple/Sources/ExactKit/IOS/NodePoolIOS.swift:30–33`).

11. **Children vs pooling.** `recyclable` requires `metal == nil`, `overlay == nil`, `canvasInput == nil` (`NodePoolIOS.swift:153–156`). Kinds today omit `canvas`; comments call canvas ineligible (`:34–36, :60`). A 2D row whose pulse is a **canvas child** (the recommended cheap pulse) is an overlay subtree on Apple GPU canvases today (`llp/1014.000-canvas-children-v1.spec.md:66–70`). If 2D keeps that overlay, **D10 pooling never fires for the list case that motivates the RFC.** Change: 2D children are ordinary subviews of a non-Metal `NodeView` (layer behind, no overlay), and `recyclable` allows that; or drop “pools like a text row” until that is designed.

12. **Multiple canvases in one commit:** tree order. **Empty list** (draw returns without ops): bitmap unchanged. **Partial list vs 1 MiB overflow during recording:** throw vs refuse vs truncate. Specify.

13. **Determinism.** “Pixels are a function of the lists” (`:117`) is false once D9 images exist (decode timing, reason 5). Even in v1 it is lists **plus** initial clear **plus** whether state persists. Tighten the sentence.

---

## 4. D6 (CSS px / device backing) and D7 (backends / threads / unsafe)

**Agree**

- Author space = content box in CSS px; backing = device pixels; replayer pre-multiplies scale; `getTransform()` omits it. That is the right author-facing model, and it **must** stay a declared deviation (web authors who `resetTransform()` after a manual `scale(dpr)` land in bitmap pixels; exact2 does not).
- Context attributes frozen (`alpha: true`, `srgb`, no `willReadFrequently` / `desynchronized` / `display-p3`) is the right v1 cut.
- **Core Graphics on Apple, in Swift, on the main thread, into a bitmap then `CALayer.contents`.** Mapping is 1:1 enough. `#![deny(unsafe_code)]` is real (`host/apple/src/lib.rs:33`). CG from Rust would be an audited `unsafe` surface per call; GPU already kept that out of the host (`llp/1009-gpu-canvas.rfc.md:45`). Swift reading a **validated** buffer via `UnsafeRawBufferPointer` is the same lesson as SVG JSON costing ~5% of main thread (`QUEUE.md:5`). Binary list from the start: yes.
- Main-thread replay for **v1 / Caltrain (one map)** is right: tens of microseconds, same `CATransaction` as the row’s text. A background queue as stage 4 if a fixture shows >1 ms is the right escalation, not v1 architecture.
- `CGLayer` out: correct.
- **Not tiny-skia on Apple** for this API: text must match box text (Core Text into the same context); Charlie asked for CG; 4.7× on the 2× probe is `133.5 / 28.6` from the appendix (same drawing). Apple vs Linux matching Chrome rather than each other matches SVG (`llp/1055.000-completing-svg.rfc.md:185–186`).
- **tiny-skia 0.12 on Linux under both painters:** the crate is `tiny-skia = "0.12"` (`host/linux/Cargo.toml:40`). CPU painter already is tiny-skia; GPU (vello) uploading a pixmap as an image brush is the island path (`llp/1055.000-completing-svg.rfc.md:201`). vello as a kept canvas is the wrong model (no persistent target, weak Porter-Duff/blur). Agree.
- Web glue loaded with the first 2D canvas, after first paint, LLP 1047: same as GPU glue (`llp/1009-gpu-canvas.rfc.md:103`).

**Disagree / change**

1. **Replayer transform composition** must be written: device scale is a **base** CTM; `setTransform` replaces the **author** matrix only; replay is `base ∘ author`. `resetTransform` restores author identity, not bitmap identity.
2. **Clockwise inversion after Y-flip** (see §1).
3. **Copy-on-write `makeImage()`** is real CG behaviour; assigning `contents` every sparkline is **not** in the probe. Stage 3 numbers must include layer commit, not only `draw + makeImage`.
4. **1055.000 islands on Apple are tiny-skia, not CG** (`llp/1055.000-completing-svg.rfc.md:19, 391, 469`). Q2 is consistent with that. `ctx.filter` “sharing islands” later means a **third** raster path on Apple (CG for unfiltered canvas, tiny-skia for filters). Say so.

---

## 5. D8 text, D9 images/pixels/Path2D, D10 pooling/fill/memory, §6 numbers

**D8 — agree** that canvas text must use the host text engine and LLP 1019 faces, that `maxWidth` scales horizontally, and that the text parity band is looser unless the font is declared (`scripts/svgparity.mjs:15–19` already special-cases Linux text). **Change:** write the font shorthand parser; resolve `inherit` direction; load the same faces into the measure path (especially web iframe); list remaining `TextMetrics` fields by name.

**D9 — agree** on handles not elements, decode → reason 5, refuse readback with a consumer trigger, Path2D in stage 2 via `kernel/src/svg/path.rs` (one `d` grammar; arcs already become cubics there, `:5–8, :250–252`). **Change:** `ImageData` is not a `drawImage` source; define pixel space vs D6; Path2D `fill`/`stroke`/`clip` overloads and `new Path2D(d)` land with Path2D, not by implication; `isPointInPath` in stage 3 pairing with `gpu_input` (`llp/1014.000-canvas-children-v1.spec.md:28`) is honest — but 2D hit-testing from geometry does not need that GPU seam; don’t wait on it unnecessarily.

**D10 — agree** that Metal stays unpooled (`NodePoolIOS.swift:155`, `QUEUE.md:8`), that 2D **should** be a pool kind, that frame requests are never “owed”, and that bitmaps exist for mounted nodes plus a bounded free list. `1ced26af` is real: “apple: memory pressure drops shaped text no view holds”. **Change:** overlay/children (above); `mounted` reset; bitmap size cap; don’t charge to image budget **and** still bound total canvas pixels.

**§6 numbers — mostly honest arithmetic, partly unverifiable, one citation slip**

| Claim | Check |
|---|---|
| 228×100×4 = 89 KiB; 342×150×4 = 200 KiB | Exact |
| 46 × 89 KiB ≈ 4.1 MB | `QUEUE.md:4` really says “46 mounted rows” (text rasters). Arithmetic holds |
| 38 + 4 ≈ 42 MB | 38 MB SVG rest is `QUEUE.md:4`. Additive is **conservative** if canvas **replaces** SVG layers; honest as an upper bound |
| 21 × 120 µs × 120 Hz ≈ 302 ms/s | Arithmetic holds. 120 µs is M5 `draw+makeImage` 42.7 µs × ~2.5 (M1) plus record/cross — **order of magnitude**, not a measurement. 21 “visible” is not in `QUEUE.md` (46 is mounted, including overscan) |
| 24 000 pt/s ÷ 64 pt/row = 375 rows/s; × 0.12 ms ≈ 45 ms/s | Sparkline row is `height=64` (`apps/sparkline/app.contract:68`). Holds as a model |
| 4.7× CG vs tiny-skia | 133.5/28.6 at 2× from the appendix. 3× is 5.7×; quote 2× or both |
| Hermes 3.8–4.0 µs / 177 doubles | Probe **omits** style setters the D1 example uses; 177 matches the stripped path ops. `Date.now()` at 50k iterations is coarse but enough for ~4 µs |
| GPU 879 ms/s, 116–117 MB | **Not in this checkout** (`GAPS.md` / cryptobench). Table says 116 MB (`:290`); summary says 117 (`:24`). Pick one |
| 45 ns host call | Not in-tree; LLP 1027/1028. Don’t cite a missing jsonl |
| CG probe Y-up, no flip | Fine for a raster-time probe; **not** a parity probe |

The conclusion “not in the GPU canvas cost class; SVG still better for a pulse on thousands of rows” is the right reading of the model. Stage 3 must replace estimates. Do not treat 42 MB / 300 ms/s as commitments.

---

## 6. §3 subset, §4 parity, §5 agent, §8 staging, §9 NOT-DOING, §10 questions

**§3 — agree** on refusing readback, `ctx.filter` until islands, focus rings, hit regions, `ctx.canvas` / app-visible `OffscreenCanvas`, `drawImage` of canvas/video/element, `display-p3`. v1 including `reset`, `roundRect`, clip, gradients, blend modes, and text is **large** relative to the Caltrain map (line + quad, `apps/caltrain/gpu/src/lib.rs:91–99`, `render` returns `false` at `:194`). That is OK if the gallery is the real stage-1 bar; say that Caltrain does not need text/`putImageData`.

**Change:** add the missing Porter-Duff ops or refuse them; fix `putImageData` smoothing; `imageSmoothing*` applies to `drawImage` (stage 2), not `putImageData`.

**§4 — agree** on generalising `scripts/svgparity.mjs` (bands `MEAN = 4`, `OFF = 0.06`, `BAND = 32`, `SLOP = 3`, Linux text 14 / 16%). Accumulation fixtures from a cleared canvas at settled times is the only deterministic choice given D5.

**Change:** the web host’s glue is **inside** the oracle. A glue opcode bug looks like “Chrome is right”. Keep one HTML page (or a `getContext('2d')` path in the gallery) that draws the **same calls without the list**, or accept that glue is untested against Chrome-the-API. Also: Display P3 → sRGB via `sips` is already in the SVG smoke (`scripts/svgparity.mjs:63–68`); canvas bitmaps are sRGB by D6, so native window captures still need that conversion.

**§5 — agree.** No ninth operation. `tree` + `context: "2d"`, `state` per canvas (list size, `animating`, refusals), `layout` decode in **dev only** (200-line cap), `screenshot` unchanged. macOS model-layer screenshots **will** show a static `CALayer.contents` bitmap, unlike SVG’s compositor pulse (`llp/1055-svg-shapes-and-css-animations.rfc.md` §0 macOS note). If the pulse is a CSS-animated **child**, that child still needs `screenshot … window`. Mention that.

**§8 — agree** on order: format/validator/replayers/gallery first; images/Path2D/shadows/unbounded ops second; pooling + iPad bench third; off-main replay only if measured. Sparkline toggle and `apps/canvas-gallery` are apparatus; Q4 asking Charlie matches `rules/RULES.md` (agents add no apparatus).

**Change:** stage 1 “text on all three hosts” is the long pole; Caltrain map as take does not require it. Either shrink stage 1 to what the take needs plus gallery shapes, or admit stage 1 is a full 2D subset and will not ship quickly.

**§9 — agree** that Canvas 2D is not admitted today (`rules/NOT-DOING.md` SVG expansion is 2026-09-26; no canvas-2d line). The drafted Components paragraph is in the house style. **Take 1 is the only take that meets the rule:** Caltrain defines v1 (`rules/NOT-DOING.md:10–14`); the map is a 2D line drawing with children for names (`apps/caltrain/app.contract:257–268`; `apps/caltrain/gpu/src/lib.rs:1–15, 104–195`); `render` returns `false` and motion is `nowMs` on a 1 s `task ticker` (`app.contract:70–71, 118–119`) — so D4 reason 2 (args changed) is enough, no rAF. Deleting `map` from the GPU registry (`lib.rs:242–248`) leaves aurora/glass/stack. That is “fewer GPU paths after than before”.

**Disagree on takes 2 and 3 as NOT-DOING takes.** `Custom(u16)` is decided-unbuilt in 1009 and still queued “when a surface needs it” (`QUEUE.md:507–508`; also LLP 1013 / 1046.002). It is not a doing-list line you can take off, and GPU surfaces may still want it. Pooling GPU rows is already a queue item; the RFC itself says it doesn’t meet the rule (`:345`). Don’t pretend they do. Recommend take 1 only.

**§10 — agree** with the four questions and the leanings: admit with take 1; CG on Apple; refuse readback/`filter` in v1; yes to gallery apparatus if Charlie says so.

**Change Q2** to note islands already chose tiny-skia on Apple (`llp/1055.000-completing-svg.rfc.md:469`).

---

## 7. Missing — what an implementer would have to invent

Already covered above, collected:

| Gap | Why it blocks |
|---|---|
 | Persist vs reset of ctx state across `draw` | Wrong clip/path/style on frame 2 |
 | Web TS first paint (`Later`) | Blank first frame vs D4 promise |
 | Hermes ABI for bytes + `draw` (ABI 2) | Cannot ship TS `draw` on the current string seam |
 | `measureText` vs worker deadlock and iframe fonts | Wrong metrics or freeze |
 | `ImageData` CSS vs device pixels vs 1 MiB | Silent refuse on 3× `putImageData` |
 | Bitmap max size | OOM |
 | Colour named/`hsl`/`currentColor`; font shorthand | `fillStyle = "red"` and `font = "10px sans-serif"` |
 | Gradient/Path2D live handles | Wrong pictures |
 | Throws vs ignore tables | Divergent from Chrome |
 | Arc clockwise after Y-flip | Arc fixtures fail parity |
 | 2D view tree vs overlay vs `NodePool` | List case never pools |
 | `frame.mounted` on reuse | Draw-in wrong |
 | Pixel rounding / 0×0 | Mismatch with GPU canvases |
 | `DOMMatrix` on Hermes | `getTransform` type lie |
 | Glue vs Chrome-the-API | False oracle |
 | Fill-policy “known cost” | Policy cannot fire |
 | Ordering, overflow, empty list, throw rollback | Divergent hosts |
 | `direction` / `start`/`end` | Text fixtures |
 | Missing Porter-Duff ops | Authors hit a wall with no refusal |

Smaller but real: `roundRect` radii overloads; `putImageData` dirty rect; `createImageData` copy constructor; `new ImageData`; save-stack depth vs 1 MiB; side-table strings in the bound; `letterSpacing` etc. already deferred.

---

## Point-by-point

1. **Spec description — disagree in part.** Defaults, non-finite drawing no-ops, bitmap persistence, whole-canvas compositing set, `shadowBlur` = 2σ, incomplete `drawImage`, `reset()`, `getTransform` identity after `resetTransform` are right. Wrong or missing: ignore-vs-throw tables; `addColorStop`; `arc` clockwise after flip; `xor` classification; omitted Porter-Duff; `drawImage(ImageData)`; `putImageData` + smoothing; live `CanvasGradient`; kernel colour/font parsers. **Change:** fix those in r2 before stage 1.

2. **D1/D2 — agree**, with ABI 2, binary TS seam, and crate layering written down. Do not claim `exact-canvas` has no dependencies if it validates CSS.

3. **D3/D4/D5 — agree on the skeleton**, not on completeness. **Must decide** state persistence, web-TS latency, measureText placement, ImageData space, bitmap caps, pool/`mounted`/overlay, throw rollback. Until those are in the RFC, this is not buildable without invention.

4. **D6/D7 — agree.** Main-thread CG in Swift is right for v1; `deny(unsafe_code)` makes the Swift/Rust split right. Add Y-flip×arcs and author-vs-base CTM.

5. **D8–D10 / §6 — agree on intent; §6 estimates are honest as models, not as measurements.** Fix ImageData/`putImageData`; don’t pool in prose until overlay is designed; don’t cite missing files.

6. **§3–§10 — agree on admission shape, agent, staging, Q1–Q4.** Take 1 only. Shrink or justify stage-1 text. Glue is not a pure oracle.

7. **Missing — yes, a long list (table above).** None of it is optional if Chrome is the oracle.

---

**Verdict: build with named changes.**
