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
distinct sphere radii plus 4,100 animated revisions retain one mesh and one draw.
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
