# exact-game-physics

Vendored Rapier 0.35.3 behind Exact's saved components, in metres, kilograms and seconds.
Call `register` during setup, `physics::capsule(w, "player").step(velocity)` after
controls, and `physics::step` per tick. The capsule component is `CapsuleController`;
it can coexist with the core flat-ground `Character`. The step result includes the
actual `displacement` and `grounded` state.

- Entity-ordered insertion maps `Body`, `Collider` and world `Transform` to Rapier;
  ordered handle maps and last-write comparisons detect edits, teleports and removal.
  Synchronization visits the union of Body/Collider membership, not every living entity.
  Dynamic poses, velocities and sleep return to components; kinematics use next pose.
- One step uses `world.dt()`. A collision-only pass after moving kinematics supplies
  same-tick sensor transitions. Events are sorted; `Announce` journals transitions.
  Sleeping bodies still step; seekable Sim observes Body/Transform changes. Support edits wake all.
- Density defaults to 1000 kg/m³; explicit mass is kg. Friction combines geometrically,
  restitution by maximum. Contact slop is 0.1 mm; other integration defaults are Rapier's.
- Shapes: sphere, box, Y capsule/cylinder (total height), static mesh and heightfield
  (row-major, rows Z/columns X). Curved shapes need uniform positive scale.
  Bodies are roots; parented boxes must not acquire shear. Sweeps translate convex shapes.
- `let q = physics::queries(world); q.raycast(..); q.sweep(..);` shares a lazy
  query scene retained in the world's physics executor, outside Data and hashes.
  Drop the scope before structural edits or `step`; the next scope reuses it.
  Body/Collider/Transform/Parent write revisions (including membership and load)
  invalidate it, so same-tick edits are visible on the next operation. Unchanged
  queries/ticks do not rebuild. Static and body dirtiness are tracked separately;
  retained geometry uses a canonical traversal certificate, with a full rebuild
  when its partition cannot be certified. Bodies sharing pages with statics or
  ancestors do not alone force static reflection. Membership changes and the
  worst relevant edits still cost a full canonical rebuild.
  The free query functions are thin one-shot calls through this same cache.
  The capsule handle uses the shared scene. Capsules use Rapier's steps/slopes/snap, saved-pose platform transport and an
  80 kg default push budget. Movement and push share the layer-mask/sensor/self filter;
  the character's rigid collider is a sensor.

Physics admits at most 200,000 participating entities (the union of Body and
Collider, counting each entity once). Solver reflection, queries and controller
geometry refuse larger worlds before construction. Retained memory and worst
rebuild work are O(total physics geometry), including all mesh/terrain samples;
query traversal has no separate hard visit budget. This is not a constant-time
claim for edits or character movement.

EXPHYS v2 persists `BroadPhaseBvh::deferred_optimize_pending`. V1 omitted state
that changes the next physics step; it cannot be migrated and is refused by name
before replacing the destination (including inside EXSIM). Start a new world.
Rapier is consumed by path from `vendor/rapier3d`; it stays outside the root
workspace's dependency graph.

Saved state is opaque bincode/serde for bodies, colliders, islands, broad/narrow phase,
joints and integration parameters, plus entity/handle maps and last writes. Restore validates and decodes live state atomically; pipeline/CCD workspaces are scratch under Rapier's serialization contract.
`Data::write(&self)` refreshes dirty bytes for save, hash and JSON; `refresh_snapshot`
measures the same operation. Stepping does not serialize. JSON summarizes the opaque bytes by length/hash.
Malformed or obsolete Rapier payloads fail during `World::load`, before replacement.

Continuation tests preserve the complete authoritative snapshot; `Executor::clone`
refreshes and decodes that same representation. There is no parallel field-by-field
Rapier clone. See [state.rs](src/state.rs), [continuation tests](tests/paranoid.rs)
and [the pile diagnostic](examples/pile.rs).

EXPHYS v2 parity has native arm64 and browser fixture evidence. The x86-64 v2
card and a non-initial `deferred_optimize_pending = true` continuation remain owed;
earlier v1 results do not establish either boundary.

Reproduce from `game/` with `EXACT_UPDATE_TRUST=development`:
```
cargo test -p exact-game-physics --no-fail-fast
cargo test -p exact-game
cargo clippy -p exact-game-physics --all-targets -- -D warnings
cargo fmt --all -- --check
cargo run -p exact-game-physics --release --example pile -- 1000 2000 5000
cargo run -p exact-game-physics --release --example pile -- --verify
cargo build -p exact-game-physics --profile web --target wasm32-unknown-unknown --example minimal
```

`Collider::of(&mesh)` matches each dimensioned primitive. A plane makes a static
1 cm slab with its top face at Y=0; assets require authored collision geometry.

The kinematic pending-flag parity pin remains owed. R5 tried a 256-static/32-kinematic
scene with per-body vertical movement and a 512-static/64-kinematic scene moved
horizontally after the initial tick. Both ran 32 ticks through the collision pipeline
with matching Off/Save/FreshGame saves, but neither left `deferred_optimize_pending`
true at a non-initial save boundary. The ignored diagnostic was deleted after those
two attempts; the dynamic-only pins above do not prove this boundary. A replacement
must reach the true flag, then show parity and divergence when the serialized flag
is cleared.
