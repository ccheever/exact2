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
| `app/` | `exact-game-app` — the shared Rust-only bake for game UIs without data sources. |
| `derive/` | `exact-game-derive` — `#[derive(Data)]`, `#[derive(Component)]`, `#[derive(Args)]`. No `syn`. |
| `render/` | `exact-game-render` — the wgpu renderer and `WorldSurface`, the `exact_gpu::Surface` a canvas binds. |
| `physics/`, `audio/`, `bake/` | as they land |
| `games/` | consumers: `greybox` (LLP 1041.000 S0), `beacons` |
| `bench/`, `twins/` | the same scenes here, in Godot 4 and in three.js; numbers, never checks |
| `diaries/` | what building with it was like, scored against the twins |

## The programming model

The complete example below is compiled and run by `cargo test --doc -p exact-game`.

```rust
use exact_game::*;

#[derive(Default, Args)]
struct Options {
    seed: u64,
    #[live]
    paused: bool,
}
#[derive(Default, Component)]
struct Beacon { glow: Spring }
struct SmallGame;
impl Game for SmallGame {
    const ID: &'static str = "small-game";
    type Args = Options;
    fn actions() -> Actions {
        Actions::new().stick("move", Stick::keys("KeyW", "KeyS", "KeyA", "KeyD"))
            .button("light", &["KeyE"])
    }
    fn setup(w: &mut World, args: &Options) {
        w.reseed(args.seed);
        w.spawn((Transform::default(), Mesh::plane(40.0, 40.0), Material::default()));
        w.spawn_named("player", (Transform::at(0.0, 0.9, 0.0), Mesh::capsule(0.4, 1.8), Material::rgb(0.8, 0.4, 0.1)));
        w.spawn_named("camera", (Transform::default(), Camera::default(),
            Follow::new("player").offset(0.0, 9.0, 13.0).lag(0.15)));
        w.spawn((Transform::at(2.0, 0.5, 0.0), Mesh::sphere(0.5), Material::default(), Beacon::default()));
        w.publish("lit", 0);
    }
    fn paused(args: &Options) -> bool { args.paused }
    fn tick(w: &mut World, input: &Input, _: &Options) {
        let movement = input.stick("move");
        w.get_mut::<Transform>(w.named("player").unwrap()).unwrap().position +=
            Vec3::new(movement.x, 0.0, -movement.y) * 4.0 * w.dt();
        let mut count = 0;
        for (_, (beacon, material)) in w.query::<(&mut Beacon, &mut Material)>().iter() {
            if input.pressed("light") { beacon.glow.set_target(w.now(), 1.0); }
            material.emissive = [beacon.glow.value(w.now()) * 3.0; 3];
            count += u32::from(beacon.glow.target == 1.0);
        }
        w.publish("lit", count);
        scene::follow(w);
    }
}
fn main() {
    let mut game = Sim::<SmallGame>::new(&[Value::Number(7.0), Value::Bool(false)]).unwrap();
    game.advance(0.0, Clock::Seekable);
    game.input(InputEvent::Key { code: "KeyE".into(), down: true, at_ms: 0.0 });
    game.advance(1000.0, Clock::Seekable);
    assert_eq!(game.world().published("lit"), Some(Value::Number(1.0)));
    assert!(game.world().get::<Transform>(game.world().named("camera").unwrap()).unwrap().position.y > 9.0);
}
```

- **A tick is a function that calls functions.** No scheduler, no plugins, no
  system parameters. Physics is `physics::step(world)`, written where it runs.
- **All state is in the `World`, and all of it is `Data`**: one derive gives the
  save game, the hash, the agent's JSON, the level file, and what a dev reload
  carries. A `Game` has no fields.
- **Arguments are a struct; field order is canvas order.** `#[derive(Args)]`
  supports bool, u32/u64, i32/i64, f32/f64, and String (`()` for none).
  Unmarked fields construct; `#[live]` fields are read each tick.
  Integer bounds are checked before casting; 64-bit fields accept safe f64 integers. A timed
  `bind(values, Some(at_ms))` validates first, seeks under the old arguments, then
  swaps. A refused bind changes nothing. Saves carry the game's `ID` and
  `SAVE_VERSION`, world time and dynamic input; the first restored host clock
  establishes a new epoch.
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

## Publications and events

`World::publish("beacons", count)` updates the current public record. Contract reads
it with `resource hud = exactSurface("world") as shape Hud`; absent fields default
and extra keys are ignored. It needs no app data module. `Sim::take_published`
drains changed state; a rebuilt or restored simulation publishes again.
The first live canvas owns its surface name: other instances cannot publish or clear its record and produce one diagnostic naming the surface.
`World::emit("won")` separately queues a string for the canvas's `message=` handler.
Undelivered events are saved in order. An empty queue adds no world save bytes.

## The agent's interface

No ninth operation (LLP 1041.001). `tree`, `state`, `layout`, `logs` and `clock`
reach the world through one export on the module; an entity is a target
(`world:fox`); `clock` is the only thing that moves the world; the journal is how an
agent hears. Capture the complete simulation with `s.screenshot('run.world', 'world', 'save')`
(CLI: `screenshot run.world world save`). `open({world: 'run.world'})` or
`--world run.world` holds the bytes until Play creates the first carrying surface,
then restores before its first render. Web, macOS and the iOS Simulator use the
same forms. The capture replies with the byte count, world hash and tick; state
reports `restored: true` until the next tick. Current app bindings win over saved
arguments. The iOS path is implemented but has not been driven in this session.

`state world:*` reads every entity's components in one reply (512 maximum,
then `truncated: true`); `state world:* under world:player` narrows to a subtree.
`s.type('world', {key: 'KeyW', for: 1500})` presses, advances the agent clock,
then releases. The reply and transcript retain all three steps.

`game/proof.mjs` supplies `proof(import.meta, async ({open, check, equal}) => { … })`.
`open()` builds this game's app only when its inputs change, opens a fresh session,
and records every operation. `check(label, condition)` reports failures without
stopping independent assertions; `equal(a, b)` compares JSON values. Every session
is closed and recorded children are checked before exit. Artifacts are in the game's
`artifacts/` directory; the first CLI argument selects web, macOS, or iOS.

## Working here

`cargo test` in this directory is the engine's loop; `bun ../scripts/caps.mjs` still
holds every file to 1,500 lines. A game is driven like any app, with
`EXACT_APP_DIR=game/games/<name>`.

A seekable world is still when the last tick changed no component of any entity
without `Ambient`, no spring is moving, and nothing called `world.busy(reason)`.
Resources do not count as visible changes. Sim samples only the last two ticks of a
seek (or the starting state and final tick for one tick); live play does no sampling.
`clock` lists up to eight changing component names. Springs supply their deadline;
other work proposes 100 ms, doubling to 2 s until settled. `busy` borrows `&World`.

`scene::follow(world)` steps saved `Follow` components where called. It snaps on its
first tick and after `world.teleport` of the target, then uses `math::ease` with a lag
in seconds. Scalar and Vec3 easing both arrive exactly within 1e-4. Primitive sizes
live in `Mesh`; `exact_game_physics::Collider::of(&mesh)` matches every primitive.

Small is a feature. When something here feels clunky, slow or bloated, the move is
to delete it and try again, not to configure it.

## The world dev loop

`EXACT_APP_DIR=game/games/greybox bun host/web/dev.mjs --loopback` uses the same
core dev server as every app. Rust source edits exclusive to the GPU cdylib's
Cargo dep-info rebuild only that module under `gpu-dev`, then swap it under the
live canvas with its entire simulation carried. Shared/app inputs still rebuild
the app. Failed builds leave the old module running; incompatible carries leave a
fresh world and a dismissible error naming the refused field/type. Agent pages
never auto-swap.

A carry keeps the old setup's entities. New component fields default by name;
changing `setup` does not respawn a carried world. Reload the page or press
`f` then Enter in the dev server to start fresh. Contract edits carry uniquely
named surfaces across the plan restart; duplicate surface instances restart fresh
because their reassigned view ids cannot identify them honestly.
