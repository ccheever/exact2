# exact-game-render

Opaque PBR over slot-indexed floats. No `World`, entities, simulation, or CPU
per-instance work in `draw`. Dependencies are `exact-gpu` and the engine's `glam`.

Create `Renderer::new(device, queue, target_format)`, add meshes, initialize
transforms with `write_transforms_both` and materials with `write_materials`, then
call `set_batches`. Each tick calls `begin_tick` followed by contiguous transform
writes. Each display frame supplies only `FrameInput`; the GPU interpolates.
The caller initializes every listed slot, including holes in sparse uploads.

The output format is fixed at construction so every pipeline is compiled there.
All subsequent calls use that device and queue. Matrices use conventional 0–1
depth (near zero), with depth comparison `Less`. The renderer resolves 4× MSAA
RGBA16F into RGBA16F, then applies ACES-fitted tonemapping. Non-sRGB targets get
an explicit sRGB transfer; sRGB targets use the hardware transfer once.
Materials and lights use linear RGB. Alpha is reserved: this renderer is opaque.
Capsule height is tip-to-tip, including both hemispheres.

`mesh_bounds` retains conservative local spheres. `set_batches` accepts an
arbitrary subset, so a future culling pass can replace the list independently
of persistent transforms. Storage and geometry grow with GPU copies and never
shrink. Sparse slot 1,000,000 consequently retains a large high-water copy range
on every tick, even if most slots are absent.

Steady frames use a 656-byte stack uniform and allocate no renderer-owned CPU
collections; wgpu owns command encoding and staging allocations. `encode_us`
includes the uniform upload, encoding and submission, not GPU completion. It is
zero on wasm32: a transitive `web-sys` dependency cannot be imported without
adding a direct dependency. WebGPU is required; there is no WebGL fallback.

From `game/`:

```sh
EXACT_UPDATE_TRUST=development EXACT_GPU_OUT=/tmp/game-render cargo test -p exact-game-render -- --nocapture
EXACT_UPDATE_TRUST=development EXACT_GPU_OUT=/tmp/game-render cargo test -p exact-game-render --release -- --ignored --nocapture
```

The ignored test measures 200,000 rotating cubes, 600 frames at 1280×720,
a full transform upload every second frame, and bounded GPU-completed wall time.
It reports upload and CPU encoding separately; it is not a vsync/FPS measurement.
GPU fixtures print an explicit skip reason if no adapter is available. Shader
parsing/validation and primitive geometry checks still run without an adapter.
