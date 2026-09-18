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


# G1b — reviewed repairs and typed gameplay

2026-09-17. Baseline `f44f5df` (including the carry/restore fixes), repairing
`6996a8a`. This section supersedes G1's current-behavior claims above. One agent;
no commit, clone or stash. Changes are confined to `game/` and the held-key form
in `scripts/agent.mjs`; root crates, host sources, `scripts/app.mjs` and game
manifests were not edited.

## Fixes and regression evidence

`engine/tests/g1b.rs` and `g1b_args.rs` hold the new engine regressions. The table
names their test functions, or the existing suite extended for the repair.

| Brief | Result | Regression test |
|---|---|---|
| A1 | `Unknown / Still / Changing`; new, rebuilt, restored, live-advanced and input/argument-invalidated worlds require observation. Observation history is absent from saves. | `new_live_restored_and_queued_worlds_require_an_observation`; `observation_is_not_saved_and_resources_are_observed` |
| A2 | Non-ambient entities contribute existence; parented entities also contribute their global pose. | `global_poses_and_componentless_existence_are_observed` |
| A3 | Resources count by default. `Resource::AMBIENT` is the single opt-out; Physics and Voices use it. | resource countdown above; `ambient_resources_are_never_serialized_by_observation`; audio player test settles with a voice still playing |
| A4 | Capped union of component changes, moving spring/tween entity.component names, and busy reasons; unknown and pending-input reasons prevent unexplained false replies. | `springs_and_tweens_explain_rest_and_deadlines`; busy/back-off test below |
| A5 | Only parsed settle requests advance back-off; rest, rebuild, restore, input and argument changes reset it. README describes last-two-tick sampling and skipped one-shots. | `clock_reads_do_not_back_off_and_input_bind_restore_restart_reset` |
| A5b | Pending input prevents rest and bounds the next jump to its delivery tick. | `current_and_future_input_prevent_rest_and_bound_the_next_jump` |
| B6 | `Game::validate` runs before construction, timed bind/seek/rebuild, and restore commit. Domain refusal leaves bytes unchanged. | `a4_3_pause_at_500_inside_one_seek_matches_two_seeks_restart_and_refusals_are_atomic` restores invalid-volume and seed-99 assertions; `decoded_float_state_is_exact_signed_zero_rebuilds_and_validation_is_atomic` |
| B7 | Saved arguments use named fields; additions default, removals are ignored. Bound args win; `restoredFrom` preserves the saved record until restored state ends. | `saved_arguments_follow_names_default_new_fields_and_ignore_removed_fields` |
| B8 | Float setup comparison uses bits; state serializes the decoded struct without display rounding. Malformed live attributes refuse with the field name. | both float tests in `g1b_args`; two compile-fail Args doctests |
| C9 | Primitive dimensions occupy instance data; at most one unit geometry per primitive kind. True capsule hemispheres use cap signs. | `primitive_dimensions_are_instance_data_and_capsules_deform_exactly`; `animated_dimensions_never_grow_geometry_and_all_spheres_batch_together`; hidden/revealed dimension regression |
| C10 | Plane collider is a translated thin box; pick/layout use the same slab, top Y=0. Dynamic bodies work. | `plane_is_a_dynamic_box_and_side_pick_matches_the_physics_slab` |
| D11 | Follow places zero-offset cameras, handles top-down aiming, reads current parent poses, reinitializes names after target replacement; finite extreme ease endpoints remain finite. Both setups call follow explicitly. | `followers_handle_top_down_zero_offsets_current_parents_and_reincarnation`; existing Follow teleport/save test; both game proofs |
| E12 | Held input resolves once and releases that carrier in finally. Partial failure steps survive; CLI requires `type world key KeyW for 1500`. | three held-key/parser tests in `proof.test.mjs`, including down/clock/up failures and ordinary text ending in `for 100` |
| E13 | Receipt binds web manifest or existing app executable (also the actual standalone carrier binary). Timer clears before best-effort inventory; unconditional cleanup closes every session. | two receipt/cleanup tests in `proof.test.mjs`; all three real proofs report no surviving recorded children |
| F14 | Typed `Sim::new(G::Args)`; host entry is `from_values`. Game tests and README construct typed args. | README doctest; both game suites; typed validation test |
| F15 | Relative run/key_down/key_up/tap/hold/settle use the host clock rules and sixteen-jump bound. | `input_sugar_and_relative_sentences_match_absolute_host_input`; busy test exhausts bound; both game suites |
| F16 | One `Target` trait lets get/get_mut accept Entity or name; position helpers are deleted. | `named_access_guarded_rows_and_release_singleton_refusal`; both game suites |
| F17 | Consuming query iteration yields items; `.iter()` retains Entity; `.one()` refuses ambiguity in release with component name and count. | named-access test above, also run with `--release`; `a20_one_returns_a_leased_item_and_refuses_ambiguity`; existing lease compile-fail doctests |
| F18 | README, grey box and Beacons logic/tests use the new API; explicit setup Follow replaces handwritten placement. | compiled README; game suites and proofs; counts and tick below |
| F19 | Closed-form saved Tween has a seconds-based duration, continuous retarget and known deadline. Beacons uses it. | spring/tween test at 30/60/120 Hz with round-trip and retarget; Beacons glow test and browser proof |
| F20 | `Stick::wasd().or_arrows()` and `input.stick_xz("move")` replace repeated mappings. | relative-input equivalence test; both games' movement tests |
| F21 | Follow accepts Entity or name, caches name resolution, checks handle generation and re-resolves after death. | Follow regression covers both target forms and replacement; both games use spawn's handle |
| F22 | Restore names an unregistered type and `world.register::<Projectile>() in setup`; README gives the fix. | `unregistered_saved_type_names_the_setup_fix` also proves registered restore succeeds |
| F23 | Asset mesh refusal reaches the surface error with its asset name. | `asset_mesh_is_a_named_surface_refusal`; GPU `asset_refusal_reaches_the_surface_error_with_its_name` |

For direct before/after evidence, a small regression executable linked against the
preserved pre-change library failed all sixteen cases: fresh, live, restore,
parent, exists, resource, reasons, read-backoff, queued, future, named-args,
float-bits, canonical-args, zero-follow, current-follow and finite-ease. Linked
against the replacement library, all sixteen pass. Logs and source are in
`target/g1b/regressions*`. The new-API tests cannot compile against the old API;
the table does not claim every new test executable was run unchanged on baseline.

Deleted: optimistic saved stillness; positional saved argument values and rounded
argument JSON; dimension-keyed geometry/cache limits and asset-as-cube fallback;
plane trimesh construction; Beacons lit/glow/age arithmetic and duplicated camera
pose; both games' position/key/time test helpers; unnamed singleton assertions;
second lookup for held-key release; receipt existence markers; voice motion
reporting that duplicated executor lifetime. No deprecated API or compatibility
shim was retained. The Sim envelope is now version 3; version 2 saves refuse.

## Mesh layout and choice

The preferred instance design stays. GPU material slots are still **12 f32 / 48
bytes**: RGBA 0–3, metallic 4, roughness 5, RGB emission 6–8, dimensions 9–11.
Ordinary shapes use XYZ scale; capsules use diameter, half-stem, diameter. The
reserved vertex UV carries cap sign/flag, so hemisphere scale remains uniform
and cap centers move independently. Both forward and shadow vertices apply it;
normals use inverse dimension scale.

At 200k slots: materials remain 9.6 MB and two transform histories remain 16 MB.
The Feed additionally retains 2.4 MB of CPU dimensions. There is no GPU slot-size
increase, dimension cache, eviction policy or exhaustion threshold. Five thousand
distinct sphere radii plus 4,100 animated revisions retain one mesh and one draw per geometry pass (shadow cascades draw again).
Dimensions still cost material uploads and feed work when changed; “free” means
no new geometry or draw groups, not zero CPU/GPU work. This capacity/batching
result is clearly better than the static-only fallback; the measurements below
do not establish a static-scene speedup.

## Size and Beacons tick

Physical lines including comments/blank lines, against `f44f5df`:

| Beacons file | before | after |
|---|---:|---:|
| `logic/src/lib.rs` | 169 | 151 |
| `logic/tests/sim.rs` | 105 | 129 |

The test file grew: direct named component assertions and typed argument literals
wrap across more lines than the deleted position helper. Gameplay has fewer
state fields and no glow clock arithmetic. Its actual tick is:

```rust
fn tick(world: &mut World, input: &Input, _: &Self::Args) {
    let dt = world.dt();
    let desired = input.stick_xz("move") * 4.0;
    let mut position = Vec3::ZERO;
    if let Some((player, pose)) = world.query::<(&mut Player, &mut Transform)>().one() {
        player.velocity.x = math::ease(player.velocity.x, desired.x, 0.074690334, dt);
        player.velocity.z = math::ease(player.velocity.z, desired.z, 0.074690334, dt);
        if input.pressed("jump") && pose.position.y <= 0.9 {
            player.velocity.y = 4.852216; // sqrt(2 * 9.81 * 1.2)
        }
        pose.position.x += player.velocity.x * dt;
        pose.position.z += player.velocity.z * dt;
        if pose.position.y > 0.9 || player.velocity.y > 0.0 {
            pose.position.y += player.velocity.y * dt - 0.5 * 9.81 * dt * dt;
            player.velocity.y -= 9.81 * dt;
            if pose.position.y <= 0.9 {
                pose.position.y = 0.9;
                player.velocity.y = 0.0;
            }
        }
        pose.position.x = pose.position.x.clamp(-19.6, 19.6);
        pose.position.z = pose.position.z.clamp(-19.6, 19.6);
        position = pose.position;
    }
    let mut count = 0;
    for (mut beacon, pose, mut material) in
        world.query::<(&mut Beacon, &Transform, &mut Material)>()
    {
        let delta = position - pose.position;
        if beacon.glow.target == 0.0
            && input.pressed("act")
            && delta.x * delta.x + delta.z * delta.z <= 1.5 * 1.5
            && position.y <= 0.9001
        {
            beacon.glow.to(world.now(), 1.0, 0.5);
            world.log("beacon lit");
        }
        count += u32::from(beacon.glow.target == 1.0);
        material.emissive = [beacon.glow.value(world.now()) * 3.0; 3];
    }
    world.publish("beacons", count);
    scene::follow(world);
}
```

## Measurements

Release, arm64 M5 Max, shared machine. Baseline binaries were preserved before
editing; paired measurements alternate them with current binaries, without
swapping source or cloning. Churn reports medians of three runs per version:

| churn measurement | before | after |
|---|---:|---:|
| rotation 100k, ns/entity/tick | 2.86 | 2.31 |
| rotation 500k, ns/entity/tick | 2.97 | 2.13 |
| respawn 1k/100k, ms | 0.328 | 0.245 |
| respawn 50k/100k, ms | 15.079 | 10.995 |
| construct 12-row query, µs | 0.011 | 0.008 |
| construct + iterate 12 rows, µs | 1.161 | 0.891 |
| propagate 500k roots, µs | 0.005 | 0.003 |
| propagate 500k / 50k parented, µs | 1864.100 | 2262.617 |
| despawn 1k among 50k parented, ms | 0.151 | 0.152 |
| reap 5k orphan descendants, ms | 1.593 | 1.978 |

Parent propagation and orphan reaping are slower in this series; these numbers
are not a claim that every path improved. The Linux builder's current rotation
cost is 5.74 ns at 100k and 5.79 ns at 500k (no fresh Linux baseline).

Final four alternating 480-frame cubes runs, 200k entities, 2560×1440, 4×MSAA,
live clock; median of each run's p50:

| measurement | before | after | range of run p50s, before → after |
|---|---:|---:|---|
| simulation, ms/tick | 0.7749 | 0.7917 | 0.7043–0.8100 → 0.6291–0.9377 |
| feed, ms/tick | 1.6216 | 1.6541 | 1.5169–1.6818 → 1.3741–1.9064 |
| CPU encode, ms/frame | 0.1564 | 0.1654 | 0.1503–0.1615 → 0.1245–0.2056 |

Feed is +2.0%, CPU encode +5.8% in this final series, with overlapping ranges.
The frame metric is CPU scene selection/encoding/submission, **not GPU completion
latency**; the example waits for GPU completion outside its timing. Earlier six
paired runs gave feed 1.1033→1.4725 ms and encode 0.0746→0.1087 ms. The last series
followed an inline annotation on the shared query mask scan, but concurrent
machine load also changed, so it cannot establish the annotation caused the
improvement. All series are retained; no speedup or “no regression” claim.

One stillness observation, Transform-only entities, 100 warm samples (ms):

| entities | median before → after | p95 before → after |
|---|---:|---:|
| 1k | 0.116500 → 0.119000 | 0.144791 → 0.214875 |
| 10k | 1.177417 → 1.015250 | 1.314750 → 1.226334 |
| 200k | 24.507791 → 16.061625 | 29.115500 → 20.996709 |

These runs were not interleaved, so they establish cost, not an isolated speedup.
A seek observes twice plus comparison; live advancement observes zero times.
The executor opt-out test panics if observation serializes its resource; it
passes, as does the existing exactly-two-samples/zero-live test.

## Proof and parity

All following hashes agree across native arm64 macOS, native x86-64 Linux builder
and Chrome wasm. The tests, snapshots and proof pins are updated.

| game / instant | macOS = Linux = Chrome wasm |
|---|---|
| greybox setup | `0619b31ec44b9156` |
| greybox W1500 | `a6449de82e10c54c` |
| Beacons setup | `c9b9da5a6a813a7b` |
| Beacons W1500 | `c483599688164cb8` |

The churn hashes also agree on Mac/Linux: 100k `7436348051b0a582`, 500k
`120ad65324678b6f`. Both browser proofs continue the entire Sim save byte-for-byte
across fresh sessions. Final proofs: greybox web 21.271 s; Beacons web 12.595 s;
greybox macOS 60.278 s. Each reports zero failures and zero recorded children left.

The game workspace builds, tests, clippy with all targets and warnings denied,
and fmt check pass locally with `EXACT_UPDATE_TRUST=development`. The full game
suite reports 237 passed, zero failed, five ignored diagnostics; Linux reports
the same, with unavailable GPU cases explicitly skipping. The README runs as a
doctest. The singleton refusal also passes in release. Bun's five driver/proof
regressions pass (20 assertions). Staged caps pass: 559 source files scanned,
none over 1,500 lines; staged whitespace verification is clean. Logs are in
`game/target/g1b/`.

The Mac's newly installed Xcode has an unaccepted license; local commands used
`DEVELOPER_DIR=/Library/Developer/CommandLineTools`. Native proof initially failed
because this Swift toolchain defaults to a build system with a different product
path. A temporary command wrapper supplied `--build-system native`; it was
removed after the passing proof. No host source was edited. Fresh local binaries
ran during this pass; no exec-policy timeout required killing a process. Linux
was used for the explicitly requested cross-architecture verification and has
no task left running. iOS was not requested or verified.

## Review corrections and API details

The reported behavioral defects were real. Rapier's `Executor` is actually a
field inside the `Physics` resource, so that containing resource opts out.
Consuming query rows need to carry leases: returning bare references from an
owning iterator would let rows outlive its borrow protection. Thus item loops
use `mut beacon`/`mut material`; `.iter()` retains plain references and Entity.
The escaped-row regression proves column access stays locked until rows drop.
Gameplay tests and the README no longer construct wire Values; low-level tests
of malformed host arguments necessarily still exercise Values and from_values.

# G1c — final quiescence round, stopped with a counterexample

2026-09-17. Read the current `b0af526` tree (repairing `a0f4ca0`), the binding
rules, this diary and the game README. One agent; no clone, stash or commit.
LLP 1041.001 is the interface rationale, still a Draft outside the working set;
this brief and the binding rules determine the changes. No LLP hand-off is needed.

**Quiescence is not closed.** After the requested repairs, the final audit ran:

```rust
let mut s = Sim::<Still>::new(Options::default()).unwrap();
assert!(s.settle());
s.world().publish("score", 1);
assert!(!s.quiescent()); // FAILS: true, with an unchanged mutation epoch
```

`World::publish` changes the public record through its own `RefCell`, outside the
storage epoch. The public record is also outside the observation. Per the explicit
third-round stop rule, no further quiescence patch was made. The ignored regression
`remaining_publication_mutation_requires_an_observation` in `engine/tests/g1c.rs`
is the executable counterexample, not a passing claim. Run it with
`cargo test -p exact-game --test g1c remaining_publication_mutation -- --ignored --nocapture`.
`target/g1c/final-audit.txt` records its failure. This is the remaining decision for
the author, rather than a fourth repair loop.

## Requested repairs and tests

All named Rust regressions below are in `engine/tests/g1c.rs` unless specified.
The initial unmodified implementation failed all eleven original runtime cases,
three new Args compile-fail doctests, and the two new Bun regressions. Logs are
`target/g1c/before-{tests,doc,bun}.txt`. A twelfth passing runtime test separately
checks structural epochs, message invalidation and cached/canonical hash agreement.

| Brief | Change | Regression |
|---|---|---|
| A1 | One shared `Cell<u64>` at the storage boundary: mutable component/query/resource/RNG leases, structural edits, load and teleport advance it. Busy also invalidates. Sim detects external epoch changes and resets back-off. See the remaining publication escape above. | `leases_invalidate_observation_and_restart_backoff`; `storage_epoch_and_fused_hash_cover_structural_edits_and_messages` |
| A2 | RNG participates in observation as `resource.Rng`, retaining its original save/hash position. | `rng_draws_are_observed_without_changing_hash_format` |
| A3 | Paused plus empty queue remains quiescent and reports `["paused"]`; unpause requires observation. README states the rule. | `pause_explains_unobserved_rest_and_unpause_requires_a_sample` |
| A4 | Parented markers read the propagated global pose even without their own Transform. | `parent_only_marker_observes_ambient_conveyor` |
| A5 | Moving reasons stream and stop at eight; changing receives the existing quiescence result. The final component observation feeds the canonical world-hash prefix in the same serialization pass. Hash replies reuse that prefix at the same mutation epoch. Resources remain lazy so settling alone never snapshots an ambient physics/audio executor; observed resource values still have their own observation serialization. | `clock_streams_eight_reasons_and_serializes_each_row_only_twice`: 200 component rows, at most nine moving probes including the quiescence probe, 400 writes including the reply instead of 600. Existing `ambient_resources_are_never_serialized_by_observation` and two-sample/zero-live tests pass. Timing is below; no speedup claim. |
| A6 | Helper follows the host sequence exactly: initial read plus at most fifteen follow-up advances, sixteen rounds total. | `helper_matches_hosts_initial_read_and_fifteen_followups`: the tick-1386 final-round deadline succeeds; work needing round seventeen refuses identically. |
| B7 | Derived `Args::check_scalars` runs before domain validation on new/bind/restore paths. Validated canonical argument text is retained for save, removing the argument `expect`. | `typed_scalars_refuse_before_domain_validation` (NaN, both float widths, infinities, unsafe signed/unsigned 64-bit integers) |
| B8 | Bound restore decodes/validates only current bindings; original argument text remains the restoredFrom record. Plain restore still validates the saved args. | `bound_restore_validates_only_the_arguments_it_uses` |
| B9 | `live` outside a field refuses, including bare, list and name-value forms. | Three Args compile-fail doctests in `engine/src/args.rs`; existing malformed field attribute cases remain. |
| B10 | Tick-to-host-microsecond conversion is checked and saturates to `i64::MAX`. | `extreme_tween_deadline_saturates_host_microseconds` |
| B11 | Named Follow targets use `named()`'s current lowest-index result; changed identity reinitializes following. | `follow_names_track_lowest_live_index` |
| C12 | Project the previous frame's up onto the new view plane; choose a fixed axis only when it degenerates. | `follow_roll_transports_continuously_through_vertical`: consecutive rotations stay below 0.05 radians; baseline jumped by π. |
| C13 | Browser key-down returns a carrier-owned release closure capturing device identity; finally invokes it without resolving/refocusing a canvas. The same closure is retained if delivery/frame completion throws. | `proof.test.mjs`: `held key release survives removal of its canvas` runs the actual browser-key carrier function against a removed-canvas fixture; existing partial-failure transcripts also pass. |
| C14 | Receipt hashes a sorted path/content manifest for the entire bundle and actual standalone product directory, covering dylibs, plan and assets. | `proof.test.mjs`: `native receipt changes with game dylibs and embedded plan/assets`; existing executable/carrier/deletion test remains. |

## Clock cost

Release arm64 M5 Max, 200,000 Transform-only entities. One operation includes a
17 ms seek, its two observations, and the complete clock reply with world hash.
Five warmups then 100 samples per run. The baseline test executable was preserved
before editing; paired runs alternate that executable and the replacement.

| series | median ms before → after | p95 ms before → after |
|---|---:|---:|
| first runs | 83.912583 → 86.142125 | 91.631708 → 95.698708 |
| alternating pair 1 | 96.580583 → 111.107500 | 129.609667 → 132.196083 |
| alternating pair 2 | 142.524542 → 119.748500 | 197.928833 → 150.332834 |
| alternating pair 3 | 89.109750 → 158.901417 | 103.093500 → 185.074166 |

Median of the three alternating run medians: **96.580583 → 119.748500 ms
(+24.0%)**. The shared machine was also building/running the benchmark lane and
proofs, and the spread is large. These results establish the observed cost and
regression, not an isolated cause or a speedup. Reduced serialization/reason
visits do not establish reduced wall time. Raw logs: `before-cost.txt`,
`after-cost.txt`, `paired-cost.txt` in `target/g1c/`.

## Hashes and proof

All four hashes agree bit-for-bit across native arm64 macOS, native x86-64 Linux
builder, and Chrome wasm; both game proof pins and greybox's state snapshot were
updated. Seed 7, run 0, unpaused, W held for 1,500 ms:

| instant | G1b | G1c: macOS = Linux = Chrome |
|---|---|---|
| greybox setup | `0619b31ec44b9156` | `9d8e9359b9f3e65f` |
| greybox W1500 | `a6449de82e10c54c` | `2464f19d35fb4996` |
| Beacons setup | `c9b9da5a6a813a7b` | `32b48c41f24f8fcf` |
| Beacons W1500 | `c483599688164cb8` | `d17e623e56fb8dc9` |

Only Follow's camera calculation changes these games' saved state: initial up
projection/normalization and subsequent transported up change floating-point
rotation/scale bits. RNG is still serialized exactly once in its original location;
its added observation, epochs and hash prefix cache add no saved/hash fields.
Cached and uncached save/load hashes agree. Gameplay position remains
`[0, 0.9, -5.7333384]`. `parity-{mac,linux}.txt` and the proof transcripts carry
the complete cross-platform cards.

Proofs passed: greybox web **16.400 s**, Beacons web **21.239 s**, greybox macOS
**63.787 s**, zero failures and no recorded children left in each. Both browser
proofs continue whole Sim saves byte-identically in fresh sessions. The native
proof retains its pre-existing screenshot-permission and browser-metrics skips.
It used `DEVELOPER_DIR=/Library/Developer/CommandLineTools` and a temporary Swift
wrapper adding `--build-system native` only to `swift build`; the wrapper was
removed. An initial wrapper that also changed `swift package describe` failed
and is recorded separately. No exec-policy timeout occurred; Linux was used for
architecture parity, and no task was left running there.

Workspace build, clippy (`--workspace --all-targets -- -D warnings`) and fmt pass.
The second full `cargo test --workspace --no-fail-fast` run reports **262 passed,
0 failed, 7 ignored**: five existing diagnostics, the new cost diagnostic, and the
explicitly unresolved publication reproducer. Bun's seven driver/proof tests pass
(25 assertions). The first workspace run exposed stale hash pins, the float-text
test assertion, and concurrent benchmark/renderer failures; it is retained rather
than presented as passing. The initial workspace build briefly failed launching
the shared filesystem helper; the second build passed without a source change.

This worktree was clean at start, but an independently running benchmark task
added/edited `game/bench`, the workspace manifests, Environment, renderer files
and the native host during the run. Those edits were preserved. They are included
in the shared workspace verification, and are not G1c changes or work delegated
by this agent. After `git add -A`, caps passed (577 source files, none over 1,500
lines) and the staged whitespace check passed. No commit was made; HEAD remains
`b0af526`.

## Review corrections

The listed behavioral defects reproduced. The extreme deadline error was in
Sim's narrowing conversion to host microseconds, not Tween's already-saturating
`u64` deadline. The vertical test exposed a 180° discontinuity, larger than the
review's approximate 90°. The host bound is sixteen rounds, but only fifteen
world follow-ups after the initial read. The requested cases are repaired; the
publication counterexample prevents claiming quiescence complete, and the clock
measurements prevent claiming a performance improvement.

# G1d — closing the remaining quiescence edges

2026-09-17, `lane/game`. One agent, no clones, stash or commit. The mutation
sample rule is unchanged. The remaining changes and their tests are:

| Change | Regression |
|---|---|
| Document the journal as telemetry, outside observation, hash and mutation epoch. | `g1d::logging_refusal_preserves_rest_and_hash`: a malformed agent read adds a journal line while preserving epoch, rest and hash. This behavior already passed before the documentation change. |
| Document semantic Data's prohibition on interior mutability on Data, Component, Resource and in the README; manual implementations violating it have undefined quiescence/hash-cache behavior. No enforcement added. | Data's new compile-fail doctest shows that deriving a semantic `Cell<u32>` field is already refused. |
| Apply the Ambient presence mask to erased moving, moving-reason and deadline scans; resources receive no entity mask. | Three `g1d::ambient_*` tests: a live ten-second Tween neither blocks rest, appears in changing, nor selects settleAt. All three failed before and pass after. |
| Report both paused and queued input. | `g1d::paused_queued_input_explains_why_it_is_not_quiescent`: queue a future key before pausing; failed before, passes after. Input delivered while already paused intentionally updates held state directly. |
| Await asynchronous step tags and retain the native carrier's release closure. Swift stores the original surface/module identity; release bypasses view resolution and focus, and cannot deliver to a replacement. | `proof.test.mjs`: asynchronous tags survive JSON serialization on success and clock failure; native canvas-removal carrier fixtures fail before and pass after. Browser canvas-removal coverage now includes both successful and failed clocks. These carrier fixtures exercise the actual JS key functions with device/host seams supplied by the fixture; the macOS proof also exercises the real Swift held-key path. |
| Hash all web dist paths/content in sorted order. | `proof.test.mjs`: asset changes, additions, deletion/rename and creation-order independence. Failed before, passes after. The new standalone-game-dylib receipt test also verifies cache misses with unchanged source inputs; native whole-bundle/product hashing was already correct. |

Validation, with `EXACT_UPDATE_TRUST=development`, all local:

- Game workspace build; `cargo test --workspace --no-fail-fast`: **272 passed,
  zero failed, six ignored diagnostics**, including 19 exact-game doctests.
- Workspace clippy, all targets with warnings denied; fmt check.
- `bun test game/proof.test.mjs`: **13 passed, 52 assertions**; existing driver
  transcript fixture matches. Both held-key step receipts contain resolved tags.
- Greybox web **13.872 s**, Beacons web **15.437 s**, greybox macOS **56.954 s**:
  zero failures, no recorded children remaining. Existing hash pins unchanged;
  browser proofs continue whole saves byte-for-byte. macOS retains its existing
  screenshot-permission and browser-metrics skips.
- Staged caps and boot pass; staged whitespace check passes. Swift used a temporary
  wrapper adding `--build-system native` only to `swift build`; it was removed.
  No native Cargo step wedged, so the Linux builder was not needed.

Logs are in `game/target/g1d/`. An additional, non-required attempt to run the broad
`scripts/caps.test.mjs` harness under `bun test` produced empty-output fixture
failures and was stopped, including its recorded child; it is not a passing
result. The direct required caps check passed. No unrelated process was stopped.


**F2b — 2026-09-18.** Live ticks now run when their deadline is strictly before
the next frame, using the last live delta capped at one step; the 250 ms catch-up
clamp, pause and seekable boundaries remain. Live scheduling retains frame
precision to avoid microsecond-rounding beats at matching rates; synthetic
3,600-frame runs cover 60, 59.94 and 120 Hz, including the necessary occasional
extra 60 Hz tick at 59.94 Hz. Alpha now clamps the fraction between the previous
and current tick, and live saves accept one early tick. Feed, trace and camera
history already use that fraction; audio transport reads completed ticks directly,
without alpha. Live input follows host delivery time; the deterministic record is
tick-stamped. The exact probe and feel table now report mean alpha on tick-running
frames as `tick_phase`, without changing the latency metric. All 22 simulation
tests, all-target clippy and 15 feel tests pass; the full engine/render retry still
has three renderer assertions for the old timing in `trace.rs` and
`surface_tests.rs`, left unchanged pending permission to extend the brief's file
scope. Workspace fmt remains blocked by concurrent audio/GPU edits, including
`restore_bound`, which was not touched. Beacons web and both Linux-headless game
proofs pass with unchanged pins; Greybox web's build fails in the concurrent GPU
wasm macro even after the prescribed one-minute retry. The feel preflight found
the console active (11.6 s idle, requiring 30), so no attempt or latency/tick_phase
result was taken. Logs are `game/target/f2b-*.log`; no commit was made.


**F2c — 2026-09-18.** Supersedes F2b's held-frame alpha and live input bypass.
Render time is `R = T + L - step`; alpha is `(T + L) / step - tick`.
The Sim retains the lookahead used by the last advance, resets it for seekable
advances, and excludes it from saves/hashes. Boundary/interpolation arithmetic is
integer microseconds multiplied by HZ before rounding (1,000,000 units per tick),
so rational frame periods retain sub-microsecond precision without equal-rate
beats. The signed alpha numerator is guarded only for missing history/transitions;
steady live frames stay within the available tick pair. Seekable interpolation
returns to its original fraction. Input again obeys `stamp < deadline`: the live
bypass had put an 18 ms input into tick 1 during catch-up instead of tick 2.

The three renderer expectations, with one world unit per 60 Hz tick:

- Seekable frames T = 17, 25, 34 ms have L = 0. R = 1/3, 25/3, 52/3 ms,
  so x = 0.02, 0.5, 1.04. The old expectations are correct; [1, 1, 2] held poses.
- Live 144 Hz frames have T = n * 1000/144 ms and L = 1000/144 ms.
  R = (n + 1) * 1000/144 - 1000/60 ms; x = (n + 1) * 60/144 - 1 after
  startup, a constant 5/12-unit delta. At the 1100 ms burst, L is capped to
  one step, R = 1100 ms and x = 66, with history [65, 66].
- Live frames at 17 and 34 ms cap L to one step, so R = T and x = 1.02, 2.04.
  Completed ticks are 2 and 3; the second frame's history is [2, 3]. A same-tick
  edit to x = 5 advances that history to [3, 5]; teleport still snaps both to 20.

Regression coverage:

- `alpha_guard_never_fires_on_600_steady_frames_at_both_world_rates`: inspect the
  signed numerator before clamping across 600 frames, 60/59.94/120 Hz displays,
  60/120 Hz worlds, four phases and three epochs; seekable clears lookahead.
- `live_half_step_frames_draw_equal_deltas_at_every_phase`: 600 frames at 120 Hz
  over a 60 Hz world, phases 0, .1, .25, .4 steps, deltas .5 within 1e-6.
- `live_stall_returns_to_steady_deltas_after_one_recovery_frame`: 40 ms stall
  followed by 60/120 Hz cadence; after the single L-reset frame, deltas are steady.
- `live_three_tick_catchup_spreads_input_by_stamp`: down at 18 ms lands in tick 2,
  up at 35 ms in tick 3 during a 40 ms catch-up; tick 1 receives neither.
- `live_input_after_deadline_is_drawn_one_frame_before_zero_lookahead`: input at
  step + .01 ms lands in tick 2 on both clocks; first partial x = .25 is drawn at
  T = 1.75 steps live versus 2.25 steps without lookahead, one 120 Hz frame earlier.
- Updated horizon-alpha, future-input, restore/pause/seekable tests pass; the
  3,600-frame no-beats test and all three renderer regressions pass.

Validation (required Cargo environment set throughout):

| Check | Result |
| --- | --- |
| `cargo test -p exact-game -p exact-game-render --no-fail-fast` | 263 passed, 0 failed, 7 ignored diagnostics; includes all 26 simulation integration tests |
| `cargo clippy -p exact-game -p exact-game-render --all-targets -- -D warnings` | pass |
| `rustfmt --edition 2021 --config skip_children=true --check` on the four scoped Rust files | pass |
| `bun test bench/feel.test.mjs` | 15 passed, 0 failed, 91 assertions |
| Beacons web / Linux headless proofs | pass, 54.446 s / 160.890 s |
| Asset-fixture web / Linux headless proofs | pass, 44.614 s / 146.506 s |
| Scoped `git diff --check` | pass |

All four proofs retained their pins and exited all recorded children. The asset
hash stays `0x8f6d518f39634478`; Beacons' continuation remains byte-identical.
`game/proof.mjs` is an imported helper, so the actual proof entry points were
`bun game/games/{beacons,asset-fixture}/proof.mjs {web,linux}`.
After rebuilding Beacons web, the single invocation of
`bun game/bench/feel.mjs exact --hz 60 --no-build --attempts 1` stopped in preflight:
console active, HID idle 19.4 s, requiring 30 s. No measurement attempt or retry;
judder, latency and `tick_phase` are unavailable for this revision. `tick_phase`
remains mean alpha on ticking frames; near 1 means just before the pose is needed.
Logs: `game/target/f2c-*.log`. No staging, commit, clone or sub-agent; the concurrent
builder's `restore_bound`, audio, surface and GPU changes were left intact.

S3a-b made asset readiness a useful boundary: a model names its textures, the renderer finishes their uploads and winding variants before a declaration becomes Loaded, and a failed name stays inspectable without freezing unrelated pixels. Cosmetic pop-in is explicitly outside simulation reads; authored bounds keep picking stable. The fixture now saves at tick 30 and reopens with the same hash and forwarded key state, while its web probe inspects the pending restore below the agent settlement barrier. This turns the declaration list into a promise about both deterministic setup and when asset GPU work finishes.


**F2d — 2026-09-18 (host connection pending).** The scheduler now uses a supplied display period for both `T + L` and `R = T + L - step`, so a stall and its first recovery frame have `ΔR = ΔT`; its integer µs×hz accumulator keeps only a bounded fractional residual, rejects raw backwards samples before mutation, and slews the origin by at most 0.25% until the grids meet, then holds. Aligned live input delivered after the preceding frame reaches the deadline tick; tick_phase is 1 at 60/60 and .5 at 120/60. Early saves and Live→Seekable catch the clock up to the completed tick's deadline without another tick; restore again enforces exact tick/time agreement, and unpause discards the pause gap. The full engine/render run passed 278 tests with 7 ignored; the final 31 simulation tests also pass, including the added visible-latency regression. All 34 web tests and the four requested application proofs pass with unchanged pins (temporary empty Beacons/Greybox art directories worked around the concurrent baker's missing-input dependency and were removed). Combined clippy is blocked by the concurrent range loop in render/src/models.rs; the engine-only all-target run also reports field_reassign_with_default in engine/tests/asset_validation.rs. Workspace fmt reports concurrent asset hunks, which were left alone; scoped test/trace formatting and caps pass (staged and unstaged using a temporary index). The pacer retains its fitted period through stalls and replaces it only when a new fit differs beyond 1% sampling noise. Frame/ABI and Apple host wiring still require permission to extend the brief's named-file scope; production hosts therefore still supply no period, and these proofs do not establish live F2d feel. The console preflight found HID idle 0.1 s, below 30 s, so no feel attempt or judder/latency/tick_phase measurement was taken. README clock arithmetic and limitations are under “The world dev loop”; logs are game/target/f2d-*.log. No commit, clone or sub-agent.


**F2e — 2026-09-18.** Period changes now slew the shared scheduling/render horizon: with `R = T + L - step`, grid correction and `ΔL` share `0.0025 * elapsed`, so every drawn delta stays within 0.25% of the frame delta (plus integer rounding), including 0→60, 60→120, 60→144 and 120→60; the pose never reverses, and changes below 0.5% do not restart acquisition. Scheduling follows the slewed horizon so the retained two poses always cover R; a known period at the initial epoch seeds L before any preceding pose. The bound means 0→16.667 ms takes 6.667 s and 0→8.333 ms takes 3.333 s, rather than the brief's approximate 4/2 s. Duplicate stamps leave the negative-half-unit remainder untouched. Queued live device stamps clamp to the paced callback time while preserving order and catch-up spreading; Seekable future stamps still wait. The web publishes its sixteen-interval median (133 ms at 120 Hz, 267 ms at 60), feeds delta/k including skipped slots, reacquires after 32 consecutive skips and republishes rolling fits beyond 1%; gradual drift no longer repeatedly resets the fit on phase residual alone. Apple uses targetTimestamp minus timestamp, quantizes cadence classes with 1% hysteresis, and publishes a 120→80 ramp once; macOS includes the attached display's rate divisions. Final engine/render tests: 286 passed, 7 ignored, no failures (35 Sim integration tests, both renderer regressions included); web: 37 passed, 7,877 assertions; clippy all-targets passed. Workspace fmt reports concurrent audio files only; all four scoped Rust files and diff whitespace pass. Caps passed with the named files staged and unstaged in a temporary index. Beacons web/Linux (22.83/36.00 s), Greybox web (19.36 s), and asset-fixture web (10.42 s) proofs passed with unchanged pins and recorded children exited. Extracted Mac/iOS period methods passed jitter, ramp, midpoint hysteresis, invalid-sample and 60 Hz checks; Mac also passed 75/144/165 Hz and half-rate classes. The macOS ExactMac package built after temporary wrappers selected SDK 26 and Xcode's Swift with `--build-system native`; the first attempts exposed the build script's default-SDK override and the CLT SwiftPM framework mismatch. Early full Rust runs caught concurrent audio edits and a Seekable test timestamp rounded below tick 2; the final run is green. The single console check found HID idle 29.3 s, below the required 30 s, so no feel attempt was made: judder, latency and tick_phase are unavailable. Logs are `game/target/f2e-*`; no commit, clone or sub-agent.


**D2 — 2026-09-18.** Deleted the 25-line Rust export parser plus its obsolete comments: Rust compiles the generated `game_logic::Type` reference, including macro exports, while the package-name check remains. Render now re-exports the engine's `Environment` and carries exposure/bloom there once, deleting the duplicate type/default/copy; removed the bounding-sphere radius, extra vertex pass and accessor because only one test used them (transparent sorting still uses the center); replaced scene errors' invented mesh/slot/4096 capacity with `RenderError::Scene(String)` beside `Capacity { arena, slot, limit }`. Deleted `SAVE_VERSION`, `migrate`, their branches and the `Updated` fixture: no game used them and post-decode migration could not repair a schema; format/game-identity refusals remain. The page-write test now uses three full pages plus a partial page instead of 500,000 entities, and animated dimensions use seven spheres/five radii instead of 5,000 spheres/4,100 revisions; both invariants pass. Deleted Beacons' 9,494-line historical proof transcript because its executable proof is the authority. Audio's duplicate start/unlock operations were already merged at the starting revision; no second deletion was invented. The five eager 16,384-f64 diagnostic rings now arm only through `state` with `perf: true` or `perf_reset: true`, retaining small counts/totals/maxima and draw counters; the state-arming and cadence tests pass. A temporary native probe retained 64 unbound WorldSurface instances and exercised 16,385 writes per ring: resident growth fell from 41,840 KiB (653 KiB/surface) to 528 KiB (8 KiB/surface), about 645 KiB recovered per surface, consistent with the 640 KiB ring payload plus allocator rounding; the probe was removed. Also changed transparent sorting to unstable sorting: its unique slot tie-breaker already defines the order, so stable-sort scratch buys nothing. The ten author edges, before → after: (1) deferred exclusive-borrow sound playback → shared `world.play("chime").at(entity).start()` was already in Greybox; Beacons intentionally has no audio; (2) hand-drained dependencies and panic-on-save → `sim.load_assets(|name| read(name))?` in a sibling engine file and `sim.save()?` with pending/failed names, including load/refusal tests; (3) coordinated scalar string keys → ordinary `Data` HUD records via `publish_record`, including nested records/options/numeric lists, saved publications and existing Contract shape validation, with scalar `publish` retained; (4) unexplained `round` → `Options::restart_generation`, explained in the template/README and using the existing setup-argument reconstruction path (Greybox has no restart control); (5) local proximity → current global parent-chain positions, with entity order and mutation-safe copied poses preserved and tested; (6) three temporary JSON structs/formatted picking request → `sim.layout("player").unwrap().screen` and `sim.pick(rect.center())`, with an explicit viewport and Greybox's wire snapshots retained; (7) optional mandatory players → `query.one().expect("one player")` in both games/template; (8) repeated five Character defaults → only `.ground(0.9).bounds_xz(..)`; (9) AudioSource storage literal → `.new("wind").gain(0.3)` was already present in Greybox; (10) Beacons screenshot-save author calls → `s.world("world").save(path)`, preserving screenshot-save as transport. Both games, the template and READMEs teach the new APIs; the three false design-of-record statements were already corrected in the current READMEs (audio forwarders wired, period supplied, finite voices ambient). Logic/test/Contract/proof physical lines: Greybox 192/249/27/192 → 191/228/27/192 (660 → 638); Beacons 130/66/38/99 → 129/70/38/99 (333 → 336, including four new typed-spatial assertion/viewport lines). Beacons GPU wasm 816,348 → 762,435 bytes (−53,913); this is the existing pre-pass artifact versus rebuilt web artifact and includes concurrent S3 changes, not an isolated D2 byte attribution. Web/Linux proofs pass for both games with unchanged pins: Greybox 22.849/19.673 s, Beacons 23.584/31.565 s, byte-identical continuations and all recorded children exited. Engine/games/audio: 271 passed, 3 ignored; Contract surface-shape boundary: 8 passed; renderer core/world: 18/5 passed (3 core ignored), library 48 passed/2 ignored with one concurrent asset-refusal fixture failure (`castle` is now an invalid model name). Requested workspace test, clippy all-targets and fmt were run but not green: concurrent generated-asset manifest/type/formatting changes blocked workspace checks, and the new save Result needs five mechanical calls changed in three out-of-scope physics example/test files (patch `/tmp/d2-physics-callers.patch`, approval requested and pending). Generator/proof tests reached the three-round stop: the bare-name command selected three test files and returned 24 pass/1 fail (empty spawned metadata output); explicit file paths ran the generated game's three native tests successfully, then its web build hit the concurrent model Mat4/iterator error, yielding 19 pass/1 fail. No fourth retry; logs are `/tmp/d2-*.log`. Production-library clippy passed for engine/render/audio/both games. Caps passed with only D2 files/hunks staged in a temporary index, then unstaged; scoped formatting/diff checks pass apart from other builders' hunks. No commit, clone, stash, sub-agent or permanent measurement apparatus.


**F2f + AU3e — 2026-09-18, closing round.** Apple now shares one three-interval cadence quantizer across macOS/iOS and republishes each session's class before every callback's render (zero until acquired); regressions cover initial publication, rate boundaries, 120 ↔ 80 hysteresis, dropped intervals and alternating 60/120 Hz sessions. The web pacer reacquires materially short intervals at raw monotonic time while retaining the fit across an isolated sub-slot callback; every transition delta and the restored 0.02 ms jitter bound pass. The live clamp skips the restore/epoch sample, with a saved future-stamp regression. Player leaves non-preferred sources stopped while a preferred source waits for Apple's real Stop acknowledgement; the A/B/C priority regression fails without that suppression. Apple lifecycles preserve process-wide no-resume state for later sessions, distinguish automatic audio requests from gestures, refresh on ExactView window attachment/removal, and retry failed activation after 300 live frames; corrected window fixtures fail against the old lifecycle and pass the new one. Attached sources now refuse non-looping definitions by name: finite World::play voices already own deterministic start ticks, so no extra source activation state was added. Removed the redundant registration-budget check/claim (valid definitions are already bounded to 60 seconds); Player's aggregate, actual-rate reservation remains tested. README timing now says 133 ms at 120 Hz, unknown until display-link ticks after module creation, and about ten seconds for serialized unknown → 60 Hz horizon/grid acquisition. Game workspace retry: 402 passed, 8 ignored, no failures; root GPU: 19 passed; web: 41 passed; macOS Swift: 81 passed, and ExactMac builds with the temporary native-build wrapper and macOS 14 deployment target. Audio all-target clippy passes. Workspace clippy and fmt remain blocked after the allowed retry by concurrent asset/deletion edits (including engine/tests/assets.rs and the asset-gate formatting hunk in sim.rs); no out-of-scope fix was made. Beacons web (30.304 s) and Greybox web with EXACT_AUDIO_PROBE=1 (36.503 s) pass with unchanged pins and all recorded children exited. Asset-fixture web failed twice on its loading-carry expectation (null) and stale crate/0-srgb.tex path; its files were left to the asset owner. Caps passes with the named files staged in a temporary index and then unstaged. Owed items are recorded verbatim in both READMEs and summarized in QUEUE.md. Logs are /tmp/f2f-*.log; no commit, clone, stash or sub-agent.


**S3a-c — 2026-09-18, uncommitted and not ready to ship.** `game.assets: true` selects the concrete model-capable shell; primitive wasm has no model shader/decoder markers. The base gate retains names, dependency results and immutable declared simulation data; Loaded is content readiness, while device preparation can be invalidated and textures re-requested. Retirement drops delivery/answered names and allows respawn, bounded to 256 names; models allow at most 64 used textures, web concurrency is eight and host drains remain sixteen rounds. A deferred invalid save refuses once without poisoning the fresh world; pending carries never export old restore bytes, and committing a valid deferred restore retains its last texture through upload. Save and headless loading return named readiness errors. Carry overlays fresh audio registrations while Open retains saved Sounds, including runtime names. Surface bind/delivery are merged, and Apple clock replies retain errors, asset states and changing keys. Publication-only changes name `published.<key>`. Baking records output SHA-256 digests and refuses edited outputs before overwriting/pruning, including rename-then-replace; opaque/emissive mips use straight RGB and MASK/BLEND base colour uses alpha weighting with distinct output names. Mirrored entity-global asset transforms refuse by name because node winding batches are immutable; authored mirrored nodes remain supported. Material final bindings wait for all dependencies, rebatch scratch is retained and the normal cache follows live records. Failing-before regressions cover lifecycle, pruning, bounds, publication, cache and mip defects. Game workspace: 406 passed, 8 ignored, plus the additional replacement-device pixel test; GPU/Linux: 73 passed; both clippy checks and fmt, 41 web tests, caps on explicitly staged files followed by unstaging, and boot pass. The extra root workspace build was stopped after more than ten minutes blocked in Weatherlight build scripts, with only its recorded processes terminated. Asset-fixture Linux passes; web passes content recovery, restore, declaration refusal, clock/save naming and tick-60 hash `0x8f6d518f39634478`, but **post-device-loss pixels remain black** despite no WebGPU validation errors; the failing assertion is retained after three rounds. Native replacement-device pixels match. macOS full proof stopped after three SDK/Swift-toolchain failures; the standalone Swift consumable-texture regression passes and caches retain reusable fonts/images/models/shaders. Beacons and Greybox web proofs preserve their hash pins and byte-identical continuations. Helmet sample: model 885,206 bytes (gzip 437,936), five textures 22,369,762 bytes each. Beacons wasm 811,977 → 758,561 bytes (gzip 322,782 → 301,524), including concurrent D2 changes: model code is absent, but the roughly 490 KB target is not met. Three interleaved 200k-cube runs against the retained S3a-b binary overlap in all-moving, one-percent and still ranges; all-moving tick/feed/encode medians are 0.4342/1.0230/0.0723 → 0.4443/1.0524/0.0723 ms. Remaining web-pixel, size and macOS proof gaps are recorded in QUEUE; no commit, clone, stash, sub-agent or permanent measurement apparatus.


**S3b — 2026-09-18, resumed after ENOSPC; uncommitted.** A game declares `game.assets: true`, `Game::ASSETS`, and `Mesh::asset` plus `Animation::play`, `Blend::across`, or an all-Data `Animator`; it writes typed parameters and owns its Transform, while animation owns compact saved local poses. Markers/root contributions, direct two-bone IK, one named socket per owner, local-TRS GPU interpolation before hierarchy composition, and matching shadow skinning are implemented. Cuts: first matching edge/one transition per tick, f32/bool parameters, a frozen outgoing pose during fades, one socket, no layered/additive graphs or scripts; imported bones never become entities. `animation.rs` is 1,145 lines including tests and `skinning.rs` 435. Web/macOS/headless-Linux proofs pass in 17.650/71.887/47.283 s on this arm64 Mac: all 24 tick-60 world matrices match the checked-in pin; tick-60/tick-120 hashes are `0xa9033d749a82ebd4`/`0xb05ce95a6c799acf`; save at 45 and fresh-process restore produce byte-identical 13,849-byte final saves (SHA-256 `cae346719d1a1e3fc6b8eb3d22eb9ddfb5295811dd09f50297bd09451c69c526`). The 120-tick paranoid Sim round-trip, edited-blend Carry/Open, Fox-leg IK (<1e-4 reachable error, straight unreachable, exact zero weight), socket and GPU interpolation/allocation tests pass; 60 fixed additions pin f32 bits `0x3f7ffffb`. The resume fixes the marker assertion to account for world's existing journal time/tick prefix. The retained release CPU diagnostic reruns at 0.182969 ms per tick for 100 Foxes (earlier 0.177966); rebuilt Metal palettes are p50/p95 0.046875/0.052750 ms (earlier 0.025208/0.049875), and 300 warm pose packs allocate zero Rust objects. Three interleaved 200k-cube pairs have overlapping ranges in every mode; all-moving median tick/feed/encode is 0.4509/1.1572/0.1060 → 0.4640/1.2014/0.1208 ms, with all modes in render/README.md. Active Fox wasm is 945,320 raw/368,310 gzip bytes, 44,991 gzip above the retained pre-skeleton model fixture, but those are different games: the isolated ≤60 KB engine-growth claim remains unverified because the reference rebuild hits the concurrent wasm-bindgen macro error. SDK 26 plus the temporary native-build Swift wrapper launches macOS; its screenshot shows a white Fox with stride/shadow, while web is textured, leaving native asset/host work to R1. Pose JSON works through `session.op`; the literal CLI suffix still needs the out-of-scope driver forwarding patch. Workspace retry: 418 passed, 5 failed, 10 ignored; remaining failures concern cosmetic-save readiness, transformless layout, detached audio, and a material-arrival fixture. All-target clippy passed before later sibling edits; workspace fmt still reports sibling hunks, while the new skeleton/fixture Rust files pass scoped formatting. The requested bare-name Bun command also selects Greybox audio tests (23 pass/2 fail); two explicit-file attempts reach 19 pass/1 fail, first on the concurrent audio helper and then on `gpu/src/web.rs`'s unqualified wasm-bindgen attribute, reaching the three-round stop. Existing-game web proofs and same-app size rebuilds are likewise blocked by that macro error after the allowed retry; their hash pins were not edited. Caps passes with only the new skeleton/fixture files staged in a temporary index and then unstaged; the shared index is untouched. READMEs hold the author API, bounds inflation and ownership rules, measurements and remaining limitations. Logs are `/tmp/s3b-resume-*`, screenshots and cross-host saves are in the fixture's ignored artifacts directory. No commit, clone, stash, sub-agent, or out-of-scope repair.


**R1 — 2026-09-18, resumed after ENOSPC; uncommitted.** This round repairs the consolidation regressions: retiring A re-requests live dependencies and prepares retained textureless B before render; deferred restores keep their carriers until committed and report late refusal once on web, Apple and Linux; pruning requires prior ownership and migrates matching legacy manifests without deleting unknown bytes. Failed cosmetics no longer block saves, headless readiness follows current mesh roots, primitive bind retries keep refusing, Apple binds use the session clock, and web delivery streams enforce 64 MiB before copying with bounded, cancellable queues. Reachable meshes determine used textures. Record publication defines unit/optional-unit behavior and safe integer boundaries; typed and JSON layout share identity-pose behavior; a focused same-ID restore test preserves new action bindings. Perf arming preserves aggregates/cadence and the feel probe arms its percentiles. Spatial audio authoring holds a mutable Transform safely, despawn retains the last voice position, and finite AudioSource refusal is named and nonpanicking in release. Apple rate classes respect the display maximum and republish without callback allocations; jittered web 60 ↔ 120 transitions stay on the provisional lattice. No-resume recovery reaches every live lifecycle and retains the first gesture before audio is requested. Game workspace: 425 passed, 10 ignored; GPU/Linux: 76 passed; both all-target clippy and fmt checks pass. Web: 48 passed; the requested app/new/proof Bun suite: 33 passed, one skipped. Beacons, Greybox with EXACT_AUDIO_PROBE=1, and asset-fixture proofs pass on web and Linux with unchanged pins; the asset fixture includes a real deferred-refusal check. The macOS Swift package product builds and 85 XCTest cases pass using Xcode's runner after compilation through the temporary native-build wrapper; the Command Line Tools runner could not launch XCTest. Caps passes with explicit R1 paths staged and unstaged in a temporary index, leaving the shared index unchanged; boot and diff checks pass. Module-level replacement-device pixels remain identical; **host recovery is owed**, including a native replacement-device ABI preserving surface tables and recreated web canvas surfaces/contexts. READMEs and QUEUE retain the review file:line evidence for host recovery, Apple's delivered texture bytes, primitive size isolation, real-device interruption/multi-display sweeps and WebAudio resume-failure propagation. Synthetic Swift fixtures call interruption directly on main and do not post background AVAudioSession notifications. R1 leaves the concurrent skeleton implementation and fixture intact and adds no permanent apparatus, commit, clone, stash or sub-agent.


**D3 — 2026-09-18, uncommitted.** `bun game/bench/size.mjs` now builds Beacons, measures shipped bytes/gzip-9 and aggregates `twiggy top -n 20000 -f json` by crate and engine/render module from the same per-game target; the benchmark README records every cumulative cut. A fresh baseline was 829,372 / 329,049 bytes, the storage/formatting/vector/shader/array cuts reached 737,768 / 315,604, and the measured final tree with exact float spelling and concurrent skeleton changes is 764,322 / 325,721; the <550,000-byte target is **not met**, and the last delta is not isolated D3 growth. Aligned erased pages and registration-specific component/singleton factories reduce pre-opt storage attribution from 126,466 to 34,240 bytes; typed strides, leases, masks, generations, hashes and save/carry behavior stay covered. Asset delivery maps use sorted vectors; saved ordered maps remain ordered. Shader packaging removes comments/indentation and 3,552 `.rodata` bytes while preserving line numbers; columns change. JSON/perf/trace/audio share small-table Ryu with exact decimal tie handling, checked against 100,000 f32/f64 samples; publication journals intentionally use Data fields instead of Debug (`Number(0.0)` → `{"Number":[0.0]}`), changing diagnostic strings in saves but not world hashes. Core float formatting still links through dynamic clamp panics, including protected animation code; allocation, remaining Data walks and formatting are the next size work, not a coarser model split. Three interleaved 200k-cube pairs overlap in every phase/mode; all-moving tick/feed/encode medians are 0.4273/0.9675/0.0653 → 0.4214/0.9537/0.0664 ms. Workspace tests: 446 passed, 10 ignored; all-target clippy, workspace fmt and boot pass. Final Beacons/Greybox/asset-fixture/skinned-fixture web proofs pass in 226.611/21.601/13.644/15.976 s with byte-identical continuations. The first three retain starting pins; the skeleton builder changed the skinned pose JSON and tick-120 hash from `0xb05ce95a6c799acf` to `0xcae264dd3df5267b`, and D3 edited none of those pins/files. Caps passes with only D3 files/hunks explicitly staged in a temporary index and then unstaged. Disk checks ran before builds; space briefly fell below 10 GiB during tests/concurrent work, then recovered to 24 GiB after cleanup restricted to recorded D3 artifacts. Measurements are in `game/games/beacons/target/d3-size/`, logs in `/tmp/d3-*.log`, paired runs in `/tmp/d3-pairs.jsonl`. No commit, clone, stash, sub-agent or shared-index edit.

**S3b-b — 2026-09-18, reviewed fixes, uncommitted.** All controllers share saved `Playback::root_motion()`/`crossed()`, with an explicitly named motion root, loop-aware weighted/faded displacement and translation removed from the pose; the fixture authors +Z Walk/Run root tracks on the originally in-place CC0 Fox, applies motion through Transform, and logs its own footsteps. Named blend parameters follow `Animator::set`; named state/blend access removes positional state edits. States support once/speed/pause, one-shots finish before transitioning, fades finish before another edge, and incoming markers require fade weight >0.5; exact blend knots silence inactive clips. Removed/unresolved sockets invalidate SocketPose, followers restore their captured authored Transform, and errors log once. First successful histories are current/current; failed samples retain pose history and contribute no stale motion/events. Standalone IK runs, pose reads return all unique joints in node order, and the literal CLI pose suffix is forwarded. GPU normals use the blended map's inverse transpose, including nonuniform scale/shear. The cubic sampler already matched Khronos C.5; asymmetric three-key bake/sample and signed quaternion tests preserve the correct neighbour tangents, duration factors and authored signs. Layout/pick is explicitly a static inflated bind AABB; the unsupported same-app skinning size comparison/claim was removed. Failing-before regressions reproduce the reviewed defects, the CLI omission, scaled normal error, reversed one-shot start and stale error output. Injected bind history changes 11,205 pixels in the Fox's 23,842-pixel rectangle; the fixed first frame changes zero against the current/current oracle (0.1% tolerance). Workspace tests: 449 passed, 10 ignored; all-target clippy and fmt pass. Final proofs: skinned web/Linux 22.276/33.022 s, Beacons/Greybox-with-audio/asset-fixture web 15.953/19.991/11.621 s, all pass and all recorded children exit; the latter three pins are unchanged. New tick-60/tick-120 pins are `0xb863e854ca85b74e`/`0x409341e24939d7c2`, reflecting authored locomotion, playback/socket data and first-history semantics. Every-tick paranoid restores compare bytes as well as hash/pose; web/Linux final saves are byte-identical, 15,084 bytes, SHA-256 `151188009e1141bc52f63a3b913cec4362d788eb5695a80ae3d71ed657f79b3f`. The optional wasm-opt process stalled; only its recorded PID was stopped, cleanup passed, and web proofs used the existing unoptimized-build fallback. Caps uses explicit files and the three-line Socket removal hunk in a temporary index, then unstages; the shared index is untouched. Disk was checked before every build and no existing artifacts were deleted. Logs: `/tmp/s3bb/`; no commit, clone, stash or sub-agent. Linux here is the headless host on this arm64 Mac; no new x86-64 or macOS proof is claimed, and the earlier native white-Fox gap remains outside this slice.


**R2 — 2026-09-18, closing storage/skeleton round, uncommitted.** Erased pages now move and replace typed values through descriptor functions; padded owned values and ZST destructors exercise insert/replace/remove/load (both tests also passed before under normal execution, so no reproduced UB claim; Miri is absent from both installed nightlies). Wrong-kind loads name the registered kind and the required setup declaration. Exact float comparisons cover Display/Debug, boundaries, ties, subnormals, signed zero, NaN/infinities and four-place agent/Contract output: std Display is decimal, Debug alone has the [-4,16) exponent window, so no spelling or pin changed. The isolated typed-storage build is 765,274/325,962 → 766,003/326,394 raw/gzip bytes (+729/+432); the crate table now explicitly labels pre-wasm-opt attribution. Nine new native regressions reproduced animation/diagnostic failures before fixes: reverse standalone one-shots start at the end, reverse markers include exact arrival, zero-length/zero-speed once states finish after an unpaused sample, bad sockets preserve locomotion with independent once-only errors, IK succeeds before Animator commit, redelivered model allocations rebuild rig caches, duplicate names choose the first parent-first match, and pose-length mismatches refuse by model name. Presentation primes current/current on birth/restore/carry/teleport/model or batch arrival; the Fox regression failed restore by 11,200/23,842 pixels before the fix and passes all four pixel scenarios with zero changes after it. Both tick hashes are asserted and unchanged. R2 clippy passed and the skinned web proof passed in 36.219 s with byte-identical continuation. The first full workspace run passed 439 tests (10 ignored) before a concurrent writer introduced an unresolved Paranoid import during doctests; subsequent shared-tree builds met that writer's manifest/lockfile and AssetMap::clear transitions. HEAD also advanced externally from 284ea691 to 6d674d28. The third full workspace attempt passed 464 tests, 10 ignored, including doctests; scoped rustfmt and explicitly staged/unstaged caps pass. Final all-target clippy is blocked only by the concurrent Greybox paranoid test's redundant closure, after the earlier R2 clippy pass. Workspace-wide fmt is blocked by that writer's vendored Rapier workspace setup. Skinned Linux and Beacons/Greybox web proofs could not finish: process inventory (`ps -axo`) hung, and the web optimizer children stalled; only recorded R2 process trees received termination signals, under the three-round limit. The proof/build/host processes exited, but eight blocked ps children remain visible even after SIGKILL (PIDs in the cleanup-result JSON). Asset-fixture web was not launched into the same blockage and remains owed. Logs are `/tmp/exact-r2-*.log`; `/tmp/exact-r2-stopped-processes.json` records the owned cleanup. No commits, clones, stashes or sub-agents; unrelated edits were retained. Further work stays owed: Miri storage validation, durable optimizer attestation, and the previously recorded native white-Fox delivery gap; no new skeleton review round.

PX1: a sibling game's every-tick reconstruction exposed Rapier's omitted deferred
BVH optimization bit: a save could decode successfully yet continue differently.
Lane/game now vendors that fix, names the incompatible bytes EXPHYS v2, refuses
v1 atomically, and shares Off/Save/FreshGame modes between physics, consumer tests
and web/Linux proofs. The reconstruction keeps this lane's exact clock validation
and host scheduler intact. Restore equivalence takes precedence over keeping old
hash pins; a green round-trip hash alone does not prove the next tick agrees.

A3 (2026-09-18): renderer residency now compares each model/texture name with the engine's content digest, preserves prepared pipelines and palette capacity across world replacement, and no longer destroys the whole renderer when a host asset reference retires. The native Fox test proves zero restore/paranoid GPU work, exactly one changed-texture upload through Carry, and zero work on identical redelivery; model replacement and texture sampler/color-space changes have a separate device test. Web Fox tick-45 restore through first draw measured p50 0.60 → 0.50 ms and p95 1.00 → 0.90 ms (20 samples, one warmup, 1280×720, shared Mac; not a speedup claim). GPU work is split before/after first ready, with buffer counts explicitly limited to model/skin buffers. The ordinary and paranoid web restore assertions pass, but the changed-texture browser probe still requests a save while its temporary asset is pending, and the three-round repair limit stops that work pending an override; the following pop-in assertion is therefore unverified. The sibling `next/t6` ref was unavailable, so its counter naming remains unconfirmed. Retired device content now lasts until renderer destruction, an explicit memory-for-reuse trade; no simulation pin was edited.


E5 (2026-09-18, uncommitted): Beacons' tick is 39 → 26 lines and logic/Contract 148+39 → 138+39; `w.character("player").step(wish, jump)`, global `w.position`, and stable `nearest_xz::<Beacon>` remove the query scope and lookup chain. Character's task numbers stay explicit. Restart uses the allowed `#[restart] bool` fallback and the existing setup bind path, with `state.world.restarted`; a canvas word would widen the plan/host transport beyond this slice. Default Cargo/app manifests are ignored bake outputs (existing overrides win); Beacons' launcher is removed, Greybox retains its audio manifest. The template is 120 Hz with grid/background and a centred, explicitly named/autofocused victory control; Beacons and Greybox stay at 60 Hz. Button focus-visible/hover defaults and dynamic autofocus remain QUEUE findings. Standalone Character changes Beacons' tick-907 hash `331c074e0f135059` → `7dde46ef4bc4bdb6` and Greybox setup/forward `7df5e5a89b4d0207`/`0f14b8b231091d12` → `7544ef30a82fdcdc`/`a655423c9a442bce`, without changing the pinned position. Both games pass web/Linux; Linux reads the runner's actual HUD and accessible names. Beacons' warm Linux proof is 0.506 s (0.53 s external wall), Greybox 0.499 s; `prove.mjs beacons --hosts web,linux --repeat 2 --compare-saves` reports web 5.447/5.879 s and Linux 1.389/0.503 s with every final hash and save identical (final save 5,505 bytes, SHA-256 `e388bd748ade00ee7d4a4c50fcaba89024842005d200bef9697a5faae4e75a15`). The Linux host ran on this arm64 Mac; no new x86-64 claim. The generator's Linux-first proof and web-only screenshot pass; workspace tests pass (470, ten ignored), all-target clippy/fmt pass, Bun tests pass (34, one optional skip). Web uses the supported no-wasm-opt fallback; ps stalled, so descendant auditing is explicitly unavailable while owned carrier exits are checked. Mechanical Args literals/pins in existing renderer tests were updated; A3's renderer implementation and fixture proofs were untouched. Caps passes with E5 paths explicitly staged in a temporary index and then unstaged; no commit, clone, stash or sub-agent was used.

R3 (2026-09-18, uncommitted): removed the macOS disk gate; paranoid proofs now finish with Off and an Off web receipt, with three lifecycle regressions (including failed modes). Shared-model redelivery uses upgraded Weak/Arc identity, reaches both entities even in one tick, resets changed topology and primes history; reverse one-shots initialize from their own saved controller clocks, and pose inspection refuses either corrupt history array. All three new animation regressions failed before and pass after. The GPU replacement regression confirms HEAD already replaces geometry/material handles, inverse binds, hierarchy and both instances' batches by content digest. Storage now checks constructor IDs in a non-ZST ownership companion; a true ZST cannot carry an ID, and aggregate ZST/padded tests do not lock typed moves without Miri, which is not installed. Fox birth/restore/carry/model-arrival each assert and measure 0/23,842 changed pixels (channel tolerance 2; bind-history negative control 11,205). Beacons asserts HEAD's existing tick-907 pin 7dde46ef4bc4bdb6 before reset; all HEAD pins remain unchanged. Costs are corrected to headless Sim totals, approximately 0.010/0.34/0.37 ms per physics tick. Workspace tests pass (475 passed, 11 ignored), all-target clippy/fmt and 21 Bun tests pass. All four games pass ordinary/paranoid Linux proofs on this arm64 Mac. Beacons passes paranoid web and a following cached Off ordinary proof; one ordinary run reported a remaining descendant, the retry passed with ps stalled and descendant auditing explicitly unavailable, while carrier exits were awaited. Skinned web retains both 60/120 pins but its residency probe fails on an undefined value's length during KeyC; Save also reports a residency-counter mismatch. Those out-of-scope findings are in QUEUE. The required kinematic pending-flag pin is NOT closed: three fixture attempts did not reach a true pending flag at a non-initial boundary, so the attempted diagnostic is ignored and recorded as owed; no new kinematic pin is claimed. No commit, clone, stash, git add -A or sub-agent was used.


**F3 — 2026-09-18, saved-trace re-score; uncommitted.** The feel analyzer separates raw callback intervals from drawn-clock intervals, adds physical-pixel displacement CV, exact repeats and a strictly greater than half-pixel change fraction, and puts event-delivery → first drawn-pose endpoints plus input phase beside latency. World-space metrics remain for continuity. The README pastes `reanalyze` output for the first full sitting, links every input trace, retains the old table, and preserves load/focus/20-trial/50-edge rules; reanalysis now excludes the unfocused attempt. The archive lacks camera projection for every engine and raw rAF arguments for Exact (its other timestamp is draw-boundary wall time), so those cells and Exact's refresh-normalized latency are `—`. World-space and millisecond-latency ordering survives; raw pacing and perceptual ranking remain unproven. The browser probes now capture explicit clocks, three.js and Godot retain drawn camera matrices and physical canvas sizes, and Exact's adapter can read an extended ring. The ring implementation moved to `render/src/trace.rs`, outside this brief's allowed files: its small row-extension patch is prepared at `/tmp/f3-trace-row.patch`, pending the requested scope approval, so Exact camera capture is not yet complete. All 21 feel tests pass (including clocks, projection, threshold boundaries, missing evidence and reanalysis validity); Godot's headless parser check passes, every saved September 18 trace was reanalyzed, and caps passes with explicit F3 paths staged and unstaged in a temporary index (the shared index untouched). No live sitting, game-rule change, clone, stash, commit or sub-agent was used.


**R4 — 2026-09-18, final E5/A3 review fixes; uncommitted.** Generated manifests are tracked with removable headers, refresh directory/ID/type changes, preserve authored overrides, and materialize before Cargo discovery; the generator test no longer runs a disk probe. Character handles borrow the world mutably (held leases fail to compile); the predicate nearest query searches unlit candidates before comparing distance and breaks ties by entity index. The Rust world helper is explicitly `global_position`, while the agent's `position` remains local; only `#[restart]` edges increment `restarted`. Web/Linux victory autofocus is exercised through the actual focused flag and logical focus, including the web raw-input canvas yielding to its newly mounted child control. Documentation labels pre-E5 pins, records the 0.47–0.53 s external Linux range and the 120 Hz/25 ms integer-seek lattice. Retired models remain inactive through complete dependency delivery/preparation; a 64 MiB conservative retired-byte budget and same-name replacement compact geometry/material/skin arenas to the live set, retaining live texture views. Cosmetic residency reads stay on Sim, with a compile-fail test preventing observation through game World; delivery computes model digests once; named failure reasons determine readiness. The 20-model budget, changed-byte first-frame, repeated Fox replacement and delivery-hash-count regressions pass. Both browser fixture probes now preserve and restore the original save and exercise changed-texture, identical-redelivery and pop-in paths; scoped counters are named `modelSkinBufferReallocations`. Existing simulation pins remain unchanged. Workspace tests, final residency/doctest regressions, clippy, fmt and caps pass; the full Bun command passes 37 tests with one optional skip. All six games pass ordinary/paranoid Linux proofs; Beacons and both asset fixtures pass web, and Fox also passes all web paranoid modes. Web descendant auditing reports ps unavailable while awaiting every recorded carrier exit.

**P1 — 2026-09-18, uncommitted.** `Emitter::sparks().rate(200.).lifetime(5.).seed(7).burst(50)` and one explicit `emitter::step(w)` keep particles outside entity storage: saved admission batches retain deterministic RNG/budget history, while retained render scratch derives motion. Ambient is the author's explicit choice to let decorative motion settle. Sprite, saved atlas animation and one Camera projection field share the same clock, assets and spatial reads; nearest sampling plus integer ortho scale reveals extra world area at noninteger ratios, with CSS viewport rounding shared by render/layout/pick and a 2× display-scale pixel regression. The texture-free path stays primitive; sprites require the asset module and refuse by name otherwise. Displayed pose interpolation now agrees between camera, shader and transparent sort centers; particle/sprite/model translucency uses depth, layer and slot order. The engine emitter is 344 lines and the shared quad path 500. Generated fixtures pass web/Linux at tick 300 (`7b8d9188b32e7d5c`, 20,000 sparks; `f598d0032d70cce5`, 200 leaves), with byte-identical saved continuation, seven web overlap pixel checks, and zero native pixel changes after Open/Carry. Budget admission accepts 65,536 of 80,000 and reports 14,464 refusals. Live web 1280×720 measurements are CPU encode p50/p95 3.600/4.200 ms and GPU 0.282579/0.459493 ms for 20×1,000 particles; walking 2D is 0.200/0.300 ms CPU and 0.304995/0.309746 ms GPU. The final shared-tree Beacons size is 858,785 raw/358,320 gzip bytes, +57,576/+20,496 from the before snapshot; concurrent edits prevent isolated P1 attribution. Sprite retirement/redelivery and both README examples pass their tests; the exact Bun generator/proof files pass 24 tests. Final all-target clippy/fmt and caps pass; explicit P1 paths were staged/unstaged in a temporary index without changing the shared index. Full-suite completion still requires updating the out-of-scope old assertion that all declared `.tex` files fail. Existing regression pins remain unchanged, but the asset web proof's final recheck flakes on readiness despite zero after-ready residency work. macOS stopped at the three-attempt limit: SDK linking then SwiftPM's missing BuildServerProtocol symbol prevented its screenshot. No scheduler/storage/data edits, disk probes, commits, clones, stashes or sub-agents were introduced; only asset-discovery/save-readiness seams were added in sim.rs. Measurements, author code verbatim, pins and reproduction commands are in the two READMEs.


**R5 — 2026-09-18, closing review fixes; uncommitted.** Same-tick model redelivery rebuilds pose histories at the existing controller clock, preserving this tick's markers/root motion without another transition or journal entry. Two regressions cover Animation, Blend and Animator, shared-model entities, repeated redelivery and an active fade; both fail against the pre-R5 implementation and pass with the fix. The kinematic diagnostic was deleted after two attempts: 256 static/32 kinematic colliders moving vertically and 512 static/64 kinematic colliders moving horizontally after tick one each matched Off/Save/FreshGame saves through 32 ticks, but neither reached a non-initial pending optimization boundary; the pin remains owed in QUEUE. Paranoid lifecycle tests now use actual dist files and receipts, reject paranoid artifacts on ordinary runs, reuse successful trailing-Off output, and rebuild after a trailing Off dies before its receipt; removing mode hashing or accepting the failed build makes these tests fail. Headless steady residency reports SKIP, device runs assert readiness and zero work, and the asset proof establishes its first draw before steady ticks. Both fixtures pass Linux/web with unchanged pins, and KeyR/KeyC/KeyP all run; R4 made `delta` used, so it stays. Beacons/greybox pass all Linux paranoid modes and the trailing Off run with unchanged pins. README pins, cache behavior, current probes and the non-ZST companion/Miri limitation now agree. The size helper's disk gate is deleted (stubbed-command regression fails before, passes after); the skinning test skips an unavailable adapter (injected no-adapter path panics before and exits cleanly after; the real GPU test passes). Clippy, fmt, explicit-file caps and all 23 proof tests pass. P1 corrected the old texture-declaration refusal test during validation; the final workspace rerun passes 497 tests with ten ignored. The combined Bun suite's generated game passes gameplay/Linux and its web screenshot but fails the existing descendant audit, after three attempts including an initial empty discovery result. The unqualified Bun command also selected audio-proof tests whose digest-source regex and two native notification fixtures failed; the explicit two-file run finished 25 passed/one failed. The Bun gate failures are recorded, not green. No commit, clone, stash or sub-agent was used; caps staging used a temporary index and was undone.


**U1 — placed Contract children, 2026-09-18.** `Placed::child(1).width(1.2)`
puts an existing Contract child on an entity plane; the engine saves its intent,
while the surface derives the displayed homography, larger-nearer depth and an
explicit hidden outcome. The browser uses CSS and native captures use the ordered
premultiplied quad pass, including the primitive module. The fixture's real Pull
button drives a lamp and HUD, survives save/restore, and hides/refuses the fixed
sign from behind on web. Its tick-330 web/Linux hash is `0x61007363bd681d3c`.
Linux still has no placement consumer; its painter/surface extension awaits scope
clarification. The macOS package loader failed before Swift compilation with a
missing BuildServerProtocol symbol, so native tap/accessibility and forty-child
capture measurements remain owed; the temporary wrappers were removed. Release
projection measured 40.594 ns/child/frame; browser encode p50/p95 was 0.100/0.200 ms
and GPU p50/p95 0.508701/0.784862 ms at 1280×720. Shared-tree wasm grew 29,880 raw
and 12,396 gzip bytes, including concurrent R6 work. Native GPU pixels prove
premultiplied overlap, equal-depth Contract order and wall occlusion. Browser tests
also caught and fixed unchanged frame delivery during module replacement and
preserved placement on a failed swap. Beacons remains untouched by this slice:
its conditional nearest prompt needs stable child identity and ownership transfer,
more than one Contract line plus one component. Displayed sockets still use the
existing tick-resolved follower interpolation. Final game tests: 516 passed,
11 ignored; all-target clippy, both fmt checks, root build, 51 browser tests and
caps pass. All six prior fixtures pass web/Linux with their working-tree pins;
U1 did not edit those pins. No commit or sub-agent.


**R6 — 2026-09-18, uncommitted.** Camera now derives and always saves its projection, moving the enumerated pre-1.0 pins once; aspect-only feeds retain authored orthographic height. Quad pipelines/capacity prepare before interaction, compatible sprite runs coalesce (the strip is six draws), mixed-kind ties are total, and invalid components journal without blacking out neighbors. Particle invariant hoisting preserves the old evaluator's float bits and measures 2.80 ms median encode for 20 × 1,000 on this M5 Max; both timing receipts are retained outside build directories. Texture residency is shared, atlas frames do not rediscover dependencies, and retired assets are charged from GPU allocation sizes, including pending same-name replacements; slot reclamation keeps live hero handles/history, with GPU arena packing reserved for the remaining capacity excess. Restart counters survive Carry/paranoid reconstruction, authored Cargo package names survive directory renames, explicit focus precedes autofocus, host replacement clears autofocus history, and topology replacement cannot freeze an Animator fade. Greybox intentionally keeps the nearest-one interaction introduced earlier: one press lights one eligible beacon, a gameplay change documented in its README, not disguised as an equivalent query idiom. Process survivors fail and remain in the cleanup receipt; inventory parsing covers both day widths and zombies. Device skips are Linux-only in residency proofs, KeyP checks textures/pipelines too, and Apple fixtures pin 299/300-frame cooldown plus visibility refresh. Feel records only the host draw callback, refuses overflow, retains every legacy drawn row and projects the first beacon as a fixed landmark; the three cited trace batches and their reanalysis are retained. Sitting #2 remains provisional under shared load, with landmark CV distinct from world CV; no perceptual winner or new sitting is claimed. Final validation and the finding-to-regression map are in the game README.
