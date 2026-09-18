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
names, nearest ray intersection first after rounding distances to 0.0001 m,
entity index breaking quantized ties; unnamed
entities use `#index`). Occlusion counts the eight oriented-bound corners,
six face centres and centre equally. It is a geometric estimate, independent
of material opacity and rendered pixels. Behind-camera or out-of-frustum bounds
report `occluded: null` and a `reason`; `behindCamera` means all eight corners
are behind the eye. Hidden subjects also report null occlusion. Cameras are
perspective-only. Hidden entities, fully degenerate bounds and
singular transforms do not obstruct. Visibility rays count entry surfaces only:
a shape containing the ray origin (including its boundary) is ignored, so a
room containing the camera cannot hide its interior. Picking still returns exits. Rays use the same primitive intersections
and declared model boxes as picking. Undeclared models without authored
`ModelBounds` have null bounds and unavailable screen/visibility reads; they
never block a pick or occlusion ray, even after their presentation bytes arrive.
Authored bounds remain usable before and after cosmetic delivery.
A missing global pose reports `world: null` and unavailable geometry with a
reason, never an identity pose. `entity.facing.forward` is normalized world −Z
(null for a collapsed axis), `towardCamera` its dot with
the normalized direction to the active camera (null without camera/viewport).
With `to`, facing also contains `bearingTo` (signed degrees about +Y, −180…180,
with the boundary canonicalized to +180),
`distanceTo` (world-origin distance) and `lineOfSight` (open origin-to-origin
segment). Each endpoint and its ancestors and descendants are one object for
this read: their meshes are excluded. Camera occlusion applies the same rule to
the subject and active camera; LOS applies it to subject and target. Siblings
remain separate objects, even under a common scene root. Exclusion crosses every
Parent edge (there is no separate rigid attachment edge). Coincident origins have distance
and bearing zero and clear sight; zero horizontal directions have bearing zero.
A missing target pose makes distance, bearing and LOS null with a reason;
a collapsed forward axis makes bearing null. Rounded JSON canonicalizes signed
zero to 0. Visibility retains `{"unavailable":true}` without camera/viewport.
These queries take only shared world reads and preserve the mutation epoch.
Visibility uses a separate, retained mesh-bounds BVH (physics indexes colliders).
The cache is world-owned derived data, excluded from saves, hashes and observation.
It invalidates on Mesh, Transform, Parent, Visible and ModelBounds revisions,
entity membership, propagated hierarchy generation, presentation generation and
asset geometry revision. Layout never advances or mutates simulation state.
An index rebuild admits at most 262,144 entity slots, including dead slots;
beyond that `layout` returns an explicit index-limit error before allocation.
Balanced median construction is O(N log N), at most 19 levels and 524,287 nodes.
Each request additionally allows 1,000,000 total hierarchy-exclusion/BVH-node
visits across all 15 sample rays and the optional line-of-sight ray; exhaustion
returns `layout visibility work budget exceeded`, never a partial fraction.
Exclusions are built once, line-of-sight exits on the first blocker, and rays
allocate and sort no hit lists. Occlusion retains only four distinct nearest hits.

T5b diagnostic (`cargo test --manifest-path game/Cargo.toml -p exact-game --lib
mesh_bvh_200k -- --nocapture`), optimized test profile, shared x86-64 Linux:
200,000 meshes, a 99,999-deep chain, interleaved/reversed spatial order, recycled
slots, a Character component, a loading cosmetic model and a subject 10 km from
the camera. The complete layout request (15 samples plus `to`, including JSON)
measured **84.679 ms cold**, **1.972 ms warm median / 1.998 ms maximum** over nine
warm reads, with **300,737 visits**. Construction of the world is excluded;
cold includes building the index and warm includes rebuilding endpoint exclusions.
The successful answer must name the real wall and change when it is removed.
The genuinely unprunable case, 200,000 overlapping on-ray meshes, explicitly
refuses at **1,000,000 visits**, **102.424 ms cold / 35.775 ms warm**; the same
scene's boolean LOS stops in fewer than 64 visits. These are diagnostic timings,
not a frame-time guarantee. The slot-limit test includes despawned slots.

A temporary negative control returning zero occlusion and no occluders failed
three engine tests and the viewport-bearing WorldSurface snapshot test; restoring
the implementation passes those tests. Linux preserves the engine's geometric
visibility reply even without a GPU; Greybox compares the same native snapshot's
geometry fields (screen coordinates depend on the host viewport).



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
saved IDs and updated scene identity to its pre-binding fixture before comparing.
M3 reran that pre-binding game with EXPHYS v2 in all three paranoid modes to
regenerate ticks 60/180; tick zero and the Beacons captures remain byte-identical. No
fixture directory environment variable is needed. Both games also restore and
advance after every tick; immediate comparisons cover world bytes, and following
ticks compare complete Sim saves (restore clears consumed input edges).

T3b2 verification on the merged `9bd587bffaa3a74a6293153593fc1df78f5d351f`
trunk, before M3: **493 Rust tests pass, 1 fails, 15 ignored**. This
was the lane's shipping blocker; M3 resolves it with EXPHYS v2 below.
The active Lanterns restore-every-tick test diverges on its first continuation
(loop index 1): live world hash `f2e496b15441b4e5`, restored
`b981f5010ceaad9d`; complete world bytes also differ. Three fix rounds stopped,
the failing assertion remains, and `QUEUE.md` records the reproducer. Beacons'
180-tick restore continuation and both games' partial-entity/idle-page tests pass.
The child replacement regression separately proves identical live/restored errors
and bytes. No cause is claimed for the remaining continuation failure.

Game workspace Clippy (`-D warnings`) and formatting pass. Bun is **55/2**:
the generated-game browser launch needs Chrome, and the feel probe needs its
60/120 Hz web bakes. All Linux proofs pass: Beacons **54**, Greybox **62**,
Lanterns **8**, asset-fixture **7** assertions (131 total). Their elapsed times,
including builds, were 71.747 / 21.466 / 54.057 / 19.408 seconds respectively;
see the run logs for the actual build breakdown. GPU pixels, browser and Apple
runtime execution remain unverified. Full root build/test/Clippy are blocked by
the absent lean Hermes executor; root formatting passes.

Source counts (`logic/src/lib.rs`, including inline tests): Lanterns **634 lines,
9 `unwrap()` and 8 `expect()`**; Beacons **159 lines, 0 `unwrap()` and 2
`expect()`**. Lanterns' separate 63-line kind/binding module is deleted (previous
combined count: 620). Both game tick call paths contain zero `unwrap()`,
`expect()` or panic-substituting `unwrap_or_else`; setup binds the saved IDs.
Five fresh 10,000-tick samples in the optimized dev/test profile, with held W:
Lanterns **167.818 µs/tick** median (range 165.775–196.900), Beacons
**1.896 µs/tick** (1.397–1.935). These are shared-machine diagnostics, not a
paired performance claim against the earlier T3b measurements.

Only Lanterns pins changed: saved `Lantern.bulb`, saved `Session.actors`, and
the scene digest of the newly saved default ID. The three world hashes at
0/60/180 are `a778d065d6cea372`, `8f7cfe89cd32bdef`, `ecf7e7cab49ab213`.
[Every changed pin, with file:line and old → new](games/lanterns/fixtures/binding-hash-changes.txt)
includes the 609 existing difficult-moment hash occurrences as well as these
three pins. The two difficult-moment binary saves grew by 1,160 bytes each;
all existing non-hash gameplay fields in the JSON/JSONL fixtures compare equal.
The committed before-change fixtures themselves remain unchanged.

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
Asset declarations keep EXGAME v3; simulation saves now use T2's EXSIM v6. The baker refuses
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

A carry compares the old build's tick-zero values, the new build's tick-zero
values, and the carried world, field by field through `Data`. A changed initializer
applies when the carried value still equals its old initializer. If the simulation
changed that field, its current value stays; the report says “kept: initializer
changed, applies on restart”. Unedited fields stay silent. This is value comparison,
not write-history tracking: a field that returned to its initializer is eligible.

`Sim::restore(&mut self, &[u8]) -> Result<(), DataError>` and
`Sim::restore_bound(&mut self, &[u8]) -> Result<(), DataError>` remain atomic.
The latter compares against the destination's freshly constructed scene while
retaining saved construction arguments and current live bindings. Patches use the
ordinary mutation leases, so physics and renderer change detection see placement
and appearance edits. Named entities and entity-handle fields match by unique name;
unnamed entities require the same tick-zero index and component set, and a surviving
carried incarnation. Ambiguous identities are reported as `unmatched`.

The canvas journal and `state.world.reload` contain `applied`, `kept`, `added`,
`removed`, and `unmatched` arrays, with entity/component/field, old/new initializer
previews, and reason. The report stays until the next restore (or restart), outside
the world hash; at most 64 items are retained, then `omitted` counts the remainder.
Each text preview is at most 256 characters. The report's `old` and `new` are JSON
preview strings. App-level `state.reload` continues to describe the host reload.
New/removed entities, components and resources are reported for restart; carry never
spawns or despawns them. New schema fields retain the existing load-by-name defaults.
Use the development Restart control to instantiate new content.

EXSIM v6 adds the old tick-zero Data projection as compact bulk bytes. It also
retains the arguments that produced that base when they differ from the carried
gameplay arguments, so saving and restoring again cannot undo an applied scene edit.
Older saves
are refused with an explicit restart-required error; no inferred base or migration
is possible. Unedited restores preserve all v6 save bytes, world hashes, queued and
held input, executor state and clock continuation. Records merge recursively by
field name; sequence fields are values. `#[data(skip)]` is excluded, including during
in-place patch reads. `Writer::entity(index, generation)` defaults to the unchanged
Entity wire/hash record; reload overrides it to resolve names. `Reader::patching()`
defaults to false; derived and container readers preserve existing skipped members
when applying a reload patch.

Initializer projection is bounded to 1,000,000 slots (including dead slots), 256
storage types, 16 million visits, depth 64 and 512 MiB of accounted projection
storage; encoded bases are at most 128 MiB. Changed-build decoding shares a 1 GiB
allowance, or an importer's tighter `LoadBudget`. Overflow refuses with `DataError`
before replacing the live simulation. Work is linear in visited Data plus ordered
map lookup/insertion, at most O(V log V) with bounded depth; no merge runs in a tick.
Registered custom `Data` code remains trusted, as it is for save/load.

Contract edits carry uniquely named surfaces across the plan restart; ambiguous duplicate surface instances refuse transactional continuation. A GPU swap stages
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

## Proving that state is saved

**Paranoid execution is THE way to show a new component/resource is saved
correctly.** Run its scripted simulation normally, then with every-tick
reconstruction; compare final world hash, tick, published record and journal.
A round-trip hash alone cannot detect a skipped field that affects the *next* tick.
The consumer tests in `paranoid-test.rs` compare complete EXSIM saves as well.
They include physics, input, clock remainder, publications and journal cursors.

`Sim::paranoid(self, mode: Paranoid) -> Self` overrides the driver environment.
`Paranoid::{Off, Save, FreshGame}` default to `Off`; `EXACT_GAME_PARANOID=1`
selects `Save`, and `EXACT_GAME_PARANOID=fresh-game` selects `FreshGame` for
native simulations, including tests and Linux proof modules. The instrument itself
changes no save format or pin; the EXPHYS correction below versions physics data.

```rust
# use exact_game::*;
# struct Example;
# impl Game for Example {
# const ID: &'static str = "paranoid-example";
# type Args = ();
# fn setup(w: &mut World, _: &()) { w.spawn(Transform::default()); }
# fn tick(_: &mut World, _: &Input, _: &()) {}
# }
let mut sim = Sim::<Example>::new(())?.paranoid(Paranoid::FreshGame);
sim.run(1000.0); // Every completed tick saves, rebuilds and asserts its world hash.
# Ok::<(), String>(())
```

```sh
EXACT_GAME_PARANOID=1 cargo test --manifest-path game/Cargo.toml -p greybox-logic
EXACT_GAME_PARANOID=fresh-game cargo test --manifest-path game/Cargo.toml -p lanterns-logic
bun game/games/greybox/proof.mjs linux --paranoid
# Likewise beacons, lanterns, asset-fixture, and game/bench/cubes/proof.mjs.
```

`--paranoid` runs the actual proof once normally, once in each paranoid mode,
and compares every session's final hash, tick, publications and complete retained
world journal. Ordinary proof assertions run in all three modes. The flag requires
Linux: browser Wasm cannot read a native process environment. Direct environment
runs exercise reconstruction but do not themselves supply a normal-run comparison.

Both modes use the production `restore` path: freshly decoded arguments, actions,
setup/registration, component/resource values (including skipped fields), hierarchy,
spatial/hash caches, and physics executor reconstructed from its saved snapshot.
`Game` is a type with static functions, not an owned instance; there is no `G` value
to clone or retain. FreshGame additionally drops the old world *before* setup and
re-encodes/decodes immutable model assets instead of sharing their Arcs.
Statics are deliberately outside this instrument.

The surrounding driver retains the seek horizon and host epoch, capture recorder,
ownership/contamination, undelivered messages/publication notification, asset I/O
request bookkeeping, and observation samples/backoff. These are transport outputs
or the test observer, not inputs available to `Game::tick`; the observation result
is recomputed on the rebuilt world. The checkpoint uses the completed tick boundary,
then restores the seek horizon, so large seeks and future input remain meaningful.
Restore intentionally clears consumed input edges; the next tick clears them in a
normal run too. Comparisons at script completion include held/future input.

Static audit (`rg "static |thread_local|OnceLock|lazy" game/`, 2026-09-18):
`'static` lifetimes, comments about static geometry/lazy evaluation and archived
`artifacts/d7/d7.patch` are not storage declarations. No OnceLock/lazy singleton was
found. Every actual declaration is accounted for below; none supplies hidden
simulation state in shipped games.

| Declaration (under game/) | Verdict |
| --- | --- |
| `games/lanterns/logic/src/lib.rs`: `ASSETS` | Harmless immutable asset table. |
| `render/src/lib.rs`: generated `REGISTRY`; `render/src/surface_tests.rs`: both `REGISTRY` fixtures | Harmless immutable surface factory tables. |
| `render/src/perf.rs`: `PERFORMANCE` | Harmless browser timer handle cache; presentation timing only. |
| `render/src/perf.rs`: `CLOCK_READS` | Harmless test instrumentation counter. |
| `engine/src/capture.rs`: `EXECUTED_TICKS` | Harmless test instrumentation counter. |
| `engine/src/world/tests.rs`: `MADE` | Harmless test-only allocation/laziness observation. |
| `engine/tests/g1c.rs`: `MOVES`, `WRITES`; `engine/tests/ergonomics.rs`: `WRITES` | Harmless test-only serialization/cost counters. |
| `engine/tests/ecs.rs`: `DROPS` | Harmless test-only destructor counter. |
| `engine/examples/memory.rs`: `BYTES`, `ALLOCATOR` | Harmless memory measurement instrumentation. |
| `render/src/world/tests.rs`: `COUNT`, `ALLOCATOR` | Harmless test-only allocation instrumentation. |
| `audio/src/surface.rs`: `ATTEMPTS`, `FAIL`, `UNLOCKS` | Test-only executor counters/failure injection; hidden test fixture state, not shipped simulation state. |
| `scene/tests/authoring.rs`: `SERIAL` | Harmless test scratch-name allocator. |
| `render/tests/timing/mod.rs`: `REPORTED` | Harmless one-time test diagnostic flag. |

The following T4 evidence predates M3 and its I3 fixture regeneration and saved
child bindings; the M3 section below supersedes the open failures and Lanterns
pins. The initial T4 sweep found an engine defect: Lanterns diverged at tick 2
inside `Physics.executor`'s Rapier snapshot. `BroadPhaseBvh::deferred_optimize_pending`
was skipped by Rapier's serde implementation; the engine's post-kinematic
`CollisionPipeline::step` can leave it set at a save boundary. The next normal
broad-phase update executes the deferred optimization; the restored one loses it.
The dynamic-stack fixture does not take that extra collision pass and agrees.

Decision (owner, T4 follow-up, 2026-09-18): a world restored from its save must
continue exactly like uninterrupted execution; this outranks old hash pins.
Rapier 0.35.3 is now vendored with `deferred_optimize_pending` serialized, and the
physics crate’s path dependency makes that fix reproducible for external consumers
too. The physics envelope is
**EXPHYS v2**. Nonempty v1/unversioned physics snapshots are refused explicitly
(`expected EXPHYS v2 ... snapshots incomplete; start a new world`), atomically.
There is no migration: v1 omitted the bit, so a correct continuation cannot be
recovered reliably. Worlds without a populated physics snapshot are unaffected;
EXGAME v3 is unchanged; T2 independently advances EXSIM to v6. No new engine
unsafe code is introduced.

Only the physics-dependent pins below changed. The assertions themselves now
compare uninterrupted execution with every-tick reconstruction before accepting
a pin; tick zero and all nonphysics game pins remain unchanged. Paths are relative
to `game/`, with current source lines.

| Pin location | EXPHYS v1 → v2 | Executed parity evidence |
| --- | --- | --- |
| `games/lanterns/logic/tests/timing.rs:50` | `0x99071d4692d75e6f` → `0x99dd217d6f058a61` | Tick 60: Off, Save, FreshGame hashes and complete Sim saves equal. |
| `games/lanterns/logic/tests/timing.rs:51` | `0xbb79c1986b61792a` → `0x432af075dec92c9b` | Tick 180: same three-way hash/tick/complete-save equality. |
| `physics/tests/scenes.rs:101` | `0x5ba7691abdc98058` → `0x129ba6d92f9ac217` | Pile: Off/Save/FreshGame hashes agree every tick through 600, final complete saves equal; ordinary tick-90 restore also agrees. |
| `physics/examples/minimal.rs:46` | `0x5ba7691abdc98058` → `0x129ba6d92f9ac217` | Same `common::scene("pile")` fixture; `pile --verify` checks restored continuation. |
| `physics/examples/pile.rs:61` | `0x9960c10fadbb9c4b` → `0x5608994347e54d28` | `simulate(120)` equals `simulate_with_restore(120, true)`; the latter saves/loads the world after each raw physics step and checks the immediate hash. |

`physics/README.md:80` updates the current pile description to the same v2 value;
its original v1 cross-platform measurements remain labelled historical. No other
executable pins were changed. There were **no committed EXPHYS v1 saves/captures
in the T4 clone** to regenerate; the existing JSON snapshots belong to Greybox,
which does not use Rapier. The Linux proofs regenerate their own ignored outputs.
The I3 difficult-moment fixtures were absent in T4 and were not touched there; M3
regenerated them as recorded below. T4's integration instruction was to
regenerate their saved worlds/capture checkpoints containing Physics, plus their
expected hashes and artifact receipts, through the I3 fixture scripts. Any v1
physics capture checkpoint will now refuse; filenames/scripts unavailable in this
clone were not guessed or reconstructed by hand.

The newly exercised child-respawn scenario finds a **game-side** hidden cache:
`Lantern.bulb` (`games/lanterns/logic/src/lib.rs:60`) is skipped by Data, while
`Session.actors` (line 75) makes `kinds::actors` return early and prevents rebinding.
After tick 1, the test despawns `lantern-1/bulb` and respawns it with the same name,
parent and rendering components but a new entity generation. Continuous execution
panics on the stale Bulb ID on tick 2; Save and FreshGame both reach tick 7 with
hash `0xf0d40911add2cd8b` and identical full saves. The explicit regression
`respawned_cached_child_matches_continuous_and_every_tick_restore` remains red;
game logic is unchanged, as requested. This is not an engine stale-ID bug: the
engine correctly refuses the invalid cached handle. Game caches need structural
invalidation or revalidation, not just initialization after load.

Follow-up verification after the EXPHYS correction (2026-09-18): **417 Rust
workspace tests passed, 1 failed, 11 ignored**. Only the new cached-child regression
fails; the original continuation comparison and every corrected pin pass.
Each of the three consumer-suite runs (environment `0`, `1`, `fresh-game`) reports
**29 passed, 1 failed, 1 ignored**, with that same game-side failure. The physics
suite reports **35 passed, 0 failed, 1 ignored**. `pile --verify` passes in all
three environments, including the two-body every-step reconstruction comparison.

| Linux proof, after correction | Normal | Save | FreshGame |
| --- | --- | --- | --- |
| Greybox | 62/0 | 63/0 | 63/0 |
| Beacons | 54/0 | 55/0 | 55/0 |
| Lanterns | 8/0 | 9/0 | 9/0 |
| Asset fixture | 6/0 | 7/0 | 7/0 |
| Cubes | 3/0 | 4/0 | 4/0 |

All **15 proof runs / 409 assertions** pass. The proof scripts regenerated their
ignored artifacts; no committed physics checkpoint exists in this clone. Clippy
`--workspace --all-targets -- -D warnings`, formatting, caps and boot pass.
`cd game && bun test` remains **38 passed / 2 environmental failures** (Chrome and
60/120 Hz feel bakes unavailable). GPU pixels, browser and Apple execution are
unverified here. No further engine divergence surfaced after the Rapier fix.
Root Cargo metadata confirms that no game or Rapier package entered its workspace.
All vendored Rust files also meet the 1,500-line task cap, independently of caps'
vendor exemption. The only upstream Rust changes are the serialization fix and
three same-module file splits; no dependency-cache source was modified.

Initial T4 verification, before EXPHYS v2 (2026-09-18): the game workspace reported **415 passed, 1 failed,
11 ignored**. The failure is the newly added Lanterns normal/paranoid comparison;
all pre-existing pins still pass normally. Consumer suites report **28/1** with
normal defaults and **27/2** with each paranoid environment setting (one ignored
in each): the second failure is Lanterns' existing pinned-hash test. Clippy with
`-D warnings` and formatting pass. `cd game && bun test` reports **38/2**, the same
missing Chrome and missing 60/120 Hz feel-bake prerequisites documented below.

| Linux proof assertions | Normal | Save | FreshGame |
| --- | --- | --- | --- |
| Greybox | 62/0 | 63/0 | 63/0 |
| Beacons | 54/0 | 55/0 | 55/0 |
| Lanterns | 8/0 | 8/1 | 8/1 |
| Asset fixture | 6/0 | 7/0 | 7/0 |
| Cubes | 3/0 | 4/0 | 4/0 |

Counts are pass/fail; paranoid adds the final comparison. Cubes' new Linux branch
executes one tick with the existing 100,000-cube manifest; its simulation test
uses 100 cubes and 120 ticks. No GPU, browser or Apple runtime was verified.

Initial T4 median wall time over three scripted simulation runs, optimized dev/test profile,
excluding construction and compilation (shared CPU; these are diagnostic numbers):

| Script | Normal ms | Save ms / slowdown | FreshGame ms / slowdown |
| --- | ---: | ---: | ---: |
| Greybox, 120 ticks | 0.298 | 28.578 / 95.9× | 27.766 / 93.2× |
| Beacons, 120 ticks | 0.254 | 73.124 / 287.9× | 78.754 / 310.1× |
| Lanterns, 120 ticks (diverges) | 5.935 | 503.717 / 84.9× | 506.211 / 85.3× |
| Asset fixture, 60 ticks | 0.060 | 5.417 / 90.3× | 7.868 / 131.1× |
| Cubes, 120 ticks | 1.980 | 110.037 / 55.6× | 109.738 / 55.4× |
| Dynamic physics stack, 120 ticks | 1.428 | 52.801 / 37.0× | 51.952 / 36.4× |

This intentionally expensive mode is a test instrument, never the default.

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
T2 extends this instrument with the production three-way merge and reload report.
The test binary compiles
independent copies of the game with explicit source substitutions; shipped Lanterns
is unchanged. `games/lanterns/fixtures/difficult-moment.script.json` uses the
existing Sim key and clock verbs from tick zero, including scheduled key edges.
`difficult-moment.sim` is its EXSIM v6 save, including the tick-zero base; the adjacent JSON describes the
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
[scene/README.md](scene/README.md). A scene edit rebakes content; Continue merges
changed authored fields into existing entities and reports deferred structural edits.
Restart constructs the entire new scene. Source links refuse a stale digest or
changed source file.


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


## Five-lane integration (M3, 2026-09-18)

T2, T4, T1b2, T3b2 and I1 now coexist. The unchanged Lanterns 180-tick restore
regression passes with EXPHYS v2, confirming the incomplete Rapier snapshot
hypothesis without a typed-kind workaround. Paranoid reconstruction preserves
the live phase, display period, batch horizon and T2 reload report; I3 asserts
that report across the entire continuation in Off, Save and FreshGame.

Saved child IDs intentionally refuse replacement rather than silently rebinding.
The inherited cached-child regression now requires the same named generation
refusal in all three modes, alongside T3b2's unchanged live/restored byte/error
parity regression. No position or gameplay assertion was removed.

The combined tick 0/60/180 hashes are `a778d065d6cea372`,
`8d712ef8ea7aa587`, `264947d99e722167`; all three modes agree on complete saves.
I3's own `EXACT_I3_RECORD=1` test regenerated both EXSIM snapshots, moment JSON
and 605 continuation rows. Both paranoid modes then compared those exact files
without recording. The 612-entry binding-hash inventory reflects both changes.

Merge decisions:

| Lane | Conflict resolution |
| --- | --- |
| T2 | Clean merge; retain EXSIM v6 authored baseline, base arguments and bounded reports. |
| T4 | Combine `Sim` paranoid reconstruction with F2d live clocks and T2 saves; move its helper to `sim/paranoid.rs` to meet the source cap. |
| T1b2 | Only `physics/README.md` conflicts; retain all unfavorable-case evidence and use the EXPHYS v2 pile hash. |
| T3b2 | Combine Beacons tests and the three-mode Lanterns pin test; regenerate conflicted I3 binaries/JSONL by execution. Keep saved-ID refusal semantics and every parity assertion. |
| I1 | Clean additive merge; no trial model sessions launched. |

The existing `tick_10k_median` measured **34.906 µs/tick before T3b2**
(commit `03e76d5`) and **153.571 µs after**, a **4.40× regression** in the
same optimized dev/test profile with zero debug info and incremental disabled.
Each is the median of five independent 10,000-tick runs, W held, sound enabled.
An isolated, reverted diagnostic replaced `prepare_kind`'s all-entity
`K::check` scan with the existing shared query's matching rows, retaining every
matching row's binding validation. It measured **56.237 µs/tick**. Thus roughly
**97.3 µs/tick (82% of the increase)** comes from scanning nonmembers and
constructing their missing-component diagnostics. Lanterns validates Lamp rows
in updating and publishing each tick. This probe changed neither observation
nor the game's shared/mutable row choices. It is attribution, not a shipped
optimization; the remaining 21.3 µs above the pre-merge sample is not separately
attributed. The performance follow-up is recorded in `QUEUE.md`.


A one-bit negative control confirms causality: temporarily restoring Rapier's
`serde(skip)` on `deferred_optimize_pending` makes the unchanged Lanterns test
fail at loop index 1 (tick 2); restoring only serialization makes it pass again.
The temporary edit is reverted. EXPHYS v1 refusal remains active.

Final verification: **524 game Rust tests pass, 0 fail, 21 ignored diagnostics**;
the affected core (`exact-web`, `exact-gpu`, `exact-runner`, `exact-motion`,
`exact-linux`, `exact-apple`) builds and passes **261/0, 1 ignored**. Game and
affected-core Clippy pass with `-D warnings`; both workspaces pass formatting.
Standalone web fixtures pass **68/0**, with two real-Bridge cases additionally
run through `exact-web`. Game Bun is **55 passed, 2 environmental failures**:
no Chrome for generated-game web launch and no prebuilt feel artifacts. The
generated game builds and its three Rust tests pass. One extra Bun failure was
the audio test's source extractor missing T4's parenthesized benchmark-aware
filter; the updated test executes that condition and retains both exclusion
assertions, adding own-benchmark-source inclusion and probe-exclusion controls.

| Linux proof | Off | Save | FreshGame |
| --- | ---: | ---: | ---: |
| Beacons | 54/0 | 55/0 | 55/0 |
| Greybox | 62/0 | 63/0 | 63/0 |
| Lanterns | 8/0 | 9/0 | 9/0 |
| Asset fixture | 7/0 | 8/0 | 8/0 |

All **12 runs / 401 assertions pass**. Each paranoid proof also compares every
session's final hash, tick, publications and retained journal with Off.
I3 separately passes its complete fixture comparison in all three modes.
The inherited 200k interleaved/churn kind test passes in the workspace suite.
The additional ignored T2 200k reversed-order/churn reload diagnostic passes
every entity assertion: **3.288 s merge**, **1,008,756 KiB peak RSS**, optimized
test profile (a diagnostic, not a constant-work or 60 Hz claim).

I1's seven JavaScript modules parse, and its real Linux adapter passes eight
additional smoke assertions for input, exact save/load, held-key release,
explicit 216,000-tick/unknown-operation refusals and unavailable world pixels.
Only the scratch-root literal was redirected into M3's allowed directory; no
trial models or full candidate evaluations were launched.

Caps and boot pass; boot remains **two JavaScript modules and one Wasm reference**.
Full root build/test/Clippy were attempted and remain blocked by TypeScript
app bakes requiring the absent lean Hermes executor. GPU pixels, browser
execution, physical audio and Apple SDK/runtime behavior remain unverified;
device-dependent tests can return early. The game remains a separate workspace.
All commands, regeneration/diagnostic probes and logs are retained under
`~/lanes/gamenext/scratch/M3/`. No pushes or remote commands were used.
