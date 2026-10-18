# exact-game-render

PBR over slot-indexed floats and separate model draw instances. `WorldSurface<G>` connects a simulation to an
Exact canvas; the simulation crate owns no GPU or host. Each frame a GPU pass culls
every opaque item against the camera and each shadow cascade; each geometry pass
then issues one indirect draw per mesh batch (and winding) over its view's survivors.
See [Culling](#culling).

Model forward programs specialize out fill and clustered-light code when the
frame has neither. The shared fallback still evaluates both while a new program
compiles, so adding a light takes effect on its first frame. Sun, ambient,
materials, shadows, fog and outlines retain their usual shading.

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
surface continuation, and optional HDR post before bloom/tone. Geometry uses
`Game::SAMPLES` (4× MSAA by default); hooks use `FrameView.sample_count`. `SCENE_COPY` supplies resolved HDR and positive view-space depth (zero for
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

Three compute passes after skinning (`cull.wgsl`, one dispatch each) test every opaque
item against the camera frustum and each cascade's light box, then write each view's
visible slots into its own region and fill that view's `draw_indexed_indirect`
arguments. The camera's region takes a group's 64-item chunks nearest first (a
counting sort over 256 depth steps, for groups of 16 chunks or more), so depth
tested before shading rejects what they hide; within a chunk, and in the cascades,
order is the retained order, so equal-depth ties resolve as unculled.
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

Pass boundaries separate the dependent stages on every backend. An Adreno 620
with its 2020-11-30 Vulkan driver corrupted compacted model slots when the stages
shared a pass, even while the indirect draw counts were correct. The readback
regression compares every output slot and indirect argument with a CPU reference
across 72 groups and four views. The boundaries add no dispatch, upload or readback
to normal rendering.

Culling never changes pixels: `cull_tests.rs` renders moving primitive fields
(perspective and orthographic), animated skinned Foxes, models, sockets, sprites,
blended models and emitters culled and again keeping every item, and requires
identical bytes. A device without indirect execution (the iOS simulator's Metal;
probed once at construction, while WebGPU always has it) or whose storage limits
cannot hold the per-view lists (only region padding can exceed them below
`max_slots()`) draws every group directly and unculled, with identical pixels.
Culling does not reduce CPU draw calls; measurements are in
[bench/README.md](../bench/README.md#culling-and-environment-lighting--2026-09-23).

## Forward variants for tiling GPUs

The forward pipelines beyond the eight made at construction are made on first use
(`Pipelines::forward_variant`), keyed by what the frame needs. Native renderers
keep a uniform-driven fallback while a worker compiles a new live-settings
variant. *Lean* compiles without the fill and local lights and the grid when no light or material uses
them: their registers, unused, cost a mobile GPU most of its occupancy. *Early*
forces depth tests before shading (`SHADER_EARLY_DEPTH_TEST`) when no slot in the batch is
fading, whose dither discards. *Uniform* reads a one-colour sky's irradiance and
radiance without evaluating either. *Direct* ([below](#direct-frames)) draws a
frame that needs no HDR image straight into the target, the shader tonemapping:
at 4×, MSAA in the target's format resolves into it — half the bytes a sample,
no tone pass. Each leaves the image as it was; direct at 4× moves MSAA edges
only, each sample tonemapped before the resolve
(`effects::a_direct_frame_draws_what_the_tone_pass_draws`).
On a Pixel 5 (Adreno 620) these took 64,000 cubes from 4 to 60 frames a second
([bench/README.md](../bench/README.md#phone--cubes-on-a-pixel-5-2026-10-16)).

*Half precision*: the device is asked for `SHADER_F16` where the adapter has it,
and with it the primitive and model forward modules shade in f16 (`enable f16`,
frame.wgsl's `shading` alias): the ambient light, the BRDF's colour terms, rim,
emission and the fog's blend. Positions, depth, directions, texture coordinates,
the BRDF's scalar lobes and shadow sampling stay f32, and a specular peak is capped
at 2048 times its light so that it stays finite in half. A mobile GPU runs f16 at
twice the rate in half the registers. Without the feature (WebGPU without
`shader-f16`) the modules are the f32 ones, unchanged. On this Mac (Metal grants
it) the half-precision town is within two levels of the f32 town
(`effects::half::*`). `state.world.gpu.shaderF16` and the log's
`game renderer first frame encoded` line (`f16=`) say which a device draws.
On native Vulkan, Exact also grants it on adapters with f16 arithmetic/storage
but no 16-bit uniform access (Adreno 620). The patched HAL rejects shaders that
actually use 16-bit uniforms there and removes Naga's unused uniform capability
from arithmetic-only modules; see `vendor/wgpu-hal/EXACT-PATCHES.md`, patch 11.
Forward buffers and stage interfaces remain f32/u32, checked by the translation test.

Opaque models select a variant without fragment discard when their own batch has
no coverage fading; devices with `SHADER_EARLY_DEPTH_TEST` force its depth test
before shading. Masked, blended and fading batches keep their coverage paths.
Model arrival compiles no pipeline. On first draw each material family builds
the current specialization and uniform-driven opaque/coverage fallbacks. Later
fog, cascade, filter and fade changes use that fallback while a worker builds the requested
specialization; unchanged frames reuse it. Web builds keep using the fallback
until the material family is recreated, without compiling a live setting change.
Shadow and camera-depth families prepare both coverage modes, with no lighting cross-product, so changing opacity does not compile depth pipelines.
Materials with no authored texture use a texture-free variant: no material
sampling, UV transforms or cotangent-frame derivatives. Both textured and
untextured materials use the geometric normal when no normal map is authored;
RGBA8's neutral-map quantization must not tilt an absent map. Both share the
same bindings and asset lifetime.
One-cascade frames specialize both model and primitive shadow receivers to
constant matrix/layer indices, retaining the same PCF and distance fade.
The receiver chooses manual-plane or comparison sampling before the tap loop.

### Direct frames

A frame draws straight into the target, with no HDR image and no tone pass,
when nothing after its forward pass reads that image (no hook, bloom or SSAO,
no soft particle drawn) and whatever blends would blend alike there: nothing
blends (then any tone curve, applied per pixel, is the tone pass's), or tone
mapping is `ToneMapping::None` and the target's format is sRGB. A blend is
linear in what it mixes, so it commutes with exposure, all that `None` applies,
and an sRGB target blends in linear light as the HDR image does; ACES, or a
target the shader encodes, would mix other values than the tone pass shows.
Each forward pipeline has a direct twin in the target's format, made on first
use, that tonemaps its output and passes alpha to the blend: primitives,
models (opaque, masked, blended), hulls, the sky, sprites, particles and placed
children. The opaque depth prepass writes the frame's own depth, at the
target's size. At 1× the forward pass draws into the target itself; at 4× into
transient MSAA colour in the target's format, resolved into it. A direct frame
makes and grows no HDR attachment, so a renderer that only draws direct frames
keeps them at their first 64-pixel bucket.

`Renderer::set_direct` allows it: by default at one scene sample everywhere,
and at four on Android and iOS, whose tiling GPUs gain most (elsewhere moving
MSAA edges, and pinned pixels with them, is not worth it). A one-sample game's
surface asks for an sRGB target (`Surface::srgb_target`), offscreen and in a
window alike: a window keeps its unorm format and renders through an sRGB view,
so the bytes shown are the same.

At 1× the frames differ in one way: the target clamps each fragment to [0, 1]
before it blends, where the HDR image is clamped after its tone pass. A
translucent fragment over light above 1, or a translucent value above 1, blends
with 1, as a renderer without an HDR image draws it
(`effects::direct::a_translucent_fragment_over_light_above_one_blends_with_one`).
Opaque values above 1, emission and rim included, clamp alike, and additive
blending, which only adds, clamps alike too. Otherwise the frames agree within
two levels (`effects::direct::a_direct_town_frame_draws_what_its_hdr_frame_draws`:
Castle Town's settings with toon models and hulls, a blended model, sprites,
straight-alpha and additive particles in linear fog, with and without the
prepass, at two exposures); at 4× the edges of values above 1 move as well.
`state.world.perf.directFrames` counts the frames drawn direct, and the log's
`game renderer first frame encoded` line ends `direct=true` when the first was.

## Occlusion

A frame whose lists hold 32,768 slots or more (`Renderer::set_occlusion_items`)
first draws what the camera drew last frame, depth only at this frame's poses,
into a texture a quarter of the frame's size, builds a pyramid of the farthest
depth under each texel, and the cull drops from the camera's lists what lies
behind it wherever it can cover (occlusion.rs, cull.wgsl `occluded`). Only
closed convex meshes occlude (`occlusion::inscribed`), each shrunk and pushed
back by a texel (occluder.wgsl), so no crack narrower than a texel between two
occluders hides what shows through it: the frame is the frame drawn without
occlusion, sample for sample (`effects::occlusion_*`). It is off for a frame
after a new setup, a level-of-detail change or with anything fading. At 256,000
cubes on a Pixel 5 the camera then draws 42,000 of the 140,000 the frustum
keeps; drawn unshrunk, 18,000, at the cost of the far cubes seen through a
crack narrower than a texel.

## Effects

`FrameInput::default()` supplies a shadowed sun, gradient sky, [environment
light](#environment-lighting) and bloom. Supply matching view/projection/camera
position. All colours are linear. `Bloom` and `Fog` are re-exports of the engine's
saved types.

- Sun shadows default to 60 m, three 2048² Depth32Float cascades, practical splits
  (lambda 0.7), rotation-invariant fitting spheres and texel snapping. Casters up
  to one shadow distance towards the sun are included. The last 10% of each slice
  cross-fades; the final slice fades to unshadowed. Games can insert the saved
  `Shadows` resource to choose 1–3 layers, reach, filter and strength; absence
  preserves the defaults. `ShadowFilter::ReceiverPlane` uses `softness` as its
  PCF radius. `ShadowFilter::PcfSoft { depth_bias, normal_bias }` uses the
  three-texel PCF-soft box, reconstructed with four hardware bilinear comparisons.
  `ShadowFilter::Bilinear { depth_bias, normal_bias }` uses one bilinear comparison
  over four depth texels. It has a narrower, crisper footprint and a separate
  model/primitive program; the fallback follows live filter changes.
  Its depth bias is subtracted in normalized depth; normal bias is in world
  metres, matching three.js `normalBias`, independent of cascade extent. `strength`
  blends shadow visibility with unshadowed light.
  `Shadows::fixed_box` uses one camera-independent sun projection around a world
  centre. `cache_static` reuses opaque, unskinned model groups whose poses and
  geometry are unchanged; primitives, moving/fading/attached models and custom
  materials draw normally. A cached depth layer is copied before dynamic casters
  draw. Sun/projection, geometry, visibility, LOD, winding and pose changes rebuild
  it. Direct transform writes without Feed's matching CPU histories disable it;
  only a submitted frame validates newly encoded cache contents. Both options are
  off by default. Fixed boxes must have positive finite extents and a finite centre.
  `Shadows::resolution` sets each layer's width and height (128–4096, default
  2048). Changing it reallocates the maps; camera snapping, world texels and
  all sampling filters follow the actual dimensions. A PCF-soft footprint
  remains three texels, so reducing resolution makes shadows softer.

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
  `Fog::linear(near, far)` selects three.js-style near/far fog instead: smoothstep
  of camera-space forward depth, not radial distance. Set `color: Some(clear)`
  to match a flat background. Linear fog leaves the sky unchanged. Both fog modes
  apply to primitive/model surfaces, outline hulls, sprites and particles.
- `Environment.tone_mapping = ToneMapping::None` skips ACES; exposure and bloom
  still precede the linear-to-sRGB transfer. Disable bloom and use exposure 1
  for unprocessed linear output. `ToneMapping::Aces` remains the default.
- Set `const SAMPLES: SceneSamples = SceneSamples::One` in the `Game` implementation
  for one scene sample per pixel. `SceneSamples::Four` remains the default.
  Direct renderer users call `Renderer::with_samples(device, queue, format, samples)`.
  This is a construction setting: attachments, primitive/model/hull/quad pipelines,
  sky, soft particles and depth readers all agree. At 1× the HDR colour is already
  resolved; no MSAA resolve or multisampled colour attachment exists. Shadows remain
  single-sampled at either setting. Scene-copy hooks copy the stored HDR colour
  before continuing into it; they must use `FrameView.sample_count` for geometry.
  Only a drawable hook requesting `Needs::SCENE_COPY` creates that snapshot.
  Soft particles split the forward pass to read depth; SSAO, bloom, linear fog,
  `HDR_POST` and `FINAL_DEPTH` do not request a colour snapshot. With linear fog
  but no hooks, post effects or soft particles, a 1× frame keeps depth transient
  and draws [direct](#direct-frames) where it may, otherwise one HDR forward pass
  followed by the output transfer.
- At 1×, `Game::DEPTH_PREPASS = true` draws opaque camera depth before colour.
  Direct users choose `Renderer::configured(device, queue, format, samples, true)`.
  Direct frames keep depth transient and draw it at the start of the forward
  pass, with colour writes disabled. HDR frames retain depth between a separate
  depth pass and forward. Both trade another draw per opaque batch for less
  hidden-fragment shading. It reuses camera culling, skinning, masked and fading
  coverage, mirrored winding and the viewmodel depth range. Blended models, quads
  and hulls stay in their normal passes. Custom materials disable the prepass for
  the whole frame, preserving their single-pass depth and tie behavior. Camera depth
  uses the colour vertex module. Batches without fading or masking draw position
  only: primitives through `vs_depth`, models through `model_depth_vs`, each
  computing the colour entry's world position through the same functions
  (`primitive_world`, `model_world`) and none of its normal, material or look work;
  the others use the colour entry for the varyings their discards read. The
  positions are invariant. Colour uses Equal with a clear-depth guard; built-in
  coplanar ties follow draw order. On the Pixel 5 at 1080×2340 and 90 Hz,
  combining the direct frame's passes raises Castle Town's first two views from
  65.2/63.4 to 67.9/65.4 fps. The default is off; measure on the target GPU before
  enabling. HDR's timestamp is `opaque depth prepass`; direct frames include
  depth in `forward + sky`.

### Toon models

`MeshData.outline` optionally holds `MeshOutline { width, min_pixels, color,
normals, reach }`. Width is in world metres, `min_pixels` in CSS pixels, colour
in linear RGB, normals three floats per vertex, and reach one local-space distance
per vertex along its outline normal. `MaterialData.rim` defaults to zero.
The binary codec carries both. The bake reads `_OUTLINE_NORMAL` (FLOAT VEC3),
`_OUTLINE_REACH` (FLOAT SCALAR), `primitive.extras.outline = {width, minPixels,
color}` and `material.extras.toon = {rim}`. It also accepts the exporter aliases
`castle_outline` and `castle`; other unknown semantics/extensions remain refused.
Both attributes and outline settings must occur together, and each accessor count must
equal POSITION before primitives merge. Outlines with alpha-tested (MASK) or blended
(BLEND) materials are refused at bake and model validation. Custom material hooks
on outlined meshes are refused by model/mesh name when validated; baked models
have no custom shader field, and shader extensions such as KHR_techniques_webgl
are refused at import. Units conversion scales
width and reach, and primitives only merge when their outline settings match.

An outlined model adds one instanced indexed hull draw per visible mesh/winding
batch, after opaque surfaces in the existing forward render pass. Front faces are
culled; the hull tests and writes depth, shades its unlit colour with fog, and never
enters shadow or occluder passes. A batch with a fading slot drops its hull's
dithered texels with its surface's; any other draws its hull with no discard, so
depth is tested before shading (`toon::fading_hulls_dither_and_opaque_hulls_draw_whole`). Its displacement is
`min(max(width, min_pixels * pixel_metres), transformed_reach)` along the transformed
reach vector. For perspective, `pixel_metres = 2 * view_depth / (projection_y * css_height)`;
orthographic projection omits view depth. The feed supplies CSS height; direct
callers can set `FrameInput.viewport_css_height` (otherwise the target pixel height).
The reach vector follows the linear part of every node, blended skin, attachment
and instance transform, then is normalized for the push. Position and reach share
one transform walk; the offset uses w=0
and never subtracts two world positions. Surface lighting still uses inverse-transpose
normals. Mirrored-instance winding follows the surface.
The camera cull grows the surface sphere by the requested world width or CSS pixel
floor, capped by transformed reach. The floor uses the farthest depth in the sphere:
metres per pixel increase with depth, so its nearest depth is not a conservative
bound. Sun and local-light caster bounds remain those of the surface.

Rigid parts merge when material, outline width, colour and min_pixels match.
Static merges transform normals and reach vectors separately, preserving both
under non-uniform scale. Animated merges retain each vertex's palette index.
Parts with different outline settings remain separate surface/hull/caster draws.

Rim adds `rim * (1 - saturate(dot(n, v)))^3 * mix(albedo, 1, 0.2)` before fog.
It occupies an unused material-uniform word and uses a draw-uniform zero guard:
no varying control flow, texture fetch or extra vertex data for rim, including on
tiling GPUs. Models without outlines allocate no hull data or pipelines and issue
no hull draw. Each outlined mesh adds a 24-byte vertex stream (normal and reach vector) and a
32-byte uniform;
the two winding pipelines are prepared once on first outlined-model arrival.
Only the hull pipeline reads this stream; it needs eight vertex storage bindings,
the same as surfaces. For indirect draws, two GPU copies per hull group retain the
camera's culled count and index range in a 20-byte draw record with zero base vertex.
`Stats` includes the extra hull draws, instances and triangles.
Hull vertices evaluate the shared model transform twice to obtain world-space
reach; fragments have no material texture reads or lighting. The pass adds no
attachment or pass boundary. Device timing is still required on Adreno 620 and iPad;
these are work counts, not a GPU-time claim.


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

`Game::MATTE_MODELS = true` selects diffuse-only model lighting for a renderer's
lifetime (`Renderer::with_matte_models` before preparing models for direct use).
Models use Lambert direct light and SH environment irradiance without specular
or radiance-cube sampling. Metallic and roughness values do not change this
response. Base/normal/occlusion/emission textures, light shadows, fog, rim and
alpha still apply; primitive and custom materials keep their existing shading.
The default is physical. Castle's matched mode opts in for its matte artwork;
its native-quality mode keeps physical lighting.

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
tonemapping (or `ToneMapping::None`) applies sRGB transfer once. HDR reads sanitize NaN and clamp to
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

Instance tint and emission are read once per vertex and passed flat, avoiding
per-fragment storage reads. Opaque alpha does not read the fade buffer.

Model materials use a separate forward pipeline. Opaque, unfaded shadow casters
use depth-only pipelines without fragment shaders; masked and fading casters
retain fragment coverage. Opaque/mask/blend, culled/double-sided and mirrored
variants are prepared when their material family first draws. Device loss preserves Loaded content and re-requests
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
frame: world transparency first, view-model transparency last, each back-to-front
in camera depth, using retained tick poses and local centers.
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
them; the vertex shader takes the last part starting at or before its vertex.
Static merges add no per-vertex data. The parts' own meshes stay resident: a model
whose merged draw has a material a game's `CustomMaterial` shades draws its parts
unmerged, since a custom vertex shader (wind sway about a node) sees node-local
positions and the node's offset. Merging moves
static vertices into model space on the CPU, so their pixels can differ from the
unmerged draw by float rounding (an intended change, under 8 pixels in the tests).
Parts of different materials merge too when the materials are opaque, untextured,
of one sidedness and with a base colour factor of one (the colour in the vertices,
as Castle Town's exporter writes it): their factors become the instance's part
entries, the look table's entries with each part's metallic, roughness, rim and
emission beside its tint and glow, and each vertex numbers its part in `uv.x`, which
no texture reads. The draw's own material has factors of one that the entries
scale, so each part shades as it did alone
(`instances::parts_of_several_materials_draw_once_and_look_the_same`). An instance
with `MaterialOverrides`, which recolour one material's records, draws such a
model's parts unmerged. In Castle Town's spawn view this takes a frame from 418
draws to 274; two pixels on the fountain's water edge move with the rounding.
A generated one-joint skin reads its matrix directly; weighted skins retain the
four-joint blend. Colour computes the skin once for position and normal, and
reuses that position for world space. Camera depth uses the same arithmetic.
`ModelLod { levels, hide }` swaps an instance's model by camera distance. Each
frame the renderer picks one level per entity from its displayed position, with a
5% hysteresis band (presentation only), falling back to the nearest resident level
while one streams in. Every other level's records carry a hidden word the GPU cull
reads, in every view and keep-all group (shadow cascades and spot shadows too);
the blended pass skips them, and their skinning jobs are not dispatched, so a far
crowd skins nothing. Direct drawing (no indirect execution, or lists past the
device's storage limits) has no per-instance cull and draws level 0 only. Levels
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
virtual clock (seekable frames), so `state world perf:true` then `clock +ms` times
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

`Model::requires_palette()` selects skins, clips and marked posable nodes. One
mark promotes the whole model, even instances without offsets. Ordinary static
models keep pretransformed merges and no weight-buffer upload. Posable models
retain material merging; rest-mirrored rigid parts reverse their indices in merged
and lone-part draws. Authored shear is refused for posable models.

Each (entity, model) instance packs one local-pose span shared by all its material
and skin jobs. `NodeOffsets` composes once per span at both endpoints, after the
saved clip/layer/IK pose, or over bind TRS without a `Pose`. Rows disappear through
one tick of interpolation to identity. Same-tick presents replace the current
endpoint; birth, first pose, teleport, model/LOD arrival, replacement and restore
prime current/current. Hidden instances retain their offset histories. Displayed
socket chains use the same composition; simulated sockets remain unchanged.
Diagnostics are bounded to 64 distinct name/reason pairs per prepared model.

Each LOD resolves names against its own model. Static levels ignore offsets and
run no skin jobs, though the near level's local span is still packed. Palette
culling uses conservative joint bounds; blended parts bypass CPU rejection and
sort by their undeformed centre.

`clear_present_feed_and_pack_allocate_nothing_clustered_or_sparse` counts zero
allocations over warm live clear/present/recording-feed/local-packing cycles for
30 and 1,000 twenty-node instances, with clustered and sparse entity indices.
This excludes seekable observation work, wgpu staging, command encoding and draws.
Payload pages are reclaimed after 60 consecutive empty presents; index metadata
retains its peak entity capacity. `World::presentation_bytes()` reports both,
excluding registry and lease bookkeeping. The sparse/churn engine proof prints residency
before and after reclamation.

For CPU timings with a supplied twenty-node avatar GLB, run from `game/`:
`cargo run -p exact-game-render --example poses -- /path/to/avatar.glb`.
It marks the five gait nodes in memory and measures tick/clear/present separately
from the real renderer feed at 30 and 1,000 instances. Queue submission, frame
encoding and GPU drawing are outside those CPU intervals.

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

Primitives with `Material.color[3]` in the half-open range `(0.0..1.0)` join this same sorted quad arena: each
triangle is one instance (the quad's second triangle is degenerate), compatible
with untextured straight-alpha particles. One halo and a hundred share one draw;
triangles still cost derivation, upload and sorting. Their look is flat
hemisphere/directional diffuse light plus emission and fog; PBR, local-light
shadows and procedural grids stay on opaque primitives. They depth-test without
depth writes or refraction and cast no shadows. `Material::alpha(a)` takes [0, 1].
Ring, torus and cone geometry also works on the opaque path. See the
[halo recipe](../recipes/halo.md).

Opaque and masked sprites write
depth; the ordered translucent pass follows the sky, depth-tests and does not
write depth. World transparency draws first and view-model transparency last.
Within each group, ordering is descending view depth, ascending layer and entity
slot, then kind rank and stable per-owner ordinal. Models use layer zero. Sprites and particles cast no
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
including masked shadows. Primitive materials outside that alpha range remain opaque, including the negative-alpha grid sentinel. The packed public
`Vertex` is 48 bytes including colour, with white in built-in shapes; skin weights
retain their separate 32-byte stride. The asset fixture constructs a seeded coloured
heightfield and three shared rocks in setup, with no runtime texture generator.
