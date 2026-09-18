# Game bake

`exact-game-bake` imports glTF models and their textures into the engine's
validated asset records at build time. Runtime surfaces consume those records;
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
