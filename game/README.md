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
| `bake/` | `exact-game-bake` — build-time glTF, PNG/JPEG decode, geometry preparation and mip generation. Never linked into a running game. |
| `app/` | `exact-game-app` — the shared Rust-only bake for game UIs without data sources. |
| `derive/` | `exact-game-derive` — `#[derive(Data)]`, `#[derive(Component)]`, `#[derive(Args)]`. No `syn`. |
| `render/` | `exact-game-render` — the wgpu renderer and `WorldSurface`, the `exact_gpu::Surface` a canvas binds. |
| `physics/`, `audio/` | Rapier integration and optional synthesis/playback executors |
| `games/` | consumers: `greybox` (LLP 1041.000 S0), `beacons` |
| `bench/`, `twins/` | the same scenes here, in Godot 4 and in three.js; numbers, never checks |
| `diaries/` | what building with it was like, scored against the twins |

## The programming model

The complete example below is compiled and run by `cargo test --doc -p exact-game`.

```rust
use exact_game::character::Character;
use exact_game::*;

#[derive(Default, Args)]
pub struct Options {
    pub seed: u64,
    #[live]
    pub paused: bool,
    // Restart idiom: round is the world's identity; Play again increments it.
    pub round: u32,
}
#[derive(Default, Component)]
struct Player { character: Character }
#[derive(Default, Component)]
pub struct Beacon { pub lit: bool, glow: Spring }
pub struct SmallGame;
impl Game for SmallGame {
    const ID: &'static str = "small-game";
    type Args = Options;
    fn actions() -> Actions {
        Actions::new().stick("move", Stick::wasd().or_arrows())
            .button("light", &["KeyE"]).button("jump", &["Space"])
    }
    fn setup(w: &mut World, args: &Options) {
        w.reseed(args.seed);
        w.spawn((Transform::default(), Mesh::plane(40.0, 40.0), Material::grid([0.16, 0.23, 0.24], 1.0)));
        let player = w.spawn_named("player", (Transform::at(0.0, 0.9, 0.0), Mesh::capsule(0.4, 1.8), Material::rgb(0.8, 0.4, 0.1),
            Player { character: Character::new().ground(0.9).bounds_xz(-19.6..=19.6) }));
        w.spawn_named("camera", (Transform::default(), Camera::default(),
            Follow::new(player).offset(0.0, 9.0, 13.0).lag(0.15)));
        for (i, x) in [2.0, 6.0].into_iter().enumerate() {
            w.spawn_named(format!("plinth-{}", i + 1), (Transform::at(x, 0.1, 0.0),
                Mesh::cylinder(0.9, 0.2), Material::rgb(0.3, 0.4, 0.42)));
            w.spawn_named(format!("beacon-{}", i + 1), (Transform::at(x, 0.7, 0.0),
                Mesh::sphere(0.5), Material::default(), Beacon::default()));
        }
        w.publish("lit", 0);
        w.publish("near", "");
    }
    fn paused(args: &Options) -> bool { args.paused }
    fn tick(w: &mut World, input: &Input, _: &Options) {
        let dt = w.dt();
        if let Some((player, pose)) = w.query::<(&mut Player, &mut Transform)>().one() {
            player.character.step(pose, input.stick_xz("move"), input.pressed("jump"), dt);
        }
        let position = w.get::<Transform>("player").unwrap().position;
        let mut nearest = None;
        for (entity, pose) in w.near_xz::<Beacon>("player", 1.5) {
            let mut beacon = w.get_mut::<Beacon>(entity).unwrap();
            if !beacon.lit && input.pressed("light") {
                beacon.lit = true;
                beacon.glow.set_target(w.now(), 1.0);
            }
            let distance = Vec2::new(pose.position.x - position.x, pose.position.z - position.z).length_squared();
            if !beacon.lit && nearest.is_none_or(|(_, old)| distance < old) {
                nearest = Some((entity, distance));
            }
        }
        w.publish("near", nearest.and_then(|(e, _)| w.name(e)).unwrap_or(""));
        let mut count = 0;
        for (beacon, mut material) in w.query::<(&Beacon, &mut Material)>() {
            material.emissive = [beacon.glow.value(w.now()) * 3.0; 3];
            count += u32::from(beacon.lit);
        }
        w.publish("lit", count);
        scene::follow(w);
    }
}
fn main() {
    let mut game = Sim::<SmallGame>::new(Options { seed: 7, ..Options::default() }).unwrap();
    game.hold("KeyD", 500.0);
    assert!(game.settle());
    assert_eq!(game.world().published("near").unwrap().as_str(), Some("beacon-1"));
    game.tap("KeyE");
    game.run(100.0);
    assert!(game.get::<Beacon>("beacon-1").unwrap().lit);
}

```

`Character` composes `Move`, `Jump` and `Gravity`. Its velocity and configuration
are saved and hashed inside the player's component. `step` reports `grounded`,
`jumped` and `landed`; bounds stop outward velocity, braking and landing arrive
exactly. It uses no physics dependency; collider worlds can use Rapier's controller.
`near` and `near_xz` return `(entity, pose)` in entity order, with an inclusive radius
and authored Transform positions (XZ ignores height). A missing origin yields no rows.

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
  swaps. `Game::validate` runs before construction, binding, seeking for a bind, or restore; a refusal changes nothing. Hosts construct with `Sim::from_values`. Saves encode argument fields by name: reordering is safe, additions default, removals are ignored. Saves carry the game's `ID` and
  `SAVE_VERSION`, world time and dynamic input; the first restored host clock
  establishes a new epoch.
- **Time is an input.** Under the seekable clock, `tick = floor(clock_ms × hz / 1000)`; a step is `1/hz`
  exactly; there is no `delta`. Rendering interpolates between the last two ticks,
  so motion is smooth at any refresh rate and the simulation never knows.
- **Names are first class.** `world.get::<Transform>("fox")` accepts a name or an entity handle; `world:fox` addresses it in the agent. Consuming `query()` yields guarded items (mutable bindings use `mut`); `.iter()` yields `(Entity, item)` with plain references. `.one()` returns an item and refuses multiple matches in all builds.
- **Iteration is in entity order, always** — storage scans presence bitmasks in
  ascending index order, so a world loaded from a save replays exactly as the one
  that wrote it.

## Determinism — the contract (LLP 1041.001 D5)

Same seed, same tick-stamped inputs, same completed tick ⇒ the same `world.hash()`,
on every host, bit for bit. The seekable clock fixes the input-to-tick mapping.
What that costs, and the only rules a game author must remember:

1. No clock but the world's. No randomness but `world.rng()`.
2. Transcendentals come from `exact_game::math` (libm), never `f32::sin`.
3. No `HashMap` iteration in a tick, no threads in a tick.
4. State lives in components and resources, nowhere else.

The journal is telemetry: a record outside the world hash and observation, so a
read that logs (such as a malformed agent request) must not change the world's course.
Semantic `Data` has no interior mutability: derives introduce none; a manual
implementation that changes semantic state through a shared reference is outside
this contract, and quiescence and the hash cache are undefined for it.

Pixels are held to a band; simulation state is held exactly. The two game proofs
and saved physics pile demonstrate this on arm64 macOS, x86-64 Linux and Chrome
wasm; unexercised engine APIs do not inherit a measured parity claim.

## Publications and events

`World::publish("beacons", count)` updates the current public record. Contract reads
it with `resource hud = exactSurface("world") as shape Hud`; absent fields default
and extra keys are ignored. It needs no app data module. `Sim::take_published`
drains changed state; a rebuilt or restored simulation publishes again.
The first live canvas owns its surface name: other instances cannot publish or clear its record and produce one diagnostic naming the surface.
`World::emit("won")` separately queues a string for the canvas's `message=` handler.
Undelivered events are saved in order but excluded from the simulation hash.
An empty queue adds no world save bytes.

## The agent's interface

No ninth operation (LLP 1041.001). `tree`, `state`, `layout`, `logs` and `clock`
reach the world through one export on the module; an entity is a target
(`world:fox`); `clock` is the only thing that moves the world; the journal is how an
agent hears. Capture the complete simulation with `s.screenshot('run.world', 'world', 'save')`
(CLI: `screenshot run.world world save`). `open({world: 'run.world'})` or
`--world run.world` holds the bytes until Play creates the first carrying surface,
then restores before its first render. Web, macOS, Linux and the iOS Simulator use the
same forms. Linux loads the same module with no device: simulation reads, input,
clock, publications, saves and CPU point picks work; canvas pixels report unavailable.
The native bake binds the GPU product digest to the app and cohort before loading.
`EXACT_GPU_MODULE` (Linux) and `EXACT_GPU_DYLIB` (Apple) select a path only in a development-trust bake; the product must still match its baked digest.
Its screenshots paint the Contract UI with flat canvas rectangles. Both carriers refuse input files and captures above 256 MiB before
reading/encoding the carrier. A refused restore is reported once by the creating
operation and remains in that canvas's `state.world.restoreError` and journal;
other operations continue on the fresh world. The capture replies with the byte count, world hash and tick; state
reports `restored: true` until the next tick or setup-argument rebuild. Current app bindings win over saved
arguments; `state.world.restoredFrom` shows the saved arguments while `restored` is true. Register types first spawned mid-game with `world.register::<Projectile>()` in `setup` so a fresh world can restore them. The iOS path is implemented but has not been driven in this session.

`state world:*` reads every entity's components in one reply (512 maximum,
then `truncated: true`); `state world:* under world:player` narrows to a subtree.
`s.type('world', {key: 'KeyW', for: 1500})` presses, advances the agent clock,
then releases on the same carrier even if the clock fails. CLI: `type world key KeyW for 1500`. The reply or error transcript retains partial steps.

`game/proof.mjs` supplies `proof(import.meta, async ({open, check, equal}) => { … })`.
`open()` builds this game's app only when its inputs change, opens a fresh session,
and records every operation. `check(label, condition)` reports failures without
stopping independent assertions; `equal(a, b)` compares JSON values. Every session
is closed and recorded children are checked before exit. Artifacts are in the game's
`artifacts/` directory; the first CLI argument selects web, macOS, iOS, or Linux.

## Working here

From the repository root:

```sh
bun game/new.mjs my-game                 # create the three files and logic crate
bun game/dev.mjs my-game                 # the shared dev loop, on loopback
bun game/games/my-game/proof.mjs web      # or linux / macos
```

Games are members of this workspace: one lockfile, profiles and dependency pins in
`game/`, no workspace manifest or lockfile in a game. The generator registers the
new packages in that shared lockfile without upgrading dependencies. An author owns exactly this:

```text
game/games/my-game/
  logic/          Cargo.toml, src/lib.rs, tests/sim.rs
  app.contract
  app.json
  proof.mjs
```

`app.json` declares `"game": { "crate": "my-game-logic", "type": "SmallGame" }`.
Add `"audio": true` to `game` to include the sound executor; omit it for a silent
GPU module with no audio dependency. Audio games define sounds in setup and call
`audio::step(world)` in tick (see `audio/README.md`).

Audio verification is explicit: the web probe (`bun game/bench/probes/audio.mjs greybox`, or `EXACT_AUDIO_PROBE=1` on its web proof) asserts a trusted first gesture,
a running context with an advancing clock, and nonzero analyser RMS while wind
plays. It runs separately from deterministic world/hash checks. macOS tests drive
the actual callback with fixture buffers; they do not verify device output. The
AU3c macOS greybox proof could not launch because SwiftPM failed loading
`BuildServerProtocol`, even with SDK 26 and a native-build wrapper. Audio/render
Rust libraries build for iOS; iOS has not been driven on this machine.

`Surface::lifecycle` carries visibility and audio interruptions without changing
simulation; `Surface::clock` supplies ownership before input. Host delivery and
SurfacePlayer regressions are present, and dev carry preserves fresh definitions
for new voices while old voices retain theirs. The generated audio hook still
needs three forwarders in `game/render/src/lib.rs`, outside the supplied AU3c scope;
first-gesture/lifecycle integration awaits that permission. iOS interruption
handling is unproven on a device. See `audio/README.md` for the precise bounds and
remaining integration work.

The crate is the package in `logic/`; its name ends in `-logic`. The type can
include a module path. Resolving the app for dev, proof, build or deploy generates
`game/.shells/<app-id-hash>-{gpu,web,apple,linux}/` before Cargo metadata. These ignored
crates contain the GPU module and web/Apple/Linux bakes. Complete replacements are staged
outside the member glob and installed by rename; unchanged inputs keep their timestamps.
Renaming a logic crate reuses the app identity, and resolving removes deleted games’ shells.
The workspace glob includes them; its tracked `.gitignore` lets Cargo resolve an
otherwise empty glob before the first bake. Games embed the engine shaders and
have no `gpu/shaders` directory.
`cargo test` from `game/` runs the engine and every game's logic tests.
`bun test game/new.test.mjs` copies the real files in `game/new/`, builds and proves
the untouched game, then removes it. `bun scripts/caps.mjs` from the repository
root still holds every source file to 1,500 lines.

A world's observation starts `Unknown`, also after rebuilding, restoring, live
advancement, queued input, or live argument changes; every mutable storage lease invalidates the sample. Unknown is not quiescent.
A paused world with no queued input is quiescent because time is stopped, reports `changing: ["paused"]`, and becomes Unknown on unpause.
Seekable advances sample the last two ticks of each jump (the start and end for
one tick). Unmarked entities count by existence, components, and parented global
pose. The RNG counts as `resource.Rng`; parent-only markers use an identity local pose. Resources count unless their `Resource` implementation declares
`const AMBIENT: bool = true`; physics and audio executor bookkeeping opt out.
Observations are derived caches, excluded from saves. Pending future input keeps
the clock awake and bounds its next jump. Replies report up to eight changed
components, moving springs/tweens, and explicit `world.busy(reason)` reasons.

`sim.run(ms)`, `key_down`, `key_up`, `tap`, `hold(code, ms)`, and `settle()` read
like proof operations. `settle()` returns whether it reached rest within sixteen
rounds: an initial read and at most fifteen follow-up advances, exactly the host loop. Spring/tween deadlines and queued input bound the
jumps; other work backs off from 100 ms to 2 s. Only a settle request increases
back-off; ordinary clock reads do not. A settle jump observes its last two ticks:
a one-shot change entirely inside a 2 s jump can be skipped. It is an observation
of rest, not a record of every intermediate transition.

The engine places saved `Follow` components after setup and setup-argument rebuilds,
and initializes new followers on restore. Call `scene::follow(world)` in `tick`
where following should happen; setup needs no follow call.
It accepts a handle (no name search) or a name resolved by the same lowest-index rule as `world.get`, reads the target's current
parent chain, and reinitializes after teleport or a name's new incarnation.
The up-axis is transported from the previous view through vertical; coincident aim preserves rotation.
`math::ease` arrives within 1e-4; `Tween::to(now, target, seconds)` supplies a finite
smoothstep beside `Spring`, with a known settle deadline.

Primitive dimensions are instance data; animation creates no geometry and all
spheres share one draw per geometry pass (each shadow cascade draws again). The existing 48-byte material record is RGBA, metallic,
roughness, RGB emission, then three dimension floats (capsules: diameter, half
stem, diameter). At 200k slots it remains 9.6 MB; the two 40-byte transform records
remain 16 MB. Capsule cap signs occupy the reserved vertex UVs and position true
hemispheres without stretching them. Normals use inverse dimension scale.
Picking, layout and `Collider::of` use authored dimensions. A plane's slab is
1 cm thick, its top at Y=0; its box collider supports dynamic bodies.
`Mesh::asset("crate.model")` draws a baked model's mesh nodes under one entity.
The nodes keep their own materials; an optional entity `Material` multiplies base
colour and adds emission. Models supply layout/pick bounds after arrival; absent
models have no stand-in geometry.

Declare `Game::ASSETS = &["crate.model"]` for anything setup or simulation needs.
Setup and tick zero wait for all declared bytes, on headless hosts too. Missing or
malformed files refuse by name through the surface error. `state.world.loading`
lists outstanding names. Later `Mesh::asset` references request on first sight and
pop in when delivered; declare them up front if simulation reads their data.
`world.model(name)` reads immutable Data. Asset caches are excluded from saves and
hashes; restore and module carry re-request declarations before continuing.

Put `.glb`/`.gltf` sources and their image/buffer dependencies in a game's `art/`.
The synthesized GPU shell's `build.rs` calls `exact_game_bake::bake_art`; Cargo builds
the GPU product before the app bake and host asset copy. Each model becomes
`assets/<stem>.model`; add `/assets/*.model` to that game's `.gitignore`. Stems must
be unique even across art subdirectories. The standalone equivalent is
`cargo run -p exact-game-bake -- art/fox.glb assets/fox.model` (from `game/`).

The runtime decodes only `bin` Data and uploads plain vertices/RGBA8 mip chains.
No glTF, JSON asset parser or image decoder enters the module. The existing typed
bulk `Vec<f32/u32/u16/u8>` codec is unchanged: EXGAME v3 and EXSIM v5 remain current,
and existing saves retain identical bytes. The baker refuses sparse accessors,
morph targets, non-triangle primitives, unsupported vertex channels and extensions
other than `KHR_materials_emissive_strength`/`KHR_texture_transform` by name. UV0
transforms are baked per material texture; additional UV sets are currently refused.
Skins and TRS animation tracks round-trip as Data but are not played until S3b.

Small is a feature. When something here feels clunky, slow or bloated, the move is
to delete it and try again, not to configure it.

## The world dev loop

Live frames run ticks whose deadlines fall strictly before the next frame. The
lookahead is the last live frame delta, clamped to one simulation step, and is
zero until two live frames have been seen. A display gap still contributes at
most 250 ms of world time; pausing stops time. Matching 60 Hz frames and ticks run
one tick per frame; 59.94 Hz frames occasionally catch up an extra 60 Hz tick.
The live scheduler retains sub-microsecond frame precision to avoid rounding beats.
Scheduling and interpolation use integer microseconds scaled by `hz` (one step is
1,000,000 units), rounding only after scaling. Seekable clocks have no lookahead:
`clock +16`, tick hashes and `settleAt` keep exactly their existing boundaries.
Rendering uses `R = T + L - step`, one tick behind the scheduling horizon, and
`alpha = (R - (tick - 1) * step) / step` between the previous and current tick.
The Sim retains the exact lookahead used by its last advance. At steady live
cadence alpha is in (0, 1]; the clamp only guards missing history at startup,
restore, a clock-mode switch or the first frame after a stall. Tick zero draws the
initial pose. Seekable rendering retains the old `frac(T / step)` interpolation.
Live saves may carry one early tick; presentation lookahead is not saved.
`tick_phase` is mean alpha on frames that run ticks: near 1 means ticks run just
before they are needed.

Input is consumed by stamp, strictly before the tick's deadline, on both clocks;
future host stamps wait. A multi-tick catch-up therefore spreads input across its
ticks. Only a gap exceeding the 250 ms live cap collapses its input onto the first
remaining step. Inputs already delivered before an early tick's deadline need no
special live bypass. Determinism is the tick-stamped input record with the same
seed; seekable input retains its exact clock-to-tick mapping.

`bun game/dev.mjs greybox` uses the same
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
because their reassigned view ids cannot identify them honestly. A GPU swap stages
all replacement canvases before cutover; a create/bind/render failure leaves the
old worlds running. Dev bindgen glue has function scope so old Wasm instances can
be collected; production keeps its static ES module loader.

The native `Sim` and `session.world("world")` share `run(ms)`, `settle()`,
`tap(code)`, `hold(code, ms)`, `key_down`, `key_up`, `position(entity)` and `get`.
Rust reads use `get::<Component>(entity)`; JavaScript uses `get(entity, "Component")`.
JavaScript operations are awaited; `settle()` returns a boolean. `snapshot()` keeps
simulation fields only. These helpers dispatch the existing eight agent operations.
The generated game demonstrates nearby prompts, beacon plinths, and `round` as the
world's restart identity, with the same movement/light sequence in its test and proof.
