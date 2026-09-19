# Landing next/trunk on the engine line — 2026-09-19

Engine parent: `8db5e42e8f05122e7f4ca265ba0bd480c6ab7621`.
Incoming parent: `360921d386f212965cbc28229f21c93d280a753a`.
Merge base: `6a03eba833170f1854cc2f13cec43303f99e2d84`.

Charlie's execution instruction overrides the brief's preparation-first order:
commit the actual merge once the game workspace compiles and engine tests pass,
then adapt the remaining capabilities in separate validated increments. The
pristine merge reproduced all 51 conflicts. No remote commands or agents used.

The first merge retains the engine's APIs, renderer, games, workspace ownership,
proof recorder and host lifetimes. It activates incoming name lookup and bounded
placement; the remaining coupled implementation paths are deferred to the ordered
increments below, using the immutable incoming parent. This is not a completion
claim for those capabilities. Incoming historical diaries retain their original
scope; their numbers do not measure the combined runtime.

## Baseline

Disk: 121 GiB free before the cold build. Bun explicitly selected from
`~/lanes/gamenext/tools/bin` (1.4.2); inherited debug/incremental settings retained.
App locks refused the resynced ibex graph. All eight consumer locks and the starter
seed were regenerated through `game/app/shells.mjs --update-lock`, with an online
fetch. The root lock also needed refreshing for the bake's filesystem helper.
These are separate prerequisite commits. No pins changed.

Engine baseline tests, game clippy/fmt, caps and boot passed. Full game workspace
tests have 12 GPU-dependent failures: no adapter. Consumer workspace tests have
three more: skinned first-presentation pixels, sprite atlas rendering and primitive
particle rendering. All assertions remain intact. Other GPU tests sometimes return
early; their `ok` status is not evidence of GPU execution.

Initial Bun tests: 112 pass, 1 skip, 3 fail (starter seed twice, missing default
Chrome path). After selecting installed Chromium and refreshing the starter seed:
114 pass, 1 skip, 1 fail (root lock refused in filesystem bake helper). Root lock
refresh follows that evidence. The Chromium reuse test passed. Browser WebGPU proof
has not yet been certified.

## Ordered capability status

1. Name index and placement: carried/adapted in merge. Lookup is logarithmic,
   with lowest-slot duplicate resolution. No public rename method exists at the
   engine tip; decode/replacement rebuilds names. Scatter returns explicit errors
   instead of partial vectors, and uses engine asset bounds for placement.
2. Incremental observation: carried with engine storage and hierarchy semantics.
3. Typed kinds and hot joins: carried with engine Target and removal semantics.
4. Static physics split and fixes: adapted to CapsuleHandle and decoded snapshot clones.
5. Ownership, restore clock anchor, EXCAP v2: adapted and headless-verified.
6. Three-way authored merge and EXSIM v7: pending. Full typed scratch decode must
   precede setup/authored merge; restore tick/time agreement must be exact.
7. Geometric layout and movement diagnostics: pending.
8. Typed scenes and adapted Lanterns: pending.
9. DX host transactions and handoff: pending.
10. I3 fixed evidence and opt-in trials: pending.
11. Named real-GPU Fox acceptance, final documentation/matrix: pending.

The T6 production backend, embedded GLB renderer, old Beacons conversion,
duplicate EXPHYS implementation and Lanterns-only repin command stay behind.
Incoming retained tests will be adapted in their capability increments; immutable
incoming history holds their original versions and bulk receipts.

Merge gate: game workspace build and clippy passed; engine suite passed 331 tests (3 ignored diagnostics). Includes 200,000-name interleaved churn, index exclusion from saves/hash/observation, scatter impossible packing, admission, arithmetic overflow and shared budget refusals.

After documenting the two public Linux Activation fields (the preexisting host
build failed deny(missing_docs)), all seven games pass normal Linux proofs and
all three paranoid modes: 28 executions total; all world and save pins unchanged.
All eight app workspaces pass clippy/fmt. Game Bun tests now pass 115, with one
existing skip. The workspace/consumer GPU failures remain adapter refusals.

Chromium 1234 was tried with SwiftShader flags: production Beacons wasm built and
launched, but WebGPU found no adapter; browser proof failed on host exceptions.
No further software-WebGPU tuning is attempted.

The brief incorrectly treats engine-tip cubes/proof.mjs as a Linux proof: it is
an unconditional Chrome benchmark sanity run, ignores the host argument, and has
no pins.json. prove refuses the missing file; direct invocation fails without
WebGPU. Its temporary BENCH_N app.contract rewrite was reverted after the run.
Cubes logic tests/clippy/fmt pass; benchmark/browser proof remains unverified.

Root build/test/clippy were attempted: TS app bakes refuse the missing lean
Hermes executor on this producer. Root fmt, caps and boot pass. Web fixture sweep:
93 pass, 5 fail (four require xcrun/Apple SDK; Caltrain needs its web dist baked).
These platform/tooling failures do not authorize weakening any assertion.

Observation gate: 344 engine tests pass (4 ignored diagnostics), workspace and all
eight consumer clippy/fmt pass, 28 Linux normal/paranoid executions preserve every
pin, Bun 115 pass/1 skip, caps and boot pass. Dirty slot hashing and page hierarchy
stamps retain the slow oracle; tests include 200,000 interleaved slots, churn and
production Body edits. The physics Body declaration was extracted early to let
that oracle use the real component schema; bytes and stepping are unchanged.
The existing ergonomic seek test now demands one serialization on a warm seek
and two after mutation, preserving its live-path zero-serialization assertion.

Selected root crates: 517 tests pass, three exact-gpu native placement tests fail
without an adapter, one diagnostic ignored; selected core clippy passes. Caltrain
web dist was baked successfully, but its placement fixture then timed out after
60 seconds. Apple fixture failures still require xcrun.

Kind integration preserves Target::label and invalidates cached membership on the
internal remove_component path used by animation Pose cleanup. Its added negative
control proves refusal occurs before Transform takes a write lease. Incoming
200k sparse/interleaved/churn and allocation tests remain; saved child bindings
and compile diagnostic tests remain. The implementation's actual nesting bound is
32, rather than the brief's proposed 64; no reason was found to expand it.

Kind gate: 371 engine tests pass (6 ignored diagnostics), game and eight app clippy/fmt pass, 28 Linux normal/paranoid executions preserve pins, Bun 115 pass/1 skip, caps and boot pass. Full workspace and app test reds remain the recorded adapter-dependent rendering tests.

Physics integration retains the engine's capsule target validation, displacement
result, dual-controller refusal and snapshot-through-decode clone. The incoming
implementation separates static/body dirtiness but retains one query collider set
and canonical binned traversal certificate; it is not literally two independent
query scenes. New admission counts the union once and runs before solver, query
or controller geometry preparation. Boundary and over-limit tests include a
controller that creates its own body/collider. Full rebuilds and total geometry
memory (including terrain samples) remain the unfavorable bound.

Physics repair rounds: incoming query tests still used the removed Character /
move_character names; adapted them to CapsuleController and capsule.step. The
full-scan snapshot oracle then refused an unregistered Parent on decode; added
explicit registration in that test's destination, retaining every equality check.
Physics tests now pass 48, with six opt-in timing diagnostics pending below.

Physics release timing diagnostics: all six pass on this x86-64 box. Median
microseconds with 20k statics: 100 bodies step 88.502, 1k checked rays 594.351,
1k overlaps 695.793; single-static teleport with checked ray/overlap 3807.746
(1000 edits total 3817.426 ms). One controller batch 1.171, eight 372.388;
interleaved body/static pages with one controller: batch 2.619, solver 735.903,
checked ray/overlap 744.486. Character-only batches: one median 18.828/p95 47.632,
eight median 391.792/p95 31846.755. Changing scene extrema by teleport:
median 4617.675/p95 4933.463 versus ordinary outside-scenery movement
17.757/17.877. Retained 1024² terrain: first query 8191.275, first controller
7923.414; RSS world/query/controller 12516/14988/20216 KiB. These are measurements
of the merged code, not speedup claims; the canonical-fallback tail remains.

Physics full gate: game and eight app clippy/fmt pass; 28 Linux normal/paranoid executions retain all world and save pins; Bun 115 pass/1 skip, caps and boot pass. Workspace/consumer test failures remain only the recorded GPU adapter refusals.

Capture integration decisions: retain EXSIM5 in this increment. Queued delivery
metadata lives in EXCAP's scheduler record, while EXSIM5 restoration preserves
its existing delivered-event interpretation. input() keeps G's live pacing;
new scheduled_input() spells future scheduling explicitly. Incoming scheduling
tests use that spelling and retain all assertions. New named-control capture
coverage includes a held checkpoint, release, invalid-name refusal before
recording, and handoff cancellation. A rejected-input journal diagnostic is
outside the accepted-input suffix; the byte-equality checkpoint starts after it.

G's default-state equality contract excludes host scheduling. clockState:true
requests host/world microseconds explicitly; exactTicks uses that request.
Remembered layout dimensions live in an inspection-only field, preserving G's
subsequent layout behavior without changing saved input or capture records.
Capture replay can reuse baked assets with replay_capture_using; no embedded GLB
API is introduced. Restore now constructs one candidate, keeps current bound
arguments, checks exact tick/time agreement and retains queued textures. Shared
Data decode budgets and strict-field hooks landed as decoder prerequisites.

The driver's capture artifact inventory and eight-operation helpers retain G's
carrier reuse, PID callbacks, complete pin inventory and paranoid journal
comparison. Incoming duplicate repin and T6 upload exemptions are not carried.
prove now forwards --paranoid to the existing three-mode proof entrypoint.
Native acknowledged detach and host reload transactions remain the DX increment.

Capture gate: 404 engine tests pass (six ignored diagnostics); complete game
workspace 641 pass / 14 fail / 22 ignored, with all 14 failures refusing the absent
GPU adapter. App workspaces: 29 pass / three no-adapter failures / one ignored.
Game and all eight app clippy/fmt pass; all 28 Linux normal/paranoid proofs pass,
Bun 117 pass / one skip, caps and boot pass. Every pin is unchanged.

The first complete gate exposed Beacons losing its first 750 ms after a fresh
restore: Linux had relied on a state read to set its epoch. Explicit clock samples
now occur after headless restore/delivery and at the headless placement frame.
Repair round one fixed placement but missed non-Each surfaces; round two added the
post-restore sample and fixed Beacons and the generated-game proof. Greybox's exact
state snapshot adds only ownership/capture fields, retaining the pin placeholder.
The exactTicks mock now requires the explicit clockState request. All prior
assertions remain; no extra seek or tick was added to the proofs.
