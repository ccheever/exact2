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
Every row access validates generation and Parent; required component membership
is checked once per kind/entity generation and cached in the owning world. Removing
a component invalidates that slot’s proofs; despawn changes its generation; load
starts with empty proofs. Decoded or cross-world IDs therefore check on first use;
a stale ID fails identically live and restored, naming the owning field. Tick
paths never resolve or rebuild child bindings. Actor IDs are likewise saved in
Session and resolved during setup. `Id<K>: Data` has exactly Entity's encoding.
`edit_resource::<Session, _>(f)` releases the resource lease at return.

Work bounds: at most eight components and eight direct child checks per row;
child checks do not recursively traverse bindings. A single bind uses indexed
name lookup. Iteration and singleton search scan at most 200,000 entity slots,
including dead slots, and explicitly refuse larger worlds before acquiring
leases. Bound-ID proofs have the same 200,000-slot bound and at most 64 kind
types per world; exceeding either refuses explicitly. Edit nesting is bounded
to 32 operations, with an explicit refusal before acquiring editing leases.
Child-bearing iterators validate the matching rows before mutable leases;
both passes use the existing query presence-mask intersection, visiting only
members in ascending entity order. Successful iteration and warmed ID access
allocate nothing; diagnostic text is produced only on failure. TypeId uniqueness
takes at most 28 comparisons. The 200k interleaved/churn regression checks every returned row and
the over-limit error; it cannot pass with an empty iterator.

`Target::entity(&self, world: &World) -> Option<Entity>` borrows its target so
failed typed boundaries can describe it lazily, without formatting on success.

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
  swaps. `Game::validate` runs before construction, binding, seeking for a bind, or restore; a refusal changes nothing. Hosts construct with `Sim::from_values`. Saves encode argument fields by name: reordering is safe, additions default, removals are ignored. Saves carry the game's `ID` and world time and dynamic input. `Sim::restore(&[u8])` and
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

Observation caches per-instance digests in 64-slot mask words. A separate dirty
bit per slot records component writes; the public 1,024-slot page generation
used by the renderer is unchanged. `get_mut` dirties its slot even if unwritten;
mutable queries dirty only yielded slots. An empty mutable query invalidates the
world observation epoch but hashes no values. Changed membership or Ambient masks
rebuild at most one 64-slot word per changed word. Resources and RNG cache their
digests until a mutable lease or replacement. A retained exclusive guard refuses
observation, even on a cache hit. Skip-only changes report Still, like the oracle.
Existence pages track membership/incarnations; propagation marks global-pose pages
when a descendant's pose or availability changes, including Ambient ancestors.
Observation reads the last propagated pose.

Caches are derived state: neither saved nor hashed, discarded by in-place `read`,
load and world identity/presentation changes. Empty component pages immediately
release digest capacity; structure caches release empty pages at the next sample.
Trailing empty caches are truncated. Cached rows go directly into the sorted
output; equal vectors return Still immediately, preserving the eight reasons and
Ambient/AMBIENT rules. Public API signatures and deterministic pins are unchanged.
`World::hash` and its streaming definition remain unchanged. Fused observation/hash
still streams every component into that hash, even when observation digests hit.

Work is O(storage mask words + output entries + dirty Data bytes); changed observed
membership can hash up to 64 values per word. Propagated globals retain page-level
invalidation. The unfavorable fully dirty case hashes all observed values. Each
sample explicitly panics before allocating/hashing beyond 1,000,000 entity slots
(including dead slots), 256 component/resource types, or 16,000,000 entry visits
(conservatively two per slot plus storage memberships). Custom Data writers remain
trusted, as for save/hash; this is a visit bound, not a byte/time bound. Comparison
is O(entries), with at most eight reasons; settle remains bounded to sixteen rounds
and retains its canonical streaming/hash cost.

Diagnostics (release profile, from the repository root):

```sh
cargo test --manifest-path game/Cargo.toml -p exact-game --release --lib stillness_hash_cost -- --ignored --nocapture
cargo test --manifest-path game/Cargo.toml -p exact-game --release --lib settle_200k_cost -- --ignored --nocapture
```

The observation benchmark includes 1k/10k/200k still worlds and 200k entities with
100 writes clustered in one page or spread over 100 pages, one write in every
64-slot word, and all 200k slots dirty. It separates observation,
canonical hash, fused observation/hash and the seek pair. The extended cases use
100 samples after five warmups; single-operation timings exclude writes. Settle
measures an initial Unknown world, then 100 samples after five warmups, invalidating
each sample with an unwritten Transform lease so it must actually observe rest.
Both 100-mover shapes now hash exactly 100 values; every-word hashes 3,125
values and all-slots hashes 200,000. Timings are diagnostics, not CI gates.

T1a2 measurements on shared x86-64 Linux (EPYC 9454), with 200k Transform
entities; milliseconds are median / p95. T1a is the preceding page-cache result.

| Operation | T1a | T1a2 |
| --- | ---: | ---: |
| Observe, still | 0.758 / 0.786 | 0.746 / 0.811 |
| Observe, 100 clustered movers | 0.960 / 0.976 | 1.035 / 1.090 |
| Observe, 100 spread movers | 21.210 / 21.383 | 0.797 / 0.813 |
| Observe, every word dirty (3,125 movers) | — | 1.346 / 1.366 |
| Observe, all 200k slots dirty | — | 39.440 / 40.435 |
| Settle after an unwritten lease | 66.954 / 67.779 | 63.276 / 65.138 |

Spread meets the ≤2 ms target. The fully dirty case still serializes every value;
there is no sparse-work saving to claim there. The small clustered difference is
reported rather than hidden. Still 1k/10k samples measured 0.00290 / 0.00293 ms
and 0.0302 / 0.0330 ms. Initial settle was 119.479 ms (T1a: 126.409 ms),
excluding construction; canonical hashing still dominates settle. Samples use the release profile with reduced debug info
and incremental compilation disabled; other lanes share this host.

The original uncached path remains a test-only oracle. Three seeded scripts
compare 2,304 samples across 576 ticks, before/after propagation and with/without
the fused hash. Coverage counts only actions that change bytes, replacement
context, or the deliberate unwritten-lease epoch. A deterministic API table first
asserts each mutation, then compares cached/oracle entries, verdicts, reasons and
canonical hashes. It covers replacement/removal, resource replacement, reseed,
retained guards, teleport/followers, consecutive Transform/Body write-back using
the production Body schema, skipped fields, empty queries, rejected load and a
higher-index parent. Multi-page optional/owned queries, in-place reads and full
Sim restore/carry/paranoid reconstruction have dedicated cases.

The 200k write-count regression checks spread, every-word and fully dirty work,
plus partial owning iteration; the capacity regression empties an interior page
before the trailing pages and checks reallocation after recycling. Disabling slot
invalidation makes the write-count negative control fail (0 hashes versus 1).
Returning no entries fails both oracle equality and explicit changed/nonempty
assertions. No cache optimization is inferred from timing alone.

T1a2 verification after fetching/merging local trunk
`77ef8d2c86cf5757818e44f960f6cf5ccfb09c09` (already an ancestor): **531 game
Rust tests passed, 0 failed, 21 ignored**; workspace Clippy with `-D warnings`,
game/root formatting, caps and boot pass. All five Linux proofs pass with
`--paranoid`: **15 runs / 412 assertions**, including identical final hashes,
ticks, publications and journals. Off / Save / FreshGame assertion counts are
Beacons 54/55/55, Greybox 62/63/63, Lanterns 8/9/9, Asset Fixture 7/8/8,
and Cubes 3/4/4. Existing position/hash pins and fixture files are unchanged.

Bun reports **55 passed, 2 environmental failures**: absent Chrome for the generated
game web proof and absent feel web bakes. That generated game's three Rust tests
pass. Full root build/test/Clippy remain blocked by the absent lean Hermes
executor. A stale ignored Asset Fixture model without its bake manifest initially
blocked compilation; moving it to task scratch let the normal bake regenerate the
unchanged 2,190-byte model and 1,475-byte texture. No GPU pixels, physical audio,
Chrome or Apple execution was verified; device-dependent tests can return early.
Commands, negative-control output and measurements are retained in
`~/lanes/gamenext/scratch/T1a2/`.

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
agent hears. Capture the complete simulation with `s.world('world').save('run.world')`
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
room containing the camera cannot hide its interior. Picking still returns exits.
Rays use the same primitive intersections and declared model boxes as picking. Undeclared models without authored
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
The unprunable case, 200,000 overlapping on-ray meshes, explicitly
refuses at **1,000,000 visits**, **102.424 ms cold / 35.775 ms warm**; the same
scene's boolean LOS stops in fewer than 64 visits. These are diagnostic timings,
not a frame-time guarantee. The slot-limit test includes despawned slots.

A temporary negative control returning zero occlusion and no occluders failed
three engine tests and the viewport-bearing WorldSurface snapshot test; restoring
the implementation passes those tests. Linux preserves the engine's geometric
visibility reply even without a GPU; Greybox opens an 800×600 headless session
and compares the complete unchanged native layout snapshot, including screen
coordinates. Only the host's transport epoch/incarnation/clock are outside that
engine reply.

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
Asset declarations keep EXGAME v3; simulation saves now use T2's EXSIM v6. The baker refuses
textures above 2048×2048 and files above the 64 MiB carrier limit. It traverses
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

`gpu_period(ms: f64)` supplies `Frame::period_ms: f64` to every surface;
`Sim::frame_period(&mut self, period_ms: f64)` applies it on the next accepted
live advance. Web and Apple hosts forward their measured display period;
headless Linux leaves it unknown (zero).

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
The generated game demonstrates nearby prompts, beacon plinths, and `restart_generation` as the
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

EXCAP v2 hashes the checkpoint at the same exact tick deadline as EXSIM saves,
then retains the fixed-size live scheduler state (fractional phase, lookahead,
display period and host epoch) separately. Every frame records its original
floating-point timestamp and period; scheduled and delivered-device inputs retain
their distinct timing. Ordinary saves remain independent of presentation timing.
EXCAP v1 is refused explicitly; regenerate development captures with their scripts.
The checkpoint and ordered input, live bindings, viewport changes and clock
advances reproduce the simulation. Defaults are 2 MiB, 4,096 records and
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


## K1: typed-kind hot paths (2026-09-18)

`World::rows<K>(&self) -> KindRows<'_, K>` and
`World::rows_mut<K>(&self) -> KindRowsMut<'_, K>` retain their signatures and
ascending entity order. Membership uses the raw query's presence-mask join.
Child-bearing joins validate only matching rows, using the references already
selected by that join, before taking mutable leases. Prepared query columns also
serve lease preflight, eliminating duplicate storage lookups. Thin row adapters
and static kind preflight are inlineable. `the::<K>()` uses the same masks without
leasing values, preserving structural lookup during an edit.

The remaining regression came from eager target descriptions and edit-context
strings, repeated component-set validation, redundant owner-component fetches,
column lookups and row wrappers. Context records now hold static strings and an
entity; formatting happens only on errors. World-local proofs cache each kind's
validated entity generations. Removing a component invalidates that slot's proofs;
despawn increments the entity generation; load replaces the cache. Decoded and
cross-world IDs validate in their destination world before caching. Parent edges
are still checked on every child use. Id encoding, saved state and pins are unchanged.
The public `Target::entity(&self, world: &World) -> Option<Entity>` now borrows
its target so error descriptions can be lazy; gameplay row/edit APIs are unchanged.

For N slots, M matches and k required columns, a join costs O(k ceil(N/64) + kM),
with one extra shared pass for child-bearing kinds and at most eight direct child
checks per match. The explicit limits remain 200,000 slots and eight fields/children;
proof caches additionally refuse more than 64 kinds and edit contexts more than
32 nested operations. First-use proof storage can initialize up to 200,000 entries;
component removal invalidates at most 64 cached entries. Steady ID validation is
an indexed generation comparison after a bounded kind lookup, not a component join.

Existing `tick_10k_median`, median of five fresh 10,000-tick runs, W held, with
Lanterns sound enabled; identical optimized dev/test profiles, debug info and
incremental compilation disabled. Historical sources at `03e76d5` were measured
in this clone with the same timing tests, then restored before verification.

| Game (µs/tick) | Before regression (`03e76d5`) | Entry trunk (`77ef8d2`) | K1 |
| --- | ---: | ---: | ---: |
| Lanterns | 35.365 | 154.615 | **36.988** |
| Beacons | 0.919 | 1.024 | **0.888** |

The 200k-slot benchmarks have 1% members interleaved with nonmembers, slot churn,
exact nonempty checksums, and five alternating samples of 1,000 passes each:

| Join (µs/pass) | Raw query | Typed rows_mut | Overhead |
| --- | ---: | ---: | ---: |
| Three columns, including optional material | 75.985 | 60.020 | −21.01% |
| Saved child, equivalent validation | 88.023 | 88.322 | +0.34% |

The child-bearing raw control performs the same generation/Parent validation in
a shared prepass before its mutable query. Separate assertions require zero
allocations in idle kind paths and both sparse cases. Removing the final child's
required component refuses before any parent page is dirtied. Temporarily restoring
the all-entity diagnostic scan makes the sparse-child allocation assertion fail
with **1,383,902 allocations**, versus zero; the probe was reverted.

Final verification: **530 game Rust tests passed, 0 failed, 23 ignored diagnostics**;
both ignored sparse timings also pass. Game Clippy with `-D warnings`, both
workspaces' formatting, caps and boot pass (two boot JS modules, one Wasm reference).
Linux proofs in Off / Save / FreshGame pass Beacons **54/55/55**, Greybox
**62/63/63**, Lanterns **8/9/9**, and Asset Fixture **7/8/8**: **401 assertions**,
with all pins and complete continuation saves unchanged. Game Bun is **55 passed,
2 environmental failures**: absent Chrome and absent prebuilt feel artifacts.
Root build/test/Clippy were run and remain blocked by the missing lean Hermes
executor for TypeScript app bakes. GPU pixels, browser execution, Apple runtime
and physical audio remain unverified on this headless Linux host.

Ran `git fetch ../exact2-next next/trunk && git merge FETCH_HEAD` against
**77ef8d2c86cf5757818e44f960f6cf5ccfb09c09**; it was already an ancestor, so no
merge commit or conflict resolution was needed. Full verification above ran after
that merge. Commands, complete sample arrays, negative control and verification
logs are retained under `~/lanes/gamenext/scratch/K1/`.


## Geometric eyes follow-up (T5b, 2026-09-18)

`Sim::agent(&mut self, request: &str) -> String` and the layout request signature
are unchanged. Layout now has bounded mesh visibility work, symmetric endpoint
hierarchy exclusions, explicit unavailable geometry, canonical rounded numbers
and quantized occluder ordering. The API rules and 200k measurements are above.
Model bounds were already validated as finite and ordered on merged trunk; the
new asset-load regression proves rejection on each inverted axis. Authored
ModelBounds remain deterministic geometry even before cosmetic model delivery.

The required local fetch/merge of `next/trunk` resolved to
`77ef8d2c86cf5757818e44f960f6cf5ccfb09c09`, already an ancestor of this branch.
After that merge check, **533 game Rust tests passed, 0 failed, 21 diagnostics
ignored**. Game workspace Clippy (`-D warnings`) and formatting pass. The affected
Linux host additionally passes **15 unit tests** and Clippy. Root formatting,
staged caps and boot pass; boot retains two JavaScript modules and one Wasm.

| Linux proof | Off | Save | FreshGame |
| --- | ---: | ---: | ---: |
| Greybox | 63/0 | 64/0 | 64/0 |
| Beacons | 54/0 | 55/0 | 55/0 |
| Lanterns | 8/0 | 9/0 | 9/0 |
| Asset fixture | 7/0 | 8/0 | 8/0 |
| Cubes | 3/0 | 4/0 | 4/0 |

All **15 runs / 415 assertions** pass. No pinned positions, hashes or committed
snapshot files changed. The forced-empty occlusion negative control failed four
tests, including the viewport-bearing WorldSurface test; production code is restored.
Bun remains **55 passed / 2 environmental failures** (missing Chrome and prebuilt
feel bakes). Full root build/test/Clippy cannot bake TypeScript apps without the
lean Hermes executor. GPU pixels, browser/Wasm execution and Apple runtime are
unverified on this host; device-dependent Rust tests may return early.
The initial game build refused a stale ignored asset-fixture `crate.model` without
its generation manifest; it was preserved in task scratch and regenerated from
tracked art. Logs and that preserved artifact are in `~/lanes/gamenext/scratch/T5b/`.


## Ready, and nothing after it

`state world` adds `ready`, `readyReasons`, and `gpu`. Ready means all declared
assets (including their textures) are Loaded or Failed with a reason, a renderer
exists for the current surface format, and its first frame has been presented.
Headless uses the same Feed with recording Writes and its first completed feed;
`device:false` means no shaders, pipelines or attachment textures were exercised.
`Lifecycle::Presented` is sent after the host submits a frame for presentation;
it is not a physical-display scanout measurement. Device loss makes ready false.

The exact shape below shows a newly bound headless world with one cube (values
vary with content). Both phases have the same eleven allocation/upload counters;
`violations` counts after-ready work events, not bytes. Exceptions have bounded
aggregate counters and only their last named event, never an unbounded log.

```json
{
  "ready": true,
  "readyReasons": [],
  "gpu": {
    "device": false,
    "beforeReady": {
      "shaderModules": 0, "renderPipelines": 0, "computePipelines": 0,
      "bindGroupLayouts": 0, "textures": 0, "textureBytesUploaded": 0,
      "vertexBuffers": 1, "indexBuffers": 1, "meshBytesUploaded": 1104,
      "bufferReallocations": 3, "slotCapacityGrowth": 1,
      "streamingVertexBytes": 0
    },
    "afterReady": {
      "shaderModules": 0, "renderPipelines": 0, "computePipelines": 0,
      "bindGroupLayouts": 0, "textures": 0, "textureBytesUploaded": 0,
      "vertexBuffers": 0, "indexBuffers": 0, "meshBytesUploaded": 0,
      "bufferReallocations": 0, "slotCapacityGrowth": 0,
      "streamingVertexBytes": 0,
      "violations": 0,
      "declaredExceptions": {
        "events": 0,
        "counts": {
          "shaderModules": 0, "renderPipelines": 0, "computePipelines": 0,
          "bindGroupLayouts": 0, "textures": 0, "textureBytesUploaded": 0,
          "vertexBuffers": 0, "indexBuffers": 0, "meshBytesUploaded": 0,
          "bufferReallocations": 0, "slotCapacityGrowth": 0
        },
        "last": null
      }
    },
    "lastAfterReady": null
  }
}
```

A violation replaces `lastAfterReady` with `{ "what": "meshBytesUploaded",
"name": "Sphere", "tick": 1 }`. Undeclared `Mesh::asset` delivery is the existing
pop-in exception: preparation/upload work goes to `declaredExceptions`, with the
asset name in `last`. Growing world/instance buffers remains a violation.
`vertexBuffers`/`indexBuffers` count actual shared arena creation, including growth;
`meshBytesUploaded` counts initial geometry payloads. Existing animation writes
into retained vertex ranges are `streamingVertexBytes`, separately visible like
ordinary transform/material updates; they create no asset or capacity and are
not violations. This distinction is necessary for the existing animated Fox.

Accounting is always on, independent of perf sampling, and never reset by perf
reset, restore, visibility, format change or device recovery. Work after the first
ready stays after-ready even while readiness is temporarily false. It uses fixed
counter storage and one last event per category, O(1) per allocation/upload.
The headless feed records the same geometric payload lengths and power-of-two
arena growth without retaining duplicate payloads; it refuses entity/draw slots
past 200,000 explicitly. Feed work remains O(entity slots + changed geometry),
asset validation retains its existing name/payload/texture limits, and device
feeds retain their granted buffer limits. These are work bounds, not a promise
that arbitrarily growing a world meets a frame deadline.

Beacons, Greybox, Lanterns and the asset fixture assert ready and zero violations
at session close in their standard proofs, printing counters and the last event.
The negative control spawns a never-seen sphere after ready and must trip it.
The unfavorable 256→2,560 entity test must report buffer and slot growth, and
checks explicit refusal past the headless limit. No asset-loader or pipeline
preparation policy is changed by the accounting.

T6 measurement on Linux: Lanterns before ready creates four vertex and four index
buffers, uploads 103,624 mesh bytes and one 4,194,304-byte texture, and records four
buffer reallocations/two slot growth events. Off has **zero after-ready violations**.
Its retained animation streams 69,120 bytes before and 138,240 after ready.
Save and FreshGame each expose **eight violations**: two Fox uploads add 152,064
mesh bytes, two textures/8,388,608 texture bytes and one new vertex/index buffer.
`lastAfterReady` is `{"what":"meshBytesUploaded","name":"Fox.glb","tick":60}`.
Paranoid restore invalidates the feed's embedded asset instances; this is a real
reported upload, not an allowed undeclared-asset exception. These two proof runs were red in T6; M5 applies the explicitly agreed diagnostic
exception below pending the behavioral fix recorded in `QUEUE.md`. All shader/pipeline/layout counters are zero in
these headless measurements; they do not establish device behavior.


## Four-lane integration (M5, 2026-09-18)

K1, T1a2, T5b and T6 merge in that order, one merge commit each. README lane
reports and every test survive; the surface-test conflict keeps both the viewport
occlusion fixture and the device presentation/pipeline negative control. K1's
query leases retain T1a2's per-slot observation invalidation. The resolved M3
performance queue item is removed. No save, position or hash pin changes.

`state.world.gpu.restoreUploads` adds `{events, counts, last}` using the same
allocation/upload counter keys. It is an inclusive subset of `afterReady` and
`violations`, not a reset or a declared cosmetic exception. Only embedded assets
with the same entity generation and name in the preceding feed are attributed
when a presentation-generation reset forces them to upload again. New geometry,
new entities and ordinary buffer growth remain violations. Attribution reuses the
old bounded instance map for that feed; it adds no entity scan or unbounded log.

The proof's zero-after-ready rule remains strict in normal mode. In Save and
FreshGame only restore-upload events are subtracted for the assertion, and the
proof prints a REPORT with their complete counters and last event. Readiness and
all unrelated hitch assertions still apply. The ignored regression
`restoring_fox_uploads_zero_asset_bytes_after_ready` requires a normal restore to
upload zero mesh plus texture bytes; its comment points to the engine asset-path
follow-up in `QUEUE.md`. This integration does not fix or conceal that defect.


M5 verification: **553 game Rust tests pass, 0 fail, 24 ignored** (including the
new known-defect regression); affected core builds and passes **261/0, 1 ignored**.
Game and affected-core Clippy pass with `-D warnings`, and both workspaces pass
formatting. The initial new-fixture Clippy initializer warning was corrected;
its six focused tests pass again. Standalone web fixtures pass **68/0, 2 skipped**;
the two real-Bridge cases run through `exact-web`. Game Bun is **55/2**, exactly
the expected missing-Chrome and missing-feel-bakes failures; the generated game
builds and its three Rust tests pass. Full-root build/test/Clippy were attempted
and cannot bake TypeScript apps without the lean Hermes executor.

Linux proof counts in Off / Save / FreshGame are Beacons **57/58/58**, Greybox
**68/69/69**, Lanterns **9/10/10**, asset-fixture **9/10/10**, and cubes **3/4/4**:
**15 runs, 448 assertions, zero failures**. The cubes fixture has 100,002 entities.
Every paranoid final hash/tick/publication/journal comparison passes. Lanterns
Off reports zero after-ready violations; each paranoid mode reports eight restore
events, 152,064 mesh bytes and 8,388,608 texture bytes, still included in the total
violations. Explicitly running the ignored normal-restore regression fails with
**4,270,336 asset bytes** instead of zero, preserving the engine owner's reproducer.

Tick diagnostics each ran once as the existing five-fresh-simulation benchmark,
10,000 ticks per sample, W held, optimized dev/test profiles, debug info and
incremental compilation disabled. Lanterns retains sound. Results in µs/tick:

| Game | K1 lane median | M5 median | M5 range |
| --- | ---: | ---: | ---: |
| Lanterns | 36.988 | 37.231 | 37.190–37.358 |
| Beacons | 0.888 | 0.900 | 0.897–0.909 |

Observation and settle diagnostics each ran once in release, with 200k Transform
entities and 100 measured samples after five warmups. Values are ms median / p95:

| Operation | T1a2 lane | M5 |
| --- | ---: | ---: |
| Observe, still | 0.746 / 0.811 | 0.748 / 0.798 |
| Observe, 100 clustered movers | 1.035 / 1.090 | 0.774 / 0.791 |
| Observe, 100 spread movers | 0.797 / 0.813 | 0.784 / 0.801 |
| Observe, every word dirty (3,125 movers) | 1.346 / 1.366 | 1.459 / 1.474 |
| Observe, all 200k slots dirty | 39.440 / 40.435 | 39.840 / 40.171 |
| Settle after an unwritten lease | 63.276 / 65.138 | 69.761 / 70.700 |

Initial settle is 126.981 ms. Tick medians differ by +0.66% / +1.35%, and
still/spread/fully-dirty observation stays close to the lane samples. Every-word
observation is +8.4% and settle +10.2%; these single, unpaired shared-host runs
do not establish that those two differences are noise, or attribute them to a
merge interaction. The observation implementation and storage dirty tracking
are unchanged from T1a2; the diff of adjacent paths is retained with the logs.
A paired performance follow-up is in `QUEUE.md`; no full-parity claim is made.
The complete hash/fused/seek-pair timing rows and sample arrays are in scratch.

Caps and boot pass (two JavaScript modules, one Wasm reference). No GPU adapter,
Chrome or Apple SDK is available: GPU pixels/pipelines, browser execution,
physical presentation/audio and Swift runtime behavior remain unverified;
device-dependent Rust tests can return early. The game remains a separate Cargo
workspace. No remote commands or pushes were used; local bundles imported the
four lane heads. Commands, outputs and the known-defect failure are retained in
`~/lanes/gamenext/scratch/M5/`.

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


## M4 engine-line merge checkpoint (2026-09-18)

The engine line's `Surface::asset(name, Result<&[u8], AssetError>)`,
`bind(inputs, Option<f64>)`, `restore(bytes, Restore::{Carry, Open})`,
`Lifecycle::{Hidden, Visible, Interrupted, Resumed, Presented}` and
`Frame::period_ms` are integrated. `Sim::save()` returns `Result<Vec<u8>, DataError>`;
pending model dependencies refuse by name. EXSIM v6 retains trunk's authored
three-way merge and reload report; EXPHYS v2 retains its snapshot refusal.
`SAVE_VERSION` and migration hooks are removed. Restore requires exact tick/time
agreement. Capture schema identity now follows EXSIM v6 rather than a game migration
version.

The adapted engine-line paranoid scheduler is the single implementation, retaining
trunk's recorder, reload report and ownership. Shared raw storage retains trunk's
per-slot observation caches, cached kind validation and bounded load preflight.
The existing 200k sparse/churn and observation oracle tests pass. Layout keeps the
BVH, occlusion and unavailable-pose semantics; typed layout/pick use that geometry.
`Sim::input(event)` preserves scheduled stamps; `device_input(event)` marks actual
delivery so live pacing can clamp it. Both use the existing 1,024-event bounded
queue, and delivery status is saved and captured.

Baked skeletons use compact joint arrays, sim-side sockets and saved playback;
Lanterns retains its distinct `scene::Animation` embedded-Fox controller and existing
world pins. Its manifest opts into assets. Beacons keeps the authored scene and
saved typed kinds; both lines' movement/continuation assertions remain.

Checkpoint validation: game workspace compilation and **379 engine tests pass,
0 fail, 8 ignored diagnostics**. Full workspace, host proofs, capture-clock regression
and fixture regeneration follow in separate commits. No GPU/Apple/browser execution
is claimed on this machine.
