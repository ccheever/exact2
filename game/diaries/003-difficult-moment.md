# I3 — hold a difficult moment while editing Lanterns

2026-09-18, Linux x86-64, headless. **T2: carry now explains and applies authored
edits through a three-way merge.** The old build's tick-zero Data, new build's
tick-zero Data, and carried values distinguish unchanged authored fields from
simulation state. The unedited continuation retains its original hash pins and
complete-save equality. The original I3 findings (silent placement/colour loss and
split gravity) motivated this change; the table below records the new behavior.

The exact requested combination was **not achieved**. Lanterns has no animation
blend or jump clip: airborne selects `Run`; `Animation` stores only `clip`,
`seconds`, `speed`, and `looped`. Three route exploration rounds reached the
crate-to-ledge apex but found the crate asleep there. Following the three-round
rule, this instrument records that gap, keeps the genuine apex, and separately
probes a genuine moving-crate checkpoint. It does not inject velocity, teleport
the player, change friction, or add blend state. This is evidence about the
current implementation, not a successful demonstration of the full proposed moment.

## Reproduction and boundaries

The fixtures are in [`../games/lanterns/fixtures/`](../games/lanterns/fixtures/):

- `difficult-moment.script.json`: executable sequence from tick zero, seed
  `1041003`, started and sound enabled. `key_down`/`key_up` dispatch the existing
  Sim helpers; `clock` dispatches the existing agent operation. `after_ticks`
  schedules the same `InputEvent::Key` at an explicit host timestamp.
  `checkpoint` is a recorder marker and executes no simulation mutation.
- `difficult-moment.sim`: 131,793-byte EXSIM v6 save at **tick 225 (3.75 s)**.
  Two independent scripted runs produce **`0xec3b43cc9f709c4c`**, and their complete
  save bytes match. The normal test compares against the committed save.
- `difficult-moment.json`: hash pair, launch/support evidence, components,
  timer, held input, spring sample, pending input and journals, including the
  two unmet requirements. No animation weights exist to record.
- `moving-crate.sim`: 130,813-byte supplementary save at tick 133, hash
  `0x676338add4cd9154`. Both velocities are nonzero and `Body.asleep == false`.
- `difficult-moment.continuations.jsonl`: restored boundary plus **every one of
  120 continued ticks for each of five builds** (605 records). This records
  hashes, positions, velocities, animation, spring anchors, appearance, held
  input and elapsed timer. Full fresh/restored/continued observations and
  journals, including all supplementary runs, can also be emitted with
  `EXACT_I3_OUT=<directory>`.

At tick 192 the player's `Character.support` names entity 3 (the crate),
`grounded` is true, and position is `[6.624192, 1.8599124, 8.408652]`.
At tick 225 the player is airborne at `[9.099186, 3.619911, 8.408652]`, moving
`[4.5, 0.0000015199184, 0]` m/s: the discrete jump apex. Fox `Animation` is
`Run`, seconds `0.55000013`, speed 1, looped. The ledge lantern's spring has
start tick 219, target 1, value **0.34029984**, velocity **5.33507204/s**;
its stored light intensity is 1.2683034. The game writes presentation during the
tick, so the stored intensity reflects the preceding spring sample; the report
keeps that distinction. `Space` and `KeyD` are held. `Session.elapsed == 225`,
phase 1. Pending Space edges are release +10 ticks, press +75, release +76;
timestamp rounding consumes these at continued ticks 11, 76 and 77.

At this apex `crate.Body.velocity` and `.spin` are both zero and `.asleep` is
true. At the supplementary pushed-crate save they are respectively
`[2.086549, -0.0005824671, -0.14935672]` m/s and
`[0.00051561696, 0.000000028017523, 0.0013452333]` rad/s. Those actual values,
including the small angular component, are tested through all four carries.

## How the second builds work

`logic/build.rs` compiles separate copies of the actual Lanterns implementation
into the **test binary only**, with explicit substitution anchors that fail if
the source disappears. There is no post-restore world patch. The copies have
the same Game ID, SAVE_VERSION, named component schemas and action bindings.
The placement and colour edits bake modified copies of the actual `scene.json`
through `exact_game_scene::bake::compile`; temporary sources are removed.
The original scene and shipped game logic remain unchanged.

Each destination first starts fresh with its edited construction, establishes
agent ownership at host time 9000 ms, and calls
`Sim::restore_bound(&mut self, &[u8]) -> Result<(), DataError>`, the same path
used by `WorldSurface::restore`. For the unedited row, the test compares the **complete save**, including physics
executor bytes, held input and queued future input, with v1. Edited rows compare
unaffected fields at restore and throughout continuation; authored changes and
reload journal lines intentionally change their complete saves. It then advances exactly 120 ticks. The unedited restore's
trajectory is compared at every tick with uninterrupted v1 execution. The
supplementary moving-crate save takes the same paths and continuation length.

The third edit replaces the running selection `Run` with the Fox asset's
existing `Walk` clip, and now changes the setup initializer `Survey` to `Walk`
to exercise the `kept` report. It tests replacement by a different clip name, not an asset
file replacement retaining the same clip name. A separate CPU test samples
both real clips and confirms different skinned vertices; no GPU is needed.

## Per-edit result (T2)

Every row preserves player/crate velocities, animation cursor, spring state,
held/queued input, and timer at restore. The moving-crate fixture proves the
nonzero velocities too. During continuation, inputs and timer remain identical
in every row; crate state remains identical except when gravity intentionally
changes integration. Clip and appearance preserve character and spring trajectories.

| Second build | Applied / kept | Continued result and evidence |
| --- | --- | --- |
| **Ledge +1 m X** | `applied ledge.Transform.position [10,1.2,8] → [11,1.2,8]`; scene digest also applies. | X=11 at restore and all 120 ticks. Rays at X=12.5 hit the new edge; X=8.5 no longer hits the ledge. Fox lands at tick 242 at `[10.374183,3.1099114,8.408652]` and remains supported past the old edge (tick 271 is grounded only in the edited row). The renderer's real CPU feed uploads X=11 on the next tick. |
| **Jump 6.4 → 8; gravity 12 → 18** | `applied resource.Physics.gravity [0,-12,0] → [0,-18,0]`. | Rigid bodies and tick-code character acceleration now agree. The queued +76 jump still starts at 8 m/s. No velocity reset at restore. |
| **Running clip Run → Walk; initializer Survey → Walk** | `kept fox.Animation.clip Survey → Walk: initializer changed, applies on restart`. | Saved Run and its cursor survive restore. Tick code selects Walk at +1 and `Animation::play` resets seconds to 0.016666668, as before. Real Fox Run/Walk CPU skinning remains different. |
| **Lantern appearance**: bulb colour changes, intensity initializer 0 → 2, tick gain 5 → 9 | `applied lantern-12/bulb.Material.color`; `kept lantern-12/bulb.PointLight.intensity 0 → 2`; scene digest applies. | Blue material persists and reaches the CPU renderer feed; the simulation's current intensity survives restore and the new gain applies on the next tick. Spring state is unchanged. Unedited light initializers produce no kept noise in other rows. |

Reports are asserted in both `state.world.reload` and the canvas journal, and
remain unchanged through tick +120. They retain at most 64 items plus an omitted
count. Existing host-level Continue/Restart outcome messages remain intact.
The changed intensity and clip initializers extend the original four edits so
simulation-owned fields exercise `kept`; the shipped game/scene is unchanged.

| Build | Hash at restored tick 225 | Hash at +1 | Hash at +120 (tick 345) |
| --- | --- | --- | --- |
| v1 | `ec3b43cc9f709c4c` | `bf22b9b91da4f03c` | `14e9fd0a88907090` |
| placement | `5676ee63de32a8ed` | `839f88fa13f1e0cd` | `d09155ed691c6021` |
| physics | `626a26177e322ed0` | `20b8094d1b63af2b` | `1a8f375f1eef7193` |
| clip | `ec3b43cc9f709c4c` | `f25fc07ab444b712` | `607a57ff1393d62f` |
| appearance | `95b4b91d6f16f034` | `5f70c2446656ff54` | `8808ff8b40c06cf4` |

## Merge bounds, choices, and unfavourable cases

EXSIM v6 stores a compact bulk tick-zero projection: the apex save grows by
49,958 bytes (81,835 → 131,793); the moving save grows by the same amount.
EXSIM v5 is explicitly refused because it lacks the old authored base. This
follows the repository's pre-1.0 no-migration rule. The base advances to the new
initializer after a successful restore. Save/hash representation of `Entity`
and `Id<K>` is unchanged; merge comparisons resolve unique names across reordered
spawns. Unnamed rows require agreeing indices/component sets and a surviving
carried incarnation. Added/removed entities/components are deferred to restart.

The randomized test checks 128 worlds with random initializer edits and carried
values: identity, every applied/kept value, skipped-field exclusion, identical
reports, saves and hashes on two runs. Separate tests cover nested skipped fields,
entity/typed handles under interleaved spawn order, structural changes, churn,
report overflow, and explicit budget refusal. These assert positive report entries
and changed values; returning an empty report or removing the merge fails them.

An isolated **200,000-entity** run reverses spawn order, interleaves component sets,
recycles every third slot and edits all 400,000 Probe fields. It checks every result,
including after the 64-item report cap: **3.248 s merge**, **1.246 s construction and
projection**, **1,008,688 KiB process peak RSS**, final hash `1897ba3ddadebdf1`.
The base is **11,672,475 bytes**, the edited initializer **23,872,519 bytes**.
These are optimized dev/test CPU observations on this shared machine. The first
run explicitly refused the original 512 MiB decode allowance; the final limit is
1 GiB across both decoded projections (or an importer's tighter shared budget).

Projection limits: 1,000,000 slots, 256 storage types, 16 million visits, depth 64,
512 MiB accounted projection storage and 128 MiB encoded base. Exceeding them
returns a named error; atomic restore leaves the running simulation intact.
Work is O(V log V) in visited data with bounded nesting, runs only at construction
or restore, and is never triggered by a tick. Custom Data implementations remain
trusted code. Static collider translation now uses Rapier's affected contact
islands instead of waking every body; a negative-control test moves a sleeping
body's support and checks that body falls while a remote sleeper stays byte-identical.
I3 includes the real character controller and both asleep/moving crate checkpoints.
Existing deferred-asset surface restore tests cover setup waiting for assets.

## CPU timing and memory

The ignored `exact-game-render` test `difficult_moment_cost` restores v1, primes
its real `Feed` with the real Fox asset, and measures each of the next 120 ticks.
Simulation uses `Sim::advance` with the seekable clock (including physics,
audio state and observation, excluding agent JSON serialization). Animation
sampling is timed inside the actual `Model::sample` call used by feed; it includes
channel evaluation and CPU skinning. The remaining feed sample is its elapsed
time **minus that same tick's animation sample**, not the difference of medians.
The existing recording `Writes` backend performs CPU preparation/copies and
omits GPU uploads. Asset parse/setup and initial feed are outside per-tick timing.
Instrumentation is `#[cfg(test)]` and opt-in; production has no timing changes.

Original I3 baseline run, median of the 120 per-tick samples:

| Measurement | Result |
| --- | ---: |
| Simulation | **372.683 µs/tick** |
| Animation evaluation + CPU skinning | **37.291 µs/tick** |
| Feed, excluding that tick's animation sample | **42.834 µs/tick** |
| Peak process RSS (`VmHWM`) | **17,664 KiB (17.25 MiB)** |
| Final v1 hash | `0x14e9fd0a88907090` |

This is the repository's optimized **test/dev** profile, not a release/GPU
benchmark: Rust 1.97.0 (`2d8144b78`), Bun 1.3.14, x86-64 Linux.
`CARGO_PROFILE_DEV_DEBUG=0`, `CARGO_PROFILE_TEST_DEBUG=0`, and
`CARGO_INCREMENTAL=0` remained set. No concurrent build/proof was running during
this final sample. The machine is shared, so these are observations rather than
latency guarantees.
GPU time is **not measurable on this machine**. CPU preparation does not prove
GPU upload cost, pixels, material appearance, visual transition quality, browser
reload integration, or Apple behavior. Linux `/proc/self/status` `VmHWM` measures
whole-process peak RSS, including setup and the test harness, not allocator-only
live bytes or GPU memory. Timings are diagnostic observations, not test gates.

## Re-run

```sh
export PATH=$HOME/.cargo/bin:$HOME/.local/bin:$PATH EXACT_UPDATE_TRUST=development
cargo test --manifest-path game/Cargo.toml -p lanterns-logic --test difficult_moment
cargo test --manifest-path game/Cargo.toml -p exact-game-render difficult_moment_cost -- --ignored --nocapture
```

Set `EXACT_I3_OUT` to a directory to retain full observations; fixture changes
require the explicit `EXACT_I3_RECORD=1` mode. The ordinary deterministic test
never rewrites pins. The original game proof hashes and unedited continuation pins are unchanged.
Only the explicitly edited I3 rows and versioned save fixtures are regenerated;
the root workspace gains no game dependency.


## Original I3 verification (before T2)

- `cargo test --manifest-path game/Cargo.toml --workspace --no-fail-fast`:
  **408 passed, 0 failed, 12 ignored**. The ignored I3 timing test was run
  separately and passed. The two new ordinary tests are the carry/fixture probe
  and the real Fox Run/Walk CPU-sampling comparison. Some existing GPU tests
  return early without an adapter; their test-runner passes are not pixel proof.
- Game workspace clippy (`--workspace --all-targets -- -D warnings`) and
  `cargo fmt --all -- --check`: **pass**. Clippy initially encountered the absent
  generated Lanterns scene; its normal Linux proof produced that bake. Two
  narrow test-only lint allowances document intentional duplicate module builds
  and the intentionally identical Walk branches after replacing Run.
- `cd game && bun test`: **38 passed, 2 failed**, reproducing the documented
  environment limits. The generated-game web proof fails opening Chrome
  (`output.setEncoding` on null); feel `--no-build` lacks Beacons' 60 Hz
  `dist/index.html` (and this box has no prepared 120 Hz web bake). Neither was
  repaired or hidden by this instrument.
- Every checked-in game's `proof.mjs linux`: **130/130 assertions**:
  asset-fixture 6, Beacons 54, Greybox 62, Lanterns 8. End-to-end durations,
  including builds: 70.951 s, 35.578 s, 20.866 s, 57.664 s respectively.
- Root caps and boot checks: **pass**. No existing deterministic pin changed.
  Disk was checked before the initial cold build: 133 GiB free; final check
  showed 121 GiB. No extra worktree or alternate target directory was created.

Unverified here: actual GPU drawing/timing, browser reload integration, Apple
hosts, and the exact requested simultaneous blend-plus-moving-crate apex.
The first two state requirements are recorded honestly as absent/unreached;
none of the passing checks upgrades them into a demonstrated capability.

## T2 verification before trunk integration

- Game workspace: **453 passed, 0 failed, 14 ignored**; the 200k reload diagnostic
  was also run explicitly and passed. Game clippy with `-D warnings`, formatting,
  caps and boot pass. No unedited hash or position pin changed.
- All Linux proofs: **130/130** (asset-fixture 6, Beacons 54, Greybox 62,
  Lanterns 8), including complete continuation-save equality.
- `cd game && bun test`: **49 passed, 2 environmental failures**, unchanged:
  missing Chrome and prebuilt feel distributions. Root build/test/clippy cannot
  finish app bakes without this producer's lean Hermes executor; root formatting,
  caps and boot pass.
- The existing 500,001-entity renderer test passes unchanged. Initializer encoding
  now streams each component rather than retaining a whole tick-zero value tree.
  The former Lanterns “ignore edited scene” test now asserts applied authored
  placement/digest while retaining all simulation-state and plain-restore checks.
- I3 fixtures were regenerated only through `EXACT_I3_RECORD=1`; ordinary runs
  compare them. The same-build v1 hashes remain `ec3b43cc9f709c4c`,
  `bf22b9b91da4f03c`, `14e9fd0a88907090`.

The renderer evidence is its actual CPU feed/upload seam, not GPU pixels. GPU,
Chrome, Apple SDK/runtime, physical sound, and the original simultaneous
blend-plus-moving-crate apex remain unverified/unavailable here.
