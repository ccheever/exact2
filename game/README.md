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
| `scene/` | `exact-game-scene` — [typed JSON authoring](scene/README.md), native content bake and the same runtime World. |
| `derive/` | `exact-game-derive` — `#[derive(Data)]`, `#[derive(Component)]`, `#[derive(Args)]`, `#[derive(Kind)]`. No `syn`. |
| `render/` | `exact-game-render` — the wgpu renderer and `WorldSurface`, the `exact_gpu::Surface` a canvas binds. |
| `physics/`, `audio/` | Rapier integration and optional synthesis/playback executors |
| `games/` | consumers: `greybox` (LLP 1041.000 S0), `beacons` |
| `bench/`, `twins/` | the same scenes here, in Godot 4 and in three.js; numbers, never checks |
| `diaries/` | what building with it was like, scored against the twins |

## The programming model

Start with **kinds**: named bundles over the existing component columns. A kind
adds no tag, storage, save bytes, or agent fields. The derive emits `HeroRef<'w>`
and `HeroMut<'w>` with named guarded fields and a typed `id`; nongeneric input
structs contain 1–8 distinct component types. `Option<Component>` makes a field
optional. `#[read]` keeps that field shared even in an editing row.

All Rust examples here run as doc-tests with `cargo test --doc -p exact-game`.

```rust
use exact_game::character::Character;
use exact_game::*;

#[derive(Default, Component)]
struct Player { character: Character }
#[derive(Default, Component)]
struct Beacon { lit: bool, glow: Spring }
#[derive(Kind)]
struct Hero { player: Player, transform: Transform }
#[derive(Kind)]
struct Light {
    #[read]
    transform: Transform,
    beacon: Beacon,
    material: Material,
}
#[derive(Default, Args)]
struct Options { seed: u64, #[live] paused: bool, round: u32 }
struct SmallGame;
impl Game for SmallGame {
    const ID: &'static str = "small-game";
    type Args = Options;
    fn actions() -> Actions {
        Actions::new().stick("move", Stick::wasd().or_arrows())
            .button("light", &["KeyE"]).button("jump", &["Space"])
    }
    fn setup(w: &mut World, args: &Options) {
        w.reseed(args.seed);
        let hero = w.spawn_kind("player", Hero {
            transform: Transform::at(0.0, 0.9, 0.0),
            player: Player { character: Character::new().ground(0.9).bounds_xz(-19.6..=19.6) },
        });
        // IDs erase explicitly when calling engine APIs that take Entity.
        w.insert(hero.entity(), Mesh::capsule(0.4, 1.8));
        w.insert(hero.entity(), Material::rgb(0.8, 0.4, 0.1));
        for (i, x) in [2.0, 6.0].into_iter().enumerate() {
            let light = w.spawn_kind(format!("beacon-{}", i + 1), Light {
                transform: Transform::at(x, 0.7, 0.0),
                beacon: Beacon::default(), material: Material::default(),
            });
            w.insert(light.entity(), Mesh::sphere(0.5));
        }
    }
    fn paused(args: &Options) -> bool { args.paused }
    fn tick(w: &mut World, input: &Input, _: &Options) {
        let hero: Id<Hero> = w.the::<Hero>(); // Exactly one, or a panic naming Hero.
        let position = w.edit(hero, |h: &mut HeroMut| {
            h.player.character.step(&mut h.transform, input.stick_xz("move"), input.pressed("jump"), w.dt());
            h.transform.position
        }); // Leases end at the call boundary.
        let mut count = 0;
        for mut row in w.rows_mut::<Light>() { // One lease per column for the loop.
            let d = row.transform.position - position;
            if Vec2::new(d.x, d.z).length_squared() <= 2.25 && input.pressed("light") && !row.beacon.lit {
                row.beacon.lit = true;
                row.beacon.glow.set_target(w.now(), 1.0);
            }
            row.material.emissive = [row.beacon.glow.value(w.now()) * 3.0; 3];
            count += u32::from(row.beacon.lit);
        }
        w.publish("lit", count);
    }
}
let mut game = Sim::<SmallGame>::new(Options { seed: 7, ..Options::default() }).unwrap();
game.hold("KeyD", 500.0);
assert!(game.settle());
game.tap("KeyE");
game.run(100.0);
let light: Id<Light> = game.world().bind("beacon-1")?;
let a = game.world().row(light)?;
let b = game.world().row(light)?; // Shared rows coexist; reads never dirty pages.
assert!(a.beacon.lit && b.beacon.lit);
# Ok::<(), KindError>(())
```

`row(id) -> Result<K::Ref<'_>, KindError>` checks liveness and required components.
`bind::<K>(name_or_entity) -> Result<Id<K>, KindError>` gives the same check to
entities instantiated by `exact-game-scene`; errors name the kind and missing
component. `Id<K>` is `Copy` without requiring K to be Copy, and its `Data` bytes
and JSON are exactly `Entity`'s. A decoded ID is checked when used. `edit(id, f)`
and `with_row(id, f)` are closure forms for established game invariants: they
panic with the same diagnostic if an ID becomes stale or incomplete.
`rows::<K>()` and `rows_mut::<K>()` visit entity order; `.one()` accepts zero or
one row and refuses ambiguity. `the::<K>()` requires exactly one. Membership is
structural: kinds with the same required components select the same entities.

Resolve named children once and retain the typed result in an existing component
or resource. `child::<K>(parent, "bulb")` checks both the kind and the direct Parent
relation. Names stay unchanged for agent targets such as `world:lantern-7/bulb`.

```rust
use exact_game::*;
#[derive(Kind)] struct Bulb { light: PointLight }
#[derive(Default, Component)] struct Lantern {
    // Derived binding: rebuild once after loading, preserving existing wire data.
    #[data(skip)] bulb: Id<Bulb>,
}
#[derive(Kind)] struct Lamp { lantern: Lantern }
let mut world = World::new(60, 0);
let lamp = world.spawn_kind("lantern-7", Lamp { lantern: Lantern::default() });
let bulb = world.spawn_kind("lantern-7/bulb", Bulb { light: PointLight::default() });
world.insert(bulb.entity(), Parent(lamp.entity()));
let bound = world.child::<Bulb>(lamp, "bulb")?;
world.edit(lamp, |l| l.lantern.bulb = bound);
let bulb = world.with_row(lamp, |l| l.lantern.bulb);
world.edit(bulb, |b| b.light.intensity = 5.0);
# Ok::<(), KindError>(())
```

Ordinary ID fields are saved as entities. Use `#[data(skip)]` only for bindings
that can be reconstructed from saved state; Lanterns caches its bindings in
Session/Lantern and rebuilds them on first use after restore. No per-tick name
formatting or lookup is needed. `edit_resource::<Session, _>(f)` similarly releases
the resource lease at return. All editing leases still lock entire columns;
read or edit another row of the same column after the call, or use a single row
iterator for multiple entities. Holding a row past iteration keeps its leases alive.

The raw layer remains available for engine code and ad hoc component access:

```rust
use exact_game::*;
let mut world = World::new(60, 0);
let entity = world.spawn_named("fox", (Transform::default(), Material::default()));
assert_eq!(world.get::<Transform>("fox").unwrap().position, Vec3::ZERO);
world.get_mut::<Transform>(entity).unwrap().position.x = 2.0;
for (pose, mut material) in world.query::<(&Transform, &mut Material)>() {
    material.emissive = [pose.position.x; 3];
}
assert_eq!(world.get::<Material>(entity).unwrap().emissive, [2.0; 3]);
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
- **Time is an input.** `tick = floor(clock_ms × hz / 1000)`; a step is `1/hz`
  exactly; there is no `delta`. Rendering interpolates between the last two ticks,
  so motion is smooth at any refresh rate and the simulation never knows.
- **Names are first class.** Name lookup is O(log n); duplicate names select the lowest live entity index, including recycled slots. The derived index is rebuilt on load and excluded from saves, hashes and observations. `world.get::<Transform>("fox")` accepts a name or an entity handle; `world:fox` addresses it in the agent. Consuming `query()` yields guarded items (mutable bindings use `mut`); `.iter()` yields `(Entity, item)` with plain references. `.one()` returns an item and refuses multiple matches in all builds.
- **Iteration is in entity order, always** — storage scans presence bitmasks in
  ascending index order, so a world loaded from a save replays exactly as the one
  that wrote it.


Name lookup diagnostic: `cargo test --manifest-path game/Cargo.toml -p exact-game
--release named_cost -- --ignored --nocapture` compares the original scan with the
index, looking up the last entity. One x86-64 Linux sample (ns/call): 1k names
2,925 → 61.7; 10k 33,263 → 95.3; 200k 1,233,892 → 140.9. Timing is diagnostic,
not a gate; this measures lookup only, excluding construction and formatting.

## Placement

`exact_game::place` supplies deterministic placement in meters, Y-up, −Z forward:

- `ring(count, radius)`, `ring_jittered(seed, count, inner, outer)`,
  `grid(cols, rows, spacing)`, and `line(a, b, count)` return allocation-free
  `ExactSizeIterator<Item = Vec3>` values. Rings start at +X toward +Z; grids
  are centered XZ rows. Lines include endpoints (one point means `a`).
- `scatter(seed, min, max, count, min_distance) -> Vec<Vec3>` reserves `count`
  points and tries at most `64 * count` candidates in the 3D box. Impossible
  packing returns fewer points, preserving spacing. Seeded helpers use `Rng`
  privately, without advancing world randomness.
- `facing(from, to) -> Quat` aims −Z, with +Y up; coincident points give identity
  and vertical aims use +Z as the roll reference.
- `on_top_of(&World, impl Target, own_height) -> Vec3` and
  `next_to(&World, impl Target, Side, gap, own_size) -> Vec3` use layout's mesh
  bounds transformed through the current parent chain. Own dimensions are full
  world dimensions for an unparented, axis-aligned object. Missing targets or
  poses panic; `Side::{Left, Right, Front, Back}` are −X, +X, −Z, +Z.
- `blockout(&str, cell, origin, impl FnMut(char, Vec3)) -> Result<(), BlockoutError>`
  validates ASCII rectangular rows before any callback. Rows advance +Z, columns
  +X; `.` and space are empty. Errors identify the one-based row and column.

Dimensions must be finite and nonnegative; invalid arguments panic. The helpers
allocate no hidden world state. A tiny placement game in greybox's integration
suite exercises them without changing the shipped games' pinned positions.

```rust
use exact_game::{place, Mesh, Transform, Vec3, World};
let mut world = World::new(60, 7);
place::blockout("P.\n.P", 3.0, Vec3::ZERO, |ch, at| {
    world.spawn_named(ch.to_string(), (
        Transform { position: at, ..Transform::default() }, Mesh::cube(2.0)));
}).unwrap();
let at = place::on_top_of(&world, "P", 1.0);
assert_eq!(at, Vec3::new(0.0, 1.5, 0.0));
```

## Determinism — the contract (LLP 1041.001 D5)

Same seed, same inputs, same clock ⇒ the same `world.hash()`, on every host, bit for
bit. What that costs, and the only rules a game author must remember:

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

A canvas can request user-visible durable checkpoints with monotonic
`surface-save=` and `surface-load=` integer tokens. The host stores the surface's
opaque carry bytes under the manifest app identity and registered surface name,
then reports `surface-save:saved`, `surface-load:loaded`, or the corresponding
`:error` string through the existing `message=` handler. A successful load is
atomic; a refused or incompatible save leaves the live surface unchanged. The
first live publisher rule also defines the checkpoint slot: canvases registered
under the same surface name in one app intentionally share one slot. Apps that
need independent slots use distinct registered surface names.

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
Entity layout requests also accept an optional target:
`{"op":"layout","entity":"fox","to":"lantern-2","width":800,"height":600}`.
The reply adds `entity.visible.occluded` (0–1) and `occluders` (at most four
names, nearest ray intersection first, entity index breaking ties; unnamed
entities use `#index`). Occlusion counts the eight oriented-bound corners,
six face centres and centre equally. It is a geometric estimate, independent
of frustum status, material opacity and rendered pixels. Hidden entities and
singular transforms do not obstruct. Rays use the same primitive intersections
and authored model boxes as picking; unloaded assets use a unit box.
`entity.facing.forward` is normalized world −Z, `towardCamera` its dot with
the normalized direction to the active camera (null without camera/viewport).
With `to`, facing also contains `bearingTo` (signed degrees about +Y, −180…180),
`distanceTo` (world-origin distance) and `lineOfSight` (open origin-to-origin
segment, excluding both endpoint entities and their descendants). Coincident origins have distance
and bearing zero and clear sight; zero horizontal directions have bearing zero.
Visibility retains `{"unavailable":true}` without camera/viewport.
These queries take only shared world reads and preserve the mutation epoch.

The native bake binds the GPU product digest to the app and cohort before loading.
`EXACT_GPU_MODULE` (Linux) and `EXACT_GPU_DYLIB` (Apple) select a path only in a development-trust bake; the product must still match its baked digest.
Its screenshots paint the Contract UI with flat canvas rectangles. Both carriers refuse input files and captures above 256 MiB before
reading/encoding the carrier. A refused restore is reported once by the creating
operation and remains in that canvas's `state.world.restoreError` and journal;
other operations continue on the fresh world. The capture replies with the byte count, world hash and tick; state
reports `restored: true` until the next tick or setup-argument rebuild. Current live bindings win; saved setup arguments retain the world’s construction identity; `state.world.restoredFrom` shows the saved arguments while `restored` is true. Register types first spawned mid-game with `world.register::<Projectile>()` in `setup` so a fresh world can restore them. The iOS path is implemented but has not been driven in this session.

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

Audio integration is verified on macOS and the web; iOS is built but not driven
(this machine cannot run iOS today), and interruptions are not handled. AU3c owes
an `exact_gpu::Surface` visible/hidden lifecycle hook, reached through a GPU module
entry point after the assets slice lands, so hidden surfaces stop device work and
Apple suspend/resume and `AVAudioSession` policy can be wired. AU3c also owes the
`sim.rs` dev-carry fix: preserve the fresh `Sounds` registry and overlay it after
carry, leaving each serialized voice definition untouched; the regression must
carry old voice A alongside registry B and prove a new voice uses B. Journal/state
proof alone does not verify sound: the web needs a trusted gesture, an
`AudioContext` in `running` state with advancing `currentTime`, and nonzero analyser
RMS while wind is active; Apple needs stereo frames captured from the real render
callback, as in `apple_tests`. The opt-in greybox web probe (`EXACT_AUDIO_PROBE=1`)
now observes trusted resume, a running clock advancing 2.784 s, and wind RMS up to
0.00280; input before the first frame still needs the live/seekable input plumbing
outside this slice's permitted edits.

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

The ignored Lanterns timing test runs five fresh simulations for 10,000 ticks
each and prints the median nanoseconds per tick. It includes clock advancement,
physics, follow, audio and observation, with W held and sound enabled:

```sh
cargo test --manifest-path game/Cargo.toml -p lanterns-logic --test timing tick_10k_median -- --ignored --nocapture
```

The T3b Linux comparison in the same dev/test profile measured 38.409 µs before
and 38.565 µs after (+0.4%; shared-machine timing, no demonstrated speedup).
These paired samples preceded the builder's switch to zero debug information
and disabled incremental compilation. Lanterns' `src/lib.rs` went from 621 to
557 lines and 28 to 9 `unwrap()` calls; its new 63-line kind/binding module makes
620 lines combined. Beacons stayed at 145 lines and went from 1 to 0 `unwrap()`
calls. Both tick paths have zero `unwrap()`/`expect`, lease-scoping blocks, or
repeated entity-name lookups; Lanterns rebuilds cached bindings once after load.
The new 60-line timing/regression test is separate from those source counts.
The regression test pins the original Lanterns hashes at ticks 0, 60 and 180;
`EXACT_KIND_BASELINE=<directory>` additionally compares complete world saves
against `lanterns-<tick>.world` captures from the original implementation.

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
models use a unit box for CPU queries and have no rendered stand-in geometry.

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

## Linux proof baseline (T0c, 2026-09-18)

The original converged trunk (`8189f90`) was compared by running both merge parents:
engine `01f4c48` (`origin/lane/game`) and DX `7b34fc5`
(`origin/llp-ship/game-dx-20260918/integration`). These failures are inherited;
T0c leaves their assertions, hash pins, save formats and runtime APIs unchanged.

| Linux proof assertion | Engine | DX | Trunk |
| --- | --- | --- | --- |
| Beacons: glow fully up at one second after E, after restore | pass | fail | fail |
| Beacons: restored continuation matches original | pass | fail | fail |
| Beacons: complete continuation save is byte-identical | pass | fail | fail |
| Greybox: saved queued jump executes after capture, after restore | pass | fail | fail |
| Greybox: D6 two sessions continue to the same simulation snapshot | pass | fail | fail |
| Greybox: D6 entire Sim save is byte-identical | pass | fail | fail |
| Lanterns: proof reaches the Linux session without web `dist/index.html` | game absent | interrupted | interrupted |

DX and trunk restore Greybox successfully at tick 30 with
`clockState.hostMicros: null`; the following `clock +500` stays at tick 30.
The first continuation advance establishes the epoch instead of executing ticks.
DX's `1e636eb` made inspection read-only, while `7b34fc5` establishes ownership
before restore; restore resets the host epoch. This needs an explicit host clock
anchor after restore. The intentionally invalid `refused.world` is a separate,
passing refusal test, not an EXSIM version mismatch in the valid checkpoint.

Lanterns' unconditional web adapter installation comes from DX's `1176d78`.
Its headless interruption must be repaired in that consumer; creating a web
distribution to conceal the dependency would not prove a headless launch.
DX also fails Beacons' old native-hash assertion twice; trunk's existing
`0x58d5d637a36c8365` pin passes and was not changed in this audit.

`cd game && bun test` requires more than the headless host: the generated-game
test builds successfully but explicitly launches a web proof and requires Chrome
on both parents. The feel `--no-build` test requires pre-existing Beacons 60 Hz
`dist/` and 120 Hz `target/feel120/` web bakes; it already fails without them on
engine and is absent on DX. Both failures are environmental on this machine.
DX separately has a missing `games/beacons/feel.mjs` import, already resolved on trunk.

Clippy's `type_complexity` diagnostic at `render/src/assets.rs:239` and rustfmt
differences in that file and `engine/tests/capture.rs` also reproduce on DX;
engine passes both checks. GPU pixels, browser execution and Apple SDK/runtime
behavior are outside this machine's verification capabilities.

Final trunk verification: the clean game workspace has **375 Rust tests passed,
0 failed, 9 ignored**; `bun test` has **38 passed, 2 environmental failures**.
The Linux proofs report Beacons **51 passed / 3 failed**, Greybox **57 / 3**,
Lanterns **1 / 1** (cleanup passes; the proof is interrupted), and asset-fixture
**6 / 0**. An untouched generated game separately passes its three Rust tests
and Linux proof. No merge-only failure was demonstrated. Parent scratch worktrees
were removed; cold rebuilds retained `CARGO_PROFILE_DEV_DEBUG=0`,
`CARGO_PROFILE_TEST_DEBUG=0`, and `CARGO_INCREMENTAL=0` after disk cleanup.
