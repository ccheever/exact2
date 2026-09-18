# exact-game-physics

Rapier 0.35.3 behind Exact's saved components, in metres, kilograms and seconds.
Call `register` during setup, `move_character` after controls, and `step` per tick.

- Entity-ordered insertion maps `Body`, `Collider` and world `Transform` to Rapier;
  ordered handle maps and last-write comparisons detect edits, teleports and removal.
  Synchronization visits bodies and changed 1,024-slot static pages. A bodyless
  collider's own page and all ancestor Transform/Parent pages determine dirtiness;
  component membership, world identity and load invalidate the derived page cache.
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
  same-tick edits are visible on the next operation. Bodyless colliders retain a
  separate static BVH; only their membership, collider data or global poses rebuild
  it. Bodies (including explicit static/kinematic bodies) use the dynamic partition.
  A moving body sharing a page with scenery does not rebuild unchanged static shapes.
  Raycasts, overlaps and sweeps visit both BVHs with the same entity-order ties.
  The free query functions are thin one-shot calls through this same cache.
  `move_character` uses the shared scene. Characters use Rapier's steps/slopes/snap, saved-pose platform transport and an
  80 kg default push budget. Movement and push share the layer-mask/sensor/self filter;
  the character's rigid collider is a sensor.

P1 query/sync measurements, 2026-09-17, release, 2,000-box pour: median of three
run summaries. Entries with two numbers are p50 / p95 ms. Each row carries its
three load1 readings. The ray batch includes the first lazy scene build; character
timing is `move_character` per 60 Hz movement tick against the settled pile,
without a solver step between movements. The pile timings separately include
`physics::step` through active and sleeping ticks.

| Host / work | Before ms | Before load1 | After ms | After load1 |
|---|---:|---|---:|---|
| Mac, 1,000 rays | 652.038 | 21.58/21.58/20.73 | 1.417 | 19.15/19.15/18.10 |
| Mac, character | 1.108 / 1.330 | 21.58/21.58/20.73 | 0.971 / 1.164 | 19.15/19.15/18.10 |
| Mac, pile active | 4.463 / 5.868 | 21.58/21.58/20.73 | 3.400 / 3.936 | 19.15/19.15/18.10 |
| Mac, pile asleep | 0.023 / 0.048 | 21.58/21.58/20.73 | 0.020 / 0.029 | 19.15/19.15/18.10 |
| Linux, 1,000 rays | 1914.686 | 3.32/3.37/3.18 | 3.991 | 1.85/1.78/1.78 |
| Linux, character | 2.820 / 2.871 | 3.32/3.37/3.18 | 2.039 / 2.442 | 1.85/1.78/1.78 |
| Linux, pile active | 7.978 / 8.367 | 3.32/3.37/3.18 | 7.830 / 8.166 | 1.85/1.78/1.78 |
| Linux, pile asleep | 0.046 / 0.050 | 3.32/3.37/3.18 | 0.045 / 0.048 | 1.85/1.78/1.78 |

Rapier's character controller requires a single concrete query pipeline. Its
combined view is assembled lazily from retained shapes, with the original ordered
handles and binned BVH to preserve traversal ties and pinned crate pushes. This
controller-only assembly remains O(n) after relevant geometry changes; ordinary
ray/overlap/sweep calls do not assemble it.
A view cannot ignore same-tick edits merely to enforce one rebuild per tick.
Repeated unchanged scopes/ticks reuse the scene. These reads do not serialize
or advance the saved solver. `pile -- 2000 --queries` reproduces the timings.

The ignored `tests/timing.rs` diagnostic measures 600 moving ticks after 20 warmup
ticks for (static boxes, dynamic bodies) = (100,10), (2,000,10), (20,000,100).
It reports median step, 1,000-ray and 1,000-overlap batch times separately; the ray
batch includes the first scene refresh after each step. Run from `game/` with
`cargo test -p exact-game-physics --release --test timing -- --ignored --nocapture --test-threads=1`.

T1b, 2026-09-18, shared x86-64 Linux host, release; each cell is **before → after**
in microseconds, from that same diagnostic (query batches include both partitions):

| Static / dynamic | Step | 1,000 rays | 1,000 overlaps |
|---|---:|---:|---:|
| 100 / 10 | 16.885 → 17.827 | 265.253 → 278.462 | 463.670 → 491.126 |
| 2,000 / 10 | 189.729 → 108.838 | 1,132.437 → 528.216 | 618.537 → 632.323 |
| 20,000 / 100 | 2,399.926 → 133.330 | 8,040.879 → 656.344 | 675.241 → 705.503 |

The extra query partition has a small fixed cost; the large-world savings are
reflection and scene reconstruction. Cached Lanterns Linux proof wall time was
0.613 → 0.618 s (single runs, builds excluded); no measured proof speedup.
The EXPHYS v2 saved-pile tick-600 hash is `0x129ba6d92f9ac217`; static edits, ancestor
edits, membership/recycling, cache identity/load and query ties have regression tests.

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
The original D1b EXPHYS v1 cards agreed on arm64 macOS, x86-64 Linux and
Chrome 153 wasm: pile `0x5ba7691abdc98058`, two-body `0x9960c10fadbb9c4b`.
The corrected EXPHYS v2 cards are pile tick 600 `0x129ba6d92f9ac217` and
two-body `simulate(120)` `0x5608994347e54d28`, verified on x86-64 Linux in
continuous and every-step save/load runs. Apple and browser rechecks are pending.
`minimal.wasm` exports `pile_hash()` and `simulate(ticks)` for direct
`WebAssembly.instantiate` calls; the pile card restores at tick 90 and compares
continuation through tick 600. `pile --verify` also checks `simulate(120)` against
a world reconstructed after every physics step.
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

EXPHYS v2 includes Rapier's deferred BVH optimization bit. V1 cannot recover this
missing state and is refused (including inside EXSIM/captures); start a new world.
The physics crate depends on vendored Rapier 0.35.3 by path; no shared Cargo
cache edits or root workspace dependency are needed. T4 pin evidence is recorded
in `game/README.md`.
