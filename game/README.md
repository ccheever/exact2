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

Gameplay code uses **kinds**: named joins over component columns. Engine-level
component access is shown second below. A kind adds no tag or storage; saved
binding IDs live in the component or resource that owns them. The derive accepts
1–8 distinct component types, with at least one required field. `Option<Component>`
is optional; spell it directly, since aliases of Option are unsupported. `#[read]`
keeps a field shared in an editing row. Identical type spellings fail to compile;
aliases of the same component fail a TypeId preflight before spawning or leasing.

All Rust examples run as doc-tests with `cargo test --doc -p exact-game`.

```rust
use exact_game::character::Character;
use exact_game::*;
#[derive(Default, Component)]
struct Player { character: Character }
#[derive(Kind)]
struct Hero { player: Player, transform: Transform }
let mut world = World::new(60, 7);
let hero = world.spawn_kind("player", Hero {
    player: Player::default(), transform: Transform::at(0.0, 0.9, 0.0),
});
world.edit(hero, |h| h.transform.position.x = 2.0);
let a = world.row(hero)?;
let b = world.row(hero)?;
assert_eq!(a.transform.position, b.transform.position);
# Ok::<(), KindError>(())
```

`spawn_kind<K>(&mut self, name, value: K) -> Id<K>` creates components.
`bind<K>(&self, name_or_entity) -> Result<Id<K>, KindError>` is the setup boundary
for scene entities and saved child bindings. `row(id) -> Result<K::Ref<'_>, KindError>`
is shared access; `edit(id, f) -> R` releases editing leases when its closure returns.
The redundant `with_row` entry point is removed. Generated views have the kind's
visibility and live in an anonymous scope, so user types named `HeroRef`/`HeroMut`
do not collide; use inferred closures or `<Hero as Kind>::Ref<'_>` in signatures.

`rows::<K>()` / `rows_mut::<K>()` visit entity order, with one lease per column.
`.one()` permits zero rows; `the::<K>()` requires exactly one. Match the join to
what the operation needs: Beacons scans Transform + Beacon, edits selected IDs,
then updates Material with a separate read-only Beacon + mutable Material join.
A no-op mutable lease still dirties pages. Lease conflicts name the operation,
kind, entity and component; overlapping nested edits also name the outer edit.
Drop rows before another operation on an overlapping column.

Declare a child on its owning component field. Here `#[child("bulb", bulb)]`
means “the saved `lantern.bulb` field binds the direct child named bulb.” The field
must be an ordinary saved `Id<Bulb>`, never `#[data(skip)]`.

```rust
use exact_game::*;
#[derive(Kind)] struct Bulb { light: PointLight }
#[derive(Default, Component)] struct Lantern { bulb: Id<Bulb> }
#[derive(Kind)] struct Lamp {
    #[child("bulb", bulb)]
    lantern: Lantern,
}
let mut world = World::new(60, 0);
// Spawn children before their parent; spawn_kind installs the Parent edge.
let bulb = world.spawn_kind("lantern-7/bulb", Bulb { light: PointLight::default() });
let lamp = world.spawn_kind("lantern-7", Lamp { lantern: Lantern::default() });
assert_eq!(world.row(lamp)?.lantern.bulb, bulb);
world.edit(bulb, |b| b.light.intensity = 5.0);
# Ok::<(), KindError>(())
```

For an instantiated scene, `bind::<Lamp>("lantern-7")` fills a default child ID
and validates its Parent edge. An existing saved ID is never retargeted by name.
Every row access validates generation, required child components and Parent;
a stale ID fails identically live and restored, naming the owning field. Tick
paths never resolve or rebuild child bindings. Actor IDs are likewise saved in
Session and resolved during setup. `Id<K>: Data` has exactly Entity's encoding.
`edit_resource::<Session, _>(f)` releases the resource lease at return.

Work bounds: at most eight components and eight direct child checks per row;
child checks do not recursively traverse bindings. A single bind uses indexed
name lookup. Iteration and singleton search scan at most 200,000 entity slots,
including dead slots, and explicitly refuse larger worlds before acquiring
leases. Child-bearing iterators validate the matching rows before mutable leases;
ordinary kinds need no extra entity scan. TypeId uniqueness takes at most 28
comparisons. The 200k interleaved/churn regression checks every returned row and
the over-limit error; it cannot pass with an empty iterator.

Engine-level component access (`get`, `get_mut`, `query`) is for engine modules:

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
  `SAVE_VERSION`, world time and dynamic input. `Sim::restore(&[u8])` and
  `restore_bound(&[u8])` return `Result<(), DataError>`: under agent ownership,
  they anchor to the destination's established host clock, so the next advance
  executes its full duration. Hosts establish ownership and time together with
  `clock {owner: "agent", now: milliseconds}` before restoring. Under live
  ownership (or without an established clock), the next host sample establishes
  a new epoch without simulating time spent paused. Inspection never anchors time.
- **Time is an input.** Under the seekable clock, `tick = floor(clock_ms × hz / 1000)`; a step is `1/hz`
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

Observation caches each storage page's per-instance digests by its write
version, presence mask and observed Ambient membership. Unchanged pages reuse
those digests; a mutable lease dirties its page even if nothing is written, so the
next sample rehashes values and still reports Still when they are equal. Resources
and RNG cache their digests until a mutable lease or replacement. Existence pages
track slot membership/incarnations, and hierarchy propagation marks global-pose
pages when a descendant's pose or availability changes, including motion inherited
from Ambient ancestors. Observation reads the last propagated pose.

Caches are derived state: neither saved nor hashed, discarded on load and world
identity/presentation-generation changes. The sorted entries vector and comparison
are unchanged, including the eight reasons and Ambient/AMBIENT rules. Building
that flat vector still costs O(entries); value hashing costs O(dirty-page values).
`World::hash` and its canonical streaming definition are unchanged. A fused
observation/hash sample still streams every component into the canonical hash,
while reusing the separate observation digests.

Diagnostics (release profile, from the repository root):

```sh
cargo test --manifest-path game/Cargo.toml -p exact-game --release --lib stillness_hash_cost -- --ignored --nocapture
cargo test --manifest-path game/Cargo.toml -p exact-game --release --lib settle_200k_cost -- --ignored --nocapture
```

The observation benchmark includes 1k/10k/200k still worlds and 200k entities with
100 writes clustered in one page or spread over 100 pages. It separates observation,
canonical hash, fused observation/hash and the seek pair. The extended cases use
100 samples after five warmups; single-operation timings exclude writes. Settle
measures an initial Unknown world, then 100 samples after five warmups, invalidating
each sample with an unwritten Transform lease so it must actually observe rest.
At 1,024 slots per page, the spread case rehashes 102,400 values; the clustered
case rehashes 1,024. Neither timing is a gate.

T1a observation-only measurements on shared x86-64 Linux (EPYC 9454), with 200k
Transform entities; milliseconds are median / p95:

| Operation | Before | After |
| --- | ---: | ---: |
| Observe, still | 37.829 / 38.254 | 0.758 / 0.786 |
| Observe, 100 clustered movers | 37.803 / 38.517 | 0.960 / 0.976 |
| Observe, 100 spread movers | 37.864 / 38.435 | 21.210 / 21.383 |
| Settle after an unwritten lease | 139.267 / 140.468 | 66.954 / 67.779 |

The single initial settle sample was 158.295 → 126.409 ms, excluding world
construction. Settle retains the full canonical hash cost. Both measurements
used the same diagnostic and reduced-debug/non-incremental environment.

The old uncached observation path remains a test-only oracle: three seeded scripts
cover 576 ticks and compare 2,304 samples before/after propagation, with and without
the fused hash. They cover spawn/despawn/reuse, unwritten leases, parent edits,
resources, RNG, Ambient, load and identity changes; entries, verdicts, reasons,
saves and canonical hashes agree. Separate tests count value writes on dirty pages
and cover equal-pose globals becoming available after slot reuse or reparenting.

Validation: 381 Rust tests passed, 10 diagnostics ignored; engine Clippy, caps and
boot pass. Bun matches the baseline (38 passed, 2 failed). Workspace Clippy remains
red on the existing `render/src/assets.rs:239` type-complexity warning; workspace
formatting remains red in that file and `engine/tests/capture.rs`, both untouched.
The Linux proofs retain identical failure lines and printed hashes: Beacons has
three baseline failures, Lanterns the missing `dist/index.html`, Greybox three
baseline failures, and Asset Fixture passes. GPU, Chrome and Apple execution
remain unverified on this host.

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
and declared model boxes as picking. Undeclared models without authored
`ModelBounds` have null bounds and unavailable screen/visibility reads; they
never block a pick or occlusion ray, even after their presentation bytes arrive.
Authored bounds remain usable before and after cosmetic delivery.
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
reports `restored: true` until the next tick or setup-argument rebuild. Saved construction arguments are retained; current compatible live bindings win; `state.world.restoredFrom` shows the saved arguments while `restored` is true. Register types first spawned mid-game with `world.register::<Projectile>()` in `setup` so a fresh world can restore them. The iOS path is implemented but has not been driven in this session.

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

The ignored Lanterns timing test runs five fresh simulations for 10,000 ticks
each and prints the median nanoseconds per tick. It includes clock advancement,
physics, follow, audio and observation, with W held and sound enabled:

```sh
cargo test --manifest-path game/Cargo.toml -p lanterns-logic --test timing tick_10k_median -- --ignored --nocapture
```

T3b2 saves gameplay IDs explicitly and removes the lazy Lanterns binding module.
Both games have committed before-change world fixtures for ticks 0, 60 and 180;
normal tests compare complete bytes. Lanterns' comparison adds only the approved
saved IDs and updated scene identity to the old fixture before comparing. No
fixture directory environment variable is needed. Both games also restore and
advance after every tick; immediate comparisons cover world bytes, and following
ticks compare complete Sim saves (restore clears consumed input edges).

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
colour and adds emission. Only declared models supply simulation data and baked
layout/pick bounds. An undeclared model is presentation-only: `world.model(name)`
always returns `None`. Give it authored bounds with
`Mesh::asset("prop.model").bounds([-1., -1., -1., 1., 1., 1.])` inside a spawn
bundle, or leave it unpickable. Arrival cannot change a game's reads or bounds.

Declare `Game::ASSETS = &["crate.model"]` for anything setup or simulation needs.
Declaring a model also declares every texture it references. Setup, tick zero,
clocks and saves wait for those dependencies. On a device, `Loaded` means geometry
and textures uploaded and every needed forward/shadow pipeline variant prepared,
including mirrored nodes; headless hosts validate the same bytes without GPU work.
A loading carry returns an already pending restore, or refuses a new save.
`Sim::save()` requires completed declarations. Every web agent operation waits at
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
owns generated outputs: renamed/deleted sources prune only those files, including
when `art/` disappears; authored collisions refuse. Ignore this manifest and the
generated `.model`/`.tex` files. Stems must be unique across art subdirectories.
The standalone baker is `cargo run -p exact-game-bake -- art/fox.glb assets/fox.model`
(from `game/`); its texture files accompany the model under the output directory.

Names use the shared `gpu::asset_name` rule: nonempty ASCII, at most 128 bytes,
slash-separated nonempty segments other than `.` and `..`, no backslashes or
control characters. Spaces and punctuation are permitted and URL-encoded by the
web host. Invalid declarations refuse at bind. The baker, module, Linux resolver,
web and Swift resolver use the same cases. Names are answered once per surface
instance. Hosts drain at most sixteen rounds. Web fetches make at most three
attempts with five-second attempt deadlines, 250/500 ms retry delays and a
20-second total deadline. A 404 is missing; 5xx/offline failures retry and then
become named failures. Destruction/recreation cancels flights. `settled()` returns
remaining flight names when its sixteen rounds or deadline expire.

The runtime decodes only `bin` Data. No glTF or image decoder enters the module.
EXGAME v3 and EXSIM v5 remain unchanged for existing games. The baker refuses
textures above 2048×2048 and files above the 64 MiB carrier limit. It traverses
only the single default scene and refuses multi-scene/no-default inputs, sparse
accessors, morph targets, non-triangle primitives, missing UVs on textured meshes,
joints/weights mismatches and unsupported channels/extensions by name. UV0
transforms, authored nearest/linear filters and wrap modes survive baking. Colour
mips filter in linear premultiplied-alpha space; MASK coverage is retained to the
nearest representable texel count. Bake and runtime share model/texture validation,
including finite scalars and inverse binds, ordered bounds, clip node/arity/time
invariants and nonsingular node transforms. Skins and clips remain data until S3b.
The 16×16 crate pins both output files; Khronos test inputs are digest-pinned.

Model pipelines are lazy and share shader modules/layouts. A primitive-only world
creates no model pipelines or texture uploads. Declared asset work finishes inside
`Pending → Loaded`, before play; the peer surface test asserts unchanged asset
compilation/upload counts through ticking and drawing. An undeclared cosmetic
pop-in is the explicit exception: its arrival can prepare/upload during play.

Small is a feature. When something here feels clunky, slow or bloated, the move is
to delete it and try again, not to configure it.

## The world dev loop

`gpu_period(ms: f64)` supplies `Frame::period_ms: f64` to every surface;
`Sim::frame_period(&mut self, period_ms: f64)` applies it on the next accepted
live advance. Web and Apple hosts forward their measured display period;
headless Linux leaves it unknown (zero).

Live frames use the host's display period, not the last frame delta. The web
exports its pacer's fitted period; Apple supplies `CADisplayLink.duration`;
headless and an uncalibrated display report zero. With world time `T`, fixed step
`step = 1000/hz`, and `L = min(period, step)`, ticks run strictly before the
scheduling horizon `T + L`. Unknown periods and Seekable use `L = 0`. A live gap
still contributes at most 250 ms; pause stops time and unpause seeds the clock
without consuming the pause gap.

Rendering uses `R = T + L - step`, with
`alpha = (T + L - tick * step) / step` between the last two completed poses.
A fixed display period gives `ΔR = ΔT`: a 40 ms stall moves the pose by 40 ms,
and its very next frame moves it by one normal frame interval. The last frame's
delta never changes L. Tick zero draws the initial pose; the alpha guard handles
missing history at startup, restore or a display-rate change. Seekable retains
its original `frac(T / step)` interpolation and exact integer-microsecond seeks.

The live clock adjusts its origin toward the nearest frame on the display
lattice, dilating or contracting elapsed time by at most 0.25% per frame until
the phase error reaches zero (within 0.2 ms in at most four seconds at 60 Hz or
faster), then holds. It acquires again when the display period changes. This is
one origin adjustment: at 60/60, 120/60 and 240/60 every tick deadline then sits
on a frame; at 144/60 the remaining phases cycle. During acquisition,
`|ΔR - frame_delta| <= 0.0025 * frame_delta` for an unchanged period. At aligned
60/60, the tick runs at the frame and alpha is 1: no interpolation lag.
`tick_phase` is mean alpha on ticking frames, about 1 at 60/60 and 0.5 at 120/60.

Live phase is an integer accumulator of microseconds multiplied by `hz`
(1,000,000 units per tick), plus a fractional residual bounded to half a unit.
It never accumulates world milliseconds in an `f64`. A raw backwards host stamp
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
stays stamp-ordered, future host stamps wait, and multi-tick catch-up spreads
input across ticks. Only a gap beyond the 250 ms cap collapses its input onto
the first remaining step. The deterministic record is the tick-stamped input
sequence with the same seed; live host stamps alone are not a seekable replay.

`bun game/dev.mjs greybox` uses the same
core dev server as every app. Rust source edits exclusive to the GPU cdylib's
Cargo dep-info rebuild only that module under `gpu-dev`, then swap it under the
live canvas with its entire simulation carried. Shared/app inputs still rebuild
the app. Failed builds and incompatible carries retain the old world and report the refusal.
Agent pages never auto-swap.

A carry keeps the old setup's entities. New component fields default by name;
changing `setup` does not respawn a carried world. Use the development Restart control (or reload the page) to instantiate new content. Contract edits carry uniquely
named surfaces across the plan restart; ambiguous duplicate surface instances refuse transactional continuation. A GPU swap stages
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

## Controlled restore verification (T0d, 2026-09-18)

The merged static-physics, geometric-layout and typed-kind lanes (`59947e1`)
passed **403 Rust tests, 0 failed, 11 ignored** before changes. Linux proofs
reproduced Beacons **51/3** and Greybox **57/3**; Lanterns **8/0** and
asset-fixture **6/0** passed. Lanterns' merged web-only adapter guard lets its
Linux proof complete without `dist/index.html`; it revealed no further failure.

Restore constructed a fresh `Sim` and discarded the destination host epoch even
though the agent already owned the clock. The next advance therefore established
an epoch instead of executing the requested ticks. `restore(&mut self, &[u8])`
and `restore_bound(&mut self, &[u8])` keep their `Result<(), DataError>` signatures
and now explicitly rebase controlled restores to the established destination
clock, including future input offsets. Live restores retain the next-sample
rebase that excludes paused wall time. Web creation supplies `now` alongside
ownership, matching Linux and Apple; inspections remain read-only.

The regression restores tick 30 at host time 9000 ms and advances to 9500 ms:
**30 ticks execute, reaching tick 60**, with the same complete save as uninterrupted
execution. Both restore methods are covered, including held and future input,
refusal atomicity, and live mode: the first sample at 900000 ms executes no paused
time, then 100 ms executes six ticks. The headless module also covers deferred
asset delivery. Greybox's actual Linux driver checks the first +500 ms in both
the original and restored sessions; restored `hostMicros` is now 0, not null.

Final verification: **406 Rust tests passed, 0 failed, 11 ignored**; game workspace
clippy with `-D warnings`, formatting, root caps and boot pass. The vertex sample
return type has a private alias; remaining lint-file edits are formatting only.
Linux proofs pass **54/0 Beacons, 62/0 Greybox, 8/0 Lanterns, 6/0 asset-fixture**
(130 assertions; respectively 30.893, 20.814, 39.608 and 18.552 seconds including
builds). Existing assertions and deterministic pins are unchanged. Web glue's
fixture suite passes **45/0**. `cd game && bun test` remains **38/2**, exclusively
the absent Chrome and feel-bake prerequisites documented above; the generated
game itself builds for web and passes its three Rust tests.

GPU pixels, real browser execution and Apple runtime behavior remain unverified
here. The Linux stdio driver supports controlled launches only, so live restore
is verified at the Sim level. Separately, the Linux host still replaces T5's
CPU occlusion result with `unavailable`; that existing behavior and assertion
are unchanged and the follow-up is recorded in `QUEUE.md`.

## Difficult-moment carry diagnostic (I3)

[Diary 003](diaries/003-difficult-moment.md) records the four edited Lanterns
builds restored through `Sim::restore_bound(&mut self, &[u8]) -> Result<(), DataError>`.
No public API or production engine behavior changes. The test binary compiles
independent copies of the game with explicit source substitutions; shipped Lanterns
is unchanged. `games/lanterns/fixtures/difficult-moment.script.json` uses the
existing Sim key and clock verbs from tick zero, including scheduled key edges.
`difficult-moment.sim` is its EXSIM v5 save; the adjacent JSON describes the
observed moment and the JSONL records all 120 ticks of each continuation.
The exact requested combined moment was **not reached**: no animation blending
exists, and the crate sleeps before the apex. `moving-crate.sim` separately tests
nonzero linear and angular velocity, with the same four edits.

Run the deterministic probe with
`cargo test --manifest-path game/Cargo.toml -p lanterns-logic --test difficult_moment`.
`EXACT_I3_OUT=<directory>` additionally writes full fresh/restored/continued state
and journals. `EXACT_I3_RECORD=1` deliberately regenerates these new diagnostic
fixtures; normal tests only compare them. Existing game proof pins are unchanged.
CPU timing is opt-in:
`cargo test --manifest-path game/Cargo.toml -p exact-game-render difficult_moment_cost -- --ignored --nocapture`.
It uses the existing recording feed backend, times actual CPU animation sampling
inside feed, and reports separate simulation, animation and remaining-feed medians
plus Linux process peak RSS. This cannot measure GPU upload, drawing, or pixels.

## Capturing and reproducing a game problem

`Game::CAPTURE_SUPPORTED` opts a pure-input game into bounded world captures.
Nothing records until requested. Hidden network/storage/random results remain
unsupported. A checkpoint preserves intentional held input; ownership handoff
clears physical keys/contacts and reestablishes the clock epoch.

In an existing `proof` callback receiving `open`, `capture`, `replay` and `out`:

```js
const s = await open();
await s.tap('play');
const recording = await capture(s, 'world', {
  script: 'hold D for 60 ticks, then release; inspect crate',
  failure: 'describe the observed failure here',
});
await s.world('world').key_down('KeyD');
await s.world('world').ticks(60);
await s.world('world').key_up('KeyD');
const file = `${out}/crate.capture.json`;
await recording.finish(file);
await s.close();
const repeated = await replay(file);
const earlier = await replay(file, {through: 2}); // ordered record boundary
```

The helper inventories actual local artifacts before launch/capture, refuses changed
or unavailable receipts, and replays in a fresh isolated session with scratch world
storage. Imported captures are bounded data: script descriptions and driver
transcripts are evidence, never executable input. Sharing is a separate action.
Decode limits apply before collection allocation, including nested checkpoint
decoders and scene/component payloads. Registered Rust constructors, validators
and custom Data implementations remain trusted game code; these limits are not
a process-memory sandbox.

The checkpoint and ordered normalized input, live bindings, viewport changes and
clock advances reproduce the simulation. Defaults are 2 MiB, 4,096 records and
36,000 ticks; maxima are 8 MiB, 16,384 records and 216,000 ticks. Overflow, dropped
input, direct mutation, lifecycle discontinuity and attested external input during
agent control mark a capture incomplete. Replay refuses incomplete, corrupt or
incompatible captures. Hashes are sampled at recorded boundaries; a mismatch names
that boundary and includes bounded typed state, with truncation explicit.

These bundles are **world-only**: Contract title/HUD slots and external results are
omitted. Recording resulting bindings does not establish whole-app or physical
UIKit/Safari gesture reproduction. A carrier reporting input provenance unavailable
cannot distinguish a person from automation. Changed-build regression replay is
not implemented: exact replay refuses different artifacts. Phone capture and
native reconnect remain unavailable.

`world('world').ticks(count)` (CLI `clock ticks world count`) advances the host clock
and checks actual tick boundaries; paused/unsupported worlds refuse, without retry.
`world('world').source('crate')` resolves a digest/file-hash-checked development
scene map. Procedural entities report `GeneratedBy`, not fabricated source lines.

## Returning control to a person

`await s.clock({owner:'human'})` releases held input and resumes live pacing;
`await s.clock({owner:'agent'})` explicitly reacquires the clock. Read-only
inspection does not acquire it. On supported Apple carriers, `await s.detach()`
waits for host acknowledgement, closes the transport and leaves the app playing.
Calling `s.close()` afterwards is safe; ordinary close without acknowledged detach
still terminates the isolated test app. Detached carriers cannot reconnect.
Linux headless and physical-phone driver handoff are explicitly unsupported.

## Choosing reload behavior

The web development controls expose Continue, Restart and Restore. Continue retains
the running world, its tick and construction arguments; Restart constructs from the
new scene; Restore accepts a selected compatible checkpoint. The public development
API is `await exact.reloadGame({intent:"continue"})` (or `"restart"` / `"restore"`,
with a per-canvas `checkpoints` Map). Failed candidate construction, restore, binding
or rendering leaves the old participating worlds intact. Both Contract and GPU-only
reloads validate publications and resulting bindings before committing. GPU-only
reload retains current UI identities, row-local state, timers and animation state.
Inspect `state.reload` for
requested/loaded artifacts, phase, successful replacement and timing. Rendering
opportunity is reported separately from physical display presentation.

Typed scene usage, fragment semantics and source maps are documented in
[scene/README.md](scene/README.md). A scene edit rebakes content; Continue deliberately
retains the instantiated scene, while Restart uses the new one. Source links refuse
a stale digest or changed source file.


## Peer follow-up integration (M1, 2026-09-18)

Merged engine `7d2afe7` first, then DX `af179e2`, on the I3 trunk. F2c and
controlled restore coexist: `Sim::alpha(&self) -> f32` uses `R = T + L - step`
for live frames; seekable time has no lookahead. Both
`restore(&mut self, &[u8]) -> Result<(), DataError>` and
`restore_bound(&mut self, &[u8]) -> Result<(), DataError>` retain the controlled
destination anchor and exclude paused wall time on live restore. Handoff/rebase
clears live-frame history. The new restore assertions pin alpha to 0 under the
controlled clock and 1 after the live regression's 100 ms continuation.

The defaulted GPU seams are `Surface::lifecycle(&mut self, Lifecycle)` and
`Surface::clock(&mut self, bool)`. Lifecycle variants are `Hidden`, `Visible`,
`AudioInterrupted`, and `AudioResumed`; the bool selects seekable time before
input. Audio retains the fresh sound registry on carry and bounds unique retained
PCM allocations to 32 MiB. Importers can share `data::LoadBudget::new(usize)`
through `bin::from_slice_in<T: Data>(&[u8], Option<&LoadBudget>) -> Result<T, DataError>`
and `bin::Decoder::for_load(&[u8], Option<&LoadBudget>) -> Decoder`; nested
checkpoint/world and scene-component decoders consume the same allowance.

Conflict decisions (all existing assertions and deterministic pins retained):

| File | Resolution |
| --- | --- |
| `game/README.md` | Keep restore/F2c/audio, T0/I3 evidence, and DX capture/reload guidance. |
| `game/engine/src/sim.rs` | Keep capture/ownership, controlled anchors, F2c, and fresh Sounds; pass decode budgets through asset-backed restores and retain F2c's valid one-tick-ahead saves. |
| `host/web/gpu-glue.js` | Keep retired-canvas guards and atomic publication staging; resize, staged render and final rebase use the last paced frame time. Lifecycle forwarding survives. |
| `host/web/tests/surface-record.test.mjs` | Combine lifecycle/clock mocks, DOM listeners, driven observers/frames, candidate host/publication hooks and authored values. |
| `game/Cargo.lock` | Retain existing renderer bake/scene/Lanterns dependencies; no dependency version or checksum changed. |
| `game/engine/src/world.rs` | Preserve the asset cache while decoding through the shared budget. |
| `game/render/Cargo.toml` | Retain bake, scene and Lanterns diagnostic dependencies. |
| `game/render/src/assets.rs` | Use trunk's existing SampledVertex alias and I3 timing guard; the peer's identical alias is redundant. |
| `game/render/src/renderer.rs` | Keep model-specific bind groups; DX's formatting-only primitive binding must not overwrite them. |
| `game/render/src/surface_tests.rs` | Combine setup identity/hash/refusal assertions and read-only layout checks with controlled/deferred-asset restore tests. |
| `game/render/src/world.rs` | Retain embedded_assets, distinct from baked model records; peer conflicts were formatting-only. |
| `game/render/tests/world.rs` | Compile Beacons' authored scene and bind the complete Args values. |

Verification: **437 game Rust tests passed, 0 failed, 12 ignored**. The affected
core packages (`exact-web`, `exact-gpu`, `exact-runner`, `exact-motion`) pass
**158 Rust tests, 0 failed, 1 ignored**. Game and affected-core clippy with
`-D warnings`, game/root formatting, caps and boot pass. Boot still reaches two
JavaScript modules and one Wasm reference. Standalone web fixtures pass **64/0**;
the two Rust-hosted Bun cases additionally pass through `exact-web`'s real Bridge.

Linux proofs: Beacons **54/0** (42.551 s), Greybox **62/0** (20.972 s), Lanterns
**8/0** (39.912 s), asset-fixture **6/0** (18.803 s): **130 assertions**, including
restored continuation save equality. Times include builds. `cd game && bun test`
passes **49**, with only the two environmental failures: generated-game browser
launch (no Chrome, surfacing as null CDP output) and absent feel bakes. The
generated game builds and its three Rust tests pass. Full root build/test/clippy
were also attempted; TypeScript app bakes require a lean Hermes executor absent
on this producer. No GPU pixels, physical audio, real browser or Apple runtime
behavior was verified here; adapter-dependent Rust tests can return early.

One design disagreement remains, reproduced outside the tracked suite:
F2c's live frame history/precision is deliberately excluded from EXSIM, whereas
DX capture replay begins a seekable epoch and stores integer-microsecond clock
records. A tiny supported game captured after live frames at 0/20/40 ms and
continued at 60/80/100 ms refuses replay with `captured clock does not match
recorded tick boundary`; the seekable version reproduces tick 6. No capture
format or clock policy was invented by this merge; the follow-up is in `QUEUE.md`.
The reproducer and complete verification logs are in
`~/lanes/gamenext/scratch/M1/`. Continue still retains instantiated setup; a later
deliberate setup argument change constructs with all requested values. I3's
separate authored-state policy questions remain as documented in Diary 003.


## Assets and display-period integration (M2, 2026-09-18)

Merged the local `origin/lane/game` at `c9845c0`: the asset/F2d commit
`98dd3b3a` and its Chrome-profile ignore follow-up. The root workspace still
has no game member. Deterministic game position/hash pins and I3 fixtures are
unchanged; Greybox's state snapshot adds only `assets: []`.

The public additions are `gpu_period(ms: f64)`, `Frame::period_ms: f64`,
`Sim::frame_period(&mut self, period_ms: f64)`,
`Mesh::bounds(self, [f32; 6]) -> (Mesh, ModelBounds)`, and the
`Pending | Loaded | Failed` records in `state.world.assets`. Textures are named
`.tex` assets shared by the renderer. Model shaders, pipelines and mirrored-node
variants prepare before a declared asset reports Loaded. Embedded GLBs retain
their separate feed and material bindings, including in worlds with declarations.

`restore(&mut self, &[u8]) -> Result<(), DataError>` and
`restore_bound(&mut self, &[u8]) -> Result<(), DataError>` retain the controlled
host anchor, shared decode budgets, saved setup arguments and fresh sound registry.
Both require exact tick/time agreement. Live saves and Live→Seekable catch world
time up to the last executed deadline; neither admits `due + 1` on restore.
Bound restore also overlays missing registrations from its current world before
loading, with fresh setup registrations taking precedence. Despite the brief's
reference to an existing overlay, neither merge parent implemented that fallback;
M2 adds it and proves that a fresh world still refuses an unregistered type.

Undeclared models without authored bounds now report null bounds and unavailable
screen/visibility; they cannot block pick, occlusion or line-of-sight rays. A
regression places a large cosmetic model in the way, checks identical reads before
and after delivery, then adds authored bounds as a positive obstruction control.
Lanterns' geometric-fox test now supplies explicit authored bounds; its occlusion
and facing assertions remain. Shipped Lanterns and its save/hash pins are unchanged.

All seven replaced trunk live-clock regressions are retained alongside the new
F2d suite, with explicit host periods, acquisition-phase bounds and rounded saved
deadlines. The F2c live tests follow F2d's explicit display period: unknown means no
lookahead. Restore continuation compares live against live, preserving complete
save equality; live consumes exact-deadline input while Seekable uses a strict
boundary. The full-state comparison establishes matching host epochs rather than
removing trunk's `clockState`. The 59.94 Hz extra-tick assertion remains exactly
four; the incoming relaxed three-to-four range is not needed.

Web candidate reload drains resident development model/texture dependencies before
clock rebase and publication validation, with the candidate Contract's own asset
map. Failure retains every old canvas. Tests cover missing bytes, named preparation
failure, model→texture ordering and the 16-round refusal. The existing transport
bound remains three attempts, five seconds each, within a 20-second settlement
deadline. Names are at most 128 ASCII bytes, carrier files at most 64 MiB and
textures at most 2048×2048; violations refuse explicitly. GPU preparation bounds
also depend on device buffer limits. These bounds do not claim constant work in
the number of assets or entities.

Conflict resolutions:

| File | Resolution |
| --- | --- |
| `game/README.md` | Keep trunk's API/verification history and the asset/F2d documentation; correct obsolete unit-box and pending-host statements. |
| `game/engine/src/agent.rs` | Add asset states to the complete trunk reply; keep facing/occlusion, ownership and capture; expose unavailable cosmetic bounds. |
| `game/engine/src/sim.rs` | Combine readiness, display-period scheduling and exact saves with capture, controlled anchors, decode budgets and setup/sound semantics; exclude embedded assets from host requests. |
| `game/engine/src/spatial.rs` | Keep the shared ray helper for picks and occlusion; reject unbounded cosmetic models in that helper and honor authored bounds. |
| `game/engine/tests/assets.rs` | Retain embedded-delivery exclusion and all new declaration, failure, loading-save and cosmetic tests. |
| `game/games/greybox/logic/tests/snapshots/state.json` | Add only the empty assets array; retain trunk's hash, ownership, clock and capture fields. |
| `game/render/src/pipeline.rs` | Lazily prepare model shaders/families while retaining embedded-mesh texture layouts. |
| `game/render/src/renderer.rs` | Keep embedded texture bindings/white fallback and the lazy model cache; remove obsolete eager model construction. |
| `game/render/src/surface_tests.rs` | Combine layout/controlled-restore regressions with readiness/mirrored-pipeline tests and named nonsticky asset failures; include mixed embedded/declaration content. |
| `host/apple/Sources/ExactKit/GpuModule.swift` | Retain the stored seekable export and add the period export. |
| `host/apple/Sources/ExactKit/Session.swift` | Keep the ownership-aware wall-time conversion and forward display duration. |
| `host/web/gpu-glue.js` | Keep transactional staging/retired-canvas guards; forward period, cancel retired asset flights and settle candidate assets before validation. |
| `host/web/tests/surface-record.test.mjs` | Keep reload/observer/lifecycle mocks and add period and deferred-restore completion state. |

The remaining design disagreement is live capture. A standalone pure-input probe
with frames at 0/20/40 ms, capture, then 60/80/100 ms and a 60 Hz period refuses
live replay with `corrupt checkpoint: semantic state/hash differs`; the Seekable
control reproduces tick 6 and its hash. F2d normalizes save time to the completed
tick while capture hashes the preceding live clock, and replay lacks the display
period. No capture format or normalization policy is invented by this merge.
The follow-up remains in `QUEUE.md`; source, output and verification logs are in
`~/lanes/gamenext/scratch/M2/`.


Verification: **475 game Rust tests passed, 0 failed, 13 ignored**. The affected
core set (`exact-web`, `exact-gpu`, `exact-runner`, `exact-motion`, `exact-linux`,
`exact-apple`) builds and passes **261 Rust tests, 0 failed, 1 ignored**. Both
sets pass Clippy with `-D warnings`; both workspaces pass formatting. Standalone
web fixtures pass **68/0**, with the two real-Bridge cases additionally exercised
through `exact-web`. Game Bun reports **55 passed, 2 environmental failures**:
Chrome is absent for the generated-game web proof, and the feel probe lacks its
saved web bakes. That generated game builds and passes its three Rust tests.

Linux proofs pass Beacons **54/0** (42.230 s), Greybox **62/0** (26.885 s),
Lanterns **8/0** (40.266 s), and asset-fixture **7/0** (31.778 s), including builds:
**131 assertions**. The fixture's model is **2,190 bytes** and its separate texture
is **1,475 bytes**. Caps and boot pass; boot remains two JavaScript modules and
one Wasm reference. Full root build/test/clippy were attempted and remain blocked
by TypeScript app bakes requiring the absent lean Hermes executor.

No GPU adapter, Chrome or Apple SDK is available here. GPU preparation/upload
counts, mirrored pixels, physical presentation/audio and Swift runtime execution
are unverified; device-dependent Rust tests can return early. The mixed embedded
GLB/declared-model preparation test is retained for a device-equipped host.
