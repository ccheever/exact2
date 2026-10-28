# LLP 1148: One renderer on every platform — the options

**Type:** Research
**Status:** Draft r3 (a record: it answers a question Charlie relayed on 2026-10-28 and sends its recommendation to LLP 1149, the RFC that acts on it; not linked into `llp/current/`). r2 corrected two section citations and counted the web's two devices, from Muse's review of LLP 1149 r1; r3 corrects the Windows presentation claim and records the alternatives all three reviews named (`llp/reviews/rfc-2026-10-28-1149-r1.{muse,grok,astra}.md`).
**Systems:** the painted presenter (`host/linux`: the paint walk, `paint/backend.rs`, `gpu.rs` and `raster.rs`); `host/windows`; the Android swapchain host (`host/linux/src/gpu/present.rs`); the Canvas 2D GPU module (`canvas/vello`); the web targets (LLP 1071)
**Author:** Claude (Fable 5.1) for Charlie Cheever
**Date:** 2026-10-28
**Related:** LLP 1015 (the painter and its two backends); LLP 1015.000 and 1015.001 (the Windows window and its composition numbers); LLP 1056 §8.5 (the Skia versus vello bake-off); LLP 1076 (Android rendering choices, §3.5 GPUI, §5 the GPUI pilot); LLP 1009 D1 (wgpu as the one GPU API for app code); LLP 1115 and 1115.004 (what is the platform's; D2 the presenter stays Exact's; D6.3 Windows text); LLP 1071 (the web's JS target); LLP 1149 (the renderer, the RFC this record recommends); LLP 1149.000 (the renderer on the web)

## 1. The question

A company with an Electron app on web, desktop and mobile is considering a rewrite in Rust,
user interface included. They do not need the app to feel native: it is an Electron app
today. Windows is the platform they care about most, and Windows has no native toolkit with
a strong identity. They want one look everywhere, styled by them, and the most performance
they can get: startup, frame rate, CPU, memory and battery. Krzys suggested "one generic
renderer that works on all the platforms, doesn't use native UI elements but has best
possible performance, then a way to mix it with platform specific renderers", and named GPUI
as a possible base.

Charlie's question: what are the options (GPUI, Flutter's renderer, Skia, vello, our own
thing, "WebGPU directly"), and which is fastest and most efficient?

External sources were checked on 2026-10-28. Repository observations are of the main
checkout at `d687090088`. Measurements are quoted from the LLPs that made them; nothing new
was measured for this record.

## 2. What exists here today

What the prospect describes is a painted presenter: a layout engine, one painter that draws
the tree the same way everywhere, and native pieces mixed in where they matter. Exact has
that on three platforms, with the kernel as the display list (LLP 1015 §2):

| Platform | Presenter | GPU | CPU |
|---|---|---|---|
| Linux | the painter, into DRM/KMS or a buffer | vello over wgpu (Vulkan), read back to the CPU | tiny-skia, the fallback and the pixel oracle |
| Windows | the painter, into a `softbuffer` surface, presented whole (damage presents only the caret's blink) | none for the UI; a game's full-window canvas presents from its GPU DLL, which composes the UI's pixel layers above and below it (LLP 1015.000, 1015.001; `gpu/README.md`) | tiny-skia |
| Android | the painter | vello into the window's Vulkan swapchain, pipelined one frame deep (`gpu/present.rs`); or the Canvas host, a Kotlin view replaying an op stream with a `RenderNode` per row (LLP 1076 §3.3) | tiny-skia |
| Apple | native views (UIKit, AppKit); the painter runs headless on macOS for tests | vello in the Canvas 2D module, kernels compiled at build (LLP 1056 §8.5) | Core Graphics |
| Web | the DOM (the JS target, LLP 1071) | wgpu over WebGPU for a game's canvas | the browser |

The painter's `Backend` trait already carries what a retained renderer needs: damage,
kept rows (a scroller's rows recorded once and replayed moved), animated layers kept apart
from their rows, image slots, opacity groups, masks, clips and backdrop filters. Of the two
backends, only the Android Canvas recorder keeps rows today; vello encodes the whole scene
every frame.

## 3. Findings

Each finding carries a confidence: **high** when measured here or read in the source,
**medium** when read in upstream documentation or reports, **low** when inferred.

### F1 — The ask is a painted presenter with native mixed in per control (high)

"One generic renderer, mix it with platform-specific renderers" is LLP 1115.004's shape for
Windows: Exact's presenter paints every node (D2), and what Windows owns comes from Windows
through the system's own API: a windowless RichEdit drawn into Exact's frame for text
editing (D7), the system's menus, dialogs and pickers (D10), UI Automation through AccessKit
(D11). The mixing is at the control, not the renderer. Nothing in the prospect's ask needs a
second UI framework.

### F2 — GPUI is a framework, not a renderer; its renderer's lessons are worth taking (high on the facts, medium on performance)

GPUI is an element tree, Taffy layout, focus, its own text system and a renderer, published
together. Taking it as the presenter means its tree owns what the kernel owns: layout, hit
testing, focus. LLP 1076 §3.5 and LLP 1115.004 D2 already rule it out on that ground, and
D2 names its Windows code "a reference for how others did the same pieces, not a
dependency".

Its renderer is the interesting part, and three facts about it bear on a Windows-first
customer:

- Zed first reused its Vulkan backend on Windows. Users' machines would not run it, so Zed
  wrote a DirectX 11 renderer, with a third shader dialect in HLSL beside MSL and WGSL
  ([the port](https://git.secluded.site/zed/commit/15ad9863296d427966098f6b9864d5b819725101);
  [the progress report](https://zed.dev/blog/windows-progress-report)). Zed's Windows
  documentation requires a DirectX 11 compatible GPU
  ([docs](https://zed.dev/docs/windows.html)).
- In February 2026 Zed removed blade and reimplemented the Linux renderer on wgpu, because
  blade "caused several issues for Zed users and third-party apps using GPUI"
  ([the commit](https://git.secluded.site/zed/commit/af8ea0d6c26192c45f44f473c0d4a7d6f72ed018)).
- Its renderer is a handful of instanced pipelines (quads, shadows, underlines, sprites from
  a glyph atlas, paths) with no compute shader, so it boots with almost nothing to compile and
  runs on old GPUs. Text is rasterized by DirectWrite on Windows and Core Text on macOS, so
  it looks like the platform's text.

The 2026-09-30 pilot (LLP 1076 §5) compared published GPUI 0.2.2 with Exact's native Apple
messages path at 10,000 rows. Idle application CPU was 0.25% of a core for Exact and 2.50%
for GPUI; GPUI was lower on tail updates (2.37% against 5.25%) and resize. The pilot did not
rank renderers (different endpoints, no GPU work measured), but the idle number is what an
immediate-mode renderer that redraws the frame costs against a retained one, and it bears on
battery.

### F3 — Flutter's engine is not a renderer Rust can link (high)

Impeller is a rasterization component inside the Flutter engine; the embedder "launches and
manages the Flutter engine", Dart VM included
([glossary](https://docs.flutter.cn/resources/glossary)). There is no Impeller library to call
from Rust, and the engine is C++ and megabytes. Flutter's web target paints to a canvas,
which is the failure mode LLP 1149.000 refuses as a default: no accessibility tree without a
parallel one, no selection, no find in page, megabytes before first paint. Flutter's 2026
roadmap completes Impeller on Android and removes the Skia backend there
([roadmap](https://flutter.dev/blog/flutter-darts-2026-roadmap)); none of that changes the
finding.

### F4 — Skia lost the one bake-off we ran (high, measured)

LLP 1056 §8.5 built Skia (skia-safe 0.153, Ganesh on Metal) and vello to the same level
behind one host ABI for Canvas 2D on Apple, and measured them the same way. Skia lost on:

- shader compilation: Ganesh writes SkSL per draw and Metal compiles it on the device (7
  programs for the full-screen fixture, 81 for the gallery), with no binary archive on its
  Metal backend; moving it off the first draw took a hand-laid C++ vtable;
- memory: 1.1 to 1.2 GB on the iPad during the full-screen fixture and still growing, against
  vello's 154 to 162 MB;
- size: 5.2 MB in the bundle and a 4.8 MB prebuilt download per target at build.

It won the main thread (118 to 148 ms/s against vello's 172 to 191), cause not found. Skia is
also what Chrome and Flutter used, so its text and its antialiasing are the familiar ones;
that is the one thing it would give that vello does not.

### F5 — vello: what it costs, what it gives, and where its own line is going (high on measurements, medium on sparse strips)

vello renders a scene through compute shaders over wgpu. In this repository it is the Linux
painter's GPU backend (crates.io 0.10), Android's swapchain painter, and the Apple Canvas 2D
module (`vendor/vello`, patched, kernels compiled to a Metal library at build).

What it costs, measured:

| Cost | Measurement | Where |
|---|---|---|
| Shader compilation on the boot path | 773 ms the first launch on a Mac (Metal), 9.8 ms after from the driver's cache; on RADV 77 ms the first launch, 6 ms after from wgpu's pipeline cache | LLP 1015 §7 |
| Device creation | 23 ms warm, 93 ms the first launch, on RADV | LLP 1015 §6 |
| Boot to first flip on the GPU painter | 155 to 164 ms at 1920×1080 on Linux, cached | LLP 1015 §6 |
| The readback, where the frame is not presented from the GPU | 4 ms to render and 17 to 19 ms to wait and map per frame on Metal, at 420×860 and 840×1720 alike; the CPU painter 3.8 to 6.9 ms | LLP 1015 §7 |
| A 1920×1080 frame on RADV | 12.6 ms (render 7.6, readback 4.8) against the CPU painter's 10.2 | LLP 1015 §6 |
| Memory over the CPU path | about one full-screen surface: 24 MB (iPhone) and 32 MB (iPad) more than SwiftUI on the same fixture | LLP 1056 §8.5 |
| Kernels | 22 compute kernels in the Canvas module; the module is 3.98 MB on iOS | LLP 1056 §8.5 |
| Two builds of wgpu | vello 0.10 pins wgpu 29, the GPU module is on 30 | `host/linux/Cargo.toml` |

It is accepted as the one exception to "the boot path compiles nothing"
(`rules/DEFERRED.md` §Runtime, Charlie, 2026-10-22).

What it gives, measured: 119.6 to 120 frames per second with 0 to 0.5 late frames per second
on an iPhone 13 Pro Max and an M1 iPad Pro at a full-screen canvas redrawn every frame, at
55 to 58% of SwiftUI's main-thread time; parity with Chrome on 94 of 94 crops (LLP 1056 §8.5).
A general vector renderer that handles any path, gradient, blend and clip at that speed is
not something to write again.

Where its line is going: Linebender's sparse strips architecture splits the work into CPU
coarse rasterization and a GPU fine stage with no compute shader. `vello_cpu` is a
multithreaded SIMD CPU renderer; `vello_gpu`, formerly `vello_hybrid`, renders the strips on
the GPU and runs on WebGL2 and DirectX 11 class hardware
([vello_gpu](https://docs.rs/vello_gpu)). Latest release 0.0.7 on 2026-03-24
([changelog](https://skia.googlesource.com/external/github.com/linebender/vello/+/refs/tags/sparse-strips-v0.0.7%5E%21/)).
It is pre-1.0 and I found no later release notes; it is the right CPU fallback to evaluate
and the right GPU path renderer to evaluate when paths need the GPU, not a base to commit the
presenter to today.

### F6 — "WebGPU directly" is wgpu, and the renderer is the decision (high)

WebGPU is an API. On native it is implemented by wgpu (Rust) or Dawn (C++); in the browser by
the browser. Everything in this table is already on wgpu: vello, the GPU module, Dioxus
Native. "Using WebGPU directly" means writing the renderer ourselves against that API, which
is GPUI's design and LLP 1149's. The question underneath is whether the renderer's device
layer is wgpu or the platform's own API, and wgpu's costs for a UI renderer are concrete:

- device creation: 23 ms warm and 93 ms the first launch on RADV (LLP 1015 §6); with Metal the
  instance and adapter enumeration are on the boot path too;
- no DirectX 11 backend, so the Windows reach Zed went to DirectX 11 for is not available
  through it; its Windows backend is DirectX 12;
- presentation is wgpu's: dirty-rectangle presents, waitable swapchains, the Metal layer's
  own options and external-memory import (IOSurface, DXGI shared handles, `AHardwareBuffer`)
  are behind its abstraction or patches (`vendor/wgpu-hal/EXACT-PATCHES.md` carries two);
- it is a large dependency for six pipelines, and the repository already carries two versions.

What wgpu gives in return is one backend to keep correct and WebGPU on the web for free. The
trade is LLP 1149 D5.

### F7 — What decides the five metrics (high)

For a UI, the renderer's API decides little. These do:

| Metric | What decides it | Evidence |
|---|---|---|
| Startup | Runtime shader compilation, then device creation, then fonts | F5's table; fonts 3.7 ms for 43 faces on a builder, 25 ms on a laptop (LLP 1015 Summary and §3) |
| Frame rate | Hitches: shader compilation mid-session, allocation, a frame encoded on the main thread | Every candidate holds 120 Hz on a flat workload (F5) |
| CPU and battery | Not redrawing: kept rows, animated layers kept apart, a frame only when something changed | the Backend trait's rows and layers; F2's idle pilot |
| Memory | The renderer's buffer policy and whether rows are cached as pixels | F4: Skia's 1.1 GB against vello's 154 MB on the same fixture; vello's bump buffers |
| Reach | A CPU path for the machines without a usable GPU | F8 |

### F8 — The Windows fleet needs a CPU path and a DirectX 11 class floor (high)

Business desktop apps run in virtual machines, over Remote Desktop and Citrix, and on old
Intel integrated GPUs. Zed's Vulkan experience (F2) is the evidence; Chrome's software raster
fallback is the precedent. A renderer whose only path needs compute shaders and a DirectX 12
device excludes those machines or runs them through WARP, DirectX 12's software rasterizer,
at a speed nobody measured. Two things follow: the GPU path should need no more than
DirectX 11 feature level 11_0 (no compute, no indirect dispatch), and the CPU painter stays.

### F9 — On the web the DOM is the platform's renderer (high)

"One renderer everywhere" means painting into a canvas on the web. LLP 1115's test applies:
would someone who knows the platform notice? On the web they notice at once: text cannot be
selected or found, screen readers see nothing without a parallel tree, the IME fights a
hidden input, zoom and print break. Flutter web is the running example. Exact's web target
is the DOM, and the painted hosts are held to Chrome's pixels, so an Exact app is already
consistent across the web and the painted platforms in the way that matters: the same layout
and the same look, verified. A painted web target is for apps whose content is not a
document (design tools, maps, very large grids), chosen on purpose. That is LLP 1149.000.

### F10 — Electron is the baseline the prospect will compare against (medium)

They run Electron today, so Electron's numbers are the ones to beat and to show: a Chromium
process tree, 100 to 300 MB resident for a small app, hundreds of milliseconds to first
paint. Any of the candidates wins on memory and startup. The comparison that will decide
them is against each other on the same screen: a Rust process with a GPU device against the
same screen in Electron and in a hand-built WinUI 3 app, on the same Windows machine. LLP
1149 §4 makes that a measured stage, not a claim.

## 4. Assessment

**Keep the painted presenter; replace what draws its frame on the GPU with a renderer of our
own, written for a user interface, on each platform's own graphics API.** Not GPUI (a
framework), not Flutter's engine (not linkable), not Skia (lost the bake-off, C++), and not
vello as the presenter's renderer: a general compute-based vector renderer is more than a UI
needs on every frame and its costs (F5) are exactly the prospect's metrics. vello stays where
it is right, in the Canvas 2D module, where any drawing may arrive.

The renderer is GPUI's renderer, in Exact's painter, with Exact's retained contract: rounded
quads, borders, analytic shadows, gradients, images, glyphs from an atlas rasterized by the
platform's own text rasterizer, paths as coverage masks, clips and layers, in a handful of
pipelines with no compute, encoded from the painter's walk and replayed from kept rows.
Shaders compiled at build. Metal on Apple, DirectX 11 on Windows, Vulkan on Linux, OpenGL ES
on Android, WebGPU with a WebGL2 fallback on the web for the apps that opt in. The CPU
painter remains the oracle and the fallback everywhere.

The costs of that answer are real and LLP 1149 states them: six device layers to keep
correct (WebGPU and WebGL2 are two), which the oracle and the parity suite hold; per-platform text rasterization, which
is a feature on Windows and Apple and a tolerance to audit; and an amendment to LLP 1009 D1,
which keeps wgpu as the one API for an app's own GPU code and takes it out of the host's
renderer.

**The alternatives the panel named, which LLP 1149 S0 measures rather than argues.** Muse
and Grok: keep vello and remove its three costs in place — compile its kernels at build on
every platform, present from the swapchain everywhere, and keep rows as retained scene
fragments (`Scene::append` under a transform); it keeps one renderer and one device shared
with an app's canvases, and keeps the compute-class floor and glyph outlines in place of the
platform's rasterizer. Astra: the same UI-shaped core on wgpu where wgpu meets the
requirements, with a direct DirectX 11 layer for Windows, deciding each further direct layer
by a matched measurement. All three say the same thing about this record's F5 and F6: the
readback, the runtime shader compilation and the absence of kept rows are the integration's
costs, and device creation is mostly the driver's, so a per-platform device layer is not yet
shown to buy boot time. LLP 1149 D15 builds the arms (vello as today, vello with kept rows,
the ink core on wgpu, the ink core on direct Metal) on one path and declares the thresholds
before the measurement.

## 5. Sources

- Zed: [windows: Port to DirectX 11 (#34374)](https://git.secluded.site/zed/commit/15ad9863296d427966098f6b9864d5b819725101); [Windows progress report](https://zed.dev/blog/windows-progress-report); [Zed on Windows](https://zed.dev/docs/windows.html); [gpui: Remove blade, reimplement linux renderer with wgpu (#46758)](https://git.secluded.site/zed/commit/af8ea0d6c26192c45f44f473c0d4a7d6f72ed018)
- Linebender: [vello_gpu](https://docs.rs/vello_gpu); [sparse strips v0.0.7](https://skia.googlesource.com/external/github.com/linebender/vello/+/refs/tags/sparse-strips-v0.0.7%5E%21/)
- Dioxus: [0.7 release](https://dioxuslabs.com/blog/release-070) (Dioxus Native: Blitz over Stylo, Taffy and vello on wgpu; experimental)
- Flutter: [Flutter & Dart's 2026 roadmap](https://flutter.dev/blog/flutter-darts-2026-roadmap); [glossary: embedder, Impeller](https://docs.flutter.cn/resources/glossary)
- This repository: LLP 1015 §5 to §7, LLP 1015.000, LLP 1015.001, LLP 1056 §8.5, LLP 1076 §3.5 and §5, LLP 1115.004 D2, D6.3 and D7
