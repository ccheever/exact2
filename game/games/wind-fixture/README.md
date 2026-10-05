# wind-fixture

Render hooks in a game, end to end: `app.json`'s `game.render` names the
`render/` crate's `Wind` hooks and its `shaders::SHADERS` registry, and the
generated GPU shell exports `module!(WindGame, assets, hooks =
game_render::Wind, shaders = game_render::shaders::SHADERS)`.

- `render/src/wind*.wgsl`: a custom vertex material (`CustomMaterial`) over the
  generated `reed.model`, composed after the engine's `MATERIAL_WGSL`; its paired
  shadow pass bends the same way. Compiled in, because it needs the engine prelude.
- `render/shaders/sky.wgsl`: a dusk sky in the background stage, shipped as a
  shader asset (`gpu.shaderRoots` declares `render/shaders`), so an edit
  reloads live.

The wind is presentation: it reads simulation time and never moves an entity. Its
strength is a `#[derive(Presentation)] Gust` that `Game::present` rebuilds from the
tick (with `World::presentation_rng`) and the hooks read; it is never saved or hashed.
Simulation tick and save hashes live in [pins.json](pins.json). Run
`bun game/games/wind-fixture/proof.mjs linux --paranoid` and
`bun game/games/wind-fixture/proof.mjs web` from the repository root, and
`bun game/app/shells.mjs game/games/wind-fixture --test` for the GPU test.
