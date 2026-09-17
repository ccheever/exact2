# game/ — the game engine add-on

An agent-native game engine on exact2, as an **add-on**: its own Cargo workspace,
absent from the root `members`, so the five checks never compile it and an app
without a world carries none of it (LLP 1041 §5). The record of why is LLP 1041 and
its sub-documents; this file is the map and the rules. The code is the authority.

## The one idea

**A world is a guest in a `canvas`**, as a web page is a guest in an `iframe`
(LLP 1020). A game is an ordinary exact2 app — Contract for every screen, a manifest,
a bake, delivery — plus one module holding the simulation *and* its renderer in one
memory, loaded after first pixel like any GPU module. Game UI is the app engine:
canvas children are the HUD, a placement is a sign in the world.

## Layout

| | |
|---|---|
| `engine/` | `exact-game` — the simulation: world, data, ticks, input, scene, the agent's reads. **No GPU, no host.** |
| `derive/` | `exact-game-derive` — `#[derive(Data)]`, `#[derive(Component)]`. No `syn`. |
| `render/` | `exact-game-render` — the wgpu renderer and `WorldSurface`, the `exact_gpu::Surface` a canvas binds. |
| `physics/`, `audio/`, `bake/` | as they land |
| `games/` | consumers: `greybox` (LLP 1041.000 S0), `lanterns` |
| `bench/`, `twins/` | the same scenes here, in Godot 4 and in three.js; numbers, never checks |
| `diaries/` | what building with it was like, scored against the twins |

## The programming model

```rust
#[derive(Component, Default)]
struct Lantern { lit: bool }

impl Game for Lanterns {
    fn setup(world: &mut World, args: &Args) { /* spawn the level from the canvas's arguments */ }
    fn tick(world: &mut World, input: &Input) { /* one fixed step: the only place state changes */ }
}
```

- **A tick is a function that calls functions.** No scheduler, no plugins, no
  system parameters. Physics is `physics::step(world)`, written where it runs.
- **All state is in the `World`, and all of it is `Data`**: one derive gives the
  save game, the hash, the agent's JSON, the level file, and what a dev reload
  carries. A `Game` has no fields.
- **Time is an input.** `tick = floor(clock_ms × hz / 1000)`; a step is `1/hz`
  exactly; there is no `delta`. Rendering interpolates between the last two ticks,
  so motion is smooth at any refresh rate and the simulation never knows.
- **Names are first class.** `world.named("fox")` in code is `world:fox` to an agent.
- **Iteration is in entity order, always** — storage scans presence bitmasks in
  ascending index order, so a world loaded from a save replays exactly as the one
  that wrote it.

## Determinism — the contract (LLP 1041.001 D5)

Same seed, same inputs, same clock ⇒ the same `world.hash()`, on every host, bit for
bit. What that costs, and the only rules a game author must remember:

1. No clock but the world's. No randomness but `world.rng()`.
2. Transcendentals come from `exact_game::math` (libm), never `f32::sin`.
3. No `HashMap` iteration in a tick, no threads in a tick.
4. State lives in components and resources, nowhere else.

Pixels are held to a band; simulation state is held exactly.

## The agent's interface

No ninth operation (LLP 1041.001). `tree`, `state`, `layout`, `logs` and `clock`
reach the world through one export on the module; an entity is a target
(`world:fox`); `clock` is the only thing that moves the world; the journal is how an
agent hears. The inner loop needs no host at all:

```rust
let mut sim = Sim::<Lanterns>::new(&args);
sim.key("KeyW", true);
sim.advance_to(1500.0);
assert_eq!(sim.world().hash(), 0x…);
```

## Working here

`cargo test` in this directory is the engine's loop; `bun ../scripts/caps.mjs` still
holds every file to 1,500 lines. A game is driven like any app, with
`EXACT_APP_DIR=game/games/<name>`.

Small is a feature. When something here feels clunky, slow or bloated, the move is
to delete it and try again, not to configure it.
