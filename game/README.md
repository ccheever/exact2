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
    // Changing restart_generation reconstructs setup; Play again increments it.
    pub restart_generation: u32,
}
#[derive(Default, Component)]
struct Player { character: Character }
#[derive(Default, Component)]
pub struct Beacon { pub lit: bool, glow: Spring }
#[derive(Default, Data)]
struct Hud { lit: u32, near: String }
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
        w.publish_record(&Hud::default());
    }
    fn paused(args: &Options) -> bool { args.paused }
    fn tick(w: &mut World, input: &Input, _: &Options) {
        let dt = w.dt();
        {
            let mut query = w.query::<(&mut Player, &mut Transform)>();
            let (player, pose) = query.one().expect("one player");
            player.character.step(pose, input.stick_xz("move"), input.pressed("jump"), dt);
        }
        let position = Vec3::from(w.current_global(w.named("player").unwrap()).unwrap().translation);
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
        let near = nearest.and_then(|(e, _)| w.name(e)).unwrap_or("").to_owned();
        let mut count = 0;
        for (beacon, mut material) in w.query::<(&Beacon, &mut Material)>() {
            material.emissive = [beacon.glow.value(w.now()) * 3.0; 3];
            count += u32::from(beacon.lit);
        }
        w.publish_record(&Hud { lit: count, near });
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
and global positions, including parent chains (XZ ignores height). A missing origin yields no rows.

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
  swaps. `Game::validate` runs before construction, binding, seeking for a bind, or restore; a refusal changes nothing. Hosts construct with `Sim::from_values`. Saves encode argument fields by name: reordering is safe, additions default, removals are ignored. Saves carry the game's `ID`, world time and dynamic input; the first restored host clock
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

Audio verification is explicit: `bun game/bench/probes/audio.mjs greybox`, or
`EXACT_AUDIO_PROBE=1` on its web proof, checks both live frame/gesture orderings,
synchronous first-gesture resume, advancing device time, nonzero wind analyser RMS,
and retry after a refused WebAudio start. It reports gesture construction and
input-to-frame costs without withholding callbacks. Synthetic lifecycle tests cover
persisted pageshow and `document.hidden`; they do not claim an actual bfcache navigation.
Greybox, Beacons and the asset fixture retain their deterministic web hash pins.

`module!(Game, audio)` already forwards all GameAudio hooks. Generic
`Surface::lifecycle` carries Hidden/Visible/Interrupted/Resumed independently of
simulation; `Surface::clock` supplies ownership before input. Player reserves its
32 MiB PCM budget before synthesis and releases acknowledged allocations, trying
smaller candidates after refusals. When a preferred candidate waits for stopped PCM,
non-preferred sources stay stopped until acknowledgement frees capacity. Attached
`AudioSource`s require looped definitions and refuse finite definitions by name;
`World::play` records a finite voice's deterministic start tick. Registration validates
parameters; the Player owns the actual PCM budget. Finite voices are ambient and do
not block settle. File restore preserves saved sound registries; dev carry
overlays fresh definitions in GameAudio while retaining runtime names and frozen
finite voices. That policy belongs to the audio adapter, not Sim.

Apple tests drive the real callback with fixture buffers. The macOS Swift package
builds. Its synthetic macOS tests call the interruption method on the main thread
(they do not post `AVAudioSession.interruptionNotification` from a background queue)
and cover lifecycle delivery, window attach/detach,
process-wide no-resume inheritance and recovery across live sessions, a trusted
gesture latched before the first audio request, and 300-live-frame
activation retries. Automatic `exact:audio` requests cannot override no-resume.
The same shared cadence helper tests initial persistence, boundaries, 120 ↔ 80
hysteresis, dropped intervals and alternating 60/120 Hz sessions. These fixtures
open no audio device; iOS was not driven here. See `audio/README.md` for the bounds.

Owed: real-device Apple interruption and multi-display sweeps; WebAudio resume-failure propagation. The Swift fixtures and web proofs do not establish those claims (`review-F2f-sol.md`, `CanvasSeams.swift:507`; `review-F2f-grok.md`, `game/audio/src/web.rs:47–50,117–119`). Spatial `play().at(entity)` is tested while `Transform` is mutably borrowed; it resolves positions later, preserving a final position at despawn.

The crate is the package in `logic/`; its name ends in `-logic`. The type can
include a module path. Resolve checks manifest syntax and the package name, but
does not check Rust exports: compilation of the generated GPU shell diagnoses
missing/private types and supports module paths and macro/cfg exports. Resolving the app for dev, proof, build or deploy generates
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
`app.json` selects model support with `game.assets: true` (or `new.mjs --assets`).
The synthesized shell uses `module!(Game, assets)`; primitive shells link no model
decoder or model shader markers in the measured wasm. Asset-map and `Models`
storage isolation is still owed; this is not a claim that all model machinery is absent. A primitive module refuses asset meshes by
name at bind. `Mesh::asset("crate.model")` draws a baked model's mesh nodes under one entity.
The nodes keep their own materials; an optional entity `Material` multiplies base
colour and adds emission. Only declared models supply simulation data and baked
layout/pick bounds. An undeclared model is presentation-only: `world.model(name)`
always returns `None`. Give it authored bounds with
`Mesh::asset("prop.model").bounds([-1., -1., -1., 1., 1., 1.])` inside a spawn
bundle, or leave it unpickable. Arrival cannot change a game's reads or bounds.

Declare `Game::ASSETS = &["crate.model"]` for anything setup or simulation needs.
Declaring a model also declares every texture it references. Setup, tick zero,
clocks and saves wait for those dependencies. `Loaded` means validated content is
ready. Device preparation is separate: loss preserves content readiness, re-requests
texture bytes and prepares pipelines again. Module-level recovery is verified:
retained content is re-uploaded, pipelines re-prepare, and native fixture pixels are
identical on a replacement device. **Host recovery is owed.** Native hosts lack a
recovery ABI: Apple result 3 makes a canvas non-presentable and `gpu_load` would
destroy the module surface table. The web keeps the old instance’s presentation
surface/context, producing a black canvas. The required native ABI requests a
replacement device while preserving the surface table; web recovery must recreate
each canvas’s surface/context on the new device, as module swap does. Both analyses:
`review-S3ac-sol.md` (`gpu/src/native.rs:224`, `CanvasSeams.swift:353`,
`gpu/src/lib.rs:358`, `gpu-glue.js:148,583`) and `review-S3ac-grok.md`
(`gpu/src/web.rs:33–60`, `gpu/src/native.rs:90–99`). No renderer-owned decoded
mip copy survives upload; Apple’s generation store still retains delivered bytes. A loading carry refuses with named asset states, including when a
restore is deferred. Hosts retain a deferred carrier until `state.restored` confirms commit; a late
refusal populates the surface restore error and journal once and leaves the fresh
world usable. `Surface::restore(bytes, Restore::Carry)` overlays fresh audio
registrations on saved registrations; `Restore::Open` (the default) restores saved
Sounds. Runtime-registered saved names survive either mode.
`sim.load_assets(|name| std::fs::read(asset_dir.join(name)))?` drains headless
requests and dependencies without ticking. `let saved = sim.save()?` returns a
named error while declared assets or current mesh dependencies are pending; it
checks current roots even before the request drain. Failed cosmetics do not gate
save/carry; failed declarations still refuse. Saving does not panic. Every web agent operation waits at
the same bounded delivery barrier, including world-save screenshots. Loading does
not establish or advance the simulation's host clock epoch.

`state.world.loading` lists pending names. `state.world.assets` lists each declared
or requested name as `{name, state: "Pending" | "Loaded" | "Failed", reason?}`.
Failed declarations keep setup gated; refused clocks retain their named failures.
Missing or malformed cosmetic assets leave other presentation running. Asset
failures never use the surface's sticky rendering error. Caches, readiness and
failures are outside saves/hashes; a fresh restore re-requests declarations before
applying the saved tick, world and forwarded input.

Put `.glb`/`.gltf` sources and image/buffer dependencies in a game's `art/`.
The GPU shell's build script produces `assets/<stem>.model` containing geometry,
materials, nodes, skins, clips and named texture references. Each
`assets/<stem>/<name>.tex` is one RGBA8 texture with its complete mip chain. The
renderer requests textures as model materials arrive, shares them by name across
materials/models, uploads each once and releases the CPU mip payload. Models use
shared 1×1 placeholders until textures arrive. The `.baked-assets.json` manifest
records the SHA-256 digest of each generated output. Overwrite and pruning require
bytes matching that recorded digest, even when bytes equal the desired output.
Authored collisions refuse before mutation, including source renames and removal
of `art/`. Legacy array manifests migrate only listed, byte-equal desired outputs;
other old bytes are preserved. A differing legacy output refuses for manual review;
do not delete the manifest to bypass ownership. Ignore this manifest and the
generated `.model`/`.tex` files. Stems must be unique across art subdirectories.
The standalone baker is `cargo run -p exact-game-bake -- art/fox.glb assets/fox.model`
(from `game/`); its texture files accompany the model under the output directory.

Names use the shared `gpu::asset_name` rule: nonempty ASCII, at most 128 bytes,
slash-separated nonempty segments other than `.` and `..`, no backslashes or
control characters. Spaces and punctuation are permitted and URL-encoded by the
web host. Invalid declarations refuse at bind. The baker, module, Linux resolver,
web and Swift resolver use the same cases. Names are answered once per surface
while referenced by an asset mesh (or while declarations gate setup). Dropping the
last reference retires delivery states and answered names; respawning re-requests.
Declared simulation model data remains immutable and available to `world.model`.
A model may list at most 64 textures, all used by node-reachable meshes’ materials; a surface tracks at
most 256 asset names and refuses excess requests by name. Hosts drain at most
sixteen rounds. The web caps queued plus active flights at 256 and active fetches
at eight. Retirement cancels old flights before same-drain redelivery. Each has at most three
attempts with five-second attempt deadlines, 250/500 ms retry delays and a
20-second total deadline from queueing. Response streams abort above 64 MiB
before joining/copying the body; development assets have the same pre-copy limit. A 404 is missing; 5xx/offline failures retry and then
become named failures. A failed declaration draws its final status and stops asking
for frames; failed cosmetics never receive a first-frame stamp. Destruction/recreation
cancels flights. Apple’s resolver avoids a second `.tex` cache, but the generation
store retains the delivered `Data`; moving those bytes to private reloadable files
is owed (`review-S3ac-sol.md`, `Session.swift:214`, `PlanURL.swift:583,604`).
Its resolver caches reusable
fonts, images, models and shaders. `settled()` returns
remaining flight names when its sixteen rounds or deadline expire.


Owed after R1: host-level device recovery as described above; Apple delivered-`.tex`
bytes retained by the generation store (move to private files); primitive-module
size isolation (a primitive world still owns the asset maps and `Models`; 759 KB
against the ~490 KB target — the size question needs a link map, its own slice);
real-device Apple interruption and multi-display sweeps; WebAudio resume-failure
propagation. The file:line references name the six blind reviews of 4869f48c, cached under
`~/Library/Caches/exact2-game/briefs/`. Size analyses: `review-S3ac-sol.md` (`asset.rs:374`, `sim.rs:82`,
`renderer.rs:22,245`) and `review-S3ac-grok.md` §Primitive module vs 759 KB.

The runtime decodes only `bin` Data. No glTF or image decoder enters the module.
EXGAME v3 and EXSIM v5 remain unchanged for existing games. The baker refuses
textures above 2048×2048; both baking and loading enforce the 64 MiB carrier limit. It traverses
only the single default scene and refuses multi-scene/no-default inputs, sparse
accessors, morph targets, non-triangle primitives, missing UVs on textured meshes,
joints/weights mismatches and unsupported channels/extensions by name. UV0
transforms, authored nearest/linear filters and wrap modes survive baking. Colour
mips use linear-light RGB; only MASK/BLEND base colour weights RGB by alpha.
Opaque colour and emissive maps average straight RGB, and the mode is part of the
dedup key and generated name. MASK coverage is retained to the nearest texel count. Bake and runtime share model/texture validation,
including finite scalars and inverse binds, ordered bounds, clip node/arity/time
invariants and nonsingular node transforms. Skins and clips remain data until S3b.
The 16×16 crate pins both output files; Khronos test inputs are digest-pinned.

Model pipelines are lazy and share shader modules/layouts. A primitive-only world
links no model decoder or shader family. Loaded content is prepared before the
surface draws, and device loss repeats only that preparation. Entity-global
negative-determinant transforms on asset meshes refuse by name: the current feed
has immutable node winding batches, so silently accepting entity winding changes
would render inconsistently. Baked mirrored nodes remain supported. The peer
surface test asserts unchanged preparation/upload counts through normal play. An undeclared cosmetic
pop-in is the explicit exception: its arrival can prepare/upload during play.

Small is a feature. When something here feels clunky, slow or bloated, the move is
to delete it and try again, not to configure it.

## The world dev loop

Live frames use the host's display period, not the last frame delta. The web
exports its 16-sample median, refined by full rolling fits when the period differs
by more than 1%; sustained skipped slots reacquire even a harmonic rate change.
Apple quantizes `targetTimestamp - timestamp` to display rate classes with 1%
hysteresis and three consecutive candidate intervals, including initial acquisition.
One doubled interval cannot change the class; three can. Classes never exceed
the display maximum, and stable callbacks reuse cached boundaries without allocating. Both Apple hosts use the same quantizer;
every callback publishes its session's current class before rendering because the
module is process-wide. ProMotion's cadence can differ from nominal `duration`.
L is unknown (zero) headless; Apple publishes zero until three stable intervals
after module creation establish a class.
On web it is unknown for the first sixteen intervals (133 ms at 120 Hz, 267 ms at 60 Hz).
A short web interval uses a provisional half-period lattice; a second short interval
confirms it. A sustained double-slot cadence promotes the existing lattice. Jittered
60 ↔ 120 transitions are tested delta by delta; isolated sub-slot callbacks retain the fit.
With world time `T` and fixed step `step = 1000/hz`, `L` approaches
`min(period, step)`; ticks run strictly before the scheduling horizon `T + L`.
Seekable uses `L = 0`. A live gap
still contributes at most 250 ms; pause stops time and unpause seeds the clock
without consuming the pause gap.

Rendering uses `R = T + L - step`, with
`alpha = (T + L - tick * step) / step` between the last two completed poses.
A settled display period gives `ΔR = ΔT`: a 40 ms stall moves the pose by 40 ms,
and its very next frame moves it by one normal frame interval. The last frame's
delta never changes L. Tick zero draws the initial pose; the alpha guard handles
missing history at startup or restore. Seekable retains
its original `frac(T / step)` interpolation and exact integer-microsecond seeks.

The live clock adjusts its origin toward the nearest frame on the display
lattice, dilating or contracting elapsed time by at most 0.25% per frame until
the phase error reaches zero, then holds. Changes below 0.5% relative to the
accepted period are ignored. A real period change, including unknown → known,
slews L first, then reacquires the grid, sharing one 0.25% correction budget;
neither the render nor scheduling horizon steps. A known period at the initial
epoch seeds L directly, before there is a preceding pose. This is
one origin adjustment: at 60/60, 120/60 and 240/60 every tick deadline then sits
on a frame; at 144/60 the remaining phases cycle. During acquisition,
`|ΔR - frame_delta| <= 0.0025 * frame_delta`, including period transitions
(apart from integer rounding). Increasing L from zero to 16.667 ms takes 6.667 s;
zero to 8.333 ms takes 3.333 s. Grid-only acquisition needs at most four seconds
at 60 Hz or faster when L is already at target. After unknown → 60 Hz, the shared
budget serializes horizon and grid acquisition: about ten seconds before 60/60
sits at alpha 1. At aligned
60/60, the tick runs at the frame and alpha is 1: no interpolation lag.
`tick_phase` is mean alpha on ticking frames, about 1 at 60/60 and 0.5 at 120/60.

Live phase is an integer accumulator of microseconds multiplied by `hz`
(1,000,000 units per tick), plus a fractional residual bounded to half a unit.
It never accumulates world milliseconds in an `f64`. A backwards or duplicate host stamp
changes neither phase nor lookahead. Period, residual and slew are host-only:
absent from saves, state, snapshots and hashes, and never applied under Seekable.
If a live tick ran early, a save is taken at that tick's exact deadline, encoded
as `ceil(tick * 1_000_000 / hz)` microseconds. Live→Seekable makes the same clock
catch-up without another `Game::tick`, then clears L. Restore again requires
`tick == floor(world_us * hz / 1_000_000)`; a one-tick-ahead save is refused.

With aligned commensurate grids, an input is consumed by the first tick whose
deadline is at or after its arrival, and ticks run at their deadlines. This
includes input delivered after the preceding frame: there is no early delivery
cutoff window. During acquisition, at noncommensurate rates, or with an off-lattice
callback, a tick can still run early; input delivered after that execution waits
for the next eligible tick. Seekable preserves its strict `stamp < deadline`
rule, including exact-boundary inputs going to the following tick. Queued input
stays stamp-ordered. Live stamps queued before a callback are clamped to that
frame's `now_ms`: an event stamped 16.8 ms has arrived even if pacing names the
frame 16.667 ms. The initial/restore epoch sample skips this clamp so rebased
future stamps retain their offsets. Seekable future stamps still wait. Multi-tick catch-up spreads
input across ticks. Only a gap beyond the 250 ms cap collapses its input onto
the first remaining step. The deterministic record is the tick-stamped input
sequence with the same seed; live host stamps alone are not a seekable replay.

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
The generated game demonstrates nearby prompts, beacon plinths, and `restart_generation` as the
world's restart identity, with the same movement/light sequence in its test and proof.


Native spatial reads use `sim.layout("player").unwrap().screen` and
`sim.pick(rect.center())`; set `sim.viewport(width, height)` for CSS-pixel coordinates.
Both use the agent's geometry, current global poses and viewport. A missing entity,
camera, projected rectangle or hit returns `None`.
Changing the non-live `Options::restart_generation` reconstructs setup through the
same argument-binding path as any other setup change; there is no second reset path.
World performance state keeps small counts, totals, maxima and draw counters by
default. Request `state` with `perf: true` or `perf_reset: true` to arm the five
16,384-sample rings. Arming preserves aggregates and cadence; only explicit reset
clears them. `perf.armed` reports recording state; the feel probe arms it too.
Percentiles are zero before recording; counters still report total work.

## Skeletons

A declared `Mesh::asset("fox.model")` plays with
`Animation::play("Walk").speed(1.5).marker(0.3, "step")`; `.once()` clamps at the end.
Animation advances once per fixed tick, after the game's tick. Call
`animation::step(w)` earlier when the game needs this tick's `crossed("step")` or
`root_motion()`; the automatic second call does nothing. Marker flags are saved
state, cleared on the next step; `animation fox Walk step` is the matching journal
line. glTF animation `extras.markers` accepts `[[0.3, "step"]]`. Negative speed
plays backwards, with the same open-start/closed-end marker intervals. Loop jumps
emit each crossed marker name once, even when several loops fit in a tick.

`Animation`, `Blend`, and `Animator` are alternatives; insertion refuses a second
controller and logs why. `Blend::across([(0., "Survey"), (1., "Walk"), (3., "Run")])`
uses its saved `axis`, the two bracketing clips, and one saved normalized phase.
The phase advances by `dt / lerp(duration_a, duration_b, weight)`; differing clip
lengths keep corresponding phases together. This does not correct incompatible
foot contacts in source art. Sampling supports glTF step, linear and cubic tracks;
quaternions are normalized, with shortest-path slerp for linear rotations.
`time += (1.0f32 / hz as f32) * speed` uses separate f32 multiply and add, without
FMA. Scalar glam/libm provides the transcendentals. Sixty 60-Hz additions from zero
are pinned to bits `0x3f7ffffb`, and the full Fox pose has a cross-host pin.

`Animator::new([State::new("walk", Play::Clip("Walk".into())).to("run",
Condition::Arg("speed".into(), Cmp::Gt, 2.0.into())),
State::new("run", Play::Clip("Run".into())).fade(0.2)])` is all `Data`.
`w.get_mut::<Animator>("fox").unwrap().set("speed", speed)` writes a typed number;
bools also work. The first matching transition wins, at most one per tick. A fade
lerps/slerps from the outgoing local pose to the advancing destination pose.
Deliberate cuts: the outgoing pose is frozen during the fade, parameters are only
f32/bool, and there are no layered graphs, closures, scripts or additive animation.
The phase, current state, time in state, parameters, outgoing pose and fade progress
are saved and hashed. `Restore::Open` restores exactly; `Restore::Carry` overlays
fresh named controllers' definitions while preserving their pose and phase. Edited
Animator definitions fade from the carried pose. A removed current state retains
the old definition until the author supplies a matching state.

`Ik { chain: [root, mid, tip], target, pole, weight }` solves a direct two-bone chain
in model coordinates after sampling. Weight zero leaves every pose bit untouched;
an unreachable target stretches the chain. Zero-length chains and targets at the
root refuse by name. Bones and imported nodes are compact arrays, never entities.
Declare `Socket("b_Head_05".into())` on the fox and put `SocketFollow::new("fox")`
on an attachment. The declared socket's ancestor chain is composed sim-side every
tick, including offscreen and headless; other composed joints are diagnostic reads
or GPU work. This slice supports one declared socket per model owner.

Animation owns pose. `Transform` has one writer per tick: the fixture's circle
script, or the game's Character/physics step. `Animation::root_motion()` exposes
model-local translation of the first skin joint, including a loop's displacement,
and never writes Transform. The game can feed that contribution into movement or
ignore it. A socket follower owns its attachment's Transform.

The `Pose` component saves previous/current local TRS as bulk f32 arrays, normalized
phase, event flags and conservative bounds. The bounds inflate the bind AABB by
maximum chain reach plus influenced inverse-bind vertex radius, including authored
translation/scale track extrema (cubic tangent overshoot is conservatively bounded).
Layout and CPU pick use those same bounds. A model without a controller draws its
bind pose. Skinned models are bounded to 256 imported nodes and 256 joints per skin;
four influences are normalized at bake and additional influence sets refuse.
`state world:fox` includes controller state; the `state` wire form with `pose: true`
returns up to 256 named joint world matrices at the last tick. Snapshots include the
saved local arrays, bounds and transition state.

`games/skinned-fixture` is the Fox proof: a circle, Survey → Walk/Run blend, a head
socket, tick-60 joint JSON, save at 45, and fresh-process continuation to 120.
Its native paranoid test saves and loads into a new Sim on every tick for 120 ticks;
IK checks use the Fox's left leg. Run `bun game/games/skinned-fixture/proof.mjs web`
(or `macos` / `linux`) from the root. The source glb is the cached Khronos Fox sample;
the game generator created the fixture with `--assets`.

S3b proof (2026-09-18): web, macOS and the headless Linux host running on this
arm64 Mac match all 24 joint matrices in
[`tick60.json`](games/skinned-fixture/logic/tests/tick60.json). Tick 60 hashes to
`0xa9033d749a82ebd4`; tick 120 hashes to `0xb05ce95a6c799acf`. Saving at 45 and
restoring in a fresh host reaches the same tick-120 snapshot. All three final saves
are byte-identical (13,849 bytes; SHA-256
`cae346719d1a1e3fc6b8eb3d22eb9ddfb5295811dd09f50297bd09451c69c526`). The native
120-tick paranoid round-trip, edited-blend Carry/Open, Fox-leg IK and socket tests
pass. This is not a new x86-64 Linux measurement.

The macOS proof launches with SDK 26 and the temporary Xcode Swift wrapper adding
`--build-system native`. Its [screenshot](games/skinned-fixture/artifacts/fox-mid-stride-macos.png)
shows the stride and shadow but a white, untextured Fox; the
[web screenshot](games/skinned-fixture/artifacts/fox-mid-stride-web.png) is textured.
That native asset/host gap remains outside the skeleton slice. The pose request
currently uses `session.op({op: 'state', ...await session.target('world:fox'),
pose: true})`; the CLI's literal `state world:fox pose` still needs the two-line
driver forwarding change outside this slice. Performance and size qualifications
are in [the renderer README](render/README.md#skinned-model-path).
