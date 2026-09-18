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
values. Model node determinant parity selects the matching front face in forward and shadow passes. WebGPU depth is 0–1, near zero;
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

Eleven primitive/effect variants compile when the renderer is constructed. The
model family is lazy: two shared shader modules and three shared pipeline layouts,
with only the material/winding variants needed by arrived models. All four
shadow/fog combinations for each used forward variant are prepared during asset
delivery; changing effects during play never compiles a model pipeline. Declared
content becomes Loaded independently; device readiness waits for preparation and
texture uploads before drawing. Disabling effects skips their passes and releases their attachments.
HDR/depth/bloom attachments grow in 64-pixel buckets; shrinking reuses them.
Viewports and post-pass UVs respect logical size. Discarded 4× MSAA colour/depth
attachments request transient storage (a no-op where unsupported). ACES-fitted
tonemapping applies sRGB transfer once. HDR reads sanitize NaN and clamp to
[0,65472]; bright-pass sanitization precedes filtering.

## WorldSurface and feed

A primitive game's GPU shell is `exact_game_render::module!(MyGame)`; `game.assets:
true` selects `module!(MyGame, assets)` and its concrete model adapter. The primitive
module links neither model decoding nor the model shader family. Bind constructs Sim;
the first asset preparation or render constructs Renderer. Feed setup and only the last two completed ticks
of a seek. Frames interpolate on the GPU and visit retained camera/light/batch
records, without per-instance CPU work on the primitive retained path.

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
separate instance buffer. Rebatching retains its word scratch and caches immutable
node normal matrices in a cache bounded by the live draw records. A material's
final texture binding is created once all of its dependencies have arrived. Primitive records retain their compact identity encoding
in the existing slot lists: transform/material = slot, geometry = batch, offset =
identity. A primitive world binds no model group and samples no material texture.

Model materials use a separate forward pipeline and alpha-tested shadow pipeline,
with opaque/mask/blend, culled/double-sided and mirrored variants prepared before
the prepared surface draws. Device loss preserves Loaded content and re-requests
texture bytes for upload. Negative-determinant entity-global transforms refuse by
asset name; baked mirrored nodes use the prepared winding variant.
Five texture slots (base colour, normal, metallic-roughness, emission, occlusion)
share three 1×1 default views and cached samplers. Named textures are shared across
materials and models, uploaded once per renderer, and released from CPU memory. Colour/emission textures use sRGB texture formats; data maps are
linear. Mips arrive baked with authored nearest/linear filters and wrap modes; fully linear
samplers use 4× anisotropy. Only MASK/BLEND base-colour filtering weights RGB by
alpha; opaque and emissive maps average straight RGB. MASK mip coverage is
retained to the nearest texel. Normal mapping derives a cotangent frame from screen-space world/UV
derivatives (including models without tangents); baked tangents are retained in
model data but are not uploaded. Material UV transforms apply separately to every texture.

Opaque batches stay retained. Only transparent draws are sorted each displayed
frame, back-to-front in camera depth, using retained tick poses and local centers.
They keep depth testing, disable depth writes, and do not cast shadows. A model's
own materials are multiplied by entity base colour and have entity emission added.
The environment's hemisphere approximation supplies ambient metallic reflection;
this is not image-based lighting.
At 200k slots materials cost 9.6 MB, two transform histories 16 MB.

Camera/sun/point rotations use normalized linear interpolation histories. The first posed sun
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

## Residency across restore

`Models.loaded[name].digest` and `Models.textures[name].digest` use the engine's
`hash::of` over the complete decoded content, including bulk geometry/mip bytes.
Equal name/content retains GPU handles and prepared pipelines. Changed content
under the same name uploads the replacement and updates its bindings. The hash
is the engine's noncryptographic content hash, not an authentication digest.

`presentation_generation` invalidates world-derived transform histories, material
pages, draw records, instance lists and skin pose histories. It does not invalidate
asset residency or palette capacity. `Feed::reset` clears old entity identities
before inspecting the replacement world. Retiring host requests deactivates their
content without resetting unrelated entities' histories. Re-requested Pending names
remain retired until their bytes are digest-accepted. Device loss, format change
or full module replacement still requires a new renderer.

The 64 MiB retired budget charges texture mip dimensions and allocated buffer sizes,
including unused mesh-arena capacity. Compaction drops retired loaded entries and
orphan material/skin slots while keeping live pipelines, meshes and pose histories.
Same-name replacement reuses the old slots after digest acceptance. Only shared
mesh-arena pressure triggers GPU-to-GPU packing; live meshes are not re-uploaded
from CPU data. Live content is not bounded by this retired-content budget.

`state.world` includes `{ready, readyReasons, gpu: {beforeReady, afterReady,
bufferScope}}`. Each work record has `textureUploads`, `meshUploads`,
`pipelineCreations`, and `modelSkinBufferReallocations`. The first completed drawable frame
after preparation fixes `beforeReady`; later work accumulates in `afterReady`.
Restore and retirement do not reset that boundary. A device replacement resets it; budget/replacement compaction preserves it.
Texture counts include the three default maps; mesh counts include primitives;
pipelines include primitive/effect, model, skin and quad pipelines.
`bufferScope` explicitly limits reallocation counts to model instance, weight,
hierarchy, local-pose, job, palette and quad buffers; core transform/geometry arena
instrumentation is outside this slice. `ready` is exactly an empty `readyReasons` set; named declaration and render
failures are included. Headless state has `ready: false`, reason
`no device`, and zero GPU work; it is simulation evidence only.

Both asset fixture proofs exercise same-device restores after ready and compare
work across ordinary and paranoid ticks. Their web-only response instrumentation
records browser `performance.measure('a3-restore')` through the first draw. A
reference round trip retires/re-requests the original texture on the same device,
serves changed pixels, and carries the original scene back: exactly one texture
uploads. Repeating those bytes uploads nothing. The negative control restores a
mesh under a new undeclared name: its geometry uploads while shared textures and
pipelines remain resident. The native `residency_tests` also exercise changed
texture delivery and `Restore::Carry` directly on a device-backed Fox surface;
`content_digest_reuses_equal_bytes_and_replaces_changed_names` covers changed
model geometry, texture color space and sampler state. Simulation pins are unchanged.

A3 measurements, 2026-09-18, web at 1280×720, Fox tick 45/hash
`0x1f9f91652dc258e6`, 20 same-device restore-through-first-draw samples after one
warmup: p50 **0.60 → 0.50 ms**, p95 **1.00 → 0.90 ms**. Browser clock quantization
and shared-machine load make this a diagnostic, not a demonstrated speedup. The
starting HEAD already retained assets on a same-scene restore; A3 adds digest
replacement and removes whole-renderer eviction on reference retirement. The
post-change work is exactly zero for all four recorded categories. Raw samples:
`/tmp/a3-baseline.log`, `/tmp/a3-after.log`.

R4 repairs the web probe by copying the original save before replacing the mesh
name, then restoring that copy. It never carries the temporary pending world.
KeyR, KeyC (changed bytes and identical redelivery), and KeyP (pop-in) run in both
web fixture proofs. Steady GPU residency reports `SKIP: no device` headlessly;
device-backed runs must be ready and record zero after-ready work in the named
counters. The native Fox test separately enables paranoid Save.
Model digests are computed at delivery and stored with the named resident identity,
so texture arrivals do not re-hash resident models.

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

The tables above are historical diagnostics with their stated revisions and sizes.
They do not establish asset-change performance. S3a's commit-message cube numbers
used a different run and are not a comparison against these tables. Optional GPU
timestamp intervals overlap on Metal: **do not sum them**. `GPU_PASS_NAMES` maps
sixteen query pairs; disabled passes leave theirs unwritten.

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

## S3a-b asset measurements (2026-09-18)

DamagedHelmet's pinned input now produces an 885,161-byte model and five
22,369,762-byte textures (each below the 64 MiB carrier limit). Raw RGBA8 still
uses about 112 MB in total; splitting changes delivery and CPU lifetime, not GPU
compression. BC7/ASTC is queued.

Three fresh web sessions per revision, 1280×720, local delivery: Play through a
completed screenshot of the loaded helmet took 774–873 ms before (median 817 ms)
and 266–415 ms after (median 301 ms), a 515 ms / 63% median reduction. This is an
end-to-end wall-clock diagnostic, including the screenshot; it is not scanout.
Before/after screenshots were byte-identical. The old first-frame field counted the loading frame, so comparing that field
would give a false result. It now waits for declarations to become ready.
Primitive-only worlds create zero model pipelines, shader modules, layouts,
instance buffers or textures; the GPU peer test checks that allocation boundary.

Paired 200,000-cube release executables, alternating order, three runs per side,
60 warmup + 240 measured frames, 2560×1440, effects off. Medians of run p50s:

| Motion | Feed before / after ms | Encode before / after ms |
|---|---|---|
| All | 1.0363 / 1.0130 | 0.0842 / 0.0778 |
| 1% | 0.0765 / 0.0597 | 0.0892 / 0.0578 |
| Still | 0.0893 / 0.0882 | 0.1535 / 0.1502 |

No regression exceeds observed run variation. The 1% feed ranges were
0.0493–0.1128 before and 0.0560–0.0931 after; these are shared-machine diagnostics,
not a general speedup claim. The workspace also contains concurrent live-clock
work; this diagnostic uses seekable ticks. Raw run logs are
`/tmp/s3ab-cubes-{before,after}-{all,one-percent,still}-{0,1,2}.log` and
`/tmp/s3ab-play-{before,after}.log`.

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


`Environment` is re-exported from the engine. A frame carries it once, including
`frame.environment.exposure` and `.bloom`. Render failures are either
`RenderError::Capacity { arena, slot, limit }` or `RenderError::Scene(reason)`.
Mesh centers support transparent sorting; there is no unused sphere-radius API.
World surfaces retain small work counters by default. A world-state request with
`perf: true` or `perf_reset: true` arms the five 16,384-sample diagnostic rings;
`perf.armed` distinguishes recorded percentiles from the unarmed zero values.
Arming preserves accumulated counts, means, maxima and cadence; only `perf_reset`
clears them. The feel probe requests `perf: true`.

S3a-c measurement (2026-09-18): the retained Beacons GPU wasm is 811,977 bytes
before this pass and 758,561 after (gzip 322,782 → 301,524). Model shader and
validation markers are absent from Beacons and present in the asset fixture.
The roughly 490 KB pre-assets target is **not met**; concurrent D2 changes also
contribute to this comparison. Three interleaved 200k-cube runs against the retained
S3a-b binary have overlapping ranges: all-moving median tick/feed/encode is
0.4342/1.0230/0.0723 ms before and 0.4443/1.0524/0.0723 ms after. This is an
offscreen diagnostic, not an FPS claim or an isolated HEAD comparison. Native
replacement-device pixels match: retained bytes are re-uploaded and pipelines
re-prepare. This verifies module-level recovery. **Host recovery is owed.** Apple
result 3 makes canvases non-presentable; native `gpu_load` destroys the module’s
surface table. A recovery ABI must request a replacement device while preserving
that table. Web recovery retains the old instance’s presentation surface/context
and presents black; it must recreate each canvas surface/context on the new device
as the module-swap path does. Both reviews trace this: `review-S3ac-sol.md`
(`gpu/src/native.rs:224`, `CanvasSeams.swift:353`, `gpu/src/lib.rs:358`,
`gpu-glue.js:148,583`) and `review-S3ac-grok.md` (`gpu/src/web.rs:33–60`,
`gpu/src/native.rs:90–99`). The asset proof asserts state, re-fetch and hash and
prints “host recovery owed”; native pixels remain a required module test.

Also owed: Apple delivered-`.tex` bytes retained by the generation store (move to
private files; Sol, `Session.swift:214`, `PlanURL.swift:583,604`); primitive-module
size isolation (a primitive world still owns the asset maps and `Models`; 759 KB
against the ~490 KB target — the size question needs a link map, its own slice;
Sol, `asset.rs:374`, `sim.rs:82`, `renderer.rs:22,245`; Grok §Primitive module vs 759 KB).

## Skinned model path

Model-capable modules upload four joint indices/weights per vertex and a skin
handle per draw record. Primitive modules create no skin buffers or pipeline.
The feed copies the saved previous/current **local** TRS into retained buffers on
completed ticks, even when the entity Transform did not move. Rendering allocates
no new collections for these histories. Skin templates retain parent-first node
order without changing glTF's joint indices.

One compute workgroup per skinned draw interpolates local translation/scale and
shortest-path quaternion rotation at frame alpha. Lanes compute locals in parallel;
one lane composes the parent-first hierarchy, then lanes multiply joint world
matrices by inverse binds. The shared array specializes to the largest loaded rig's
next power of two (32 nodes for Fox), bounded at 256. Forward and shadow vertices
read the same palette. Normals use the inverse transpose of the blended skin
transform, preserving nonuniform/animated scale and hierarchy shear. A singular
blend has no inverse and falls back to the authored normal. A GPU test executes
the actual vertex skinning function on scaled, rotated joints and compares it with
the CPU inverse transpose.
No composed-matrix interpolation, CPU per-frame palette construction, bone entities,
or transform writes are involved. The optional seventeenth GPU timestamp pair is
`skin palettes`; as with other Metal timings, intervals are not additive.

The compute regression distinguishes a quarter-turn interpolation from a lerp of
composed matrices and checks inverse binds. The warm local-pose packing path has
an allocator-counting regression. Saved pose histories survive restore, while the
presentation buffers prime current/current on restore, carry, teleport and model or
batch arrival. Initial feeds propagate the same reset signal to skinning and entity
histories. The Fox pixel regression compares birth, restore, carry and model arrival
with an explicit current/current oracle at zero changed pixels per event in the
affected rectangle (channel differences up to 2 are ignored); its
injected bind-history control detects a flash without diluting it in the background.
The packing test checks exact local arrays and confirms priming does not mutate saves.

Original S3b measurements before the reviewed fixes, Apple M5 Max / Metal, 2026-09-18:

| Measurement | Result | Budget |
| --- | ---: | ---: |
| 100 Foxes, 24 joints, three-knot blend, live tick mean over 600 ticks | 0.182969 ms | <0.3 ms |
| 100 palettes, GPU p50 / p95 | 0.046875 / 0.052750 ms | p50 <0.1 ms |
| Warm pose packing, 300 feeds, counted Rust allocations | 0 | 0 new allocations |

The CPU number reruns the retained release fixture binary (the earlier run was
0.177966 ms); its skeleton code is unchanged. The GPU diagnostic was rebuilt during
the resume; the earlier p50/p95 was 0.025208/0.049875 ms. The allocation assertion
covers retained local-pose packing, not wgpu's command submission internals. The
existing timing rings count time/work, not allocations.

Three interleaved before/after pairs, 200k cubes, 240 measured frames after 60 warmup
frames, 2560×1440 and 4×MSAA (median of each run's p50, milliseconds):

| Moving cubes | Tick before → after | Feed before → after | Encode before → after |
| --- | ---: | ---: | ---: |
| All | 0.4509 → 0.4640 | 1.1572 → 1.2014 | 0.1060 → 0.1208 |
| 1% | 0.0047 → 0.0052 | 0.0520 → 0.0531 | 0.0419 → 0.0455 |
| None | 0.0009 → 0.0005 | 0.0357 → 0.0300 | 0.0527 → 0.0442 |

The ranges overlap in every column/mode; this shared-machine diagnostic finds no
resolved regression and is not an FPS claim. No isolated same-app skinning size
measurement is available, so there is no skinning engine-growth claim here.
The earlier comparison used different games and has been removed. Historical
logs: `/tmp/s3b-resume-*`; paired baseline artifacts: `/tmp/s3b-before-*`.

R3 adds an in-place same-name model replacement regression: changed content
rebuilds GPU hierarchy/inverse binds, geometry/material handles and both sharing
entities' batches with primed history. Equal content reuses the prepared model.
This exercises the digest-based replacement already present at 4b40b165.

## P1 particle and sprite rendering

Particles and sprites use retained CPU derivation and separate 80-byte quad
instance vertex arenas beside the entity/model draw-instance records. A thousand
sparks add no entities and no DrawInstance records. Feed retains emitter state
and previous/current transforms at ticks; a frame derives local particle motion,
transforms it, sorts the translucent entries and uploads contiguous instances.
Compatible neighboring particle entries coalesce across emitters into one draw;
a sprite/model interleaved by depth splits that batch. The particle fixture's
20,000 instances use one particle draw plus tonemapping. No frame scans the world.
Emitter state includes compact admitted-birth batches, never particle positions.
The renderer never changes that saved state.

Camera, sprite and blended-model ordering use the same normalized shortest-path
quaternion interpolation as the draw shader. Birth/restore/carry/teleport/parent
changes prime transform history. Emitter birth history is already saved and is
not reset during that priming. A model's sort center is transformed by the
interpolated pose, rather than interpolating its two transformed endpoints.
The camera's projection function is shared with layout/pick. Orthographic integer
scaling consumes CSS viewport dimensions through Feed::frame_pixels, matching
agent geometry even at noninteger heights on a 2× display. Integer display scales
keep texels on whole device pixels; fractional display scales can give uneven
physical widths with nearest sampling.

Opaque primitive/model rendering is unchanged. Opaque and masked sprites write
depth; the ordered translucent pass follows the sky, depth-tests and does not
write depth. Ordering is descending view depth, ascending layer and entity slot,
then kind rank and stable per-owner ordinal. Models use layer zero. Sprites and particles cast no
shadows. Soft circular particles choose additive or straight-alpha blending;
sprites share `.tex` samplers and choose opaque, mask or straight-alpha blend.
Billboard extents use scale magnitudes to preserve front-facing winding; texture
mirroring is Sprite.flip. Texture-free particles are available to primitive
modules. Sprite texture upload and shaders live only in the asset-capable path;
the measured primitive wasm contains neither the sprite texture shader marker nor
the sprite texture binding label, and a GPU test verifies named Sprite refusal.
Retirement removes a sprite's drawable binding while retaining its resident
texture for digest-checked redelivery. Compaction preserves only active bindings.
The residency counters include quad pipelines and quad buffer growth. Particle
pipelines and the full 65,536-particle arena prepare with the renderer; sprite
pipelines prepare with the asset-capable renderer before its ready boundary.

### Measured on 2026-09-18

Live headless Chrome, WebGPU, 1280×720 physical pixels, 6.5-second warmup, on the
shared arm64 Mac. The sprite run holds D throughout measurement; its camera and
parallax layers move. `measure.mjs` requests WebGPU timestamp-query support and
instruments actual render-pass boundaries. GPU values sum the forward and ACES
pass intervals, excluding presentation/scanout. CPU encode includes particle
derivation, sorting, uploads, encode/submit and the timestamp instrumentation;
it is not total frame wall time. Tick/feed columns are per-sample means. Browser
clock quantization produces zero p50 tick/feed samples; those operations are not
free. These are single shared-machine runs, not isolated performance guarantees.

The previous exact timing and size tables were removed: their referenced receipts
were absent from the tracked tree. They cannot support a reproducible claim.
The R6 receipts retain every sample, base commit `e0ab1904`, measured build input/
artifact digests, and hardware (Apple M5 Max, 128 GiB, macOS 26.6.2). They measure
the uncommitted R6/U1 working build, not an isolated committed revision.

| Fixture | Instances | Draws | Encode p50 / p95 ms | GPU p50 / p95 ms |
|---|---:|---:|---:|---:|
| [20 × 1,000 particles](../bench/results/r6-particles-fixture-perf.json) | 20,000 | 2 | 2.80 / 3.50 | 0.283 / 0.312 |
| [moving sprite strip](../bench/results/r6-sprites-fixture-perf.json) | 264 | 6 | 0.20 / 0.30 | 0.289 / 0.298 |

The old 3.6 ms particle encode claim lacked its receipt; the new 2.80 ms median
is a measured value, not a controlled speedup ratio. Compatible sprite runs reduce
the strip from 67 draws to 6. Both captures report zero after-ready texture/mesh
uploads, pipeline creations and tracked buffer reallocations.

Reproduce from the repository root with the required build environment:

```sh
export DEVELOPER_DIR=/Library/Developer/CommandLineTools
export SDKROOT=/Library/Developer/CommandLineTools/SDKs/MacOSX26.sdk
export EXACT_UPDATE_TRUST=development
export EXACT_IDENTITY=-
bun game/games/particles-fixture/proof.mjs web
bun game/games/particles-fixture/proof.mjs linux
bun game/games/sprites-fixture/proof.mjs web
bun game/games/sprites-fixture/proof.mjs linux
bun game/games/particles-fixture/measure.mjs particles-fixture
bun game/games/particles-fixture/measure.mjs sprites-fixture
EXACT_APP_DIR="$PWD/game/games/beacons" \
  EXACT_WEB_DIST="$PWD/game/games/beacons/target/d3-size/dist" \
  CARGO_TARGET_DIR="$PWD/game/games/beacons/target" bun host/web/build.mjs
bun game/bench/size.mjs p1-recheck --no-build
```

The two fixture proofs match web/Linux tick-300 hashes and alive counts and save/
restore at tick 150. Sprite web pixels check four leaves behind and three in front
of the animated character. Native device tests cover equal-depth layer/slot order,
mask cutoff, negative scale, sprite retirement/redelivery, and exact pixels after
Open/Carry. The macOS screenshot remains owed: after two SDK linker failures, the
third build used MacOSX26 successfully for Rust but SwiftPM failed on a missing
BuildServerProtocol symbol. The three-round limit stopped that host loop.

Sprite scale uses magnitudes; `Sprite.flip` changes UV orientation. `Sprite.layer`
affects blended ordering only; Opaque/Mask use the depth buffer. Adjacent compatible
sprite runs coalesce without crossing intervening translucent records. Translucent
keys are depth, layer, entity slot, kind (model, sprite, particle, placed child), then
stable per-owner ordinal. Particle trajectories use the emitter's **current**
parameters and transform: local-space smoke. Detached sparks would retain spawn
pose/configuration per birth batch; that behavior is not built.


## Placed children (U1, 2026-09-18)

Plane geometry is `Placed::project` in the engine. The render adapter retains
child frames and texture views, follows the same displayed camera/entity histories
as sprites, and exposes `Placement {homography, depth, hidden}`. The closed-form
homography uses the clip-space top-left corner and its right/down differences;
no matrix solve is needed. A hand-computed square regression checks all four
mapped corners. Near-plane, off-canvas and Fixed back-face hiding return an
explicit hidden placement, never None. Invisible owners keep that hidden outcome.
There is no saved presentation cache.

`Placement.depth` is negative view distance (larger nearer). `Quads::frame`
converts it once to positive view distance with `-placement.depth` for the shared
back-to-front translucent order. UI uses an explicit `Kind::Child(u16)` identity;
its owner key is the Contract child index, so equal-depth children draw in the
same order that Apple inverse-hit-testing and browser z-index use. Captured child
views share the quad texture lifetime, arena submission and ordered pass, with
premultiplied blending and depth testing. This native capture path is present in
primitive modules as well as model-capable modules. It is compiled out on wasm,
where the host supplies frames without textures and composites the real elements.
Unplaced children, including the HUD, remain ordinary host UI. A HUD-only world
returns false from `wants_children_each` and pays no per-child capture cost.

Measurements on the shared Mac, 2026-09-18:

| Measurement | Result | Scope |
| --- | ---: | --- |
| Plane geometry + homography + visibility + ABI adapter | 40.594 ns/child/frame | 400,000 release calls, `homography_cost` ignored diagnostic |
| Fixture tick CPU mean | 0.023889 ms | 180 live browser frames, 1280×720, DPR 1 |
| Fixture feed CPU mean | 0.074444 ms | Same sample |
| Fixture renderer encode p50 / p95 / mean | 0.100000 / 0.200000 / 0.115556 ms | Same sample; excludes DOM composition |
| Fixture GPU p50 / p95 | 0.508701 / 0.784862 ms | WebGPU timestamps, forward + ACES; no timestamp errors |
| Frame interval p50 / p95 | 16.666040 / 16.667620 ms | Browser callback cadence; not GPU time or scanout |
| Retained baseline module raw / gzip | 858,785 / 358,320 bytes | `bun game/bench/size.mjs u1-before --no-build` |
| Final shared-tree module raw / gzip | 888,665 / 370,716 bytes | `bun game/bench/size.mjs u1-after` |
| Module delta raw / gzip | +29,880 / +12,396 bytes | Includes concurrent R6 changes; not isolated U1 attribution |
| macOS capture of 40 name-plates | unavailable | Swift package loader aborts before compilation |

The browser sample is `games/placement-fixture/artifacts/perf-web.json`; size
attribution is `games/beacons/target/d3-size/u1-{before,after}.json`. The baseline
was the retained P1 build, not a fresh isolated checkout. The fixture's `--capture40`
mode prepares forty real Contract text children for the native capture log. The
three-round native attempt stopped at a missing `BuildServerProtocol` symbol in
`swift-package`, including with the required temporary native-build wrapper;
there is no measured macOS capture number, tap proof or accessibility result yet.
The pure native GPU pixel test passes and covers premultiplied overlap, equal-depth
Contract order, opaque wall occlusion and explicit near-plane hiding.

The fixture's browser proof passes placed layout, camera orbit, Pull → lamp/HUD,
back-face hiding and failed hidden tap, save/restore and a stationary HUD. Its
Linux proof covers state, ordinary Contract input and save/hash parity, not placed
composition: Linux's headless host has no child-placement consumer, and extending
its painter/surface scope awaits clarification. Socket attachments use the existing
interpolated tick-resolved socket follower transform; displayed bone/socket
recomposition is not implemented by this slice.

Pins exercised without U1 editing the existing fixtures' pins:

| Fixture | Tick / checkpoint | Hash |
| --- | --- | --- |
| Greybox | setup | `0x9a871d8582d905e7` |
| Greybox | W for 1500 ms | `0x71f43e51a13cc49f` |
| Beacons | 907, three lights | `0xce6c7b72a5ced1e2` |
| Asset | 60 | `0xb1365b0eb9a7c59d` |
| Skinned | 60 | `0x749639d3ffa1be59` |
| Skinned | 120 | `0x0960f8999dd20662` |
| Particles | 300 | `0x8dc0cac2d2645d93` |
| Sprites | 300 | `0x2e3d805eb6c89e55` |
| Placement | 330 | `0x61007363bd681d3c` |

Final verification: game workspace tests **516 passed, 11 ignored**; all-target
clippy and game/root fmt pass. Root `cargo build --workspace` passes. Web tests
**51 passed**; the native device-free ABI test confirms 0/1/2 and untouched output
on hidden. Greybox, Beacons, asset, skinned, particles and sprites proofs pass on
web and Linux. The sprites web build first met a concurrent Rust-edition error;
the retry after the required wait passed. The placement proof passes on web and
Linux with the limitations above. Caps passes with only explicitly named U1 files
staged in a temporary index, removed afterward; the shared index is untouched.
Logs are `/tmp/u1-*.log`. No commit, clone, stash or sub-agent.
