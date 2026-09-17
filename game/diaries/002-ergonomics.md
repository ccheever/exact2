# G1 — ergonomics after Beacons

2026-09-17. Implemented in `lane/game`, without a commit. Baseline: `9b52004`.
No changes to `game/bench/`, `game/twins/`, or root crates. Concurrent Contract
edits belonged to the other worker; this pass neither altered nor staged them.

## What changed

- `#[derive(Args)]` declares positional names, setup/live fields, decoding and
  setup-change comparison without `syn`. All eight requested scalar types work;
  `()` covers no arguments. Refusals name the field, expected kind and received
  value. Beacons binds `world(seed, run, paused)`. Decode precedes mutation; timed
  binds still seek under old arguments; saves retain bound values.
- Seekable advances observe components before and after the last tick. They take
  exactly two samples regardless of seek length; zero ticks retain the previous
  answer. Ambient entities and resource bookkeeping are excluded. Springs and
  explicit work still participate. Live advances take no samples. Clock replies
  list up to eight changing entity/component names; non-spring deadlines back off
  from 100 ms to 2 s. `busy` now borrows `&self`.
- Mesh primitives carry real dimensions, which drive generated geometry, cached
  draw groups, picking and layout. The 4,096-entry cache refuses excess dimensions
  with the entity name. Transform pages retain their memcpy upload. `Collider::of`
  matches every primitive; a plane becomes a static 1 cm slab with its top at y=0.
- Saved `Follow` cameras run explicitly through `scene::follow`. Initial placement
  and target teleports snap; ordinary following uses `math::ease`. Scalar and Vec3
  easing use an exponential time constant and arrive within 0.1 mm. Beacons and
  greybox use the shared functions.
- `state world:*` returns component records for up to 512 entities, with subtree
  narrowing. `type world KeyW for 1500` and its object form record down/clock/up.
  Both proofs import `game/proof.mjs` for builds, sessions, assertions, transcripts,
  reply artifacts, timings, exit status and child cleanup. Unchanged inputs skip
  the build. Each proof checks complete save/restore across fresh browser sessions.
- The README's complete game is the engine's runnable doc-test: capsule, plane,
  Follow, a Spring-driven glow and a published count. All game API consumers and
  current READMEs were updated, including the audio example.

Deleted: `Arg`, `Game::ARGS`, `Game::check`, string argument accessors,
`world.args()`, fallible setup, unit primitive variants, per-game camera stepping,
manual movement/glow/ camera busy declarations, physics awake-body busy reporting,
and duplicated proof lifecycle code. No compatibility shims remain.

Integer wire values are f64. `u64` and `i64` therefore accept portable safe integers
through 2^53−1, as documented, instead of silently rounding larger integer inputs.
`Mesh::asset` retains the existing renderer placeholder; authored asset colliders
still require authored geometry.

## Size and readability

Physical lines, including comments and blank lines:

| file | before | after |
|---|---:|---:|
| Beacons `logic/src/lib.rs` | 198 | 169 |
| Beacons `app.contract` | 38 | 38 |
| Beacons `proof.mjs` | 175 | 129 |
| Greybox `proof.mjs` | 212 | 144 |
| Shared `game/proof.mjs` | 0 | 116 |

Beacons reads better than before. Argument handling, primitive sizing, the camera
and settling now say what the game means. It does **not yet read better overall
than three.js's 57-line `simulation.js`**: Rust's entity queries, borrow-shaped
access and long spawn tuples interrupt the gameplay more than that file's flat
objects. The comparison is not equal scope: this Rust file includes scene setup
that the twin also spreads into rendering code. Neither the reduced line count
nor this pass establishes a new blind score. Gameplay's jump integration and
beacon envelope remain ordinary handwritten functions.

## Performance

Release builds on the shared M5 Max, arm64 macOS. For a closer comparison, saved
baseline/current executables were run in alternating order in this worktree;
source edits were preserved and restored, and current examples rebuilt afterward.
No clone, stash or commit was used. The paired churn figures below are medians of
three runs per version; each churn rotation figure itself measures five 200-tick
runs. The cubes figures are medians of four runs per version, 480 frames each at
200,000 entities, 2560×1440, 4× MSAA. Current cubes measures the live clock so agent
observation is excluded from gameplay timings.

| measurement | before | after |
|---|---:|---:|
| churn, 100k rotation, ns/entity/tick | 3.08 | 2.67 |
| churn, 500k rotation, ns/entity/tick | 3.31 | 2.89 |
| churn, despawn/respawn 1k of 100k, ms | 0.391 | 0.364 |
| churn, iterate 12 of 100k, µs | 1.229 | 1.126 |
| cubes, 200k simulation, ms/tick | 0.5743 | 0.5667 |
| cubes, 200k simulation, ns/entity/tick | 2.871 | 2.834 |
| cubes, 200k feed, ms/tick | 1.3947 | 1.3762 |
| cubes, encode, ms/frame | 0.1324 | 0.1254 |

No regression beyond measured noise in these paired runs; no speedup claim. The
200k sim medians ranged 0.5425–0.6587 ms before and 0.4876–0.5735 after; feed ranged
1.3507–1.5322 before and 1.2973–1.4058 after. The initial unpaired measurements were
less reassuring: sim 0.6138→0.7124 ms and feed 1.4266→1.7091 ms. A short 120-frame
paired series also gave sim 0.5511→0.6282 and feed 1.3065→1.4234. Those observations
are retained in the logs; their disagreement with the longer alternating series
is why a single measurement on this shared machine is insufficient.

For the historical sim table in `game/bench/README.md`, the Mac's 500k rotation row
was 3.6 ns; today's paired result is 2.89 ns. The x86-64 Linux builder's historical
5.8 ns row measures 5.78 ns now (100k: 5.74 ns). The Linux historical comparison is
not a fresh paired baseline. Bench files themselves were not modified.

One stillness observation, Transform-only entities, 100 warm samples on the Mac:

| entities | median ms | p95 ms |
|---|---:|---:|
| 1,000 | 0.149750 | 0.169167 |
| 10,000 | 0.698291 | 0.870125 |
| 200,000 | 15.350417 | 16.890625 |

These include per-component hashing and writing the reusable observation buffer.
A nonzero agent seek pays twice, plus comparison; a live tick pays no hashing or
observation allocation. Tests count serialized component writes to verify the
two-sample/zero-live distinction, and the renderer's steady live allocation test
passes. More component state costs more; these are not all-game upper bounds.

The 2,000-box pile reports **observed stillness at tick 406**, exactly the old
sleep tick, 2.967 s after the last pour. Physics active step p50/p95 was
6.636/7.731→5.593/6.828 ms; asleep was 0.024/0.046→0.023/0.043 ms. Observation in
this diagnostic runs outside the timed physics step. The physics busy call is gone.

## Parity and verification

All three executions agree, bit for bit: native arm64 macOS, native x86-64 Linux
builder and Chrome wasm. Seed 7, run 0, unpaused; W is held for exactly 1,500 ms.

| game / instant | macOS | Linux | Chrome wasm |
|---|---|---|---|
| greybox setup | `6b4d864d2da4c316` | `6b4d864d2da4c316` | `6b4d864d2da4c316` |
| greybox W1500 | `e361b9c0055bede6` | `e361b9c0055bede6` | `e361b9c0055bede6` |
| Beacons setup | `67e5994bda318b9a` | `67e5994bda318b9a` | `67e5994bda318b9a` |
| Beacons W1500 | `f1bdfbe68b382647` | `f1bdfbe68b382647` | `f1bdfbe68b382647` |

The same 500k churn world also hashes `120ad65324678b6f` on Mac and Linux. The new
game pins and greybox component snapshots are checked in the source tests and
proofs. Easing now actually arrives at the desired velocity: W1500 z moves from
−5.733332 to −5.7333384, about 6 µm, inside the unchanged 1 mm gameplay assertion.

Passed locally with `EXACT_UPDATE_TRUST=development`:

- Game workspace build, `cargo test --workspace --no-fail-fast`, clippy with
  `--all-targets -- -D warnings`, and format check. The README example compiled
  and ran. Beacons tests also passed after the final argument-order adjustment.
- Both browser proofs: greybox 15.928 s; final Beacons 35.858 s including its bake.
  Both continued whole saves byte-identically in fresh sessions and found no
  recorded children remaining. A repeated unchanged Beacons proof printed
  `BUILD cached` and passed in 11.116 s.
- The pre-existing driver transcript fixture still matches, alongside the new
  three-step type transcripts exercised in both real browser proofs.
- `bun scripts/caps.mjs` passes after staging: 554 source files scanned, none over
  1,500 lines. The staged diff has no whitespace errors. No commit was made.

Raw local validation and measurement logs are in `game/target/g1/` (ignored),
including every initial and paired timing, `parity-*-final.txt`, workspace check
logs and proof logs. Each game's ignored `artifacts/` contains its transcript,
`replies.json`, saves, screenshots and process cleanup audit. The Linux builder
ran parity and churn only; no GPU verification is claimed there. Native game UI
proofs and iOS were not part of this run. No unrelated root verification process
was stopped, and no process was left running on the builder.
