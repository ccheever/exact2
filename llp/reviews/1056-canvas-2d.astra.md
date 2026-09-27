# Review: LLP 1056 r1, 2026-09-27 (astra)

- **Family:** OpenAI — `gpt-6-astra` via `codex exec`, reasoning effort max, read-only sandbox, `-C` a detached review worktree at 2a81868d (r1); 2026-09-27T07:26:54Z to 2026-09-27T07:39:20Z; 231,397 tokens.
- **Method:** the shared brief (sha256 `9c0768ecfeb328ba8bbb48c2908761fadf653e073be7acb7f5fa02e4d7332559`) verbatim; one round; blind to the other review.
- **Reviewed revision:** LLP 1056 r1.
- **Redacted:** nothing. Absolute links into the review worktree are kept as the reviewer wrote them.
- **Verdict:** build with named changes.
- **Disposition:** LLP 1056 §0 (r2). Every finding below is accepted and answered there, in the section named, with these exceptions:
  - "Specify Promise returns": answered by refusing them (D4: `draw` is synchronous; a returned promise is a `TypeError`), rather than supporting asynchronous draws.
  - "Distinguish later stages from permanent refusals for `alpha: false`, `desynchronized`, `willReadFrequently`": each now has its own row and trigger in §3.
  - The removal of `Custom(u16)` as a take: dropped in r2 (§9), as both reviewers advised.

---

## Review (verbatim)

I agree with the direction, the single `canvas` tag, and the proposed backends. **R1 needs substantive changes before implementation**, chiefly to Canvas semantics, draw/state lifetime, web layout integration, worker queries, and validation.

This is a static audit of [LLP 1056 r1](/Users/ccheever/projects/exact2-wt-canvas2d-review/llp/1056-canvas-2d.rfc.md:1) at `2a81868d`. I made no edits and ran no builds, tests, or network requests. References below to `1056:…` identify lines in that document.

1. **HTML Canvas semantics: several statements are correct, but the proposed translation is not yet correct enough.**

   - **Defaults — agree with the examples at `1056:227`.** Black fill/stroke, line width 1, butt caps, miter joins, miter limit 10, and `10px sans-serif` are correct. TypeScript’s `lib.dom` supplies types, however, **not defaults or behavior**. Specify the complete initial state, including `textAlign: "start"`, `textBaseline: "alphabetic"`, `direction: "inherit"`, smoothing enabled with quality `"low"`, source-over compositing, alpha 1, empty dash list, transparent shadows, identity transform, empty path, and initial clip.

   - **Non-finite arguments — disagree with the blanket rule at `1056:49` and `1056:108`.** Canvas has member-specific conversion, no-op, and exception rules. Many path methods ignore non-finite coordinates; that does not make “drop every call containing a non-finite number” a conforming validator. Gradient and pixel APIs have different validation rules. Negative finite arc radii throw; invalid color-stop offsets and colors throw; invalid style assignments commonly leave the previous value unchanged. These behaviors must happen **during the author’s call**, so `try/catch` and subsequent getters observe them. Validation after `draw` returns is too late.

   - **Bitmap persistence — agree; state persistence is missing.** Canvas retains drawing state, the state stack, clipping region, and current path as well as pixels. A later `draw` must resume the same context. A `save()` in one draw followed by `restore()` in another is legal; an unmatched `restore()` is a no-op. Do not interpret the proposed “save/restore balance” checks as requiring balance within each recorded list.

   - **Clearing and `reset()` — materially underspecified.** Setting a browser canvas’s bitmap width or height resets its context, even when assigning the existing value. CSS-only resizing does not do that. `reset()` clears the bitmap and resets drawing state, stack, clipping, and current path. `clearRect()` instead obeys the transform and clip, ignores ordinary paint/compositing settings, and does not reset the current path. D3 currently discusses clearing pixels without deciding the corresponding recorder and host-state resets.

   - **The Core Graphics path mapping needs another explicit rule.** Canvas `fill()`, `stroke()`, and `clip()` preserve the current path. Core Graphics’ corresponding context-path operations consume it. The replayer needs independently retained path geometry or equivalent restoration. It must also distinguish transformations applied while building the current path from transformations applied when painting a `Path2D`. These are central semantics, beyond §1’s four listed differences.

   - **Compositing — partially correct, with two concrete errors.** The five named operations—`source-in`, `source-out`, `destination-in`, `destination-atop`, and `copy`—need treatment beyond the covered source shape. But their destructive extent is constrained by the **current clipping region**, not unconditionally the whole canvas. Also, `xor` at `1056:233` is not an unbounded operator: transparent source outside the shape preserves the destination. `destination-over`, `source-atop`, `destination-out`, and `lighter` disappear from both stage inventories. Assign them a stage or explicitly refuse them.

   - **`shadowBlur` — agree that the specified Gaussian standard deviation is half the value.** Disagree that “the replayer converts it” resolves the implementation. Canvas shadows have transform behavior that a direct `CGContext.setShadow` mapping must account for; an author’s CTM must not inadvertently scale the blur and offsets. State the coordinate convention, backend approximation, and measured tolerance. A blur parameter conversion alone is insufficient.

   - **`arcTo` — the heading identifies a special case but never specifies it.** With an existing current point, zero radius produces the line-to-first-control-point result; an empty subpath, coincident points, and collinear points have their own specified handling. A negative radius throws. Large radii are not simply clamped to fit between the supplied points. Likewise, `roundRect` requires its actual corner-radius normalization and negative-dimension rules; a generic rounded rectangle is not the whole API.

   - **`measureText` — the seven proposed fields are a reasonable subset, but not the declared DOM return type.** The four actual bounding-box fields plus two font bounding-box fields and width are correctly counted. The later fields include `emHeightAscent` and `emHeightDescent`, not just baselines. Metrics need their specified alignment/baseline-relative meaning and must not be multiplied by the drawing CTM. “`maxWidth` compresses horizontally, as the spec says” overstates the prescription: horizontal compression is an allowed implementation strategy, not the only permitted strategy.

   - **Incomplete images — agree that an incomplete image can draw nothing.** Distinguish incomplete, broken, zero-sized, and otherwise invalid sources. Automatic redraw on decode is Exact behavior, not a Canvas behavior. **`ImageData` is not a valid `drawImage` source**; its inclusion at `1056:201` contradicts the claimed HTML interface.

   - **`Path2D` and `getTransform` — agree with their inclusion, but specify their object semantics.** `getTransform()` returns a detached matrix snapshot; mutating it does not change the context. `Path2D` needs construction from another path, mutation, `addPath` transformations, and the drawing/hit-testing overloads—not only SVG-string parsing. Gradients and patterns similarly have identity and mutation semantics; a style getter can return their object, not always a serialized color string.

   The existing SVG geometry is useful but is not a Canvas geometry implementation: it uses `f32` coordinates, and its private endpoint-arc routine returns immediately for equal endpoints. Direct reuse cannot implement Canvas full-circle arcs. See [kernel/src/svg/path.rs:127](/Users/ccheever/projects/exact2-wt-canvas2d-review/kernel/src/svg/path.rs:127) and [kernel/src/svg/path.rs:250](/Users/ccheever/projects/exact2-wt-canvas2d-review/kernel/src/svg/path.rs:250).

2. **D1/D2: agree with the authoring model and one tag; disagree with the compatibility and integration claims as written.**

   Data modules are the right place for imperative drawing. Keeping Contract free of another drawing language is a sound decision. Choosing the context through surface ownership is also coherent.

   **`Pick<CanvasRenderingContext2D, …>` does not describe this subset accurately.** Picking `fill` includes its `Path2D` overload before stage 2; picking `fillStyle` admits patterns before stage 2; picking `measureText` promises complete `TextMetrics`; and picking `drawImage` neither accepts URL strings nor `ImageData`. Use DOM-derived types for unchanged members and explicitly narrow or replace the differing signatures. Provide the required runtime objects in Hermes. Likewise, “web-sys names” is a good Rust convention, but “ports by changing the type” is not supportable without matching overloads, argument objects, return types, and error behavior.

   **The purported existing CSS parsers are insufficient.** The kernel color parser accepts hex, RGB notation, and `transparent`; even `"red"` is unsupported. Filtering through it would reject valid Canvas colors on every host. I found no CSS `font` shorthand parser in the inspected kernel/compiler code; the kernel text interface takes already-resolved font fields. See [kernel/src/style.rs:723](/Users/ccheever/projects/exact2-wt-canvas2d-review/kernel/src/style.rs:723) and [kernel/src/text.rs:90](/Users/ccheever/projects/exact2-wt-canvas2d-review/kernel/src/text.rs:90). Specify the missing parser work instead of counting it as reuse.

   That also exposes a dependency contradiction: a dependency-free `exact-canvas` cannot directly invoke kernel color, font, and SVG parsers. Decide which operations are leaf functionality and which are injected services or runner-side adapters.

   **Surface ownership must cover the complete composition.** Check duplicate ownership across TypeScript, Rust, and all GPU artifacts; bind ownership and context type into the admitted plan/module generation. Define how roster metadata is obtained without executing app code before first pixel. Also preserve or explicitly reject the existing named-argument form: [runner/src/instance.rs:244](/Users/ccheever/projects/exact2-wt-canvas2d-review/runner/src/instance.rs:244) carries argument mode and names, whereas D1 supplies only `unknown[]`.

   D2 should retain the existing publish-after-success rule. Surface updates are currently published only from accepted commits, as documented and implemented at [host/web/src/host.rs:830](/Users/ccheever/projects/exact2-wt-canvas2d-review/host/web/src/host.rs:830). A draw must not escape a subsequently refused transaction.

   Finally, the “only linked when used” promise needs a concrete capability hook. The runner already has this pattern in [runner/src/runner.rs:350](/Users/ccheever/projects/exact2-wt-canvas2d-review/runner/src/runner.rs:350); an unconditional recorder/validator call from common runner code would not establish the promised omission.

3. **D3/D4/D5: agree with recording into a persistent bitmap and explicit frame time; the execution protocol is the largest blocker.**

   **“The runner calls after kernel layout” does not work on the web.** The browser performs layout there; the kernel does not supply those boxes. This is explicit at [host/web/src/host.rs:231](/Users/ccheever/projects/exact2-wt-canvas2d-review/host/web/src/host.rs:231) and [host/web/glue.js:952](/Users/ccheever/projects/exact2-wt-canvas2d-review/host/web/glue.js:952). Specify a host-driven sequence: commit changes, obtain the host’s content-box geometry, dispatch drawing with those facts, then replay. The web needs geometry feedback and resize observation; native hosts can use their completed layout.

   **The first-draw guarantee contradicts startup and existing executor behavior.** D4 promises a main-placed canvas never appears without its drawing, while the replayer and TypeScript module load after first paint. Moreover, the current browser main realm uses deferred serialized turns, not an unconditional synchronous call: [host/web/module-glue.js:171](/Users/ccheever/projects/exact2-wt-canvas2d-review/host/web/module-glue.js:171). Activation is explicitly post-pixel at [host/web/glue.js:1445](/Users/ccheever/projects/exact2-wt-canvas2d-review/host/web/glue.js:1445).

   Declare the initial transparent frame and the behavior while the executor is unavailable or busy. If a new synchronous draw path is intended after activation, specify how it respects the existing module’s turn ownership. A worker reply arriving a turn later does not necessarily imply exactly one intervening presented frame, either.

   **Persistent state requires an acceptance protocol, not just a list.** Give each request/reply a session/module incarnation, canvas lifetime, bitmap-size generation, argument revision, and draw sequence. Decide when the executor’s state becomes committed. Otherwise a rejected or stale list leaves its recorder ahead of the host.

   A concrete failure case: draw A changes the transform; the canvas resizes; A’s reply is discarded; draw B starts with A’s transform while the host has reset to identity. Another: dropping an older accumulated drawing list while keeping its successor loses pixels. Coalesce requests **before execution**; do not treat executed incremental lists as interchangeable snapshots. Define ordering with `answer`/`parse` and other canvases sharing the module. The existing worker contract already distinguishes execution lifetime, supersession, and retirement: [llp/1027.002-optional-worker-execution.rfc.md:304](/Users/ccheever/projects/exact2-wt-canvas2d-review/llp/1027.002-optional-worker-execution.rfc.md:304).

   **Errors need one explicit policy.** D2 says failures leave the background; §5 says they preserve the previous bitmap. Creation, an ordinary failed draw, failure after resize, and replay allocation failure are different cases. Discarding all drawing before an uncaught exception also differs from immediate browser Canvas execution, whose earlier successful calls have already taken effect. An atomic-draw policy is defensible, but declare it and restore recorder state consistently. Specify Promise returns, retry triggers, and termination of frame requests after failure.

   **The 1 MiB list bound is necessary but insufficient.** Enforce it during recording, including strings and pixel payloads, before unbounded allocation or transfer. Also bound aggregate queued bytes, persistent paths, save/clip depth, gradient/path objects, canvas dimensions, total bitmap bytes, and temporary compositing buffers. A short list can allocate an enormous bitmap or perform expensive drawing; many small lists can grow a persistent path forever.

   **Getters must be executor-owned, synchronous services.** Main-thread presentation does not imply worker-thread measurement can synchronously call back into the UI. The worker RFC explicitly prohibits that deadlock shape at [1027.002:283](/Users/ccheever/projects/exact2-wt-canvas2d-review/llp/1027.002-optional-worker-execution.rfc.md:283). Use worker-owned measurement state with the same admitted fonts, or another fully specified arrangement. The existing Linux text engine is `Rc<RefCell<TextEngine>>`, not a transferable service: [host/linux/src/text.rs:489](/Users/ccheever/projects/exact2-wt-canvas2d-review/host/linux/src/text.rs:489). A browser Worker also needs its own loaded font set; a scratch `OffscreenCanvas` alone does not provide the page’s declared fonts.

   **D5’s clock model is sensible, with an important distinction.** `clock settle` should not seek an arbitrary program until it stops requesting frames. It must nevertheless complete—or boundedly report pending—the draw requested for the observed clock before claiming its pixels are available. Otherwise worker placement makes `clock` followed by `screenshot` race. Existing Apple agent operations explicitly settle surface presentation at the landed clock: [host/apple/Sources/ExactKit/Agent.swift:254](/Users/ccheever/projects/exact2-wt-canvas2d-review/host/apple/Sources/ExactKit/Agent.swift:254).

   Specify ordering against timer settlements, geometry changes, and image readiness; prevent duplicate sampling at one clock during settle’s fixed-point loop. Agree that accumulating canvases remain frame-count dependent. Their deterministic inputs include ordered resource versions and initial context state, not merely list bytes.

   Agree that canvas contexts should restart on successful reload. Distinguish that from resetting every module variable: mixed composition can retain the Rust half during a TypeScript-only replacement. Cancel old subscriptions and requests, and reject all old-lifetime results.

4. **D6/D7: agree with the backend choices; tighten the coordinate and ownership contracts.**

   **Automatic CSS-coordinate drawing into a device-resolution bitmap is a reasonable declared deviation.** The actual deviations are automatic backing-store sizing/resetting and hiding the device transform. Identity after `resetTransform()` is normal Canvas behavior, not itself a deviation.

   Specify content-box measurement, fractional-size rounding, zero-sized canvases, dimension limits, and how a scale change is detected without a CSS-size change. Reset both executor and host state together. A CSS-size change that rounds to the same backing dimensions still needs a defined result.

   **`putImageData` conflicts directly with D6.** It is a raw pixel operation: it does not use the current transform, smoothing, global alpha, compositing operator, or clip. The stage-1 claim of smoothing “for `putImageData` scaling” at `1056:225` is wrong. My recommendation is to preserve raw backing-pixel semantics and expose read-only backing dimensions/scale through `Frame`. If Exact instead makes pixel writes CSS-scaled, declare that separate deviation explicitly.

   **Main-thread Core Graphics replay is a good initial choice.** It simplifies ordering and atomic presentation with neighboring views. Core Graphics bitmap drawing does not inherently require the main thread; ownership and presentation ordering justify this choice. Keep background replay as a measured later option, but measure **aggregate per-frame replay cost**, not only whether one canvas exceeds 1 ms. Twenty-one individually cheap canvases can already consume several milliseconds.

   **Swift replay with Rust validation is also appropriate.** The Apple Rust host does have `#![deny(unsafe_code)]`: [host/apple/src/lib.rs:33](/Users/ccheever/projects/exact2-wt-canvas2d-review/host/apple/src/lib.rs:33). Keep platform drawing in Swift. However, Rust validation does not itself guarantee the lifetime, alignment, endianness, or ownership of a later `UnsafeRawBufferPointer`; those belong in the binary handoff contract.

   The binary handoff is new work. The current Hermes call interface returns strings, not a typed drawing buffer: [js/src/engine.rs:191](/Users/ccheever/projects/exact2-wt-canvas2d-review/js/src/engine.rs:191). Define the transport and its versioning on native, browser, Worker, and admitted Rust-module executors.

   **tiny-skia beneath both Linux painters is coherent**, but mutable bitmap upload and retained snapshots need explicit handling. The existing GPU image cache keys by immutable bitmap identity and uploads on first insertion: [host/linux/src/gpu/images.rs:23](/Users/ccheever/projects/exact2-wt-canvas2d-review/host/linux/src/gpu/images.rs:23). Reusing that path with a mutated pixmap can display stale pixels. Use a content revision or replacement snapshot; retained regions must retain the captured revision rather than alias future mutations.

5. **D8/D9/D10 and §6: agree with the broad approach; text, resource lifetime, pooling, and the estimates need corrections.**

   **Text requires a Canvas-specific query over the host engine.** The existing kernel `TextMetrics` has width, height, and first baseline only; it cannot supply the proposed Canvas metrics unchanged: [kernel/src/text.rs:231](/Users/ccheever/projects/exact2-wt-canvas2d-review/kernel/src/text.rs:231).

   Specify Canvas whitespace handling, alignment-relative ink bounds, baseline offsets, fallback runs, empty strings, kerning, and `maxWidth`. Reusing box text measurement does not automatically implement these. Worker measurement and presentation must share font identity and font readiness. Add invalidation when a relevant font becomes available or changes; D4 currently lists only image decoding.

   The promised Linux “SVG text outlines” are not implemented as described: the SVG painter currently uses paragraph painting, with stroke and gradient text explicitly owed at [host/linux/src/paint/svg.rs:58](/Users/ccheever/projects/exact2-wt-canvas2d-review/host/linux/src/paint/svg.rs:58). Apple’s current SVG outline path skips glyphs without outlines at [host/apple/Sources/ExactKit/SvgText.swift:80](/Users/ccheever/projects/exact2-wt-canvas2d-review/host/apple/Sources/ExactKit/SvgText.swift:80). Include raster/color-glyph fallback rather than losing emoji. Matching the host’s text engine is useful, but does not establish Chrome metric or pixel parity.

   **Image handles are a reasonable Exact adaptation.** Name their type and deviation, remove `ImageData` from `drawImage`, and specify all supported overloads, natural dimensions, crop handling, negative dimensions, decoding failure, and source lifetime.

   Decode-triggered redraw needs special care for accumulation: rerunning the whole draw may paint already-rendered translucent content twice. Document that automatic invalidation behavior and how authors obtain a clean redraw. Pin image versions/readiness for an executed draw; otherwise a recorded list’s meaning changes depending on when replay reaches its handle. Define cancellation and cache accounting after destruction or reload.

   Likewise, snapshot mutable `ImageData` at the drawing operation’s semantic boundary. Define ordered mutation of `CanvasGradient`, `CanvasPattern`, and `Path2D`; retaining a pointer and reading its final contents at replay would change earlier operations.

   **Pooling is plausible but requires more than adding `"canvas"` to the kind set.** The current pool rejects separate containers, overlays, and canvas input as well as Metal views. See [NodePoolIOS.swift:117](/Users/ccheever/projects/exact2-wt-canvas2d-review/host/apple/Sources/ExactKit/IOS/NodePoolIOS.swift:117) and [NodePoolIOS.swift:153](/Users/ccheever/projects/exact2-wt-canvas2d-review/host/apple/Sources/ExactKit/IOS/NodePoolIOS.swift:153). That matters especially for the proposed animated child.

   Define a 2D-specific reset contract covering context state, path, clip, subscriptions, pending work, animation requests, mount time, and inspection data. Clear the **backing pixels**, not just `CALayer.contents`; otherwise retained pixels can reappear on reuse. Key allocations by backing dimensions and format, and account for parked buffers and the separate free list together.

   **The fill-policy guarantee must acknowledge asynchronous drawing.** “An owed row is presented drawn” conflicts with worker placement and unloaded/busy executors. State the exception or change the readiness model. This decision belongs in stage 1, even if pooling performance is measured in stage 3.

   **§6 honestly labels estimates, but several conclusions are too strong:**

   - The 89/200 KiB figures correctly describe tightly packed single RGBA buffers. They do not include stride alignment, snapshots, temporary surfaces, pooled buffers, or GPU uploads.
   - “The copy-on-write image adds a transient copy only during a replay” is unjustified. The layer/render server can retain image storage while the mutable context retains another buffer. Existing text raster code explicitly discusses this copying problem at [TextRaster.swift:125](/Users/ccheever/projects/exact2-wt-canvas2d-review/host/apple/Sources/ExactKit/TextRaster.swift:125). The appendix’s image is short-lived and never assigned to a layer, so it does not measure that lifecycle.
   - Consequently, **42 MB is an illustrative single-buffer subtotal**, not a supported footprint estimate. Replacing SVG also removes some SVG storage and adds other Canvas storage; simply adding one bitmap per row to the SVG result is incomplete.
   - The 3.8–4.0 µs recorder probe measures a small preallocated numeric writer. It omits meaningful state management, color/font validation, object handling, binary crossing, and runner validation. Label it a recorder microbenchmark, not the implemented draw cost.
   - The quoted 45 ns host-call source, `metrics/ibex2-speed.jsonl`, is absent from this checkout. Regardless, call overhead does not measure shaping, argument conversion, or returned metric allocation.
   - The 4.7× ratio is accurate for the reported **raster-only** numbers. Including the reported CG `makeImage()` cost changes that comparison to about 3.1×, still before real presentation and transport.
   - `21 × 120 µs × 120 Hz ≈ 302 ms/s` is correct arithmetic. It is **additional drawing work**, not an app-total estimate. Retaining roughly 114 ms/s of baseline main-thread work would put the illustrative total near 416 ms/s. Compare total CPU with total CPU and main-thread CPU with main-thread CPU.
   - The M1/M5 multiplier is an explicit hypothesis, not measured evidence. Moving replay to a worker would not move all presentation, validation, transfer, and upload work with it.
   - Historical benchmark revisions are not controlled comparisons. In particular, collection retirement behavior has changed; the retained-row correction is recorded at [LLP 1050.000:249](/Users/ccheever/projects/exact2-wt-canvas2d-review/llp/1050.000-choosing-the-fill-tradeoff.rfc.md:249).

   Keep the numbers, qualify them accordingly, and require the actual end-to-end measurement to include persistent layer snapshots, mounted and pooled canvases, both pulse variants, and the same revision/device conditions.

6. **§3/§4/§5/§8: agree with a staged subset and eight agent operations; the proof and stage boundaries need strengthening.**

   **The refusals are generally sensible.** Readback, app-visible element ownership, and filters can wait. Do not describe worker placement as fully equivalent to `OffscreenCanvas`: here the worker records while another thread owns and replays the bitmap. Unsupported methods and enum values need one consistent author-visible policy on every host. Complete the compositing inventory and distinguish later stages from permanent refusals. The refusal table’s wide-color trigger also does not explain the separate choices around `alpha: false`, desynchronization, and read-frequency hints.

   **The shared-list browser replay is a backend oracle, not a complete API oracle.** If the recorder or validator incorrectly drops a valid operation, Chrome receives that same damaged list and agrees with native. The restricted color parser is an immediate example.

   Add an independent browser comparison that invokes the authored operations directly, including getters, exceptions, and state changes. Keep the recorded-list comparison for the replayers. These test different boundaries.

   **The current SVG comparator is useful infrastructure, but insufficient unchanged.** It compares RGB, downsamples to one pixel per point, permits positional registration, and currently forces Linux’s CPU painter. See [scripts/svgparity.mjs:21](/Users/ccheever/projects/exact2-wt-canvas2d-review/scripts/svgparity.mjs:21), [scripts/svgparity.mjs:37](/Users/ccheever/projects/exact2-wt-canvas2d-review/scripts/svgparity.mjs:37), and [scripts/svgparity.mjs:50](/Users/ccheever/projects/exact2-wt-canvas2d-review/scripts/svgparity.mjs:50).

   Add exact or numeric checks for geometry, alpha/compositing, transforms, metrics, and errors; test multiple backgrounds where screenshots obscure alpha. Include native-resolution DPR cases and both Linux painters. Sparse missing content can pass a generous whole-image average, and registration can hide coordinate errors. Treat inherited SVG thresholds as provisional targets, not measured Canvas tolerances.

   The proposed “accumulation” fixtures explicitly clear before drawing, so they do not test accumulation. Require multi-draw sequences covering persistent paths/state, reset, clipped clearing, resize/scale changes, rejected draws, stale worker replies, image readiness, and reload. Use explicit clock samples for finite draw-in animations; Canvas cannot infer their finish time from a boolean frame request.

   **No ninth agent operation is needed — agree.** Add bitmap dimensions/scale, pending versus presented generation, readiness, and failure state to the existing inspection output. A 200-line limit also needs a byte limit because one text argument can be enormous.

   The macOS screenshot assertion needs narrowing. A model-layer screenshot can show the bitmap itself, but not necessarily the presented CSS/Core Animation child used by the pulse comparison. The SVG comparator already requests window capture on macOS at [scripts/svgparity.mjs:61](/Users/ccheever/projects/exact2-wt-canvas2d-review/scripts/svgparity.mjs:61).

   **Reorder the stages around correctness.** Stage 1 must resolve startup, module turns, persistent state, worker measurement, bounded memory, failure, resize, and fill readiness. They are foundational, not stage-3 polish. The line-count estimates are speculative until this work is accounted for. Keep background replay and baked first-frame optimization conditional on measurement, but do not postpone deciding what the initial frame means.

7. **§9/§10: agree with the proposed Caltrain take; make the admission and delivery concrete.**

   Caltrain’s map is a credible Canvas 2D consumer. Its implementation constructs a line and train marker from simple quads, returns no continuous frame request, and already composes station labels as children. See [apps/caltrain/gpu/src/lib.rs:69](/Users/ccheever/projects/exact2-wt-canvas2d-review/apps/caltrain/gpu/src/lib.rs:69), [apps/caltrain/gpu/src/lib.rs:194](/Users/ccheever/projects/exact2-wt-canvas2d-review/apps/caltrain/gpu/src/lib.rs:194), and [apps/caltrain/gpu/shaders/map.wgsl:20](/Users/ccheever/projects/exact2-wt-canvas2d-review/apps/caltrain/gpu/shaders/map.wgsl:20).

   **Add the migration and deletion to §8’s delivery criteria.** It currently appears only as an admission candidate. Preserve the map’s geometry, background, input semantics, and child composition, then delete its old surface and shader. The remaining GPU surfaces mean this does not remove Caltrain’s whole GPU artifact.

   Removing the unbuilt `Custom(u16)` commitment is a possible additional scope reduction, not a technical consequence of Canvas drawing. The GPU-row-pooling queue item correctly does not satisfy the doing-list trade by itself. Nor should Canvas admission silently decide that every future GPU canvas in a list is unsupported.

   The owner’s agreement settles direction. §10 should retain the concrete admission/take and apparatus decisions while incorporating the technical decisions above into the RFC itself. The binding rules require the take in the same change and explicit approval for new apparatus: [rules/DEFERRED.md:348](/Users/ccheever/projects/exact2-wt-canvas2d-review/rules/DEFERRED.md:348) and [rules/RULES.md:64](/Users/ccheever/projects/exact2-wt-canvas2d-review/rules/RULES.md:64). The draft should record the actual ruling without treating this audit as authorization.

**Verdict: build with named changes—correct Canvas semantics and types; specify draw/state acceptance, startup, worker queries, and memory bounds; strengthen independent parity; qualify the estimates; and include the Caltrain migration in delivery.**
