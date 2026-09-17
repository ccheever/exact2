# exact-game-physics

Rapier 0.35.3 behind Exact's components. Call `register` in setup,
`move_character` after controls, and `step` once per fixed tick. The previous
solver, narrowphase, query algorithms and character solver are deleted.

**P2 is not acceptance-complete.** After three correction rounds, both hosts pass
17 of 23 physics tests and all 101 engine tests/doctests (including greybox).
Six retained physics properties fail; they remain enabled. No commit.

## Mapping and saved state

- One `PhysicsPipeline::step` per tick, at `world.dt()`. Entity-ordered insertion;
  ordered maps in both directions. Component comparisons against the last
  writeback detect teleports, velocity edits, geometry/material changes and removal.
- Dynamic poses/velocities/sleep return to components. Kinematics use Rapier's next
  pose. A Rapier collision-only pass after kinematic movement preserves same-tick
  sensor events. Events are sorted; `Announce` journals transitions.
- Density defaults to 1000 kg/m³; explicit mass is kilograms. Friction combines
  geometrically, restitution by maximum. Linear contact slop is 0.1 mm instead of
  Rapier's 5 mm default. Other integration settings are Rapier defaults.
- `quiescent` means all dynamic bodies sleep; active physics calls `world.busy`.
  Sleeping ticks still step Rapier, with an ordered comparison instead of copying
  unchanged components. Support removal/material edits conservatively wake all.
- `Physics` owns an `Executor` Data adapter. Its saved record contains compact
  bincode/serde bytes for Rapier's bodies, colliders, islands, broad/narrow phase,
  joints and integration parameters, plus entity/handle mappings and last writes.
  `#[data(skip)]` live state is deserialized lazily. Rapier's pipeline and CCD
  workspace are scratch by its own serialization contract; no CCD history is lost.
- `Data::write(&self)` refreshes dirty bytes before save, hash **and JSON**. No
  engine pre-save hook is needed. `Physics::refresh_snapshot()` measures that same
  operation. No serialization runs in `step`. The byte representation is opaque
  and makes agent JSON large; a loaded malformed Rapier blob is rejected by a panic
  on first use, not by `World::load`.
- Queries rebuild a Rapier query view from current components, so same-tick edits
  are visible and reads cannot change simulation hashes. This is O(n) per call.
  The character uses Rapier's autostep, slope limits, ground snap and documented
  collision-impulse routine, with explicit saved-pose platform transport and an
  80 kg default maximum pushable-body mass. Its rigid collider stays a sensor.

## API differences

The named components/functions remain. `Shape` adds `Cylinder { radius, height }`,
`Heightfield { rows, cols, heights, scale }` (row-major, rows Z/columns X), and
static `Mesh { vertices, indices }`. Heightfields are also static. Curved shapes
require uniform positive scale; cylinders/capsules use total height. Translational
sweeps now accept any convex shape, including boxes and cylinders.

Deleted solver-specific fields/types: `Body.calm/previous`, `BodyState`,
`Physics.substeps/manifolds/previous`, `Contact`, `Manifold`, `ColliderState`, and
`step_observed`. Last-write detection moved into the resource. Added
`Physics::refresh_snapshot`. There is no compatibility decoder for old saves.

## Measurements — 2026-09-17

Release, no overlapping compilation from this task. Same 100-box/12-tick pour and
finite 24 m bin as P1b; active samples start after the last pour, and 180 asleep
samples end each run. Times are ms, **p50 / p95**. Sleep is seconds after the last
pour. These are shared machines. The 5,000-box fixture pours above the bin walls.

| Host | Boxes | Active ms | Sleep s | Asleep ms |
|---|---:|---:|---:|---:|
| Mac arm64 | 1,000 | 3.725 / 5.020 | 2.250 | 0.017 / 0.022 |
| Mac arm64 | 2,000 | 9.374 / 15.805 | 2.967 | 0.032 / 0.075 |
| Mac arm64 | 5,000 | 22.353 / 25.798 | 5.283 | 0.081 / 0.088 |
| Linux x86-64 | 1,000 | 3.330 / 3.932 | 2.250 | 0.019 / 0.020 |
| Linux x86-64 | 2,000 | 7.981 / 8.241 | 2.967 | 0.042 / 0.047 |
| Linux x86-64 | 5,000 | 30.801 / 31.951 | 5.283 | 0.109 / 0.114 |

At 2,000 boxes, ten dirty refreshes immediately after the last pour:
Mac **6.191 / 7.441 ms**, Linux **5.316 / 12.000 ms** (first calls 7.441 / 12.000 ms).
The last refresh produces **10,030,616 Rapier bytes**, excluding the separately
saved mapping records and outer World encoding. These figures include refreshing
those mapping records. The 5 ms refresh target and Mac 10 ms active p95 target fail;
2,000-box sleep time and asleep cost pass. The 5,000-box sleep exceeds 4 seconds.

Minimal cdylib, wasm32-unknown-unknown, `opt-level="z"`, fat LTO:
**1,231,611 bytes raw / 438,271 bytes gzip -9**. It retains registration, rigid
stepping and hashing; unused character/query entry points are excluded by LTO.
The normal/build graph adds **43 dependency crates** over the former production
physics→engine/glam graph (Rapier was already present as a dev dependency).

Tick-600 5×5×5 scene: **Mac `0x10adc45f96879746`; Linux `0x10adc45f96879746`**,
in debug tests and the release probe. Mid-bounce tick-45 saves continue exactly
through tick 240; pile tick-90 saves resume through 600. Greybox remains
`0x70c17d4a69834418` with physics in the same build graph. Clippy `-D warnings` and
fmt pass on both hosts. Repository caps passes for tracked files; each new source
is separately under 1,500 lines. Production source is about 1,330 lines.

[Mac measurements](measurements/p2-mac-pile.txt), [Linux measurements](measurements/p2-linux-pile.txt),
[Mac tests](measurements/p2-mac-tests.txt), [Linux tests](measurements/p2-linux-tests.txt).
P1b's before numbers remain in [Mac](measurements/p1b-after-mac.txt) and
[Linux](measurements/p1b-after-linux.txt): Mac 2,000-box p95 was 57.776 ms and never slept.

## Owed / conflicts with the contract

The six failing tests retain their original tolerances, not Rapier-oracle comparisons:
curved-pair normals against dense sampling; coincident-capsule penetration
(-0.5961096 m vs -0.6 m); four stable clipped-face features (Rapier supplies eight
points); capsule-ray normals; capsule sweeps (0.224 mm error vs the 0.1 mm bar);
and rolling contact speed (0.07498 m/s at 20° vs <0.02). Rest height, stack sleep,
bounce, all character fixtures, rigid CCD and terrain/mesh/cylinder checks pass.
Disabling contact recycling did not fix rolling; that change was removed. A raw
Rapier probe increasing iterations also failed the rolling threshold. Further
geometry/controller/solver changes stopped at the three-round limit. The character
impulse routine's proximity filter also needs the collision layer mask reapplied.

Rapier uses glam **0.33.7** through glamx **0.3.0**. Unified `scalar-math` and `libm`
remain enabled; neither `parallel`, `simd8`, nor `fast-math` is enabled. However,
Parry 0.30.2 always uses four-lane `wide` types, falling back to scalar where needed.
No purely scalar Rapier executor is selectable through these features. Both tested
architectures agree; wasm execution determinism was not measured.

Reproduce from `game/`, with `EXACT_UPDATE_TRUST=development`:

```
cargo test -p exact-game -p exact-game-physics --no-fail-fast -- --nocapture
cargo clippy -p exact-game-physics --all-targets -- -D warnings
cargo fmt -p exact-game-physics -- --check
cargo run -p exact-game-physics --release --example pile -- 1000 2000 5000
cargo run -p exact-game-physics --release --example pile -- --verify
cargo build -p exact-game-physics --profile web --target wasm32-unknown-unknown --example minimal
```

Validation used `CARGO_RESOLVER_LOCKFILE_PATH` with a temporary lockfile inside
this directory, leaving the shared `game/Cargo.lock` to its owning lane. Integration
must refresh that lockfile for bincode/serde-serialize. No engine files were edited.
Staging was attempted only for `game/physics/`; the sandbox refused the shared
Git index lock. Changes remain unstaged.
