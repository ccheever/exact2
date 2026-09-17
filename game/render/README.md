# exact-game-render

Opaque PBR over slot-indexed floats. No World, simulation, or CPU per-instance
work in `draw`. Dependencies remain `exact-gpu` and `glam`.

Create `Renderer::new(device, queue, target_format)`, add meshes, initialize
transforms with `write_transforms_both` and materials with `write_materials`, then
call `set_batches`. `Batch::new(mesh, slots)` casts shadows by default. Each tick
calls `begin_tick` followed by contiguous transform writes. The GPU interpolates
the two ticks; forward and shadow vertices share `shaders/transform.wgsl`.
The caller initializes every listed slot, including holes in sparse uploads.

`FrameInput::default()` gives a shadowed sun, a gradient sky, hemisphere ambient
light and bloom. Supply matching view/projection/camera-position values. Matrices
use conventional WebGPU 0–1 depth, near zero, positive view-space near distance;
reverse-Z is not supported. Materials, environment and lights use linear RGB.
Base alpha is reserved; geometry is opaque. Capsule height is tip-to-tip.

- `Sun.shadows = Some(Shadows::default())`: 60 m reach, three 2048² Depth32Float
  layers, practical splits (lambda 0.7), rotation-invariant frustum spheres,
  light-space texel snapping, comparison-sampled 3×3 PCF of radius 1.5 texels.
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
textures are retained. Attachments allocate only on enable, cascade-count change,
or resize. Steady frames allocate no renderer-owned CPU collections, upload a
1 KiB stack frame uniform (+768 bytes with shadows), and walk only retained
batches, at most four times. wgpu owns command encoding and staging allocations.
The resolved 4× MSAA HDR image passes through ACES-fitted tonemapping; non-sRGB
outputs use an explicit sRGB transfer, sRGB outputs the hardware transfer once.

`mesh_bounds` retains conservative local spheres. `set_batches` accepts any
subset; shadow casters must be present in that retained list, including offscreen
casters that should affect visible receivers. Arenas grow with GPU copies and
never shrink. Sparse slot 1,000,000 therefore retains a large high-water copy.
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

The brief's earlier baseline was 0.26 / 1.6 / 2.3. Machine load changed during the
session: these show no observed regression, not a claimed renderer speedup. The
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
