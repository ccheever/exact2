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

#[derive(Default, exact_game::Data)]
struct Hud {
    lit: u32,
    near: String,
}
#[derive(Default, Args)]
pub struct Options {
    pub seed: u64,
    #[live]
    pub paused: bool,
    #[restart]
    pub restart: bool,
}
#[derive(Default, Component)]
pub struct Beacon {
    pub lit: bool,
    glow: Spring,
}
pub struct SmallGame;
impl Game for SmallGame {
    const ID: &'static str = "small-game";
    const HZ: u32 = 120;
    type Args = Options;
    fn actions() -> Actions {
        Actions::new()
            .stick("move", Stick::wasd().or_arrows())
            .button("light", &["KeyE"])
            .button("jump", &["Space"])
    }
    fn setup(w: &mut World, args: &Options) {
        w.reseed(args.seed);
        w.insert_resource(Environment {
            background: Some([0.49, 0.67, 0.64]),
            ..Environment::default()
        });
        w.spawn((
            Transform::default(),
            Mesh::plane(40.0, 40.0),
            Material::grid([0.16, 0.23, 0.24], 1.0),
        ));
        let player = w.spawn_named(
            "player",
            (
                Transform::at(0.0, 0.9, 0.0),
                Mesh::capsule(0.4, 1.8),
                Material::rgb(0.8, 0.4, 0.1),
                // These are Character's defaults, kept visible for auditing the game.
                Character::new().speed(4.0).accel(12.0).brake(20.0)
                    .jump(1.2).gravity(9.81).ground(0.9).bounds_xz(-19.6..=19.6),
            ),
        );
        w.spawn_named(
            "camera",
            (
                Transform::default(),
                Camera::default(),
                Follow::new(player).offset(0.0, 9.0, 13.0).lag(0.15),
            ),
        );
        for (i, x) in [2.0, 6.0].into_iter().enumerate() {
            w.spawn_named(
                format!("plinth-{}", i + 1),
                (
                    Transform::at(x, 0.1, 0.0),
                    Mesh::cylinder(0.9, 0.2),
                    Material::rgb(0.3, 0.4, 0.42),
                ),
            );
            w.spawn_named(
                format!("beacon-{}", i + 1),
                (
                    Transform::at(x, 0.7, 0.0),
                    Mesh::sphere(0.5),
                    Material::default(),
                    Beacon::default(),
                ),
            );
        }
        w.publish_record(&Hud::default());
    }
    fn paused(args: &Options) -> bool {
        args.paused
    }
    fn tick(w: &mut World, input: &Input, _: &Options) {
        w.character("player").step(input.stick_xz("move"), input.pressed("jump"));
        let nearest = w.nearest_xz::<Beacon>("player", 1.5)
            .filter(|&e| !w.get::<Beacon>(e).unwrap().lit);
        if input.pressed("light") {
            if let Some(e) = nearest {
                let mut beacon = w.get_mut::<Beacon>(e).unwrap();
                beacon.lit = true;
                beacon.glow.set_target(w.now(), 1.0);
            }
        }
        let near = nearest.filter(|&e| !w.get::<Beacon>(e).unwrap().lit)
            .and_then(|e| w.name(e)).unwrap_or("").to_owned();
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
are saved and hashed as a component on the player. `step` reports `grounded`,
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
  `#[restart] pub restart: bool` marks a restart edge: either transition uses the
  same setup reconstruction path. In Contract, `state again = false`,
  `action restart writes again` / `again = not again`, then `world(7, paused, again)`.
  The explicit boolean replaces a generation counter; `state.world.restarted` counts
  reconstruction in this session, outside the save/hash. No ninth operation.
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
bun game/new.mjs my-game                 # author code, Contract, proof and tests
bun game/dev.mjs my-game                 # the shared dev loop, on loopback
bun game/games/my-game/proof.mjs linux    # HUD and gameplay, GPU-less
bun game/games/my-game/proof.mjs web --screenshot-only
bun game/prove.mjs beacons --hosts web,linux --repeat 2 --compare-saves
```

Games are members of this workspace: one lockfile, profiles and dependency pins in
`game/`, no workspace manifest or lockfile in a game. The generator registers the
new packages in that shared lockfile without upgrading dependencies. An author owns exactly this:

```text
game/games/my-game/
  logic/          src/lib.rs (the game), tests/sim.rs
  app.contract
  proof.mjs
```

Default `logic/Cargo.toml` and `app.json` are generated, ignored bake inputs.
The directory supplies the package/title; the template's literal `impl Game for T`
and `Game::ID` supply the type and app identity (`com.exact.<ID>`). An existing
manifest wins; delete a generated one to re-derive it after renaming the game.
Nonliteral/cfg/macro game declarations supply an explicit `app.json` override;
Rust still checks the exported type. `git add -f app.json` or `logic/Cargo.toml`
records an intentional override. Greybox retains `app.json` for `game.audio: true`.

New action games explicitly set `Game::HZ = 120`: the first Feel sitting measured
3.5–4.1 ms event-to-submitted-pose latency there. Seekable proofs remain tick-exact.
Beacons r4 and Greybox retain 60 Hz, their movement constants and trajectories.
The template uses grid ground, an Environment background, explicit accessible
button names, initial Play autofocus and a centred victory overlay with autofocus.
Contract has no default hover/focus-visible button styles; that gap is in QUEUE.md.
The hosts currently process autofocus once per document, so dynamically inserted
victory autofocus is declared but does not yet move keyboard focus automatically.

`prove.mjs` builds hosts serially, then runs their existing proofs in parallel with
isolated output directories. Its table compares every final world tick/hash and,
with `--compare-saves`, every named save's complete bytes across hosts and repeats.
The template runs headless gameplay first; its web screenshot-only step captures
the same Contract/world. Linux `tree` reads the runner's actual text, accessible
names and focus; no game-specific HUD mirror is involved.

`app.json` may override `"game": { "crate": "my-game-logic", "type": "SmallGame" }`.
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
decoder or model shader markers in the measured wasm. The link map attributes
most remaining size to generic storage, allocation and formatting; asset maps and
`Models` are small contributors (see [module size](#module-size)). A primitive module refuses asset meshes by
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
size reduction (D3's link map and per-cut measurements are [below](#module-size);
the current 764,322-byte Beacons module still exceeds the 550 KB target);
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
invariants and nonsingular node transforms. Skins and clips are baked data consumed by the playback controllers below.
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
The generated game demonstrates nearby prompts, beacon plinths, and a boolean restart
edge, with the same movement/light sequence in its test and proof.


Native spatial reads use `sim.layout("player").unwrap().screen` and
`sim.pick(rect.center())`; set `sim.viewport(width, height)` for CSS-pixel coordinates.
Both use the agent's geometry, current global poses and viewport. A missing entity,
camera, projected rectangle or hit returns `None`.
`World::position` and `Sim::position` read current global positions.
`w.nearest_xz::<Beacon>("player", 1.5)` returns the closest entity, breaking ties
in entity order; `.filter(|&e| !w.get::<Beacon>(e).unwrap().lit)` excludes it when lit.
Use `near_xz` to iterate all candidates (including when selecting the nearest unlit
among overlapping ranges). `w.character("player").step(wish, jump)` borrows its
Character and Transform only for the step and uses the fixed world dt.
World performance state keeps small counts, totals, maxima and draw counters by
default. Request `state` with `perf: true` or `perf_reset: true` to arm the five
16,384-sample rings. Arming preserves aggregates and cadence; only explicit reset
clears them. `perf.armed` reports recording state; the feel probe arms it too.
Percentiles are zero before recording; counters still report total work.

## Skeletons

A declared `Mesh::asset("fox.model")` plays with
`Animation::play("Walk").speed(1.5).marker(0.3, "step")`; `.once()` clamps at the end.
A fresh reverse standalone one-shot starts at the clip end (an explicitly nonzero
`time` is retained).
`Animation`, `Blend`, and `Animator` are alternatives; insertion refuses a second
controller. Each dereferences to the same saved `animation::Playback` interface:
`crossed("step")` and `root_motion()`. Animation advances once per fixed tick,
after the game's tick. Call `animation::step(w)` earlier to consume this tick's
markers or motion; the automatic second call does nothing to the sampled pose.
It still refreshes socket followers after the game has applied the owner's motion.

Declare the motion root explicitly with `.motion_root("b_Root_00")` on any controller.
There is no inferred first joint and no motion extraction without a declaration.
Legal duplicate node names resolve to the first match in parent-first traversal
(`animation::node_order`), for motion roots, sockets and IK alike. Give gameplay
nodes unique names when that choice would be ambiguous to the author.
The selected node's local translation is held at its bind anchor; extracted deltas
include complete forward or backward loops and are returned in model coordinates
(through the sampled parent's basis when the node has a parent). Applying
`transform.rotation * (transform.scale * playback.root_motion())` to the entity's
position therefore does not double its travel or snap at a loop. Rotation remains
in the local pose. The game owns Transform, including collisions or rejection of
motion; animation never moves it. Locomotion roots should have stationary ancestors.

`Blend::across([(0., "Survey"), (1., "Walk"), (3., "Run")]).parameter("speed")`
binds its axis to the containing Animator's number parameter. `animator.set("speed",
speed)` drives that axis and transition conditions. Standalone blends use their saved
`axis`; an unset parameter uses that authored axis, and a nonnumeric value refuses.
Two bracketing clips share normalized phase, advanced by
`dt / lerp(duration_a, duration_b, weight)`. Their root deltas use those same weights.
An exact knot samples and emits markers only from its active clip. Source clips still
need compatible foot contacts; phase matching cannot repair the art.

`Animator::new([State::new("walk", Play::Clip("Walk".into())).to("run",
Condition::Arg("speed".into(), Cmp::Gt, 2.0.into())),
State::new("run", Play::Clip("Run".into())).fade(0.2)])` is all `Data`.
Use `state_named("run")`, `state_mut("run")`, or `blend_mut("travel")` for named
access. A state supports `.once()`, `.speed(2.)`, and `.paused(true)`; those saved
fields can also be changed through `state_mut`. A paused state neither advances
its clock/fade nor transitions. A one-shot finishes before its first matching edge
can run, on the following tick. A zero-length clip or zero-speed once state
finishes on its first unpaused sample; explicit pause still prevents completion
and transitions. Fades also finish before another edge runs;
self-edges are ignored. At most one edge runs per tick. Looping locomotion retains
phase across transitions; entering or leaving a one-shot resets it to the playback
start (the end for negative speed).

Fades mix the frozen outgoing local pose with the advancing incoming pose. Root
motion fades from the outgoing tick's saved displacement to the incoming delta.
Frozen outgoing clips emit no events; incoming markers become eligible only when
the incoming weight is **greater than 0.5**, with suppressed crossings discarded.
Outside fades every nonzero-weight clip contributes markers. Flags clear on the next
step (failed playback contributes zero motion/events), are saved/hashed, and collapse repeated names to one event per tick.
glTF animation `extras.markers` accepts `[[0.3, "step"]]`. Negative speed uses the
same open-start/closed-end intervals. Journal lines are `animation fox travel step`;
the fixture also logs `fox footstep` when its own `crossed("step")` read is true.

Sampling supports glTF step, linear and cubic tracks. Cubic uses the left key's
out-tangent and right key's in-tangent, multiplied by the interval duration, exactly
as [Khronos Appendix C.5](https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html#interpolation-cubic)
specifies. Cubic quaternion signs and tangents are preserved and the polynomial
result normalized; shortest-path slerp applies to linear rotations. Exporters must
avoid splines that produce a zero quaternion. Asymmetric three-key and signed
quaternion tests distinguish wrong neighbours, missing duration factors, and sign
rewrites. Scalar glam/libm supplies transcendentals; fixed f32 clock additions
remain pinned (`0x3f7ffffb` after sixty 60-Hz steps).

`Ik { chain: [root, mid, tip], target, pole, weight }` also runs without a controller,
starting from bind pose. It solves a direct two-bone chain in model coordinates;
zero weight is exact and unreachable targets stretch the chain. Invalid chains or
targets refuse by name. Sampling/IK failure preserves the last good pose pair and
leaves the Animator state, clock and fade uncommitted. The first successful pose
pair is current/current. Restore, carry, teleport and model/batch arrival duplicate
current at the presentation seam without rewriting saved history. Bones remain
compact arrays, never entities.

Declare one `Socket("b_Head_05".into())` per owner; `SocketFollow::new("fox")` drives
an attachment. Socket names resolve once per loaded rig. Replacing the loaded model
rebuilds the cached bind pose, bounds and socket resolutions at the next sample.
Removing or failing a socket invalidates `SocketPose`; locomotion continues and
followers restore their captured authored Transform. Sampling, socket and follower
errors have separate once-only lifetimes, reset by recovery/removal or a changed error.
The declared socket's ancestor chain runs sim-side, including offscreen/headless.
The follower is its attachment Transform's writer.

The `Pose` component saves previous/current local TRS, phase, flags and bounds.
**Layout and CPU pick use a static inflated bind AABB, not the drawn skin.** The box
adds maximum chain reach and inverse-bind vertex radius, including track extrema
and conservative cubic overshoot. Picking can hit empty space inside it; neither
sampled vertices nor frame-alpha skinning tighten it. Models cap imported nodes and
skin joints at 256, with four normalized influences per vertex.
`state world:fox pose` forwards `pose:true` and returns all unique skin joints in
imported-node order. A saved pose/model length mismatch refuses with the model name.
There is no silent truncation or duplicate budget consumption.

Controller definitions, phase, parameters, outgoing pose/motion and fade progress
are saved/hashed. `Restore::Open` restores exactly; `Restore::Carry` overlays fresh
named definitions, preserving the situation and fading from the carried pose.
A removed current state retains its old definition until a matching state is supplied.
Deliberate cuts: f32/bool parameters, frozen outgoing pose, one socket per owner,
no layered/additive graphs, closures or scripts.

`games/skinned-fixture` teaches parameter-driven Survey → Walk/Run, consumes root
motion through Transform, and reads/logs footsteps. Its CC0 Khronos Fox source was
in place; the fixture authors linear `b_Root_00` translation tracks at 40 model
units/s for Walk and 120 for Run along the Fox’s +Z facing, plus
quarter/three-quarter-phase step markers.
The controller explicitly names that root. Heading is authored; travel comes from
the clips. Tick-60 joint JSON is pinned in
[`tick60.json`](games/skinned-fixture/logic/tests/tick60.json), and the tick-120 hash
is `0x409341e24939d7c2` (tick 60: `0xb863e854ca85b74e`). These replace S3b's circle
pins because the motion tracks, Transform path, playback data, socket home and
history semantics changed.

Run `bun game/games/skinned-fixture/proof.mjs web` (or `linux`) from the root.
The S3b-b web/headless Linux proof run on this arm64 Mac passed in
22.276/33.022 s. R2's web proof passes with both tick hashes asserted; the R2
Linux rerun and wider host sweep remain owed after shared-tree build interruptions
and stalled process inventory/optimizer children (see the ergonomics diary).
The proof saves at tick 45 during the fade and resumes in a fresh host through 120;
its final save must be byte-identical. Both hosts produce the same 15,084 bytes,
SHA-256 `151188009e1141bc52f63a3b913cec4362d788eb5695a80ae3d71ed657f79b3f`. The native paranoid test repeats restore into
a new Sim every tick and compares bytes as well as hashes and local poses. A GPU
pixel test compares birth, restore, carry and model arrival against a current/current
oracle within 0.1% of the Fox rectangle. Deliberately injecting bind history supplies
the negative control. Both advertised tick hashes are asserted by the native test.
The pre-review macOS screenshot showed a white Fox while web was textured; that
native asset delivery gap remains outside this slice. No new x86-64 run or macOS
proof is claimed here. Rendering measurements and qualifications are in
[the renderer README](render/README.md#skinned-model-path).


## Module size

`bun game/bench/size.mjs` builds Beacons and prints shipped raw/gzip bytes plus
pre-opt crate and module attribution; the per-cut table and 200k-cube paired
timings are in [the benchmark README](bench/README.md#d3--measured-module-size-2026-09-18).
D3 shares component page allocation and Data walks over aligned bytes, retains
typed query strides/leases and page generations, and links only the registered
storage kinds. Asset delivery maps are sorted vectors; saved ordered maps keep
their existing order. Pipeline shader comments/indentation are removed at build
time with newlines preserved, so line numbers survive and columns change.

JSON, perf/trace and audio diagnostics share a small-table Ryu writer. Exact
decimal tie handling preserves existing JSON number spelling; fixed audio
journal decimals also match. Publication journal values now use Data's field
walk instead of Rust Debug (for example `{"Number":[0.0]}`); the journal stays
outside the world hash. Dynamic float clamp panics still retain Rust float
formatting, including through the concurrently maintained animation code.
The <550 KB Beacons target remains unmet; this cut does not change the existing
primitive/model boundary or add a core-crate feature.

### Physics snapshot reconstruction (PX1)

Physics snapshots are EXPHYS v2. Vendored Rapier 0.35.3 now persists its deferred
BVH optimization flag; v1 omitted simulation state and is refused atomically by
name, including when nested in an EXSIM save. Start a new world. The dependency
belongs only to the game workspace.

`Sim::paranoid(Paranoid::Off | Paranoid::Save | Paranoid::FreshGame)` reconstructs
through the production restore path after every completed tick, asserting the
world hash is unchanged. FreshGame also drops the old world and decodes immutable
models again. Driver time, queued input and pending output remain with the driver;
restore still requires exact tick/time agreement. Off is the default. Native
`EXACT_GAME_PARANOID=1` selects Save and `fresh-game` selects FreshGame; wasm uses
the same environment variable at build time.

`bun game/games/<name>/proof.mjs <web|linux> --paranoid` runs the actual proof in
all three modes and compares each session's final hash, tick, publications and
journal. Web receipts include the compiled mode, so a normal proof rebuilds after
a paranoid build. Timings report both each full proof and the total including its
build. This diagnostic intentionally serializes every tick; it is not a performance
mode. `game/physics/tests/compare.rs` provides the corresponding native comparison.

PX1 native verification (2026-09-18, this lane, optimized dev tests): all game
workspace tests, clippy and formatting pass. The Linux headless proof passes for
greybox, Beacons, asset-fixture and skinned-fixture in all three modes, comparing
hash/tick/publications/journal. Physics pile tick 600 changes
`5ba7691abdc98058 → 129ba6d92f9ac217`; two-body `simulate(120)` changes
`9960c10fadbb9c4b → 5608994347e54d28`. Both agree across continuous, Save and
FreshGame. Greybox setup/forward pins (`7df5e5a89b4d0207` / `0f14b8b231091d12`),
asset tick 60 (`8f6d518f39634478`), skinned ticks 60/120
(`b863e854ca85b74e` / `409341e24939d7c2`) are unchanged. Beacons' proof endpoint
is `331c074e0f135059` at tick 907 in all modes.

| Native script | Off ms | Save ms | FreshGame ms |
|---|---:|---:|---:|
| Physics stack, 120 ticks | 1.244 | 41.256 | 44.109 |
| Beacons, movement/jump | 0.311 | 21.381 | 22.329 |
| Greybox, movement/action | 0.342 | 26.830 | 25.092 |

Skinned-fixture's 120 pairs of continuous/reconstructed ticks took 33.576 ms
(Save) and 52.041 ms (FreshGame), including both runs in each pair. These are
single-run diagnostic timings on a shared Mac. Native host proof wall times
include process inventory and, where invalidated, builds; they are not tick costs.
The live-clock regression runs 144 Hz frames against 60 Hz physics in all modes,
checks capture metadata is unchanged by save, and verifies byte-identical reopened
continuation. Lane/game has no sibling capture/checkpoint replay layer.

The real browser live-capture check also passes: a live `screenshot … world save`
reopens with its returned tick/hash and continues identically in two fresh
processes for another 1,000 ms; the seekable control passes as well. The temporary
probe exposed the existing agent entry point on a live-clock page without making
its GPU clock seekable. No lane/game save-path change was needed.

Greybox GPU size, before this task's available dist → PX1 build: **2,106,859 →
2,175,006 raw bytes (+68,147); 465,027 → 475,963 gzip-9 (+10,936)**. Both retain
wasm name/producers sections and omit wasm-opt post-processing. The new measurement
is `bun game/bench/size.mjs px1-unoptimized --app greybox` with wasm-opt absent
from PATH (the supported fallback; the installed executable stalled before running).
This is the shared lane's before/after artifact delta, including concurrent R2
changes, not an isolated Rapier cost. Greybox and Beacons do not link physics;
the root dependency graph also contains no Rapier package.

The agent's `world.run(ms)` now establishes the current clock with `+0` after
asset settlement before its positive seek, matching `Sim::run`'s initial epoch.
Without that initialization the skinned web fixture's first 750 ms seek could
leave the newly ready world at tick zero. The original tick/pose/hash assertions
now pass in all three modes. The Chrome wasm physics card also checks both pins
across all three reconstruction modes.

Verification limits on this Mac: the process-wide `ps` inventory stalled, so the
proof runs used a temporary libproc inventory retaining parent PID and process
start identity. Web functional proofs used installed Chrome for Testing and the
build's supported no-wasm-opt fallback. The root workspace build was attempted
normally, with a private TMPDIR, and serially; native JS-bake child processes
stalled each time, so that check is **not green**. Root Cargo metadata nevertheless
confirms the vendored dependency does not enter its graph. No root build code or
system configuration was changed to bypass the failure.

Final paranoid proof wall seconds (Off / Save / FreshGame; builds and process
inventory included, so these are not ratios of simulation cost):

| Game | Web | Linux headless host |
|---|---|---|
| greybox | 19.090 / 19.307 / 20.840 | 46.921 / 1.811 / 2.117 |
| beacons | 30.109 / 21.231 / 21.471 | 27.442 / 35.212 / 1.683 |
| asset-fixture | 19.608 / 17.078 / 27.088 | 39.639 / 1.338 / 0.946 |
| skinned-fixture | 24.288 / 22.019 / 20.447 | 24.379 / 1.078 / 0.804 |

All four ordinary proofs also pass on web and Linux, leaving web artifacts in Off
mode. A cold greybox browser launch timed out in `Page.navigate`, and a cold
asset-fixture Linux launch timed out before readiness; both passed unchanged on
cached retry. Every completed proof reports all recorded children exited.
