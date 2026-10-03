# exact-game-physics

Vendored Rapier 0.35.3 behind Exact's saved components, in metres, kilograms and seconds.
Call `register` during setup, `physics::capsule(w, "player").step(velocity)` after
controls, and `physics::step` per tick. The capsule component is `CapsuleController`;
it can coexist in one world with the core flat-ground `Character` on a different
entity; combining both controllers on one entity is refused. The step result includes
the actual `displacement` and `grounded` state.

- Entity-ordered insertion maps `Body`, `Collider` and world `Transform` to Rapier;
  ordered handle maps and last-write comparisons detect edits, teleports and removal.
  Synchronization visits the rows written since the previous step (`World::changed`),
  every Body and every parented collider; the first step after setup, restore or clone
  visits every row. A static collider without a Body costs nothing per tick.
  Dynamic poses, velocities and sleep return to components; kinematics use next pose.
- One step uses `world.dt()`. A collision-only pass after moving kinematics supplies
  same-tick sensor transitions. Events are sorted; `Announce` journals transitions.
  Sleeping bodies still step; any awake dynamic body reports busy to Sim, even
  when its pose is unchanged. Support edits wake all.
  Two fixed solids (colliders without a Body, or with a Static one) never pair, so a
  static world carries no contact state; a sensor pairs with anything, fixed or not.
- Density defaults to 1000 kg/m³; explicit mass is kg. Friction combines geometrically,
  restitution by maximum. Contact slop is 0.1 mm; other integration defaults are Rapier's.
- Shapes: sphere, box, Y capsule/cylinder (total height), static mesh and heightfield
  (row-major, rows Z/columns X). Curved shapes need uniform positive scale.
  Bodies are roots; parented boxes must not acquire shear. Sweeps translate convex shapes.
- `let q = physics::queries(world); q.raycast(..); q.sweep(..);` shares a lazy
  query scene retained in the world's physics executor, outside Data and hashes.
  Drop the scope before structural edits or `step`; the next scope reuses it.
  Each operation updates only the colliders whose Body/Collider/Transform/Parent rows
  were written since the last one, plus parented colliders (their pose follows their
  ancestors), so same-tick edits are visible. A write to a collider-free entity (a
  camera, a tracer) costs a membership test; loading or another world rebuilds.
  Capsule rays (also a capsule under an offset) use the closed form: Parry 0.30's
  support-map capsule raycast misses rays that pass straight through (64 of 16,000
  in RIVALS). Other shapes use Parry's ray cast.
  The free query functions are thin one-shot calls through this same cache.
  The capsule handle queries a BVH of only the colliders it can reach that step,
  built in entity order, so its result never depends on the scene's edit history. Capsules use Rapier's steps/slopes/snap, saved-pose platform transport and an
  80 kg default push budget. A default 1 m³ crate is 1,000 kg and cannot be pushed
  by that controller; author `Body { mass: 10., ..Default::default() }` for a light crate.
  Movement and push share the layer-mask/sensor/self filter;
  the character's rigid collider is a sensor, but other controllers are solid to its
  movement: characters block, slide around and push out of each other in call
  order. A character on a layer outside the mover's mask passes through.

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

EXPHYS v2 parity has native arm64, x86-64 Linux and browser fixture evidence.
On arm64 and x86-64,
`tests/continuation/mod.rs` reaches a non-initial `deferred_optimize_pending = true`
boundary at tick 6 through two bulk collision-only refreshes after a regular step.
Off/Save/FreshGame and restored continuation agree at tick 12; clearing only the
serialized pending flag changes the subsequent hash. Both continuations have consumed
the flag by then: the divergence is subsequent executor state, not a retained flag bit.
This proves the saved executor boundary, not that every kinematic scene reaches it.
The minimal-120 pin changed because awake work now enters the existing saved busy
reasons. Native arm64, x86-64 and Chrome Wasm agree in Off/Save/FreshGame; pile-600 is
unchanged. This legacy physics card keeps both values in [tests/pins.json](tests/pins.json)
rather than using the game prover's `--repin` command.
Both pins moved on 2026-10-03 when fixed solids stopped pairing: the snapshot lost
those pairs and each collider's flags changed. Every body pose, velocity and event
over 600 ticks of pile/stack/drop/bounce and 120 of minimal is byte-identical to the
previous build (native arm64); x86-64 and browser agreement on the new values is not
yet re-run.
The x86-64 v2 card passes with the existing pins and all 41 physics tests
(Rust 1.97.0, 2026-09-21). A 2,134,660-byte query/controller/save trace also
matches arm64 byte for byte. No pins changed for this verification.

Reproduce from `game/` with `EXACT_UPDATE_TRUST=development`:
```
cargo test -p exact-game-physics --no-fail-fast -- --nocapture
cargo test -p exact-game
cargo clippy -p exact-game-physics --all-targets -- -D warnings
cargo fmt --all -- --check
cargo run -p exact-game-physics --release --example pile -- 1000 2000 5000
cargo run -p exact-game-physics --release --example pile -- --verify
cargo build -p exact-game-physics --profile web --target wasm32-unknown-unknown --example minimal
```

`Collider::of(&mesh)` matches each dimensioned primitive. A plane makes a static
1 cm slab with its top face at Y=0; assets require authored collision geometry.

The copyable controller composition is [tests/living.rs](tests/living.rs):
`Move`/`Jump`/`Gravity` feed an upright unit-scale `CapsuleController` over a heightfield
slope, step and ledge; a rotated/scaled animated rig is its `Ambient` visual child.
Animation root motion is discarded, never applied after collision movement. The
explicit 10 kg crate is pushed, falls and sleeps. The sun turns, the child animates and a
derived HUD counts down throughout. Off/Save/FreshGame pin the same rest at tick 547:
player `(2.2726393, 1.5475401, -0.0044510923)`, crate
`(4.3423862, -1.5000682, 0.061061338)`. Ordinary time continues to night. The fixture
builds its two-node model in memory, so the physics test invokes neither the baker nor
the network. The imported Fox remains covered by the bake, renderer and skinned-game
fixtures; this test adds no game.
