# LLP 1149: The renderer — the painter on the GPU, on every platform

**Type:** RFC (a decision brief: each question carries a proposed answer; §5 lists the ones that are Charlie's)
**Status:** Draft r4, 2026-10-28. S0 was built the same day (§4 has every chunk's numbers; the two D15 verdicts are earned; the code is on `lane/ink` at `e83558a4d6`). Charlie ruled on it that evening: "land after fixes"; the renderer is to be used "some of the time but not all of the time, depending on what you want", which D12 now states as a per-app choice on every platform with today's painters kept (D13 amended); S2 before S1; §5's proposed answers stand where the proposer is at least 80% confident, and the two where he is not (Q14, the boot rule; Q21, the thresholds) went to the panel blind: GPT-6 Astra (94 and 91) and Muse Wonder Full 1.0 (70 and 75) agree on both and are folded (`llp/reviews/rfc-2026-10-28-1149-r4.{astra,muse}.md`; Grok's run died on a permission prompt); D6.3, D15, §4 and §8 carry their answers, and §5 gains Q25 from Muse. Landing follows the second blind code review's fixes (C6f). Charlie asked for this document and its research record (LLP 1148) on 2026-10-28, with the direction: "I'm up for doing a lot of stuff ourselves and repeating ourselves per platform if that gives us better performance and efficiency", and "we want this renderer to work on every platform"; the code is Astra's to write (Charlie, the same day: "use Astra xhigh to write code for this"). r1 went to three families blind: Muse Wonder Full 1.0 (xhigh) NOT READY, Grok 4.7 (xhigh) READY WITH CHANGES, GPT-6 Astra (xhigh) NOT READY (`llp/reviews/rfc-2026-10-28-1149-r1.{muse,grok,astra}.md`). r2 folded Muse; r3 folds Grok and Astra (§R). The three agree on the direction and on one change r1 did not propose: the performance case for a device layer per platform is unmeasured, so S0 builds one core and decides the device layer per platform by measurement (D5, D15). Sent to Charlie with §5.
**Systems:** a new crate, `host/ink` (`exact-ink`): the painter's GPU backend, its device trait, a wgpu device layer and direct ones where measured; `host/linux` (the paint walk and `paint/backend.rs`, unchanged in contract; `gpu.rs` gains kept rows as the control arm, then loses its platform branches as each passes; `raster.rs`, kept as the oracle and fallback; `text/catalog.rs`, the glyph rasterizers); `host/windows` (the window presents through a composition swap chain); a painted Apple carrier (new, S1, if S0 earns it); the build (`host/apple/build.mjs`, `host/windows/build.mjs`: shaders compiled at build); `scripts/app.schema.json` (`host.macos.presenter`, `host.ios.presenter`); LLP 1009 D1 (amended narrowly); `rules/DEFERRED.md` §Runtime (a replacement exception, §8)
**Author:** Claude (Fable 5.1) for Charlie Cheever
**Date:** 2026-10-28; r2 and r3 the same day
**Implementer:** Astra (codex `gpt-6-astra`, xhigh) lanes, orchestrated by Claude. S0 from 2026-10-28 in `exact2-wt-1126-gate` on `lane/ink`, in six chunks (§3), with its Windows spike on the NucBox; each later stage dated when S0's gate (D15) says it is earned
**Related:** LLP 1148 (the research this acts on; F1 to F10 are cited by number); LLP 1149.000 (the renderer on the web); LLP 1015 (the painter: §2 the walk, §3 text, §6 and §7 the measured costs, §8 the parity check); LLP 1015.000 and 1015.001 (the Windows window, the game DLL's direct presentation and its measured fast path); LLP 1056 §8.5 (vello's Canvas 2D module, which stays; its build-compiled kernels); LLP 1009 (the GPU module: D1 amended here, D5 the shader rule, §3 the module's web glue); LLP 1053.000 D2 (backdrop blur as three box passes over encoded sRGB); LLP 1115 (the principle) and 1115.004 (Windows: D2 the presenter stays Exact's, D4.3 the composition swap chain over Mica, D6.3 text, D7 the RichEdit, D13 the oracle); LLP 1076 §3.3 and §3.4 (Android's two hosts); LLP 1085.000 (text: Parley, fontique, swash); LLP 1043.000 (text damage); LLP 1030 D2 (the manifest); `rules/RULES.md` (the boot path compiles nothing; cold start 100 ms p50); `gpu/README.md` (the GPU module's ABI: no device or texture crosses it); `host/android/README.md` (the Android shell: arm64, Android 10 permitted, 12 default)

## R. What the reviews changed

### r3, from Grok's and Astra's reviews

| Change | Raised as | Where |
|---|---|---|
| **One core, the device layer per platform decided by measurement**: a wgpu device layer is the portable one and the control; direct DirectX 11 is built regardless (wgpu has no DX11, the composition swap chain and Direct2D need it); direct Metal and Vulkan are kept only where S0 and S3 measure a gain beyond noise on the same core. LLP 1009 D1 is amended narrowly: the host's renderer may own a device layer where measured | Astra Q2 and §7 (91), Grok Q1 and §7, Muse §7 | D5, D15, §3 |
| **Android keeps Vulkan** (through wgpu's presenter path, `gpu/present.rs`'s one-frame-deep thread); OpenGL ES only for a device without Vulkan, which Android 10's 64-bit requirement excludes | Grok Q4 (78), Astra Q4 (76) against Muse (70) | D5, S4 |
| **Acceptance thresholds predeclared**: what earns the replacement, and what stops it after S0 | Astra Q1 (99), Grok §7 | D15, §5 Q21 |
| The control arm is built: kept rows in the vello backend as retained scene fragments (`Scene::append` with a transform), on the same readback path, in S0 | all three §7 | §3 C4, D15 |
| Android has a platform rasterizer: `Canvas.drawGlyphs` (API 31+, `host/android/canvas/.../Player.kt`), admitted by the same audit; every platform rasterizer opens the font from the shaper's bytes; DirectWrite grayscale is `IDWriteFactory2::CreateGlyphRunAnalysis` | Grok Q6, E7; Astra Q6, E11 | D3.2, D3.3 |
| **The parity gate is split**: the tight band for geometry, a declared text band plus the advance audit for a platform rasterizer; an audit that fails lands the stage on swash with the tell open | Grok E8, missing Q2; Astra E11 | D9 |
| `clip-path` and SVG clips are a coverage clip carried in the instance (both painters implement them today; a layer per clip would be a new cost); the instance grows to 192 bytes | Grok Q7, E5 | D1.1, D2.4 |
| Coverage draws with a brush (solid, gradient, pattern), not only a tint: SVG paint has gradients and patterns | Astra E7 | D1.2, D2.3 |
| Kept rows pin the atlas entries they reference; eviction skips pinned entries; group boundaries (layers, opacity, masks) are recorded as markers and reopened on replay; device resources retire after their last submitted frame | Astra Q8, E8 | D4, D8 |
| The layer pool has no hard ceiling of two: it grows to the measured high-water mark, a compose target is budgeted (a swap-chain image cannot be sampled), and a frame that cannot allocate falls back to the CPU painter | Grok missing Q4; Astra E8 | D2.5, D8 |
| The boot claim is the defensible one: no application-side shader compilation on Metal and DirectX 11; what every driver still does at pipeline creation is measured and a replacement DEFERRED exception is proposed with the numbers, not a withdrawal | Astra E4, E5; Grok E16 | D6.3, §8, §5 Q14 |
| `fxc` is not on the Mac's cross-check: the build embeds DXBC where `fxc` exists and a stub that refuses at runtime where it does not | Grok missing Q8, E10 | D6.1 |
| Windows is not presented with damage on the frame path (only the caret's blink); the game DLL presents its full-window canvas itself and the numbers are its fast path | Grok E1, Astra E1 | §1 |
| D10 names the contract an imported image needs (identity, format and colour space, adapter identity, completion, ownership, release) and the interface change it implies; Microsoft does not support shared resources across D3D API versions on different adapters | Astra E9, missing Q4 | D10 |
| Device loss is specified apart from surface recreation; the CPU fallback presents through the same swap chain (DXGI forbids returning to GDI on an HWND after flip-model presentation); retry with backoff | Astra missing Q7 | D5 |
| The supported matrix is named: OS minima, the GPU floor and why, remote sessions, software-only, which Linux window systems | Astra missing Q3; Grok E10 | D5 |
| The painted manifest key is refused by the build until the carrier exposes the accessibility tree; S1's carrier is a benchmark carrier until the shipping gate; what "the same everywhere" promises is said | Grok Q9, missing Q3; Astra Q9, missing Q2, Q8 | D12 |
| `gpu.rs` loses platform branches as each passes, the shared file when its last consumer goes (S3 cannot delete what S4 uses); tvOS needs its focusable items exposed; cold start is measured by the platform's timers, latency by presented-frame timestamps | Astra E15 | D13, §3, §4 |
| Colour and blending are a contract, not API defaults | Astra missing Q6; Grok missing Q7 | D2.7 |
| The portable WGSL subset is named (ES 3.00 precision, no dynamic storage indexing); overrides are allowed for correctness as well as speed | Astra Q5; Grok Q5 | D6 |
| LLP 1091.001 is not the Windows build rule; `host/windows/build.mjs` is | Grok E10 | D6.1 |
| Separate commitments: the renderer, the Apple carrier, the web host and the compilation exception are each Charlie's own waiver | Astra missing Q11 | §5 Q24 |

### r2, from Muse's review

| Change | Raised as | Where |
|---|---|---|
| The Backend list in §1 is the trait's, `pointer` not "the caret"; the Windows numbers are LLP 1015.001's fast path and are named as game composition; LLP 1015's text is §3 and its parity check §8 | E1, E4, E5 | §1 |
| Every Backend method is mapped to an instance kind or a coverage mask: superellipse corners, arbitrary border regions, dashed and dotted borders go through the path atlas; image tint and island blend modes are instance flags; the backdrop ops include saturate | E2 | D1.2, D2.3, D2.5 |
| The instance layout is published, with its bytes; a replayed row is re-clipped under the clip in force | Q8 | D1.1, D4 |
| Windows presents through a DirectComposition swap chain, premultiplied (LLP 1115.004 D4.3); the whole frame is redrawn and presented with `FLIP_DISCARD`; dirty rectangles are a later measurement; the D3D11 device is created with BGRA support for Direct2D | E3, E7, Q3 | D7, D5 |
| The GPU module's ABI stands: no device or texture crosses it | E9 | D10 |
| Six API implementations were counted; r3 reduces the committed count (above) | E10 | D5 |
| swash first on every platform; a platform rasterizer is admitted by its audit; colour glyphs, synthetic bold and italic and font smoothing are named | Q6 | D3 |
| S0 gains a Windows present spike and a shader translation spike | Q5, Q10 | §3 |
| The parity gate names the pinned suites and the per-method inventory before deletion | Q11 | D9 |
| Colour, the fallback chain, `unsafe`, minima, screenshots from a presenting backend, resize and DPI, binaries in the bundle, accessibility as S1's gate, the boot-graph count, display-link pacing | M1 to M10 | D5, D7, D9, D11, D12, D14 |
| The manifest key is `host.macos.presenter` and `host.ios.presenter` | Q9 | D12 |

## 0. Summary

Exact's painted presenter draws an app the same way on every platform it paints on: the
kernel's tree is the display list, one walk emits it to a backend, and the backend draws
(LLP 1015 §2). What draws it on the GPU today is vello, a general compute-based vector
renderer, read back to the CPU on Linux and the Mac and presented from a swapchain only on
Android. Its measured costs (LLP 1148 F5) are the five metrics a performance-first customer
asks about; the panel is right that three of them (the readback, runtime shader compilation,
no kept rows) are the integration's, not the algorithm's, and S0 removes them from vello as
the control arm, so what remains is measured, not assumed.

This RFC proposes a renderer written for a user interface, **`exact-ink`**, and the
measurement that decides whether it replaces vello:

1. **What it draws** is what the painter's `Backend` trait already emits (D1): rounded
   quads with per-corner radii, borders, analytic shadows, gradients, images, glyphs from an
   atlas, coverage masks with a brush for everything that is a path, clips (two rounded and
   one coverage clip in the instance), opacity groups, masks, backdrop filters, blend modes,
   kept rows and animated layers. One instanced pipeline and a blur, no compute shader (D2).
   Every method of the trait maps to an instance or a coverage mask (D1.2).
2. **How it keeps work from happening** is the Backend contract's retained parts, which no
   GPU backend implements today: kept rows replayed as instance lists that pin what they
   reference (D4), animated layers kept apart from their rows, a frame drawn only when
   something changed (D7).
3. **What it runs on** is one core over a device trait (D5). A wgpu device layer runs it on
   every platform and is the control. DirectX 11 is written directly, because wgpu has no
   DX11 and Windows needs the composition swap chain and Direct2D on the renderer's device.
   Direct Metal and Vulkan are written only where S0 and S3 measure a gain beyond noise over
   wgpu on the same core (D15). That is Charlie's condition read exactly: repetition per
   platform where it gives better performance.
4. **Text looks like the platform's**, in order: swash everywhere first, then Core Text on
   Apple, DirectWrite on Windows and `Canvas.drawGlyphs` on Android, each opened from the
   shaper's own font bytes and admitted by an audit of its ink against the kernel's advances
   (D3; LLP 1115.004 D6.3). A failed audit lands the stage on swash with the tell open (D9).
5. **Shaders are compiled at build** from one WGSL source in a named portable subset, every
   output proven in S0, with a hand-written per-platform shader allowed where a measurement
   earns it (D6). The defensible claim is that no application-side shader compilation runs on
   Metal or DirectX 11; what every driver does at pipeline creation is measured and a
   replacement DEFERRED exception is proposed with the numbers (D6.3, §8, §5 Q14).
6. **The CPU painter stays** as the pixel oracle and the fallback on every platform; the gate
   is split into geometry (the tight band) and platform text (a declared band and the advance
   audit) (D9). vello stays in the Canvas 2D module (D13).
7. **S0 is a decision gate with thresholds declared now** (D15): the improved vello (kept rows,
   on the same readback path) against the ink core on wgpu against the ink core on Metal,
   and the Windows spike's numbers. If ink does not earn it, the improvements to vello that
   S0 built land and this RFC stops there.

Order of work (§3): S0 on the Mac in six chunks, with the Windows spike on the NucBox; then,
if earned, the painted Apple presentation (S1) and Windows (S2) as parallel lanes; Linux (S3);
Android (S4); the web when an app asks (LLP 1149.000).

## 1. What is there today

- **The painter** (`host/linux/src/paint.rs`): one preorder walk of the live tree emits to a
  `Backend` (`paint/backend.rs`). Its methods, which the renderer implements every one of:
  `name`, `needs_geometry_paint`, `page`, `begin`, `recycle_frame`, `damage`,
  `begin_damage`, `fill`, `fill_gradient`, `backdrop_filter`, `blurred_shadow`, `text_blur`,
  `fill_border`, `image`, `island_image`, `surface_image`, `canvas`, `split_window_canvas`,
  `recycle_window_layers`, `take_window_underlay`, `window_overlay_rows`, `native`, `text`,
  `svg_path`, `push_svg_clip`, `push_clip`, `push_css_clip`, `pop_clip`, `push_opacity`,
  `push_mask`, `pop_mask`, `pop_opacity`, `pointer`, `finish`, `finish_for` (with
  `FrameTarget::{Pixels, Window, Geometry}`), `rows`, `row_begin`, `row_end`, `row_culled`,
  `row_draw`, `row_free`, `group_begin`, `group_end`, `slot_begin`, `slot_end`, `slot_again`,
  `layer_recolored`, `layer_begin`, `layer_end`, `last_frame_ms`. The walk records every
  node's painted box, which the agent's `layout` and hit testing read. `canvas`,
  `surface_image` and `island_image` take CPU pixmaps; an imported GPU image is an interface
  change (D10).
- **Two backends.** `raster.rs`: tiny-skia on the CPU, the oracle and the fallback; it keeps
  no rows. `gpu.rs`: vello over wgpu; it keeps no rows either, encodes the whole scene each
  frame into a vello scene, renders to a texture and reads it back; on Android
  `gpu/present.rs` renders into the window's Vulkan swapchain from a present thread, one
  frame in flight. Both implement `push_css_clip` and `push_svg_clip`. The Android Canvas
  host (LLP 1076 §3.3) is the one backend that keeps rows, as a Kotlin `RenderNode` per row,
  and it draws glyphs with `Canvas.drawGlyphs` from the shaper's font files. Neither backend
  draws a `blurred_shadow`: the painter draws every shadow as a stack of up to 32 border
  fills (`paint/shadow.rs`).
- **Windows** (`host/windows`): the CPU painter into a `softbuffer` surface; the frame path
  calls `present()` whole, and `present_with_damage` is the caret's blink only. A game's
  full-window canvas presents from the GPU DLL's own device, composing the UI above and
  below it from premultiplied pixel layers the host hands it (`gpu/README.md`,
  `gpu_window_frame`). The game composition path measured, after LLP 1015.001's fast path:
  paint including the game's readback 7.22 ms p50 at 1280×720 and 10.83 ms at 1920×1080,
  upload and present under 1 ms, on a shared machine with the desktop locked. These are game
  composition numbers, not a UI paint baseline; S2 measures the UI itself.
- **Apple**: native views; the painter runs headless on macOS for tests and the seconds loop.
  The Canvas 2D module presents vello into IOSurfaces with kernels compiled to a Metal library
  at build (LLP 1056 §8.5): 0.7 to 2 ms warm to build pipeline states at launch, 103 to
  157 ms from process start to the first GPU canvas pixels at the landing tip (a canvas's
  first pixels, not a whole app's first frame).
- **Text** (LLP 1015 §3, LLP 1085.000): Parley lays out, fontique matches, swash rasterizes
  hinted grayscale glyphs into the painter's cache (`text/catalog.rs`); vello draws glyph
  outlines from the same layout.
- **Parity today** (LLP 1015 §8): one whole-app band, `the_two_painters_agree_within_a_band`
  (mean under 5/255, under 6% of pixels differing by more than 32 at 390×844), which exists
  because vello's outlines and swash's bitmaps differ; and the pinned suites that run every
  painter that can (`paint`, `borders`, `tint`, `backdrop`, `gradients`, `visual`,
  `materials`, `image`).
- **Measured costs** of the vello path are LLP 1148 F5's table. The 23 ms warm and 93 ms
  first-launch device figures are `Gpu::new`'s whole of instance, adapter and device, which
  a direct device also pays in the driver; whether any of it is wgpu's is unmeasured (D5).

## 2. Decisions

### D0 — Scope: one renderer for the painted presenter's frame, on every platform

`exact-ink` draws the painted presenter's frame on macOS, iOS, iPadOS and tvOS, Windows,
Linux, Android, and, for an app that opts in, the web (LLP 1149.000). It does not draw
watchOS (a snapshot driver, LLP 1126), a game's canvas (the app's GPU module, LLP 1009), or
the Apple native host's views (D12). It is the painter's backend, not a new painter: the walk,
the damage, the rows and the layers are LLP 1015's, and every pixel it draws is held to the
CPU painter's (D9).

### D1 — The display list is the Backend trait; the renderer's input is instances

The renderer implements `Backend`. Each call becomes one or more **instances** in a typed
buffer for the frame. Instance order is paint order; a batch is a run of instances with the
same texture bindings and scissor, drawn with one instanced draw of a four-vertex strip.
A frame is a few batches. There is no scene graph in the renderer: the kernel is the scene.

**D1.1 The instance, 192 bytes**, `#[repr(C)]`, 16-byte aligned, published so the budget is
checkable. An instance is a **geometry** and a **brush**:

| Field | Bytes | Holds |
|---|---|---|
| `rect` | 16 | the quad in local points: x, y, w, h |
| `xform` | 24 (+8 pad) | the 2×3 affine from local points to device pixels (motion's translate, rotate and scale about the pivot, then the device scale) |
| `radii` | 16 | the shape's four corner radii (circular; an elliptical corner, or any `corner-shape` that is not `round`, is a coverage geometry, D2.3) |
| `color` | 16 | premultiplied RGBA for a solid brush, or the tint; for a gradient, unused |
| `clip0`, `clip0_radii` | 32 | the innermost rounded clip in force, in device pixels, axis-aligned |
| `clip1`, `clip1_radii` | 32 | the next one |
| `mask_rect`, `mask_uv` | 32 | a coverage clip in force (`clip-path`, an SVG clip): its device rectangle and its place in the path atlas; zero when none |
| `uv` | 16 | the texture rectangle, normalized: the image, glyph, coverage mask or layer; for a shadow and a border, the inner or shape rectangle |
| `param` | 16 | by geometry and brush: sigma; a border ring's inner radii; a gradient's geometry; a pattern's tile; a layer's opacity and blend |
| `kind`, `brush`, `flags`, `pad` | 16 | the geometry (D1.2), the brush (solid, gradient, image or pattern, from `uv` into the image slot), and flags: tint, inset, even-odd, `object-fit` sampling, the blend formula |

Ten thousand instances are 1.9 MB; a frame's buffer is reused. The buffer is a vertex buffer
read with instance step mode (D6.1), twelve attributes of 16 bytes, not a storage buffer.
r1's sixty-four bytes did not hold the field list (Muse, Grok); this layout does.

**D1.2 Every Backend method, mapped.** The gate in D9 is unmeetable unless this table is
complete; it is the implementer's checklist, and S0 lands no row of it as "later".

| Backend method | Geometry and brush, or path |
|---|---|
| `fill` with round corners | `RoundedRect` × solid: a rounded-rectangle signed distance; the whole-viewport opaque fill is the clear |
| `fill` with elliptical radii or a `corner-shape` that is not `round` (squircle, bevel, scoop, notch, `-exact-continuous`, `kernel::corner::outline`) | `Coverage` × solid: the outline rasterized on the CPU into the path atlas |
| `fill_gradient` | `RoundedRect` (or `Coverage`) × gradient: the kernel's `Geometry` (linear, radial, conic; repeat) in `param`, the stops in a ramp texture row, interpolated in the painter's space (D2.7) |
| `fill_border` with `ring: Some` (one round-cornered ring) | `Ring` × solid: outer minus inner signed distance |
| `fill_border` otherwise (per-side regions, side clips, ring clips, dashed and dotted, non-round corners) | `Coverage` × solid: the region rasterized even-odd inside its clips |
| `blurred_shadow` (outer, and inset by the `inset` flag) | `Shadow`: the Gaussian-blurred rounded rectangle, analytically, as [Evan Wallace's shader](https://madebyevan.com/shaders/fast-rounded-rectangle-shadows/) and GPUI's; painted only outside `outer` by its signed distance; held to the painter's banded shadow by a fixture in S0 (the band is LLP 1064 D2's Gaussian, which the bands approximate); a non-round shape, or a miss on the fixture, falls back to the painter's bands. Returns true when it drew |
| `text_blur` | the glyph geometry's blur sigma, applied in the atlas raster (a numeric roll's blur, LLP 1077 D15) |
| `image` | `RoundedRect` × image: a texture, premultiplied, the `object-fit` rectangle already resolved, the content box as the instance rectangle, the outer shape as a clip slot; `tint` as the tint flag (RGB replaced through alpha) |
| `island_image` | the same, with the blend flag: multiply, screen, overlay and the rest of CSS `mix-blend-mode`, as a blend formula over a copy of the destination region (D2.5) |
| `surface_image`, `canvas` | `RoundedRect` × image from a texture uploaded from the pixmap, keyed by content revision (LLP 1056 D7) |
| `text` | `Glyph` instances from the atlas (D3); colour glyphs from the colour atlas |
| `svg_path` (fill, stroke, dashes, caps, joins, markers; `paint-order`) | `Coverage` × the paint's brush: solid, an SVG gradient (the ramp texture), or a pattern (an image slot, repeated by `param`); a fill and a stroke are two masks |
| `push_clip` (rectangular) | the scissor |
| `push_clip` (rounded, axis-aligned in device space) | `clip0` or `clip1` of every instance until the pop |
| `push_css_clip`, `push_svg_clip` | the mask slot: the clip's union rasterized once into the path atlas (cached by path, transform and scale) and sampled by every instance until the pop |
| a third rounded clip, a second coverage clip, or a clip under a rotation or skew | a layer (D2.5) composited through a coverage mask |
| `push_opacity`/`pop_opacity` | a layer, composited at the opacity |
| `push_mask`/`pop_mask` | a layer, composited through the mask: a colour's alpha or a gradient sampled with the gradient code |
| `backdrop_filter` | the region under the shape copied from the frame's compose target, the ordered `BackdropOp`s applied (blur as three box passes over encoded sRGB, as LLP 1053.000 D2 and `raster/backdrop.rs` do; saturate, brightness, contrast and the rest in one colour pass), drawn back under the clip |
| `split_window_canvas`, `recycle_window_layers`, `take_window_underlay`, `window_overlay_rows` | D10: unchanged in contract; the UI below and above a full-window game is read back as premultiplied pixels for those rows |
| `native` | the platform element's box is left clear for the native view layered above (D10) |
| `pointer` | `RoundedRect` × image: the arrow, as the CPU painter draws it, where a host has no system cursor (DRM) |
| `rows`, `row_*`, `group_*`, `slot_*` | D4 |
| `layer_begin`/`layer_end`/`layer_recolored` | D4: a kept instance range drawn with the layer's transform and opacity; a recolored layer is recorded again each frame it changes colour |
| `damage`/`begin_damage` | false: the whole frame is redrawn (D7); the painter's damage set still drives whether a frame is drawn at all |
| `page`, `begin`, `finish`, `finish_for`, `recycle_frame`, `last_frame_ms`, `name`, `needs_geometry_paint` | the frame's clear colour, the frame, the readback (D9, D11) and the report |

### D2 — Pipelines, clips, paths and layers

**D2.1 No compute.** Every stage is a vertex and fragment pipeline over instanced
rectangles: one pipeline for every geometry and brush (the instance selects the fragment
path), one for the box blur, one for the colour pass. "No compute" does not itself set a GPU
floor (Astra); the floor is D5's, chosen for reach.

**D2.2 Antialiasing** is analytic: a signed distance to the rounded rectangle (or ring)
evaluated per fragment, with coverage from the distance's screen-space derivative. No
multisampling, no coverage buffers. Text's antialiasing is the rasterizer's (D3); a coverage
mask's is tiny-skia's.

**D2.3 Paths** are everything D1.2 sends to the **path atlas**: rasterized on the CPU by
tiny-skia's rasterizer, which the oracle already uses, into an alpha atlas, cached by path
identity, transform and scale; drawn as `Coverage` geometry with a brush. The atlas bounds
retained storage, not CPU work: an animated path rasterizes again each frame it changes, a
path larger than an atlas page is tiled across entries or given a texture of its own for the
frame, and a working set past the ceiling spills to a per-frame overflow texture that is not
retained (D8). S0 measures the miss rate, the re-raster milliseconds and the uploaded bytes
on Shelf, Excord and Reflow, where dashed borders and sparklines are ordinary. GPU path
rendering (sparse strips, LLP 1148 F5, whose documentation lists unsupported mask, filter
and blend cases) is deferred until a measured consumer needs it: an app whose frames are
mostly animated paths.

**D2.4 Clips.** Axis-aligned rectangular clips are the scissor. Two rounded clips and one
coverage clip travel in the instance (D1.1), which covers a rounded container inside a
scroller holding a rounded image, and a `clip-path` on any of them. A third rounded clip, a
second coverage clip, or a transformed clip makes a layer (D2.5) composited through a
coverage mask. The walk's clip stack is unchanged; S0 counts how often a screen of Excord and
Shelf reaches a layer this way.

**D2.5 Layers.** An opacity group below 1, a mask, a backdrop filter, a blend mode, and the
clips D2.4 sends here draw into an offscreen texture from a pool sized to the viewport, then
composite as a `Layer` instance. A frame that contains a backdrop filter or a blend mode
draws into a compose target and blits to the swap chain at the end, since a swap-chain image
cannot be sampled; the region under a node is copied from that target. The pool has no hard
ceiling: it grows to the high-water mark the apps show and is trimmed on memory warning and
background; a frame whose layers cannot be allocated is drawn by the CPU painter instead,
so correctness survives pressure (D8).

**D2.6 Images** are textures cached by bitmap identity (as `gpu/images.rs` keys them) with
a mip chain for an image drawn smaller than its pixels, so a thumbnail of a photograph is
area-filtered as the CPU painter's `image_area` filters it, within the band.

**D2.7 Colour and blending, as a contract.** Targets are 8-bit sRGB-encoded, premultiplied
alpha throughout; blending and gradient interpolation happen in the encoded space, as the
CPU painter and the web do; coverage multiplies alpha without gamma adjustment, as
tiny-skia's does; images are uploaded premultiplied as the painter decoded them; the swap
chain is tagged sRGB on every platform so a wide-gamut display composites it as sRGB
(`CAMetalLayer.colorspace`, DXGI's sRGB formats), measured once on a P3 display against the
native host. The deviation from linear-light blending is declared (§5 Q15), not fixed.

### D3 — Text: swash first, then the platform's rasterizer, by audit

Shaping and layout stay the kernel's and the painter's (Parley over the painter's text
cache, LLP 1085.000). The renderer owns rasterization into a **glyph atlas** (alpha, with a
colour atlas for colour glyphs), keyed by face, glyph, size, variation coordinates, synthetic
slant and one of four horizontal subpixel phases, packed on shelves, evicted by frame age
except where pinned (D4).

**D3.1 swash first, everywhere.** S0 rasterizes with swash, hinted, grayscale, exactly as
`text/catalog.rs` does for the CPU painter, so the oracle and the renderer part only at the
atlas's placement. Every stage presents with swash until its platform's rasterizer is
admitted.

**D3.2 A platform rasterizer is admitted by its audit, apart from any stage's gate:**

| Platform | Rasterizer | Why |
|---|---|---|
| Apple | Core Text: `CTFontDrawGlyphs` into a `CGBitmapContext`, font smoothing as the system has it, the font made from the shaper's bytes (`CGDataProvider` → `CGFont` → `CTFontCreateWithGraphicsFont`) so glyph ids and variation axes are swash's | the platform's own text; a UIKit label and an Exact label on one screen must match |
| Windows | DirectWrite: `IDWriteFactory2::CreateGlyphRunAnalysis` with `DWRITE_TEXT_ANTIALIAS_MODE_GRAYSCALE` by default (the older factory method has no antialias mode and its sample uses ClearType), natural measuring mode, no grid fit, the face from an in-memory font file loader over the shaper's bytes; ClearType where the ground is opaque and the person's settings ask for it (LLP 1115.004 D6.3's measurement decides) | the one tell a Windows user notices; the ClearType Text Tuner settings count |
| Android | `Canvas.drawGlyphs` (API 31+) through the Canvas host's JNI into a bitmap, from the same font file the shaper opened, as `Player.kt` draws today; swash below API 31 | Android's text is Skia over FreeType with the platform's hinting; the Canvas host already matches it |
| Linux | swash stays | there is no one platform rasterizer to match |
| Web | the browser's Canvas 2D (LLP 1149.000 D5) | the browser's rasterizer and the system's fonts |

The rule LLP 1115.004 D6.3 set holds on every platform: a platform rasterizer lands only
after an audit of its ink against the kernel's advances shows no drift beyond the oracle's
tolerance on the parity fixtures. Advances never come from the rasterizer. An audit that
fails on the system font at 100%, 125% and 150% keeps swash, keeps the tell open, and does
not stop the stage (D9).

**D3.3 What each rasterizer must also answer**, named so the audit covers them: colour
glyphs (`COLR`, `sbix`, `CBDT`: swash reads all three; Core Text, DirectWrite and Android
each have their own path, and the colour atlas takes whichever drew), synthetic bold and
oblique (the kernel's `Synthesis`; swash skews, the others take a transform), and font
smoothing (the person's setting on each platform is read by the platform's rasterizer and
never by swash); and the audit runs during motion too (fractional positions, scale changes,
a monitor change), not only on a static crop.

### D4 — Kept rows and layers are instance lists, not pixels

`rows()` is true. A kept row is the instance range it recorded, relative to its origin, with
the clips pushed inside the row recorded in row space and the clips in force outside it
(the scroller's) left out; the group boundaries inside it (a layer, an opacity group, a
mask) are recorded as markers so replay reopens them. `row_draw` copies that range into the
frame's buffer with the row's offset applied to `xform`, the inner clips and the markers,
then re-clips every instance under the clips in force now: the scissor intersected, the
outer rounded clips filling the free clip slots, a row that would need a third slot drawn
through a layer. Ten thousand instances of 192 bytes are 1.9 MB, a copy. Scrolling a list of
unchanged rows walks none of them (`paint/rows.rs`) and encodes none of them: the frame is a
copy, a re-clip and a draw. If the copy shows in S0's measurement, instances are kept in
row-local coordinates and the offset becomes a per-draw constant (Grok). Animated layers
(`layer_begin`) are the same, with the layer's transform and opacity applied at draw. Images
arriving in a kept row replace the slot's instances (`slot_again`).

**What a kept row references stays valid**: a glyph, path or ramp entry a kept row's
instances point at is pinned in its atlas while the row is kept (`row_free` unpins);
eviction skips pinned entries; an atlas full of pinned entries spills new entries to the
per-frame overflow texture (D2.3) rather than evicting. Device resources (buffers, layer
textures, images) retire only after the last frame that used them has completed on the GPU
(D11), by frame index, never on drop.

Not chosen: rows cached as textures. A row at 400×100 points on a 3× display is 1.44 MB;
twenty on screen are 29 MB (the Android Canvas host's `RenderNode` per row caches pixels in
HWUI, which is that memory), and a scaled or rotated row would resample. Pixels are cached
only where re-encoding is expensive and the pixels are the point: layers (D2.5).

### D5 — One core; the device layer per platform decided by measurement

The renderer's device is a trait of about fifteen operations: create and resize a swap
chain, allocate and upload a buffer, allocate and upload a texture (alpha, colour,
mipmapped), bind a pipeline from a built binary, one render pass with a clear and a scissor,
one instanced draw, an offscreen target from the pool, a region copy for blur and blend,
import an external image, present, read back.

**D5.1 wgpu is the portable device layer and the control.** It implements the trait once for
Metal, DirectX 12, Vulkan, OpenGL ES and WebGPU, and it is what `gpu/present.rs` already
presents through on Android. S0 builds it first (§3 C1): the core runs on every platform
from the first chunk, and every direct layer is measured against it.

**D5.2 A direct device layer is written where it is required or measured.**

| Platform | Device layer | Status | Why |
|---|---|---|---|
| Windows | DirectX 11 (`windows-rs`), feature level 11_0, created with `D3D11_CREATE_DEVICE_BGRA_SUPPORT`; a DirectComposition swap chain (D7) | **built** (S0 C6, §4: hardware device 48 ms, WARP 14 ms, pipelines 2.7 ms from DXBC; wgpu's DirectX 12 took 139 and 457 ms on the same machine), then S2 | wgpu's Windows backend is DirectX 12; the composition swap chain over Mica (LLP 1115.004 D4.3) and Direct2D drawing the windowless RichEdit on the renderer's device (D7 there) want the renderer's own D3D11 device; WARP runs feature level 11_0 in virtual machines and over Remote Desktop (DirectX 12 has WARP too, Astra: the floor is for the interop and reach together) |
| macOS, iOS, iPadOS, tvOS | Metal (`objc2-metal`), `CAMetalLayer` | **kept** (S0 C5, §4): 21 to 30% on frame time on two workloads of three, 15 to 47% on CPU per frame, lower memory with no spikes; its boot regression (device creation 30 ms against 13; a 127 ms pipeline build with an empty application cache, 0.5 ms with the binary archive retained, the OS cache warm in both) carries a dated obligation (D15) | the IOSurface import and the presentation options are reachable through wgpu's Metal backend too; the direct layer earned its place on frame time, CPU and memory, not on boot |
| Linux | Vulkan (`ash`): `VK_KHR_display` with `VK_KHR_display_swapchain` and `VK_EXT_acquire_drm_display` on KMS (LLP 1015 §7), a headless target on lavapipe | **measured** in S3 the same way | wgpu's Vulkan backend presents on KMS already |
| Android | wgpu's Vulkan, through `gpu/present.rs`'s presenter path | **kept**: Android 10 requires Vulkan 1.1 on 64-bit devices (`host/android/README.md` permits Android 10), current Pixels run OpenGL ES through ANGLE over Vulkan, and the Canvas module's `AHardwareBuffer` import is Vulkan's | OpenGL ES only for a device that cannot open Vulkan, which S4's A/B on a low-end Mali or Adreno device must first show exists |
| Web | wgpu's WebGPU; WebGL2 if a browser without WebGPU must be served | with the web (LLP 1149.000) | — |

**LLP 1009 D1 is amended narrowly**: wgpu is the one GPU API for an *app's* GPU code (its
canvases, its games); the host's renderer may own a device layer on a platform where this
document measures that it earns it, and the GPU module and the renderer share no device
(D10).

**D5.3 The supported matrix**, named rather than "every machine":

| | Minimum | Software-only | Remote sessions |
|---|---|---|---|
| macOS, iOS, tvOS | macOS 14, iOS and tvOS 17 (the Canvas module's minima; the manifests' `minimumOS`) | none on Apple: the CPU painter | — |
| Windows | Windows 11 build 22621 on x64 (LLP 1115.004 D0.3), feature level 11_0 | WARP, then the CPU painter | WARP over Remote Desktop |
| Linux | Vulkan 1.0 on KMS (the DRM run); no X11 or Wayland window system in this document | lavapipe, then the CPU painter | — |
| Android | Android 10 (`host/android/README.md`), Vulkan 1.1 | the CPU painter | — |

**D5.4 Failure.** The chain per platform is chosen once at boot and reported in `state`: the
hardware device, else the platform's software device, else the CPU painter. **Surface
recreation** (resize, DPI change, a monitor change, Android's surface loss on background)
keeps the device and remakes the swap chain. **Device loss** (a driver reset, a remote
session's GPU going away) invalidates every device resource by generation, keeps the
renderer's logical state (atlases' keys, kept rows' instances), draws the frames in between
on the CPU painter presented through the same swap chain where the platform forbids a return
to the window's own surface (DXGI does not allow GDI on an HWND after flip-model
presentation), and retries the device with backoff, never every frame.

**`unsafe`** lives in `exact-ink`, which the FFI needs and whose device files are audited;
`exact-linux` and the carriers stay `#![deny(unsafe_code)]`.

**The cost named**: a wgpu layer plus DirectX 11 for certain, Metal and Vulkan only where
measured; around a thousand lines each (Vulkan split across files under the cap). The new
dependencies (`windows` features; `objc2-metal` and `ash` if measured in) are counted by the
boot check per platform.

### D6 — Shaders: one source, compiled at build, overridden where measured

**D6.1** The shaders are one WGSL source in a **portable subset** named here: `mediump`-safe
arithmetic in the fragment path (the GLSL ES 3.00 output declares `highp` for positions and
distances and `mediump` elsewhere, and screen-space derivatives are used only on `highp`
values); **no storage buffers**: GLSL ES 3.00 and WebGL2 have none (naga refuses them for
that output; found in S0 C1), so instances are **per-instance vertex attributes** (twelve
16-byte attributes, instance step mode), which Metal, DirectX 11, Vulkan, OpenGL ES 3.0 and
WebGL2 all have and which is how GPUI feeds its quads; one uniform (the viewport); no
`textureSampleLevel` on a non-mipmapped target. The
build translates it with naga to MSL and compiles a `.metallib` with `xcrun metal` (as
`canvas/vello/build.rs` does); to HLSL shader model 5.0 and compiles DXBC with `fxc` where
`fxc` exists (the Windows host is built on Windows with the MSVC target,
`host/windows/build.mjs`); to SPIR-V; to GLSL ES 3.00; and keeps WGSL for WebGPU. The Mac's
tier-2 cross-check of `exact-windows` has no `fxc`: there the build embeds no DXBC and the
DirectX 11 device refuses at runtime by name, so the check compiles and nothing pretends to
run. `dxc` emits DXIL, which DirectX 11 cannot load. S0's translation spike compiles every
output and holds the SDF, gradient and blur shaders to the oracle's band on each.

**D6.2** A platform may carry a hand-written shader in place of the translated one, in its
own language, when a measurement shows the translated one costs frame time or quality, or
when a compiler is wrong (naga's GLSL ES output on derivatives is where to expect it). The
override is a file beside the generated one; the build prefers it; it replaces the generated
file in that platform's parity set, never adding a second variant to keep.

**D6.3 What is still built at launch, said exactly.** The claim this document can defend is:
**no application-side shader compilation runs on Metal or DirectX 11**; what the driver does
from a compiled shader is still compilation in the rule's sense (the panel on r4, Astra 94:
Apple says Metal IR requires runtime compilation to the GPU's binary; Microsoft's D3D11
shader-creation callback converts shader code to the hardware's format), and is admitted
only by a measured, owned exception per platform (§8), with the condition both families set: **a cache miss never blocks first pixel**. The
pipelines are created asynchronously; when the archive misses, the first frames are the CPU
painter's through D5.4's fallback (presented through the same swap chain where the platform
forbids a return to the window's surface) while the pipelines build and the archive is
populated and serialized off the path (LLP 1009 D5's precedent, applied to the presenter
itself). A packaged `MTLBinaryArchive` with `failOnBinaryArchiveMiss` is an accelerator, not
the mechanism: Apple's archives are device-built and specific to a device and OS, so a
bundled one misses on new hardware and after OS updates, and the miss path must exist
anyway. The per-launch cost (0.5 ms Metal, 2.7 ms DirectX 11 warm) is a named phase inside
the 100 ms budget; the once-per-binary cost is the exception's, written per platform. Metal's
`.metallib` is AIR, not the GPU's binary: pipeline states are built from it at launch (0.7
to 2 ms warm in the Canvas module; a cold driver is measured in S0). DirectX 11 drivers
compile DXBC to the GPU's ISA at shader creation. Vulkan builds pipelines from SPIR-V behind
a `VkPipelineCache` on disk; OpenGL ES links programs from GLSL, and Android's
`EGL_ANDROID_blob_cache` is installed by the platform's EGL where the device's
implementation supports it, with no retention guarantee, so the renderer keeps
`glProgramBinary` itself and the first launch is budgeted as uncached; the browser compiles
WGSL or GLSL. `rules/RULES.md` says the boot path "executes and compiles nothing", counted as
a module graph; `rules/DEFERRED.md` §Runtime's exception is for the vello painter's shader
compilation on the first launch. Whether a driver's pipeline build from a compiled binary is
inside that rule is Charlie's (§5 Q14); the proposal is a **replacement exception**, written
in §8 with S0's and S3's measured numbers per platform, not a withdrawal that leaves the
behaviour the exception was written for.

### D7 — Presentation and pacing

A frame is drawn when the painter has damage or the reader has motion, never otherwise; an
idle app submits nothing. **The whole frame is redrawn**: a redraw is a few batches, and
partial presentation has its own costs (Windows's dirty rectangles need `FLIP_SEQUENTIAL`
and a redraw confined to the rectangles; `VK_KHR_incremental_present` and
`eglSwapBuffersWithDamageKHR` are hints honoured per driver; the tile-GPU win is
`EGL_KHR_partial_update`, which a full redraw cannot use; a KMS page flip scans out the whole
buffer regardless). Partial presentation is a measurement after S2 and S4, taken only if it
saves power on a screen that changes a little.

| Platform | Swap chain and pacing |
|---|---|
| Apple | `CAMetalLayer`, three drawables, `nextDrawable` off the main thread, paced by `CADisplayLink` (`CVDisplayLink` on the Mac), which runs at the display's rate on ProMotion and variable-refresh displays; during a live resize `presentsWithTransaction` with its sequence (encode, commit, `waitUntilScheduled`, present inside the transaction); the device layer remakes the drawable size on resize and scale change |
| Windows | a DirectComposition swap chain (`CreateSwapChainForComposition`, premultiplied alpha, `DXGI_SWAP_EFFECT_FLIP_DISCARD`, two buffers, a frame-latency waitable object), as LLP 1115.004 D4.3 requires so the frame composes over Mica; never a layered window; the device layer resizes the buffers on `WM_SIZE` and DPI change and follows the monitor's refresh rate as the window does today |
| Linux | Vulkan FIFO on KMS; mailbox where the display supports it; the device layer owns mode changes |
| Android | `gpu/present.rs`'s presenter: `Choreographer` frames; surface loss on background and rotation remakes the swap chain, the device kept |

Motion stays on the runner's clock; the display link paces presentation, so a spring on a
120 Hz display is sampled at 120 Hz and on a 60 Hz one at 60. The target is one frame of
latency from the walk to the glass, **measured** by presented-frame timestamps (Core
Animation's, DXGI's frame statistics, the Android frame timeline), not inferred from the
one-deep handoff.

### D8 — Memory budgets, declared

| Resource | Start | Ceiling | Under pressure |
|---|---|---|---|
| Glyph atlas (alpha) | 512² | 2048² (4 MB) | evict by frame age, skipping entries kept rows pin (D4); past the ceiling, new entries spill to the per-frame overflow texture |
| Colour atlas | 256² | 1024² | the same |
| Path atlas | 512² | 2048² | the same; a path larger than a page is tiled or given a texture for the frame (D2.3) |
| Ramp texture | 256 × 64 rows | 256 × 1024 | rows evicted by frame age, pinned by kept rows |
| Layer pool, and the compose target | none | the apps' measured high-water mark (S0) | freed on memory warning and background (`trim`, as the Canvas module has it); a frame that cannot allocate is drawn by the CPU painter |
| Instance buffers | one frame's | the largest frame seen, three in flight | shrunk on `trim`; CPU copies counted beside GPU bytes |
| Image textures | by use | the painter's image budget (LLP 1011) | the painter's eviction, after the last frame that used them completes |

The measure (§4): a hello app's footprint on each platform against the CPU painter's, with
the swap chain as the accepted difference; and, from S0, the atlas miss rates, the overflow
texture's use and the layer pool's high-water mark on Shelf and Excord against these
ceilings, during a fling with eviction forced.

### D9 — The CPU painter is the oracle; the gate is split; parity gates each platform

tiny-skia stays the deterministic pixel oracle (LLP 1015 §8) and the fallback where no
device can be made. **The gate has two parts**, because D3 asks the renderer to draw text
the oracle does not draw:

- **Geometry and compositing**: the pinned suites that run every painter (`paint`, `borders`,
  `tint`, `backdrop`, `gradients`, `visual`, `materials`, `image`) pass with the renderer as
  a painter choice (`EXACT_PAINTER=ink`), with swash glyphs, at today's thresholds; and the
  whole-app band holds.
- **Platform text**: a rasterizer D3.2 admits is held to a **declared text band** (as LLP
  1015 §8 declares one between vello's outlines and swash's bitmaps) plus the advance audit
  on the parity fixtures and during motion (D3.3); it is never held to the swash oracle's
  pixels, which it is meant to differ from. A failed audit keeps swash and the stage lands.

Before vello's backend is deleted on a platform, an inventory of D1.2's rows shows a fixture
for each (per-side borders, corner shapes, masks, backdrop saturate, blend modes, SVG
gradients and patterns, SVG clips are the ones without one today, added as stage work), and
the platform's qualification covers what pixels do not: pacing (D7's timestamps), device
loss (D5.4, forced), native composition (D10), memory (D8) and, where the carrier ships,
accessibility (D12). The suites run on that platform: the Mac, the NucBox, a builder under
lavapipe, the Android emulator, headless Chrome, in the async lane.

**Screenshots from a presenting backend**: `finish_for(Pixels)` reads the presented frame
back on demand (a blit to a shared buffer and a wait), so the agent's `screenshot`, the
smoke and delivery's captures work on every carrier; `finish_for(Window)` never reads back.

### D10 — Composition with native content and with GPU modules

The GPU module's ABI stands as `gpu/README.md` states it: no GPU device or texture object
crosses the DLL boundary, and a full-window canvas presents from its module-owned device,
composing premultiplied UI layers below and above it. The Backend's `canvas`,
`surface_image` and `island_image` take CPU pixmaps; an imported GPU image is a new method
and a new resource path, added when the first producer exists. Under the renderer:

| Content | v1 | Later, one producer-consumer pair qualified before any generalization |
|---|---|---|
| A full-window game canvas (LLP 1015.000) | unchanged: the module presents; the UI below (a solid underlay where it is one) and above reach it as premultiplied pixels, which the renderer reads back for the overlay rows only (`window_overlay_rows` bounds them) | a shared image with the contract below: IOSurface on Apple (the Canvas module's, proven); a DXGI shared NT handle with a keyed mutex from the module's D3D12 device into the renderer's D3D11 device on the **same adapter** (Microsoft supports no cross-adapter sharing across API versions); Vulkan external memory on one physical device; `AHardwareBuffer` with an Android native fence |
| A Canvas 2D surface (LLP 1056) on Apple | the module's IOSurface imported as a texture, as the native host shows it as a layer's contents today | — |
| A Canvas 2D surface elsewhere | tiny-skia's pixmap uploaded by content revision (LLP 1056 D7), as today | the vello module's texture, when LLP 1056 §8.5's Linux note is taken |
| The windowless RichEdit (LLP 1115.004 D7) | Direct2D on the same D3D11 device draws it into a texture the renderer draws as an image | — |
| Native views above the frame (menus, pickers, the IME's candidates) | views layered over the Metal layer; child windows and the `topmost` composition target D7.0 names; views above the surface on Android | — |

**The contract an imported image needs**, which the ABI change carries: identity (a stable
id and a content revision), format and colour space, the adapter identity it was allocated
on, the producer's completion (a fence or a keyed mutex the consumer waits on), ownership
and who may reuse the memory, and release (the consumer's completion before the producer
reclaims it). The first pair qualified is the Canvas module's IOSurface on Apple.

### D11 — Threads

The walk and the encode run on the host thread; submit and present on a render thread, one
frame in flight, as `gpu/present.rs` does on Android: the frame's instance buffers and
atlas uploads are handed over, the next walk starts while the GPU draws this one, and the
buffers come back for reuse. Nothing of the kernel's crosses the handoff. A readback for a
screenshot (D9) waits for the frame in flight. A resource retires only after the frame that
last used it has completed (D4).

### D12 — Where it is the presenter, and what "the same everywhere" promises

**The renderer is an app's choice, on every platform** (Charlie, 2026-10-28: "we want to
use it some of the time but not all of the time, depending on what you want"). On Linux,
Windows and Android, where the painted presenter is already the host, an app selects it in
its manifest, per platform as the manifest is shaped: `host.windows.painter: "ink"`,
`host.linux.painter: "ink"`, `host.android.painter: "ink"`; absent, the platform keeps the
painter it has today (the CPU painter on Windows, vello or the CPU painter on Linux, the
Canvas host or vello on Android). `EXACT_PAINTER=ink|gpu|cpu` overrides it for the agent,
the smoke and tests, as it does today. Nothing shipped changes when the renderer lands.
**On Windows, ink's device defaults to wgpu with the `softbuffer` present at the first
landing**; the DirectX 11 composition path of S0 C6 stays behind `EXACT_INK_DEVICE=d3d11`
until S2 proves it on a desktop (the second blind code review, Sol and Muse, both asked for
this: resize and occlusion on the composition swap chain, caret blink and the RichEdit spike
were not product-ready at `e83558a4d6`; `llp/reviews/` records nothing for code reviews, the
lane's `target/review-*.md` holds them, and C6f's commit names each finding's disposition). On Apple the native host stays the default — what an
author leaves unsaid is the platform's (LLP 1115) — and an app chooses the painted presenter
in its manifest, per platform as the manifest is shaped: `host.macos.presenter: "painted"`,
`host.ios.presenter: "painted"` (`scripts/app.schema.json`; the native host is the absent
value). **The build refuses the key** until the painted carrier exposes the accessibility
tree through the kernel's roles, as the native host does (VoiceOver reads it), with text
editing, the IME, keyboard navigation, focus and the app lifecycle: those are the shipping
gate of a presenter an app may choose. S1's carrier is a **benchmark carrier** until that
gate: a window or view over a `CAMetalLayer`, the pointer, the keyboard and the IME through
the system's text input protocols on a hidden view (the shape LLP 1115.004 D7 takes on
Windows), the agent over the same socket the native host uses; its menus, dialogs and
pickers the system's, as 1115.004 takes them on Windows.

What "the same everywhere" promises an app that opts in: the same authored styling and the
same layout, verified against the web (LLP 1001); the platform's text rasterization (D3)
and the platform's accessibility, editing and system surfaces, which differ by design. Not
identical pixels.

### D13 — vello's place after

The Canvas 2D GPU module keeps vello: a canvas may receive any drawing, and a compute vector
renderer with blur and clip-extent operators is the right tool there (LLP 1056 §8.5).
`gpu.rs` gained kept rows in S0 as the control arm (§3 C4), which land with the rest. **The
painter backends are not deleted** (amended by Charlie's ruling above): ink is the third
painter beside vello and the CPU painter, chosen per app (D12), for as long as Charlie wants
both available; the deletions this decision proposed in r3 (each platform's vello branch as
it passed D9, the shared file last, the second wgpu build with it) wait for his word.
`vendor/vello` stays for the module.

### D14 — Code

`host/ink`, crate `exact-ink`, no dependency on `exact-linux`: `instance.rs` (the instance
layout and batching), `atlas.rs` (shelf packing, pinning, eviction, the overflow texture),
`glyph/` (the rasterizers, one file per platform, swash first), `path.rs` (coverage through
tiny-skia), `shadow.rs`, `layer.rs` (the pool and the compose target), `device.rs` (the
trait), `device/wgpu.rs`, `device/d3d11.rs`, and `device/metal.rs` and `device/vulkan/` only
where measured in; `present.rs` (the render thread), `shaders/ink.wgsl` and `build.rs` (D6).
The `Backend` implementation lives in `exact-linux` as `ink.rs`, where the painter's types
are, as `gpu.rs` does today; it is the adapter, and the renderer is the two together under
one name. `exact-windows` and the painted Apple carrier take the presenter from `exact-linux`
as `exact-windows` does now; what the web would take without `exact-linux`'s font discovery
and executors is LLP 1149.000's. Shader binaries ship inside the platform's bundle (LLP
1030), and a pipeline cache on disk is keyed by the driver and the binary's hash. Every file
under 1,500 lines. Tests: instance packing, batching, re-clipping, pinning and eviction as
unit tests; parity against the oracle through the painter's pinned suites with
`EXACT_PAINTER=ink`; the device layers under the async lane on their platforms.

### D15 — The decision gate: what earns the replacement, declared before S0

S0 produces four arms on the same readback path on the Mac, and the Windows spike's numbers:

| Arm | What it isolates |
|---|---|
| A. vello as today | the baseline |
| B. vello with kept rows (scene fragments replayed under a transform) | whether retention alone is the gain |
| C. the ink core on the wgpu device layer | whether the UI-shaped primitives and the retained instance lists are the gain |
| D. the ink core on a direct Metal device layer | whether owning the device layer adds anything |

Measured per §4 on the heavy-list screen and on Shelf and Excord: cold start by phase, frame
time and encode time during a fling, CPU per second during the fling and at rest, footprint
peak and at rest, the atlas and layer counters.

**Thresholds** (proposed to Charlie, §5 Q21):

- **Ink is earned** (C over B) if it is at least 20% better on fling CPU per second or on
  frame time at 1920×1080, with no metric more than 10% worse and no more late frames. Then
  S1 and S2 follow. Otherwise B lands (it is an improvement to the shipping painter whatever
  happens), ink stays a prototype, and this RFC's stages past S0 are not scheduled.
- **A direct device layer is earned** (D over C, and the same test for Vulkan in S3) if it is
  at least 10% better on cold start by phase or on frame time, beyond run-to-run noise
  (interleaved runs, the spread reported). Otherwise the wgpu layer stays on that platform
  and the direct code is deleted, not kept.
- **DirectX 11 is built regardless** (D5.2); its measurement in S2 is against today's CPU
  path, Electron and WinUI 3, not against a wgpu arm that cannot do what it is for.
- **The replacement exception** (§8) is proposed with the measured launch numbers, and the
  cold-start budget (100 ms p50) is argued phase by phase (§4).
- **Amended in r4 from the panel (Astra, 91):** "no metric more than 10% worse" reads on
  medians, with a separate guard on the tail: a candidate-attributable peak regression above
  10% that repeats is a failure until repaired or explicitly traded; a five-run maximum
  alone is not a stable tail. **Boot has its own requirement:** the complete, interactive
  first screen at p50 within the 100 ms budget, measured with an empty application cache and
  with a retained one, and no more than 10% regression against the platform's existing
  painter; where the baseline already exceeds the budget, further regression is an explicit
  trade. A direct device layer that wins frame time, CPU and memory but loses boot is kept
  with a **dated obligation** on boot, not indefinitely: direct Metal's is 2026-11-04 or
  before S1's carrier is enabled, whichever is first; unmet and unrenegotiated, the layer is
  deleted. **The direct-layer clause, restated** (Muse, 85 that it was under-specified): at
  least 10% better on frame time beyond the spread on at least two of the three named
  workloads (the heavy list, which is Messages Stress; Shelf; Excord), none worse beyond the
  spread, CPU per frame or memory allowed as the substitute metric; and a **boot gate in
  milliseconds, not percent**: the warm device-plus-pipeline phase within 5 ms of the wgpu
  arm's, the complete first screen at p50 within the 100 ms budget with the phases summed,
  the first-launch cost handled by D6.3. A median-of-peaks guard for memory, with a
  worst-run breach triggering localization, never an automatic failure (a five-run maximum
  grows with the rounds). The ink-over-vello gate is complete only when presented late
  frames are measured on a carrier (none appear in any S0 result; D15 asked for them) and the
  readback-copy memory fix (`QUEUE.md`) is rerun against B on the wgpu arm, which is what
  ships on Android and perhaps Linux, so that obligation follows the wgpu layer into S3 and
  S4 and is not discharged by direct Metal on Apple.

## 3. Order of work

Each chunk and stage lands with its numbers in §4's table. S0 is one lane in
`exact2-wt-1126-gate` on `lane/ink` (the Mac) with the spike on the NucBox; each chunk is
one Astra run and one commit.

| Chunk | What | Gate |
|---|---|---|
| **S0 C1** The crate, the shader, the wgpu device, one quad read back (from 2026-10-28) | `exact-ink`'s instances, batching, atlases, the WGSL in its subset, `build.rs` translating every output (the translation spike's compile half; the Metal library built where the toolchain is), `device/wgpu.rs` rendering to a texture and reading back, a unit test on pixels | the test; the six checks |
| **S0 C2** The adapter and geometry parity | `ink.rs` implementing D1.2's geometry rows (fills, gradients, borders, shadows, images, clips, opacity, masks), `PainterChoice::Ink`, `EXACT_PAINTER=ink`, the pinned suites | the pinned suites at today's thresholds, text excluded |
| **S0 C3** Text, paths, layers, backdrop, blend | the swash glyph atlas, the path atlas with brushes, coverage clips, layers and the compose target, backdrop ops, blend modes; the whole-app band | every pinned suite and the band; the per-method fixture inventory started |
| **S0 C4** Kept rows, both arms, the measurement | D4 in ink; kept rows in `gpu.rs` as scene fragments (arm B); the four arms measured per §4 on the Mac | the numbers in §4; D15's first verdict (C over B) |
| **S0 C5** The direct Metal device | `device/metal.rs` (`MTLCreateSystemDefaultDevice`, the `.metallib`, pipeline states, the same trait) and its measurement against C | D15's second verdict (D over C): kept or deleted |
| **S0 C6** The Windows spike, on the NucBox | `device/d3d11.rs` in earnest (not throwaway): `D3D11CreateDevice` with BGRA, WARP forced once, a composition swap chain over Mica, the ink core drawing one screen, a windowless RichEdit drawn into a D3D11 texture; device-creation milliseconds beside wgpu's DirectX 12 device; `fxc` on that machine | the spike's numbers in §4; the cost of the composition swap chain to the first frame |
| **S1** The painted Apple benchmark carrier (if earned) | a macOS carrier over `CAMetalLayer` (window, pointer, keyboard, IME, the agent); the same on iOS; tvOS exposing its focusable items to the focus engine; Core Text's audit (D3.2) | §4 against the native Apple host, SwiftUI and UIKit on the heavy-list bench; cold start by phase under the budget; the key stays refused until D12's shipping gate |
| **S2** Windows (if earned; DirectX 11 regardless of D15's device verdicts) | the composition swap chain replacing `softbuffer`, Direct2D interop for the RichEdit, DirectWrite's audit, the game DLL's layers read back for the overlay rows | §4 against today's CPU path, Electron and the WinUI 3 oracle app, on the same screens |
| **S3** Linux | the device layer measured (wgpu Vulkan against `ash`), KMS and lavapipe; the readback branch of `gpu.rs` deleted; the replacement exception written with the first-launch numbers on the minisforum | §4 against the vello painter; parity under lavapipe in the async lane |
| **S4** Android | the ink core through the wgpu Vulkan presenter; `Canvas.drawGlyphs`'s audit; the low-end A/B that decides whether an OpenGL ES device layer exists at all; `gpu/present.rs` deleted | §4 against the vello swapchain host and the Canvas host |
| The web | LLP 1149.000: unscheduled until an app asks; the ES 3.00 subset is held in C1's translation spike so the web does not discover it alone | its own |
| **G1 to G3** The GPUI arm (§4.1) | the pilot's bench app to the heavy-list spec against a pinned gpui, Mac and Windows; measured per §4 | G1 after S0 lands on the Mac; G2 inside S2 on the NucBox; G3 inside S1 | the numbers in §4.1, whole app and renderer-only, with the non-comparables named |

S2 runs before S1 (Charlie, 2026-10-28: the prospect's platform, and the strongest numbers); S3 and S4 after either.

## 4. Measurement

The program is the same on every platform; the baselines differ. "Has numbers" is not a
threshold; D15 is.

| Metric | Definition | Harness |
|---|---|---|
| Cold start, by phase | process start to the first presented frame with the app's first screen complete, by the platform's timers (process start time and the presented-frame timestamp; never the agent's clock), median of interleaved runs, split into device creation, pipeline build, fonts, layout and the first frame; the first launch on a machine and later ones reported apart | `bench/heavy-list/cold-series.sh` on Apple; `EXACT_AGENT_WINDOW_FRAMES` on Windows (LLP 1015.000); the host's own boot log elsewhere |
| Frame interval and latency | p50, p95, p99 and late frames (over 1.5 display periods) per second during a fling at the display's rate, from presented-frame timestamps; encode time apart from GPU time | the heavy-list probe on Apple; DXGI frame statistics on Windows; `perf frames` everywhere |
| CPU | process and main-thread milliseconds per second during the fling and at rest; the renderer's encode time apart | the probe; `perf` |
| Memory | physical footprint, peak and at the end of the fling; a hello app at rest; the whole process tree for Electron; the atlases' miss rates, the overflow texture's bytes, uploads per frame and the layer pool's high-water mark | the probe; the platform's counters; the renderer's report in `state` |
| Energy | where a controlled measurement exists: Energy Impact on Apple; on Windows the battery report attributes nothing to a process, so it is recorded and not read as the renderer's | recorded, not gated |

| Platform | Baselines |
|---|---|
| Apple | the four arms of D15 on the readback path (S0); the native Apple host, SwiftUI and UIKit (`bench/heavy-list`) when a carrier presents (S1); **GPUI** (§4.1) |
| Windows | today's CPU path; an Electron window with the same list (its version pinned, production configuration, the process tree counted); the WinUI 3 oracle app (LLP 1115.004 D13; the Windows App SDK version pinned); **GPUI on its DirectX 11 backend** (§4.1); one list screen, the same pixels, the same machine, interleaved runs; renderer-only and whole-app numbers apart |
| Linux | the vello painter, the CPU painter |
| Android | the vello swapchain host, the Canvas host, the Compose arm (LLP 1085.000 stage 3's A/B tooling) |

The numbers land in this section as each chunk and stage does; a stage with no numbers has
not landed.

### 4.1 The GPUI arm

Charlie (2026-10-28): "we are going to need a measurement vs a GPUI based solution at some
point soon even if it's not apples-to-apples." It is part of the plan, as a **whole-app
comparison on the same screen, driven the same way, with what is not comparable named**:

- **The app**: the 2026-09-30 pilot's `gpui-messages-bench` (`~/projects/gpui-exact2-benchmark/`,
  gpui 0.2.2 with font-kit; LLP 1076 §5) brought to `bench/heavy-list`'s screen, row and
  parity rules (`SPEC.md`: the same 10,000 rich messages from the same data) against a pinned
  current gpui, with a Windows build on gpui's DirectX 11 backend. It is a bench app in the
  repository (`bench/heavy-list/gpui/`), beside the SwiftUI, UIKit and Expo ones.
- **The measurement**: §4's metrics and protocol, not the pilot's (which measured process
  CPU and RSS with different endpoints and no presented frames): cold start to the first
  presented frame, frame intervals and late frames during a fling from presented-frame
  timestamps, CPU per second during the fling and at rest, footprint; the fling driven into
  both windows the same way (on the Mac, the posted events LLP 1142 D3 landed; on Windows, the
  agent's window driver), five interleaved rounds, the same machine and display.
- **Where it runs, in order**: (G1) on the Mac after S0 lands: GPUI against today's native
  Apple host (the pilot's pair, redone to §4) and, renderer-only, against ink's headless paint
  with the caveat below; (G2) in S2 on the NucBox: GPUI's DirectX 11 window against Shelf and
  the heavy list on ink's DirectX 11 composition path, the prospect's comparison; (G3) in S1
  on the Mac: window against window once ink's carrier presents.
- **What is not apples-to-apples, said in the table**: GPUI's app has no kernel, runner or
  data source between the list and the renderer, and no CSS layout (its is Taffy on its own
  element tree); its text is Core Text's and DirectWrite's where ink's is swash's until D3's
  audits; it redraws whole frames immediate-mode (LLP 1148 F2's idle numbers); its window and
  input are its own platform layer, not winit's or the native host's. So two columns: the
  whole app (what a user and the prospect see) and, where it can be isolated, the renderer's
  paint time. A GPUI number that beats ink is a finding to chase, not a verdict to argue with.

The numbers land here as each G lands.

**S2.1, 2026-10-28** (`lane/ink-s2` at `dfd41f041d`; qualifying the DirectX 11 composition
path on the NucBox desktop). Six desktop journeys pass as native tests the desktop session
runs: drag-resize, minimize and restore, a DPI round trip (100% to 150% and back, Shelf
reactivated after the change), the idle caret, forced device loss (CPU composition
presented, the hardware device back 1,020 ms later with backoff) and sleep and wake (the
physical display waited for); lock and unlock is untested, having no unlock operator on
that machine. Startup: the D3D11 device is now created on a thread while fonts load and the
first layout runs, the first frame waiting only at the join; the composition path's paired
overhead over today's `softbuffer` first screen fell from 59.2 to 26.8 ms at 1024×720 and
from 48.9 to 17.1 ms at 1920×1080 (ten interleaved samples each), the remainder being the
join's wait on a 46.5 ms hardware device creation (the swap chain 1.0 to 1.1 ms, the
composition 0.7, the first present 1.3). That is above the 10 ms the stage's first chunk set
for flipping the default, so **Windows ink still defaults to wgpu**; S2.2 presents the first
frames through `softbuffer` and migrates to the composition swap chain when the device is
ready, which takes the device off the first frame's path entirely. The pinned suites and
`exact-windows`'s tests pass with both devices on the NucBox; the six checks on the Mac.

**G1, built 2026-10-28** (`6ffede6aa8` on `lane/gpui-arm`, `bench/heavy-list/gpui/`): the app
on gpui 0.2.2 (crates.io's newest non-yanked release that day, pinned with its own lockfile,
native Metal with build-compiled shaders), the 10,000-row screen to `SPEC.md`, background
ImageIO thumbnails; the driver (`measure.m`: five interleaved rounds in fresh processes, the
same four flings posted into each window through `host/apple/wheel.swift`'s events, ten
seconds of fling and ten at rest) and the probe (`probe.m`, linked into copies of both
bundles: cold start from `ri_proc_start_abstime` to GPUI's first presented drawable through a
`nextDrawable` hook, or to the native host's first settled layer snapshot, which are not
the same milestone and are reported as such; a display-link sampler for frame intervals and
late frames; `proc_pid_rusage` for CPU and footprint). Its `RESULTS.md` records the
non-comparables and the inspected screenshots. The series itself was refused by its own
preflight because other lanes were compiling on this Mac; it runs quiet after the landing
(G1m), and the tables are empty until then.

**S0 C1, landed 2026-10-28** (`lane/ink` `428ecd96fa`; the crate, the shader, the wgpu device,
one quad read back; M5 Max, Metal through wgpu, 200×200 at 2×). Instance, adapter and
device creation 22 to 209 ms across runs; the first pipeline build from WGSL 325 ms
(`cached=false`: wgpu's naga translates and Metal compiles the MSL at launch), 1.7 ms on a
later run; the first frame 10.8 ms; a warm frame including readback 0.36 ms median of 7
(0.30 to 0.84), encode and submit 0.08 ms. (C1's 325 ms is the one first-ever build S0 saw:
Metal's system cache keys on the MSL's text, so a shader text new to the machine misses it;
C4 and C5, running the same text, never missed it again and could not clear it: their
"empty" numbers are empty application caches over a warm system cache.) The translation spike's compile half passed for
MSL, HLSL 5.0, SPIR-V and GLSL ES 3.00 once instances became per-instance vertex attributes
(D6.1). C1 packed D1.1's 224-byte field list into the 192-byte stride: kind and brush share a
word, the affine carries no padding, and the two clips' radii are `u16` at 1/16 device pixel
(a larger radius makes a layer).

**S0 C2, landed 2026-10-28** (`71c6b06287`; the adapter `host/linux/src/ink.rs`,
`PainterChoice::Ink`, geometry parity). Every pinned suite that runs each painter passes
for ink (`paint` 38, `borders` 6, `tint` 2, `gradients` 1, `visual` 8, `materials` 1,
`backdrop` 1, `image` 3), with glyph ink, `clip-path`, masks and backdrop crops excluded
until C3. The whole-app geometry band against the CPU oracle: **mean 0.20/255, 0.02% of
pixels over 32** (vello's: 3.24 and 2.98%, LLP 1015 §8). Debug test profile, ten
interleaved Caltrain frames, medians: encode and render 0.38 ms (vello 1.33) at both
390×844 and 1920×1080; readback 0.94 and 1.57 ms (vello 1.52 and 1.71); the whole frame,
walk included, 15.6 and 15.7 ms (vello 18.0 and 17.7), which is the debug walk's time and
is not a renderer number: C4 measures in an optimized profile.

**S0 C3, landed 2026-10-28** (`0a007b73d8`; swash glyphs into the atlas, paths with brushes,
coverage clips, masks, the compose target, backdrop ops, fifteen blend modes; the missing
D1.2 fixtures added in `tests/pinned/paint/coverage.rs`). The full pinned suite passes for
ink (94, 1 ignored as before); the whole-app band **with text**: mean 0.21/255, 0.02% of
pixels over 32. Atlas counters at 390×844 (M5 Max, wgpu on Metal): Caltrain 307 glyph
entries (the 512² alpha page, 256 KB) and no paths; Shelf's library 135 glyphs and 12 paths;
Excord's start screen 16 and 3 (its chat did not load headless, so its loaded screen is
unmeasured). Every unchanged second frame had zero misses and zero uploads; no screen touched
the overflow texture; the colour and ramp atlases stayed empty.

**S0 C4, landed 2026-10-28** (`b7184a2433` kept rows in both arms; `f26a2b3b0b` the
measurement, `host/ink/MEASUREMENTS.md`, the harness `presenter/kept_rows_tests/measurement.rs`,
ignored by name). M5 Max, `host-dev` profile, the same readback path for every arm, five
interleaved rounds, 84,000 frames; arms A (vello), B (vello with kept rows), C (ink with kept
rows), C−rows. Paint per frame, medians:

| Workload, mode | B | C | C over B |
|---|---:|---:|---:|
| Messages Stress 1920×1080, scroll | 2.152 ms | 1.434 ms | 33% |
| Messages Stress 1920×1080, still / text | 2.129 / 2.148 | 1.306 / 1.512 | 39% / 30% |
| Shelf 1920×1080, still / text | 1.938 / 1.985 | 1.136 / 1.164 | 41% |
| Excord 1920×1080, still / text | 1.885 / 2.023 | 1.102 / 1.180 | 42% |
| Messages Stress 390×844, scroll | 2.036 | 0.670 | 67% |

Encode 0.09 to 0.18 ms against 0.17 to 0.37; readback (which includes the GPU's completion)
0.35 to 1.03 ms against 1.55 to 1.64 (the desktop scroll's difference 0.60 ms; across the modes 0.5 to 1.3 ms): a path-time
advantage that includes the GPU's completion, not a GPU timestamp; which passes cost it is
not established by these numbers. Excord's rows are its start screen (its chat did not load
headless, C3); Messages Stress is the heavy-list workload D15 names. CPU per frame equal on the desktop scroll (1.30 ms)
and 46% lower at phone size (0.667 against 1.233). Rows matter in both arms: B takes A's
scroll from 2.716 to 2.152 ms, rows take ink's from 2.155 to 1.434. Memory: footprint 380
to 424 MiB against 530 to 561 (the vello arms' buffers); sampled peak RSS during the modes
lower for ink by 9 to 11 MiB; lifetime peak RSS medians lower on five of six rows and 5.3%
higher on one (Messages Stress at 1920×1080: 86.9 against 82.5 MiB), with the worst of five
runs 15 to 23% above B's (95.7 against 83.6 MiB there; Shelf 108.6 against 94.1; Excord
115.8 against 94.5), a spike C5 must explain. Device creation is
the same in every arm (12.4 to 13.1 ms: the driver's, as the panel said); wgpu's pipeline
build 1.95 ms for ink against 9.0 ms for vello, with no run ever cached by an application
cache on this backend, so a first-ever compile was not measurable. Zero layer targets and
zero overflow bytes in every frame: the O(σ) blur and the full-viewport clip layers the
reviews flagged were not exercised. **D15's first verdict:** the improvement margin is cleared on every median (frame time 30
to 42% at 1920×1080, no median worse by more than 10%); the full gate is not: the lifetime
RSS spread and presented late frames (unmeasured on a readback path) are open, the first
carried to C5 and both to S1 and S2's carriers.

**S0 C3f, landed 2026-10-28** (`c542f79fbd`, `03198bc205`): the two blind code reviews of
C1 to C3 (GPT-6.1 Sol and Muse Wonder Full 1.0; Grok's three attempts died on its own
permission prompts) raised twenty findings; eighteen were confirmed against the code and
fixed, each with a named regression test in `tests/pinned/paint/reviews.rs` or
`exact-ink`'s unit tests, two were refuted against the code. The ones that change the
design as written: the device trait's uploads and reuse are gated on submission completion,
not on wgpu's in-order queue (D11, before any direct device); a source-over instance never
shares a batch with a replace or blend one (the pipeline is the batch's at push time);
inherited clips apply while a group's children draw, so a clipped-out child never reaches a
backdrop blur; GPU allocation failure is an `Err` that reaches the CPU fallback; the blur is
bounded by downsampling (a σ=50 blur: layer bytes 917,504 to 317,440, 1.03/255 from the
oracle); a path larger than a page is retained across frames; the gradient ramp keeps hard
stops by stop evaluation; mips are area-correct and anisotropic reductions separable; SVG
dashes are dashed. Left in `QUEUE.md`: blend and backdrop copies still span the viewport
(Muse 7). The whole pinned suite: 103 passed; the band with text: 0.21/255, 0.02%.

**S0 C5, landed 2026-10-28** (`62e64568e2`, `752f56f38f`; `device/metal.rs` over objc2-metal
0.3.2, the `.metallib` embedded at build, an `MTLBinaryArchive` kept on disk,
`EXACT_INK_DEVICE=wgpu|metal`). Arm D (ink on direct Metal) against C (ink on wgpu's Metal),
C4's protocol, 42,000 frames, both devices passing the whole Linux suite (1,887 each). Paint
per frame at 1920×1080, medians: Shelf 0.820 against 1.124 ms (27%) and Excord 0.768 against
1.091 (30%), ranges separated; Messages Stress 1.252 against 1.298 (3.5%, ranges
overlapping) to 6.6%. Encode 0.015 to 0.027 ms against 0.08 to 0.14 (wgpu's per-draw cost).
CPU per frame 11 to 47% lower in every workload (Excord still 0.23 against 0.44 at phone
size; Messages scroll 1.04 against 1.17 at 1920×1080, 11%). Lifetime peak RSS medians 5.7 to
11.9 MiB lower with no spikes (66.8 against 78.7 MiB on Messages Stress at 1920×1080, 83.8 against 93.8 on
Excord); footprint 223 to 253 MiB against 375 to 420: wgpu's own baseline is about 150 MiB
of it. Boot is where D loses: device creation 29.6 ms against 12.9 (the device timer ends before
the pipelines; the library load and the archive's population are inside the pipeline
phase, and the archive's serialization after it, so the device regression is its own and
unexplained), and the pipeline phase 126.9 ms with an empty application cache (Metal
compiling the `.metallib`'s AIR to the GPU's binary, as Astra's review E4 said it would; the
archive populated in that time) against 0.52 ms with the archive retained: whole renderer
construction 162.0 ms empty and 31.7 ms retained, against wgpu's 15.3 and 15.6. The OS's
shader cache was warm throughout and could not be cleared, so these are empty-application-
cache numbers, not a first-ever launch on a machine, on either arm. **D15's second
verdict:** direct Metal is kept: earned on frame time on two workloads of three beyond the
spread, on CPU per frame everywhere and on memory, and it has none of C4's RSS spread (C5 localized the spikes to the wgpu arm, in its
readback-copy phase; arm B on the same readback path did not spike, so the copy alone does
not explain them; the pixmap-reuse fix is in `QUEUE.md` and the obligation stays with the
wgpu layer, D15); its
boot regression carries a dated obligation (D15, amended in r4 from the panel: closed or
explicitly renegotiated by 2026-11-04 and before S1's carrier is enabled, else the direct
layer is withdrawn), and its numbers are the Apple line of §8's replacement exception as
measured: 162 ms of renderer construction with an empty application cache, 31.7 ms with the
archive retained, the OS cache warm in both.

**S0 C6, landed 2026-10-28** (`eb8507e445`, `910ca295e4`, `e83558a4d6`; `device/d3d11.rs`,
`d3d11/composition.rs`, `d3d11/rich_edit.rs`, `d3d11/text_host.rs`; `build.rs` compiles DXBC
with `fxc` where it exists and the device refuses by name where it does not;
`host/windows/src/window.rs` presents through ink with `EXACT_PAINTER=ink`, the `softbuffer`
path kept). On the NucBox (Ryzen AI Max+ 395, Radeon 8060S, Windows 11 26200, a 5120×1440
display at 60 Hz): Shelf's first screen in a real window through a two-buffer premultiplied
BGRA `FLIP_DISCARD` DirectComposition swap chain with the frame-latency waitable object and
Mica showing through (the capture: `target/ink-c6-shelf-window.png`), and a windowless
RichEdit (`CreateTextServices`, `TxDrawD2D` through Direct2D on the same BGRA device into a
render-target texture ink samples) with text typed into it from the keyboard. Every suite
green on both machines (the NucBox: `exact-windows` 29, the pinned suite 102, `exact-ink`
19). Ten fresh processes each, `host-dev`:

| Device | Creation | Pipelines |
|---|---:|---:|
| DirectX 11, hardware | 48.1 ms | 2.7 ms (from DXBC) |
| DirectX 11, WARP forced | 14.2 ms | 3.1 ms |
| wgpu DirectX 12 (the control) | 138.6 ms | 457.3 ms (WGSL translation and DirectX 12 pipeline build: most of this delta is D6's shader-form decision, not the device layer's; whether the driver defers any compile to the first draw is not split out) |

No S0 result reports cold start as §4 defines it (process start to the first presented
frame, phases summed, against the budget): C4 and C5 report device and pipeline phases, and
C6 reports an absolute first screen in a `gpu-dev` build; D15's phase-by-phase argument of
the 100 ms budget is S1's and S2's to make. The composition swap chain's own setup is 1.8 to
2.1 ms; Shelf's complete first screen reached DXGI's displayed timestamp 406 to 411 ms after
process start in the `gpu-dev` build,
46 to 49 ms later than today's `softbuffer` path by a DWM-synchronized proxy (the device's
48 ms is most of it; LLP 1115.004 D4.3 asked for this number before the swap chain lands,
and S2 owns bringing it down: the device created while fonts load). In the window, Shelf's still redraw in the `gpu-dev` build is **0.41 to 0.42 ms of paint and
0.35 ms of present** at both sizes, against today's CPU path's 3.3 ms paint plus 1.1 ms
upload at 1024×720 and 6.6 plus 3.0 at 1920×1080 (and wgpu's DirectX 12 through the same
readback-and-upload path, 3.7 plus 1.1 and 5.1 plus 3.0): about a twelfth of the per-frame
work at 1920×1080. The heavy list in the same window (`host-dev`): 0.58 to 0.67 ms of paint
a frame, the present wait being the 60 Hz refresh, 1.5 to 1.9 ms of process CPU a frame. The one
regression is headless: the DirectX 11 readback path's scrolling frame at 1920×1080 is
3.92 ms against wgpu DirectX 12's 2.72 (its staging copy; `QUEUE.md`), which the window
never pays. The NucBox accepts `FLIP_DISCARD` on a composition swap chain where Microsoft's
documentation names `FLIP_SEQUENTIAL`; that qualifies this driver, and S2 checks older
builds. The RichEdit is the spike's field, not Shelf's search field yet: binding to a
Contract field, the IME, selection and UI Automation are S2's. **S0 C7, landed 2026-10-28** (`f41aaafec4`): D12's manifest key. `host.windows.painter`,
`host.linux.painter` and `host.android.painter` take `"ink"` (`scripts/app.schema.json`,
with a manifest test that `"gpu"` is refused); the bake carries the choice into the
generated config without changing compatibility ids (`contract/cli/src/manifest.rs`,
`bake/src/compat.rs`); the presenter resolves it with `EXACT_PAINTER` applied last
(`presenter/painter.rs`); Android refuses it with the named reason that its Vulkan presenter
for ink does not exist yet; `apps/shelf/app.json` opts Shelf in on Windows and nowhere else.
On the NucBox, Shelf selected the DirectX 11 device from its manifest, presented 120 desktop
frames and opened Add Book; the CPU override did the same. **S0 is complete**: its chunks are
on `lane/ink` through `f41aaafec4`, the two D15 verdicts above stand as the panel qualified
them, and C6f landed the second blind code review's fixes (`58adeb76e8`, `94865d63e7`,
`1cca4b7396`): every finding confirmed or refuted with its disposition in the commit; failed
recordings are never kept (both arms); the direct Windows path blinks its caret, survives a
resize (the waitable object reacquired), skips an occluded present and recovers a lost
device on the CPU painter through the same composition with backoff from 250 ms to 30 s;
the RichEdit spike is test-only; Windows ink defaults to the wgpu device with the `softbuffer`
present, DirectX 11 by `EXACT_INK_DEVICE=d3d11` and WARP only when asked; oversized retained
coverage rasterizes its visible region; a fractional-pixel scroll re-records hinted rows and
replays coverage clips at their phase; CPU-written Metal textures stay `Shared`; each device
has an in-flight eviction test; the harness records its sampler, asserts its fixtures' hashes
and rotates arm order; `unsafe` is back inside `exact-ink`. The pinned suites pass on both
devices, the Linux smoke passes with ink, and the parity band is unchanged (0.21/255). Two
tests were red at the lane's gate under a system load near 400 (another lane building on the
same Mac): timing assertions in the root suite and a CPU-painter image test; both are rerun
quiet before the landing, which follows.

## 5. Questions that are Charlie's

Each carries the proposed answer and the panel's recommendation and confidence (0 to 100):
Muse (M), Grok (G), Astra (A), on r1. **Ruled (Charlie, 2026-10-28): "go w recs, ask panel if
not 80+% confident."** The proposed answers stand as rulings except Q14 and Q21, which r4
puts to the panel; Q2's revision (one core, the device layer by measurement) and Q20's (the
control arm) were decided before S0 and stand on its numbers.

1. **Write our own renderer rather than take vello, GPUI or Skia?** Yes, as S0's prototype
   with the thresholds of D15 deciding the replacement (§0). *M:* yes, 75. *G:* yes, 70. *A:*
   a bounded prototype, not the replacement commitment yet, 88. All three: the measured costs
   are mostly the integration's; the control arm decides.
2. **Each platform's own API, not wgpu?** Revised: one core on a wgpu device layer, DirectX
   11 direct regardless, Metal and Vulkan direct only where measured to earn it; LLP 1009 D1
   amended narrowly (D5, D15). *M:* direct, 65 (the boot argument weak). *G:* direct, 76.
   *A:* allow direct where justified, do not prohibit wgpu everywhere, 91. The revision is
   Astra's, which the other two's reasoning supports.
3. **DirectX 11, not DirectX 12, on Windows?** Yes (D5.2). *M:* 85. *G:* 88. *A:* 90.
4. **Android: Vulkan through the existing presenter, OpenGL ES only for a device that cannot
   open Vulkan?** Yes, revised from r1's OpenGL ES (D5.2). *M:* OpenGL ES, 70. *G:* keep
   Vulkan, 78. *A:* keep Vulkan, 76.
5. **One WGSL source in a named portable subset, translated at build, overridden for speed
   or correctness where measured?** Yes (D6). *M:* 60. *G:* 80. *A:* 93.
6. **Platform rasterizers (Core Text, DirectWrite, Android's `drawGlyphs`), swash first,
   each admitted by its audit; a failed audit keeps swash and the stage lands?** Yes (D3,
   D9). *M:* 80. *G:* 84. *A:* 91.
7. **Paths on the CPU into an atlas with brushes in v1, with the overflow texture?** Yes
   (D2.3). *M:* 80. *G:* 76. *A:* 81.
8. **Kept rows as instance lists that pin what they reference?** Yes (D4). *M:* 75. *G:* 82.
   *A:* 97.
9. **On Apple the painted presenter is an app's manifest choice, refused by the build until
   the carrier passes D12's shipping gate; the native host stays the default?** Yes. *M:*
   90. *G:* 86 (refuse the key until VoiceOver). *A:* 98 (a benchmark carrier may precede
   the gate).
10. **S0 on the Mac in six chunks with the Windows spike and the direct-Metal measurement
    inside it?** Yes (§3). *M:* spike inside S0, 65. *G:* 78. *A:* 94 (Windows is the first
    product gate).
11. **Delete vello's painter branches as each platform passes, the shared file last; keep it
    in the Canvas module; keep its kept rows whatever S0 decides?** Yes (D13). *M:* 70.
    *G:* 90. *A:* 97.
12. **Electron and WinUI 3 as the Windows baselines, versions pinned, process trees
    counted?** Yes (§4). *M:* 75. *G:* 85. *A:* 99.
13. **`host/ink`, `exact-ink`, the adapter in `exact-linux`?** Yes (D14). *M:* 60. *G:* 78.
    *A:* 99.
14. **The boot rule: a replacement DEFERRED exception for the driver's pipeline build from
    compiled shaders, written with measured numbers, rather than a withdrawal?** Yes, in the
    form D6.3 now states: it is compiling; a measured, owned exception per platform; a cache
    miss never blocks first pixel. *r4 panel:* A option (b) bounded, 94; M option (b) with the
    miss-never-blocks condition and a bundled archive as an accelerator only, 70. (r1: M
    needed the ruling; G do not withdraw until the first launch is inside the budget; A
    propose the replacement exception.)
15. **Colour: sRGB-encoded premultiplied targets, blending and gradients in the encoded
    space, the swap chain tagged sRGB, the deviation from linear declared?** Yes (D2.7).
    *M:* 70. *G:* 70. *A:* specify it as a contract, 99.
16. **The device fallback chain and the device-loss behaviour of D5.4?** Yes. *M:* 85. *A:*
    99.
17. **Shader binaries in the bundle; pipeline caches keyed by driver and binary hash?** Yes
    (D14). *M:* 70.
18. **Accessibility, editing, the IME, keyboard navigation, focus and lifecycle are the
    painted Apple carrier's shipping gate, and the key is refused before it?** Yes (D12).
    *M:* 80. *G:* 82. *A:* 99.
19. **The web form stays unscheduled until an app asks, with the ES 3.00 subset held in
    S0?** Yes (§3; LLP 1149.000). *M:* 75. *G:* 82. *A:* 97.
20. **Build the control arm (kept rows in vello) in S0?** Yes, revised from r2's no (D15;
    §3 C4). All three asked for the comparison on the same path.
21. **D15's thresholds: 20% for ink over the improved vello, 10% for a direct device layer
    over wgpu, no metric more than 10% worse?** Kept, restated as D15's r4 amendment: medians
    with a median-of-peaks guard, a boot gate in milliseconds, direct Metal kept with a dated
    obligation whose consequence is deletion. *r4 panel:* A keep the margins, repair the
    regression and startup requirements, 91; M keep both verdicts on substance, restate D15
    before S3 reuses it, 75 (85 that it was under-specified). (r1: A predeclare them, 99; G
    within 10% means the device layers are not the performance work.)
22. **What "the same everywhere" promises an app that opts in: the same styling and layout,
    the platform's text and accessibility?** Proposed (D12). *A:* 99.
23. **The supported matrix of D5.3 (minima, the 11_0 floor, WARP for remote sessions, KMS
    only on Linux, Android 10 with Vulkan 1.1)?** Proposed. *A:* 98.
24. **Four separate commitments, each its own waiver: the renderer prototype (S0), the
    painted Apple carrier (S1), the painted web (LLP 1149.000), and the replacement
    compilation exception (§8)?** Proposed. *A:* 98.
26. **A GPUI arm in the measurement, whole-app on the same screen, with the non-comparables
    named?** Ruled yes (Charlie, 2026-10-28; §4.1): G1 after S0 lands, G2 in S2, G3 in S1.
25. **How often is "first launch"?** (Muse, r4.) Every shader-changing update, driver
    update and OS update re-incurs the first-launch pipeline cost, and Exact's delivery pushes
    signed updates routinely (LLP 1030). Proposed: the pipeline cache and archive are keyed by
    the binary's hash and the driver (D14) and warmed at an update's install, before its
    activation, where the platform allows a background launch of the renderer; old binaries'
    archives are evicted by hash with a bounded count; and `metrics.mjs` reports the first
    launch after an update as its own row so the cadence's cost is seen, not assumed.

## 6. Not in this document

- The web, beyond D5's row: LLP 1149.000.
- The painted Apple carrier's menus, dialogs, pickers and delivery beyond what D12's gate
  names: a plan of its own when S1 lands, in LLP 1115.002's terms.
- Text shaping, fallback and the text cache (LLP 1085.000); the Fluent, Material and Apple
  looks the painter draws (LLP 1115.000, 1115.001, 1115.004); the kernel.
- 3D, and effects the Backend trait does not emit. (The backdrop ops it does emit, blur and
  the colour ops, are D1.2's, not excluded.)
- LCD (ClearType) glyph coverage with dual-source blending: the hook is a glyph format; built
  when LLP 1115.004 D6.3's measurement asks for it.
- Sparse strips on the GPU for paths (D2.3) and vello_cpu as the CPU fallback: evaluated
  when a consumer needs them.
- Partial presentation (D7): a measurement after S2 and S4.
- X11 and Wayland on Linux (D5.3).

## 7. Risks

1. **The project becomes a host rewrite.** Apple and the web need editing, accessibility,
   focus and platform integration beyond drawing. D12 separates the renderer's qualification
   from a shipping presenter's, and §5 Q24 keeps each a separate commitment.
2. **A false performance conclusion.** Removing the readback, fixing presentation, changing
   glyph rasterization and replacing vello at once would hide causality. D15's four arms
   change one thing at a time on one path; swash is held constant until D3's audits.
3. **Retained-resource corruption.** Atlas eviction, asynchronous uploads, external producers
   and frames in flight overlap. D4's pinning, D8's overflow and D11's completion-based
   retirement are specified before any optimization; S0 forces eviction during a fling.
4. **Windows presentation is the critical path.** r1 specified it wrong (§R); the composition
   swap chain's cost to the first frame, `D3D11CreateDevice`'s cold cost and the RichEdit's
   texture are C6, before S1 starts. If the device threatens the 100 ms budget it is created
   on a thread while fonts load and the first layout runs, with the first frame waiting only
   for it.
5. **Layer and path workloads defeat "a few batches".** Nested clips, blend modes, images
   with distinct bindings, masks and backdrop filters multiply passes. §4 counts batches,
   intermediate pixels, rasterized path pixels and uploads on real screens; D2.4's third
   in-instance clip and a small-image atlas are the first additions if they show it.
6. **Driver and deployment diversity exceeds the test matrix.** Lavapipe and headless
   browsers establish part of correctness; D5.3 names what is supported, and S2 and S4
   qualify real integrated GPUs, remote sessions and two Android GPU families.
7. **Text quality diverges in motion or configuration changes.** D3.3's audit runs during
   scrolling, transforms, monitor changes and variable fonts, apart from the screenshot band.
8. **Maintenance is underestimated.** D5 commits to two device layers and measures the
   others in; the estimate is corrected from the first complete Windows and Metal layers,
   failure paths included, before S3.
9. **The headline metric does not move.** Device creation is mostly the driver's; fonts (25 ms
   on a laptop, LLP 1015 §3) and layout bound cold start next. §4 splits the budget by phase
   so a gate argues phase by phase.

## 8. The DEFERRED trades

- **Proposed, pending §5 Q14** (the panel on r4: Astra recommends this form, 94, with the
  obligation D6.3 states; Muse's answer is folded when it lands): a replacement for the Linux
  painter's exception
  (`rules/DEFERRED.md` §Runtime, 2026-10-22). Today it admits "compiles its shaders on the
  first launch on a machine and reads them from a pipeline cache after" with 93 + 77 ms on
  RADV. The replacement admits the driver's pipeline build from compiled shaders at launch,
  per platform, with the numbers S0 (Metal, DirectX 11) and S3 (Vulkan, the minisforum's first
  launch) measure written into the entry, and names the budget each is held to. It is written
  when those numbers exist, not before.
- Unchanged: a canvas's module compiles at first use, after first pixel (LLP 1009 D5); games
  stay on wgpu and the wasm target; the GPU module's ABI (`gpu/README.md`).
- Separate commitments (§5 Q24): S0's prototype; the Apple carrier; the painted web; the
  exception. Each is admitted by its own line, or not.

## 9. What S0 decides

The three families named the same alternative in three forms: keep vello and remove its
costs in place (Muse, Grok), or keep the ink core and let wgpu stay where a direct layer
earns nothing (Astra). D15 builds both into S0's arms rather than arguing them: B is vello
improved in place, C is the ink core on wgpu, D is the ink core on a direct device layer,
all on one path with one workload. Whatever the verdicts, two things land: vello's kept rows,
and the Windows device layer that wgpu cannot provide. The rest is earned or deleted.
