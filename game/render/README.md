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
64-byte affine attachments. Writes and batch changes return named `RenderError`s before
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

Sixteen primitive/effect variants compile at renderer startup: eight forward, two
shadow, one sky, two tone and three bloom. This is startup work, not per-frame work. The
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
only. There is no filtering switch or dense/probe/adaptive-skip policy.
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
texture bytes for upload. Entity transforms and socket attachments select winding
from the composed determinant together with the baked node transform. Adjacent
instances with different winding split into separate draw ranges.
Five texture slots (base colour, normal, metallic-roughness, emission, occlusion)
share three 1×1 default views and cached samplers. Named textures are shared across
materials and models, uploaded once per renderer, and released from CPU memory. Colour/emission textures use sRGB texture formats; data maps are
linear. Mips arrive baked with authored nearest/linear filters and wrap modes; fully linear
samplers use 4× anisotropy. Only MASK/BLEND base-colour filtering weights RGB by
alpha; opaque and emissive maps average straight RGB. MASK mip coverage is
retained to the nearest texel. Normal mapping derives a cotangent frame from screen-space world/UV
derivatives; baked models contain no tangent arrays. Material UV transforms apply separately to every texture.

Opaque batches stay retained. Only transparent draws are sorted each displayed
frame, back-to-front in camera depth, using retained tick poses and local centers.
They keep depth testing, disable depth writes, and do not cast shadows. A model's
own materials are multiplied by entity base colour and have entity emission added.
The environment's hemisphere approximation supplies ambient metallic reflection;
this is not image-based lighting.

Camera/sun/point rotations use normalized linear interpolation histories. The first posed sun
wins. Point-light selection is feed-only: up to sixteen with positive tick-end
intensity, ordered by squared camera distance then entity index. `Lit` contributes
its nonnegative tick-end multiplier to eligibility. There is no incumbent advantage
or saved selection; continuous feeds, long seeks and restores select the same order.
At the sixteen-light boundary, two lights exchanging distance order can visibly
pop between included and excluded; there is no hysteresis or crossfade.

`Lit(Spring)` on a point light samples the saved spring at presentation seconds and
multiplies `PointLight.intensity`; negative overshoot clamps to zero. Keep the authored
intensity constant and retarget once with `lit.to(now, 1.0)`. Sampling changes no
world bytes. `Glow(Tween)` independently controls material emission. The GPU regression
in `tests/world.rs` measures a non-emissive cube illuminated by a meshless spring light.
Engine illuminance is lux: 10,000 lux maps to renderer radiance 3.
Missing materials/environment use defaults.

Performance samples appear only in `state.world.perf`: live frame stamps, tick,
feed, encode (frame input through submit) and ticks/frame distributions. CPU sample
rings retain 16,384 values. Seekable renders, agent advances and timed binds make
no perf clock calls; samples do not enter hashes. The allocation-free claim covers
only the `steady_sim_feed_and_frame_inputs_allocate_nothing` moving-cube/camera/light
fixture (including trace recording), after warmup,
without input edges, structural churn, audio or physics. wgpu owns its command and
staging allocations; the claim does not include those. Animation output, owner-chain
construction or hierarchy growth, physics event vectors, audio voice sorting and
particle derivation/sorting are also outside this claim. Owner chains reuse capacity
on steady feeds. Attachments share their owner history and delivered model identity;
model parts share one entity pose history. Attachment matrices use one dense upload
per frame, including removal of stale overrides, through the highest attached slot.
Compaction remains delivery-frame work with a separate latency and transient-memory
cost. The first emitter also prepares pipelines during its feed; that work belongs
in the frame budget even though it is outside drawing.

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

R4 repairs the web probe by copying the original save before replacing the mesh
name, then restoring that copy. It never carries the temporary pending world.
KeyR, KeyC (changed bytes and identical redelivery), and KeyP (pop-in) run in both
web fixture proofs. Steady GPU residency reports `SKIP: no device` headlessly;
device-backed runs must be ready and record zero after-ready work in the named
counters. The native Fox test separately enables paranoid Save.
Model digests are computed at delivery and stored with the named resident identity,
so texture arrivals do not re-hash resident models.

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
pipelines prepare with the first emitter; particle arenas reserve the emitter’s steady stream and grow for larger birth
counts at feed preparation, never in the draw loop. The 65,536 ceiling is admission
policy, not compulsory storage. Sprite pipelines prepare with the asset-capable
renderer before its ready boundary.

## Placed children

`Placed` feeds displayed plane geometry to the host's child-composition seam.
Browser children use CSS homographies; native children share the quad pass and
world depth. Side visibility clips the whole polygon against the viewport. CSS
hides a near/eye-plane crossing; native submits the quad to the clip volume and
Linux's software warp/hits apply the same near/far planes. A walk-up nameplate
therefore keeps its visible portion on native/Linux. Socket attachments retain the
full affine matrix from interpolated local joints, including non-uniform-scale shear.
The GPU palette oracle compares every matrix element at alpha 0, 0.5 and 1; a pixel
regression compares an attached cube with directly transformed vertices.
Simulation goldens live in each fixture's [pins.json](../games/skinned-fixture/pins.json),
not in this README.
See [placement geometry](src/placed.rs), [tests](src/placed_tests.rs), and the
[fixture](../games/placement-fixture/README.md). `ChildrenMode` distinguishes
host overlay, a composite subtree with optional history, and individual children.

Dated measurements and the sole module-size table are in
[bench/README.md](../bench/README.md). Residency numbers bound retired content;
live content is not a total GPU-memory budget. GPU tests report when no adapter
is available; a headless proof establishes simulation evidence, not pixels.

Generated `World::generated` models use the existing model preparation and
geometry/material/winding batches. Repeated handles share one CPU model, one GPU
upload and one draw group; preparation reuses its content digest across frames.
`MeshData.colors` is optional linear RGBA. Both model and primitive vertex paths
multiply the material base colour; model alpha also multiplies vertex alpha,
including masked shadows. Primitive materials remain opaque. The packed public
`Vertex` is 48 bytes including colour, with white in built-in shapes; skin weights
retain their separate 32-byte stride. The asset fixture constructs a seeded coloured
heightfield and three shared rocks in setup, with no runtime texture generator.
