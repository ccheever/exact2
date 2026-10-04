# Game bake

`exact-game-bake` imports glTF models and their textures into the engine's
validated asset records at build time. Tangents are omitted: the model shader derives its normal-map frame from world/UV derivatives. Runtime surfaces consume those records;
they do not import glTF or compile authoring formats. The app bake uses generated
host adapters from `game/app`, with `app.json` selecting assets and audio support.

Run the crate tests from `game/` with `cargo test -p exact-game-bake` using the
repository's documented Cargo environment.

## Texture payloads

Every texture bakes to three files, and a device fetches exactly one:

| File | Contents | Fetched by |
|---|---|---|
| `x.tex` (the authored name) | RGBA8, at most 2048², every level | devices with neither family, and the GPU-less host |
| `x.bc.tex` | BC7 colour/data, BC5 normal XY, BC4 occlusion | devices granting `texture-compression-bc` |
| `x.astc.tex` | ASTC 4×4 | devices granting only `texture-compression-astc` |

Each is the same `.tex` `Data` record (`TextureData`), digest-addressed like any
asset; its `format` says how the levels are encoded. Models and `Sprite`s name only
`x.tex`; the renderer maps that name to its device's file (see
[the renderer](../render/README.md#texture-families)). The module carries no
transcoder. A block record's base dimensions are whole 4×4 blocks (WebGPU's rule);
any other texture ships RGBA8 in all three files. Block textures reach 4096²; their
RGBA8 fallback drops levels above 2048², which costs the same device memory.

Formats follow what the material samples: base colour and emission are BC7 sRGB
(alpha-sampled only for MASK/BLEND), metallic-roughness BC7 linear, a texture
sampled only as a normal map BC5 (the shader rebuilds Z from XY), one sampled only
as occlusion BC4. ASTC is 4×4 for all of them, with the unsampled channels constant.
Mips are filtered once in RGBA8 (linear-light colour, alpha coverage preserved for
MASK) and each level is then encoded. Encoders: Intel's ISPC kernels (prebuilt, via
`ctt-intel-texture-compressor`) for BC and ARM's `astcenc` (via `ctt-astcenc`) for
ASTC, both only in this crate. Their bytes can differ bit-wise between SIMD
variants of one machine family and another; tests pin quality (top level above
40 dB PSNR against RGBA8 on the sampled channels), not encoded bytes.

A model's textures are named after it (`crate/0-srgb-straight.tex`), but the app
bake ships each distinct texture once: when models carry the same texels, sampler and
channel use, the first model in path order owns the files and the others' `.model`
records name them. Nothing is authored for this; a shared palette or material image
costs one download and one GPU texture however many models use it.

## Sprites

Put a sprite strip at `art/strip.png`; the ordinary app bake produces
`assets/strip.tex` and its two family files. Standalone PNG defaults are explicit:
sRGB colour, straight alpha, clamp-to-edge on both axes, nearest
minification/magnification/mip filtering, and a nearest-sampled mip chain that
retains the pixel palette: a family file holds BC7 or ASTC only where every level
decodes to exactly the authored texels, and RGBA8 otherwise. Use
`Sprite::new("strip.tex", Vec2::new(16., 16.))` with frames in source pixels. The
CLI also accepts `cargo run -p exact-game-bake -- art/strip.png assets/strip.tex`,
which writes the family files beside it. Dimensions must be 1..=2048. The sprite
fixture keeps its original `.tex` only as a test golden; production bytes come from
its PNG. Generated-output ownership, collision refusal and pruning are the same as
for models.

A PNG under `art/textures/` is a material texture instead, shared by name with any
model or generated mesh (`textures/soil.png` bakes as `soil.tex`): sRGB colour,
repeating on both axes, linear min/mag/mip filtering, a box-filtered mip chain in
linear light, block-compressed within the usual quality bound. Under `art/data/` the
same, but linear values: normal maps, masks, an RGBM environment map.

Put sounds at `art/<name>.wav` or `art/<name>.ogg`; the bake writes `assets/<name>.sound`
(a stem may have one source). WAV may be 8/16/24/32-bit integer PCM or 32/64-bit float,
plain or `WAVE_FORMAT_EXTENSIBLE`; Ogg is Vorbis. Mono or stereo at 8–192 kHz. Samples
become 16-bit, rounded to nearest (float scaled by 32768 and clamped); Vorbis is decoded
here and trimmed to its last page's granule position, so no game module links a
decoder. A `.sound` stores each channel's wrapping first differences, channels one after
another, which gzip compresses 1.3–2.2× smaller than interleaved PCM on recordings. More than
32 MiB of 16-bit PCM, compressed WAV, more than two channels and other rates are
refused by name. The CLI accepts `INPUT.wav|.ogg OUTPUT.sound`.

The digest manifest protects authored files from replacement or pruning. Obsolete
list-form manifests refuse with instructions to remove the manifest and its generated
outputs, then rebake; there is no ownership guess based on a filename.

`cargo run -p exact-game-bake --locked --offline -- --art /path/to/game` runs that
same owned-output bake without compiling a generated GPU module. The scoped game test
command runs it before logic tests whenever `art/` or `.baked-assets.json` exists.
The app-directory mode bakes art only; typed levels still require the generated build
script's concrete `Game` type.

With `game.assets: true`, generated GPU build scripts also call
`bake_game_level::<MyGame>`. `Game::LEVEL` names one `.level.json` file beside
`app.json`, typed by the author's existing `Data` derive and listed in `ASSETS`.
The bake refuses malformed fields by path and writes validated bytes to `assets/`;
the executable contains the decoder, not the level value. No level schema is duplicated
in the manifest. JSON remains readable through agent asset state after delivery.

The geometry allowlist accepts glTF `COLOR_0` (RGB becomes RGBA with alpha one).
Colours remain linear multipliers through `MeshData.colors`; an empty array means
white. Tangents and additional UV/colour sets retain their existing refusal rules.
