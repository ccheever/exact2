# LLP 1015: Linux host v1 — what `host/linux` is, as built

**Type:** Spec
**Status:** Draft (r3; transcribes what landed 2026-08-29; r3 the same day after two code reviews of r2 — `llp/reviews/code-2026-08-29-linux-painter.{codex,grok}.md` — whose folds §2a records)
**Revised:** 2026-08-29 (r2: the painter is a walk over a backend; vello on the GPU is the main one, tiny-skia the fallback and the pixel oracle — Charlie's call the same day; §2, §7)
**Systems:** Linux host (new), Kernel (one default corrected), Tooling (`scripts/agent.mjs` — a third carrier; `scripts/smoke.mjs`)
**Author:** Claude (Fable 5) for Charlie Cheever
**Date:** 2026-08-29
**Implementer:** Claude (Fable 5), landed 2026-08-29 (this document transcribes the landing)
**Related:** LLP 1008 (the Apple host whose orchestration this repeats without its batch), LLP 1007 (the web host; the parity oracle), LLP 1001 §5–6 (layout is a host call; text measurement is injected), LLP 1002 D2/§4 (every host but the web runs `exact-motion`), LLP 1010 (scrolling — §3's chaining rule, implemented here in Rust), LLP 1011 §4 (the image policy, repeated), LLP 1012 (the agent API — this host implements its contract, not `Agent.swift`), LLP 1014 D2/D4 (children over a canvas; a painter host as the display list), LLP 1009 (the GPU module this host does not load yet), `QUEUE.md` §Open decisions ("Linux painter: wgpu or CPU raster" — the take v1 makes, §7), `rules/RULES.md` §Time budgets, `rules/NOT-DOING.md` §Surfaces. Research, never authority: exact1's 0418/0430 (one text engine measures and paints), 0323 (the measurement cache).

## Summary

`host/linux/` is the third host and the first that **paints**. The runner
and kernel run natively; after every commit the host lays the tree out
with the kernel's own layout (Taffy, measuring text with cosmic-text),
seeks the motion engine to the app's clock, and the painter draws the
kernel tree itself — **the kernel is the display list**: no batch, no
mirror, no view tree of the platform's, because Linux offers none. One
walk emits to a backend; **vello over wgpu is the main backend** (the
GPU), tiny-skia on the CPU is the fallback where there is no adapter and
the deterministic oracle for pixels. The pixels go to DRM/KMS dumb
buffers with evdev input, or into a buffer with no display at all — which
is how the host is tested: the agent API over stdio, a screenshot, a smoke
run, on a fleet Linux box with no GPU or on macOS in the seconds-loop.
Pure Rust end to end (vello, wgpu, tiny-skia, cosmic-text, png, drm-rs,
evdev): no system library is linked, so it builds on any Linux with no
packages and on macOS as a headless binary.
Measured on `expo-build-1000` (EPYC 9454, 43 font faces), the Caltrain app
in smoke mode, warm: **process start → first frame painted 20 ms** — fonts
3.7 ms, runner + layout + 878 text measurements (498 from cache) 15.6 ms,
paint 4.9 ms at 420×860. The same binary's `layout`, `tap`, `type`,
`clock`, and `screenshot` answer `scripts/smoke.mjs linux` unchanged from
the other two hosts, and its nested-scroll numbers are the macOS ones
exactly (the node stops at 652, the page at 471). Where this document and
the code disagree, the code and its tests are the authority.

HTML dialogs have no Linux presentation yet (LLP 1021 D2, 2026-09-11).
The painter excludes their subtree from pixels and hit boxes; a press reaching
`Host::dispatch_at` with `commandfor` is refused before its application handler.

## 1. The host (`host/linux/src/host.rs`)

LLP 1039 passes the boot size to the runner as `exactViewport`; the existing resize path re-answers it before laying out once and updates the presenter from that same commit. The resource and the agent’s `layout.viewport` report the same CSS-pixel dimensions.

**Locations (LLP 1038 D5/D8/D11, 2026-09-14).** The common app entry reads
the first non-flag argument beginning with `/` or a valid scheme, derives its
location with `exact_route::location_of`, and supplies it before the first
settlement, including a selected plan or fallback. No argument means `/`.
`type <navigation root> "/post/42"` dispatches `Event::Navigate(String)` once;
Linux has no integer dispatch ABI or running-URL OS callback. The agent still
reports navigation as unavailable; the router value remains in `state.slots`.

**Requests (LLP 1016 D2, built 2026-08-30).** `host/linux/src/executor.rs`
is the Apple host's executor with a wake the loop can poll: one
`ibex2::host::Host` on a worker thread — rustls off Apple (ibex LLP 0068
OQ2, resolved the same day: `transport::rustls_http`, HTTP/1.1 over rustls
through `ureq`, trusting the machine's CA bundle and only where there is
none the compiled-in roots, which the journal's first lines say —
`NSURLSession` when this host runs on a Mac) — endowed from the data crate's grants (origins, matched
whole), and a socketpair whose reading end sits in `display.rs`'s `poll` set
beside the input devices and the VNC server's, one byte per reply. After
every commit the presenter hands the runner's new requests to the thread
(`after_commit`); `Presenter::pump` delivers the queued outcomes to the
runner (`Host::fulfill_all`, one commit each) — the display loop pumps when
the fd is readable, the agent pumps at the start of every operation and
while `clock settle` waits (twenty seconds at most, then `settled: false`),
since the headless path has no loop. `host/linux/tests/host.rs` drives a
`send` against a loopback server through the real transport. `ibex2` is a
path dependency on the sibling checkout, as on the Apple host.

The Apple host's orchestration (LLP 1008 §1) with the batch deleted:
`Host::boot(plan_bytes, data, measurer, width, height)` decodes the plan,
boots the runner against `Kernel::new(measurer)`, hears the whole tree in
the engine (values, no transitions), lays the roots out under
`Offer::definite(viewport)`. `dispatch_at(view, event, now_ms)`,
`advance(now_ms)` (through `Runner::advance_timed`: each timer's commit at
its own due time, the clock landing where the runner says — LLP 1012 §2),
`resize`, `set_intrinsic`, and `tick(now_ms)` each return an error or
nothing; the painter then reads the kernel. The one thing the host keeps
beside the kernel is each node's **presentation values** — translate,
scale, rotate, opacity from `Engine::frame` — as `Presented`, which
`presented(id)` answers from the map or, for a node the engine has not
moved, from its committed rows. There is no mirror and no diff: a painter
in the same process as the kernel has nothing to keep in step. The
orchestration is ~120 lines and the same shape as `host/apple/src/host.rs`
line for line; lifting it into one crate is a mechanical move that waits
for a third native host (iOS uses the Apple one).

**The root lays out under the viewport and a root that is a block is as
tall as its content**, so the window is a viewport over a document (LLP
1010 §3): the presenter's page scroll offset and `fitDocument`'s extent
(the roots' frames, never smaller than the viewport) are the same rule as
macOS, deviation included.

## 2. The painter (`host/linux/src/paint.rs`, `gpu.rs`, `raster.rs`)

One walk of the live tree in preorder, emitted to a **`Backend`** — fill
and stroke a shape (a box with per-corner radii), an image into a box
under clips, a paragraph at an origin, push/pop a clip, push/pop an
opacity, the pointer, finish — and two backends draw it into the same
premultiplied RGBA frame at the device scale. Per node, in order: **background**
over the border box as a path with per-corner radii (CSS
`background-clip: border-box`); **borders** — a uniform border with a
radius is a stroke inset by half its width, anything else is four side
rectangles (the Apple presenter's rule). Widths are the kernel's effective
border widths: `none`/`hidden` paint and reserve nothing, and `currentcolor`
uses the node's computed text colour (LLP 1001 §1; `tests/paint.rs`). An **image** by `object_fit`
(`fill`, `contain`, `cover`, `none`, `scale-down`) in the content box,
clipped to it and to the border box's rounded path (LLP 1011 §4); a
**text** node's paragraph — the one the kernel measured at this width,
§3 — in the content box (vello draws the glyph outlines from the same
cosmic-text layout; tiny-skia blits swash's rasterized glyphs); an **input**'s value, or its placeholder in
`#757575`, vertically centred, with a caret when focused; then the
**children**, clipped when the node's effective overflow (`style.rs`'s rule
from LLP 1010: a `ScrollView`/`List` scrolls on y unless its row says
otherwise, an unset axis beside a non-visible one is scrollable) is not
`visible` — a `tiny_skia::Mask` intersected down the tree — and moved by
its scroll offset when it scrolls. Motion presentation values become one
transform about the box's centre, CSS's `translate · rotate · scale`, and a
**group opacity**: when it is not 1 the subtree paints into a layer and
composites once (a vello layer; on the CPU a viewport-sized pixmap per
node — fine for a transition, a cost for a hundred translucent cards).
`display: none`
paints nothing. `Canvas`, `Toggle`, `Svg`, `NativeView`, and `Pressable`
are boxes: background, borders, children. Colors are the kernel's
`0xRRGGBBAA` straight into tiny-skia.

**The walk also records every node's painted box** — the transformed
bounding box in viewport points, the clip it was painted under, and its
scroll offset if it scrolls — in paint order. That list is what the
agent's `layout` reports and what hit-testing reads, so there is no second
geometry: a box is where its pixels went. The pointer, when the host draws
one (DRM has no compositor), is an arrow painted last.

**The GPU backend** (`gpu.rs`) creates a wgpu device at boot (`Backends::
PRIMARY`, the high-performance adapter) and vello's renderer with area
anti-aliasing only; encodes the walk into a `vello::Scene` (shapes as
`RoundedRect`s, clips as clip layers, opacity as a blend layer over the
viewport, images as `ImageBrush`es cached by picture, text as
`draw_glyphs` runs — the font's data handle from cosmic-text, glyph ids
and positions in points, hinted); renders into an `Rgba8Unorm` texture;
copies it to a mappable buffer and reads it back into the frame. Shader
compilation is on the boot path — the trade Charlie took (§7) — behind a
wgpu **pipeline cache persisted to disk** (`~/.cache/exact/pipelines-*.bin`,
`EXACT_CACHE`) where the driver has one (Vulkan; Metal keeps its own).
**`EXACT_PAINTER=gpu|cpu`** chooses; unset, the GPU when an adapter exists,
else the CPU with a note on stderr — the fleet's builders have no GPU and
run every check on the CPU backend. **The CPU backend** (`raster.rs`) is
the tiny-skia code r1 described, behind the same trait: masks for clips,
a layer pixmap for opacity, swash glyph bitmaps for text.

CSS path masks intersect their parent only within transformed control bounds,
rounded outward with two pixels of antialiasing slack. Viewports or transformed
coordinates beyond 8,191 pixels keep the full-mask calculation, as do failed
transforms. Mask dimensions and floor-rounded alpha multiplication are unchanged.
A 192-case regression covers curves, transforms, parent coverage and tiled
fallbacks; 24 app screenshots and their layouts match the previous renderer.
Two paired release comparisons of Textflow's 120-frame Dancer workload on this
Mac measured about 8–9% lower frame cost (the second: 5.97 → 5.42 ms).
These are headless CPU measurements, not physical display cadence.

Rectangle fills now bound their shading work to the active mask's conservative
device-space bounds, with two extra pixels of slack. Only axis-aligned, unrounded
rectangles in viewports/coordinates up to 8,191 pixels take this path; rotated,
sheared, rounded and uncertain geometry retains the original path. Original
fractional edges are preserved and the unchanged mask remains authoritative.
Two transformed stack corners establish the bounds without cloning a path.
This also bounds the white clear during a partial repaint.

A 322-case pixel oracle compares the full path with fractional edges, scales,
negative scale, rotation, nested rounded clips, damage and the tiling boundary.
Twenty-four Textflow/Caltrain screenshots and their layouts are byte-identical.
Four alternating release pairs of the six-scene, 720-frame fixture measured total
process CPU time at 4.343 → 3.926 seconds (9.6% less, including setup/warmup).
Shared load varied elapsed timings, so no display-cadence claim follows. An isolated
1,200-frame Editorial control was effectively flat at 1.209 → 1.205 ms/frame.
Evidence: `/tmp/exact-text-cpu-d28853d6/` (`selected-cpu/` and `editorial/`).

Damage clearing uses the same non-antialiased rectangle paths that build the
binary damage mask to paint opaque white directly into the copied previous frame.
The mask still governs subsequent painting. Nonfinite geometry, coordinates or
viewports beyond 8,191 pixels, and scales whose viewport-edge round trip is not
exact retain the full masked clear. Overlapping rectangles remain idempotent.
An independent 896-case oracle compares the original full masked viewport across
fractional scales, overlapping/offscreen/empty/invalid regions and tiled fallbacks;
it passes in debug and release, alongside the existing clipped-fill oracle and
Textflow damage-versus-full repaint tests. Twenty-four app screenshots and their
layouts match. Four alternating release pairs measured total process CPU at
3.067 → 2.524 seconds (17.7% less) for the six-scene fixture relative to the bounded
rectangle renderer above. A second four-pair run confirmed 2.708 → 2.278 seconds
(15.9% less); its static Editorial control was effectively flat at 1.039 → 1.046
ms/frame. This is headless process CPU including setup/warmup;
shared load varies elapsed frame timings, and physical cadence remains unproven.
Evidence: `/tmp/exact-damage-clear-0ff47ba3/`.

Partial CPU frames now begin directly from the accepted pixels. The backend's
combined `begin_damage` entry resets frame-local state and copies the previous
surface once; refused damage falls back to an ordinary white frame. This avoids
allocating and clearing a surface that would immediately be discarded.
Completed frames, flow repaint history and retained content-region fallbacks
share an immutable `Arc<Pixmap>`. `Presenter::frame` and `Frame::pixmap` expose
that shared owner; the display receipt passes it through without another wrapper
or a deep copy. Repainting still writes into a separate surface, so old displayed
pixels cannot change. A resized failed frame gets its own cropped/padded surface.
The existing damage/full-repaint oracle checks shared ownership and old-owner
release, and content-region failure/resize tests check reuse without aliasing a
resized surface. Frame reset and incompatible-size fallback have pixel coverage.
The 896-case damage oracle and all 14 painter tests pass in debug and release;
24 Textflow/Caltrain screenshots and their layouts match. Two four-pair alternating
release runs of the six-scene, 720-frame fixture measured total process CPU at
1.166 → 1.079 seconds (7.5% less) and 1.248 → 1.139 seconds (8.8% less).
These include setup/warmup and do not establish physical display cadence.
Evidence: `/tmp/exact-frame-begin-dec547d2/` and its `confirmation/` directory.

When one input damage rectangle contains the entire union and its device edges
are integers, the backend records that exact rectangular coverage at the mask's
stack depth. An opaque, axis-aligned, unrounded fill with integer device edges can
then paint its intersection directly. Nested clips, fractional edges, transparent
fills, rotations and tiled/uncertain geometry retain the mask path. Popping that
mask or beginning another frame retires the proof; no additional mask is retained.

The first shape clip under that proven rectangular damage mask now fills one
fresh child mask and clears rows outside the damage rectangle. It avoids cloning
the parent and allocating a second scratch mask for the intersection. Coverage
is exactly 0/255, so no alpha multiplication is needed. Deeper clips, unproven
unions, fractional damage edges and tiled viewports retain the existing path;
additional shapes keep their original intersection order and rounding. No mask
cache or new retained owner is added.

A 3,024-case oracle compares the original intersections across shape sequences,
parent coverage, damage, nesting, scales, transforms and tiling. It and the existing
14 painter tests pass in debug and release; Textflow repaint tests and 24 app
screenshots/layouts also match. A fixed 12-pair alternating release comparison
measured six-scene process CPU at 1.123 → 1.068 seconds (4.9% less; 10 pairs improve),
after smaller four-pair groups showed 6.0% and 2.1% reductions under varying load.
Development-build process CPU was effectively flat (3.303 → 3.275 seconds).
These include setup/warmup and establish no physical display-cadence result.
Evidence: `/tmp/exact-nested-mask-ea01a719/`, including `extended/`.

A 1,320-case independent full-mask oracle checks individual fill variants, nested
clip push/pop, replacement clips and frame resets across scales and tiling limits.
Debug and release pixel suites and the Textflow repaint tests pass. Four alternating
release pairs measured the six-scene process CPU fixture at 2.295 → 1.787 seconds
(22.1% less) relative to the damage-clear renderer above. A second four-pair run
confirmed 2.226 → 1.695 seconds (23.9% less), with the static Editorial control
effectively flat at 1.017 → 1.029 ms/frame. Twenty-four app screenshots and layouts
are byte-identical. These are headless CPU measurements including setup/warmup,
with no physical display-cadence claim.
Evidence: `/tmp/exact-mask-fill-cfd70e0e/`.

An opaque rounded background can use the same exact damage rectangle when it
lies wholly inside either of the box's two solid central strips, with two device
pixels of clearance on every side. The proof requires finite, nonnegative radii
no larger than half the smaller dimension, finite nonsingular axis-aligned
transforms and the same non-tiled bounds. Otherwise the complete rounded path and
mask remain unchanged. No visible curved or fractional edge is replaced.

A 3,456-case full-mask oracle covers fractional scales, reflection, nonuniform
scaling, rotation/shear fallback, asymmetric/oversized/negative finite radii,
edge proximity, alpha, irregular damage and opacity layers. It passes in debug
and release. The clip-lifetime oracle now exercises eligible rounded fills too;
a separate test verifies the coverage proof rejects nonfinite/invalid radii.
The oracle also exposed a pre-existing tiny-skia panic for an internal NaN radius,
reproduced on the published baseline and recorded in `QUEUE.md`; it is not a
passing pixel case. Twenty-four app screenshots and layouts remain byte-identical.
Four alternating release pairs measured six-scene process CPU at 1.727 → 1.140
seconds (34.0% less) relative to the rectangular-damage renderer above. A second
four-pair run confirmed 1.743 → 1.142 seconds (34.5% less); the static Editorial
control varied by about 3% or less across both comparisons. This is headless CPU
including setup/warmup, not evidence of physical display cadence.
Evidence: `/tmp/exact-rounded-interior-fb9f8b65/`.

**The one kernel change.** `text_color`'s default in `schema.json` was
`4278190335` — `0xFF0000FF`, opaque red in the kernel's packing — and no
host had read it: the web host lowers only set rows and the browser's
default is black, the Apple presenter falls back to black itself. The
first painter to read `style.text_color` directly painted every unset text
red. It is `255` now (`0x000000FF`, the web's black); `Color::BLACK` says
the same. The other rgba8 defaults are transparent and were right.

### 2a. The review folds (2026-08-29, r3)

Two code reviews of r2 (codex and grok, mutually blind, both NOT READY on
the reviewed tree) converged on the same defects, folded the same day:

- **A failed GPU frame no longer blanks the app.** `Presenter::frame` used
  to answer a backend error with a white frame and *no painted boxes*, so
  every later press missed — and `Auto` fell back to the CPU only at boot.
  Now, under `Auto`, the CPU painter takes over from the failed frame on
  (a note on stderr) and paints it; a forced painter that fails keeps the
  last frame's boxes so input still lands where things were.
- **The VNC server takes nothing from the wire at face value.** A
  `ClientCutText` length (a `u32`) was allocated as given — a client that
  finished the handshake could ask for 4 GiB; it is drained in 4 KiB
  reads now, as are `SetEncodings` and `FixColourMapEntries` (which was an
  unknown message before). A pixel format whose shift would overflow the
  pixel is refused (kept at ours), and the channel arithmetic is 64-bit.
  A client's writer thread, which waited on a condition variable forever
  after the client left, is told under both locks it waits on and joined.
  The bind-all default (`0.0.0.0:5900`) stands as the spec wrote it — the
  KVM-replacement use needs the LAN — with these bounds in place of the
  authentication it does not have.
- **The pipeline cache is written whole or not at all** — beside its path
  and renamed into place (a crash mid-write or two launches at once leave
  the old file or none). `cached` in the boot report means a cache file
  was found and handed to the driver; whether it was used shows in the
  shaders' milliseconds.
- **The GPU's image cache holds the picture it keys.** It was keyed by an
  `Rc<Pixmap>`'s address alone, which an allocator may reuse after a reload
  drops the picture; each entry now holds the `Rc`, and `begin` drops
  entries nobody else holds.
- **Parity is a test, not a sentence.** `the_two_painters_agree_within_a_band`
  in `tests/paint.rs` (§5 has the numbers).

Left from the reviews, declared: the empty `<input>`'s space (§6); the
glyph-placement deviation (§5); the r1 wording that survived in
`display.rs` and `Cargo.toml` is corrected.

## 3. Text: one engine (`host/linux/src/text.rs`)

cosmic-text, measuring and painting from one cache — LLP 1008 §3's lesson
in Rust. A `Paragraph` is a width-specific snapshot: the shaped, wrapped
`Buffer`, its size with the same `ceil` the kernel receives, its first
baseline — cached by (spec, width), where a spec is the runs (text, size,
weight, italic, line height, letter spacing) plus alignment and
`line_clamp`. `Measurer` (the kernel's `TextMeasurer`) and the painter
share the engine through an `Rc<RefCell<_>>`: what was measured is what is
painted, by construction. Normal text wraps at word boundaries (`Wrap::Word`). CSS `overflow-wrap:
break-word | anywhere` uses `WordOrGlyph`; only `anywhere` includes emergency
breaks in min-content sizing. The policy participates in the paragraph cache
key and applies to measurement and painting. For normal and break-word,
**min-content is the widest word**, found by
wrapping at width zero — the first Linux render broke every countdown
"28" into "2" / "8" because the probe used glyph wrapping, so min-content
was one glyph and the flex row shrank to it. `line-height: normal` is the
font's ascent + descent + line gap at the size (skrifa metrics from the
face the shaper picked; cached per size/weight/style); a set line height
centres each run's glyphs in its own box. The paragraph strut and
per-run ascent/descent extrema determine shared baselines in the cached
paragraph; both raster and GPU painters use those same baseline positions
(LLP 1035.000.000). Ratios arrive resolved per font; `None` is normal and
`Some(0)` is explicit zero. Cosmic-text requires positive shaping pitches
for its scrolling loop, so glyph metadata maps those internal pitches back
to the original CSS lengths before measurement or painting. `line_clamp` is `Ellipsize::End(Lines(n))`. Alignment
is per line. An empty text has no line box (the web; the Apple presenter
gives one). Glyphs are rasterized by swash once per (glyph, subpixel bin,
color) into small premultiplied pixmaps, cached; painting is one blit per
glyph at cosmic-text's pixel-snapped position, through the node's
transform when it has one (a scaled node resamples its glyphs, as a
browser does mid-transition).

Fonts are the system's (`fontdb`; `EXACT_FONTS` adds a directory), the
family always `sans-serif` — **resolved to an installed family at boot**:
`EXACT_FONT` when set (the pinned font a cross-machine fixture needs),
else cosmic-text's default when it is installed, else the first present
of fontconfig's own preference order (`60-latin.conf`: Noto Sans, DejaVu
Sans, …; on macOS the browser's: Helvetica Neue, Helvetica, Arial). Left
to cosmic-text, the generic family named "Open Sans" on Linux, which the
minisforum does not have, and its per-glyph fallback then scored every
font on the machine by weight: the app's weight-600 button came out in
URW Bookman with a space from Noto Color Emoji, 16 pt wide, on both
painters — a shaper's answer, and the same on both, which is how it was
told apart from a painter's. With no fonts at all the host says so on
stderr and text measures zero. The scan is the one boot cost that is the
machine's, not the app's: **3.7 ms for 43 faces** on the builder, **25 ms
for 787** on this Mac — a font cache is the known answer when it matters.
Measured on the app: 878 requests for 128 text nodes at boot (Taffy asks
several times per node), 498 from cache, 13.5 ms shaping — ~35 µs per
miss, twice CoreText's; the tail is the fixed-point layouts, not the
shaper. Chrome on Linux with the same DejaVu Sans wraps the same lines:
the app's root is 3244 pt tall here against 3246 on macOS, and the
departure rows wrap at 420 wide on both, as `QUEUE.md` §3 noted for macOS.

**The pinned font (r3).** A pixel fixture that must match across machines
needs the same font bytes on each, so `scripts/fixtures/fonts/` holds
DejaVu Sans 2.37 Book and Bold as Debian ships them (the builders'
`fonts-dejavu-core 2.37-8`), subset to the Latin blocks the app can paint
(168 + 154 KB; `SOURCE.md` there has the hashes and the exact fonttools
command; the Bitstream Vera license beside them). `scripts/agent.mjs`
launches this host with `EXACT_FONTS` at that directory and
`EXACT_FONT="DejaVu Sans"` unless the environment or a caller's `env` says
otherwise, so a picture taken through the driver is the same picture on a
Mac and on a builder; the app on a real box still uses what it finds. The
subset paints identically to the full font (0 pixels differ over the whole
app through the CPU painter — hinting and GPOS kerning kept). The smoke's
canvas reference for this host is the CPU oracle's picture over the pinned
font: exact on every machine, where the GPU painter lands within the band
(0.87% of the crop beyond 8/255 on Metal) and is held to the oracle by
`tests/paint.rs` instead. The host's own tests pin the same font (`pin_font`
in `tests/{host,paint,text}.rs`, set once per process), so their numbers
are one machine's on every machine.

**A requested weight is snapped to the family's face before shaping**
(`snap_weight`, r3). cosmic-text's fallback takes the requested weight
literally and ranks any face whose variable `wght` axis covers it above
the family's nearest static face: on a Mac, weight 500 and 600 came out in
San Francisco while 400 and 700 were the pinned DejaVu — the app's
weight-600 button measured 104 wide here against 128 on a builder with the
same font bytes, and the station's name fit one line here and wrapped to
two there (the test that caught it). The engine now asks `fontdb::Query`
— CSS font matching — for the `sans-serif` face at the requested weight
and shapes with that face's weight (600 → Bold, 500 → Book), so the family
stays first, the browser's rule. `tests/text.rs` holds it against the
fonts' own advances (Book 98.6, Bold 111.1 for "Change station" at 13 pt).

## 4. The presenter (`host/linux/src/presenter.rs`, `image.rs`)

**Router projection (LLP 1038 D6/D7/D11, 2026-09-14).** At boot and after
commits the host drains `take_router_change()` and retains the last op; no
foreign batch consumer is added. Launch is deliberately `/`; the agent's
`navigation` remains `{"unavailable":true}`. `route_visibility` projects every
container carrying `navigationBack` over its direct keyed children. The selected
route is visible and interactive; its immediate predecessor is visible but inert
when the selected presentation is `modal`; all others are hidden and inert.
A key matching no route preserves projection and journals once per key.
Hidden route subtrees produce no pixels or hit boxes; inert ancestors refuse
input. `layout <node>` reports `visible.hidden` and `visible.inert` even when
a retained route has no painted box.

The headless/DRM host has no system clipboard. `copyText(text)` is recognized
and reports `unsupported` on stderr; it neither saves a pretend clipboard nor
adds an agent operation (2026-09-10; Apple/web behavior: LLP 1008 §5, 1007 §4).

**Closed popovers** keep their inspectable logical tree but are hidden and inert
on Linux (2026-09-20). Linux has no top-layer popover presenter: tapping an invoker
reports unsupported before its accompanying application action runs; pointer
activation logs the same refusal, and retained/direct Host dispatch cannot bypass
it. This prevents closed Messages confirmations from painting over or intercepting
the inbox. The kernel still lays out their logical boxes, so a normal-flow popover
can reserve space; Messages uses absolute containers. Opening, placement, light
dismissal and removal from normal flow remain unimplemented. This is an honest
capability boundary, not completion of LLP 1021's presentation proposal.

What a painter holds beyond the kernel, and the operations that touch it.
**Scroll offsets** are host state per scroll container, clamped after
every layout to the content extent (LLP 1010's floor, ported); the page's
offset is the viewport over the document. **A wheel** (`wheel_at`; the
web's sign) goes to the innermost scroll container under the point that
can take its dominant axis, which takes what it can of both axes; else up
the ancestors; else the page — LLP 1010 §3's chaining rule. **A press**
(`press_at`) is a hit at a point — the last painted box containing it,
through every clip — then up to the nearest node with a `press` handler,
the path a click takes in a browser; focus follows the click: an input
takes it, anything else drops it. **Typing** (`type_text`) focuses the
input and dispatches one `change` with the whole value; a key in display
mode appends or backspaces the current value and dispatches likewise.
Changed finite `scrollTop`/`scrollLeft` requests on ordinary containers apply
once after layout, clamped to their content bounds, including `overflow: hidden`.
An unchanged binding leaves a reader's offset alone. `scrollFollowEnd` follows
growth and resize only while already at the end (within one point); enabling it
starts at the end, and an explicit request wins in the same commit. On a display,
the existing picture receipt owns these model offsets until acknowledged; reader
input retires stale intent. Programmatic scroll callbacks coalesce in change order
and report applied/acknowledged positions on the next pump. Hidden overflow permits
programmatic scrolling but still refuses wheel input. The headless Messages drive
covers 215 sends, a bounded 200-row transcript, earlier/later shifts and return to
latest; it does not establish physical display cadence or fix the separate hidden
confirmation/context-menu input gaps (2026-09-20).

**`clock`** advances the runner, seeks the engine to where it landed, and
reports both. **Images** (`image.rs`): after every commit the presenter
syncs every image node's `imageSource` — a relative path resolves under
`EXACT_ASSETS` (the current directory otherwise) and must stay inside it;
`http(s)` and any other scheme do not load (the out-of-process resource is
ibex2's, `QUEUE.md` §Later) — and decodes PNG on a thread; a completion
for an older generation or a gone node is dropped; the size reaches the
kernel through `set_intrinsic` and the tree relays out (LLP 1011). In the
headless modes the presenter waits for loads in flight (bounded, 500 ms)
before the first frame, so a screenshot and an agent's first `layout` have
the pictures in them. A **reload** (`EXACT_DEV_PLAN`, display mode) boots
the new plan with state carried (`Runner::carry`) and starts scroll,
focus, and pictures over, as LLP 1007 §6 says.

## 5. The agent API (`host/linux/src/agent.rs`) and the driver

Delivery synchronization runs presenter commit work only when the store's facts
change (LLP 1030 D7). Repeated first-pixel checks and idle agent reads leave a
clean presenter clean; they do not request a repaint of the same picture.
Successful activation still publishes the selected generation after its frame.

`EXACT_AGENT=1` is `Agent.swift`'s protocol on stdio, line for line: the
ready line, then JSON requests in and replies out. `tree`, `state`, `logs`,
and `settle` go to the host (`exact_runner::agent`); `layout` is the
painted boxes in ascending id with `sx`/`sy` on scroll containers, two
decimals; `tap` presses at the box's centre through `press_at` (a wheel
with `wheel: [dx, dy]`); `type` replies `typed` and `value`; `clock` is
LLP 1012 §2's fixed point — advance, seek, `settled: true` when nothing is
in flight, sixteen rounds then `settled: false`, a backwards time refused,
a timer's refusal returned with where the clock landed; `screenshot` is
the frame as a PNG (viewport points in the reply, not pixels). Pending
image loads are drained before every request. `scripts/agent.mjs` gained
the third carrier — `openStdio`, the macOS carrier with the binary and
the size in the environment (`EXACT_LINUX_BIN`, `EXACT_SIZE`) — so
`node scripts/agent.mjs linux …` and `node scripts/smoke.mjs linux` run
wherever `target/release/caltrain-linux` was built, macOS included. The
smoke's canvas reference picture (`scripts/fixtures/canvas-sky.linux.png`)
was recorded on the builder, where the fonts are DejaVu's; on a Mac the
`linux` smoke fails that one step by the font, not the painter (§7).

## 6. The display (`host/linux/src/display.rs`, `input.rs`; `app.rs`)

`exact_linux::run::<Data>(PLAN)` reads the environment once (`app.rs`
lists it) and runs one of three ways. **Headless** — `EXACT_AGENT=1`,
`EXACT_SMOKE=1`, or `EXACT_SHOT=<png>`, and always on macOS — boots under
`EXACT_SIZE` (420×860 by default) at `EXACT_SCALE` and serves, prints the
phases, or writes the frame. **The display** (Linux, otherwise) opens
`EXACT_DRM` (`/dev/dri/card0`), asks for master, takes the first connected
connector at its preferred mode and the CRTC behind its encoder, creates
two XRGB8888 dumb buffers with framebuffers, shows the first frame with
`set_crtc` and every later one by a page flip whose event it waits for —
so the loop is paced to the display's refresh and frames are painted only
when something changed. Input is evdev read directly: every device with
relative axes or a keyboard's keys, non-blocking, polled with the timers;
pointer motion (device pixels over the scale), the primary button (a
press on release inside the same box as the down), wheels (coarse and
high-resolution, folded), keys through a US keymap into the focused
input. Timers fire every 250 ms while the runner has any; motion frames
run while the engine is not quiescent; image loads and the dev plan are
polled at 100 ms. `apps/caltrain/linux` is the app: `build.rs` bakes the
plan (the Apple crate's), `main.rs` is three lines; `cargo build --release
-p caltrain-linux` is the build — **20 s cold for the whole tree on 96
cores, 7–11 s to relink after a host edit**.

**Exercised:** the whole headless path on three machines (§8): the GPU
painter on Metal (this Mac) and on Vulkan/RADV (the minisforum's Radeon
890M, once its user was in `render`), the CPU painter everywhere. **The
DRM path ran on the minisforum on 2026-08-29** — Charlie stopped `gdm3`
and added the user to `render` and `input` — from an ssh session, no VT:
`EXACT_DRM=/dev/dri/card1 EXACT_SCALE=1.5 EXACT_ASSETS=apps/caltrain
target/release/caltrain-linux` took master (nobody held it), set
`HDMI-A-1` to 1920×1080 @ 60 Hz, and has been flipping frames as the
countdown timers tick: **boot to the first flip 155–164 ms on the GPU
painter** (device 23 ms + shaders 6 ms from the pipeline cache, fonts 8 ms,
runner + layout 17 ms, the first 1920×1080 frame 12.6 ms — render 7.6,
readback 4.8 — and the rest the mode set). RADV's first launch ever paid
93 ms for the device and 77 ms for the shaders. Declared: the one input
device on that box is its i8042 keyboard controller — the KVM's HID is
not on its USB — so evdev has carried no real event yet, and absolute
pointers (`ABS_X`/`ABS_Y`, what a KVM's mouse reports) were added on the
strength of the code alone; the KVM on that HDMI was unplugged.

**The screen over VNC** (`vnc.rs`, `EXACT_VNC=1`, Charlie's ask the same
day when the KVM turned out to be unplugged): the display loop publishes
every frame it presents, and a small RFB server — protocol 3.3/3.7/3.8 as
the client speaks it, `Raw` encoding, the client's 32-bit pixel format
honoured, one reader and one writer thread per client — serves it on port
5900 and
feeds the client's pointer, buttons, wheel, and keys back into the loop
through the same `InputEvent`s evdev produces, over a socketpair the loop
polls. It is a development tool on a private network and encrypts
nothing. This is how the first DRM run was seen: a scripted client took
the 1920×1080 frame the box was presenting, clicked "Change station" at
its pixel, and took the stations screen — the display path and the input
path both exercised, from a Mac, with no KVM. macOS Screen Sharing
(`vnc://<host>:5900`) is the everyday client, and it taught the server two
things through a logging proxy: it answers a 3.8 server with **3.3**, and
in 3.3 it will not proceed past security `None` — it wants VNC
authentication, so a 3.3 client is sent a challenge that any password
answers (nobody is authenticated either way; `EXACT_VNC` is for a private
network), while 3.7/3.8 clients get `None`.

**A kernel fix this run found** (`kernel/src/arena.rs`): a `TextInput`'s
measured runs were its `value` else its `placeholder`, and the runner
sets `value=""` — `Some("")` short-circuited, the input measured an empty
run, and a measurer that gives empty text no line box (this host's; the
web's rule for a `<div>`) laid the search field out 26 pt tall with its
placeholder over the row below. An `<input>` always has a line box; here
it is given one by measuring a space — a declared deviation from the
browser, whose empty `<input>` gets its line box from the font's metrics
with no advance (the space's width shows only in a shrink-to-fit input's
min-content, and nothing paints it). The runs are now the value when
non-empty, else the placeholder, else one
space — 380×45 here against Chrome's 199×42 (the width is the block's;
Chrome's is `size=20`).

## 7. Not in v1 (and the trades taken)

`inert` subtree input/focus enforcement is not implemented in this host. The
compiler's existing-boolean admission and iOS/browser repair do not establish
Linux support (LLP 1035.001 D3).

**The painter is vello on the GPU (r2), with tiny-skia on the CPU as the
fallback and the pixel oracle.** r1 took CPU raster for the reasons
`QUEUE.md` §Open decisions named — boots with nothing compiled, runs on
the GPU-less fleet, deterministic pixels — and Charlie reversed it the
same day: the GPU is the main painter going forward, because it is what
makes `canvas` and LLP 1014's children-through-the-shader one pass rather
than a readback, and because the display that matters is a GPU's. The
costs, measured: **shader compilation on the boot path** — 773 ms on the
first launch on this Mac (Metal), 9.8 ms on every later one from the
driver's cache; on Vulkan the wgpu pipeline cache on disk plays that role
(on RADV: 77 ms the first launch on the machine, 6 ms from the cache after — §6) — which is the "boot path compiles nothing"
rule not met, and the amendment LLP 1009 §5 already proposes ("compiles
nothing on the boot path; a canvas compiles its shaders at first use")
now needs a second clause for this host: *the GPU painter compiles its
shaders on the first launch on a machine and reads them from the cache
after*. And **the readback**: 4 ms to render and 17–19 ms to wait and map
per frame on Metal, at 420×860 and at 840×1720 alike — latency, not
bandwidth — against 3.8–6.9 ms for the CPU painter; on RADV the same
readback is 3–5 ms and a 1920×1080 frame is 12.6 ms against the CPU's
10.2 ms. Until the frame is
presented from the GPU (a KMS surface through `VK_KHR_display`, the
follow-up), the CPU backend paints the small pictures faster; the GPU
backend is right for the display, wrong for the readback that is its
only path today. Nothing else in the host knows which backend paints;
`EXACT_PAINTER=cpu` is one environment variable away. The two-wgpu build
(vello 0.10 pins 29, the GPU module 30) ends when vello moves.

Also not in v1, each declared: authentication or encryption on the VNC
server (`EXACT_VNC` is for a private network), and its `Raw` encoding
only (8 MB a frame at 1080p — a LAN's, not a WAN's); a KMS surface for the GPU (`VK_KHR_display`
/ `VK_EXT_acquire_drm_display`, no readback); the GPU module on this
painter's device (`canvas`); libinput and xkbcommon (pointer
acceleration, touchpad gestures, hotplug, non-US keymaps — evdev reads
raw, and nothing links); Wayland or X11 windows (DRM or headless only);
a cursor blink, selection, IME; `text_decoration`, `font_family` (always
sans-serif), RTL untested; shadows, gradients, grid (as on macOS); JPEG
and other image formats (PNG only), image URLs (the executor of §1 exists;
the image loader does not ask it yet); `symbol:` images and symbol tint
(LLP 1035.004: declared boxes paint no symbol until a Linux consumer earns the
same schema paths in the painter); accessibility of
any kind; HiDPI beyond `EXACT_SCALE`; a font cache for the scan;
pixel fixtures against Chrome (the instrument exists — `screenshot`, the
smoke's canvas reference, and the pinned font, §3 — the comparison itself
is not made); the page extent past the root's frame (LLP 1010
§3's deviation, shared); lifting the orchestration out of `host/apple`
and `host/linux` into one crate.

## 8. Checks that hold this

`host/linux/tests/host.rs` (11): the tree lays out with real text and
every node has a painted box; `layout` is the agent API's shape; a press
goes through hit-testing and bubbles to the handler; typing replaces the
value and the runner hears one change; a wheel scrolls the page when the
inner container cannot; a nested scroll container takes the wheel then
chains to the page and clips what it scrolled out; a spring arrives as
presentation values frame by frame and settles at 1.5 / 0.5; the clock
fires timers at their due times; an image lays out from its decoded size
and never resolves outside the asset root; every agent request answers on
the wire; a reload carries state and starts the pictures over.
`host/linux/tests/text.rs` (1): a weight the family lacks resolves within
the family — 500 is Book and 600 is Bold, at the fonts' own advances.
`host/linux/tests/paint.rs` (7, each run under both painters where a GPU
exists, under the CPU alone where none does — said, never failed on),
pixel by pixel: backgrounds land in their boxes with their radii (`#eeeeee` inside the app's button, the page at its
corner; `#f7f7f7` inside an inline fixture's card past the 16 pt radius —
the app's own panels became translucent white over the sky in LLP 1014
§1a, which a review caught as a stale assertion); **the two painters agree
within a band** over the whole app at 390×844 — measured on Metal
2026-08-29 at mean 3.24/255 with 2.98% of pixels differing by more than 32
with the Mac's Helvetica, and 1.35/255 with 0.73% once the tests pinned
DejaVu (2026-08-30), asserted at 5 and 6% (glyphs are where they part: vello draws hinted
outlines at the layout's positions, tiny-skia blits swash's bitmaps at
snapped ones — a declared deviation, the band its measure); text and images
leave ink in their boxes and none between; motion presents as a transform
and a group opacity (the box 1.5× wider, the ink lighter); a scroll
container clips what it scrolled out; a device scale of 2 paints 780×1688
for the same 390×844 points; a screenshot is the viewport as a PNG.
`node scripts/smoke.mjs linux`: ok on `expo-build-1000` (Ubuntu 24.04,
Rust 1.97.0, the CPU painter) in 0.9 s and on macOS 26.6 (the GPU painter
on Metal) in 7 s (the canvas reference step aside, §5), 2026-08-29; the
peer lane's motion step (LLP 1012 r2's fixture) gives the same numbers as
the other hosts. Under the five checks the same day.

**`layout <node>`** (2026-09-09, LLP 1035.002 D1): `layout` with an `id`
adds `node` — the runner's rows and sources (`Host::agent`'s `node`
message) plus what a painter knows: the painted box as `space.viewport`
and a 1:1 capture scale. No window, no screen, no scroll or clip chain
yet, and `native` is `{"unavailable": true}` rather than a guess; the
inherited colour and font rows the painter now reads through
`NodeRef::computed_style` (LLP 1035.000) are what it reports.

**`state`** (2026-09-10, LLP 1035.002 D2): the painter appends `focus`,
`keyboard` and `navigation` as `{"unavailable": true}` each — present, so a
reader can tell "no keyboard" from "no report" — and tags every reply it
answers itself (`layout`, `tap`, `type`, `clock`, `screenshot`) with the
runner's `epoch`/`incarnation`/`clock` through its `tags` message (D3,
`agent::tagged`).
