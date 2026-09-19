# game/ — the game engine add-on

- To see the game: `bun game/dev.mjs beacons` (commands run from the repository root).
- To change it: edit `game/games/beacons/logic/src/lib.rs` or `game/games/beacons/app.contract`; the dev page reloads.
- To format the UI: `cargo run -q -p contract -- fmt game/games/beacons/app.contract` (`--stdout` previews; `--check` prints a diff).
- To find UI declarations and references: `cargo run -q -p contract -- symbols game/games/beacons/app.contract` (JSON, including values in named world arguments).
- To verify gameplay and HUD: `bun game/games/beacons/proof.mjs` (GPU-less Linux, incremental `gpu-dev`; E8 warm measurement: 0.512 s. E10 warm timing is not verified: native startup timed out).
- Empty pins → fill: `bun game/prove.mjs <name|path>` (all modes, Linux and web; omit `--repin`).
- Existing pins → verify: `bun game/prove.mjs beacons` (one Linux proof).
- To compare hosts: `bun game/prove.mjs beacons --hosts linux,web --compare-saves`.
- To verify pixels: `bun game/games/beacons/proof.mjs web`.
- To drive the simulator: `env -u SDKROOT DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer EXACT_UPDATE_TRUST=development EXACT_IDENTITY=- bun game/games/beacons/proof.mjs ios`.
- To check save/setup determinism: `bun game/games/beacons/proof.mjs linux --paranoid`.
- When pins move intentionally: `bun game/prove.mjs beacons --repin --reason "describe the saved-state change"` (all modes, Linux and web).
- Without a web carrier: `bun game/prove.mjs beacons --repin --hosts linux` (records Linux only).
- When something stalls: `EXACT_APP_DIR=game/games/beacons EXACT_WEB_DIST=game/games/beacons/dist bun scripts/agent.mjs web "tap play" "clock settle" "state world:* busy" state logs`.
- To see a box or blocker: `EXACT_APP_DIR=game/games/beacons EXACT_WEB_DIST=game/games/beacons/dist bun scripts/agent.mjs web "tap play" "layout world:player"`.
- To explain a stalled/refused proof (or report a clean run): `bun game/prove.mjs beacons --report`.


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

A game implements `setup` and `tick`; components and resources are ordinary `Data`.
This small example is compiled by `cargo test --doc -p exact-game`:

```rust
use exact_game::*;
struct Example;
impl Game for Example {
    const ID: &'static str = "example";
    type Args = ();
    fn setup(w: &mut World, _: &()) {
        w.spawn_named("player", (Transform::default(), Mesh::cube(1.0)));
    }
    fn tick(w: &mut World, _: &Input, _: &()) {
        w.require_mut::<Transform>("player").position.x += w.dt();
    }
}
let mut sim = Sim::<Example>::new(()).unwrap();
sim.run(1000.0);
assert!(sim.local_position("player").unwrap().x > 0.0);
```

See [Beacons](games/beacons/logic/src/lib.rs), [Greybox](games/greybox/logic/src/lib.rs)
and the [new-game template](new/logic/src/lib.rs) for complete movement, HUD and input examples.

`Character` composes `Move`, `Jump` and `Gravity`. Its velocity and configuration
are saved and hashed as a component on the player. `step` reports `displacement`, `grounded`,
`jumped` and `landed`; bounds stop outward velocity, braking and landing arrive
exactly. It uses no physics dependency; collider worlds can use Rapier's controller.
`near` and `near_xz` return `(entity, pose)` in entity order, with an inclusive radius
and global positions, including parent chains (XZ ignores height). A missing origin yields no rows.

- **A tick is a function that calls functions.** No scheduler, no plugins, no
  system parameters. Physics is `physics::step(world)`, written where it runs.
- **All state is in the `World`, and all of it is `Data`**: one derive gives the
  save game, the hash, the agent's JSON, the level file, and what a dev reload
  carries. A `Game` has no fields.
- **Arguments are a struct, bound by name.** `world(seed=7, paused=paused, restart=again)`
  follows the fields of `#[derive(Args)]`; names may be reordered and omitted fields
  use the struct's Rust defaults. Unknown names and invalid values refuse before
  changing the world or clock. A call uses either names or positions; positional
  calls follow declaration order. `world()` takes every struct default; `world(7)`
  takes the remaining defaults. Excess arguments and unknown names refuse at bake.
  Raw host JSON is transport, never a hash input: Apple may reorder object keys;
  saved arguments use declared field order. Plan v5 changes plan bytes; world pins are unchanged.
  `#[derive(Args)]`
  supports bool, u32/u64, i32/i64, f32/f64, and String (`()` for none).
  Unmarked fields construct; `#[live]` fields are read each tick.
  `#[restart] pub restart: bool` marks a restart edge: either transition uses the
  same setup reconstruction path. In Contract, `state again = false`,
  `action restart writes again` / `again = not again`, then `world(seed=7, paused=paused, restart=again)`.
  The explicit boolean replaces a generation counter; `state.world.restarted` counts
  reconstruction in this session, outside the save/hash. No ninth operation.
  Integer bounds are checked before casting; 64-bit fields accept safe f64 integers. A timed
  `bind(values, Some(at_ms))` validates first, seeks under the old arguments, then
  swaps. `Game::validate` runs before construction, binding, seeking for a bind, or restore; a refusal changes nothing. Hosts construct with `Sim::from_values`. Saves encode argument fields by name: reordering is safe, additions default, removals are ignored. Saves carry the game's `ID`, world time and dynamic input; the first restored host clock
  establishes a new epoch under live scheduling. Controlled restore anchors at the
  destination's current host stamp, so the first seek advances its full duration.
- **Time is an input.** Under the seekable clock, `tick = floor(clock_ms × hz / 1000)`; a step is `1/hz`
  exactly; there is no `delta`. Rendering interpolates between the last two ticks,
  so motion is smooth at any refresh rate and the simulation never knows.
- **Names are first class.** `world.get::<Transform>("fox")` accepts a name or an entity handle; `world:fox` addresses it in the agent. Consuming `query()` yields guarded items (mutable bindings use `mut`); `.iter()` yields `(Entity, item)` with plain references. `.one()` returns an item and refuses multiple matches in all builds.
  Name lookup is logarithmic in distinct names and resolves duplicate names to the lowest living slot. The index is derived and never saved or hashed.
- **Iteration is in entity order, always** — storage scans presence bitmasks in
  ascending index order, so a world loaded from a save replays exactly as the one
  that wrote it.

Warm E8 Beacons whole-command measurements: **0.512 s** for
`bun game/games/beacons/proof.mjs`, **4.327 s** for its `web` variant.
A generated template under `game/games/` measured **0.306 s** Linux and **2.930 s**
web (warm compiler/artifact cache). Web reuses one document target in the same Chrome carrier between ordinary stages,
reloading an isolated document after releasing keys and contacts, clearing origin
storage (retaining HTTP cache), and resetting history. `fresh`/`world` close that
carrier and spawn a new process. Build
and packaging reuse the content/artifact receipt.

## Author-facing edges

`#[derive(Kind)]` defines typed component bundles without additional storage.
`world.spawn_kind(name, bundle) -> Id<K>` and `world.bind::<K>(target)` establish
checked identities; `row(id)`, `edit(id, closure)`, `rows::<K>()` and
`rows_mut::<K>()` use joined component columns. `#[read]` keeps an editing field
shared, `Option<C>` is optional membership, and `#[child("name", field)]` binds a
saved `Id<Child>` field. Child identities are saved once in their component;
validation refuses stale or missing components before taking write leases.
`world.the::<K>()` requires exactly one structural match. Kind iteration admits
at most 200,000 entity slots, 64 cached kinds and 32 nested edits, with named
refusals beyond those bounds. `Id<K>` has exactly `Entity`'s wire representation.

`place::{ring, ring_jittered, grid, line, facing, on_top_of, next_to, blockout}`
supplies deterministic procedural placement. `place::scatter(seed, min, max, count,
min_distance) -> Result<Vec<Vec3>, ScatterError>` returns a complete packing or a
named refusal: at most 4,096 points, 64 attempts per point and 1,000,000 distance
comparisons in total. Worst-case work is quadratic below those bounds. These
helpers use a private seeded RNG and leave the world's RNG unchanged.

These small changes keep the tick and its ordering explicit. The examples come
from the games linked above; Equivalent spellings preserve pins; moving saved glow state into `Glow(Tween)` intentionally changes Beacons and Greybox saves.

| Before | After |
|---|---|
| Beacons: `w.nearest_xz_where::<Beacon>("player", 1.5, \|b\| !b.lit)`, then `w.get_mut::<Beacon>(e).unwrap()` | `w.nearest_xz_mut::<Beacon>("player", 1.5, \|b\| !b.lit)` returns `(entity, guard)`, preserving the prompt's identity, inclusive radius and lowest-index tie. |
| Beacons: joined `(&Beacon, &mut Material)` loop to count and copy the glow | `w.count::<Beacon>(\|b\| b.lit)` counts without a mutable material lease; a `Glow(Tween)` component supplies presentation intensity. |
| Camera: `scene::follow(w)` at the end of each game tick | `Follow` steps automatically after the game tick. An explicit `scene::follow(w)` remains available for ordering and is not stepped twice. |
| Proof: `(await session.state()).world[0]` to populate the summary | `await session.world('world').snapshot()` records the same world observation. |
| Skinned: `w.get_mut::<Transform>("fox").unwrap()` | `w.require_mut::<Transform>("fox")`; `w.require::<Animator>("fox")` also names the entity and missing component on failure. |
| Placement: `Placed::child(1).width(1.8).facing(Facing::Fixed)` | Still ordinal. `Placed::named_child("sign")` is pending child-identifier delivery through the GPU/host seam; it is not yet an available API. |
| Sprites: `SpriteAnimation::new([[0, 0, 16, 16], [16, 0, 16, 16]], 6.)` plus a separate initial rectangle | `SpriteAnimation::strip([0, 0], [16, 16], 2, 6.).sprite(Sprite::new("strip.tex", [24., 32.]))` initializes the first rectangle; explicit frame lists use the same `.sprite(...)` pairing. |
| Skinned: `fox.translate_local(motion.root_motion("fox"))` | `motion.apply_local(w, "fox")`, after `w.require_mut::<Transform>("fox").rotation = Quat::from_rotation_y(-end.seconds() * 0.45)`. `animation::step(w)` and marker reads remain explicit. |
| Sprites tests: `Sim::new(())`, then `s.load_assets(...)` | `Sim::with_assets((), \|_\| Ok::<_, String>(atlas())).unwrap()` uses the same loader without engine-owned fixture paths. |
| Greybox: `world.register_audio()` then `world.resource_mut::<Sounds>().add(...)` | `world.sounds([` installs saved audio types and the inline `"footstep"`, `"chime"`, and `"wind"` definitions together. |
| Asset tests: `World::assert_pin(include_str!("../../pins.json"), "asset-fixture", 60, sim.world().hash())` | `sim.assert_pin(include_str!("../../pins.json"))`. |
| Beacons Contract: `world(7, paused, again)` | `world(seed=7, paused=paused, restart=again)`. Omitted fields use Rust defaults; positional calls retain their binding order; plan v5 changes the plan bytes. |
| Sprites: interpreting `layer: i as i32` as a 2D z-order | The same field is an **equal-depth tie breaker**: translucent geometry sorts back-to-front by depth first, then layer. It does not override depth. |

`Material::glow` creates an opaque emissive mesh that writes depth; bloom supplies
its visible glow. It is not an unlit alpha halo. The starter combines `Glow(Tween)` with `Material::glow`, a ground grid, pads and
explicit sky-colored height fog. The renderer samples the tween at frame time;
no tick copies intensity into a material. Its HDR shoulder preserves highlight
headroom at full bloom.


Beacons' tick before E10 (verbatim):

```rust
# use exact_game::*;
# struct Options;
# #[derive(Default, Component)]
# struct Beacon { lit: bool, glow: Tween }
# #[derive(Default, Data)]
# struct Hud { lit: u32, near: String }
    fn tick(w: &mut World, input: &Input, _: &Options) {
        w.character("player")
            .step(input.stick_xz("move"), input.pressed("jump"));
        let near = if let Some((entity, mut beacon)) =
            w.nearest_xz_mut::<Beacon>("player", 1.5, |b| !b.lit)
        {
            if input.pressed("light") {
                beacon.lit = true;
                beacon.glow.to(w.tick_end(), 1.0, 0.5);
            }
            if beacon.lit {
                String::new()
            } else {
                w.name(entity).unwrap_or("").to_owned()
            }
        } else {
            String::new()
        };
        let mut count = 0;
        for (beacon, mut material) in w.query::<(&Beacon, &mut Material)>() {
            material.emissive = [beacon.glow.value(w.tick_end()) * 3.0; 3];
            count += u32::from(beacon.lit);
        }
        w.publish_record(&Hud { lit: count, near });
        scene::follow(w);
    }
```

After E10 (verbatim):

```rust
# use exact_game::*;
# struct Options;
# #[derive(Default, Component)]
# struct Beacon { lit: bool }
# #[derive(Default, Data)]
# struct Hud { lit: u32, near: String }
    fn tick(w: &mut World, input: &Input, _: &Options) {
        w.character("player")
            .step(input.stick_xz("move"), input.pressed("jump"));
        let near = if let Some((entity, mut beacon)) =
            w.nearest_xz_mut::<Beacon>("player", 1.5, |b| !b.lit)
        {
            if input.pressed("light") {
                beacon.lit = true;
                w.get_mut::<Glow>(entity)
                    .unwrap()
                    .0
                    .to(w.tick_end(), 1.0, 0.5);
            }
            if beacon.lit {
                String::new()
            } else {
                w.name(entity).unwrap_or("").to_owned()
            }
        } else {
            String::new()
        };
        w.publish_record(&Hud {
            lit: w.count::<Beacon>(|b| b.lit),
            near,
        });
    }
```

`Game::CAPTURE_SUPPORTED = true` opts into replay whose dependencies are fully
represented by world state, arguments, input, time and loaded artifacts.
`sim.start_capture(build, CaptureLimits)` / `stop_capture()` produce an EXCAP v2
window. `Capture::{to_bytes,from_bytes}` and
`Sim::replay_capture(capture, loaded_build, through)` validate artifact identity,
checksum and every boundary before reporting a replay. For baked assets, use
`live.replay_capture_using(...)`; it shares the destination's delivered immutable
assets without changing that destination. Imported script descriptions are never
executed. Bounds are 8 MiB, 16,384 records and 216,000 ticks, with an explicit
incomplete prefix on exhaustion and a shared decode-allocation budget.

`Sim::input` retains delivered-event pacing; `device_input` states that origin
explicitly. `scheduled_input` keeps a future stamp under live pacing. EXCAP saves
fractional scheduling separately from EXSIM; EXSIM v7 retains queued delivery metadata.
`handoff(agent)` releases keys and named controls; `Game::release_input` can also
clear physical-input argument bindings. `rebase(now_ms, release_input)` changes
the host anchor without advancing a tick. Ordinary state omits host timing;
`sim.agent(r#"{"op":"state","clockState":true}"#)` requests those diagnostics.
Layout's remembered inspection viewport is separate from saved input dimensions.

The proof callback supplies `capture(session, target, options)` and
`replay(path, options)` through its capture tools. The driver's
`session.world(name).ticks(count)` verifies the exact resulting tick, refusing a
mismatch without retry. `bun game/prove.mjs <game> --hosts linux --paranoid`
runs the existing three-mode comparison through the same recorder.

## Proof pins

`state.world.game` exposes `Game::ID`; pin publication requires the same observed
identity in every host/mode, including external games whose directory has another
name. Each game's `pins.json` is the authority for simulation tick and continuation-save goldens. Algorithm constants and synthetic hash-format fixtures are outside this claim. Its proof calls `pin(tick, state)`
and `pinSave("continuation", path)`; Rust tests use `sim.assert_pin(include_str!("../../pins.json"))`, deriving identity, tick and hash.
The tiny Data parser skips metadata; it is smaller than a generated `pins.rs` build step.
`bun game/prove.mjs beacons --repin` runs continuous, Save and FreshGame on Linux
and web, refuses incomplete or disagreeing tick/save observations, then rewrites only
`pins.json` among authored sources (refusing if manifest normalization would write) and prints old → new. Other proof assertions still run. `hosts` records
only exercised hosts, also named in `generated`. A missing requested Chrome carrier
refuses; Linux-only repinning of an existing baseline requires explicit `--hosts linux`;
a first baseline always requires both Linux and web. `CHROME` selects the browser,
which runs headless. `at` is HEAD, not a claim that the working tree was clean.
With empty pins, `bun game/prove.mjs <name|path>` establishes the first baseline
through that same Linux/web agreement. Once pins exist, it defaults to one headless
Linux proof, including the Contract HUD. A single-host run builds only if its own
receipt is stale; it needs no separate build-only pass. `--hosts` selects hosts,
and `--compare-saves` defaults to Linux plus web. From the game's directory use
`bun /path/to/exact2/game/prove.mjs .`.
An ordinary proof exits nonzero and reports `UNVERIFIED` until the complete saved
tick and continuation-save baselines match. The summary's `status` field is
`PASS`, `UNVERIFIED` or `FAIL`; build/capture-only runs and repin collection also
remain `UNVERIFIED` because they do not check the full saved baseline. Screenshots, receipts,
transcripts and saves belong under ignored `artifacts/`; only README-cited evidence
is retained. No ninth operation. `--report` names unused facilities tied to observed
refusals/stalls, retaining failed summaries and counting only relevant successful queries.
A passing run reports no recorded stalls or refusals.

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

`w.publish_record(&Hud { beacons: count })` publishes a `#[derive(Default, Data)]`
record through its existing field traversal. Nested records, lists, options and scalars
keep their field names and JSON types; Contract validates them against `shape Hud`
at the app boundary. Typed numeric vectors publish arrays; enum variants are not Contract values.
`World::publish("beacons", count)` remains the scalar operation. Contract reads
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
agent hears. Capture the complete simulation with `s.world('world').save('run.world')`
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
arguments; `state.world.restoredFrom` shows the saved arguments while `restored` is true. Register saved types, including those first spawned mid-game, with `world.register::<Projectile>()` in `Game::register(world, args)` so standalone saves and captures can restore them before setup. Follow argument-dependent declarations in this hook: restore runs this hook on a scratch registry and fully decodes the world before gameplay setup. The hook must have no gameplay side effects. The iOS simulator carrier is driven by the same proof scripts. Explicit `size` uses a logical viewport fitted into the simulator window, so save bytes and projection checks use the same points as web/Linux; omit `size` to use the phone viewport. UIKit canvas input is labelled `recognized`, since UIKit exposes no synthetic touch constructor.

`layout world:entity` includes facing, signed horizontal bearing and mesh line of
sight to an optional `to` target. Visibility samples eight corners, six face
centres and the centre; it reports the hidden fraction and nearest four occluders.
These are geometric rays, independent of texture alpha. Sprites use camera-facing
bounds, sockets use current affine animation poses, and undeclared models need
explicit bounds to block. Reads preserve saves, hashes and ticks. Missing global
poses/facing are null; a transform-less mesh retains its identity screen rectangle.

The derived visibility index admits at most 262,144 entity slots (including dead
slots), with at most 1,000,000 entity/model-node visits to resolve current poses.
Its median BVH builds in O(N log N), with O(N) retained memory. Each layout/route
request shares a 1,000,000-visit ray/exclusion budget; cycles and excess work return
an explicit error. Pose, socket, sprite, camera, asset and hierarchy edits invalidate
geometry. Public optional layout/pick APIs return `None` on unavailable geometry.

`Sim::move_to(entity, target: Vec3, tolerance, speed) -> Result<(), String>` and
`world.moveTo(entity, [x,z], {tolerance, speed})` drive WASD toward **global** XZ
coordinates. Each has at most 160 bursts of ten ticks plus one released tick and
releases its key on clock refusal. A stall includes the nearest blocker and clear
side rays, also available from `Sim::route_diagnostic(entity, from, to)`. These
rays explain a failed route; they do not certify a swept-character path.

Saves use **EXSIM v7**; v5/v6 are refused with recreate/inspect guidance and no
migration. `restore(&[u8])` and `restore_bound(&[u8])` merge saved tick-zero Data,
loaded runtime Data and the current authored initializer. Changed authored fields
apply only while the runtime field still equals its saved base. `open_bound(&[u8])`
and `from_save(&[u8])` restore exact saved world Data; bound forms retain every
current app argument. Current live bindings and construction-base arguments are
saved separately. Every path fully decodes a typed scratch world and checks hierarchy, input and
exact tick/time agreement before any `Game::setup`. It then constructs one
candidate and applies the authored merge after decode, committing only after
full validation. Standalone `from_save` and capture replay require all saved
types in `Game::register(&mut World, &Self::Args)`; restoring into an existing
simulation also retains its registered types. Registration must be free of
gameplay side effects and follow argument-dependent setup variants.

`state.world.reload` reports applied, kept, added, removed and unmatched fields,
with 64 entries and an omitted count. Structural edits require restart, except
for the existing model adapter's authored animation-controller membership. Existing
animation clip/speed fields follow the three-way rule. The audio adapter retains
its explicit Carry policy: fresh named sound definitions win, runtime-only names
and already playing voices survive; Open keeps saved definitions. Reload projection
refuses more than 1,000,000 slots, 256 storage types, 16M visits, 512 MiB projection
work or a 128 MiB serialized base. Nested projection decode shares a 1 GiB allowance
(or the importing capture's smaller allowance); malformed or over-budget restores
leave the destination unchanged.

`state world:*` reads every entity's components in one reply (512 maximum,
then `truncated: true`); `state world:* under world:player` narrows to a subtree.
`s.type('world', {key: 'KeyW', for: 1500})` presses, advances the agent clock,
then releases on the same carrier even if the clock fails. CLI: `type world key KeyW for 1500`. The reply or error transcript retains partial steps.

`game/proof.mjs` supplies `proof(import.meta, async ({open, check, equal}) => { … })`.
`open()` builds this game's app only when its inputs change, opens a fresh session,
and records every operation. `check(label, condition)` reports failures without
stopping independent assertions; `equal(a, b)` compares JSON values. Every session
is closed and recorded children are checked before exit. Artifacts, saves and screenshots are in the game's
`artifacts/<host>/` directory; the first CLI argument selects web, macOS, iOS, or Linux.

## Working here

Commands run from the repository root unless stated otherwise:

```sh
bun game/new.mjs my-game
bun game/dev.mjs beacons
bun game/games/beacons/proof.mjs linux --paranoid
bun game/games/beacons/proof.mjs web
bun game/bench/size.mjs
```

Use a path to choose the game's directory: `bun game/new.mjs ./my-game`.
`bun game/dev.mjs ./my-game` and `bun game/prove.mjs ./my-game` use the same Exact2 hosts and
driver. A bare name creates `game/games/<name>`. The first bake puts the generated
workspace and hosts in that game's ignored `.shells/`; its logic is a member of
that workspace alone. Dependencies and profiles come from `game/Cargo.toml`;
build output uses the game's `target/`. Resolution and generation run no Cargo
and create no `.shells` workspace. Authored `app.json` stores identity, the game entry point and authored host overrides.
The bake derives `.shells/app.json`; it never rewrites an existing authored manifest.
The template supplies a captured source `Cargo.lock`
beside `app.json`. Keep that lock in version control. Every later bake and deploy
copies it into `.shells` and resolves with `--locked --offline`; a stale or corrupt
cache cannot choose versions. After deliberate dependency changes, run
`bun game/app/shells.mjs <game-directory> --update-lock` and review the source lock.
An authored logic manifest sets `package.workspace = "../.shells"`; use concrete
package fields and path dependencies so other crates can read it before a bake.
From an empty game directory, run `bun /path/to/exact2/game/new.mjs .`.
Generation only writes inside that game: it never normalizes another game or
updates `game/Cargo.lock`. `resolveApp` selects `<game>/.shells/Cargo.toml` in memory;
the first bake materializes it and locates adapters through Cargo metadata. No authored nested
workspace is needed. Its first build has a cold cache. Updating existing pins with
`--repin` requires a Git checkout for provenance; an initial external baseline
without Git is labeled as such. The cold starter passes Linux and web; Beacons uses this ordinary path on all
four hosts. See the [R11 receipt](diaries/002-ergonomics.md#r11-review-fixes-and-app-owned-starter).

A fresh checkout needs the existing host dependencies beside it: `../ibex` and
`../snapback-sb4` source checkouts. For an isolated verification checkout, use
`ln -s /path/to/ibex ../ibex` and `ln -s /path/to/snapback-sb4 ../snapback-sb4`
from the verification checkout so the links land in its private parent directory;
these are source dependencies, not build caches.

On a fresh checkout, first run `bun install --frozen-lockfile` at the repository
root (the `exact-js` build script needs Rolldown).

From `game/`, use `cargo test --workspace --no-fail-fast`,
`cargo clippy --workspace --all-targets -- -D warnings`,
`cargo fmt --all -- --check`, and `bun test ./proof.test.mjs`.
Also run `bun app/shells.mjs --test` from `game/`: this prepares and tests every
app-owned workspace (all seven games and `bench/cubes`) with `--no-fail-fast`,
including logic tests that are no longer engine-workspace members.
Then run `bun game/games/beacons/proof.mjs`; the proof performs its own first bake. The device lifecycle test is explicitly opt-in:
`cargo test -p exact-game-render surface_lifecycle -- --ignored` on a GPU host.
Local Cargo validation uses
`DEVELOPER_DIR=/Library/Developer/CommandLineTools`,
`SDKROOT=/Library/Developer/CommandLineTools/SDKs/MacOSX26.sdk`,
`EXACT_UPDATE_TRUST=development`, and `EXACT_IDENTITY=-`.
Apple host builds use Xcode's developer directory and no `SDKROOT`.
The [proof runner](proof.mjs) owns host setup and cleanup; game proofs own assertions.
Generated host adapters build as products; their empty Cargo test and documentation
harnesses are disabled. Tests remain in the authored engine and game crates.
Paranoid proofs run Off, Save and FreshGame; only web needs a final build-only
step to restore the ordinary artifact. Native keeps one executable.

The generated shell follows `app.json`: `game.assets: true` selects the model-capable
module; `game.audio: true` links the audio executor. Authored keys override generated
defaults; see [shell generation](app/shells.mjs). No core-crate Cargo feature selects
optional capabilities. Production module sizes and dated evidence live only in
[bench/README.md](bench/README.md#d4--module-size-2026-09-18).

## Assets and animation

Put models and sprite PNGs under `art/`; [the baker](bake/README.md) writes validated
`.model` and `.tex` assets. Declare simulation dependencies in `Game::ASSETS`.
Setup waits for that closure; cosmetic arrival cannot change gameplay reads or bounds.
`Mesh::asset(name).bounds(...)` supplies authored bounds for an undeclared cosmetic.
The primitive module refuses named asset meshes and sprites.

Animation execution stays in [animation.rs](engine/src/animation.rs), linked by games
that explicitly call it and by the model presentation executor for inspection/carry.
The smaller [pose data core](engine/src/asset/pose.rs) contains saved local poses and
rig geometry, without playback. `World` owns no animation-specific runtime field.
Registering a controller also registers its produced `Pose`; its derived cache
is created on first execution and excluded from saves. Put saved type declarations
in `Game::register` so a fresh process can decode before setup. Unregistered saved types refuse by name.

`let motion = animation::step(w)` returns owned markers/root motion. Apply movement
with `motion.apply_local(w, "fox")` after authored rotation, then read `animation::socket` for a tick-boundary
joint. Socket access refuses a controller's stale pose by target name: the order is
**step → apply Motion → socket**. `animation::socket_matrix` preserves the full affine
map. `SocketFollow::new("fox", "head").offset(t)` uses that joint and offset for
`global_position`, agent layout/pick and spatial audio at the tick boundary; its
saved `Transform` remains the unavailable-target fallback. At setup (tick zero, outside
a tick), an unstepped controller uses the bind pose, including after restore. Followers always compose the saved local pose with the current owner and offset;
an explicit `socket()` query still refuses an unstepped controller.
The renderer interpolates the owner hierarchy and joint chain locally before composing
the full affine matrix, retaining shear and mirrored winding. Unresolved
displayed followers share one warning per entity across placement, rendering and
history resets, outside saved journals; dead entities are pruned. Ordinary transforms use
shortest-path normalized linear quaternion interpolation; skin locals use spherical
interpolation. See the [skinned fixture](games/skinned-fixture/logic/src/lib.rs) and
[its continuation tests](games/skinned-fixture/logic/tests/sim.rs).

`Animation.sampled` is the saved reverse-playback initialization flag; signed zero
has no sentinel meaning. [Pins](games/skinned-fixture/pins.json) are the authority.

## Particles, sprites and placed children

Untextured `Emitter` presentation remains available to primitive games. Emission
intent is saved; positions are derived. Particle storage and pipelines prepare with
the first emitter; capacity grows at feeds up to the 65,536-particle admission ceiling. Sprites require the model-capable artifact;
its sprite pipelines and instance storage are absent from the primitive web path.
See the [particle game](games/particles-fixture/logic/src/lib.rs),
[sprite game](games/sprites-fixture/logic/src/lib.rs), and their proofs.

A `Placed` component associates a Contract child index with a world plane. The
browser composites CSS homographies; Apple captures child textures and can depth-test
them against the world. Hidden placement is explicit. Kernel layout, accessibility
and ordinary controls remain the app host's responsibility. See the
[placement fixture](games/placement-fixture/logic/src/lib.rs), its
[Contract](games/placement-fixture/app.contract), and [proof](games/placement-fixture/proof.mjs).

## Further reading

- [Engine API](engine/README.md), [renderer contract](render/README.md),
  [physics](physics/README.md), [audio](audio/README.md).
- [Design of record](../llp/1041.003-game-engine-as-built.explainer.md) and
  [agent contract](../llp/1041.001-agent-interface-to-a-game.rfc.md).
- [Bench receipts](bench/README.md) and [ergonomics diary](diaries/002-ergonomics.md).

Offline builds require a populated Cargo cache. After `bun game/new.mjs <path>`,
materialize its shell with `bun -e 'import {gameDefaults,gameShells} from "./game/app/shells.mjs"; import {resolve} from "node:path"; const dir=resolve(process.argv[1]); gameShells(dir,gameDefaults(dir).game,resolve("game"));' <path>`
and prefetch with `cargo fetch --locked --manifest-path <path>/.shells/Cargo.toml`.
A shell lock without a captured source lock is removed and refused; the committed
`game/new/Cargo.lock` is the first-bake seed, never a publisher's generated cache.

## Typed scene content and user checkpoints

`exact-game-scene::Types::prepare(content, &[Asset::new(name, sha256)])` validates
baked scene bytes against typed component readers and the baked asset manifest.
`PreparedScene::instantiate(&mut World)` constructs the initial world. Lanterns uses
`Game::ASSETS = &["fox.model"]`, `CapsuleController`, and explicit animation stepping;
its `world(..., scene=sceneContent())` construction uses named arguments throughout.
`bun game/dev.mjs lanterns --scene-only [--reuse-baker]` rebakes content in its
app-owned workspace. [Scene API and bounds](scene/README.md) describe fragments,
source provenance, work limits and reload behavior.

A canvas accepts integer `surface-save` and `surface-load` tokens. A changed positive
token requests one operation. The first owning canvas stores opaque bytes by app ID
and surface name. Save replies `surface-save:saved`; Open restore replies
`surface-load:loaded` after commit, including deferred assets. Failures reply the
corresponding `:error`. Other canvases cannot overwrite that surface's checkpoint.
Web uses localStorage; Apple/Linux use app data (`EXACT_SURFACE_STORE` for isolated
proofs). The common carrier limit is 256 MiB; filesystem writes replace atomically.

Development web reload stages the candidate module, declared assets, decoded
worlds and all published HUD records before replacing live canvases. Continue
uses Carry and the three-way authored merge; Restart uses requested arguments;
Restore uses Open for the complete selected checkpoint set. Named argument
objects retain field names and explicit `{}` through reload. `state.reload`
reports requested/loaded artifact identities, refusal, timing and the last
successful replacement. First usable frame means a rendering opportunity after
submission, not GPU completion or scanout. Native game code still requires a
rebuild/relaunch; native UI can restart with carry.

A reload admits at most 256 canvases, 16 asset-delivery rounds, 256 asset deliveries
and 256 MiB of asset bytes shared across candidates (64 MiB per asset). Publications
have 16 settling rounds, a shared 32 MiB text limit and 65,536 operations. A refused
candidate keeps the live executors; device recovery and authored reload serialize.
The original control bindings survive failed canvas attachment.

`session.clock({owner:'human'})` releases physical input and resumes live time;
`owner:'agent'` takes controlled time. Observing state does not acquire ownership.
On macOS/iOS simulator, `await session.detach()` requires a complete native
acknowledgement before closing the transport and releasing the proof's process
ownership. Ordinary `close()` cleans up its isolated launch. Linux and physical
phone carriers refuse detach. `session.world(name).source(entity)` returns typed
scene provenance or an explicit unavailable result.

### Fixed engine evidence and explicit authored launch

`open({host, app, plan, world, worldMode: 'carry'})` opens saved state against the
current compiled initializer; `worldMode` defaults to `'open'` for exact checkpoint
loading. Both preserve intentional saved input and the restored clock. Native
carriers use `EXACT_WORLD_MODE=carry` with `EXACT_WORLD`; web uses the same restore
mode. HUD checkpoint loads remain exact Open. Authored merge follows world decode.

`pin(tick, state, key = String(tick))` accepts a namespaced observation key while
requiring the exact numeric tick. Tick/save inventories must agree completely in
continuous, Save and FreshGame; no branch can omit another branch's observations.
The fixed I3 consumer runs with
`bun game/prove.mjs game/verification/lanterns --hosts linux,web`. Its first pins
are published only after both hosts agree. The trial harness under
`game/bench/trials` is opt-in and is outside default checks.


The GPU acceptance command is
`cargo test --manifest-path game/Cargo.toml -p exact-game-render restoring_fox_uploads_zero_asset_bytes_after_ready -- --nocapture`.
It is unignored and requires a real adapter. Initial Fox uploads must be nonzero;
repeated restores on retained residency must upload zero mesh/texture bytes.
Changed content and a replacement device must upload again. A machine without
an adapter fails this test and cannot certify GPU residency or rendering.

Game bakes validate surface names and arguments even in hidden UI branches.
The ordinary `bake_game` reads `Game::NAME` and `Args::FIELDS` directly, without
depending on the game renderer. `bake_registry` validates modules with several
surfaces against their exported arguments. Caltrain-style apps without a game
module do not check surface argument names at bake.

`exact_game_render::fixture::Recording::feed(&mut self, &mut Feed, &World)`
records the production primitive feed for consumer tests without a GPU. The
renderer fixture owns its frozen Fox model/texture; it depends on no game.
