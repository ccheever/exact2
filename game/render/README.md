# exact-game-render

PBR over slot-indexed floats and separate model draw instances. `WorldSurface<G>` connects a simulation to an
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
The forward shader is opaque (alpha 1). Negative uploaded base alpha encodes positive grid spacing; WorldSurface clamps authored alpha to nonnegative when the grid is off. Quaternions should be unit
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
- Bloom defaults to threshold 1, intensity 0.16, radius 1.5: one-sided knee, 13-tap
  downsampling and additive tent upsampling. Up to six RGBA16F levels, stopping
  before either dimension falls below 8; tiny outputs retain one level.
- Sky and hemisphere illumination share zenith/horizon/ground colours. A constant
  sky without disc or differing fog colour uses the clear directly. `sun_disc` is
  angular radius in radians. Fog integrates exponential distance and Y-height
  density analytically, including a stable near-horizontal limit; sky uses 10 km.
  Fog is enabled by default. Default density is 0.012/m and height falloff 0.1/m; absent fog colour uses horizon.

`Material::grid(color, spacing)` uses a derivative-antialiased world-space grid,
projected onto any face in the existing forward shader. Positive saved spacing
reuses the material's former padding; its GPU flag/spacing uses the opaque alpha
slot (negative spacing). Uploads stay twelve floats per instance and there is no
extra texture or pipeline. Non-grid materials skip the grid branch. The cubes
bench explicitly disables fog/bloom to retain its effects-off fast path.

Twenty-nine pipeline variants compile on first world render, including effect-free
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

Feed checks storage write generations against each target history and reads only
changed pages. It patches parented global poses into retained scratch, coalesces
dirty runs and writes them. Same-value assignment filtering hashes changed pages
only; `filter_same_values(false)` skips hashes for streams known to change every
leased page. The default filters. There is no dense/probe/adaptive-skip policy.
Metadata still scales with allocated pages; selected pages are copied into scratch,
not uploaded zero-copy. Parented pages are checked when any ancestor might move.
Materials use the same generation gate, one history, and repack engine records
into GPU records; structural mesh/transform changes also refresh defaults/dimensions.

Fresh/teleported entities and Parent edits patch both histories. World replacement
generations force both histories to refresh even without a tick. Revisions and
presentation histories are excluded from saves/hashes. Parented TRS decomposition
is exact under uniform ancestor scale; shear is approximated. Each primitive kind
shares one unit mesh; dimensions are instance data. Capsules translate cap
hemispheres instead of stretching them. Models use `DrawInstance { transform, geometry, material, local }`: the feed
allocates render slots from `RENDER_SLOT_BASE`, above entity indices. The transform
slot still addresses the unchanged ten-float page upload. Geometry/material form
batch keys, and composed node matrices plus inverse-transpose normals live in a
separate instance buffer. Primitive records retain their compact identity encoding
in the existing slot lists: transform/material = slot, geometry = batch, offset =
identity. A primitive world binds no model group and samples no material texture.

Model materials use a separate forward pipeline and alpha-tested shadow pipeline,
with opaque/mask/blend and culled/double-sided variants created at construction.
Five texture slots (base colour, normal, metallic-roughness, emission, occlusion)
have 1x1 defaults. Colour/emission textures use sRGB texture formats; data maps are
linear. Mips arrive baked, with trilinear and 4x anisotropic sampling and authored
wrap modes. Normal mapping derives a cotangent frame from screen-space world/UV
derivatives (including models without tangents); baked tangents are retained for
S3b, not uploaded. Material UV transforms apply separately to every texture.

Opaque batches stay retained. Only transparent draws are sorted each displayed
frame, back-to-front in camera depth, using retained tick poses and local centers.
They keep depth testing, disable depth writes, and do not cast shadows. A model's
own materials are multiplied by entity base colour and have entity emission added.
The environment's hemisphere approximation supplies ambient metallic reflection;
this is not image-based lighting.
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

P1, 2026-09-17, release: before is a237949; after is this worktree. Each entry
is the median of three run summaries (p50 / p95 milliseconds); load1 lists the
three runs in order. Final benchmarks started after our builds completed. The Mac
was shared and busy; Linux supplies the CPU-only comparison.

The cubes diagnostic uses 60 warmup + 240 frames, 2560×1440, 4× MSAA. The 1%
case moves a contiguous prefix, plus the camera; scattered movers can touch every
page. All-moving disables same-value filtering explicitly; the old adaptive policy
also normally skipped hashing there. Sparse/still cases retain filtering.

| Mac GPU feed | Before ms | Before load1 | After ms | After load1 |
|---|---:|---|---:|---|
| 200,000, all moving | 1.0662 / 1.3807 | 19.63/19.63/19.63 | 0.9591 / 1.1879 | 18.10/18.10/18.10 |
| 200,000, 1% moving | 0.2752 / 0.4076 | 18.62/18.62/18.62 | 0.1634 / 0.2915 | 17.29/17.29/17.29 |
| 200,000, camera only | 0.2630 / 0.3597 | 18.62/18.62/18.62 | 0.0567 / 0.1143 | 17.29/17.29/17.19 |
| 500,000, all moving | 2.9601 / 3.5473 | 18.62/17.61/17.40 | 3.9434 / 4.8623 | 17.19/17.19/16.69 |
| 500,000, 1% moving | 0.6926 / 0.8115 | 17.40/17.40/16.49 | 0.1284 / 0.1638 | 16.69/16.16/16.16 |
| 500,000, camera only | 0.7176 / 0.9204 | 16.49/16.49/16.49 | 0.0643 / 0.0988 | 16.16/15.74/15.74 |

The still-camera target passes. The all-moving 500k GPU target remains unmet:
the final measured median is slower than baseline on this shared Mac. The CPU
recording backend below excludes wgpu, copies the same feed writes into retained
arrays, and makes no claim about GPU submission latency. Its setup fresh list is
cleared before timing; the initial diagnostic that failed to clear it was discarded.

| Linux CPU feed | Before ms | Before load1 | After ms | After load1 |
|---|---:|---|---:|---|
| 200,000, all moving | 0.5006 / 0.9388 | 1.20/1.33/1.52 | 0.4371 / 0.4483 | 1.72/1.66/1.88 |
| 200,000, 1% moving | 0.4431 / 0.4501 | 1.20/1.33/1.52 | 0.0136 / 0.0137 | 1.72/1.66/1.88 |
| 200,000, camera only | 0.4395 / 0.4458 | 1.20/1.33/1.52 | 0.0063 / 0.0063 | 1.72/1.66/1.88 |
| 500,000, all moving | 1.6043 / 2.7648 | 1.20/1.33/1.52 | 1.6659 / 1.7357 | 1.72/1.66/1.88 |
| 500,000, 1% moving | 1.1081 / 1.1204 | 1.20/1.33/1.52 | 0.0287 / 0.0291 | 1.72/1.66/1.88 |
| 500,000, camera only | 1.0985 / 1.1067 | 1.20/1.33/1.52 | 0.0091 / 0.0091 | 1.72/1.66/1.88 |

Latest paired renderer diagnostics: 200k cubes, effects off, 1280×720, CPU encode
0.2429 ms, tick upload 1.4430 ms, GPU-completed frame 2.8317 ms. Shadowed Beacons
GPU frame 0.5214 ms, CPU encode 0.2570 ms. These are medians of three run summaries;
shared-machine variance precludes a tight speedup claim. Optional GPU timestamp
intervals overlap on Metal: **do not sum them**. `GPU_PASS_NAMES` maps sixteen
query pairs; disabled passes leave theirs unwritten. Resolve after completion;
invalid/reversed pairs are NaN.

## U1 UI/default-look measurements (2026-09-17)

Paired local release executables, original source versus U1, alternating order,
three runs each, 60 warmup + 240 measured frames at 2560×1440. Effects are off.
Medians of run p50s below; this shared Mac varies substantially. The 500k
numbers rose, including simulation time where the render change adds no tick
work, so these measurements **do not establish the no-regression gate**.

| Cubes | Motion | Feed before / after ms | Encode before / after ms |
|---|---|---|---|
| 200,000 | all | 1.3054 / 0.9518 | 0.0669 / 0.0445 |
| 200,000 | one-percent | 0.0500 / 0.0511 | 0.0392 / 0.0391 |
| 200,000 | still | 0.0313 / 0.0270 | 0.0406 / 0.0357 |
| 500,000 | all | 2.7262 / 2.9332 | 0.0743 / 0.0841 |
| 500,000 | one-percent | 0.0925 / 0.1198 | 0.0508 / 0.0683 |
| 500,000 | still | 0.0415 / 0.0485 | 0.0420 / 0.0515 |

The Beacons fixture (`beacons_designed_defaults`) now renders the lit scene,
asserts saved grid spacing survives restore, and compares it against the same
scene without the grid. Before/after inspection: the grid gives scale and depth;
fog softens distant crates and the ground into the horizon; bloom is restrained.
The victory overlay is a full-canvas centred Contract column.

## Reproduce

From `game/`, with `EXACT_UPDATE_TRUST=development`:

```sh
cargo build --workspace
cargo test --workspace --no-fail-fast
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
cargo build -p greybox-gpu --profile web --target wasm32-unknown-unknown
cargo run -p exact-game-render --release --example cubes -- 500000 240
cargo run -p exact-game-render --release --example cubes -- 500000 240 one-percent
cargo run -p exact-game-render --release --example cubes -- 500000 240 still
cargo test -p exact-game-render --release --lib feed_cpu_cost -- --ignored --nocapture --test-threads=1
cargo test -p exact-game-render --release -- --ignored --nocapture --test-threads=1
bun games/greybox/proof.mjs web
bun games/beacons/proof.mjs web
bun games/greybox/proof.mjs macos
```

GPU tests explicitly skip without an adapter; geometry and shader validation still
run. Set `EXACT_GPU_OUT` for image artifacts. The current game proofs pin native
and browser hashes. [Game README](../README.md) covers clock, save and dev carry;
[ergonomics diary](../diaries/002-ergonomics.md) retains experiment history.
