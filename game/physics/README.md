# exact-game-physics

Rapier 0.35.3 behind Exact's saved components, in metres, kilograms and seconds.
Call `register` during setup, `move_character` after controls, and `step` per tick.

- Entity-ordered insertion maps `Body`, `Collider` and world `Transform` to Rapier;
  ordered handle maps and last-write comparisons detect edits, teleports and removal.
  Synchronization visits bodies and changed static rows. Page generations gate
  comparisons of cached local Transform/Parent values; ancestors are keyed by entity,
  including ancestors without a Transform. A body writing on an ancestor's page
  does not dirty its descendants. Body/Collider membership, world identity and
  presentation generation reset the cache; unrelated entity churn does not.
  Dirty rows still merge in entity order before Rapier insertion or mutation.
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
  Body/Collider/Transform/Parent write revisions gate page-generation checks, so
  same-tick edits are visible on the next operation. One retained collider/body set
  now serves all queries, including characters. Static shapes survive body motion;
  Geometry keeps a collider page generation, pose and handle, not source mesh or
  heightfield vectors. A changed Collider page conservatively refreshes its shapes.
  Raycasts and sweeps keep entity-order ties; overlaps sort and deduplicate.
  The free query functions are thin one-shot calls through this same cache.
  `move_character` uses the shared scene. Characters use Rapier's steps/slopes/snap, saved-pose platform transport and an
  80 kg default push budget. Movement and push share the layer-mask/sensor/self filter;
  the character's rigid collider is a sensor.

## Retained queries and performance (T1b2)

Public signatures are unchanged: `move_character(&mut World, Entity, Vec3)`,
`step(&mut World)`, `queries(&World) -> Queries<'_>`, and the existing raycast,
overlap and sweep methods/free functions. Saved Rapier set order and EXPHYS bytes
are unchanged. Query caches, bounds and partition certificates are derived only.

Rapier's controller needs one concrete pipeline. It now uses the retained query
set directly; there is no combined copy. Refresh restores only body controls and
updates changed poses/shapes. Finding the character handle is a map lookup;
push eligibility and writeback visit bodies, not all colliders.

A partial BVH refit is used only after certifying that Parry 0.30.2's canonical
binned build would retain the same partitions and leaf order. Static bin summaries
are cached; verification visits branches containing bodies. This preserves
traversal-dependent equal-TOI hits, support and impulses. A changed partition
rebuilds the canonical BVH from retained bounds. Static edits also rebuild the
BVH. The certificate deliberately mirrors the pinned Parry splitting algorithm;
its traversal oracle must keep passing when that dependency changes.

**This is not a worst-case O(changes) guarantee.** Repartitioning still costs O(n),
and the eight-character timing below exposes the tail. A changed bin grid reuses
static summaries when their center ranges certify unchanged bins; otherwise it
rebins that branch's statics. Fully interleaved Body writes still compare local
values across all dirty pages, although unchanged statics no longer reach solver
reflection or hierarchy walks. Physics membership changes rebuild ordered query
sets. These are remaining limits after three controller fix rounds.

The following are paired single-run diagnostics on shared x86-64 Linux,
2026-09-18, release (`opt-level=3`, thin LTO, one codegen unit). **Before** is the
T1b code at `7de8975`; **after** is T1b2. Both run the same checked `tests/timing.rs`
fixtures. Timings are not gates. Run from `game/`:

```sh
cargo test -p exact-game-physics --release --test timing -- --ignored --nocapture --test-threads=1
# RSS needs a fresh process; results after other fixtures include allocator reuse.
cargo test -p exact-game-physics --release --test timing retained_terrain -- --ignored --nocapture
```

**Consecutive movement only:** 20,000 unit static boxes, spaced 3 m on a 100-column
grid; one/eight characters moving +X at 1 m/s, no solver or intervening queries.
120 measured batches after 20 warmups; each subsequent call must observe the
previous character's Transform write. Values are whole-batch **median / p95 µs**.
Ray/overlap checks bracket the run and every character must advance.

| Characters | Before µs | After µs |
|---|---:|---:|
| 1 | 7,931.894 / 8,084.918 | 19.059 / 45.879 |
| 8 | 64,296.899 / 64,816.382 | 405.498 / 36,400.410 |

The eight-character p95 remains **36.4 ms** because sequential moves can each
change a canonical partition. The common refit path is much cheaper, but this
case does not meet a 60 Hz frame budget. Removing that rebuild without changing
hit order remains work; median improvement is not a claim that this tail is fixed.

**Whole ticks:** the same 20,000 statics, zero gravity, 120 measured ticks after
20 warmups. Each tick performs the character batch, `step`, then a checked ray
and overlap. The final query pays refresh for the last character, so the movement
column alone is not the tick cost. Each cell is **before → after, median µs**.
Churn spawns/despawns one empty entity before movement. Interleaved allocation
puts a moving body on every 1,024-slot page; the level root is spawned immediately
after the first body and all statics are its children.

| Scenario | Character batch | Step | Ray + overlap with refresh |
|---|---:|---:|---:|
| 1 character, statics first | 7,584.438 → 1.202 | 73.630 → 20.962 | 254.947 → 21.216 |
| 8 characters, statics first | 62,709.589 → 386.720 | 122.544 → 39.188 | 268.873 → 25.448 |
| 1 character + unrelated churn | 10,569.186 → 1.152 | 3,566.351 → 21.111 | 304.517 → 21.342 |
| 1 character + interleaved bodies/root | 7,877.253 → 2.354 | 6,445.056 → 753.484 | 5,501.742 → 759.323 |

**Moving extrema:** one character outside the same scenery, 120 consecutive
movement calls after 20 warmups. Continuous movement starts at X=1,000 m; the
teleport case alternates X=−1,000/+1,000 before each move and forces repartitioning.
Values are **median / p95 µs**; no solver or intervening queries.

| Scenario | Before µs | After µs |
|---|---:|---:|
| Continuous movement beyond scenery | 6,915.586 / 6,982.095 | 17.917 / 18.137 |
| Teleport across scenery every call | 7,206.407 / 7,463.337 | 4,653.044 / 4,928.197 |

**One static edit:** 1,000 single-static teleports in the 20,000-box world, each
followed by an asserted ray hit and exact overlap result. Pose-only edits retain
shapes, but the canonical BVH rebuild remains **O(n) per edit**, as allowed for
this slice: **9,750.967 → 3,855.286 ms total**, **9,736.008 → 3,832.610 µs median**.
This is still an unfavorable workload, not an incremental static-tree result.

**Frozen scenery with moving bodies:** statics allocated first, then dynamic boxes
moving +X at 1 m/s with zero gravity. 600 measured ticks after 20 warmups. The
1,000-ray batch includes refresh after `step`; the 1,000-overlap batch follows.
Every ray must hit; every overlap must equal its expected static entity. Each cell
is **before → after, median µs**.

| Static / dynamic boxes | Step | 1,000 rays | 1,000 overlaps |
|---|---:|---:|---:|
| 100 / 10 | 17.651 → 12.489 | 286.579 → 244.317 | 500.530 → 463.790 |
| 2,000 / 10 | 107.381 → 35.833 | 553.285 → 427.410 | 645.462 → 597.490 |
| 20,000 / 100 | 135.503 → 89.419 | 672.173 → 573.865 | 719.524 → 668.987 |

**Terrain retention and first use:** isolated processes, one flat 1,024×1,024
heightfield with 1,000 m X/Z extent, followed by its first ray and first character
move. Linux RSS is allocator/zero-page dependent, not an allocation census.

| Measurement | Before | After |
|---|---:|---:|
| RSS: ECS only, KiB | 3,996 | 3,968 |
| RSS: after first ray, KiB | 14,304 | 10,224 |
| RSS: after first character, KiB | 14,832 | 10,560 |
| First ray, µs | 10,586.257 | 8,433.851 |
| First character, µs | 555.643 | 7,883.727 |

The query's retained source-vector clone is gone (about 4 MiB less RSS here).
The first-character latency **regresses**: installing its Body/Collider changes
membership after the terrain query was built, reconstructing the ordered set;
the old split retained its static shape through that insertion. Warm movement
shares the same terrain shape, proved by a shape-identity regression. Query
Geometry no longer owns any `Collider` or `Body` clone; the saved solver's existing
last-write state is unchanged.

Behavioral parity runs one/eight characters with carry, support casts, coincident
equal-TOI static/dynamic obstacles in both insertion orders, overlapping AABBs,
masks, sensors, pushed bodies and restored controls. It compares ordered hit
traces, support hits and complete resulting world bytes against a fresh canonical
single-scene build. This controller oracle does not step its deliberately
coincident rigid bodies; existing solver snapshot/continuation tests cover step.
Churn, mixed-page ancestors, static edits, membership/recycling, world identity
and load have separate regressions. The saved-pile tick-600 pin remains
`0x5ba7691abdc98058`; no pinned hash or position was changed.

Validation on this Linux host: **413 workspace Rust tests passed**, plus all
**6 release timing diagnostics**. Game workspace clippy (`-D warnings`), formatting,
root caps and boot pass. Linux proofs pass **54 Beacons, 62 Greybox, 8 Lanterns,
6 asset-fixture assertions** (130 total). Bun is **38 passed / 2 environmental
failures**: absent Chrome for the generated-game web proof and missing prebuilt
60/120 Hz feel artifacts. GPU pixels, browser execution and Apple runtimes were
not verified here. No public signature, pinned position or hash was changed.

## Saved solver and historical measurements

Saved state is opaque bincode/serde for bodies, colliders, islands, broad/narrow phase,
joints and integration parameters, plus entity/handle maps and last writes. Restore validates and decodes live state atomically; pipeline/CCD workspaces are scratch under Rapier's serialization contract.
`Data::write(&self)` refreshes dirty bytes for save, hash and JSON; `refresh_snapshot`
measures the same operation. Stepping does not serialize. JSON summarizes the opaque bytes by length/hash.
Malformed or obsolete Rapier payloads fail during `World::load`, before replacement.

Measurements, 2026-09-17: release; 100 boxes poured every 12 ticks into a finite 24 m
bin. Active samples start at the last pour; 180 asleep samples finish each run. Sleep
is seconds after the last pour. Times are p50/p95 ms; shared hosts, no concurrent build.
Snapshots are ten dirty refreshes from the last pour; bytes exclude maps/outer World,
while refresh timing includes mapping copies. The 5,000-box pour exceeds the bin walls.

| Host | Boxes | Active ms | Sleep s | Asleep ms | Rapier payload bytes | Snapshot ms |
|---|---:|---:|---:|---:|---:|---:|
| Mac arm64 | 1,000 | 3.895 / 4.875 | 2.250 | 0.017 / 0.031 | 4,016,891 | 1.786 / 2.518 |
| Mac arm64 | 2,000 | 8.287 / 10.805 | 2.967 | 0.032 / 0.033 | 10,030,616 | 4.714 / 5.373 |
| Mac arm64 | 5,000 | 24.776 / 30.287 | 5.283 | 0.089 / 0.285 | 28,096,249 | 15.191 / 18.336 |
| Linux x86-64 | 1,000 | 3.322 / 3.926 | 2.250 | 0.018 / 0.019 | 4,016,891 | 1.879 / 3.849 |
| Linux x86-64 | 2,000 | 7.936 / 8.327 | 2.967 | 0.042 / 0.046 | 10,030,616 | 5.418 / 12.080 |
| Linux x86-64 | 5,000 | 31.206 / 32.517 | 5.283 | 0.109 / 0.116 | 28,096,249 | 19.121 / 41.395 |

Payload counts above exclude the eight-byte EXPHYS marker; `refresh_snapshot()`
includes it. The D1 Sim save measurement is in [engine README](../engine/README.md).
The wasm `minimal` cdylib retains register/step/hash plus the saved-pile continuation
card, excluding query/character APIs: web profile, **1,393,618 raw / 502,053 gzip-9
bytes** on the Mac builder. The Rapier dependency graph adds 43 normal/build crates
on Mac, 44 on Linux (`safe_arch`), excluding this crate and engine/glam.
Mac 2,000-box active p95 still exceeds 10 ms; Linux snapshot p50 exceeds 5 ms.
2,000-box sleep/asleep targets pass; 5,000-box sleep exceeds 4 s.

At the tenth 2,000-box snapshot, per-set bytes and serialization p50 ms (Mac / Linux):
bodies 693,953, 0.173 / 0.246; colliders 256,198, 0.088 / 0.148;
islands 219,453, 0.150 / 0.213; broad phase 406,301, 0.838 / 1.073;
narrow phase 8,454,595 (84.3%), 3.154 / 3.268. Remaining fields total 116 bytes.
Omission trials (bytes; refresh p50/p95 ms Mac, Linux), all reverted:
broad 9,624,317; 4.007/5.423, 4.149/9.062; narrow 1,576,023; 1.858/2.905, 1.803/2.937;
both 1,169,722; 0.711/0.796, 0.735/1.454. Retained before/after is the full snapshot above.
Rebuilding broad phase diverged at pile tick 92; rebuilding narrow/both broke island/contact links on both hosts; bounce passed, but exact pile resume did not, so full phase state stays saved.

Accuracy against independent geometry: capsule-ray normal ≤3° (measured 1.845°),
ray distance ≤3 mm; curved sweeps ≤5 mm per 3 m (measured 2.955 mm; Parry stops near
1e-3 relative); exact sphere/box cases ≤0.1 mm (sweep worst 0.04077 mm).
Rolling contact drift is bounded at 0.1 m/s and acceleration within 3% of (5/7)g sin θ.
Physics, engine and audio tests pass on arm64 macOS and x86-64 Linux.
Pile resume hashes are checked every tick through 600, mid-bounce through 240.
D1b typed-bulk cards agree on **arm64 macOS, x86-64 Linux and Chrome 153 wasm**:
pile tick 600 `0x5ba7691abdc98058`, two-body `simulate(120)`
`0x9960c10fadbb9c4b`. The browser pile card saves at tick 90, restores and checks
exact continuation every tick through 600. `minimal.wasm` exports `pile_hash()`
and `simulate(ticks)` for direct `WebAssembly.instantiate` calls.
Enhanced determinism, glam scalar-math/libm; no parallel, simd8 or fast-math features.
Parry still uses four-lane `wide`. These executed cards establish fixture parity,
not a claim of whole-engine determinism for every physics query and character API.
Owed: joints API, CCD policy beyond Rapier’s automatic fixed-collider CCD, compound shapes.

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
