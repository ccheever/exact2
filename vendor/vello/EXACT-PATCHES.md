# Vendored vello — Exact patches

- **Upstream:** `vello` 0.10.0 from crates.io, renamed `exact-vello` so the
  Linux host's painter keeps the crates.io vello. The fine, coarse and ptcl
  WGSL patches live in `canvas/vello/build.rs`, which compiles every kernel to
  a Metal library at build time.
- **Why vendored:** the Canvas 2D GPU module (`canvas/vello`, LLP 1056 §8.5)
  needs four things upstream does not do. Every change is marked `EXACT:` in
  the source.
- **Owner:** Charlie Cheever (Canvas 2D on Apple).
- **Replacement plan:** propose 1, 2 and 4 upstream; 3 is ours for as long as
  `rules/DEFERRED.md` refuses runtime shader compilation and wgpu has no
  Metal-library loader for vello.

## 1. Bump buffers sized to the scene (`src/exact.rs`, `src/render.rs`, `src/lib.rs`)

Upstream allocates its bump buffers at fixed sizes (about 150 MB) whatever the
scene. They now start small; a render that overflows reports what it needed
and runs again at that size. Pooled buffers remember the render that last
released them and are dropped when unused (`Renderer::trim`).

## 2. No indirect dispatch where the device has none (`src/wgpu_engine.rs`, `src/render.rs`)

wgpu disables indirect dispatch on the iOS Simulator (it reports only the
Apple2 family), which made every INDIRECT buffer invalid. Without it, the path
count and path tiling stages run over their buffers' whole capacity; both stop
at vello's own counters.

## 3. Precompiled shaders (`src/lib.rs`, `src/shaders.rs`, `src/wgpu_engine.rs`)

`RendererOptions::shaders` takes each kernel as a Metal library, entry point
and workgroup size (`PrecompiledShader`), loaded through wgpu's passthrough
shaders. The CPU shaders and `util` (whose blitter needs the WGSL front end)
are gone, so the module carries no WGSL front end and compiles no shader source.

## 4. Rendering onto kept pixels (`src/render.rs`, `src/shaders.rs`)

`Renderer::render_exact` renders into a BGRA8 target starting each pixel from a backdrop
image (the canvas's previous pixels), writing premultiplied colour, so a
canvas keeps its bitmap without a copy pass. A scene of solid paths keeps the
image atlas instead of shrinking it (an upstream bug that blanked images).
