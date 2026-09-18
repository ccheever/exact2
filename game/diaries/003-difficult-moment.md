# I3 — hold a difficult moment while editing Lanterns

2026-09-18, Linux x86-64, headless. **Carry preserves state, but does not explain
how an edit interacts with it.** All four restores succeed with the entire EXSIM
save bit-identical. Two authored changes silently disappear; executable changes
apply at the next relevant tick. No engine behavior was fixed.

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
- `difficult-moment.sim`: 81,835-byte EXSIM v5 save at **tick 225 (3.75 s)**.
  Two independent scripted runs produce **`0xec3b43cc9f709c4c`**, and their complete
  save bytes match. The normal test compares against the committed save.
- `difficult-moment.json`: hash pair, launch/support evidence, components,
  timer, held input, spring sample, pending input and journals, including the
  two unmet requirements. No animation weights exist to record.
- `moving-crate.sim`: 80,855-byte supplementary save at tick 133, hash
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
used by `WorldSurface::restore`. Immediately afterward the test compares the
**complete save**, including physics executor bytes, held input and queued future
input, with v1. It then advances exactly 120 ticks. The unedited restore's
trajectory is compared at every tick with uninterrupted v1 execution. The
supplementary moving-crate save takes the same paths and continuation length.

The third edit replaces the running selection `Run` with the Fox asset's
existing `Walk` clip. It tests replacement by a different clip name, not an asset
file replacement retaining the same clip name. A separate CPU test samples
both real clips and confirms different skinned vertices; no GPU is needed.

## Per-edit result

“Preserved” below means bit-for-bit **at restore**, not that a running simulation
should stay frozen. Every row preserves player/crate velocity, animation cursor,
all spring fields, held and queued input, and timer at that boundary. The
supplementary checkpoint verifies the nonzero crate velocities too. No carry
refusal or corruption was observed.

| Second build | Continue / effect | Difficult state after continuing | Understandable to the developer? | What the value is today |
| --- | --- | --- | --- | --- |
| **Ledge +1 m X**: `scene.json` position 10 → 11 | Succeeds, **silently ignores edit**. Fresh build has X=11; restore and all 120 ticks have X=10. Entire trajectory equals v1. Only a fresh restart using the new scene applies it. | All requested existing state preserved; no field reset. Crate is already sleeping at apex, independently of reload. | `state` exposes old Transform and saved construction args (`restoredFrom` at the restore boundary). No journal line or reload report says the new placement was ignored. A developer must compare values. | Authored placement becomes persistent `Transform.position`; the scene has provenance and a digest, but restore retains saved `Options.scene` and saved components. No authored-value merge policy. |
| **Jump 6.4 → 8; gravity 12 → 18**: change the jump literal, character tick acceleration, and setup's `Physics.gravity` | Succeeds, **partly applies**. At +1, character velocity Y is −0.2999985 instead of −0.1999985. The next eligible jump (+76) starts at 8 m/s. But carried `Physics.gravity` remains `[0,-12,0]`; fresh build has `[0,-18,0]`. | No velocity reset at restore. Thereafter Character velocity/position change as new code executes. Animation progress, spring anchors, held/queued input and elapsed timer continue. Earlier landing makes the queued +76 jump eligible in v2; v1 is still falling and ignores that same press. | Effects are deterministic but the split between character gravity and rigid-body gravity is surprising. `state` exposes `Character.velocity` and the old `Physics.gravity`; logs contain ordinary `jump` events, with no changed-constant notice. | Jump strength and character gravity are **authored constants in executable tick code**, unknown to the engine as editable properties. `Physics.gravity` is a **persistent resource field that setup wrote once**. Only a restart applies that setup edit. |
| **Running clip Run → Walk** | Succeeds. Saved Run remains at the exact restore boundary; Walk applies on the **next tick**. | `fox.Animation.clip` changes, and **`fox.Animation.seconds` resets** from 0.55000013 to 0.016666668 at +1, rather than advancing to 0.5666668. `speed` and `looped` survive. No blend weights exist. Other difficult state follows v1 exactly. | `state` shows Walk and the reset cursor. No journal or reload message explains lost phase. Reading `Animation::play` makes it predictable, but the reload offers no explanation. | Clip selection is a tick-code **authored constant**; `Animation.clip/seconds/speed/looped` are **persistent component state**, despite driving presentation. CPU skinned vertices are derived presentation, rebuilt by feed. No engine classification links the new clip to the old cursor. |
| **Lantern appearance**: bulb colour `[.18,.16,.14,1]` → `[.1,.35,.8,1]`; intensity gain 5 → 9 | Succeeds, **partly applies**. Fresh colour is blue; restore and 120 ticks retain the old colour. At +1 intensity is 3.0626986 instead of 1.7014992 (gain 9/5). Colour needs a fresh restart; gain takes the next tick. | Spring configuration, anchor tick, target, start value and start velocity are preserved. Animation, velocities, inputs and timer follow v1; only tick-written intensity differs. No spring restart. | `state` exposes the mixed result; no journal or reload report explains why one half applied. No “next spawn” mechanism is exercised—this game spawns these lanterns only during setup. | Colour is **authored scene data saved as `Material.color`**. Gain is an **authored tick-code constant**. `PointLight.intensity` is conceptually **derived presentation**, but is still saved/hashed component state overwritten each tick; the engine has no separate lifetime policy for it. |

The saved construction argument wins even though `restore_bound` retains current
**live** bindings. There is no “applied / ignored / deferred” per-field report.
Immediate restore success alone therefore does not establish the claim that
every edit has an understandable effect.

| Build | Hash at restored tick 225 | Hash at +1 | Hash at +120 (tick 345) |
| --- | --- | --- | --- |
| v1 | `ec3b43cc9f709c4c` | `bf22b9b91da4f03c` | `14e9fd0a88907090` |
| placement | `ec3b43cc9f709c4c` | `bf22b9b91da4f03c` | `14e9fd0a88907090` |
| physics | `ec3b43cc9f709c4c` | `2573b41091925c32` | `60d02285cb7a8705` |
| clip | `ec3b43cc9f709c4c` | `f25fc07ab444b712` | `607a57ff1393d62f` |
| appearance | `ec3b43cc9f709c4c` | `a154ab3313643b77` | `e4d43103bb91b117` |

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

One isolated final run, median of the 120 per-tick samples:

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
never rewrites pins. The original game tests/proofs and all existing hashes are
unchanged. No root workspace membership or public API was added.


## Verification

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
