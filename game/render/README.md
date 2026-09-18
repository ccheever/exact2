# exact-game-render

Opaque PBR over slot-indexed floats. `WorldSurface<G>` connects a simulation to an
Exact canvas; the simulation crate owns no GPU or host. Geometry draws once per
mesh batch **per geometry pass**: each shadow cascade draws its casters again,
then forward rendering draws the scene. There is no frustum rejection in those loops.

## Renderer contract

Construct `Renderer::new(device, queue, format)`, add meshes, initialize both
transform histories with `write_transforms_both`, upload materials, and set batches.
`begin_tick()` swaps the history roles without copying. The caller must make every
listed slot current before `draw(target, size, frame)`; a retained target page may
already match. The renderer retains the device/queue and pipelines for its format.
The target must match that format. Direct callers initialize sparse holes too.

Transform records are ten floats (position, quaternion, scale); materials are twelve
(linear base RGBA, metallic, roughness, emissive RGB, primitive dimensions XYZ).
Base alpha is reserved; the forward shader returns 1. Quaternions should be unit
length; zero draws as identity. Scale is positive; negative inputs use absolute
values. Mirroring awaits double-sided materials. WebGPU depth is 0–1, near zero;
reverse-Z is unsupported. Capsule height is tip-to-tip.

`max_slots()` is an exclusive limit from granted storage binding/buffer limits and
48-byte materials. Writes and batch changes return named `RenderError`s before
mutation on capacity refusal. Incomplete records/invalid meshes are caller errors.
Arenas grow with GPU copies and never shrink. `Stats.instances/triangles` describe
the forward scene; `draws` includes all passes; `texture_creations` is cumulative.
CPU timing belongs to the caller; `draw` makes no performance clock calls.

## Effects

`FrameInput::default()` supplies a shadowed sun, gradient sky, hemisphere ambient
light and bloom. Supply matching view/projection/camera position. All colours are
linear. `Bloom` and `Fog` are re-exports of the engine's saved types.

- Sun shadows default to 60 m, three 2048² Depth32Float cascades, practical splits
  (lambda 0.7), rotation-invariant fitting spheres and texel snapping. Casters up
  to one shadow distance towards the sun are included. The last 10% of each slice
  cross-fades; the final slice fades to unshadowed.
- Receiver normal bias is `0.5 × world texel × sin(theta) × cos(theta)`. Nine PCF
  taps project the receiver plane into the light map, using exact plane depth
  slopes. Bilinear footprint bias is capped at two world-depth texels plus 1e-6
  normalized depth. Beyond that budget, four explicit depth loads per tap compare
  against their individual plane depths. Raster depth bias is zero. Thin sheets
  cast; back faces are culled. Supported direct sun fades over N·L 0.01→0.005;
  at/below 0.005 no singular plane slope is evaluated, including beyond shadow reach.
  Shadow-disabled lighting is unfaded. Ambient/emission/point lights are unaffected.
- Bloom defaults to threshold 1, intensity 0.08, radius 1: one-sided knee, 13-tap
  downsampling and additive tent upsampling. Up to six RGBA16F levels, stopping
  before either dimension falls below 8; tiny outputs retain one level.
- Sky and hemisphere illumination share zenith/horizon/ground colours. A constant
  sky without disc or differing fog colour uses the clear directly. `sun_disc` is
  angular radius in radians. Fog integrates exponential distance and Y-height
  density analytically, including a stable near-horizontal limit; sky uses 10 km.
  Default density is 0.02/m and height falloff 0.1/m; absent fog colour uses horizon.

Eleven pipeline variants compile on first world render, including effect-free
entry points. Optional loading after app first pixel does not remove this Play
latency. Disabling effects skips their passes and releases their attachments.
HDR/depth/bloom attachments grow in 64-pixel buckets; shrinking reuses them.
Viewports and post-pass UVs respect logical size. Discarded 4× MSAA colour/depth
attachments request transient storage (a no-op where unsupported). ACES-fitted
tonemapping applies sRGB transfer once. HDR reads sanitize NaN and clamp to
[0,65472]; bright-pass sanitization precedes filtering.

## WorldSurface and feed

A game's GPU shell is `exact_game_render::module!(MyGame)`. Bind constructs Sim;
first render constructs Renderer. Feed setup and only the last two completed ticks
of a seek. Frames interpolate on the GPU and visit retained camera/light/batch
records, without per-instance CPU work.

Feed fingerprints allocated Transform pages against each target history, patches
parented global poses into retained scratch, coalesces dirty runs and writes them.
After two still ticks, unchanged column revisions skip hashing. A changed column
still requires a page scan, even when most pages are unchanged. Selected pages
are copied into retained scratch; this is not zero-copy. At least 75% dirty pages
selects full-run uploads; every 32nd feed probes three ticks to return to hashing.
Scenes below 32 pages always hash. Materials use one history and repack 40-byte
engine records into 48-byte GPU records.

Fresh/teleported entities and Parent edits patch both histories. World replacement
generations force both histories to refresh even without a tick. Revisions and
presentation histories are excluded from saves/hashes. Parented TRS decomposition
is exact under uniform ancestor scale; shear is approximated. Each primitive kind
shares one unit mesh; dimensions are instance data. Capsules translate cap
hemispheres instead of stretching them. Asset meshes are refused by name.
At 200k slots materials cost 9.6 MB, two transform histories 16 MB.

Camera/sun/point rotations use normalized slerp histories. The first posed sun
wins. Point-light selection is feed-only, at most sixteen lights, with a 10%
incumbent distance margin and entity-order ties. Engine illuminance is lux:
10,000 lux maps to renderer radiance 3. Missing materials/environment use defaults.

Performance samples appear only in `state.world.perf`: live frame stamps, tick,
feed, encode (frame input through submit) and ticks/frame distributions. CPU sample
rings retain 16,384 values. Seekable renders, agent advances and timed binds make
no perf clock calls; samples do not enter hashes. The allocation-free claim covers
only the `steady_sim_feed_and_frame_inputs_allocate_nothing` moving-cube/camera/light
fixture (including trace recording), after warmup,
without input edges, structural churn, audio or physics. wgpu owns its command and
staging allocations; the claim does not include those.

Capacity errors precede history swaps and propagate through the surface ABI;
failed draws are not presented. Invalid viewports skip drawing and preserve the
input viewport, while a finite seekable clock can still advance.

## Latest recorded performance and remaining budgets

Apple M5 Max / Metal, release, 2026-09-17; shared-machine observations, not vsync
or a speedup claim. Feed diagnostic: 2560×1440, 4× MSAA, 60 warmup + 240 frames.
These are the latest recorded feed measurements; D1 changes persistence/APIs.

| Cubes | Sim ms p50/p95 | Feed ms p50/p95 | Encode ms p50/p95 |
|---:|---:|---:|---:|
| 10,000 | 0.0418 / 0.0911 | 0.1724 / 0.4077 | 0.1671 / 0.5722 |
| 100,000 | 0.4080 / 0.8699 | 0.9537 / 2.0966 | 0.1929 / 0.4053 |
| 200,000 | 0.7976 / 1.1324 | 1.6949 / 2.2815 | 0.1960 / 0.3274 |
| 500,000 | 2.3245 / 3.0835 | 4.5562 / 5.6280 | 0.2170 / 0.4038 |

**Both feed targets remain unmet.** Turning 500k: 4.5562 ms p50 versus 3 ms.
Still 500k with only a moving camera: **1.1262 / 1.4261 ms p50/p95 versus a
0.3 ms target**. Unchanged pages still cost a scan when the column changes.
The 28.72 GB/s page-hash microbenchmark does not establish whole-feed latency.

Latest paired renderer diagnostics: 200k cubes, effects off, 1280×720, CPU encode
0.2429 ms, tick upload 1.4430 ms, GPU-completed frame 2.8317 ms. Shadowed Beacons
GPU frame 0.5214 ms, CPU encode 0.2570 ms. These are medians of three run summaries;
shared-machine variance precludes a tight speedup claim. Optional GPU timestamp
intervals overlap on Metal: **do not sum them**. `GPU_PASS_NAMES` maps sixteen
query pairs; disabled passes leave theirs unwritten. Resolve after completion;
invalid/reversed pairs are NaN.

## Reproduce

From `game/`, with `EXACT_UPDATE_TRUST=development`:

```sh
cargo build --workspace
cargo test --workspace --no-fail-fast
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
cargo build -p greybox-gpu --profile web --target wasm32-unknown-unknown
cargo run -p exact-game-render --release --example cubes -- 500000 240
cargo run -p exact-game-render --release --example cubes -- 500000 240 still
cargo test -p exact-game-render --release -- --ignored --nocapture --test-threads=1
bun games/greybox/proof.mjs web
bun games/beacons/proof.mjs web
bun games/greybox/proof.mjs macos
```

GPU tests explicitly skip without an adapter; geometry and shader validation still
run. Set `EXACT_GPU_OUT` for image artifacts. The current game proofs pin native
and browser hashes. [Game README](../README.md) covers clock, save and dev carry;
[ergonomics diary](../diaries/002-ergonomics.md) retains experiment history.
