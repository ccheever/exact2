# exact-game-physics

Rapier 0.35.3 behind Exact's saved components, in metres, kilograms and seconds.
Call `register` during setup, `move_character` after controls, and `step` per tick.

- Entity-ordered insertion maps `Body`, `Collider` and world `Transform` to Rapier;
  ordered handle maps and last-write comparisons detect edits, teleports and removal.
  Dynamic poses, velocities and sleep return to components; kinematics use next pose.
- One step uses `world.dt()`. A collision-only pass after moving kinematics supplies
  same-tick sensor transitions. Events are sorted; `Announce` journals transitions.
  Sleeping bodies still step; seekable Sim observes Body/Transform changes. Support edits wake all.
- Density defaults to 1000 kg/m³; explicit mass is kg. Friction combines geometrically,
  restitution by maximum. Contact slop is 0.1 mm; other integration defaults are Rapier's.
- Shapes: sphere, box, Y capsule/cylinder (total height), static mesh and heightfield
  (row-major, rows Z/columns X). Curved shapes need uniform positive scale.
  Bodies are roots; parented boxes must not acquire shear. Sweeps translate convex shapes.
- Queries rebuild an O(n) component view, see same-tick edits and never mutate hashes.
  Characters use Rapier's steps/slopes/snap, saved-pose platform transport and an
  80 kg default push budget. Movement and push share the layer-mask/sensor/self filter;
  the character's rigid collider is a sensor.

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
D1 bulk-format cards agree on **arm64 macOS, x86-64 Linux and Chrome 153 wasm**:
pile tick 600 `0x68fadd78ef1d93f8`, two-body `simulate(120)`
`0x2c1221fdf12e6744`. The browser pile card saves at tick 90, restores and checks
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
