# Game bake

`exact-game-bake` imports glTF models and their textures into the engine's
validated asset records at build time. Tangents are omitted: the model shader derives its normal-map frame from world/UV derivatives. Runtime surfaces consume those records;
they do not import glTF or compile authoring formats. The app bake uses generated
host adapters from `game/app`, with `app.json` selecting assets and audio support.

Run the crate tests from `game/` with `cargo test -p exact-game-bake` using the
repository's documented Cargo environment.


Put a sprite strip at `art/strip.png`; the ordinary app bake produces
`assets/strip.tex`. Standalone PNG defaults are explicit: sRGB colour, straight alpha,
clamp-to-edge on both axes, nearest minification/magnification/mip filtering, and a
nearest-sampled mip chain that retains the pixel palette. Use `Sprite::new("strip.tex", Vec2::new(16., 16.))`
with frames in source pixels. The CLI also accepts `cargo run -p exact-game-bake --
art/strip.png assets/strip.tex`. Dimensions must be 1..=2048. The sprite fixture keeps
its original `.tex` only as a test golden; production bytes come from its PNG.
Generated-output ownership, collision refusal and pruning are the same as for models.

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
