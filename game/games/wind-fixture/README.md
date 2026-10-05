# wind-fixture

Render hooks in a game, end to end: `app.json`'s `game.render` names the
`render/` crate's `Wind` hooks and its `shaders::SHADERS` registry, and the
generated GPU shell exports `module!(WindGame, assets, hooks =
game_render::Wind, shaders = game_render::shaders::SHADERS)`.

- `render/shaders/wind_forward.wgsl`, `wind_shadow.wgsl`: a custom vertex material
  (`CustomMaterial`) over the generated `reed.model`; its paired shadow pass bends
  the same way. `gpu.shaderPreludes` puts the engine's material WGSL and
  `render/wgsl/wind.wgsl` in front of each, so they are ordinary shader assets.
- `render/shaders/sky.wgsl`: a dusk sky in the background stage, after the engine's
  frame-uniform prelude.
- `render/build.rs` reflects the inventory exactly as the bake ships it (each shader
  after its preludes), so `shaders::SHADERS` and `<name>::module()` match what hosts
  register; nothing is assembled at run time, and every shader reloads live.

The wind is presentation: it reads simulation time and never moves an entity. Its
strength is a `#[derive(Presentation)] Gust` that `Game::present` rebuilds from the
tick (with `World::presentation_rng`) and the hooks read; it is never saved or hashed.
Simulation tick and save hashes live in [pins.json](pins.json). Run
`bun game/games/wind-fixture/proof.mjs linux --paranoid` and
`bun game/games/wind-fixture/proof.mjs web` from the repository root, and
`bun game/app/shells.mjs game/games/wind-fixture --test` for the GPU test.
