# LLP 1056: Canvas 2D — Core Graphics everywhere, by the web's name

**Type:** RFC
**Status:** Accepted (r3: Charlie's rulings of 2026-09-27 on §10, recorded in §0.1 and folded into D4, D6, §9 and §10. r2 folded two blind reviews of r1; dispositions in §0 and in `llp/reviews/1056-canvas-2d.{astra,grok}.md`). Canvas 2D is admitted in `rules/DEFERRED.md` §Components with Caltrain's line map as the take.
**Systems:**
- Data modules: a `draw` export and a `surfaces` roster in TypeScript and Rust, and a module ABI 2 that returns bytes (LLP 1027, 1027.002).
- A new crate, `exact-canvas`: the recorder, the list format, the canvas colour and `font` parsers, and arc geometry.
- Contract: the `canvas` tag's surface rosters.
- Runner: draw requests, generations, acceptance.
- Web host: geometry feedback, and replay into the real `CanvasRenderingContext2D`.
- Apple host: replay into Core Graphics, and pooling.
- Linux host: replay into tiny-skia.
- Agent: `state` and `layout` for a canvas.

**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-27 (r1, r2 and r3)
**Implementer:** Claude (Opus 5.5), stage by stage in §8's order; stage 1 from 2026-09-27 on `feat/canvas2d-stage1`, stage 2 on `feat/canvas2d-stage2` (§8.2).
**Related:**
- LLP 1009: the `canvas` node and wgpu surfaces. D2's roster and publish-after-commit rule and D4's frame ownership are reused here.
- LLP 1014 and 1014.000: canvas children.
- LLP 1055 and 1055.000: SVG, the declarative drawing model, and its rasterizer choices.
- LLP 1027 and 1027.002: data modules, the executor, and worker placement.
- LLP 1002 and 1003: motion and the agent-owned clock.
- LLP 1007 §6: reload.
- LLP 1010 §6 and LLP 1050.000: the collection, pooling and the fill policy.
- LLP 1019: declared fonts.
- LLP 1047: pay for what you use.
- `~/bench/cryptobench/SPEC.md` and `GAPS.md`.
- `rules/DEFERRED.md` §Components.

## Summary

Charlie's wish is "Core Graphics everywhere". The web already has it under another name. Apple designed the canvas element's 2D context for WebKit in 2004, on Core Graphics' model:
- paths, a current transform, and a graphics-state stack;
- clipping and fill rules;
- caps and joins;
- Porter-Duff compositing;
- CG's shadow model.

WHATWG then standardised it. `CanvasRenderingContext2D` is, in effect, Core Graphics as a cross-platform standard, with a conformance suite (WPT's `html/canvas/element`) and a browser to act as the oracle.

This RFC adds that context to exact2's existing `canvas` tag on every host:

- **Who draws (D1).** The app's data module draws, in TypeScript or Rust, against the web's own interface. SVG stays the one declarative drawing model; Contract gains no drawing language.
- **One tag, the code chooses the context (D2).** A surface registered by a data module is `2d`. A surface registered by a GPU module is `webgpu`.
- **The canvas model, kept whole (D3).** A 2D canvas is a context and a bitmap, and both persist between draws, as on the web: pixels, drawing state, the state stack, the clip and the current path. Calls are checked when the author makes them, with the spec's ignore, no-op and throw rules, by a recorder in the executor. The recorded list is replayed in order into the host's bitmap. On the web the replay target is the real `CanvasRenderingContext2D`.
- **A protocol, not just a list (D4).** Every draw is stamped with the canvas's lifetime, its size generation and a sequence number. An executed list is applied in order or its generation is abandoned whole, so the executor's context state and the host's never diverge. The host reports each canvas's content box: the kernel's layout natively, `ResizeObserver` on the web.
- **Time (D5).** `draw` receives the frame time and returns whether it wants another frame. Under an agent-owned clock that time is the agent's. `clock` does not wait for a canvas to stop animating, but it does wait for the draws due at the landed time.
- **Backends (D7).** The browser on the web. Core Graphics in Swift on the main thread on Apple (4.7× tiny-skia's raster speed on the same sparkline here). tiny-skia on Linux, under both painters. Text goes through each host's text engine, and measurement is local to the executor.
- **Stages (§8).** Stage 1 is shapes and the protocol, and it moves Caltrain's line map off wgpu. Stage 2 adds text, images, `Path2D`, shadows and the whole-clip compositing operators. Stage 3 adds lists, pooling and the benchmark.

**The list case (§6).** A 2D canvas row is one plain layer with a bitmap: 89 KiB at 2×. It pools like a text row, where a GPU canvas row has a Metal layer and wgpu surface and does not pool. The illustrative figures (single-buffer subtotals and modelled CPU, not measurements; stage 3 measures them) are:
- **Drawing only when data changes:** close to the SVG app's cost, plus 4–8 MB of bitmaps.
- **Pulse drawn every frame:** about 230–300 ms/s more main-thread time, which puts the main thread near the GPU canvas's and far below its total CPU and memory.

**Ruled (r3):** Canvas 2D is admitted with Caltrain's line map as the take (§9), and is built in §8's order. Charlie's rulings are in §0.1.

## 0. Disposition after review (r2)

Astra (`gpt-6-astra`, reasoning max) and Grok (`grok-4.6`, xhigh) reviewed r1 (`2a81868d`) blind, from the same brief. Both kept the direction: data modules draw, one tag, a recorded list into a kept bitmap, Core Graphics in Swift on the main thread, tiny-skia on Linux, no ninth agent operation, and Caltrain's map as the take. Both said "build with named changes". Each finding and where r2 answers it:

| Finding | Who | r2 |
|---|---|---|
| Validation after `draw` returns is too late: canvas has member-specific ignore, no-op and throw rules (negative radii, `addColorStop`, negative `lineWidth`, `setLineDash` with a bad element) that `try/catch` and getters must observe | both | D3: the recorder applies the spec's argument rules at the call; the runner's check is structural only |
| Drawing state, stack, clip and current path persist across draws; r1 said only pixels | both | D3: the whole context persists; the recorder owns the authoritative state |
| An executed list dropped or applied out of order desynchronises the recorder from the host | Astra | D4: lifetime, size generation and sequence stamps; coalescing happens before execution, never after |
| On the web the browser lays out, so "after kernel layout" does not hold | Astra | D4: the host reports content boxes (web: `ResizeObserver` with `device-pixel-content-box`) |
| r1's first-draw promise contradicts post-pixel activation and the web's deferred module turns | both | D4: a canvas is transparent until its executor is active and its first list arrives. Same-turn first draws are claimed only for native `main` placement |
| Synchronous `measureText` from a worker into the UI thread is the deadlock 1027.002 D4 forbids; the web module realm lacks the page's fonts | both | D8: measurement is executor-local, with the same declared faces loaded where the executor runs |
| The kernel colour parser lacks named colours and `hsl()`; no `font` shorthand parser exists; "dependency-free" and "uses kernel parsers" contradict | both | D3 and §7: `exact-canvas` owns new canvas colour and `font` parsers; it stays a leaf |
| Hermes's call seam is strings; `draw` and bytes need a module ABI 2 | both | D1: ABI 2 adds `draw`, `surfaces` and a byte result |
| `ImageData` is not a `drawImage` source; `putImageData` is raw backing pixels and ignores smoothing, transform, clip, alpha and compositing | both | D6 and D9: raw device pixels; `Frame` carries `pixelWidth`, `pixelHeight` and `scale`; the bound counts pixel payloads separately |
| `CanvasGradient` and `Path2D` are live objects | both | D3: objects are ids, and their mutations are ordered operations in the list |
| CG consumes the path on fill and clip, and canvas keeps it; Y-flip inverts `arc`'s direction | Astra; Grok | §1: the replayer keeps its own path and inverts `clockwise` under the flip; `exact-canvas` owns f64 arc geometry (the SVG routine is f32 and skips equal endpoints) |
| The whole-canvas operators are bounded by the clip; `xor` is not one of them; four Porter-Duff operators were missing | both | §3: complete inventory. Stage 1: all bounded operators and blend modes. Stage 2: the five clip-extent operators |
| Shadows ignore the current transform | Astra | §3 stage 2: offsets and blur in canvas pixels × device scale, never × the author's transform |
| Pooling is blocked by the overlay and container rules; `mounted` must reset; clear the backing pixels | both | D10: 2D children are ordinary subviews (no overlay); a reset contract; `mounted` is the new node's |
| Bitmap size, persistent paths and stack depth are unbounded | both | D4: limits, each refused through `state` |
| Error policy inconsistent (background vs previous bitmap) | both | D4: one policy, atomic draws, declared as a deviation |
| Fill-policy "known cost" does not exist for a first draw | Grok | D10: the first draw always runs; its measured cost is kept per surface |
| The shared list makes the glue part of the oracle | both | §4: a direct-API page draws the same calls on Chrome without the recorder; a WPT subset runs against the recorder |
| The SVG comparator hides alpha and coordinate errors; accumulation fixtures clear first | Astra | §4: numeric checks, two backgrounds, native resolution, both Linux painters, multi-draw sequences |
| §6 over-claims: 42 MB ignores the layer's retained image; 300 ms/s is additional main-thread work, not a total; 3.1× with `makeImage`; the 45 ns source is not in this tree; historic runs are not controlled | both | §6 rewritten with those qualifications |
| Text on Linux: SVG text is paragraph painting, not outlines; outline paths drop colour glyphs | Astra | D8: Apple `CTLineDraw`; Linux the paragraph painter into the pixmap |
| `ctx.filter` sharing SVG islands is a third raster path on Apple | Grok | D7 and Q2 say so |
| Take 2 (`Custom(u16)`) and take 3 (queue item) are not doing-list takes | Grok; Astra partly | §9: take 1 only; the Caltrain migration is part of stage 1's delivery |
| Stage 1 too large for its consumer (text is the long pole) | Grok | §8: text moves to stage 2 |
| `direction: inherit` and `currentColor` resolve against the node; odd dash lists double; the dirty-rect `putImageData`; `DOMMatrix` on Hermes; `roundRect` radii | Grok; Astra | §3 and D3 |
| macOS: a CSS-animated child still needs `screenshot … window` | both | §5 |

Declined:
- **Throw-free drawing.** Both reviews offered making every failure silent instead of throwing. It is refused: the web throws, and Chrome is the standard.

## 0.1 Charlie's rulings (r3, 2026-09-27)

Charlie ruled on §10's four questions on 2026-09-27. In his words and in this text's:

1. **Admit Canvas 2D, with Caltrain's line map as the take:** "seems reasonable". §9's text goes into `rules/DEFERRED.md` §Components as drafted. The take: Caltrain's wgpu `map` surface and `shaders/map.wgsl` are deleted when stage 1 moves the map to Canvas 2D.
2. **Core Graphics on Apple, tiny-skia on Linux:** accepted as recommended.
3. **The fixture apparatus:** approved. `apps/canvas-gallery` with its direct-API page, a canvas mode in `apps/sparkline` at stage 3, and `smoke.mjs canvas` over the generalised SVG comparator. This approval is the human say-so `rules/RULES.md` §Agents and `CLAUDE.md` require before an agent adds apparatus.
4. **The deviations,** revised by Charlie's "ok" to these:
   - **(a) Coordinates.** Automatic CSS-pixel coordinates over a device-resolution backing store remain the default. But an author who sets the canvas's bitmap size explicitly gets exactly the web's behaviour: a fixed bitmap of that size, no automatic device scaling, stretched to the CSS box as the browser stretches it. That keeps web canvas code portable. It follows his standing principle that each platform reaches its full potential rather than the lowest common denominator, and that the web is the standard. D6 has the text.
   - **(b) Display-scale change:** clear and redraw, as drafted. "Stretch the old pixels into the new store, then redraw" is recorded as the future option for accumulating (paint-style) canvases, triggered by such a consumer (D6).
   - **(c) Errors are not atomic.** Match the web: a draw that throws keeps everything it recorded before the throw, pixels and state changes both, exactly as Chrome does. The rollback snapshot is removed (D4).
   - **(d) Limits** stay, but the size limits are what browsers actually enforce, so nothing that works in Chrome or Safari is refused here. The total-memory budget is set from measurement, not a guess, and still refuses through `state` (D4).

## 1. The model, and why it is Core Graphics

| Canvas 2D (HTML) | Core Graphics | tiny-skia |
|---|---|---|
| `save()` / `restore()` | `saveGState` / `restoreGState` | a state stack in the replayer |
| `translate` `rotate` `scale` `transform` `setTransform` | `CGAffineTransform` on the author matrix | `Transform` |
| `beginPath` `moveTo` `lineTo` `closePath` | a `CGMutablePath` the replayer keeps | `PathBuilder` |
| `quadraticCurveTo` `bezierCurveTo` | `addQuadCurve` `addCurve` | `quad_to` `cubic_to` |
| `arc` `arcTo` `ellipse` | `addArc` (direction inverted under the flip), `addArc(tangent1End:…)` | `exact-canvas`'s f64 arc geometry, as cubics |
| `fill(rule)` `stroke()` `clip(rule)` | `addPath` then `fillPath(using:)`, `strokePath`, `clip(using:)` | `fill_path` `stroke_path` `Mask` |
| line style, dashes | `setLineWidth` `setLineCap` `setLineJoin` `setMiterLimit` `setLineDash` | `Stroke`, `StrokeDash` |
| `globalAlpha` `globalCompositeOperation` | `setAlpha` `setBlendMode` | `Paint::blend_mode` |
| linear and radial (two-circle) gradients | `drawLinearGradient`, `drawRadialGradient` (the same two-circle model) | `LinearGradient`, `RadialGradient` |
| shadows | `setShadow(offset:blur:color:)`, the model the API came from | a blur in the replayer |
| `fillText` `strokeText` | Core Text `CTLineDraw` into the context | the host's paragraph painter into the pixmap |
| `drawImage` | `draw(_:in:)` | `draw_pixmap` |

The web's names are CG's operations under standard spellings. Where the two models differ, the replayer carries the rule and the API does not change.

**As built (r3).** The recorder resolves path geometry itself. `arc`, `arcTo`, `ellipse`, `rect` and `roundRect` become line and cubic segments by `exact-canvas`'s f64 geometry, and every point is transformed by the author matrix current when it was added, as the spec says. So the list carries only `moveTo`, `lineTo`, `quadraticCurveTo`, `bezierCurveTo` and `closePath` in canvas coordinates, and every replayer, Core Graphics included, draws the same segments. CG's `addArc` is never called, so its flip inversion below cannot go wrong. The arc-direction fixture still comes first, because it proves the whole chain. The notes below remain true of the model:

- **The current path survives painting.** Canvas's `fill`, `stroke` and `clip` keep the current path; CG's context operations consume it. The replayer keeps the path as a `CGMutablePath` in canvas coordinates and adds it to the context before each paint. Points are transformed by the matrix current when each was added, as the spec says. A `Path2D` is painted under the matrix current at the paint.
- **Coordinates.** CG's origin is at the bottom left. The replayer flips the context once, which reverses the sense of `clockwise`. It passes `clockwise: !anticlockwise` to `addArc` for that reason. This is the first parity fixture (§4).
- **Compositing extent.** Five operators (`source-in`, `source-out`, `destination-in`, `destination-atop`, `copy`) change destination pixels outside the shape, within the current clip. CG applies a blend mode only under the shape. The replayer draws the shape into a transparency layer bounded by the clip, as WebKit does (§3 stage 2).
- **Shadows.** A canvas shadow's offsets and blur ignore the current transform, and `shadowBlur` is twice the Gaussian's standard deviation. CG's shadow parameters are also in base space, so the replayer multiplies by the device scale only and converts the blur to CG's parameter. The residue is a measured band (§4).

## 2. Decisions

### D1. Who writes the drawing code: the data module, in either language

A 2D surface is a function in the app's data module, TypeScript or Rust, beside `answer` and `parse` (LLP 1027). The data module is where exact2 apps run imperative code. Contract has no loops by design (LLP 1027 §2), and a chart is a loop over its points.

**TypeScript.** Module ABI 2 adds two exports:

```ts
export const surfaces = { spark: 2 };            // name → arity; read at build (below)
export function draw(surface: string, args: unknown[], ctx: Ctx2D, frame: Frame): boolean {
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

- **`Ctx2D`** is generated per stage. Members that behave exactly as `lib.dom` declares them are taken from `lib.dom`'s own `CanvasRenderingContext2D`. Members this RFC narrows are declared explicitly: `fill` without the `Path2D` overload before stage 2, `fillStyle` without patterns before stage 2, `drawImage` taking an `ImageHandle` (D9), and `measureText` returning the fields its stage provides. A member not in the current stage is absent from the type, which makes it a compile error, and from the recorder object, which makes it a `TypeError` if called.
- **Objects.** `getTransform()` returns a detached `DOMMatrix`-shaped object (`a`–`f`, `is2D`, `isIdentity`, `inverse()`). `CanvasGradient` and `Path2D` are recorder objects backed by ids (D3).
- **`Frame`** is `{ time, mounted, cause, width, height, pixelWidth, pixelHeight, scale }` (D4, D5, D6). It differs from the GPU `Frame` (`gpu/src/lib.rs:40`) on purpose: no `period`, because a 2D canvas does not pace itself.
- **`args`** is what the GPU `bind` receives for the same `surface=` expression: positional values in order, or the named-argument object when the call uses names (`runner/src/instance.rs:244`).
- **ABI 2 on native** adds a byte result to the Hermes call (`exact_js_call` returns a string today, `js/src/shim.cc:261`): the recorder's buffer returns as an `ArrayBuffer` without a JSON trip. **On the web** the module realm transfers the buffer to the page. The module reports `abi: 2`, and a host that links an ABI-1 engine refuses a module with `draw` when it loads.

**Rust.** The data crate implements `exact_canvas::Surfaces` (a roster and `draw(&mut self, surface, args, ctx: &mut Context2d, frame) -> Result<bool, DrawError>`). `Context2d`'s methods are web-sys's names for the same IDL members (`begin_path`, `line_to`, `set_stroke_style_str`, `set_line_width`, `fill_rect`, and so on), with web-sys's error type for the methods that throw. Porting web-sys drawing code is mostly renaming the type. The exceptions are overloads that web-sys spells as `JsValue`, which this crate spells with typed variants, listed per stage.

**The roster is known at build, without running app code before first pixel.** For TypeScript, the bake already evaluates the module at build through the same engine (LLP 1027 D5), and it reads `surfaces` there. For Rust, `exact_canvas::surfaces!` emits the roster from the crate's build, as the GPU module's roster is emitted. The compiler checks every `canvas surface=` against the union of the data module's roster and every GPU artifact's roster (LLP 1009 D6). It refuses a name registered twice, including across TypeScript and Rust in a mixed module (LLP 1027 D8), and a name registered nowhere. The plan records each canvas's context and owning artifact.

**Contract gains no drawing language.** SVG is the declarative drawing model, admitted complete (LLP 1055).

**Not chosen:**
- **The GPU module as the 2D home.** Its per-canvas Metal layer and wgpu surface cost 116 MB at rest in the crypto bench, and its rows cannot pool (§6).
- **Drawing from a Rust data crate straight into a host context.** It would need a per-host drawing API, which is what this RFC exists to avoid.

### D2. One `canvas` tag, and the code chooses the context

The web has one `<canvas>`. Code calls `getContext("2d")` or `getContext("webgpu")`. In exact2 the same choice is where the surface is registered: a GPU module's roster is `webgpu`, and the data module's roster is `2d`. No `context=` attribute exists, because the web has none.

The two contexts share:
- the node, its 300 × 150 default and its box;
- children composited over the surface on every host (LLP 1014 D2);
- the publish-after-commit rule (LLP 1009 D2): a draw is requested only for arguments of a commit that was accepted, never from a refused transaction;
- the on-screen rule for frames (D4).

A 2D canvas never loads the GPU module. Not carried over to 2D:
- **`wants_children`.** Reading children's pixels is HTML-in-Canvas's `drawElementImage`, refused in §3.
- **1009's `Custom(u16)` extension point.** A 2D draw reads its frame time directly.

### D3. The canvas model, kept whole, recorded where it runs

**A 2D canvas is one context and one bitmap for its lifetime.** On the web both persist between draws: pixels, the drawing-state attributes, the state stack, the clip, the current path, and live objects. exact2 keeps all of it.

A `save()` in one draw and its `restore()` in another is legal, and an unmatched `restore()` does nothing. What resets both the bitmap and the state is what resets them on the web when `width` is assigned: here, a new size generation (D4), and `ctx.reset()`, which also clears the bitmap.

**The recorder is the context the author calls.** It lives in the executor, one per canvas, and it owns the authoritative drawing state. Each call:
1. **Applies the spec's argument rules at the call:**
   - Non-finite arguments make the listed path and drawing methods do nothing.
   - Invalid assignments are ignored: a negative or zero `lineWidth`, `miterLimit` ≤ 0, `globalAlpha` outside [0, 1], an unparseable `fillStyle` or `font`, an unknown enum value.
   - `setLineDash` with any negative or non-finite element does nothing, and an odd list is doubled.
   - Negative radii (`arc`, `arcTo`, `ellipse`, `roundRect`, `createRadialGradient`) and an `addColorStop` offset outside [0, 1] throw `IndexSizeError`. An unparseable stop colour throws `SyntaxError`.
   - Every other throw follows the IDL: a `TypeError` for a wrong type, `IndexSizeError` for `putImageData`'s dimensions.

   The rules are one table per member in `exact-canvas`. The Rust recorder is that table. The TypeScript recorder is generated from it, so both languages throw and ignore identically.
2. **Updates its state,** so getters answer synchronously and exactly: `fillStyle` returns its serialisation (`#rrggbb` or `rgba(…)`) or the gradient object; `getTransform()` returns a snapshot; `getLineDash()` returns the list.
3. **Appends an operation** to a flat buffer. Strings go in a side table. A `CanvasGradient` or `Path2D` is an id: `createLinearGradient` and `new Path2D()` record a creation, and `addColorStop` and `path.lineTo` record mutations of that id. The replayer holds live objects, and replaying in order gives the web's live semantics: stops added after assignment affect later paints, not earlier ones.

**Colours and fonts are parsed by `exact-canvas`'s own parsers.** Canvas colours are CSS Color 4: named colours, `hsl()`/`hwb()`, `transparent`, and `currentColor` resolving to the canvas node's `color`. `font` is the CSS `font` shorthand. The kernel has neither: its colour parser takes hex, `rgb()` and `transparent` only (`kernel/src/style.rs:723`), and fonts arrive as separate rows. These are new code in `exact-canvas`, and the kernel may adopt the colour parser later. On the web, the TypeScript recorder uses a scratch context's own parser, so the browser remains the oracle for serialisation.

**When `draw` returns, the buffer crosses as bytes.** The runner then does the structural check only: opcode validity, operand counts, id references, and the bounds (D4). Semantics were settled at the call.

**Why a recorded list, on every host including the web:**
- **One path.** Every host replays the same bytes.
- **Transport.** A worker (D4) and the Rust runner receive plain bytes.
- **Inspection.** The agent can read the list (§5).
- **Cost.** A numeric recorder of a sparkline runs in 3.8–4.0 µs under Hermes here (§6; a microbenchmark, not the full recorder).

The list makes the web glue part of what Chrome is asked to judge, so §4 adds a direct-API page as an independent oracle.

**Not chosen:**
- **Calling the browser's context directly from TypeScript on the web.** It skips recording on one host, at the cost of a web-only path, web-only worker behaviour and no agent view.
- **A retained scene graph.** That is SVG.

### D4. When the draw runs, the acceptance protocol, and the limits

**Geometry comes from the host.** Each 2D canvas's content box (CSS px) and device scale come from the host that lays it out:
- **Native hosts:** the kernel's layout, in the same turn.
- **The web:** the browser lays out, so `canvas2d-glue.js` watches each canvas with a `ResizeObserver` (`device-pixel-content-box`, falling back to `content-box` × `devicePixelRatio`) and reports changes to the runner.

The backing size is `round(css × scale)` per axis. A zero-sized canvas has no bitmap and is not drawn.

**A size generation begins** at creation and at every change of backing size or scale. A CSS change that rounds to the same backing size is not a new generation. A new generation does two things together:
- **Host:** clears the bitmap and resets the replayer's state.
- **Executor:** gives the next draw a fresh recorder at the defaults.

On the web a DPR change leaves the bitmap and the page redraws when it chooses. Here a scale change clears the bitmap and redraws at once. That is declared in D6.

**The runner requests a draw** (`frame.cause`) for one of these:
- `"mount"`: the node is created;
- `"args"`: its surface arguments changed in an accepted commit;
- `"size"`: a new size generation;
- `"frame"`: it asked for another frame and the host presents one while it is on screen (D5);
- `"image"` or `"font"`: an image it named decoded, or a font it used became available (D8, D9).

Nothing else causes a draw. Requests coalesce **before** execution: a canvas has at most one request queued, and its cause set is the union. Once a list has executed it is never dropped individually. It is applied, or its whole generation is abandoned.

**Stamps and acceptance.** Every request and reply carries four stamps:
- the module incarnation (a reload is a new one);
- the canvas's lifetime id;
- its size generation;
- a sequence number.

The host applies a canvas's lists in sequence order. A reply stamped with a retired incarnation, lifetime or generation is discarded, and the recorder state it produced is discarded with it, because the next request starts that generation's recorder from the defaults. Two cases show the rule:
- **Draw, then resize.** Draw A sets a transform. The canvas resizes before A's list lands. A is discarded, and draw B starts from identity. So does the host.
- **Two accumulating draws.** Neither can be dropped while its successor applies, because each executed list belongs to a live generation and applies in order.

Within one commit, canvases are drawn in tree order, after that commit's `answer` calls, so a draw sees committed state. An empty list leaves the bitmap unchanged.

**Where it runs.** `draw` runs in the module's executor:
- the runner's thread for native `main` placement;
- the page's module realm on the web;
- the module's worker when placed there (LLP 1027.002).

Replay runs on the host's presenting thread: the main thread on Apple and the web, and the painter on Linux.

**What the first frame is.** A 2D canvas shows its box's background, with a transparent bitmap, until two things are true: its executor is active and its first list has arrived. Executors activate after the first pixel on every host (LLP 1027), so no canvas is drawn in the first frame. After activation:
- **Native `main` placement** (TypeScript on Hermes, or Rust) draws in the same turn as the commit that mounts the canvas. The canvas is presented drawn.
- **The web's module realm and every worker** reply at least one turn later. The web's module turns are deferred and serialised (`host/web/module-glue.js:171`). The canvas is transparent for one or more frames.

`state` reports a canvas as `pending` until its first list applies. A baked first frame is stage 4.

**On screen only, for frames.** A `"frame"` request is honoured only while the host judges the canvas on screen. This is the GPU canvas's lesson from the crypto bench (`GAPS.md` §1). The request is held until the canvas is seen. The other causes draw whether or not the canvas is on screen, so a row built in overscan arrives drawn.

**Errors, as the web has them** (Charlie, r3: not atomic). On the web the calls before an uncaught exception have already painted, and the context keeps every state change they made. Here too: a draw that throws keeps everything recorded before the throw, pixels and state both. The list up to the throw is applied in order, and the recorder is not rolled back. There is no snapshot. `state` reports the error with the canvas's lifetime. Frame requests stop until the next `"args"`, `"size"` or `"mount"`. The possible failures, and what each shows:

| Failure | Shows |
|---|---|
| A draw that throws | the previous bitmap with the calls made before the throw applied over it; the context keeps their state changes |
| A first draw that throws | the calls made before the throw, over the transparent bitmap and the box's background |
| A bitmap that cannot be allocated, or is over the limit | the background; the canvas is refused in `state` |
| An unknown surface or arity | a compile error; if it escapes, the background |

`draw` is synchronous in both languages. A returned promise is a `TypeError`, because a draw must finish in its turn. The calls it made before returning are kept, as for any throw.

**Limits** (Charlie, r3: what browsers enforce, so nothing that works in Chrome or Safari is refused here). Each is enforced where the cost is incurred. A refusal is reported through `state`, and the canvas shows its background:

| Limit | Value | Source |
|---|---|---|
| Backing side | 65,535 px | Chrome's `kMaxSkiaDim` (`canvas_rendering_context_host.cc`). WebKit has no per-side limit, only area |
| Backing area, web, macOS, Linux | 268,435,456 px (16,384²) | Chrome's `kMaxCanvasArea` = 32,768 × 8,192; WebKit's `maxCanvasArea()` = 16,384² off iOS (`CanvasBase.cpp`) |
| Backing area, iOS | 67,108,864 px (8,192²) | WebKit's `maxCanvasArea()` on `IOS_FAMILY`. Every iOS browser is WebKit, so nothing that runs in a browser on the device is refused |
| Total canvas backing, mounted plus free list, per process | a quarter of physical memory (native hosts; the browser's own on the web) | WebKit's `maxActivePixelMemory()` was `ramSize() / 4` on iOS until 2023 (WebKit `6bd11f37`, bug 195325, which removed it). Chrome and today's Safari have none. Measured below |
| Operations in one list | no refusal: a list is sealed at 1 MiB and the draw continues in the next, applied in order | Chrome flushes its recording at a byte threshold too (`BaseRenderingContext2D::UpdateRecordingLimits`, `kMaxRecordedOpKB`) |
| Queued lists across all canvases | 8 MiB; beyond it, requests wait | back-pressure, not a refusal |

**The total budget, measured.** A Core Graphics probe on this Mac (§11) holding full-screen canvases, each drawn, published to a layer and drawn again, measured an in-process footprint of 22.5 MiB per iPad Pro 13″ canvas (2,064 × 2,752 px; one bitmap is 21.7 MiB) and 14.5 MiB per iPhone 6.7″ canvas (1,290 × 2,796 px). The budget counts two bitmaps per canvas regardless (the context's and the layer's image, §6), so an iPad 13″ full-screen canvas is charged 43.4 MiB. r2's 64 MiB held one of them. A quarter of physical memory holds 47 of those on an 8 GB iPad, and 27 iPhone 6.7″ canvases (27.6 MiB charged each) on a 3 GB phone, and leaves three quarters of memory to the rest of the app. It is WebKit's own figure for the years it had one.

Not limits any more, because no browser has them: state-stack and clip depth, path segments, and live gradient and `Path2D` ids. Chrome only records its maximum stack depth in a histogram (`Canvas2DRecorderContext`, `Blink.Canvas.MaximumStateStackDepth`). A draw that exhausts memory with them does so as a page does.

**Reload.** A dev reload that replaces the module owning a surface (LLP 1007 §6) starts a new incarnation. Every 2D canvas it owns gets a new lifetime, and in-flight replies are discarded. In a mixed module, replacing only the TypeScript half re-creates only the TypeScript half's canvases.

### D5. Time, animation and the agent's clock

`draw` returns `true` to ask for another frame. That is `requestAnimationFrame` without a global: modules have no timers (LLP 1027). The host serves the request from its frame source under LLP 1009 D4's `canvas` batch signal. It passes its presentable clock as `frame.time`, which under an agent-owned clock is the agent's time. `frame.mounted` is the time this canvas node was created, so a draw-in is `min(1, (time − mounted) / 600)` with no module state.

- **`clock +N` and `clock settle` do not wait for a canvas to stop asking.** Whether it ever stops is its code's business, as with an infinite CSS animation (LLP 1055).
- **They do wait for the draws due at the landed time.** Each on-screen canvas that asked for a frame is drawn once at the landed clock. `clock` replies after those lists are applied, or after a bounded wait (the agent's existing settle bound), reporting any still pending. This is how the Apple agent already settles surface presentation (`host/apple/Sources/ExactKit/Agent.swift:254`). A canvas is sampled at most once per clock value, so settle's fixed-point loop does not draw it twice.
- **Ordering in one clock step:** timers settle, then commits, then geometry, then draws.
- **Accumulating canvases depend on frame count.** A trail drawn without clearing depends on how many frames it got. That is true on the web too (rAF throttles in background tabs), and it is declared.
- **Determinism.** A canvas's pixels are a function of its stamped lists, applied in order to a bitmap cleared at its generation's start, plus the images and fonts each list's request marked ready (D8, D9).

### D6. Coordinates and resolution: a declared deviation

The context's coordinate space is the content box in CSS pixels. The backing store is at device pixels: `frame.pixelWidth` × `frame.pixelHeight`, `frame.scale`. The replayer's transform is `base ∘ author`, where `base` is the device scale and the flip, and `author` is the recorder's matrix. `setTransform` and `resetTransform` replace only `author`, and `getTransform()` returns only `author`.

On the web authors write `canvas.width = w * dpr; ctx.scale(dpr, dpr)` by hand. exact2 does it because the canvas's size is a CSS box (LLP 1009 D3). Declared, for the default:
- the backing store is sized and reset automatically;
- a scale change clears and redraws (D4). The future option for accumulating (paint-style) canvases is to stretch the old pixels into the new store and then redraw; its trigger is such a consumer (Charlie, r3);
- the device scale is invisible to the author's matrix.

**An explicit bitmap size is the web's canvas exactly** (Charlie, r3). On the web a canvas's `width` and `height` content attributes size its bitmap, and CSS sizes its box. In Contract `width` and `height` are the box's CSS size (LLP 1009 D3), so the bitmap attributes are spelled `bitmap-width` and `bitmap-height` (whole pixels, as HTML's non-negative integers). When either is set, the canvas behaves as the web's:
- the bitmap is `bitmap-width` × `bitmap-height` pixels; an attribute left unset takes the web's default, 300 or 150;
- `frame.width` = `frame.pixelWidth` = the bitmap width, and likewise the height; `frame.scale` is 1, and `base` is the identity (and the flip);
- the host stretches the bitmap to the content box, as the browser does for `object-fit: fill`, with smoothing;
- a change of display scale is not a new generation, because the bitmap does not change;
- a change of either attribute is a new generation, as assigning `canvas.width` is on the web.

So canvas code written for a fixed bitmap runs unchanged, including its blur on a high-density display.

**Pixels are raw backing pixels, as on the web.** `createImageData` and `putImageData` address device pixels, and `putImageData` ignores the transform, clip, alpha, compositing and smoothing, per the spec. An author sizes a full-canvas `ImageData` from `frame.pixelWidth` and `frame.pixelHeight`.

Context attributes are fixed: `alpha: true`, `colorSpace: "srgb"`, `willReadFrequently: false`, `desynchronized: false` (§3).

### D7. Per-host backends

| | Web | Apple (iOS, macOS) | Linux |
|---|---|---|---|
| Executor of `draw` | the page's module realm, or its Worker | Hermes or the Rust data crate on the runner's thread, or the module worker | the same |
| Replayer | `canvas2d-glue.js` into the element's `CanvasRenderingContext2D` | `Canvas2D.swift` into a `CGContext` bitmap, then the view layer's `contents` | Rust into a tiny-skia `Pixmap` |
| Text | the browser | Core Text `CTLineDraw` (colour glyphs included), fonts by LLP 1019 | the host's paragraph painter into the pixmap |
| Oracle | Chrome, directly and through the list (§4) | Chrome, within §4's bands | Chrome, within §4's bands |

**Apple: Swift, the main thread, a bitmap context.**
- **Why Swift.** The Apple host's Rust is `#![deny(unsafe_code)]` (`host/apple/src/lib.rs:33`), and calling CG from Rust would be an audited `unsafe` surface per call. Platform drawing stays in Swift.
- **The handoff.** The list crosses as one little-endian, 8-byte-aligned buffer with a version header. Rust owns it until the batch that carries it is released, so the Swift reader's `UnsafeRawBufferPointer` is valid for exactly the replay. The reader is typed, from the start. An SVG scene read as generic JSON costs 5% of the main thread in a scrolling list (`QUEUE.md`).
- **The layer.** Each 2D canvas view owns a premultiplied BGRA sRGB `CGContext` at backing size. The view's own layer `contents` is `makeImage()` after each replay, so the drawing sits beneath the view's subviews with no overlay view (D10).
- **Why the main thread.** Main-thread replay keeps a batch atomic: new pixels appear in the same `CATransaction` as the row's new text. CG itself does not need the main thread. Moving replay off it is stage 4, judged by **aggregate** replay time per frame, not by one canvas's.
- **Not `CGLayer`.** It is a cache for repeated drawing inside one context.

**Why Core Graphics and not tiny-skia on Apple.**
- **Speed, measured here** (§6): 28.6 µs against 133.5 µs at 2× (4.7×), and 37.6 against 215.9 µs at 3× (5.7×). With `makeImage()` included, CG is 3.1× faster at 2×.
- **Text** is the app's own Core Text.
- **It is Charlie's model.**

The costs:
- **Apple and Linux match Chrome rather than each other.**
- **If `ctx.filter` ever shares LLP 1055.000's islands, which chose tiny-skia on Apple, Apple gains a third raster path.** It would then have Core Graphics, Core Animation and tiny-skia. That is why `ctx.filter` waits (§3).

**Linux: tiny-skia under both painters.** A canvas is a kept bitmap, and vello renders a scene per frame with no kept target, no clip-extent operators and no blur. tiny-skia is already the CPU painter and oracle (`host/linux/Cargo.toml:40`). The GPU painter uploads the pixmap as an image brush. The pixmap carries a content revision: the upload cache, which keys on bitmap identity (`host/linux/src/gpu/images.rs:23`), keys on the revision too, and retained regions snapshot a revision rather than alias the live pixmap.

**The web.** `canvas2d-glue.js` (the replayer and the `ResizeObserver`) is injected after first paint, when the first 2D canvas mounts, as the GPU glue is. An app without one never fetches it (LLP 1047). The glue sits beside a `<canvas>` element under the node's wrapper, as LLP 1014.000 does for GPU canvases.

### D8. Text (stage 2)

`fillText`, `strokeText` and `measureText` use the host's text engine, so a canvas label matches the `text` node beside it:
- **`font`** is parsed by `exact-canvas`; family names resolve as box text resolves them, LLP 1019's declared faces first.
- **`direction: "inherit"`** resolves to the canvas node's CSS `direction`, which `textAlign: start|end` follow.
- **`maxWidth`** is honoured by horizontal scaling, which the spec allows among other strategies and which Chrome uses.
- **Metrics** are in CSS px, relative to the alignment point and baseline, never multiplied by the drawing matrix.

**Measurement is executor-local, never a call into the UI thread** (LLP 1027.002 D4 forbids that shape):
- **Apple:** Core Text is thread-safe, and LLP 1019's faces are registered process-wide, so the executor measures with Core Text on its own thread.
- **Linux:** the paragraph engine is main-thread state (`host/linux/src/text.rs:489`). A worker-placed module gets its own measuring font system loaded from the same face files. A `main`-placed module uses the host's.
- **The web:** the module realm or Worker loads the page's declared `@font-face` sources into its own `FontFace` set before activation, and measures with a scratch context.

A face that finishes loading causes a `"font"` redraw of canvases whose recorded fonts named it.

`measureText` returns `width`, the four `actualBoundingBox*`, `fontBoundingBoxAscent/Descent`, `emHeightAscent/Descent` and the three baselines. Whitespace follows the spec: every space separator becomes U+0020, no collapsing. A string with no glyphs measures zero width.

Parity for text is Chrome's within the text band (§4). A declared font is the tight case. The host's engine is not Chrome's.

### D9. Images, pixels and paths (stage 2)

**`drawImage` takes an `ImageHandle`, never an element.** A surface argument of type `image` is the same URL or asset value an `image` node's `src` takes, resolved through the host's decoded-image cache under LLP 1010 §6's budget. The three overloads (position, size, source rectangle) follow the spec, including negative-dimension normalisation and source clipping.

A handle's readiness is pinned per draw. The runner stamps each request with the set of the canvas's handles that are decoded. A `drawImage` of a handle outside that set draws nothing, as an incomplete `HTMLImageElement` does, even if the image decodes before replay. A broken or zero-sized image draws nothing, and `state` names it.

When a named handle decodes, the runner requests a `"image"` draw. That is the web's `img.onload = draw` made automatic. An accumulating canvas checks `frame.cause` to avoid painting twice.

**Pixels.** `createImageData(w, h)`, `createImageData(imageData)`, `new ImageData(…)` and `putImageData` with its dirty-rectangle overload are in stage 2, in device pixels (D6). The pixels are copied into the list at the call, so a later mutation of the `ImageData` does not change an earlier operation.

**Readback is refused:** `getImageData`, `toDataURL`, `toBlob`. The executor is not where the pixels are. A synchronous read would put the host's rasterizer in the executor's turn. The agent reads pixels with `screenshot`.

**`Path2D`**, stage 2: the constructors (empty, from another path, from SVG path data), the path methods, `addPath(path, transform)`, and the `fill`, `stroke` and `clip` overloads. The SVG `d` grammar is the kernel's parser (`kernel/src/svg/path.rs`), producing segments that `exact-canvas`'s f64 geometry flattens. `isPointInPath` and `isPointInStroke` are stage 3. They answer in the executor from `exact-canvas`'s geometry and need no host call.

### D10. Lists: pooling, the fill policy and memory (stage 3)

**Pooling.** On Apple a 2D canvas is a plain `NodeView`:
- its bitmap is its own layer's `contents`;
- its children are ordinary subviews above that content, with no overlay view and no Metal.

So `NodePoolIOS` can take it. The eligibility rules (`NodePoolIOS.swift:117`, `:153`) gain the `canvas` kind for 2D canvases only. A GPU canvas, with its `metal`, overlay or `canvasInput`, stays ineligible.

The reset contract on park: the backing pixels are cleared, not only the layer's `contents`, and the view drops:
- its lifetime, generation and sequence;
- queued requests and its frame request;
- its image and font subscriptions;
- its inspection record.

The next node's `mounted` is that node's creation time, so a reused row replays its draw-in. A parked bitmap is kept only if the next node's backing size matches. Parked and free-listed buffers are counted together under the total budget (D4) and dropped under memory pressure, as shaped text is (`1ced26af`).

**The fill policy (LLP 1050.000).**
- **The first draw always runs** with its row. Its cost is unknown until it has run once.
- **The runner keeps a moving cost per surface name** (record + transfer + replay), so later rows of that surface have a known cost for D3's "a row known to cost more than a frame".
- **Only native `main` placement can present an owed row drawn**, since the web and worker placements answer later (D4). There the row presents with its canvas `pending`, and `state` says so.
- **Frame requests are never owed.** A missed frame shows the previous one.

**Memory is O(mounted canvases):** bitmaps for mounted 2D canvases, the image the layer holds, and the bounded free list. It is not charged to the decoded-image budget, and it is bounded by D4's total budget.

## 3. The subset by stage, and what is refused

Every default is the spec's:

| State | Default |
|---|---|
| `fillStyle`, `strokeStyle` | `#000000` |
| `lineWidth` | 1 |
| `lineCap` | `butt` |
| `lineJoin` | `miter` |
| `miterLimit` | 10 |
| line dash | an empty dash list, `lineDashOffset` 0 |
| `globalAlpha` | 1 |
| `globalCompositeOperation` | `source-over` |
| shadows | transparent black, zero offsets and blur |
| `font` | `10px sans-serif` |
| `textAlign` | `start` |
| `textBaseline` | `alphabetic` |
| `direction` | `inherit` |
| image smoothing | enabled, quality `low` |
| transform, path, clip | identity; empty path; no clip |

A stage's unsupported enum values are ignored as invalid values are, with a development log line.

**Stage 1: shapes and the protocol.**
- **State:** `save`, `restore`, `reset`.
- **Transforms:** `translate`, `rotate`, `scale`, `transform`, `setTransform` (six numbers or `DOMMatrix2DInit`), `resetTransform`, `getTransform`.
- **Paths:** `beginPath`, `moveTo`, `lineTo`, `quadraticCurveTo`, `bezierCurveTo`, `arc`, `arcTo` (the spec's zero-radius, empty-subpath, coincident and collinear cases), `ellipse`, `rect`, `roundRect` (its radii list and normalisation), `closePath`.
- **Painting:** `fill(rule)`, `stroke()`, `clip(rule)`, `fillRect`, `strokeRect`, `clearRect` (obeys transform and clip, ignores style, alpha, compositing and shadows, keeps the current path).
- **Lines:** `lineWidth`, `lineCap`, `lineJoin`, `miterLimit`, `setLineDash`, `getLineDash`, `lineDashOffset`.
- **Styles:** colours (D3), and linear and radial `CanvasGradient`s with `addColorStop`.
- **Compositing:** `globalAlpha`, and every operator that changes only covered pixels:
  - the Porter-Duff operators `source-over`, `source-atop`, `destination-over`, `destination-out`, `xor` and `lighter`;
  - the blend modes `multiply` through `luminosity`.

**Stage 2:**
- **Text:** D8, including `letterSpacing`, `wordSpacing`, `fontKerning`, `fontStretch`, `fontVariantCaps` and `textRendering`.
- **Images and pixels:** `drawImage`, `ImageData` and `putImageData` (D9).
- **Paths:** `Path2D` (D9).
- **Paint:** `createPattern` over an `ImageHandle`, and `createConicGradient`.
- **Shadows** (§1).
- **The clip-extent operators** `source-in`, `source-out`, `destination-in`, `destination-atop` and `copy` (§1).
- **Smoothing:** `imageSmoothingEnabled` and `imageSmoothingQuality`, which apply to `drawImage` and patterns.

**Stage 3:** `isPointInPath` and `isPointInStroke` (D9), and pointer coordinates in a 2D canvas's handlers. These are content-box CSS px on the existing press and move events, so they do not wait on the GPU input seam.

**Refused by name, each with its trigger:**

| Refused | Why | Trigger to revisit |
|---|---|---|
| `getImageData`, `toDataURL`, `toBlob` | readback in the executor (D9) | a consumer that reads its own pixels |
| `ctx.filter` | no CSS filter chain in CG or vello; sharing SVG's tiny-skia islands would give Apple a third raster path | SVG islands landing, and a consumer |
| `drawFocusIfNeeded`, `scrollPathIntoView` | focus and scrolling belong to boxes; the canvas node is focusable as a box | none planned |
| hit regions | removed from HTML | never |
| `ctx.canvas`, `getContext`, `OffscreenCanvas`, `transferControlToOffscreen` in app code | the app never holds an element. Worker placement records off the main thread, but the host owns the bitmap: it is not `OffscreenCanvas` | none |
| `drawImage` of a canvas, video or element; `createImageBitmap` | cross-surface pixels (HTML-in-Canvas `drawElementImage`); images are host-decoded handles | a consumer, with LLP 1014's capture as the route |
| `colorSpace: "display-p3"` | sRGB bitmap (D6) | a wide-colour consumer |
| `alpha: false` | an opaque bitmap saves nothing measurable at these sizes | a measured full-screen consumer |
| `willReadFrequently`, `desynchronized` | no readback; the host owns presentation | follows readback |

## 4. Parity with Chrome

There are two oracles, because the recorded list puts the web glue inside the first:

1. **The API oracle.**
   - **The direct-API page.** A development-only page in `apps/canvas-gallery` runs each fixture's TypeScript `draw` against the real `CanvasRenderingContext2D` in Chrome, bypassing the recorder and the glue. It compares pixels with the recorded path on the web.
   - **The recorder's logs.** Getters, thrown exceptions and state after each call are logged from both, and the logs must be equal.
   - **WPT.** A subset of WPT's `html/canvas/element` tests for the stage's members runs against the TypeScript and Rust recorders in `cargo test` and the web host's tests.
2. **The replay oracle.** The SVG smoke's comparator (`scripts/svgparity.mjs`), generalised to take an app and page names, compares each native host's fixture with Chrome's. The comparator's current form hides some errors, so for canvas it gains:
   - an alpha check on two backgrounds (white, black);
   - comparison at native resolution as well as one pixel per point;
   - registration limited to ±1 px;
   - both Linux painters;
   - numeric probes that read `layout` and `state`: list counts, bitmap size, generation.

**Bands, provisional until measured on the first gallery:**
- **Shapes, gradients and compositing:** SVG's (mean |Δ| ≤ 4/255, ≤ 6% of pixels off by more than 32).
- **Text:** SVG's text band on Linux.
- **Shadows:** measured in stage 2.

**The fixtures** (one `fx-*` canvas each):
- `arc` direction under the flip;
- caps and joins;
- dashes, including odd lists;
- `arcTo` cases;
- `roundRect` radii;
- both gradients, with a stop added after assignment;
- every stage-1 operator on a translucent destination;
- clip with both rules;
- the current path kept after `fill`;
- the state stack across draws;
- throws and ignored assignments (getter logs).

**Multi-draw sequences:**
- accumulation without clearing;
- `reset` versus `clearRect` under a clip;
- a resize mid-sequence (new generation);
- a draw that throws (the calls before the throw are kept);
- a worker-placed canvas with a stale reply;
- a reload.

Finite animations use explicit `clock` samples, since a boolean frame request says nothing about when a canvas finishes.

## 5. The agent

No operation is added:
- **`tree`** shows a 2D canvas as `canvas` with `context: "2d"`.
- **`state`** has one record per canvas:
  - surface, context and owning artifact;
  - lifetime, generation and last applied sequence against the latest requested, which gives `pending`;
  - backing width, height and scale;
  - `animating`;
  - the last list's operation and byte counts, and the time of its last draw;
  - the last error or refusal.
- **`layout <node>`**, in development builds only, adds the last list decoded to readable calls, capped at 200 lines and 16 KiB.
- **`clock`** behaves as D5 says.
- **`screenshot`** is unchanged. A 2D canvas's bitmap is model-layer `contents`, so the default macOS screenshot shows it. A CSS-animated child over it, such as the pulse in §6, still needs `screenshot … window`, as LLP 1055's compositor animations do.

## 6. Performance and memory: a sparkline per row

**Measured here** (Apple M5 Max; release builds; the crypto bench chart: a 114 × 50 pt canvas, a 48-point 1.5 pt round-joined polyline, a ring and a 3 pt dot; the probes in §11):

| | 2× (228 × 100 px) | 3× (342 × 150 px) |
|---|---|---|
| Numeric recorder under Hermes (`hermes -O`; path calls only, no style setters, no validation, no transfer) | 3.8–4.0 µs | same |
| Core Graphics raster | 28.6 µs | 37.6 µs |
| Core Graphics raster + `makeImage()` (not assigned to a layer) | 42.7 µs | 56.4 µs |
| Core Graphics, draw-in frames (partial polyline) | 17.3 µs | 23.2 µs |
| tiny-skia 0.12 raster | 133.5 µs | 215.9 µs |
| One tightly packed bitmap | 89 KiB | 200 KiB |

These are microbenchmarks. The layer commit, the transfer, the runner's check and the recorder's state handling are not in them.

**What exists to compare with.** M1 iPad Pro 12.9″ at 120 Hz, `~/bench/cryptobench`, medians of three interleaved rounds. The rows are from different revisions and series, so they are not a controlled comparison: the collection's retirement has changed since, LLP 1050.000 §6.

| App | rest: CPU ms/s (main) | rest: memory end | fling: fps, late/1k | source |
|---|---|---|---|---|
| SwiftUI | 381 (322) | 22.9 MB | 79.5, 135 | `ipad-r2` |
| Expo + Skia `Canvas` per row | 857 (398) | 112 MB | 105.8, 127 | `ipad-r2` |
| exact2 GPU `canvas` per row | 879 (359) | 116 MB | 108.0, 96 | `ipad-r1` |
| exact2 SVG | 119 (114) | 53.5 MB; 38 MB after perf/crypto-svg | 118.4, 13 | `ipad-r2`; `QUEUE.md` |

**A model of a Canvas 2D sparkline per row on the iPad.** It is not a measurement. It assumes the M1 is 2–2.5× slower than this M5 Max on this work, and about 120 µs per canvas per drawn frame for record, check, transfer and replay.

- **Memory.** The layer can hold the previous image while the context holds its own buffer (`TextRaster.swift:125` discusses the same copy). So a canvas can cost two buffers: 178 KiB at 2×.
  - 46 mounted rows (`QUEUE.md`) give 4–8 MB on the iPad and 9–18 MB on a 3× phone.
  - An illustrative subtotal is the SVG app's 38 MB plus 4–8 MB, about 42–46 MB. It is not a footprint estimate: removing the SVG layers saves some memory, and a canvas's own structures add some.
  - For comparison: GPU canvas 116 MB, Skia 112 MB, SwiftUI 23 MB.
- **CPU when drawing follows data.** A tick changes 40 of 5,000 coins, so about 0.2 visible charts per tick, at about 0.12 ms each. The pulse is a **canvas child** with a CSS `@keyframes` animation (LLP 1014 D2 with LLP 1055), which Core Animation runs. Expected rest CPU is near the SVG app's.
- **CPU when the pulse is drawn in the canvas every frame** (rAF style): about 16–21 visible canvases × 120 µs × 120 Hz, which is **about 230–300 ms/s of additional main-thread work**. Added to the SVG app's 114 ms/s main-thread baseline, that is about 345–416 ms/s on the main thread, near the GPU canvas's 359. It is still well under the GPU canvas's 879 ms/s total CPU and a third of its memory. Replay off the main thread (stage 4) would move the raster share. Validation, transfer and the layer commit stay.
- **Fling.** Only on-screen canvases draw per frame (D4). At 24,000 pt/s a 64 pt row is on screen for about 43 ms, so the per-frame load is the same 16–21 canvases. What rises with speed is mounting: 375 rows/s × about 0.12 ms ≈ 45 ms/s plus the row itself. The GPU canvas instead builds a `MetalView`, a `CAMetalLayer` and a wgpu surface per row. Its ladder retained 824 MB by 96k pt/s (`GAPS.md` §2).

**The claim stage 3 tests:**
- When drawing follows data, Canvas 2D costs a list about what SVG costs, plus one or two small bitmaps per mounted row.
- When drawing follows the clock, it costs bounded main-thread time per visible canvas: near the GPU canvas's main-thread time, and far below its total CPU and memory.
- SVG remains the tool for a pulse on thousands of rows. Canvas 2D is the tool when the drawing is computed.

Stage 3 measures, on the same revision and device:
- both pulse variants;
- mounted and pooled canvases;
- the layer's retained images;
- the same series as the other four apps.

## 7. What it costs to carry

The line counts are estimates. The protocol (D4) is most of the new work.

- **`exact-canvas`** is a leaf crate with no dependencies above it. It holds:
  - the per-member rule table and the Rust recorder;
  - the list format and structural check;
  - the CSS Color 4 and `font` shorthand parsers;
  - f64 arc and `roundRect` geometry;
  - the WPT subset.

  About 3,000 lines, split into files under the 1,500-line cap. It is a linked capability (LLP 1047), in an app's artifact only if its plan names a 2D surface. The runner reaches it through the capability seam that already keeps optional runner code out (`runner/src/runner.rs:350`).
- **TypeScript:** the generated recorder (about 800 lines), bundled only into modules that export `draw`, and the `Ctx2D` type per stage.
- **`exact-js`:** ABI 2's byte result, and executor-local text measurement (stage 2).
- **Runner:** requests, stamps, acceptance, coalescing, limits, per-surface cost. About 800 lines.
- **Web:** `canvas2d-glue.js`, the replayer and the `ResizeObserver`. About 500 lines, fetched after first paint with the first 2D canvas.
- **Apple:** `Canvas2D.swift`, the replayer, bitmap lifecycle and pooling reset. About 900 lines. No new dependency.
- **Linux:** the tiny-skia replayer and the pixmap revision. About 800 lines.
- **No new blocking check.** The parity smoke runs asynchronously, as `smoke.mjs svg` does.

## 8. Staging, the fixture and the parity smoke

| Stage | Ships | Proven by |
|---|---|---|
| 1 | `exact-canvas`; ABI 2 with `draw` and `surfaces`; the recorders in both languages; the runner's protocol (D4: geometry, generations, stamps, errors, limits, first frame); replayers on web, Apple and Linux; §3's stage-1 members; `state`/`layout`/`clock` (§5, D5). **Caltrain's line map moves** from its wgpu `map` surface to a Rust `draw` in Caltrain's data crate, keeping its geometry, background, train marker and station-label children. The `map` surface, its registry entry and `shaders/map.wgsl` are deleted. The map draws on `"args"` only: its `nowMs` ticks once a second. | WPT subset and rule-table tests; the gallery's stage-1 fixtures and multi-draw sequences under both oracles (§4) on web, macOS, iOS and Linux; Caltrain's smoke unchanged except the map's pixels, held to Chrome |
| 2 | text (D8) with executor-local measurement; `drawImage`, `ImageData`, `Path2D`; patterns; conic gradients; shadows; clip-extent operators | gallery pages per item, same smoke |
| 3 | pooling 2D canvas rows on iOS (D10); per-surface cost in the fill policy; `isPointInPath`/`isPointInStroke` and pointer coordinates; a fifth crypto bench app (`exact-canvas2d`, outside this repo like the others) in both pulse variants; `apps/sparkline`'s canvas chart | the iPad series against SwiftUI, Skia, GPU and SVG on one revision; §6's model replaced |
| 4 | only if measured: Apple replay off the main thread (by aggregate replay per frame); a baked first frame for fixed-size canvases whose arguments settle at bake; web replay in a Worker through `OffscreenCanvas` | a fixture showing aggregate main-thread replay over 2 ms a frame, or a blank first frame worth removing |

**The fixture and the demo.**
- **`apps/canvas-gallery`** holds §4's fixtures, sequences and the direct-API page. It is modelled on `apps/svg-gallery`.
- **`apps/sparkline`** gains a canvas chart beside its SVG chart in stage 3: one Contract row, two drawing models, Chrome the reference for both.
- **The smoke** is `bun scripts/smoke.mjs canvas`, over the generalised comparator.

The gallery, the sparkline mode and the smoke mode are apparatus. Charlie approved them on 2026-09-27 (§0.1, §10 Q3), which is the human approval `rules/RULES.md` §Agents requires.

## 8.1 Stage 1 as built (2026-09-27, `feat/canvas2d-stage1`)

What stage 1 ships, and where it differs from the text above. `QUEUE.md` lists what it still owes.

- **`exact-canvas`** (`canvas/`): the Rust recorder `Context2d` with web-sys's names; the list (`list.rs`: little-endian, 8-byte aligned, sealed at 1 MiB, never refused); CSS Color 4's sRGB forms with Chrome's 8-bit alpha; f64 geometry. The recorder resolves `arc`, `arcTo`, `ellipse`, `rect` and `roundRect` into canvas-space segments (§1, as built), so every replayer draws the same segments.
- **The rules, and the TypeScript recorder.** `canvas/tests/cases.txt` holds the rules as calls, getters and throws. Headless Chrome agrees with all 14 cases (`bun canvas/tests/cases.mjs --chrome`). The Rust recorder passes them in `cargo test`, and so does `canvas/recorder.js`, the TypeScript recorder (`bun test`). `cases.rs` compares the two recorders' lists record by record. The TypeScript recorder is a hand port held to the Rust one by that comparison, not generated from a rule table as D3 says.
- **The seam.** The data module is reached through `DataSource`, not an `exact_canvas::Surfaces` trait: `canvas_surfaces()` (the roster, known before any app code runs), `draw_2d()` for a Rust crate, `draw()` for an executor with its own recorders, and `canvases_retired()`. Every forwarding source forwards them. A module that exports `draw` and `surfaces` speaks ABI 2. The bake bundles the recorder only into such a module and records its roster beside the bytecode (`CANVAS_SURFACES`). A TypeScript draw's reply crosses as JSON with base64 lists (`exact_canvas::seam`); the byte result is owed.
- **The protocol** (`runner/src/runner/canvas2d.rs`): as D4 says, with these differences:
  - A Rust source is active at boot, so its canvases draw in the first frame natively (Caltrain's map does). A TypeScript module's canvases wait for activation.
  - On the web, the module realm draws synchronously in the turn, because a draw awaits nothing.
  - Worker-placed and replaced Rust modules report that they do not draw yet.
  - `state.canvas` is as §5 says. `layout <node>`'s `canvasList` is there wherever inspection is linked, not only in development builds; it is bounded as §5 says.
  - `clock` draws the canvases that asked for a frame at the landed time.
- **Hosts.**
  - Linux replays into tiny-skia under both painters. `clearRect` is a destination-out fill, because tiny-skia's `Clear` ignores the mask.
  - Apple replays into Core Graphics on the main thread. A 2D canvas is a plain view (kind `canvas2d`: no Metal view, no overlay, as D10 wants for pooling). The bitmap is a sublayer masked to the content edge's curve. Lists cross the batch as base64, where D7 asks for a typed buffer (owed).
  - The web replays into the element's own context. A `ResizeObserver` reports the content box and a `matchMedia` watch the scale.
- **Explicit bitmaps** (D6, r3): the `bitmap-width` and `bitmap-height` props (schema 133, 134).
- **Caltrain.** The line map is `caltrain_data::map`. `map.wgsl`, the `map` surface and its tests are gone.
- **Parity** (`bun scripts/smoke.mjs canvas`, all four hosts, one revision). The gallery has 70 crops a host: 16 fixtures on white and black, and page 3's five sequences at four steps. Every crop is within §4's bands, which hold as the measured bands. Mean |Δ| per crop (/255) and pixels off by more than 32:
  - **The API oracle.** The web's recorded path against `direct.html` (the fixtures on Chrome's own context, no recorder or glue): mean 0.20, worst 1.66; pixels off 0.21% mean, 1.11% worst.
  - **Linux against the web**, cpu and gpu painters alike: mean 0.17, worst 0.81; pixels off 0.20% mean, 2.19% worst. At native resolution (1×) against `direct.html`: worst 1.17.
  - **macOS against the web:** mean 0.31, worst 2.10; pixels off 2.37% worst. At native resolution (2×): mean 0.28, worst 1.82.
  - **iOS against the web:** mean 0.18, worst 1.07; pixels off 2.35% worst. At native resolution (3×): mean 0.11, worst 2.23.
  - **Caltrain's line map**, the strip no child covers, with the sky off: Linux 0.25 (cpu) and 0.31 (gpu), macOS 0.26, iOS 0.23.

## 8.2 Stage 2 as built (2026-09-27, `feat/canvas2d-stage2`)

What stage 2 ships, and where it differs from the text above. `QUEUE.md` lists what it still owes.

- **The recorders.** Both recorders have every stage-2 member of §3:
  - text: `font` (Chrome's parse and serialisation), `textAlign`, `textBaseline`, `direction`, `letterSpacing`, `wordSpacing`, `fontKerning`, `fontStretch`, `fontVariantCaps`, `textRendering`, `fillText`, `strokeText` and `measureText`;
  - images and pixels: `drawImage` (three overloads), `createPattern` with `setTransform`, `createImageData`, `putImageData` (with its dirty rectangle), `ImageData`;
  - paths and paint: `Path2D` (every method, `addPath`, SVG path data), `createConicGradient`, shadows, smoothing, and the five clip-extent operators.

  `canvas/tests/cases.txt` gains ten stage-2 cases; headless Chrome agrees with all 24, and the two recorders' lists agree record by record. The list gains records 22–25, 33–37, 60–62, 70–72 and 80–87 (`canvas/src/list.rs`); no existing record changed meaning, so its version stays 1.
- **Text (D8).** The recorder measures through the environment's text engine and resolves alignment, baselines and `maxWidth` itself, with Chrome's formulas (`canvas/src/font.rs`). The list carries a run's left end on its alphabetic baseline and its squeeze, so each replayer only draws a line. The engines:
  - **Web:** the page's own context, called from the module realm; Chrome measures what it draws. A declared face that loads redraws the canvases that drew text (cause `"font"`).
  - **Apple:** Core Text through a new callback, `exact_set_canvas_text`, on the executor's thread; one Core Text line measures and draws.
  - **Linux:** a canvas font system of its own (cosmic-text), loaded from the host catalog's faces on the first text; glyph outlines are drawn as tiny-skia paths.

  Families resolve as box text resolves them: declared faces by name first, then the generics, then an installed family by name, then serif (Chrome's default).
- **Images (D9).** An image handle is a string: the URL or asset an `image` node's `src` takes. Readiness is the runner's image table, read at the call. A handle not yet decoded draws nothing and is requested of the host. Its decode redraws every canvas that asked for it (cause `"image"`), and `state.canvas[].brokenImages` names the ones that failed. The web loads handles into the page's own table, Apple decodes off the main thread (assets and http(s)), and Linux decodes PNG assets synchronously.
- **Path2D** crosses as segments at its paint, transformed by the matrix current then, followed by the paint (records 80–87). The replayers keep no live path objects.
- **The TypeScript seam.** `measureText` and `drawImage` reach the host through two new host ops (8, 9) natively, and through the page's functions on the web. `Ctx2D` gains stage 2's members, with `drawImage` and `createPattern` taking an `ImageHandle`.
- **Deviations resolved:** `currentColor` is the canvas node's `color`, and `direction = "inherit"` its `direction` (§8.1's (5)). `lab()`, `lch()`, `oklab()`, `oklch()` and `color()` parse; they keep Chrome's serialisation and draw as Chrome's sRGB pixels (§8.1's (6)).
- **Declared differences:**
  - `em`, `rem`, `%`, `larger` and `smaller` in `font` resolve against 10px, a detached canvas's font, as Chrome does for one outside a document.
  - Handles are discovered at the call, not by a surface argument's type.
  - Canvas images have their own cache on each host, outside LLP 1010 §6's budget.
  - Linux draws colour glyphs as outlines, and decodes PNG only.
  - Apple ignores `fontStretch` and `textRendering`, and draws oblique as italic.
  - A shadow under a clip-extent operator may differ from Chrome's on Linux.
  - A Rust data crate that draws text on the web measures with the estimator: no web text engine is set on the wasm runner.
- **Parity** (`bun scripts/smoke.mjs canvas`, 94 crops a host). Mean |Δ| per crop (/255) and pixels off by more than 32:
  - **The API oracle** (the web against `direct.html`): mean 0.21, worst 1.66; 0.22% mean, 1.59% worst.
  - **Linux** (cpu and gpu identical): pages 1–3 unchanged (worst 0.81); page 5 mean 0.48, worst 1.46; page 4 (text) mean 1.79, worst 4.94, 8.1% worst. That is inside SVG's Linux text band (mean 14, 16%), which the smoke now holds Linux text to, as §4 says.
  - **macOS:** mean 0.56, worst 5.31; at 2× against `direct.html` mean 0.48, worst 3.97.
  - **iOS:** mean 0.50, worst 6.04; at 3× mean 0.37, worst 3.92.
  - Every Apple crop is within §4's default bands except `fx-textstyle` on white: macOS 5.31 (7.84% off), macOS 2× 3.97 (6.29%), iOS 6.04 (9.28%), iOS 3× 3.92 (5.36%). Its glyph positions and bounds match Chrome's. The residue is the 1 px `strokeText` "Stroke", which carries 1.66× Chrome's ink.
- **The Apple text band (Charlie, 2026-09-27).** Charlie ruled "(b) is ok": emulate Chrome's `strokeText` by stroking each glyph's Core Text outline as an ordinary Core Graphics path with the context's line style, falling back to (a), a declared Apple text band, if three rounds did not bring `fx-textstyle` within §4's bands. The rounds:
  1. **Outlines stroked as a path.** This is how the replayer already draws `strokeText`: the glyph outlines from `CTFontCreatePathForGlyph`, stroked with the context's width, join, cap, miter and dash. It measures as above.
  2. **Skia's glyph-mask contrast.** A probe drew the stroked outlines into a coverage mask with a gamma curve, against Chrome's own 1× crop. It lowers the ink but not the difference: at best 11.75/255 on the stroke's region, against 18.24 without the curve.
  3. **Where the residue is.** The same probe varied the line width. Chrome's stroke carries the ink of a Core Graphics stroke about 0.6 px wide, and still differs by about 6/255 in shape at the best width. So the difference is Skia's rasterization of glyph masks, not the stroke's geometry, and a path stroke cannot remove it.

  So Apple takes (a). `smoke.mjs canvas` holds Apple's `fx-text*` crops to a declared band of mean 8/255 and 12% of pixels off by more than 32. It is above the measured worst (6.04, 9.28%), and tighter than Linux's text band (14, 16%). The replayer keeps drawing `strokeText` as outlines stroked as a path, which is Chrome's model.
  - **Caltrain's line map** is unchanged: Linux 0.25 (cpu) and 0.31 (gpu), macOS 0.26, iOS 0.23.

## 9. `rules/DEFERRED.md`: the admission

Admitted by Charlie on 2026-09-27 (§0.1). The text below is in §Components, after the SVG entry:

> **Expanded (Charlie, 2026-09-27: "Core Graphics everywhere", by the web's name):** the HTML Canvas 2D context on the `canvas` tag (LLP 1056). A surface in the app's data module, TypeScript or Rust, draws with `CanvasRenderingContext2D`'s own names and rules. Its recorded calls are replayed in order by the browser, Core Graphics or tiny-skia into the canvas's kept bitmap. Unblocks computed 2D drawing (charts, sparklines, maps, custom controls) on every host without a GPU module, with Chrome as the oracle. Take: Caltrain's line map leaves wgpu. Its `map` surface and shader are deleted, and it is redrawn as a Canvas 2D surface in Caltrain's data crate, so one fewer GPU path exists after than before. Still refused: a drawing language in Contract (SVG is the declarative one), readback (`getImageData`, `toDataURL`, `toBlob`), `ctx.filter`, and an app-visible `OffscreenCanvas`.

**The take:**
- **Proposed.** Caltrain defines v1, so the admission has a consumer inside the v1 bar. The GPU module stays for the aurora, glass and deck, which are shaders. Stage 1 delivers the migration (§8).
- **Considered and dropped (r2).** 1009's unbuilt `Custom(u16)` extension point, and closing the GPU-row-pooling queue item. Neither is a doing-list line, and GPU surfaces may still want the first. Both reviewers said so.

## 10. Questions for Charlie, as ruled (r3)

1. **Admit Canvas 2D with the Caltrain map as the take?** Ruled yes: "seems reasonable". §9's text is in `rules/DEFERRED.md`.
2. **Core Graphics on Apple rather than tiny-skia on every native host?** Ruled yes, as recommended. The costs stand: Apple and Linux each match Chrome rather than each other, and `ctx.filter` stays refused, because sharing SVG's tiny-skia islands would give Apple a third raster path.
3. **Approve the fixture apparatus** (`apps/canvas-gallery` with its direct-API page, a canvas mode in `apps/sparkline` at stage 3, and `smoke.mjs canvas` over the generalised SVG comparator)? Ruled yes. It adds no blocking check, and it generalises rather than copies.
4. **Accept the declared deviations?** Ruled "ok" to these revisions:
   - CSS-pixel coordinates over a device backing store stay the default, and an explicit bitmap size is the web's canvas exactly (D6);
   - a scale change clears and redraws; stretching the old pixels first is the recorded future option (D6);
   - draws are not atomic: a throw keeps what was recorded before it, as Chrome does (D4);
   - limits stay, at what browsers enforce, with a measured total budget (D4).

**The spelling of the bitmap attributes (Charlie, 2026-09-27):** "let's do bitmap-width/bitmap-height for now i guess but keep an eye on whether agents stumble on it." HTML spells them `width` and `height`, which Contract already uses for the box's CSS size, so they stay `bitmap-width` and `bitmap-height`. Revisit trigger: agents (or people) writing `width`/`height` on a canvas meaning its bitmap. Contract should then say so by name, and the web's spelling (`width`/`height` meaning the bitmap on `canvas` only) is the alternative. QUEUE.md tracks it.

## 11. Appendix: the probes (2026-09-27, Apple M5 Max)

The total budget's footprint (`swiftc -O main.swift -o probe && ./probe 2064 2752 8 && ./probe 1290 2796 8`), r3. It reads `phys_footprint` before and after holding `n` canvases, each drawn, published to a `CALayer` as `makeImage()`, drawn and published again, then drawn once more:

```swift
import CoreGraphics
import Foundation
import QuartzCore
func footprint() -> Double {
  var info = task_vm_info_data_t(); var count = mach_msg_type_number_t(MemoryLayout<task_vm_info_data_t>.size / MemoryLayout<natural_t>.size)
  let kr = withUnsafeMutablePointer(to: &info) { $0.withMemoryRebound(to: integer_t.self, capacity: Int(count)) { task_info(mach_task_self_, task_flavor_t(TASK_VM_INFO), $0, &count) } }
  precondition(kr == KERN_SUCCESS); return Double(info.phys_footprint) / 1048576
}
let (w, h) = (Int(CommandLine.arguments[1])!, Int(CommandLine.arguments[2])!)
let n = Int(CommandLine.arguments[3])!
let cs = CGColorSpace(name: CGColorSpace.sRGB)!
let base = footprint()
var keep: [(CGContext, CALayer)] = []
for i in 0..<n {
  let ctx = CGContext(data: nil, width: w, height: h, bitsPerComponent: 8, bytesPerRow: 0, space: cs, bitmapInfo: CGImageAlphaInfo.premultipliedFirst.rawValue | CGBitmapInfo.byteOrder32Little.rawValue)!
  let layer = CALayer()
  for pass in 0..<2 {
    ctx.setFillColor(red: CGFloat(i % 3) / 2, green: CGFloat(pass), blue: 0.5, alpha: 1)
    ctx.fill(CGRect(x: 0, y: 0, width: w, height: h))
    ctx.setStrokeColor(red: 0, green: 0, blue: 0, alpha: 1); ctx.setLineWidth(3)
    ctx.stroke(CGRect(x: 10, y: 10, width: w - 20, height: h - 20))
    layer.contents = ctx.makeImage()
  }
  ctx.fill(CGRect(x: 0, y: 0, width: 4, height: 4))
  keep.append((ctx, layer))
}
let per = (footprint() - base) / Double(n)
print(String(format: "%dx%d px, %d canvases: %.1f MiB each (one bitmap = %.1f MiB)", w, h, n, per, Double(w*h*4)/1048576))
```

It printed 22.5 MiB each at 2,064 × 2,752 (one bitmap 21.7 MiB) and 14.5 MiB each at 1,290 × 2,796 (13.8 MiB). An offscreen layer is never committed to the render server, so the probe sees about one bitmap per canvas. The budget charges two.


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
