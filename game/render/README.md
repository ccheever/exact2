# exact-game-render

PBR over slot-indexed floats and separate model draw instances. `WorldSurface<G>` connects a simulation to an
Exact canvas; the simulation crate owns no GPU or host. Each frame a GPU pass culls
every opaque item against the camera and each shadow cascade; each geometry pass
then issues one indirect draw per mesh batch (and winding) over its view's survivors.
See [Culling](#culling).

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
the submitted forward scene before culling; `draws` includes engine-issued draws across
all passes (an indirect draw counts even when the GPU keeps nothing); `texture_creations` is cumulative.
CPU timing belongs to the caller; `draw` makes no performance clock calls.

## Game render hooks

`WorldSurface<G, P, ASSETS, H>` accepts `H: Hooks`; `()` preserves the standard
frame. `module!(Game, hooks = Effects, shaders = SHADERS)` exports a hooked canvas;
add `assets` for engine-drawn custom model materials, and `audio` for audio support.
A game names its hooks and shader pack in `app.json`'s `game.render` and the bake
writes that line ([game README](../README.md#exact2-integration)).
See LLP 1046.006.000 and `tests/hooks.rs` for the independent public fixture.

Hooks borrow `RenderWorld` for immutable extraction and `FrameView` for the exact
engine camera, interpolation, viewport/capacity and clock-reset facts. Register
entities through `entities()` to get tick-interpolated `FrameView::pose`. Use
`Pipelines<T>` for asynchronous candidate validation, `Needs::PENDING` until ready,
and `error()` for a named rejected replacement. Never step or mutate the simulation.

Stages are compute before shadows, opaque, background after sky, an optional
surface continuation, and optional HDR post before bloom/tone. Geometry uses 4×
MSAA. `SCENE_COPY` supplies resolved HDR and positive view-space depth (zero for
sky); `FINAL_DEPTH` requests final sampled depth independently; `HDR_POST` supplies
a distinct HDR output. Respect the active rectangle inside bucketed textures.
The refractor writes depth. Transparent objects behind it and intersecting
refractors are outside this first composition contract.

`CustomMaterial` replaces a loaded opaque, unskinned model material. The engine
owns geometry/draws and a compact per-instance `data` word; the game supplies paired
forward/shadow pipelines and group 2 resources. `MATERIAL_WGSL` supplies the frame,
transforms and instance accessor. Group 1 holds the engine's shadow maps in forward
and the light camera in shadow; group 3 is engine instances. Two vertex storage
bindings remain under the default limit of eight. A forward module that appends
`MATERIAL_SHADOWS_WGSL` gets `sun_shadow(world, normal)` (1 without sun shadows),
`light_visibility`, `brdf` and `add_local_lights` over those maps and the scene's
light buffer; it binds nothing more. Use conservative `ModelBounds` for GPU
deformation. Batches with a custom material are never culled, because the game's
vertex shader may move them.

`app.json` declares `gpu.shaderRoots` and optional `gpu.shaderPreludes` (shader stem
→ ordered source paths), all relative to the manifest. The bake merges and reflects
the assembled sources, rejects duplicate stems/symlinks and ships ordinary shader
assets. Compose the corresponding reflected slices with `exact_gpu::shader_packs!`.
No manual copy or private game dependency is needed in public engine code.

Tree/state report stages. `world.renderHooks` contains CPU rings, attachment-byte
estimates and separate engine/game creation counts before/after readiness; absent
raw-device reporting is `null`. Raw hook draws/dispatches are neither culled nor
counted in the engine’s draw/triangle counters. `world.gpuMs` (below) names each hook
stage; unsupported finer intervals are `null`.

## Culling

One compute pass after skinning (`cull.wgsl`, three dispatches) tests every opaque
item against the camera frustum and each cascade's light box, then writes each view's
visible slots into its own region, **in retained order** — equal-depth ties resolve
exactly as unculled — and fills that view's `draw_indexed_indirect` arguments.
Draws read their region through a dynamic offset on the scene group's `slots`
binding with a zero first instance, so WebGPU's optional `indirect-first-instance`
is not required. The CPU keeps the draw groups (batch × winding, recomputed each
frame, uploaded only when they change) and issues the same number of draws as before.

Bounds are conservative, never tight: an item's sphere covers every pose between its
previous and current tick (the translation segment, the scaled centre's segment and
the rotation's arc, which nlerp keeps under π) plus its mesh at the larger scale.
Primitives take their dimensions from the material; model nodes their composed box;
a skinned node the union of its mesh sphere under every joint of its current palette
(skinned positions are convex combinations of joint-transformed bind positions);
socket attachments their displayed affine. Game custom materials keep everything.
A margin covers f32 rounding. The camera culls sprites, emitters (from a bound on
speed, gravity and the longest lifetime; skipped emitters still charge the world
particle budget) and unskinned blended models on the CPU before derivation/upload.
Shadow casters are tested against their own cascade only; particles and sprites
cast no shadows.

Culling never changes pixels: `cull_tests.rs` renders moving primitive fields
(perspective and orthographic), animated skinned Foxes, models, sockets, sprites,
blended models and emitters culled and again keeping every item, and requires
identical bytes. A device without indirect execution (the iOS simulator's Metal;
probed once at construction, while WebGPU always has it) or whose storage limits
cannot hold the per-view lists (only region padding can exceed them below
`max_slots()`) draws every group directly and unculled, with identical pixels.
Culling does not reduce CPU draw calls; measurements are in
[bench/README.md](../bench/README.md#culling-and-environment-lighting--2026-09-23).

## Effects

`FrameInput::default()` supplies a shadowed sun, gradient sky, [environment
light](#environment-lighting) and bloom. Supply matching view/projection/camera
position. All colours are linear. `Bloom` and `Fog` are re-exports of the engine's
saved types.

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
  Shadow-disabled lighting is unfaded. Ambient, emission and the fill light are unaffected.
- Local light shadows are opt-in: add the `LightShadows` marker to a `SpotLight` or
  `PointLight`. A spot takes one 1024² Depth32Float layer (a perspective view of
  its cone, up to 170°), a point light six (a cube's faces, picked per pixel by the major axis
  from the light). Lights are served nearest first while the frame has layers
  left, at most eight (`local_shadows::MAX_VIEWS`). One shadowed point light takes
  six of the eight, leaving two spots; eight spots fit otherwise; the rest light
  unshadowed. The layer array grows to the most layers a frame has used (4 MiB
  each, at most 32 MiB) and is kept, so toggling a light reallocates nothing. Each layer's casters are culled
  on the GPU (four views per cull pass) and drawn depth-only with the sun's caster
  pipelines. Receivers offset 1.5 texels along the normal (less towards the light)
  and take 3×3 PCF. Forward passes always bind group 1 (cascades, the comparison
  sampler, local maps), with a one-texel placeholder for an absent map.
- Screen-space ambient occlusion is off by default; inserting the engine's
  `AmbientOcclusion { radius, intensity }` resource (or setting
  `FrameInput::ambient_occlusion`) turns it on. It retains the forward depth and
  takes hemisphere samples within `radius` metres around a normal rebuilt from
  depth, at `quality`: `Low` half resolution and 8 samples, `Medium` (default) half
  resolution and 12, `High` full resolution and 16. A depth-aware 3×3 upsample
  (edges do not bleed) multiplies the resolved HDR colour before post and bloom. It darkens all light at a crease, not only ambient, and translucent
  surfaces over a crease take its darkening. Off, no texture or pass exists.
  The `Medium` default (half resolution, 12 samples) replaced full-resolution
  occlusion: scenes with SSAO on change pixels slightly (intended; `High` is the
  former full-resolution cost and look).
- Bloom defaults to threshold 1, intensity 0.16, radius 1.5: one-sided knee, 13-tap
  downsampling and additive tent upsampling. Up to six RGBA16F levels, stopping
  before either dimension falls below 8; tiny outputs retain one level.
- Sky and environment lighting share zenith/horizon/ground colours. A constant
  sky without disc or differing fog colour uses the clear directly. `sun_disc` is
  angular radius in radians. Fog integrates exponential distance and Y-height
  density analytically, including a stable near-horizontal limit; sky uses 10 km.
  Fog is enabled by default. Default density is 0.012/m and height falloff 0.1/m; absent fog colour uses horizon.

### Environment lighting

Ambient light is image-based, from the procedural sky (`Environment.zenith`,
`horizon`, `ground`) — the flat `background` stays independent of lighting, and the
sun disc is left out because the sun is a direct light. When those colours change
(and only then) the renderer projects the sky onto nine SH coefficients on the CPU
(irradiance / π, in the frame uniform's `irradiance`, part of `FRAME_WGSL`) and
renders a 32² RGBA16F cube whose six mips hold GGX-prefiltered radiance, roughness
`mip / 5`, 256 samples per texel: 36 small passes before the frame's geometry.
Diffuse SH follows every change; the cube is prefiltered again only when a colour
moves more than 2% from the colours it was filtered from, or after the sky holds
still for 30 frames, when it is made exact. A dusk dimming 0.1% a tick prefilters
about once per 20 ticks instead of every tick.
Primitive and model shaders share `ibl.wgsl`: split-sum specular samples the cube at
the reflected direction and `roughness × 5` and scales it by Karis's analytic
environment BRDF; diffuse is SH irradiance × base × (1 − metallic) × (1 − specular).
`Environment.ambient` scales both, model occlusion multiplies both, and exposure and
ACES apply once as before. Metals reflect the sky out of direct light; dielectrics
reflect about 4% of it at normal incidence.

An authored map replaces the sky as the source: the engine's `EnvironmentMap`
resource (or `FrameInput::environment_map` for a direct renderer) names an
equirectangular texture (+Y the top row, −Z the centre column) delivered as a `.tex`
asset or added with `add_texture`, with a linear `intensity` and an optional RGBM
`rgbm` range (radiance = rgb × alpha × range) that carries HDR through 8-bit, BC7 or
ASTC payloads. The prefilter samples it at the level of detail matching each cube
mip, and a compute pass projects its SH9 on the GPU into a buffer copied over the
frame uniform's `irradiance` each frame. Both run once per map content, intensity or
range change. With `visible`, the map is also the visible sky: the sky pass samples
it (bilinear, mip 0) in place of the gradient and `background`, at its intensity and
RGBM range; the sun disc and fog still apply. `rotation` turns the map about +Y for
the light and the sky alike (a change filters again). Cube-map sources are not
supported; the source is equirectangular.

`Material::grid(color, spacing)` uses a derivative-antialiased world-space grid,
projected onto any face in the existing forward shader. Positive saved spacing
reuses the material's former padding; its GPU flag/spacing uses the opaque alpha
slot (negative spacing). Uploads stay twelve floats per instance and there is no
extra texture or pipeline. Non-grid materials skip the grid branch. The cubes
bench explicitly disables fog/bloom to retain its effects-off fast path.

Twenty primitive/effect pipelines compile at renderer startup: eight forward, two
shadow, one sky, two tone, three bloom, three culling compute and the environment
prefilter. This is startup work, not per-frame work. The
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
Model instances keep a CPU pose history only for translucent ordering and winding;
a completed tick steps the instances in blocks whose local or propagated poses
changed since the last step (`World::poses_changed_since`) and those still
interpolating, so static instances, parented or not, cost nothing per tick or
frame. Parented global poses are retained and recomputed only in changed blocks. A model batch stays one draw group per view unless an
instance has a negative scale axis or an attachment.

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
derivatives; baked models contain no tangent arrays. The shader reads a normal
map's XY and rebuilds Z (`sqrt(1 − x² − y²)`), so RGBA8, BC5 and two-channel ASTC
draw alike. Material UV transforms apply separately to every texture.

### Texture families

A `.tex` record is RGBA8, BC4, BC5, BC7 or ASTC 4×4 (`TextureData.format`,
[the bake](../bake/README.md#texture-payloads)); the renderer uploads its levels as
delivered, whole blocks per level, and never transcodes. The device requests every
available block family (`exact_gpu::requested_features`); `Surface::device_ready`
passes the granted features, and the surface chooses **BC** when granted, else
**ASTC**, else **RGBA8**. Worlds and models name the authored `x.tex`; the surface asks
the host for `x.bc.tex`, `x.astc.tex` or `x.tex` and maps the delivered file back,
so hosts fetch by name and the simulation never sees a family. A headless surface
has no device and fetches RGBA8, which validates and hashes like any delivery
(texture bytes are outside world hashes). A missing family file (an authored `.tex`
without a bake) falls back to the authored file once. A replacement device with a
different family retires the old files and re-requests every texture. A record
whose format the device did not enable, or whose edge exceeds its limit, is a named
delivery failure, not a wgpu error. `state.world.gpu` reports `textureFamily` and
`textures: {bytes, <format>: count}` for the active delivered textures (device bytes
are the delivered level bytes).

`ViewModel` marks an entity for the camera's viewmodel layer (a first-person weapon,
hands). Its opaque batches draw in the nearest `VIEWMODEL_DEPTH` (5%) of the depth
range and the world in the rest, so the layer is in front of every wall it reaches
into; it casts no shadows and still receives them. Mark each part: the marker is
per entity, not inherited. Without a drawn viewmodel the world keeps the whole 0–1
range. Blended viewmodel parts keep the layer's range in the translucent pass.
While a viewmodel draws, raw depth is split: the layer fills [0, 0.05) and the
world [0.05, 1]. The depths hooks receive (`SceneCopy.depth`, `PostInputs.depth`)
are resolved to linear view depth with the split undone, so they are correct for
both layers; SSAO skips viewmodel pixels and never samples them as occluders.

Opaque batches stay retained. Only transparent draws are sorted each displayed
frame, back-to-front in camera depth, using retained tick poses and local centers.
They keep depth testing, disable depth writes, and do not cast shadows. A model's
own materials are multiplied by entity base colour and have entity emission added:
one model serves every team colour or mutation look. `NodeMaterials` adds a tint and
emission per named node of one instance (stored in that node's instance record).
`Opacity(o)` (presentation state, like `NodeMaterials`: written by `Game::present`,
never saved or hashed) fades any instance, primitive or model: opaque surfaces drop pixels in
an ordered 4×4 screen-door dither (depth stays exact, nothing is sorted; model
shadows fade with it, primitive shadows too: their depth pass drops the same
dithered texels, so at `Opacity(0)` an entity casts nothing), blended model
materials multiply their alpha. This is coverage fading; a custom vertex or
fragment shader that wants it calls `faded(slot, pixel)` (fade.wgsl, in
`MATERIAL_SHADOWS_WGSL`) itself. Custom-material hooks ignore both.
`MaterialOverrides` replaces a model material's base colour factor, zero channels
included (and adds emission), on one instance: it becomes that material's records'
tint with a flag that drops the authored factor, so instances in different colours
still share their batches.
Present rebuilds these every tick; the feed compares each entity's content, so
unchanged looks cost nothing and changed ones (a pulsing glow) patch that entity's
records and part looks in place in the instance buffer, without a rebatch. Only a
look appearing on a merged part that had none (the part-look table changes shape)
rebatches.
At load, a model's rigid, non-blended parts that share a material merge into one
draw: static parts into one mesh pre-transformed into model space (mirrored parts
rewound), parts a clip animates into one mesh skinned with weight one to each
part's node, through the palette rigid parts already use. A many-part prop or a
rigid-limbed character is one draw per material. `NodeMaterials` still colours
parts one by one: an instance's per-part looks follow its records in the instance
buffer, each with its part's first vertex, and the record's last word points at
them; the vertex shader binary-searches them for the last part starting at or
before its vertex (the run's first entry holds its part count).
Static merges add no per-vertex data, and a part drawn only merged is not uploaded
on its own. A model whose merged draw has a material a game's `CustomMaterial`
shades draws its parts unmerged, since a custom vertex shader (wind sway about a
node) sees node-local positions and the node's offset: the surface's next asset
pass uploads those parts (a direct `Renderer` caller prepares the model again),
and until then it draws merged, as on the frame the material first appears. Merging moves
static vertices into model space on the CPU, so their pixels can differ from the
unmerged draw by float rounding (an intended change, under 8 pixels in the tests).
`ModelLod { levels, hide }` swaps an instance's model by camera distance. Each
frame the renderer picks one level per entity from its displayed position, with a
5% hysteresis band (presentation only), falling back to the nearest resident level
while one streams in. Every other level's records carry a hidden word the GPU cull
reads, in every view and keep-all group (shadow cascades and spot shadows too);
the blended pass skips them, and their skinning jobs are not dispatched, so a far
crowd skins nothing. A level change uploads only what it changed: the hidden words
of the records that changed and the places in the dispatched job list it filled. Direct drawing (no indirect execution, or lists past the
device's storage limits) has no per-instance cull and selects level 0 for every
entity, so the level it draws is the one skinned and blended. Levels
share the pose, looks and opacity. Distances must be finite, increasing and
positive, with `hide` beyond them; the feed refuses others by entity.
`world.perf.culled.cameraTriangles` counts what the cull kept for the camera
(`stats.triangles` counts every submitted level). On this Mac (`tests/lod_bench.rs`,
20,000 2,048-triangle trees, 1080p): 36.7 M camera triangles and a 15.8 ms frame
without `ModelLod`; 0.27 M and 4.5-7.9 ms with a 12-triangle level from 40 m and
`hide: Some(400.)`.
`Glow(Tween)` also multiplies the model's baked emissive factor and texture,
including on entities without a `Material` component. Removing `Glow` restores
authored emission. The model-only multiplier uses material slot 9 (primitive
dimension X for primitive draws); it does not change authored model assets.

Camera/sun/light rotations use normalized linear interpolation histories. The first
two posed `DirectionalLight`s in entity order light the scene: the first is the sun
(shadowed when its `shadows` is set), the second an unshadowed fill such as the
moon. Local lights are `PointLight` and `SpotLight` (a cone along the entity's −Z:
full intensity inside `inner`, smoothly zero at `outer`, both half-angles in
radians). Selection is feed-only: lights with positive tick-end intensity and
range, ordered by squared camera distance, then points before spots, then entity
index; the first `MAX_LIGHTS` (256) are drawn. `Lit` contributes its nonnegative
tick-end multiplier to eligibility. There is no incumbent advantage or saved
selection; continuous feeds, long seeks and restores select the same order. Lights
past the cap are counted in `FrameInput::lights_dropped` and
`state.world.perf.lights` (`{drawn, dropped}`), and the first frame that drops any
logs a warning.

Each frame the renderer bins the drawn lights on the CPU into a 16 × 9 × 24 grid
of screen tiles and exponential view-depth slices (`lights.rs`); the forward and
model shaders read the fragment's cluster from one storage buffer (`lights.wgsl`).
A light joins every cluster its range sphere may touch, so a pixel skips only
lights whose windowed contribution is exactly zero there, in frame order.

`Lit(Spring)` on a point or spot light samples the saved spring at presentation seconds and
multiplies `PointLight.intensity`; negative overshoot clamps to zero. Keep the authored
intensity constant and retarget once with `lit.to(now, 1.0)`. Sampling changes no
world bytes. `Glow(Tween)` independently controls material emission. The GPU regression
in `tests/world.rs` measures a non-emissive cube illuminated by a meshless spring light.
Engine lights are photometric and share one scale, `PHOTOMETRIC_SCALE` (0.0003):
`DirectionalLight.illuminance` is lux, and `PointLight.intensity` is candela, so a
light delivers `intensity / d²` lux at `d` metres. 10,000 lux of sun, or a 10,000 cd
lamp seen from 1 m, maps to renderer radiance 3; the 100 cd default lights a surface
1 m away at 1% of the default sun. `FrameInput` carries renderer radiance.
Missing materials/environment use defaults.

Performance samples appear only in `state.world.perf`: live frame stamps, tick,
feed, encode (frame input through submit) and ticks/frame distributions. CPU sample
rings retain 16,384 values. Seekable renders, agent advances and timed binds make
no perf clock calls; samples do not enter hashes. Armed perf also reads back, a few
frames late and asynchronously, `perf.culled` (instances each view drew: `camera`,
`shadows[0..3]`) and, where timestamps are granted, `world.gpuMs` rings for every
pass by `GPU_PASS_NAMES`: forward, the cascades, local shadows, `cull`, skin palettes,
the environment prefilter, SSAO, each bloom level (and `bloom`, first start to last
end), tonemap, depth resolves and hook stages; the translucent run (particles,
blended models) and inside-pass hook stages where the device times inside passes.
A pass the frame did not run is not read. GPU timings also fill under the agent's
virtual clock (seekable frames), so `state world perf` (the agent's `perf: true`;
`state world perf_reset` clears) then `clock +ms` times
an offscreen frame there; CPU frame rings stay live-only. Arming allocates the query set and
readback buffers; an unarmed canvas creates none. GPU intervals overlap; do not sum
them. The allocation-free claim covers
only the `steady_sim_feed_and_frame_inputs_allocate_nothing` moving-cube/camera/light
fixture (including trace recording) and the warm culling preparation
(`steady_culling_preparation_allocates_nothing`), after warmup,
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
Textures are resident under their authored name, whatever family file delivered
them; the digest covers that delivered record.

`presentation_generation` invalidates world-derived transform histories, material
pages, draw records, instance lists and skin pose histories. It does not invalidate
asset residency or palette capacity. `Feed::reset` clears old entity identities
before inspecting the replacement world. Retiring host requests deactivates their
content without resetting unrelated entities' histories. Re-requested Pending names
remain retired until their bytes are digest-accepted. Device loss, format change
or full module replacement still requires a new renderer.

The 64 MiB retired budget charges delivered texture level bytes and allocated buffer sizes,
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
cargo run -p exact-game-render --release --example cubes -- 100000 240 field
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
Animated unskinned mesh nodes use one hierarchy matrix per draw, with no vertex
weights or authored skin. They share interpolation, culling and shadow transforms
with skinned nodes.
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

Particles and sprites use retained CPU derivation (after [camera culling](#culling))
and separate 80-byte quad instance vertex arenas beside the entity/model draw-instance records. A thousand
sparks add no entities and no DrawInstance records. Feed retains emitter state
and previous/current transforms at ticks; a frame derives local particle motion,
transforms it, sorts the translucent entries and uploads contiguous instances.
Compatible neighboring particle entries coalesce across emitters into one draw;
a sprite/model interleaved by depth splits that batch. The particle fixture's
20,000 instances use one particle draw plus tonemapping. No frame scans the world.
Emitter state includes compact admitted-birth batches, never particle positions.
The renderer never changes that saved state.

Soft particles (`ParticleLook.soft`) fade alpha by the gap along the view ray to
the opaque scene behind them. On a frame with any, the forward pass stores its
multisampled colour and depth instead of continuing into translucency; a second
pass loads them, keeps the depth read-only and binds it to the soft particle
pipelines (sample 0). That store and reload is the cost, so it is opt-in per look.
Under a hook's scene copy, whose continuation already owns translucency, soft
particles draw hard.

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
