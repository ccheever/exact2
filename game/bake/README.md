# Game bake

`exact-game-bake` imports glTF models and their textures into the engine's
validated asset records at build time. Runtime surfaces consume those records;
they do not import glTF or compile authoring formats. The app bake uses generated
host adapters from `game/app`, with `app.json` selecting assets and audio support.

Run the crate tests from `game/` with `cargo test -p exact-game-bake` using the
repository's documented Cargo environment.
