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
2. Incremental observation: pending.
3. Typed kinds and hot joins: pending.
4. Static physics split and fixes: pending.
5. Ownership, restore clock anchor, EXCAP v2: pending.
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
