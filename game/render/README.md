# exact-game-render

Opaque PBR over slot-indexed floats. No World, simulation, or CPU per-instance
work in `draw`. Dependencies remain `exact-gpu` and `glam`.

Create `Renderer::new(device, queue, target_format)`, add meshes, initialize
transforms with `write_transforms_both` and materials with `write_materials`, then
call `set_batches`. `Batch::new(mesh, slots)` casts shadows by default. Each tick
calls `begin_tick(Rewrite::Some)` followed by contiguous transform writes: history
is copied GPU-side so untouched slots stay still. `begin_tick(Rewrite::All)` swaps
roles without copying; the caller promises to rewrite every live slot before draw. The GPU interpolates
the two ticks; forward and shadow vertices share `shaders/transform.wgsl`.
The caller initializes every listed slot, including holes in sparse uploads.
Quaternions should be unit length; a zero quaternion draws as identity.
**Scale is positive.** Stray negative components use their absolute values in both
position and normal transforms, before interpolation. Mirrored instances return
as a double-sided material, not as a sign.

`max_slots()` is the exclusive slot limit from the device's granted adapter storage
binding limit and the widest arena (48-byte materials), capped by max buffer size.
`write_transforms`, `write_transforms_both`, `write_materials`, and `set_batches`
return `Result<(), RenderError>` with arena, requested slot and limit on capacity
refusal, before changing state. Incomplete records and invalid meshes/draw ranges
remain caller errors.

`FrameInput::default()` gives a shadowed sun, a gradient sky, hemisphere ambient
light and bloom. Supply matching view/projection/camera-position values. Matrices
use conventional WebGPU 0–1 depth, near zero, positive view-space near distance;
reverse-Z is not supported. Materials, environment and lights use linear RGB.
Base alpha is reserved; geometry is opaque. Capsule height is tip-to-tip.

- `Sun.shadows = Some(Shadows::default())`: 60 m reach, three 2048² Depth32Float
  layers, practical splits (lambda 0.7), rotation-invariant frustum spheres,
  light-space texel snapping in a [Duff basis](https://graphics.pixar.com/library/OrthonormalB/paper.pdf)
  (Y is the sign axis; continuous near vertical, hemisphere seam at the horizon), comparison-sampled 3×3 PCF of radius 1.5 texels.
  The final 10% of each slice cross-fades; the last fades to unshadowed. Casters
  up to one shadow distance towards the sun beyond the slice are included.
- Shadow bias is **slope scale 3, constant 2 depth units, plus receiver normal
  offset 0.35 texels × (1 − N·L)**. Back faces are culled. This keeps single-sided
  casters, unlike front-face culling. Slope scale 1.5 produced visible PCF acne;
  3 removes it while preserving contact in the fixtures. Bias is tuned for the
  default softness; very wide custom kernels may need a different bias policy.
- `FrameInput.bloom = Some(Bloom::default())`: threshold 1, intensity 0.08,
  tent radius 1. A one-sided soft knee starts at the threshold, followed by
  13-tap half-resolution/downsample filters and additive tent upsampling.
  Up to six levels, stopping before either dimension would fall below 8;
  tiny outputs still have one level. Coarser octaves contribute half as much.
  Portable, filterable/blendable RGBA16F avoids optional Rg11b10Ufloat features.
- `Environment { zenith, horizon, ground, .. }` supplies both the directional sky
  and hemisphere lighting. Sky draws at far depth after geometry. `sun_disc`
  is an angular radius in radians (zero removes disc/glow). Equal sky colours
  and no disc use the clear directly, with no sky draw or inverse-matrix work.
- `Environment.fog = Some(Fog::default())`: exponential extinction, analytically
  integrated through an exponential Y-height density profile. `color: None`
  uses the horizon. Density defaults to 0.02/m, height falloff to 0.1/m.

All pipeline variants compile in `Renderer::new`, including fog-free and
shadow-free forward entry points and bloom-free tonemapping. Setting an effect
to None skips its passes and releases its textures/bindings; no dummy effect
textures are retained. HDR/depth and bloom attachments grow in 64-pixel buckets;
shrinking never reallocates. Viewports/scissors use the logical size, and post-pass
UVs clamp to its edges, excluding unused padding. Bloom level count still follows
the logical size. `Stats.texture_creations` counts cumulative attachment textures,
including shadow/effect enables; it stays unchanged on shrink or within a bucket.
Steady frames allocate no renderer-owned CPU collections, upload a
1040-byte stack frame uniform (+768 bytes with shadows, +3072 bytes with bloom), and walk only retained
batches, at most four times. wgpu owns command encoding and staging allocations.
The resolved 4× MSAA HDR image passes through ACES-fitted tonemapping; non-sRGB
outputs use an explicit sRGB transfer, sRGB outputs the hardware transfer once.
HDR reads map NaN to zero and clamp to [0, 65472] (the last half-float below 65504).
The bright pass sanitizes before bilinear interpolation; tonemapping sanitizes
HDR, bloom and exposure-scaled values before the ACES fit.

`mesh_bounds` retains conservative local spheres. `set_batches` accepts any
subset; shadow casters must be present in that retained list, including offscreen
casters that should affect visible receivers. Arenas grow with GPU copies and
never shrink. Sparse slot 1,000,000 retains a large high-water copy with `Rewrite::Some`;
`Rewrite::All` avoids that tick copy.
`Stats.instances/triangles` describe the forward scene; `draws` includes all passes.
`encode_us` includes upload/encoding/submission, not completion, and is zero on wasm.

From `game/`, keeping build and image artifacts inside this crate:

```sh
export EXACT_UPDATE_TRUST=development CARGO_TARGET_DIR="$PWD/render/target"
export EXACT_GPU_OUT="$PWD/render/target/pictures"
cargo build -p exact-game-render
cargo test -p exact-game-render -- --nocapture
cargo clippy -p exact-game-render --all-targets -- -D warnings
cargo fmt -p exact-game-render -- --check
cargo build -p exact-game-render --target wasm32-unknown-unknown
cargo test -p exact-game-render --release -- --ignored --nocapture --test-threads=1
```

GPU fixtures print an explicit skip reason without an adapter. Geometry and WGSL
validation still run. `tests/core.rs` includes the effects and timing modules.
The pictures are PPMs; all were converted to PNGs and visually inspected on Metal.
The lit plane is clean, the shadow contacts its cube, and yaw changes the recovered
world edge by 4 mm (under one output pixel). Shadow/ambient-only probes both read
59/255 versus 183/255 in sun; lit-patch variance is zero. Shadows exist at 5, 25,
55 m and vanish at 70 m. The bloom fixture lights 16,392 pixels outside the sphere;
0.95 HDR stays identical with bloom enabled/disabled. Fog leaves the nearby dark
cube clear and washes the far cube toward the horizon; lifting both to Y=20
removes almost all fog. The 170° sky diagnostic deliberately exaggerates its
perspective curvature. The 300-object image shows soft grounded cubes, a yellow
beacon halo, and atmospheric depth. Remaining visual limitations: distant shadows
are blurrier than nearby ones, the beacon's bright core clips to white, and the
simple gradient has no atmospheric scattering. There is no ambient occlusion.

Measured 2026-09-17, Apple M5 Max, shared machine (not vsync/FPS):

| Scene | CPU encode ms | Tick upload ms | GPU-completed wall ms |
|---|---:|---:|---:|
| B0 before edits, 200k cubes, 1280×720 | 0.3812 | 2.4352 | 3.9829 |
| B0b, all effects off, same 200k geometry/resolution | 0.1437 | 1.1760 | 1.8355 |
| B0c, `Rewrite::Some`, same 200k scene | 0.2558 | 1.6117 | 2.3480 |
| B0c, `Rewrite::All`, same 200k scene | 0.3186 | 2.0196 | 3.2794 |

B0c ran both modes for 600 frames each on the shared M5 Max. `All` removes the
GPU history copy but was slower in this run; these measurements do not establish
a speedup. All release tests and both ignored diagnostics pass on Metal; scoped
clippy (`-D warnings`), fmt, wasm32 build and the repository caps check pass.
The new fixtures verify half-float RGB before byte conversion, the unchanged far
corner under over-range bloom, zero/sparse quaternions, negative-scale twins,
capacity refusal without mutation, both tick modes, and grow/shrink pixel identity.
The vertical-sun sweep reaches ±8.25° (the old switch was 8.11°): its straight edge,
recovered across a small patch, moves at most 0.008200 m versus a 0.029480 m shadow
texel. A single rasterized scanline can jump a full projected texel; this is not a
universal sub-texel motion guarantee for arbitrary casters and camera settings.

The brief's earlier baseline was 0.26 / 1.6 / 2.3. Machine load changed during the
earlier B0/B0b session: those numbers show no observed regression, not a claimed renderer speedup. The
effects-off benchmark uses a constant environment (flat ambient, no sky pass).

The 300-object diagnostic renders at **2560×1440**, 4× MSAA. Off / shadows / all
CPU encode: **0.0691 / 0.1627 / 0.3194 ms**. GPU frame spans:
**0.2814 / 0.4438 / 0.5401 ms**. Shadow delta: **0.0936 ms CPU**, **0.1624 ms GPU**
by frame-span difference; summing depth intervals plus the forward delta gives
0.2493 ms, also below budget. Measurements include optional timestamp writes.

All-effects GPU intervals (ms):

| Pass | Interval |
|---|---:|
| Shadow 0 / 1 / 2 | 0.0210 / 0.0266 / 0.0261 |
| Forward + sky + fog | 0.2693 |
| Bloom bright | 0.2938 |
| Bloom down 1 / 2 / 3 / 4 / 5 | 0.2907 / 0.2767 / 0.2609 / 0.2497 / 0.2358 |
| Bloom up 4 / 3 / 2 / 1 / 0 | 0.2272 / 0.2219 / 0.2186 / 0.2192 / 0.2558 |
| Tonemap | 0.2670 |

**These intervals overlap; do not sum them.** Metal samples from vertex start to
fragment end, and later fullscreen vertices run ahead of dependent fragments.
`FrameInput.timestamps` optionally borrows a 32-entry query set; the caller must
request TIMESTAMP_QUERY and resolve/read it outside draw. `GPU_PASS_NAMES` maps
the 16 pairs; disabled passes do not write theirs. The diagnostic waits for GPU
completion before its separate resolve submission (otherwise Metal can return
trailing zero samples), and reports invalid/reversed pairs as NaN, never zero.
