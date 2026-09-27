# LLP 1056: Canvas 2D — Core Graphics everywhere, by the web's name

**Type:** RFC
**Status:** Draft (r1). Charlie agreed to the direction on 2026-09-27; Canvas 2D is not admitted in `rules/NOT-DOING.md` until he rules on §9.
**Systems:** Data modules (a `draw` export in TypeScript and Rust; LLP 1027, 1027.002), a new leaf crate `exact-canvas` (the recorder, the list format, the validator), Contract (the `canvas` tag's surface roster), Runner (calling `draw` after layout; the canvas's frame requests), Web host (replaying into the real `CanvasRenderingContext2D`), Apple host (replaying into Core Graphics), Linux host (replaying into tiny-skia), Agent (`state` for a canvas)
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-27 (r1)
**Implementer:** Claude (Opus 5.5), from Charlie's ruling on §9, stage by stage in §8's order. Nothing is built before that ruling.
**Related:** LLP 1009 (the `canvas` node and its wgpu surfaces; D2's surface roster and D4's frame ownership are what this reuses), LLP 1014 / 1014.000 (canvas children: they composite over a 2D surface too), LLP 1055 / 1055.000 (SVG: the declarative drawing model, and the rasterizer choices this shares: Core Graphics on Apple, tiny-skia on Linux, the islands question), LLP 1027 / 1027.002 (data modules, the executor, worker placement), LLP 1002 / 1003 (motion; the agent-owned clock and `clock settle`), LLP 1010 §6 and LLP 1050.000 (the collection, row pooling, the fill policy), LLP 1019 (declared fonts), LLP 1047 (pay for what you use), `~/bench/cryptobench/SPEC.md` and `GAPS.md` (the list case), `rules/NOT-DOING.md` §Components and §Runtime.

## Summary

Charlie's wish is "Core Graphics everywhere". The web already has it under another name. Apple designed the canvas element's 2D context for WebKit in 2004, on Core Graphics' model: paths, a current transform, a graphics-state stack (`save`/`restore`), clipping, fill rules, caps and joins, Porter-Duff compositing, and CG's shadow model. WHATWG then standardised it. `CanvasRenderingContext2D` is, in effect, Core Graphics as a cross-platform standard. It has a conformance suite and a browser to act as the oracle.

This RFC adds that context to exact2's existing `canvas` tag, on every host:

- **Who draws (D1).** The app's data module draws, in TypeScript or in Rust, against the web's own interface. A TypeScript surface is typed with `lib.dom`'s own `CanvasRenderingContext2D` member names. A Rust surface uses web-sys's names for the same members. Contract gains no drawing language: SVG (LLP 1055) is the declarative one.
- **One tag, and the code chooses the context (D2).** `canvas surface=spark(points, up)` names a surface. A surface a data module registers is `2d`; one a GPU module registers is `webgpu`. This matches the web, where code, not markup, calls `getContext`.
- **Immediate calls into a kept bitmap, carried as a recorded list (D3, D4).** A draw call appends to a compact list in the executor. The runner validates the list and puts it on the batch. The host replays it into the canvas's own bitmap, which persists between draws as the web's does. On the web the replay target is the real `CanvasRenderingContext2D`, so Chrome renders the same list the natives do.
- **Time is the host's frame clock (D5).** `draw` receives the frame time and returns whether it wants another frame, which is `requestAnimationFrame` without a global. Under an agent-owned clock that time is the agent's.
- **Backends (D7).** The browser on the web. Core Graphics into a bitmap context on Apple, which maps almost 1:1 and is measured below at 4.7× tiny-skia's speed on the same drawing. tiny-skia on Linux, under both painters. Text goes through each host's own text engine, so a canvas label matches the box text beside it.
- **v1 is the chart and control subset (§3).** Readback (`getImageData`, `toDataURL`), `ctx.filter`, focus rings, hit regions and an app-visible `OffscreenCanvas` are refused by name, each with the trigger that would bring it back.

**The list case (§6).** On this Mac, a crypto-bench sparkline records in about 4 µs under Hermes, and Core Graphics rasters it in 29 µs at 2×, or 43 µs once it is made into an image. Its bitmap is 89 KiB at 2×. A Canvas 2D row costs one plain layer, which pools like a text row, and not a Metal layer with a wgpu surface, which does not pool. The GPU-canvas bench app held 117 MB at rest and used 879 ms/s of CPU. A Canvas 2D version is estimated at about 42 MB. Its CPU would be near the SVG app's 119 ms/s if the pulse is a CSS animation on a canvas child, or about 300 ms/s on the main thread if every visible chart redraws every frame. §6 shows the arithmetic, and stage 3 measures it.

**Recommendation:** admit Canvas 2D with the take in §9, and build it in §8's order. The sparkline fixture gets a canvas chart beside its SVG one, and a canvas gallery is held to Chrome by the SVG parity comparator.

## 1. The model, and why it is Core Graphics

| Canvas 2D (HTML) | Core Graphics | tiny-skia |
|---|---|---|
| `save()` / `restore()` | `saveGState` / `restoreGState` | a state stack in the replayer |
| `translate` `rotate` `scale` `transform` `setTransform` | `translateBy` `rotate` `scaleBy` `concatenate`, `CGAffineTransform` | `Transform` |
| `beginPath` `moveTo` `lineTo` `closePath` | `beginPath` `move(to:)` `addLine(to:)` `closePath` | `PathBuilder` |
| `quadraticCurveTo` `bezierCurveTo` | `addQuadCurve` `addCurve` | `quad_to` `cubic_to` |
| `arc` `arcTo` `ellipse` | `addArc(center:…)`, `addArc(tangent1End:…)` (CG's `CGContextAddArcToPoint`), a scaled arc | arcs flattened to cubics (the SVG geometry's, `kernel/src/svg/`) |
| `rect` `roundRect` `fillRect` `strokeRect` `clearRect` | `addRect`, `CGPath(roundedRect:)`, `fill` / `stroke` / `clear` | `Rect`, paths |
| `fill(rule)` `stroke()` `clip(rule)` | `fillPath(using:)` `strokePath` `clip(using:)` | `fill_path` `stroke_path` `Mask` |
| `lineWidth` `lineCap` `lineJoin` `miterLimit` `setLineDash` `lineDashOffset` | `setLineWidth` `setLineCap` `setLineJoin` `setMiterLimit` `setLineDash(phase:lengths:)` | `Stroke`, `StrokeDash` |
| `globalAlpha` `globalCompositeOperation` | `setAlpha` `setBlendMode` | `Paint::blend_mode` |
| `createLinearGradient` `createRadialGradient` (two circles) | `drawLinearGradient`, `drawRadialGradient` (two circles, the same model) | `LinearGradient`, `RadialGradient` (two-point conical) |
| `shadowColor` `shadowBlur` `shadowOffsetX/Y` | `setShadow(offset:blur:color:)`, the model it came from | a blur in the replayer |
| `fillText` `strokeText` `measureText` | Core Text into the context | glyph outlines (swash) as paths |
| `drawImage` | `draw(_:in:)` | `draw_pixmap` |

The table is why the Apple backend is thin, and why the web's names cost nothing to adopt: they are CG's operations under their standard spellings. CG differs from the canvas model in four places, and each is a replayer rule, not an API change:

- **Coordinates.** CG's origin is at the bottom left. The replayer flips the bitmap context once, as the Apple host already does for text rasters.
- **`arcTo` with a zero radius, and non-finite arguments.** The spec says a method with a non-finite argument does nothing. The validator drops such calls before a host sees them (D3).
- **Compositing extent.** Canvas applies `source-in`, `source-out`, `destination-in`, `destination-atop` and `copy` to the whole canvas: pixels outside the shape are cleared. CG applies a blend mode only under the shape. WebKit resolves this with a transparency layer over the clip. Stage 2 does the same (§3).
- **Shadow blur.** Canvas's `shadowBlur` is twice the Gaussian's standard deviation. CG's blur parameter is not the same quantity. The replayer converts it, and the parity band covers the residue.

## 2. Decisions

### D1. Who writes the drawing code: the data module, in either language

A 2D surface is a function in the app's data module, TypeScript or Rust, beside its `answer` and `parse` (LLP 1027). Canvas 2D is an imperative JavaScript API, and the data module is the one place an exact2 app runs imperative code. Contract has no loops by design (LLP 1027 §2), and a chart is a loop over its points.

**TypeScript.** The module exports one more function:

```ts
// app.d.ts (generated): Ctx2D is Pick<CanvasRenderingContext2D, …the v1 members (§3)…>,
// so every name, signature and default comes from TypeScript's own lib.dom.
export function draw(surface: string, args: unknown[], ctx: Ctx2D, frame: Frame): boolean {
  if (surface !== "spark") return false;
  const [points, up] = args as [number[], boolean];
  ctx.clearRect(0, 0, frame.width, frame.height);
  ctx.strokeStyle = up ? "#16a34a" : "#dc2626";
  ctx.lineWidth = 1.5; ctx.lineJoin = "round"; ctx.lineCap = "round";
  ctx.beginPath();
  points.forEach((y, i) => i ? ctx.lineTo(9 + 96 * i / 47, 9 + y) : ctx.moveTo(9, 9 + y));
  ctx.stroke();
  return false; // no further frame wanted
}
```

`Frame` is `{ time, mounted, width, height }`. `time` is the frame's timestamp in milliseconds, as `requestAnimationFrame` passes it (D5). `mounted` is the time this canvas node was created. `width` and `height` are the content box in CSS pixels (D6). The surface roster comes from a `surfaces` export (name and arity), which the compiler checks against `canvas surface=` arity, as it checks a GPU module's roster (LLP 1009 D2).

**Rust.** The data crate implements the same function against `exact_canvas::Context2d`. Its methods are web-sys's names for the same IDL members: `begin_path`, `line_to`, `set_stroke_style_str`, `set_line_width`, `fill_rect`, `set_global_alpha`, and so on. A drawing routine written for web-sys ports by changing the type. `exact-canvas` is a leaf crate with no dependencies. Data crates depend on it; it depends on nothing above it.

**Contract: no drawing language.** SVG is the declarative drawing model, and it is admitted complete (LLP 1055). A second declarative one would be a second set of bugs. A Contract `canvas` names a surface and passes arguments, and that is all it says about drawing.

**Not chosen:**
- **The GPU module as the 2D home.** It works: the crypto bench's GPU app draws its sparklines in wgpu. But it costs a Metal layer and a wgpu surface per canvas. It loads a second artifact of 1.7 MiB natively, and its rows cannot pool (§6). A chart does not need a GPU.
- **Drawing in the host.** A Rust data crate is linked into the native hosts and could, in principle, draw straight into a `CGContext`. That would need a per-host drawing API, which is what this RFC exists to avoid.

### D2. One `canvas` tag, and the code chooses the context

The web has one `<canvas>` element. The context is chosen when code calls `getContext("2d")` or `getContext("webgpu")`. exact2 already has the element (LLP 1009 D3), its 300 × 150 default, its children (LLP 1014) and its `surface=` attribute. What corresponds to `getContext` is where the surface is registered:

- a surface in a **GPU module's** roster is a `webgpu` context (LLP 1009, unchanged);
- a surface in the **data module's** `surfaces` roster is a `2d` context (this RFC).

The compiler refuses a name in both rosters, and a name in neither. No `context=` attribute exists, because the web has none: an author who wanted a different context would be writing the code, not the markup. `state` reports each canvas's context (§5).

A 2D canvas never loads the GPU module. The two share the node, the box, the children overlay (LLP 1014 D2: children composite over the surface on every host), the per-canvas failure reporting (LLP 1009 D2: a failing surface leaves the box's background) and the on-screen rule (D4).

Not taken over for 2D: `wants_children` (a 2D surface cannot read its children's pixels, which is HTML-in-Canvas's `drawElementImage`; refused in §3), and 1009's `Custom(u16)` property extension point, because a 2D draw reads its frame time directly.

### D3. Immediate calls into a kept bitmap, carried as a recorded list

**The canvas model is kept.** A web canvas is a bitmap. Drawing calls change it immediately, and it keeps its pixels until they are drawn over, cleared, or the canvas is resized. exact2 keeps exactly that. Each 2D canvas has a bitmap. A `draw` call's operations apply on top of what is there. The bitmap is cleared to transparent black on creation and whenever its size or device scale changes; that is the web's rule for setting `width`.

**The calls are recorded where they run and replayed where the pixels are.** The context handed to `draw` is a recorder. Each call appends an opcode and its operands to a flat buffer: a `Float64Array` in TypeScript, a `Vec` in Rust. Strings (colours, fonts, text) go into a side table. When `draw` returns, the buffer crosses the data seam as bytes. The runner validates it in `exact-canvas`, which does the following:

- enforces the size bound (1 MiB per draw; a larger list refuses that draw, and `state` reports it);
- checks opcodes and operand counts;
- drops calls with non-finite arguments, as the spec says a method must;
- parses colour and font strings with the kernel's CSS parsers, ignoring an invalid assignment as the spec does;
- keeps state setters in order.

The validated list goes on the batch as a `canvas2d` op (node, list). The host replays it into the bitmap. The web host replays into the real `CanvasRenderingContext2D` of the canvas's `<canvas>` element.

**Why a recorded list everywhere:**
- **One path.** The web runs the same list as every native host, so Chrome is the oracle for the list and not for a different code path. A recording also crosses to a worker (D4) and to the Rust runner as plain bytes.
- **Inspection.** The agent can read what was drawn (§5).
- **Determinism.** A canvas's pixels are a function of the lists it received.
- **Cost.** Recording 177 numbers for a sparkline takes 3.8–4.0 µs in Hermes on this Mac (§6).

**Getters and queries answer in the executor.** `ctx.fillStyle` returns the serialised colour, `ctx.getTransform()` the current matrix, and `ctx.measureText()` metrics. For that the recorder tracks its own state stack and matrix, and it answers colour serialisation and `measureText` through a synchronous host function on the executor. Hermes's host call is 45 ns (`metrics/ibex2-speed.jsonl`), and the browser's executor uses a scratch `OffscreenCanvas` context for the same answers. `measureText` must use the host's text engine (D8). Otherwise a label measured in the executor would not fit the label drawn by the host.

**Not chosen:**
- **Calling the browser's context directly from TypeScript on the web.** It saves the recording on one host and costs an unshared path, a web-only behaviour for worker placement, and the agent's view of the list. It can come back as a measured optimisation, but not as a second semantics.
- **A retained scene graph.** That is SVG.

### D4. Where the draw runs, when, and on what thread

**`draw` runs in the module's executor.** That is the page's JavaScript or Hermes on the runner's thread by default, or the module's worker when the manifest places it there (LLP 1027.002). **Replay runs on the host's presenting thread:** the main thread on Apple and the web, and the painter on Linux.

**The runner calls `draw` after layout, in the same turn,** when one of these happens:
1. the canvas node is created;
2. its surface arguments change (they are evaluated with the node's bindings, as LLP 1009 D2's are);
3. its content box or device scale changes (the bitmap was cleared);
4. it asked for another frame, and the host presents one while it is on screen (D5);
5. an image it drew became decoded (D9).

Nothing else causes a draw. Size is known because the kernel has just laid out the box. The list reaches the host in the batch that mounts or changes the node. So on `main` placement a canvas is never presented without its first drawing: its first draw is part of its row's build (§6).

On `worker` placement the runner sends the call and commits the list when it returns, which is at least one turn later. A worker-placed canvas shows its bitmap's previous contents (transparent on first mount) for at least one frame. That is declared, as it is on the web with an `OffscreenCanvas` in a worker.

**On screen only, for frames.** A canvas that asked for another frame is drawn only while the host judges it on screen. The GPU canvas learned this in the crypto bench (`GAPS.md` §1): mounted off-screen rows rendering every frame doubled the CPU. The request is held, and it is honoured on the first frame the canvas is seen. The first draw (reasons 1–3, 5) happens whether or not the canvas is on screen, because a row built in overscan must arrive drawn.

**Apple: the main thread, into a bitmap context.** Each 2D canvas view owns a `CGContext` bitmap (premultiplied BGRA, sRGB, device pixels) and a plain `CALayer` whose `contents` is `makeImage()` of it after each replay. The image is copy-on-write, so the next replay copies only if the layer still holds the old image. Main-thread replay keeps a batch atomic: the canvas's new pixels appear in the same `CATransaction` as the row's new text. It costs tens of microseconds per sparkline (§6). A background replay queue is not in v1. It is the stage-4 option if a fixture shows replay over a millisecond on the main thread. `CGLayer` is not used: it is an offscreen cache for repeated drawing inside one context, and Apple has recommended against it since 2011.

**Web: the glue, loaded with the first 2D canvas.** `canvas2d-glue.js` (the replayer, about 300 lines) is injected after the first paint, when the first 2D canvas mounts, as the GPU glue is (LLP 1009 D4). An app with no 2D canvas never fetches it (LLP 1047).

**Linux: a pixmap per canvas.** The CPU painter composites it. The GPU (vello) painter uploads it as an image brush, as SVG islands are drawn (LLP 1055.000 D2). Retained regions snapshot the pixmap.

### D5. Time, animation and the agent's clock

`draw` returns `true` to ask for another frame. That is `requestAnimationFrame` without a global: data modules have no timers (LLP 1027). The host serves the request from its existing frame source (the display link on Apple, `requestAnimationFrame` on the web, the Linux frame loop) under LLP 1009 D4's `canvas` batch signal. It passes the host's presentable clock as `frame.time`. Under an agent-owned clock that is the agent's time.

- **`clock +N` and `clock settle` do not wait for canvases.** A canvas has no settle time: whether it will ever stop asking is its code's business. When the agent moves the clock, each on-screen canvas that asked for a frame is drawn once, at the new time. `state` reports it as `animating` while it keeps asking. This is how LLP 1055 treats infinite CSS animations. It is deterministic for a canvas that clears and redraws from `frame.time`, which is the normal pattern.
- **Accumulating canvases are frame-rate dependent.** A canvas that draws a trail without clearing depends on how many frames it was given. That is true on the web too, where rAF is throttled in a background tab. It is declared, not fixed.
- **`frame.mounted`** replaces the state a web page keeps in a closure for "animate from when this appeared". The sparkline's 600 ms draw-in is `min(1, (time − mounted) / 600)`, with no module state. A draw function should depend only on its arguments and its frame. A dev reload re-creates every canvas and calls `draw` again (LLP 1007 §6), so state kept in module variables is lost, as it is on a page reload.

### D6. Coordinates and resolution

The context's coordinate space is the canvas's content box in CSS pixels. The bitmap is at device pixels (the layer's scale on Apple, `devicePixelRatio` on the web). The replayer pre-multiplies the scale, and `getTransform()` does not include it. On the web every author writes `canvas.width = w * dpr; ctx.scale(dpr, dpr)` by hand. exact2 does it for them because the canvas's size is a CSS box (LLP 1009 D3), not a bitmap attribute.

This is a declared deviation:
- no `width`/`height` bitmap attributes;
- `getTransform()` is identity after `resetTransform()`;
- lines are as crisp on a 3× phone as on the web page of an author who did the scaling.

Context attributes are fixed in v1: `alpha: true`, `colorSpace: "srgb"`, `willReadFrequently: false`, `desynchronized: false`. `"display-p3"` is refused until a consumer draws wide colour.

### D7. Per-host backends

| | Web | Apple (iOS, macOS) | Linux |
|---|---|---|---|
| Executor of `draw` | the page (or its Worker) | Hermes or the Rust data crate, on the runner's thread (or the module worker) | the same |
| Replayer | `canvas2d-glue.js` into `CanvasRenderingContext2D` | Swift `Canvas2D.swift` into a `CGContext` bitmap, then `CALayer.contents` | Rust into a tiny-skia `Pixmap` |
| Text | the browser | Core Text (`CTLineDraw`), fonts by LLP 1019 | cosmic-text shaping, swash outlines as paths (LLP 1055.000's SVG text) |
| Images | `ImageBitmap` from the host's decoded image | the host's decoded `CGImage` | the host's decoded pixmap |
| Oracle | itself (Chrome) | Chrome, within §4's band | Chrome, within §4's band |

**Apple replays in Swift.** CG is a Swift-shaped C API. The Apple host's Rust is `#![deny(unsafe_code)]`, and calling CG from Rust would add an audited `unsafe` surface for every call. The list is validated in Rust first, so the Swift reader trusts its structure and reads it with one bounds-checked pass over an `UnsafeRawBufferPointer`. The typed-reader lesson from `QUEUE.md` applies: an SVG scene that crosses as generic JSON costs 5% of the main thread in a scrolling list. The canvas list is binary from the start.

**Why Core Graphics and not tiny-skia on Apple.** LLP 1055.000 asks the same question for SVG islands. The answer for Canvas 2D is Core Graphics:
- **Speed.** On this Mac (M5 Max) the crypto sparkline takes 28.6 µs in Core Graphics against 133.5 µs in tiny-skia 0.12 at 2×, and 37.6 against 215.9 µs at 3×, in the same drawing loop (probes in §6).
- **Text.** Core Text draws into the same context, with the same fonts and rendering as every other label in the app.
- **It is Charlie's model.** It is the one the API was made from.

The cost is a second rasterizer beside Linux's, so Apple and Linux differ from each other by antialiasing shades. Each host is held to Chrome, not to the other.

**Why tiny-skia and not vello on Linux.** A canvas is a bitmap that keeps its pixels (D3). vello renders a scene each frame and has no kept target, no unbounded compositing operators and no blur. tiny-skia is already the Linux host's CPU painter and its oracle. It is slower (above), but Linux is the headless and DRM host, not the list-performance host.

### D8. Text

`fillText`, `strokeText` and `measureText` go through the host's own text engine, not a canvas-private one. A canvas axis label then matches the `text` node beside it, in font, fallback and hinting. The `font` property is the CSS `font` shorthand, parsed once by the kernel's parser. Family names resolve as box text resolves them, with LLP 1019's declared fonts first.

`measureText` answers in the executor through the host function (D3). It returns `width` and the six bounding-box metrics (`actualBoundingBox*`, `fontBoundingBoxAscent/Descent`). The remaining `TextMetrics` baselines come in stage 2. `textAlign`, `textBaseline` and `direction` are in v1. `letterSpacing`, `wordSpacing`, `fontKerning`, `fontStretch`, `fontVariantCaps` and `textRendering` come in stage 2. `maxWidth` compresses horizontally, as the spec says.

The parity band for text is looser than for shapes (§4), because Chrome's system font differs from a host's unless the app declares its font (LLP 1019). A declared font is the tight case.

### D9. Images, pixels and paths

**`drawImage`** takes an image handle, never an element. The handle is an `image` argument of the surface: a URL or asset path, the same value an `image` node's `src` takes. The host resolves it through its decoded-image cache under LLP 1010 §6's image budget.

A handle not yet decoded draws nothing, as an incomplete `HTMLImageElement` draws nothing on the web. When it decodes, the runner calls `draw` again: reason 5 in D4. That is the web page's `img.onload = draw`, made automatic. `ImageData` is also accepted.

**`createImageData` and `putImageData`** are in v1. The pixels travel in the list and count against the 1 MiB bound. **Readback is refused:** `getImageData`, `toDataURL` and `toBlob`. The executor is not where the pixels are. A synchronous read would need the host to replay and read back inside the draw, which puts the rasterizer in the executor's turn. The agent reads pixels with `screenshot`, which is unchanged. The trigger to revisit is a consumer that needs its own pixels, for example a flood fill or a colour picker.

**`Path2D`** is in stage 2, including `new Path2D(svgPathData)`, parsed by the kernel's SVG path parser (`kernel/src/svg/`). SVG's `d` grammar and the canvas's must agree, and here they are one parser. `isPointInPath` and `isPointInStroke` answer in the executor from geometry, through the same host function family. They are in stage 3, with pointer coordinates (§8).

### D10. Lists: pooling, the fill policy and memory

- **Pooling.** A 2D canvas row is a plain view with a layer and a bitmap. `NodePoolIOS` gains the `canvas` kind for 2D canvases only; a `metal` view stays ineligible. On reuse the layer's contents are cleared and the bitmap is kept if the new node's size matches, and freed otherwise. A pooled canvas starts transparent, as a new web canvas does. The kept buffer saves an allocation, not a draw. Freed bitmaps go to a size-keyed free list, bounded to one viewport's canvases, and dropped under memory pressure, as shaped text is (`1ced26af`).
- **The fill policy (LLP 1050.000).** A canvas's first draw is part of its row's build (D4), so a row owed under `complete` is presented drawn. Its cost counts toward D3's "a row known to cost more than a frame". An `auto` row that is left pending builds its canvas when it is built, never separately. Frame requests are never owed: an animating canvas that misses a frame shows its previous frame, not a blank.
- **Memory is O(mounted canvases).** Bitmaps exist only for mounted canvas nodes, plus the bounded free list. They are not charged to the decoded-image budget, which is for images the app names.

## 3. The v1 subset, the later stages, and what is refused

**Stage 1 (v1):**
- **State:** `save`, `restore`, `reset`.
- **Transforms:** `translate`, `rotate`, `scale`, `transform`, `setTransform` (six numbers or a `DOMMatrix2DInit`), `resetTransform`, `getTransform`.
- **Paths:** `beginPath`, `moveTo`, `lineTo`, `quadraticCurveTo`, `bezierCurveTo`, `arc`, `arcTo`, `ellipse`, `rect`, `roundRect`, `closePath`.
- **Painting:** `fill(fillRule)`, `stroke()`, `clip(fillRule)`, `fillRect`, `strokeRect`, `clearRect`.
- **Line style:** `lineWidth`, `lineCap`, `lineJoin`, `miterLimit`, `setLineDash`, `getLineDash`, `lineDashOffset`.
- **Fill and stroke style:** colour strings, and `CanvasGradient` from `createLinearGradient` or `createRadialGradient` with `addColorStop`.
- **Compositing:** `globalAlpha`, and `globalCompositeOperation` limited to `source-over` and the separable and non-separable blend modes (`multiply` … `luminosity`), which touch only covered pixels on every backend.
- **Text:** `fillText`, `strokeText`, `measureText` (D8's subset), `font`, `textAlign`, `textBaseline`, `direction`.
- **Pixels:** `createImageData`, `putImageData`.
- **Smoothing:** `imageSmoothingEnabled`, `imageSmoothingQuality`, for `putImageData` scaling and the stage-2 `drawImage`.

Every default is the spec's. For example, `fillStyle` and `strokeStyle` are `#000000`, `lineWidth` is 1, `lineCap` is `butt`, `lineJoin` is `miter`, `miterLimit` is 10, and `font` is `10px sans-serif`. A web author's expectations hold without a reset.

**Stage 2:**
- `drawImage` (D9) and `Path2D` (D9);
- `createPattern`, `createConicGradient`;
- shadows (`shadowColor`, `shadowBlur`, `shadowOffsetX/Y`, with D1's blur conversion);
- the unbounded composite operators (`source-in`, `source-out`, `destination-in`, `destination-atop`, `copy`, `xor`), using a transparency layer over the clip on Core Graphics as WebKit does;
- the remaining `TextMetrics` fields and text properties (D8).

**Stage 3:** `isPointInPath` and `isPointInStroke`, with pointer coordinates in the canvas's handlers (a chart's tooltip). These pair with LLP 1014.000's input seam, which today serves GPU surfaces only.

**Refused by name, each with its trigger:**

| Refused | Why | Trigger to revisit |
|---|---|---|
| `getImageData`, `toDataURL`, `toBlob` | readback in the executor (D9) | a consumer that reads its own pixels |
| `ctx.filter` | Core Graphics has no CSS filter chain. LLP 1055.000's islands (`exact-svg-raster`: tiny-skia with the filter primitives) are the one place filters will be implemented | those islands landing: `ctx.filter` then shares them |
| `drawFocusIfNeeded`, `scrollPathIntoView` | focus and scrolling belong to boxes, not pixels; the canvas node is focusable as a box | none planned |
| hit regions (`addHitRegion`, removed from the spec) | removed from HTML itself | never |
| `ctx.canvas`, `getContext`, `OffscreenCanvas`, `transferControlToOffscreen` in app code | the app never holds an element. The manifest's `worker` placement is exact2's `OffscreenCanvas` (D4) | none |
| `drawImage` of a canvas, video or element | cross-surface pixels; HTML-in-Canvas `drawElementImage` | a consumer, with LLP 1014's capture as the route |
| `colorSpace: "display-p3"`, `alpha: false`, `willReadFrequently`, `desynchronized` | D6 | a wide-colour consumer |
| `createImageBitmap` in app code | images are handles the host decodes (D9) | none |

## 4. Parity with Chrome

The instrument is LLP 1055.000's SVG parity comparator (`scripts/svgparity.mjs`), generalised to take its app and page names rather than copied. Chrome draws each fixture from the same Contract and the same list. Each native host's fixture is cropped by its own layout, brought to one pixel per point, registered within ±3 px, and compared.

- **Shapes, gradients and compositing:** mean |Δ| ≤ 4/255 and at most 6% of pixels off by more than 32, which is the SVG band. It is measured, not assumed: antialiasing differs between Skia, Core Graphics and tiny-skia by a shade on edges.
- **Text:** the looser SVG text band (mean 14, 16%) on Linux with its pinned font. A fixture with a declared font (LLP 1019) is held to the shape band on Apple and the web.
- **Shadows (stage 2):** a band measured when they land, because the blur conversion (§1) is approximate on Core Graphics.
- **Accumulation and time:** fixtures draw at settled clock times from a cleared canvas (D5), so every host draws exactly one list per fixture.

## 5. The agent

No operation is added. The eight suffice:
- `tree` shows a 2D canvas as a `canvas` node with `context: "2d"`.
- `state` gains one record per canvas: surface, context, the last list's byte and operation counts, whether it asked for another frame (`animating`), the time of its last draw, and any refusal (unknown surface, arity, a list over the bound, a thrown exception in `draw`, which is reported per canvas and leaves the previous bitmap).
- `layout <node>` (LLP 1035) includes the last list decoded into readable calls, capped at 200 lines, in development builds only. An agent can then confirm "stroke, 48 points, #16a34a" without reading pixels.
- `screenshot` is unchanged. On macOS it shows the layer's contents, which are model values, so no `window` mode is needed.
- `clock` advances canvases as D5 says.

## 6. Performance and memory: a sparkline per row

**Measured on this Mac today** (Apple M5 Max; release builds; the drawing is the crypto bench's chart: a 114 × 50 pt canvas, a 48-point 1.5 pt round-joined polyline, a ring and a 3 pt dot):

| | 2× (228 × 100 px) | 3× (342 × 150 px) |
|---|---|---|
| Record in Hermes (the `draw` above, 177 numbers; `hermes -O`, vanilla build) | 3.8–4.0 µs | same |
| Core Graphics raster (bitmap context) | 28.6 µs | 37.6 µs |
| Core Graphics raster + `makeImage()` | 42.7 µs | 56.4 µs |
| Core Graphics, draw-in frames (partial polyline) | 17.3 µs | 23.2 µs |
| tiny-skia 0.12 raster | 133.5 µs | 215.9 µs |
| Bitmap per canvas | 89 KiB | 200 KiB |

The probes are recorded in the appendix (§11) so that stage 3 can rerun them on the iPad.

**What exists to compare with** (M1 iPad Pro 12.9″, 120 Hz, `~/bench/cryptobench`, medians of three interleaved rounds):

| App | rest: CPU ms/s (main) | rest: memory end | fling: fps, late/1k | notes |
|---|---|---|---|---|
| SwiftUI | 381 (322) | 22.9 MB | 79.5, 135 | `ipad-r2` |
| Expo + Skia `Canvas` per row | 857 (398) | 112 MB | 105.8, 127 | `ipad-r2` |
| exact2 GPU `canvas` per row (LLP 1009) | 879 (359) | 116 MB | 108.0, 96 | `ipad-r1`; rows never pool (`GAPS.md` §2) |
| exact2 SVG (LLP 1055) | 119 (114) | 53.5 MB → **38 MB** after perf/crypto-svg | 118.4, 13 | `ipad-r2`; 38 MB from `QUEUE.md` |

**Estimated cost of a Canvas 2D sparkline per row on iOS** (the M1 iPad is taken as about 2–2.5× slower than this M5 Max on this single-threaded work; stage 3 replaces every estimate with a measurement):

- **Memory.** 89 KiB per mounted canvas at 2× (the iPad), 200 KiB at 3× (an iPhone). The bench holds about 46 rows mounted at rest (`QUEUE.md`), which gives 4.1 MB on the iPad or 9.2 MB on a 3× phone. The copy-on-write image adds a transient copy only during a replay. The estimated rest footprint is the SVG app's 38 MB plus about 4 MB of bitmaps, **about 42 MB**. That compares with 116 MB for the GPU canvas, 112 MB for Skia and 23 MB for SwiftUI.
- **CPU, drawing only when data changes.** A tick changes 40 of 5,000 coins, so about 0.2 visible charts per tick. The chart costs about 0.1 ms per changed row. At rest that is close to the SVG app. Its pulse runs in Core Animation if the pulse is a **canvas child** with a CSS `@keyframes` animation (LLP 1014 D2 and LLP 1055), which is how exact2 would write a pulsing dot over a drawing.
- **CPU, the pulse drawn in the canvas every frame** (what a web developer might write with rAF): 21 visible canvases × about 120 µs (record, crossing and raster on the M1) × 120 Hz is **about 300 ms/s on the main thread**. That is below the GPU canvas's total (879 ms/s) and above SVG's (119). The 300 ms/s would move off the main thread if Apple replay moves to a queue (stage 4). The record and crossing would too, with worker placement.
- **Fling.** Only on-screen canvases draw per frame (D4). At 24,000 pt/s a row is on screen for about 43 ms, so the per-frame load is the same 16–21 canvases. What rises with speed is mounting: about 375 rows/s × (record + first raster ≈ 0.12 ms) ≈ 45 ms/s, plus the row itself. That compares with the GPU canvas's `MetalView`, `CAMetalLayer` and `gpu_create` per row, which the ladder showed retaining 824 MB by 96k pt/s.

**The conclusion stage 3 must test.** Canvas 2D should cost a list about what SVG costs, plus a small bitmap per row, when drawing follows data. It costs a predictable, bounded amount of main-thread time per visible canvas when drawing follows the clock. It is not in the GPU canvas's cost class. SVG remains the better tool for a pulse on thousands of rows, because Core Animation runs it without waking the app. Canvas 2D is the better tool when the drawing is computed.

## 7. What it costs to carry

- **`exact-canvas`:** the recorder, the list format and the validator. Estimated at 1,200 lines, with no dependencies. It is linked by data crates that draw and by the runner. Per LLP 1047 it is a linked capability: in an app's artifact only if its plan names a 2D surface.
- **TypeScript:** a recorder of about 400 lines, bundled into the module only when the module exports `draw`. The generated `Ctx2D` type is a `Pick` over `lib.dom`, with no copy of the IDL.
- **Web:** `canvas2d-glue.js`, about 300 lines, fetched after first paint with the first 2D canvas.
- **Apple:** `Canvas2D.swift`, about 600 lines (the replayer, the text calls and the bitmap lifecycle), under the 1,500-line cap. No new dependency.
- **Linux:** a replayer of about 600 lines over tiny-skia, which it already links, and the SVG text outlines.
- **No new blocking check.** The parity smoke runs asynchronously, as `smoke.mjs svg` does.

## 8. Staging, the fixture and the parity smoke

| Stage | Ships | Proven by |
|---|---|---|
| 1 | `exact-canvas` (recorder, format, validator); the TS and Rust `draw` exports and roster; the runner's calls (D4) and `canvas2d` op; replayers on all three hosts; §3's v1 subset including text; `state` and `layout` (§5); on-screen frame requests | unit tests of the validator against spec edge cases (non-finite arguments, invalid colours ignored, `save`/`restore` balance); `apps/canvas-gallery`'s fixtures under the parity smoke on web, macOS, iOS and Linux |
| 2 | `drawImage`, `Path2D`, patterns, conic gradients, shadows, unbounded composite operators, the remaining text properties | gallery pages per item, same smoke |
| 3 | pooling 2D canvas rows on iOS; the fill-policy accounting (D10); `isPointInPath`/`isPointInStroke` and pointer coordinates; a fifth crypto bench app (`exact-canvas2d`, outside this repo, as the others are), with a pulse variant in-canvas and as a canvas child | the iPad series beside SwiftUI, Skia, GPU and SVG; §6's estimates replaced |
| 4 | only if measured: Apple replay off the main thread; a baked first frame for a fixed-size canvas whose arguments settle at bake (the list is data, so the bake can record it); web replay in the Worker through `OffscreenCanvas` | a fixture showing main-thread replay over 1 ms, or a blank first frame worth removing |

**The fixture and the demo.**
- **`apps/sparkline` gains a canvas chart beside its SVG chart:** the same row, a `chart` toggle, and the same Freeze and Tick. That is the side-by-side demo: one Contract row, two drawing models, and Chrome as the reference for both.
- **`apps/canvas-gallery`** holds the parity fixtures, one `fx-*` canvas per behaviour, modelled on `apps/svg-gallery`:
  - caps and joins;
  - dashes;
  - arcs and `arcTo`;
  - gradients;
  - blend modes;
  - clip with both rules;
  - the state stack;
  - text alignment and baselines;
  - `putImageData`.
- **The smoke** is `bun scripts/smoke.mjs canvas`, the SVG smoke's comparator run over the gallery. Adding the app and the smoke mode is apparatus. `rules/RULES.md` needs Charlie to say so, and §10 Q4 asks.

## 9. `rules/NOT-DOING.md`: the admission for Charlie to rule on

Canvas 2D is not admitted today. The GPU canvas came in through LLP 1009 and 1014, and SVG through LLP 1055. Drafted for §Components, after the SVG entry:

> **Expanded (Charlie, 2026-09-27: "Core Graphics everywhere", by the web's name):** the HTML Canvas 2D context on the `canvas` tag (LLP 1056). A surface in the app's data module, TypeScript or Rust, draws with `CanvasRenderingContext2D`'s own names. Its recorded calls are replayed by the browser, Core Graphics or tiny-skia into the canvas's kept bitmap. Unblocks computed 2D drawing (charts, sparklines, custom controls) on every host, without a GPU module, with Chrome as the oracle. Take: *[the take Charlie names]*. Still refused: a drawing language in Contract (SVG is the declarative one), readback (`getImageData`, `toDataURL`, `toBlob`), `ctx.filter` until SVG's islands exist, and an app-visible `OffscreenCanvas`.

**The take.** The rule is to take something off the doing-list in the same change. The candidates, in the order recommended:

1. **Recommended: Caltrain's line map moves from wgpu to Canvas 2D.** Its `map` surface (`apps/caltrain/gpu/src/lib.rs`, about 250 lines of Rust, and `shaders/map.wgsl`) is a 2D line drawing. It becomes a `draw` in Caltrain's data crate, and the wgpu surface and shader are deleted. That gives Canvas 2D a consumer inside the v1 bar (Caltrain defines v1). It also leaves fewer GPU paths after than before, which is the Snapback admission's shape. The GPU module stays for the aurora, glass and deck, which are genuinely shaders.
2. **LLP 1009's `Custom(u16)` animatable-property extension point** (decided and never built) comes off. A 2D surface reads its frame time directly (D5), and nothing else has asked for it.
3. **Pooling GPU-canvas rows** (`QUEUE.md`, "Pooling a list row that holds a canvas needs a ruling") is closed as won't-do. A chart in a list is Canvas 2D or SVG. This is a queue item, not a doing-list line, so on its own it does not meet the rule.

## 10. Questions for Charlie

1. **Admit Canvas 2D, and name the take.** Recommendation: admit it with take 1, Caltrain's line map moving to Canvas 2D and its wgpu surface deleted, and add take 2 if one is not enough.
2. **Core Graphics on Apple rather than tiny-skia everywhere native.** Recommendation: Core Graphics. It is 4.7× faster on the same drawing here, its text is the app's text, and it is the model you asked for. The cost is that Apple and Linux each match Chrome rather than each other. SVG's islands (LLP 1055.000 §8 Q4) can still choose tiny-skia: they need filters, which Core Graphics lacks.
3. **Readback and `ctx.filter` refused in v1.** Recommendation: refuse both by name, with the triggers in §3. `ctx.filter` returns with SVG's islands, sharing their code.
4. **The fixture apparatus:** `apps/canvas-gallery`, a canvas mode in `apps/sparkline`, and `smoke.mjs canvas` over the generalised SVG comparator. Recommendation: yes. It adds no blocking check, and the comparator is generalised, not copied.

## 11. Appendix: the probes (2026-09-27, Apple M5 Max)

Core Graphics (`swiftc -O main.swift -o probe && ./probe 2 && ./probe 3`):

```swift
import CoreGraphics
import Foundation
// A crypto-bench sparkline: 114x50 pt canvas (chart 96x32 inset 9), 48 points, 1.5 pt round stroke, dot + ring.
let scale = Int(CommandLine.arguments.count > 1 ? CommandLine.arguments[1] : "2")!
let w = 114 * scale, h = 50 * scale
let cs = CGColorSpace(name: CGColorSpace.sRGB)!
var pts: [CGFloat] = []
var v = 100.0
for i in 0..<48 { v += Double((i * 7919) % 13) - 6; pts.append(CGFloat(v)) }
let lo = pts.min()!, hi = pts.max()!
func draw(_ ctx: CGContext, trim: Int, ringR: CGFloat) {
  ctx.clear(CGRect(x: 0, y: 0, width: w, height: h))
  ctx.saveGState()
  ctx.scaleBy(x: CGFloat(scale), y: CGFloat(scale))
  ctx.translateBy(x: 9, y: 9)
  ctx.setStrokeColor(red: 0.086, green: 0.639, blue: 0.29, alpha: 1)
  ctx.setLineWidth(1.5); ctx.setLineJoin(.round); ctx.setLineCap(.round)
  ctx.beginPath()
  for i in 0..<trim { let x = 96 * CGFloat(i) / 47, y = 32 - 32 * (pts[i] - lo) / (hi - lo); if i == 0 { ctx.move(to: CGPoint(x: x, y: y)) } else { ctx.addLine(to: CGPoint(x: x, y: y)) } }
  ctx.strokePath()
  let lx: CGFloat = 96, ly = 32 - 32 * (pts[47] - lo) / (hi - lo)
  ctx.setFillColor(red: 0.086, green: 0.639, blue: 0.29, alpha: 0.25)
  ctx.fillEllipse(in: CGRect(x: lx - ringR, y: ly - ringR, width: 2 * ringR, height: 2 * ringR))
  ctx.setFillColor(red: 0.086, green: 0.639, blue: 0.29, alpha: 1)
  ctx.fillEllipse(in: CGRect(x: lx - 3, y: ly - 3, width: 6, height: 6))
  ctx.restoreGState()
}
let ctx = CGContext(data: nil, width: w, height: h, bitsPerComponent: 8, bytesPerRow: 0, space: cs, bitmapInfo: CGImageAlphaInfo.premultipliedFirst.rawValue | CGBitmapInfo.byteOrder32Little.rawValue)!
let n = 20000
var sink = 0
for _ in 0..<2000 { draw(ctx, trim: 48, ringR: 6) }
var t0 = DispatchTime.now().uptimeNanoseconds
for i in 0..<n { draw(ctx, trim: 48, ringR: 3 + CGFloat(i % 60) / 10) }
var t1 = DispatchTime.now().uptimeNanoseconds
print("draw only: \(Double(t1 - t0) / Double(n) / 1000) us/frame at \(scale)x (\(w)x\(h) px, \(w*h*4/1024) KiB)")
t0 = DispatchTime.now().uptimeNanoseconds
for i in 0..<n { draw(ctx, trim: 48, ringR: 3 + CGFloat(i % 60) / 10); let img = ctx.makeImage()!; sink &+= img.width }
t1 = DispatchTime.now().uptimeNanoseconds
print("draw + makeImage: \(Double(t1 - t0) / Double(n) / 1000) us/frame \(sink > 0)")
t0 = DispatchTime.now().uptimeNanoseconds
for i in 0..<n { draw(ctx, trim: 1 + i % 48, ringR: 3) }
t1 = DispatchTime.now().uptimeNanoseconds
print("draw-in frames: \(Double(t1 - t0) / Double(n) / 1000) us/frame")
```

tiny-skia 0.12 (`cargo run --release -- 2`, then `-- 3`):

```rust
use tiny_skia::*;
use std::time::Instant;
fn main() {
    let scale: f32 = std::env::args().nth(1).map(|s| s.parse().unwrap()).unwrap_or(2.0);
    let (w, h) = ((114.0 * scale) as u32, (50.0 * scale) as u32);
    let mut pm = Pixmap::new(w, h).unwrap();
    let mut v = 100.0f32; let mut pts = vec![];
    for i in 0..48 { v += ((i * 7919) % 13) as f32 - 6.0; pts.push(v); }
    let lo = pts.iter().cloned().fold(f32::MAX, f32::min); let hi = pts.iter().cloned().fold(f32::MIN, f32::max);
    let draw = |pm: &mut Pixmap, trim: usize, r: f32| {
        pm.fill(Color::TRANSPARENT);
        let t = Transform::from_scale(scale, scale).pre_translate(9.0, 9.0);
        let mut pb = PathBuilder::new();
        for i in 0..trim { let x = 96.0 * i as f32 / 47.0; let y = 32.0 - 32.0 * (pts[i] - lo) / (hi - lo); if i == 0 { pb.move_to(x, y) } else { pb.line_to(x, y) } }
        let mut paint = Paint::default(); paint.set_color_rgba8(22, 163, 74, 255); paint.anti_alias = true;
        if trim > 1 { let p = pb.finish().unwrap(); let s = Stroke { width: 1.5, line_cap: LineCap::Round, line_join: LineJoin::Round, ..Default::default() }; pm.stroke_path(&p, &paint, &s, t, None); }
        let ly = 32.0 - 32.0 * (pts[47] - lo) / (hi - lo);
        let ring = PathBuilder::from_circle(96.0, ly, r).unwrap();
        paint.set_color_rgba8(22, 163, 74, 64); pm.fill_path(&ring, &paint, FillRule::Winding, t, None);
        let dot = PathBuilder::from_circle(96.0, ly, 3.0).unwrap();
        paint.set_color_rgba8(22, 163, 74, 255); pm.fill_path(&dot, &paint, FillRule::Winding, t, None);
    };
    for _ in 0..2000 { draw(&mut pm, 48, 6.0); }
    let n = 20000; let t0 = Instant::now();
    for i in 0..n { draw(&mut pm, 48, 3.0 + (i % 60) as f32 / 10.0); }
    println!("tiny-skia draw: {:.1} us/frame at {}x ({}x{} px)", t0.elapsed().as_secs_f64() * 1e6 / n as f64, scale, w, h);
    let t0 = Instant::now();
    for i in 0..n { draw(&mut pm, 1 + i % 48, 3.0); }
    println!("tiny-skia draw-in: {:.1} us/frame", t0.elapsed().as_secs_f64() * 1e6 / n as f64);
}
```

The recorder under Hermes (`~/projects/ibex/tools/hermes-vanilla/hermes -O rec.js`):

```js
// A recording 2D context: ops into a Float64Array, as a native recorder would.
function Rec() { this.buf = new Float64Array(4096); this.n = 0; }
Rec.prototype.op = function (c, a, b) { var q = this.buf, n = this.n; q[n] = c; q[n + 1] = a; q[n + 2] = b; this.n = n + 3; };
Rec.prototype.beginPath = function () { this.op(1, 0, 0); };
Rec.prototype.moveTo = function (x, y) { this.op(2, x, y); };
Rec.prototype.lineTo = function (x, y) { this.op(3, x, y); };
Rec.prototype.stroke = function () { this.op(4, 0, 0); };
Rec.prototype.arc = function (x, y, r) { this.op(5, x, y); this.op(6, r, 0); };
Rec.prototype.fill = function () { this.op(7, 0, 0); };
Rec.prototype.clearRect = function () { this.op(8, 0, 0); };
var pts = []; var v = 100; for (var i = 0; i < 48; i++) { v += (i * 7919) % 13 - 6; pts.push(v); }
var lo = Math.min.apply(null, pts), hi = Math.max.apply(null, pts);
function draw(ctx, r) {
  ctx.n = 0; ctx.clearRect();
  ctx.beginPath();
  for (var i = 0; i < 48; i++) { var x = 96 * i / 47, y = 32 - 32 * (pts[i] - lo) / (hi - lo); if (i === 0) ctx.moveTo(x, y); else ctx.lineTo(x, y); }
  ctx.stroke();
  ctx.beginPath(); ctx.arc(96, 16, r); ctx.fill();
  ctx.beginPath(); ctx.arc(96, 16, 3); ctx.fill();
}
var ctx = new Rec();
for (var i = 0; i < 5000; i++) draw(ctx, 6);
var n = 50000, t0 = Date.now();
for (var i = 0; i < n; i++) draw(ctx, 3 + (i % 60) / 10);
var dt = Date.now() - t0;
print("record: " + (dt * 1000 / n).toFixed(2) + " us/draw, " + ctx.n + " doubles");
```
