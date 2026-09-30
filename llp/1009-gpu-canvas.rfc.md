# LLP 1009: The GPU canvas

**Type:** RFC
**Status:** Review (super-refine loop 2026-08-29: two rounds on r2, both families NOT READY on in-delta findings only — dispositions in `llp/reviews/1009-gpu-canvas.{codex,grok}.md`; r4 is Charlie's requested simplification; round 3 reviewed r4 at his request — both families NOT READY, neither on architecture (codex: "the core direction … is feasible"); r5 folds round 3 and is unreviewed. `rules/DEFERRED.md` §Process forbids refine loops; this one ran on his in-session instruction, and the trades it owes are in §5.)
**Systems:** Kernel (node type), Contract (tag), Plan (surface row), Runner (surface arguments), GPU module (new), Apple host, Web host
**Author:** Claude (Fable 5) for Charlie Cheever
**Date:** 2026-08-29
**Revised:** 2026-09-29 (D7 accepted and landed — one submit per tick: `Surface::render` records into the module's encoder, `gpu_flush` submits once and presents; `Surface::submitted` for post-submit maps. §3 has the trace and macOS's off-main acquisition.) 2026-09-23 (D6 — an app may declare GPU modules beside the primary, each loaded the first time a canvas of one of its surfaces mounts; built for Weird Castle's title sky and engine demo, recorded in LLP 1046.003.) 2026-08-29 (r5 — round-3 fold, unreviewed: GPU code lives in the app's GPU crate, never the host-linked data crate; `bind` returns a result and surfaces register an arity; surface arguments are evaluated with the node's bindings before apply and published as a runner side-output only after a successful commit; a bare `canvas` is 300×150 by tag default — the web's size, no deviation; fixtures read back from a module-owned copyable texture; D5 narrowed to the build-declared shader set; the minimal presentation-value extension point decided; the loader is a post-paint injected script element; §5 proposes one concrete take.) 2026-08-29 (r4 — cut to the five decisions that matter, at Charlie's request: wgpu is the one API on every host; the module is on demand; shaders are validated at build and compiled at first use, off the boot path; extensible properties are a later RFC. r3 carried an exact-owned handle, a profile table, a shader catalogue, and declared properties — machinery that answered review findings by adding rather than removing; superseded by this text.) r3, r2, r1: see the review artifacts.
**Related:** `rules/DEFERRED.md` §Runtime (the "door stays open" clause; this RFC walks through it) and §Components (`canvas`; §5 records the trade), LLP 1000 (the map), LLP 1001 (`NativeView`; layout is a host call), LLP 1002 (one representation, two executors; the browser as oracle), LLP 1004 D4 (app computation is a Rust data crate), LLP 1007/1008 (the hosts), LLP 1008 §6 (startup: nothing GPU joins the boot path)

## Summary

An app draws with the GPU through a **`canvas`** node: a leaf with a
kernel-owned box, whose content the app renders in **Rust against wgpu** —
the same code on macOS, Linux, and the web. That code lives in a
**separate module per app**, loaded the first time a canvas is on screen
and after the first pixel, so an app without a canvas carries nothing and
an app with one boots exactly as fast. The host owns the frame. The
browser is the oracle: a readback fixture holds native to Chrome. That is
the whole design; everything else is the spec's.

## 1. Decisions

**D1 — wgpu is the one GPU API, on every host.** App rendering code is
Rust in the app's **GPU crate** — a sibling of its data crate (LLP 1004
D4), and the only crate that depends on wgpu; the host-linked data crate
never does — written against wgpu's own types: on Apple and Linux wgpu runs on Metal and Vulkan; on the web wgpu
runs on the browser's WebGPU through its WebGPU backend. WebGPU's model
is therefore the representation and the browser is one of its two
executors, LLP 1002's shape — and we own no API of our own. The parity
instrument is a **readback**: the same Rust rendering the same frame on
each host, compared to Chrome within a per-fixture band (bit-exact for
opaque geometry; a declared band where blending or precision differ).
Not chosen: an exact-owned WebGPU-shaped handle with two implementations
(r2/r3). It avoided wasm-bindgen in the web host, but the module loads
after first pixel, where the boot rule does not reach, and it would have
been a second WebGPU surface to keep in step with the first.

**D2 — The GPU is a separate module per app, loaded on demand.** (Or
several, each loaded by its own surfaces: D6.)
`<app>-gpu` is a second artifact: wgpu plus the app's surfaces behind a
small C ABI (`gpu_load`, `gpu_create`, `gpu_bind`, `gpu_render`,
`gpu_destroy`, `gpu_readback`; `gpu_flush` since D7) — a `dylib` in the bundle on Apple, loaded
with `dlopen`; on the web a second wasm with its generated glue, fetched
when needed. The core hosts stay as they are (`#![deny(unsafe_code)]`,
no wasm-bindgen); the module holds the two audited `unsafe` boundaries
(the Metal layer handoff, `dlopen`). The app implements one trait (as first written; D7 gives `render` the module's encoder):

```rust
pub trait Surface {
    /// The canvas's inputs from the plan, as typed values; before the
    /// first render and whenever they change. A refusal names the input.
    fn bind(&mut self, inputs: &[Value]) -> Result<(), SurfaceError>;
    /// One frame into `target`. Returns whether another is wanted. The
    /// clock in `frame` is the host's presentable clock — deterministic
    /// under the agent's `clock`; a surface that samples wall time is not.
    fn render(&mut self, frame: &Frame, device: &wgpu::Device, queue: &wgpu::Queue, target: &wgpu::TextureView) -> bool;
}
```

and registers each surface in the module by **name and arity**; the
compiler checks a `canvas`'s argument count against a roster the GPU
crate's build emits (types are checked at `bind`, as a resource's are at
its shape). wgpu objects never cross the module's ABI: it carries encoded
values, fixed-width handles, and the target pointer. **The runner's
half:** a canvas's arguments are evaluated with the node's other bindings
— per realized node, before the kernel applies the commit, so a trap
refuses the commit whole as any binding's does — and published as a
**runner side-output** after a successful apply (and at boot), never as a
kernel op; a refused commit publishes nothing. The host puts them on its
batch as a `surface` op (node, name, values); the presenter hands them to
the module, queuing them until it is loaded. Instances are per canvas
node, created when the module is up, dropped with the node, re-created
on a plan reload. Failures are reported per canvas, never as a boot
refusal — a name the module lacks, a `bind` refusal, a module that fails
to load, an adapter or device that cannot be created; the box keeps
painting its background. Device loss (the platform's) drops every
instance's device-owned state: the module re-creates the device and
re-creates every instance, whose next `bind` and `render` rebuild what
they own.

**D3 — `Canvas` is a leaf node with a kernel-owned box.** `schema.json`
gains node type `Canvas`, sized by its style rows exactly as `NativeView`
is: never measured, content never influencing layout. A bare `<canvas>`
is 300×150 on the web, so the `canvas` tag's defaults are `width=300
height=150` — the web's size, by the tag table, no deviation. Contract
gains `canvas surface=map(location, trains) width="100%" height=240`; the compiler lowers `surface=` to a `surfaces` row — the
name and the argument expressions — that is not a resource: not settled,
not baked, no boot value. The drawable's pixel size is the host's (the
layer's scale on Apple; the element's content box and `devicePixelRatio`
on the web) and travels in `Frame`.

**D4 — The host owns the frame, visibility, and the target; the device
comes after first pixel.** A canvas renders when the host judges it on
screen and it has something to render — new inputs, a wanted frame, a
new size, or the device just became ready — from the host's existing
frame source (the display link on Apple, `requestAnimationFrame` on the
web) under a third batch signal, `canvas`, beside `timers` and `motion`;
nothing runs per frame otherwise. The presenter registers the platform
target (a `CAMetalLayer`, a `<canvas>` element) with the module and
unregisters it on destroy. The first canvas on screen starts the module
load and device creation **after** the frame that painted its box — on
the web the glue injects the module's `<script>` element from the
animation frame after the paint stamp (never an `import`, which the
`boot` check counts), and the smoke asserts the element is absent before
that stamp; the time from then to the first rendered frame is a reported
startup phase. The claim is exact: no GPU fetch, initialization, device,
or shader work is on the pre-pixel path. The device outlives plan
reloads; surfaces do not. Readback is asynchronous, for fixtures, not
the agent's screenshot: a fixture renders into a **module-owned texture**
with `COPY_SRC` (a presented surface texture need not be copyable) and
reads that back.

**D5 — Shaders are validated at build and compiled at first use, off the
boot path.** The WGSL files under the GPU crate's `shaders/` directory
are checked by naga in its `build.rs`; a bad one fails the build with its
line and column. That is the build-declared set; a surface holds a raw
`wgpu::Device` and may also compile a string at runtime, which the build
cannot see and does not claim to — wgpu validates it then, and it fails
that surface, not the app. At runtime wgpu compiles it on first use — which by D4 is after
the first pixel, so the measured 115 ms cold cost (§3) lands on a canvas's
first frame and never on boot. `rules/DEFERRED.md` says the built door
"compiles no shaders at runtime"; this decision does not meet that
wording and asks for it to be amended (§5) rather than met with a
precompilation pipeline whose feasibility with bind groups is unproven.
Precompilation is a later, measured trade.

**D6 — More than one artifact, each loaded by the surface names it
registers** (amended 2026-09-23, the split lane; LLP 1046.003 records it).
D2's one module per app makes every screen pay for the heaviest surface:
Weird Castle's title sky shipped inside its game engine. So an app may
declare **GPU modules** beside the primary `<app>-gpu`: `app.json`'s
`gpu.modules` maps a module name to the surface names it registers —
`"gpu": {"modules": {"world": ["world"]}}` — and module `m` is the crate
`<app>-gpu-m` in the app's workspace, with the primary's ABI and
`module!` registry (an engine world is `exact_game_render::module!`
unchanged). Every surface a module does not list is the primary's, and
the primary is optional. A surface listed twice, a malformed name, and
modules on a game app (whose one module is generated) are refused when
the manifest is read. The bake binds the declaration into the
compatibility inputs as `gpuModules` — which artifact owns which surface
is part of the binary — and natively one signed digest per module beside
`embedded.gpu`, as `embedded.gpuModules.<m>`, which the loader checks
before opening it, as it checks the primary's. Each host keeps the
artifacts apart and routes by name: the first canvas of an artifact's
surface loads that artifact, after the frame that mounted it (D4, per
artifact); each artifact has its own device, so its own loss and
recovery; everything addressed to a canvas goes to the artifact that
created it; process-wide signals (display period, lifecycle, a lost
device's notice) reach every loaded artifact, which answers for itself,
and agent replies that list worlds are joined. On the web a module is
`gpu/<m>.js` and `gpu/<m>_bg.wasm`, and `gpu-modules.js` — loaded instead
of `gpu-glue.js` only when modules are declared — is `exact.gpu` and
imports one `gpu-glue.js?artifact=<stem>` instance per artifact (a URL is
one module instance); natively a module is `libexact_gpu_<m>.dylib`
beside the primary (Frameworks on iOS), and on Linux its Cargo product
beside the binary. The dev server rebuilds and swaps only the artifacts
an edit's files reach. An app that declares none bakes, ships and loads
exactly as before: no key, no router, no extra file. Runtime shader
assets (LLP 1030 D8) stay the primary's — the native ABI does not name a
registry's shaders — so a module compiles the WGSL it carries, as the
engine's renderer does. Not chosen: always separating the engine and
loading it with the first world canvas. It is not smaller — every host
needs the same per-artifact loading, routing and recovery — and it would
teach the core what a world is, which `game/` keeps out of it.

**D7 — One submit per tick: canvases record into the module's encoder**
(amended 2026-09-29, accepted and landed; Charlie approved changing the
trait on 2026-09-28, and on 2026-09-29, after the lane first recorded it as
held for its small saving: "breaking weird castle and other apps is fine.
focus on optimizing and ideal end states rather than a smooth journey
there." Out-of-repo surfaces follow; Weird Castle's migration is a patch
kept with the lane.)
D2's `render` gave each surface the queue, and each surface submitted its
own frame. On Metal that is three command buffers per canvas per frame —
wgpu's pending writes (the surface's `write_buffer`), the render, and
wgpu's present — each committed on the main thread and each retired by
Metal's submission and completion threads. On the M1 iPad Pro, with two or
three shader rows of the Extra Heavy feed at 120 Hz, that was about
120 ms/s on Metal's threads and 75 ms/s of commits on the main thread,
where SwiftUI's `layerEffect` rows cost the app nothing (§3 has the trace
and what landing it measured).
So the trait changes:

```rust
fn render(&mut self, frame: &Frame, device: &wgpu::Device, queue: &wgpu::Queue,
          encoder: &mut wgpu::CommandEncoder, target: &wgpu::TextureView,
          format: wgpu::TextureFormat) -> bool;
fn submitted(&mut self) {}
```

The encoder is the module's. `gpu_render` records a canvas into it and
shows nothing; a new export, `gpu_flush`, finishes it, submits it once,
then presents each canvas's drawable in the order rendered. Each host's
tick renders every canvas it judges on screen and dirty (D4, unchanged),
then flushes each module it rendered into — so per module per tick one
pending-writes buffer, one frame buffer and one present per canvas: two
plus N command buffers where there were 3N. A surface never submits.

- **Pending writes.** `queue.write_buffer`/`write_texture` in `render` go
  to wgpu's staging as before and land in the one submit, ahead of every
  command in it. For what an instance owns that is exactly what its own
  submit did; the rule the trait states is that a surface does not rewrite,
  mid-tick, a resource another canvas's already-recorded commands read — a
  shared cache may add textures (the shader row's photos) but not overwrite
  one in use. Copies that must precede a write are encoded, not written.
- **Ordering.** Commands run in render order, then the presents. Any other
  call about a canvas already in the open frame — bind, input, agent, an
  asset, restore, its children or child textures, destroy — flushes first,
  and so does a second render of it, `gpu_sync`, a readback, a shader
  registration and a clock change: whatever a host observes about a canvas
  is what it observed when each render submitted. A readback and the
  fixture render into their own encoder and submit it.
- **A canvas not drawing this tick** records nothing and is not in the
  frame: off screen, clean, or — iOS — without a drawable yet. A frame with
  no canvases is not submitted; `gpu_flush` with nothing open is a no-op.
- **iOS off-main acquisition** (`gpu/src/acquire.rs`) is unchanged in
  shape: a canvas whose drawable has not arrived is starved and records
  nothing; the next drawable is requested after its present, which is now
  in the flush. Drawables arriving at different times give the late
  canvases a second, smaller submit: `gpu_on_acquire` renders the starved
  canvases and flushes once for them.
- **Web.** WebGPU presents a canvas when the task that rendered it ends.
  Before this, each canvas's `render` was its own `queue.submit` — N
  submits a frame, not one. The glue flushes at the end of its animation
  frame, after a resize observer's render, in the agent's settle loop and
  after a reload's staged renders — each in the task that rendered.
  `queue.writeBuffer` is ordered on the queue timeline as native staging
  is, so the same rule holds.
- **Linux.** The Linux host presents no module canvas (a platform target is
  refused off Apple, LLP 1015 §7; the host loads modules headless for their
  records and agent), and its own painter — vello on wgpu, tiny-skia as the
  oracle — is not a `Surface`: neither changes. The fixture and readback
  paths, which Linux tests use, submit their own encoder.
- **Errors and a lost device.** A surface that reports a failure after
  recording keeps its commands in the frame — they name its drawable, so it
  is kept alive until the submit, then discarded, not presented. A wgpu
  validation error while recording now invalidates the whole frame rather
  than one canvas's submit; natively wgpu's default handler already aborts
  on one, so this does not change what a broken surface does. A device found
  lost at a render or a flush drops the open frame unsubmitted before the
  presentations go, and recovery (D2) proceeds as before.
- **After the submit** the module calls `submitted` on each canvas in the
  frame: a surface that copied something for reading maps it there (a map
  cannot precede the submit of its copy) — the engine's culling counts and
  pass timings.

Not chosen: an encoder per canvas submitted together (`queue.submit` of N
buffers) keeps a validation error to one canvas but commits N frame buffers,
2N + 1 in all; and folding the presents into the frame's command buffer
needs the drawable, which wgpu keeps private to its surface texture.

**Not taken: small uniforms as immediates** (measured 2026-09-29, branch
`perf/gpu-immediates-on-d7`). Redeclaring a surface's small `var<uniform>`
as `var<immediate>` where the device grants it removes `write_buffer`'s
per-write staging buffer (about 60 µs of the iPad's main thread a frame),
and alone it saved about 19 ms/s on the iPad shader feed. On top of D7 it
saved nothing measurable: wgpu 30's Metal backend sets immediates for every
stage (`setObjectBytes`, `setMeshBytes`, `setVertexBytes`,
`setFragmentBytes` — about 8 ms/s) and zero-fills them again on each
pipeline change (about 8 ms/s), which is what the staging cost. It returns
if wgpu sets only the stages that read them.

**A per-frame uniform is written in place** (2026-09-29, with D7).
`exact_gpu::FrameUniform` is what a surface writes each frame instead of
`queue.write_buffer`: on Apple a ring of four shared-storage buffers
imported into wgpu as uniforms, the frame writing the next slot's memory and
binding that slot's bind group — a canvas holds at most three drawables, so
the slot written was last read by a frame whose drawable has come back, its
commands complete; elsewhere one buffer written through the queue. No
staging buffer per write, and no pending-writes command buffer in the tick
of a surface that writes only this. Caltrain's aurora, glass and stack and
Weatherlight use it; the engine's per-frame buffers still write through the
queue.

**The extension point for animatable properties** (the DEFERRED clause
"animatable properties are extensible"): committed state is not
presentation state (LLP 1002), so "a property is more state" would not
deliver a display-rate value to a surface. The minimal extension point
is decided here and built when a surface needs it: `exact_motion::Property`
gains `Custom(u16)`, the plan declares custom properties in a table, the
engine transitions them as it does `opacity`, and their presentation
values reach `bind` as trailing inputs each frame they change — on every
host through `exact-motion`, since no such property maps to CSS. The
authoring syntax and the web-side sampling are the later RFC's.

**Not decided here, deliberately:** A 2D vector layer
(vello-class; adopted, not built, as a `Surface`). A GPU paint stage for
ordinary subtrees. A GPU-drawn presenter. Compute-only surfaces. Video
and camera. The Linux host's use of this device is that lane's decision.

## 2. Sequencing (web first)

1. The seam and the node: `Surface`, the `surfaces` row, `Canvas` in the
   schema and tag table, build-time naga validation, headless tests with
   a recording surface.
2. The web host: the on-demand module, a `<canvas>` per node, the
   `canvas` signal, readback; the Caltrain line map; the fixture recorded
   from Chrome.
3. The Apple host: the `dylib`, the `CAMetalLayer` target, the display
   link, device-after-first-pixel with its phase reported; the fixture
   held natively.

The spec (1009.000) transcribes the landing.

## 3. Costs, measured 2026-08-29

- **On the web** the module is wgpu's WebGPU backend plus generated glue.
  Built (step 2, 2026-08-29): the Caltrain module with the line map is
  **142 KiB / 70 KiB gzip** after wasm-bindgen and `wasm-opt`, plus 47 KiB
  of generated `gpu.js`; the device is up **~94 ms** after the loader is
  injected, in headless Chrome. Fetched only by canvas apps, only after the
  first pixel; the Caltrain app wasm (173 KiB gzip) is untouched. (The
  scratch probe's 829 KiB was mostly wasm-bindgen's unprocessed export
  stubs — 169 KiB of it was code.)
- **Natively** the module is ~1.7 MiB stripped (Metal backend plus naga).
  Measured (step 3): `dlopen` + device + the first canvas's surface is
  **23–36 ms warm, ~260 ms on a cold first launch**, after the first
  paint; absent from apps without a canvas. The scratch crate and the commands
  are recorded in `llp/reviews/1009-gpu-canvas.codex.md` (round 3 asked;
  `scripts/probe/` when the GPU crate lands). The ~100-crate dependency
  joins `cargo build --workspace`: its cold compile is ~35 s here, its warm
  cost nothing — measured against the five checks' budgets in step 1.
- **Time:** adapter + device + one pipeline from WGSL + one draw on Metal
  is 11–13 ms warm, 115 ms cold; by D4 both land after the first pixel.
- **D6, measured 2026-09-23** (LLP 1046.003): Weird Castle's title
  module is 360 KiB (180 KiB gzip, `wasm-opt -Oz`) once its 751 KiB
  (321 KiB gzip) engine module is separate — from 883 KiB (398) for the
  one module; the title fetches half the bytes after first pixel.
  Natively each artifact carries its own wgpu: 3.34 + 3.92 MB stripped
  against 4.06 MB for one.
- **A dependency:** wgpu (v30, ~100 crates), pinned; the wasm-bindgen CLI
  in the web module's build. Reached through one module, so replacing
  either is one crate's change.

- **D7, traced 2026-09-28** (M1 iPad Pro, the Extra Heavy shader-only
  feed, `fling 0`, Time Profiler over the same 28 s window, 2–3 shader rows
  on screen at 120 Hz; `~/bench/xheavy/gpusubmit`). A submit per canvas:
  the canvases cost the main thread 95 ms/s (wgpu's `Queue::submit` 25,
  its encoding 26, `write_buffer` 22, the present 8, Metal's commits 9),
  and Metal's submission and completion threads 70 and 17 ms/s; the
  acquiring threads 51 ms/s in `nextDrawable`. One submit per tick
  (D7, alone): 75 ms/s on the main thread (encoding 22, submit 16, `write_buffer`
  16, present 8) and 57 and 13 on Metal's threads — 120 fps both.
- **D7, landed 2026-09-29** (the Extra Heavy feed, `fling 0`, three
  interleaved rounds each, medians, origin/main against D7): the M1 iPad
  Pro's shader-only feed 621 → 584 ms/s process CPU (main 332 → 319), the
  iPhone 13 Pro Max's 598 → 569 (main 322 → 310), both at 120 fps. The
  19-kind feed: iPad 111.6 → 115.0 fps, late frames 4.5 → 2.9 a second,
  24k pt/s 101.7 → 112.7 fps, CPU 658 → 668; iPhone 107.9 → 111.1 fps,
  late 6.9 → 5.2, 24k pt/s 93.0 → 103.7, CPU 663 → 657. GPU readbacks
  (Caltrain, Weatherlight, the engine's core and compressed suites, 100
  images) are byte-identical to origin/main.
- **At rest, 2026-09-29** (iPhone 13 Pro Max, the Extra Heavy feed,
  `rest 0`, three interleaved rounds each, medians). A GPU canvas that does
  not ask for a frame already records and submits nothing; at rest the
  canvases' work is the shader rows animating at 120 Hz, as SwiftUI's and
  UIKit's do (UIKit's `MTKView` draw is about 6 ms/s of its main thread per
  row). What was more than the drawing: `write_buffer`'s staging (about
  8 ms/s a row), `applicationState` read every tick and starved pass
  (3–12 ms/s), and one starved pass per landed drawable, each walking every
  canvas's ancestors again (`onScreen`, 24 ms/s with three rows). With
  `FrameUniform`, the state read once per change and one coalesced starved
  pass that trusts the tick's judgment: the 19-kind feed's main thread
  59 → 49 ms/s (CPU 108 → 93), the shader-only feed's 217 → 160
  (CPU 429 → 350), 120 fps and no late frames in both. Predicted from the
  traces: about 51 and 187 for the first two changes, which measured 52 and
  184 alone.
- **macOS, 2026-09-28/29** (the same feed at rest, five canvases on
  screen; `~/bench/xheavy/gpusubmit/mac`). With the drawable acquired on
  the main thread, 54% of the main thread's wall-clock samples were waiting
  in `-[CAMetalLayer nextDrawable]` — blocked, not CPU; acquired off it
  (`gpu/src/acquire.rs`, on macOS since 2026-09-29, as on iOS), none.
  Four interleaved 40 s runs each, medians: display-link frames late
  (> 1.5 periods) 2.2/s → 0.16/s, the longest gap 44–58 ms → 21–28 ms,
  each canvas presenting 117 → 119 times a second, main-thread CPU
  116 → 98 ms/s, process CPU 351 → 354 ms/s.

## 4. Open questions

1. Readback bands for blending and MSAA across Metal and Chrome —
   declared per fixture, as measured.
2. Whether the module's C ABI stays hand-written (as `exact.h` is) or is
   generated from a table — hand-written until a second consumer.

## 5. The DEFERRED trades this RFC owes (Charlie's, before Acceptance)

The rule: name what it unblocks, and take something off the doing-list
in the same PR.

- **§Runtime "GPU / WebGPU substrate" and §Components `canvas`** (one
  trade). Unblocks everything an app draws that is not a box or text —
  the Caltrain line map first. **Proposed take, in the same PR: the three
  gradient style rows** (`gradient_type`, `gradient_angle`,
  `gradient_colors`) leave `schema.json` — declared in v1, implemented by
  no host, and a canvas draws a gradient; they return when a host earns
  them. *(Applied 2026-08-29 under LLP 1014 §5, the same door widened.)*
  them. (Round 3 held that deferring iOS is a reorder and dropping the
  agent's screenshot an alias; both withdrawn.)
- **§Runtime, the wording "compiles no shaders at runtime"** — amend to
  "compiles nothing on the boot path; a canvas compiles its shaders at
  first use" (D5), or refuse, in which case D5 waits for a proven
  precompilation path and this RFC waits with it.
- **§Process: refine loops** (this loop). Unblocks building the canvas on
  text two families have read three times; the take is the same one as
  LLP 1004's, Charlie's to name.

## Ratification note

Review. Three rounds: two on r2, one on r4; r5 folds round 3 and is
unreviewed (see the artifacts). Charlie may say Accepted on this text,
apply §5, or ask for another round.
