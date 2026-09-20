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
6. Three-way authored merge and EXSIM v7: adapted and headless-verified.
   Typed world decode and exact tick/time validation precede authored merging.
   R14 will move the decode ahead of setup as well.
7. Geometric layout and movement diagnostics: adapted and headless-verified.
8. Typed scenes and adapted Lanterns: adapted; Linux verified, first web baseline pending.
9. DX host transactions and handoff: adapted; local fixtures/headless proofs verified; Apple matrix pending.
10. I3 fixed evidence and opt-in trials: adapted and headless-verified; first shared fixed-consumer baseline awaits web.
11. Named real-GPU Fox acceptance: adapted and unignored; adapter creation fails
    on this box, so the residency criterion remains unverified. Final matrix below.

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
GPU adapter. App workspaces initially: 28 pass / four failures / one ignored. One was the
Greybox Rust layout/pick inspection-viewport regression corrected immediately
after this commit; the other three refuse the absent adapter.
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

Capture followup: public Rust layout/pick now share the inspection viewport.
Greybox's original pick assertion passes; the original tick-60 layout assertion
now follows an explicit clock operation, with new tick-zero and unchanged-save
assertions proving the preceding layout read is inert. Complete repeated gate:
641 workspace passes / 14 adapter failures / 22 ignored; 29 app passes / three
adapter failures / one ignored; game/eight app clippy/fmt, all 28 Linux proofs,
Bun 117 pass / one skip, caps and boot pass. Pins remain unchanged.

Authored restore uses a saved tick-zero projection and its construction arguments.
Bound restore keeps all current arguments and the destination's cached initializer;
unbound restore reconstructs its saved construction arguments once, then reinstates
saved current live bindings. This avoids reauthoring initial values after a live
binding changed. Open restores saved declarations exactly; Carry patches only
unchanged runtime fields. The merge is after typed world load and exact tick/time
validation, with a second hierarchy validation refusing cycles introduced by the
combination. R14's typed-scratch-before-setup work remains its own incoming slice.

The model adapter retains engine-owned controller membership but no longer
reapplies existing clip/speed definitions over the merge's kept values. A new
Carry/Open/repeat test checks runtime clip, speed and playback and untouched authored
changes. G's audio Carry definition overlay is deliberately retained: fresh named
Sounds win, runtime-only names and existing Voices survive. Generic merge reports
are about the Data patch; these documented presentation ownership rules still apply.

Entity Data now delegates to Writer::entity: ordinary codecs retain precisely the
same index/generation encoding, while initializer projection records references by
identity. This fixed the two incoming reference-remapping failures in one repair
round. No assertion was relaxed. Additional tests cover v4/v5/v6 refusal, all
current bound arguments, independent base/current arguments, invalid pending controls,
exact one-tick-ahead refusal and a combined Parent cycle with atomic rollback.

Reload's 200k reverse/interleaved/churn diagnostic passed in release: base
11,672,475 bytes, new initializer 23,872,519 bytes, preparation 992.867085 ms,
merge 2.45556701 s, peak RSS 992,060 kB. All 200k value/identity/skipped-state checks
and the 500,000-item report/omission control passed. This is worst-case work, not
an incremental reload or speedup claim.

EXSIM7 pin refresh used only the engine recorder, each command
`bun game/prove.mjs <game> --repin --hosts linux`. All continuous / Save / FreshGame
values agreed and every world hash stayed unchanged. Web was not exercised;
receipts honestly record Linux only and need the owner's cross-host run. Each
continuation SHA256 below moved because EXSIM7 adds the initializer/base arguments,
serialized queued-delivery flags and a new format header. No value was hand-edited.

| Game | Old continuation SHA256 | New continuation SHA256 |
|---|---|---|
| asset-fixture | `6e49ed23ac776fa6f0f6df7198d8f3a7d310c9f93d00634dbba36f4f8a46c07a` | `a4be74d1e6e64d344abdc21e5359f99505c8ccaaeffc4a223046c6960b5d6b43` |
| beacons | `1012a823b7ec264cfdd64c748a794691225ac186761d7e9d76f390bc626409e9` | `8959bf658432452fac9bc4297f6053c1bcba8f455ee6fe3ce5de33eb11bdc478` |
| greybox | `d3e4364bdf399ae565efbde077bc3cf4bb829b3b855d4a067d06776e8bccae65` | `efd556700b72639a941e4d4f3575bcb9f01bc28fae8ba55e10a19dd14ff61843` |
| particles-fixture | `6f850600b9b7a87f6e12d2c42adc587c8ef5dc9d48ecea1b2af51ecfbd34a7e6` | `0b6a72ae80d2e9bfb745a69d472e097824b41f3eeb541c1a771bd3bbeed14616` |
| placement-fixture | `27f28608444a701e73c10a96d20bcb801c840b2f148bf30a1ba26592c6c51b8f` | `e1be13a0c7b30d37fe6651e3345b43f5766c32164bbfab24e9104bd3c66bea33` |
| skinned-fixture | `0d50ebe6388c4db317aae21fb3a1f30269f9e67300c3a0ec50e82eec0b7e5a79` | `76b33c647ee21818ad4f2b31ac130872594f73485c09f0899a5ee78f1e3971d3` |
| sprites-fixture | `ca774847c5db059a2dd7991927ce9c38578c8d0ed45ef34e788b99bcfe4c5b41` | `bc16e30a2114d3b219aabd6ede719a818876bf26eb6183175c4486f721c9d314` |

Authored gate completed: 654 game-workspace passes / 14 adapter failures / 23 ignored; 29 app-workspace passes / 3 adapter failures / 1 ignored. Full game and all eight app clippy/fmt pass. All 28 Linux normal/paranoid executions pass, Bun 117 pass / 1 skip, caps and boot pass. The opt-in 200k reload timing test passed separately. Report contents survive repeated Save/FreshGame rebuilds as well as ordinary Carry/Open; combined valid authored/runtime parent edges that create a cycle refuse atomically after typed decode.


Geometric integration preserves G's orthographic/integer viewport, lossless world
positions, billboard sprite point-picking, precomputed/Pose model bounds, socket
fallback and transform-less identity screen rectangle. N's test expecting a fresh
unpropagated child to have no pose was adapted to assert G's current parent pose.
The original G precision and typed/wire screen assertions remain unchanged.
The index caches current poses with iterative Parent/SocketFollow dependency
resolution; valid sockets share animation's local joint matrix helper. This adds
an explicit 1M entity/model-node pose-work bound and cycle refusal, followed by
N's 262144-slot/1M shared ray-visit bounds. The entire agent layout and optional
route share one ray budget. Mesh bounds take precedence when an entity also has a
Sprite, as in G layout. Emitters remain cosmetic. Cache keys include Pose,
SocketFollow, Sprite, Camera and the engine model/presentation stamps; mutable
leases are checked on cache hits. In-place World.read discards the cache.

The 200k mesh test now also registers SocketFollow, retaining its 99999-deep
parent-chain success, <=400k-visit assertion, blocker deletion and dense refusal.
Optimized test-profile measurements: cold sight 143.630242ms, warm median
10.390022ms; full layout cold 130.887065ms, warm median 3.835598ms. The standalone
sight measurement also includes G.global's 100k attachment-aware ancestor walk;
the full layout uses the memoized geometry. Dense refusal cold/warm
142.240479/33.695338ms at exactly 1M visits. No speedup claim.

Focused geometry tests pass, including sprite size/camera changes, current socket
Pose/offset/Parent edits, affine composition, warm-cache lease refusal, pose-work
refusal and atomic cycle refusal. Rust and JS move helpers use global coordinates;
tests retain the 1760-tick ceiling and require release on success/stall/clock
refusal. Greybox's read-only layout snapshot was recorded from the actual reply;
its existing projection/position values and all pins remain unchanged. The first
Rust movement fixture used the opposite stick-Y convention; it now uses G's
negative-Z forward convention. Bun proof tests pass 70/70 with installed Chromium.

Geometry gate completed: 672 game-workspace passes / 14 adapter failures / 23 ignored; 29 app-workspace passes / 3 adapter failures / 1 ignored. Game and all eight app clippy/fmt pass; all 28 Linux normal/paranoid executions pass without moving a pin. Bun 118 pass / 1 skip, caps and boot pass. No non-hardware red was left by this increment.

Typed scenes use a scene-owned `Asset::new(name, digest)` descriptor derived from
G's baked manifest. Lanterns declares `fox.model`, uses `CapsuleController` and
explicitly steps `Animation::play` through G's motion API. Its concrete logic
manifest owns `.shells`, with a separately captured Cargo.lock. The Contract uses
named arguments throughout: G correctly refuses mixing the brief's suggested
trailing named scene with positional arguments. Beacons r5 remains procedural.

N's source-span parser repeatedly rescanned source prefixes; line/column tracking
now advances with its byte cursor. Shared bake bounds cover repeated empty
fragments as well as output entities: 64 MiB total reads, 16 MiB per read and
256 MiB charged expansion/cloning work. JSON/fragment/parent depths remain
64/32/256, entities 100k and component decode allocation 64 MiB. Parent lookups use
a name map. JS dependency discovery is iterative, bounded to 4096 files/64 MiB,
and refuses oversized reads. The unfavorable repeated-fragment and cyclic-edit
controls pass. Scene tests 17/17; the real scratch content rebake measured 9.00 ms
(n=1), zero Cargo targets compiled, and unchanged baker SHA/mtime.

Schema IDs 77/78 add canvas-only save/load tokens without moving action=76. Web,
Apple and Linux scope the opaque checkpoint by app/surface, refuse duplicate
publishers and acknowledge Load only after deferred restore commits. Load uses
G's Open mode. Web quota/write errors retain the preceding value; filesystem
writes replace atomically. Tokens preserve G controls, ChildrenMode and recovery.
Linux's real HUD Save/Load proof preserves exact tick/hash and subsequent save
bytes, as does a fresh-process restore. Apple source and its scoped store test are
carried but cannot execute without its SDK. Production web fixture tests cover
late asset failure, deferred success, unchanged tokens, duplicate ownership and
app isolation (60 focused Bun tests including scene tooling pass).

The enlarged won-state live capture test exposed G paranoid reconstruction
clearing the completed tick's transient input edges. World hashes agreed while
save bytes differed (`released: ["light"]` versus `[]`). Reconstruction now retains
the same input snapshot until the next tick clears edges; ordinary Open/Carry
restore semantics remain unchanged. Actual won/lost live instances now pass
fractional clock, held input, prefix-zero, full replay and full save-byte equality
across Off/Save/FreshGame and Seekable/Live. All 14 Lanterns tests pass. Its wall
fixture was scaled/positioned against G's actual conservative Fox rig bounds;
the original increasing-occlusion, wall identity, LOS and removal assertions stay.

Lanterns Linux runs pass in all three modes. G's `agreePins` accepts the three
independently recorded Linux tick/save inventories; no pins were hand-written.
The first-baseline command requires web as well as Linux, so `pins.json` stays
empty and ordinary `prove --hosts linux` intentionally refuses. The Linux-only
candidate is recorded in scratch, not promoted to a cross-host baseline. Owner
must run `bun game/prove.mjs lanterns --hosts linux,web` on a WebGPU host. Existing
seven-game pins have not moved in this increment. Core selected tests: 519 pass,
3 existing no-adapter registry failures, 1 ignored; selected core clippy passes.

Scene gate completed: game tests 689 pass / 14 no-adapter failures / 23 ignored;
app tests 43 pass / 3 no-adapter failures / 1 ignored. Game and all nine app
workspaces pass clippy/fmt. All 28 established-game Linux normal/paranoid runs
pass without moving a pin. Lanterns additionally passes normal and all three
modes, including fresh-process title Load without Play, and G's Linux-only
pin/save agreement; its first cross-host baseline remains blocked as above.
Bun 121 pass / 1 skip; host web fixtures 94 pass / 5 known platform failures
(four missing xcrun, Caltrain placement 60s timeout). Root build/test/clippy still
refuse the absent lean Hermes producer; root fmt/caps/boot pass. Final checkpoint
checks add a dotted app/surface collision control (JSON tuple key) and allow
opaque non-game surfaces to acknowledge synchronous restore; 57 web production
fixture tests and 37 Linux host tests pass, Linux clippy passes. Apple remains
source-reviewed, not SDK-verified. No incoming assertion was weakened/deleted.

DX joins N's staged runner/motion clone and publication validation with G's
named bindings, child modes, checkpoint observers and device recovery. Bound
restoration finishes declared asset delivery before publication; Continue uses
Carry and selected Restore uses Open. Engine reload diagnostics now include
argument field names, so reordered named values and explicit `{}` never depend
on object enumeration. Shared staging bounds: 256 canvases, 16 asset rounds,
256 deliveries / 256 MiB bytes (64 MiB each), 16 publication rounds / 32 MiB text /
65,536 operations. After a draw, real readiness refusal retains the old module.

The incoming GPU swap destroyed old instances before attaching all new canvases.
Old instances now survive reversible canvas/listener installation, and all
attachment failures restore maps, original canvases and held controls. Child
placement frames are supplied privately before candidate draw; CSS ownership
transfers only with the committed instance. Repeated device loss stays in G's
recovery path. Actual WebGPU child pixels and browser attachment behavior still
require the owner-host sweep; deterministic DOM doubles are not that evidence.

Native ownership is acknowledged per session. Complete detach ACK now releases
G's proof process ownership; partial/no ACK releases nothing. Original named
controls, raw contacts and keyboard aliases are cancelled at handoff. The real
stdio fixture remains alive beyond the old force-kill deadline after natural
parent exit; ordinary and missing-ACK cleanup still end their isolated launch.
Apple SessionOwnership/CanvasInputFocus tests are carried but SDK-unverified.

First DX JS run found missing G lifecycle fields in incoming doubles, a duplicate
checkpoint observer and a misplaced shebang from extracting the driver's world
helpers. One repair round fixed these; assertions stayed intact. The observer
count remains exactly two for a non-input surface across repeated swaps. With
installed Chromium configured, 176 focused JS tests pass / two wasm-dependent
fixtures skip; the latter execute through the Rust web fixture runner. Additional
shared-budget/readiness tests are included in the full increment gate.

DX gate completed: 689 game passes / 14 no-adapter failures / 23 ignored; 43 app
passes / 3 no-adapter failures / 1 ignored. Game and all nine app clippy/fmt pass.
All 28 established-game Linux executions pass with unchanged pins. Lanterns'
three independently recorded Linux inventories agree again, including complete
save bytes; initial Linux/web baseline is still unavailable. Game Bun: 131 pass /
1 skip. Final web fixture sweep: 138 pass / 5 platform failures / 2 conditional
skips. Those two production runner/glue transaction cases execute through Rust;
selected runner/web/motion tests: 143 pass / 1 ignored, selected clippy passes.
Root fmt/caps/boot pass; root build/test/clippy remain blocked by lean Hermes.
The four missing-xcrun tests and Caltrain 60s placement timeout remain unchanged.

Full verification caught two duplicate web property arms, removed in one repair
round, and a successful Each-mode replacement retaining the old CSS transform
until the next frame. Candidate placements now apply at commitment; the original
placement assertion passes unchanged. The real runner/glue fixture also retains
its assertion that publication occurs after the host commits. Reversible canvas
attachment precedes commitment; unexpected later presenter failures are reported
as committed (`retained:false`) and retire the predecessor. Its incoming GPU
doubles now acknowledge G's deferred restore lifecycle. Each failure cleared in
one repair round. Three child-mode tests and the complete-session-roster ACK
negative controls pass. Apple product-resolution fixture: 1 pass; no Swift toolchain
is installed. No GPU, Apple SDK or browser-rendering success is inferred here.


## Increment 10 — fixed I3 evidence and opt-in trials

The fixed consumer is `game/verification/lanterns` (own generated `.shells`
workspace, captured lock, `lanterns-evidence` save identity). It carries the twelve
lanterns, original ordinary-input script, four independently compiled edits,
52-entity Rust-construction equality, and all original apex/moving-crate dynamic,
queued-input, collision, timer, spring, animation and reload assertions. The
construction comparison now also checks complete world bytes. Model declarations
use the baked Fox; controller/animation fields follow G. Historical binary literals,
605 continuations and the 612-entry migration inventory remain at immutable N;
none are migrated or overwritten. New evidence is v7 under a distinct identity.

The CPU invariant test runs Off/Save/FreshGame and compares every generated
checkpoint/continued save and full observation record byte-for-byte. The real host
proof then restores the same ordinary-input checkpoints through Carry and checks
all 1,210 observations (five builds × two moments × 121 instants), including full
save bytes, against that oracle. No queued control field or assertion is removed.
The recorder gains `pin(tick,state,key=String(tick))`; it retains numeric tick
validation and complete-inventory/three-mode agreement. The first shared pins file
stays empty until Linux and web both agree. The fixed app has no historical pin to
move. Bound: ten probes, 120 continued ticks each, production scene/restore limits.
The edited contact edge, changed clip palettes and moving-crate velocity/spin are
negative controls; a removed feature cannot return empty evidence and pass.

An explicit driver launch option `worldMode:'carry'` passes existing Restore::Carry
through Linux/web/Apple; default Open and HUD checkpoint load remain unchanged.
The G proof resolves external app paths consistently, including build target and
process launch. R14 must keep authored merge after typed world decode; when adding
argument-aware `Game::register`, this fixed consumer's variant dispatch must follow
its setup/tick branch. No pre-decode authored mutation was added here.

The first host byte comparison exposed test-environment differences. The oracle
now uses the same integral-millisecond host lattice and 1280×720 viewport as the
carrier. Full bytes then agree, including queue phase; nothing is normalized out
of saves. Two fixture repair rounds, with the assertion unchanged. The old
construction test needed G's declared baked asset digest; it retains all names,
handles, parents, global-transform/hash assertions and additionally checks bytes.

G runs animation inside Game::tick. Adapted CPU tests use production baked clip
sampling/joint matrices and the real primitive Feed/Writes path. Timing reports
whole simulation (including animation) and primitive feed, without inventing a
separate animation/GPU timing or subtracting medians. Apex still lacks moving crate
and blending; the supplementary ordinary-input checkpoint proves nonzero velocity
and spin. No stronger coverage claim is made.

Trials remain opt-in, with immutable prompts, unchanged assertions/negative routes,
parent-owned receipts, complete timestamp/unknown fields and a 900-second ceiling.
Version 4 changes only G adapter field/asset names and artifact target selection.
`--prepare-only` validates archive packaging, locks, build, package tests, same-ref
negative controls, Landlock content denial and cleanup without auth or agent
launch. Every archived consumer is prepared before its implementation is stripped;
only metadata and empty stubs remain. Fixed verification, templates, research,
evaluators and inherited instructions are also removed. No warm target tree is
copied. Preparation shares this clone's target; an explicitly requested real trial
gets its own target and removes it afterward. All three historical result sets and
methodology, including protocol defects and lack of speed evidence, are preserved
verbatim with immutable path/ref provenance in `006-historical-change-trials.md`.


Increment 10 preliminary measured evidence: real Linux continuous host ran all
1,210 observations and complete-save comparisons successfully; first baseline
remains unverified until cross-host agreement. Focused Bun proof/reload/surface
fixtures: 183 passed, two actual-wasm cases conditionally skipped (those passed
with the built bridge in increment 9). Fixed construction unit: one passed; I3
CPU invariant integration: one test covering all three modes passed. CPU feed /
changed-clip negative controls: two passed. Release 120-tick CPU medians:
simulation clock operation including animation/reply 230,926 ns; primitive feed 24,056 ns; VmHWM 18,432
kB; final hash `0xe616dc56c02e8795`. These are one local run, not a speedup claim.

Archive preparation passed against immutable prepared-source object
`47e8a17727654aac3e5a7b68676c5039bb58c98b` (prepared snapshot; no branch checkout or agent dispatch). Warm production bake 63.731 s; package tests 11 + 3
passed. Evaluator controls: 8 passing cases, 14 expected failures, six unsupported
rendering cases across 28 cases. Acceptance's unchanged direct crate route is the
one baseline mechanics failure; Task A/B fail for the absent requested features.
All controls emitted full parent receipts, with no unavailable state fields.
Landlock's real read probe denied evaluator content and allowed resolver/root
capability reads. Auth was never copied; workspace/build/auth paths and generated
PNGs were removed. Shared parent build cache is retained. Receipt:
`scratch/land-exec/trials/47e8a17727654aac3e5a7b68676c5039bb58c98b/a-1/trial.json`.
Prompts and six evaluator hashes are recorded there; the historical three cohorts
remain separate from this untimed preparation.


The explicit `plan + world` web launch required one additional transactional edge:
its supplied checkpoint now takes precedence over a running world (including a
world whose assets are still loading). The candidate consumes that carrier only
privately until commit, preserves saved input, and abort retains the public carrier.
Ordinary authored reload still releases physical input. A regression proves those
branches without a GPU; the production web proof remains a receiving-host job.
The one-shot world-mode flag clears together with its checkpoint after commit.
Focused final web launch/reload/surface fixtures: 103 passed, two actual-wasm
cases conditionally skipped. This fixture is included in the full web sweep too.


Increment 10 complete gate sweep: game workspace 691 passed / 14 no-adapter
failures / 24 ignored diagnostics; app workspaces 43 passed / three no-adapter
failures / one ignored; fixed app two tests passed (integration exercises all
three modes). Workspace plus ten app clippy/fmt checks passed. Existing seven games
passed 28 Linux proof executions (normal + three paranoid modes), with unchanged
pins. Live Lanterns and fixed evidence each passed three direct Linux runs; the
G `agreePins` comparator accepted complete identical mode inventories. Fixed keys:
1,210 ticks and 1,210 saves. Live Lanterns retains the same three ticks/two saves
reported in increment 8. These Linux-only candidates stay in scratch; both
`pins.json` files remain empty and `prove --hosts linux` correctly refuses their
first baseline without web. Game Bun: 132 passed, one skipped. Web: 140 passed,
five platform/environment failures (four missing `xcrun`, Caltrain Chromium timeout),
two actual-wasm conditional skips. Root build/test/clippy remain blocked by the
missing lean Hermes producer; root fmt, caps and boot pass. No new unexplained
red or weakened assertion. Cubes remains the recorded browser-only benchmark.

## Increment 11 — strict residency acceptance and final handoff

`restoring_fox_uploads_zero_asset_bytes_after_ready` is now an ordinary, unignored
test over G's baked Fox and actual GPU residency. Existing residency tests moved
unchanged to `surface_residency.rs` to preserve the source cap. The new test first
requires a real adapter, refuses ready/save while the final texture is missing,
and requires positive initial mesh and texture uploads. It warms actual animation,
then checks absolute upload counters and complete saves through three repeated
Carry/Open pairs and eight ticks in each paranoid mode. The pose must actually
change. Same-name changed texture/model contents must upload; identical redelivery
must not. Device replacement must rebuild residency and reach real ready. There is
no skipped branch, recording backend, counter reset allowance or upload subtraction.
Work is fixed: six restores, sixteen continued ticks and bounded fixture assets.

Explicit invocation compiles and fails (0 passed, 1 failed, 0 ignored) at adapter
creation: `No suitable graphics adapter found`; Vulkan drivers/libraries could
not be loaded, and noop/Metal/DX12/GL/WebGPU support is not compiled into that
native fixture. The residency assertions never execute here. This is an exact
unverified acceptance, not a claimed fix demonstrated by this box. The incoming
known-defect evidence remains at
`360921d386f212965cbc28229f21c93d280a753a:game/render/src/surface_ready.rs:338`;
the production replacement is G residency, not its T6 instrumentation.

Primitive artifact measurement used the unchanged
`bun game/bench/size.mjs land-final --app beacons`. Production wasm build succeeded;
`wasm-opt` is unavailable and the report then refuses missing Twiggy. The resulting
unoptimized GPU module is 2,306,840 bytes / 510,282 gzip, SHA256
`97650b8eb201f2a05a2f02499ba0a873ff48d62453cde748cda37d03bf70f1f2`.
Preopt module: 4,842,394 bytes. `WebAssembly.compile` validates the module. A scratch
function-name audit demangles all 8,017 entries (2,501 exact-game entries) with the
installed Rust demangler: no GLTF runtime, image decoder, animation execution,
model/sprite pipelines, Sprite type or T6 backend names. Primitive particle
pipelines and model type drop glue remain. This limited static audit is neither
byte attribution nor real GPU boot evidence; no comparison to G's optimized size
is valid. No optimizer or analysis dependency was added.

Affected core crates (kernel, plan, runner, Linux/web hosts, motion, GPU and all
Contract compiler layers): 535 passed, three failed at native GPU registry load,
one ignored. Their clippy checks pass. The two actual production wasm host/glue
transaction fixtures also pass via that Rust suite; the ordinary Bun sweep's
conditional skips do not hide missing execution of those two cases. The native
GPU failures are `retiring_each_releases_textures_and_zero_frame_releases_a_capture`,
`replacement_lost_during_preparation_refuses_then_retries`, and
`recovery_of_a_healthy_device_does_not_prepare_again`: registry load returns 1
instead of 0 without a device. Other adapter-optional tests returning `ok` here
are not certified GPU executions.

README remains current API/commands/constraints; this diary contains landing
evidence. The as-built LLP states the combined contracts and R14 decode ordering.
QUEUE now distinguishes cleared headless failures from unverified Apple focus,
real-GPU residency, first baselines, I3 blend coverage and measured physics tails.

Final broad gate: 51 commands completed. Game workspace: **691 pass / 15 fail /
24 ignored**. Nine ordinary app workspaces: **43 pass / 3 fail / 1 ignored**;
fixed app: **2 pass**, including the full three-mode invariant test. All Rust
failures are the named GPU cases below. Game workspace and all ten app workspaces
pass clippy `-D warnings` and fmt. Affected core: **535 pass / 3 fail / 1 ignored**,
clippy passed; its two actual-wasm Bun transaction fixtures passed as well.
Game Bun: **132 pass / 1 skip / 0 fail**. Web fixtures: **140 pass / 2 conditional
skips / 5 fail**. Four failures invoke unavailable `xcrun`: Mac/IOS clears
zero-sized and display-none captures, and Mac/IOS hidden placement box is zero.
The fifth, `Caltrain web frame-only stack remains a plain column`, still times
out after 60 seconds with fleet Chromium; its underlying cause is unresolved.
The two conditional cases did execute successfully through the Rust wasm runner.

Root build/test/clippy all stop in the existing TypeScript bakes: "TypeScript
bake requires the lean Hermes executor on this producer" (Messages, Fieldnotes,
Update Lab). Root fmt, caps and boot pass. Boot graph: two reachable JavaScript
modules, one wasm reference, 88,500 JS bytes and a 3,298-byte page; this source
graph is not real GPU startup evidence. Caps scanned 730 sources, with generated
and vendor exclusions, all within the declared budgets. No added unsafe code
exists outside game storage. The seven established games pass **28 Linux proof
executions**, normal plus all three paranoid modes, preserving every world and
current save pin. The four first-baseline commands for live/fixed Lanterns
intentionally refuse Linux-only publication, because web has not agreed.

The final direct collectors then completed **six additional Linux runs**: live
Lanterns and fixed I3 each in continuous, Save and FreshGame. Every assertion
passed, every recorded child exited, and G's `agreePins` accepted identical complete
inventories. Live inventory: three tick keys and two saves; fixed inventory:
1,210 tick keys and 1,210 full-save keys per mode (3,630 comparisons across modes).
Collector status remains `PROOF UNVERIFIED` because neither first shared baseline
exists. Only scratch candidates were written; both committed pin objects stay empty.
This distinction is intentional and no assertion or publication guard was relaxed.

No browser rendering success is claimed: the single attempted production
Beacons/SwiftShader proof obtained no WebGPU adapter and failed on host exceptions.
Cubes' inherited proof is Chrome-only, ignores a Linux argument and has no pins;
the browser attempt is unverified. The primitive size command builds successfully
but cannot finish its optimized/attributed report without Binaryen and Twiggy.
The trial preparation's baseline direct-crate route and unsupported pixels are
recorded in increment 10; its expected absent-feature failures are negative
controls, not successful change trials.

## Receiving-host work — exact remaining matrix

Use Bun 1.4.2, the captured app locks and `EXACT_UPDATE_TRUST=development`. Record
an actual obtained adapter; tests with no-adapter early returns are insufficient.

1. On a native GPU host, rerun
   `cargo test --manifest-path game/Cargo.toml --workspace --no-fail-fast`,
   `bun game/app/shells.mjs --test`, and the fixed app's `.shells` workspace tests.
   In particular run
   `cargo test --manifest-path game/Cargo.toml -p exact-game-render restoring_fox_uploads_zero_asset_bytes_after_ready -- --nocapture`
   **without `--ignored`**, then
   `cargo test --manifest-path game/Cargo.toml -p exact-game-render surface_lifecycle -- --ignored --nocapture`.
   Rerun `cargo test -p exact-gpu --no-fail-fast` for the three native registry
   failures. Inspect actual rendered pixels, not just the tests' exit code.
2. With production Chrome/WebGPU, run
   `bun game/prove.mjs <game> --repin --hosts linux,web` for each of
   `asset-fixture`, `beacons`, `greybox`, `particles-fixture`, `placement-fixture`,
   `skinned-fixture`, `sprites-fixture`. All world pins and the seven v7 save pins
   must equal this branch; only host/provenance fields should need publication.
   This executes all three modes on both hosts. For the first baselines run
   `bun game/prove.mjs lanterns --hosts linux,web` and
   `bun game/prove.mjs game/verification/lanterns --hosts linux,web`, without
   `--repin`. Require complete inventories and full-save equality at all 1,210
   fixed observations. Run `bun game/bench/cubes/proof.mjs web` separately; that
   existing benchmark has no Linux proof or pins file.
3. On macOS and iOS simulator, run
   `bun game/prove.mjs <game> --hosts macos,ios --compare-saves` for those seven,
   live Lanterns and `game/verification/lanterns` after their baselines exist.
   Run `swift test --package-path host/apple`, `bun scripts/smoke.mjs host`, and
   `bun scripts/smoke.mjs host-ios` with their built embedding fixtures. Exercise
   play/restart/save/load after HUD commit, restore with held controls, editor
   focus, multiple canvases, complete/partial/failed detach ACK, late model
   textures and actual presentation after device recovery. Recheck Beacons'
   previously reported victory-control logical focus assertion. Simulator input
   does not establish physical-touch timing; no device-phone result is claimed.
4. On browser and Apple, compare geometric layout with intended sampled visibility
   in perspective/orthographic/integer-scale scenes: parents, sprites,
   transparency/depth, mirrored and skinned geometry, affine sockets, missing
   assets and stale/unavailable poses. Run the placement/sprites/skinned proofs
   above and inspect pixels. Geometric samples are not exact raster visibility.
5. On browser and Apple, capture an actually live Lanterns won/lost instance before
   any restore, including held input and fractional clock phase. Replay prefix
   zero and the complete stream, continue, and check the first controlled seek.
   Exercise malformed capture, wrong artifact, early live phase and partial/failed
   ownership ACK. Headless regression tests passed, but live scheduling is owed.
6. On real WebGPU, lose the device during asset delivery and during staged authored
   reload/publication; test commit and rollback. Check rebuilt Fox pixels, child
   placement, controls, requested/loaded identity and real ready transitions.
   Exercise explicit plan+world Carry while the predecessor's assets are loading.
7. In the warm producer with lean Hermes, Binaryen and Twiggy, rerun the root five
   checks and `bun test ./host/web/tests` (with Chrome and `xcrun` available), plus
   `bun game/bench/size.mjs land-final --app beacons`. Confirm optimized size and
   attributed primitive isolation, then actually launch primitive Beacons and a
   starter app. The local size command's missing tools and Caltrain's 60s Chromium
   placement timeout remain red until this succeeds.

The trial harness is opt-in, not a receiving-host blocking gate. Its prepare-only
controls disclosed one unchanged baseline direct-crate route failure and six
unsupported rendering cases. Full agent trials were intentionally not dispatched.
Historical trial findings make no speedup claim. I3 still lacks the simultaneous
apex/moving-crate/active-blend case, and EXPHYS deferred-bit negative-control debt
is retained. Those are stated coverage debts, not assertions removed to get green.


## Named red Rust tests on this box

All failures below require a GPU. Game/app failures report no suitable adapter;
the three core tests fail their initial native registry-load success assertion.
No assertion was removed or weakened, and the strict Fox case is unignored.

Game workspace:

- `models::retirement_regressions::pending_names_count_retired_bytes_and_compaction_keeps_hero_handles`
- `placed::tests::captured_children_share_draw_and_hit_depth_and_a_wall_occludes_them`
- `quad_tests::equal_depth_uses_layer_then_slot_and_mask_respects_cutoff_with_signed_scale`
- `quad_tests::particle_storage_and_pipelines_prepare_only_with_emitters`
- `quad_tests::invalid_quads_are_journaled_without_refusing_valid_neighbors`
- `quad_tests::retired_sprite_waits_for_redelivery_and_reuses_identical_texture`
- `quad_tests::same_owner_sprite_then_particle_is_pinned_and_adjacent_sprites_batch`
- `renderer::packing_tests::oversized_retired_arenas_pack_without_reuploading_live_meshes`
- `skinning::normal_tests::skinned_normal_is_inverse_transpose_under_scaled_rotated_joints`
- `skinning::tests::displayed_affine_matches_gpu_with_animated_translation_scale_and_rotated_child`
- `surface::residency_tests::identical_redelivery_survives_entry_and_post_acceptance_budget_compaction`
- `surface::residency_tests::restoring_fox_uploads_zero_asset_bytes_after_ready`
- `surface::tests::placement_and_renderer_share_warnings_across_restore_and_prune_dead_followers`
- `surface::tests::world_surface_retires_and_readds_a_real_placed_child`
- `affine_attachment_pixels_match_transformed_vertices_and_detach_cleanly`

App workspaces:

- `first_presented_fox_matches_current_pose_in_fox_rectangle`
- `rendered_atlas_and_mid_fall_restore`
- `primitive_particles_render_and_do_not_pick`

Affected core:

- `native::placement_abi_tests::retiring_each_releases_textures_and_zero_frame_releases_a_capture`
- `native::placement_abi_tests::replacement_lost_during_preparation_refuses_then_retries`
- `native::placement_abi_tests::recovery_of_a_healthy_device_does_not_prepare_again`

## LANDFIX1 — fresh touch parity on a real Mac GPU

The unmodified reproducer failed on this Mac with the reported hashes:
rendered `9202442258233057753`, headless `10118339518646435961`.
A temporary test dumped `state world:*`-equivalent component JSON and resource/
clock state at 0, 17, 34 and 50 ms, and compared rendered world bytes against
a direct `Sim::new(())` given the same viewport and input. It was removed after
diagnosis; local output is in `game/target/landfix1/diagnostic-canonical.log`.

| Host ms | Rendered tick / player Transform.position.x | Headless test tick / x | Direct Sim tick / x |
| --- | --- | --- | --- |
| 0 | 0 / 0 | 0 / 0 | 0 / 0 |
| 17 | 1 / 1 | 0 / 0 | 1 / 1 |
| 34 | 2 / 2 | 0 / 0 | 2 / 2 |
| 50 | 3 / 3 | 0 / 0 | 3 / 3 |

The first mismatch is at 17 ms: world tick 1 versus 0, and
`player.Transform.position.x` 1 versus 0. Resources are empty on every path;
rendered and direct Sim world saves are byte-identical at all four samples.
There is no tick-zero construction, authored-base or restore divergence.

Root cause: `93782c1c` made inspection read-only, but the old renderer test
still used `state(now:17,width:64,height:64)` as its headless frame.
That neither advances the clock nor installs the input viewport, so the test
compared an advanced rendered world to an untouched headless world.
Restoring the old inspection side effect would violate the documented ownership/
capture semantics and the existing engine regressions.

The fix uses the explicit `clock` operation with dimensions, shared by the
GPU test and a new device-free headless-presentation-versus-Sim regression.
The original hash assertion remains; explicit tick assertions now precede it.
The device-free case compares the initial hash and full EXSIM bytes, then
tick, position, hash, world bytes and full EXSIM bytes after each of five
touch down/move/up/down/cancel steps. Incidental timestamped state reads must
leave the save unchanged. Before the clock fix, this new regression failed
without requesting an adapter: expected tick 1, got 0.

Verification (one fix round; prescribed Mac environment; Cargo run from `game/`):

- `cargo test -p exact-game-render --lib fresh_touch_region -- --nocapture`:
  2 passed, 0 failed, 0 ignored (GPU original and device-free regression).
- `cargo test -p exact-game-render --lib`: 108 passed, 0 failed, 6 existing
  ignored tests; the actual adapter executed the GPU tests.
- `cargo test -p exact-game --no-fail-fast`: 452 passed, 0 failed, 8 existing
  ignored tests across 28 suites, including 37 passing doctests.
- `cargo test -p exact-game-render restoring_fox_uploads_zero_asset_bytes_after_ready -- --nocapture`:
  exit 0. The real Fox acceptance ran and passed, without changes to that test
  or its implementation. Its unfiltered library result was:

```text
running 1 test
test surface::residency_tests::restoring_fox_uploads_zero_asset_bytes_after_ready ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 113 filtered out; finished in 0.22s
```

The named Fox command's integration-test binaries each had zero matching tests;
they also exited successfully. Local full output is `game/target/landfix1/fox.log`;
the engine and renderer results are beside it as `engine.log` and `render-lib.log`.
Changed-file rustfmt, staged caps, boot and `git diff --check` passed.
No runtime changes, pin changes, assertion removals, pushes or sub-agents.


## LANDFIX4 — deterministic saved journals, unsaved session diagnostics

`World::log(&self, impl Display)` still records exactly the game's events. EXSIM
v7 saves its independent 4,096-entry game ring and game cursor. The new
`World::session_log(&self, impl Display)` writes only the unsaved combined log
view; ownership, host/adapter diagnostics and their ordering metadata cannot
change saved bytes or a hash. Agent `logs`, `World::journal()` and `journal_next()`
keep the existing event text and append order, including same-tick interleaving.
No prefix-based filtering can accidentally remove a game's own log message.
The v7 `overflow_logged` wire slot is always false; the diagnostic throttle is
session state, retained through in-place/paranoid reconstruction.

Restores reinstate saved game history and merge retained session diagnostics at
their game-event boundaries. Deferred asset setup keeps attachment diagnostics
that arrived before setup. FreshGame reconstruction retains the same unsaved
history as Save. The first implementation kept the destination's game history
instead of loading the checkpoint's history; the unchanged fixed Lanterns
continuation assertion caught it. The merge correction and a dedicated regression
landed in `db2f7db5`; no assertion was weakened. The other repair was a missing
Character import in the new test fixture. All edits stayed in this clone.

Every rerouted telemetry write (line numbers in the landed source):

| File | Lines | Diagnostic |
| --- | --- | --- |
| `game/engine/src/sim.rs` | 435 | construction/restart outcome |
| `game/engine/src/sim.rs` | 518, 652 | invalid ordinary/source-attested input |
| `game/engine/src/sim.rs` | 606 | dropped-input queue overflow |
| `game/engine/src/sim.rs` | 643 | agent attach/detach and clock ownership |
| `game/engine/src/sim.rs` | 660 | external-human contamination |
| `game/engine/src/agent.rs` | 245 | agent request/capture refusals |
| `game/engine/src/world/reload.rs` | 502, 509 | reload items and omitted-item count |
| `game/engine/src/animation.rs` | 1205, 1213, 1221 | adapter carry insertion refusals for Animation/Blend/Animator |
| `game/render/src/quads.rs` | 214, 223 | invalid sprite/emitter presentation |
| `game/audio/src/lib.rs` | 302 | executor refusing a non-looping AudioSource |

Restore/checkpoint refusals, host reload diagnostics and screenshot notes already
live outside Sim in the Linux/web/Apple host journals/state; those paths were
kept. Greybox's real Linux proof still passes “restore refusal is in the canvas
journal.” Spawn/despawn/publication, authored `world.log`, game animation markers,
physics events, audio simulation events and deterministic scene/emitter diagnostics
retain their original game-owned logging path.

Five device-free regressions live in `engine/src/world/journal_tests.rs`:
ownership/clock changes + refused-restore reporting + reload report preserve the
complete save and hash; same-tick interleaving survives Off/Save/FreshGame; 10,000
rounds of host/game/host churn preserve the clean simulation's entire save;
attachment before versus after asset delivery gives identical saves; actual
authored reload reports stay out of the next save; and restoring another world's
saved event history merges diagnostics without duplicate logs on a second restore.
The five tests group these assertions; they all pass. The fixture contains a
Character, the loading case really defers setup, and game-event persistence plus
exact nonempty log sequences are negative controls against returning nothing.

Append bookkeeping is O(1), session formatting O(message bytes), capped at 65,536
UTF-8 bytes per message. Oversized text emits the explicit
`session journal refused: line exceeds 65536-byte budget` diagnostic; the exact
boundary and one byte beyond it are tested. The combined view retains 4,096
entries; reads visit at most 4,096 and restoration merges at most two such rings.
Session churn never evicts saved game events. Game-owned message text remains
unchanged and is governed by the existing game API, not the session text cap.

All seven `bun game/prove.mjs <game> --repin --hosts linux` commands passed
continuous, Save, FreshGame and release agreement. Every game also passed its
normal Linux proof and `linux --paranoid` (28 ordinary/paranoid executions).
Beacons was repinned again against the final implementation. All tick inventories
and tick hashes are unchanged; only pin provenance metadata changed. No Linux
continuation contained a rerouted telemetry line, so no baseline value moved.

| Game | Old continuation SHA-256 | New continuation SHA-256 |
| --- | --- | --- |
| beacons | `0eb3e3de0f622153a875a04cc26d29a57f4da8ff477b07fde08933c3c1cfaac1` | `0eb3e3de0f622153a875a04cc26d29a57f4da8ff477b07fde08933c3c1cfaac1` |
| greybox | `4a14675296f891ef23732a91a1a9698e028ccd4f57159dbe6505e975d483ada4` | `4a14675296f891ef23732a91a1a9698e028ccd4f57159dbe6505e975d483ada4` |
| asset-fixture | `a4be74d1e6e64d344abdc21e5359f99505c8ccaaeffc4a223046c6960b5d6b43` | `a4be74d1e6e64d344abdc21e5359f99505c8ccaaeffc4a223046c6960b5d6b43` |
| skinned-fixture | `9b1e9a3d9536f22f5de8fda464ec109a62c39b4b87e1c53163b048113651906f` | `9b1e9a3d9536f22f5de8fda464ec109a62c39b4b87e1c53163b048113651906f` |
| particles-fixture | `0b6a72ae80d2e9bfb745a69d472e097824b41f3eeb541c1a771bd3bbeed14616` | `0b6a72ae80d2e9bfb745a69d472e097824b41f3eeb541c1a771bd3bbeed14616` |
| sprites-fixture | `655da85cb8bd062f398efa16e7a505cca58843de511e740c64d8020d70014916` | `655da85cb8bd062f398efa16e7a505cca58843de511e740c64d8020d70014916` |
| placement-fixture | `4e6d679868d48183d2d97ad22fcb86d6c62f762e2a59c3348f430cdaa624b5f5` | `4e6d679868d48183d2d97ad22fcb86d6c62f762e2a59c3348f430cdaa624b5f5` |

Final verification on this Linux producer (`EXACT_UPDATE_TRUST=development`,
all supplied zero-debug/no-incremental environment settings retained):

| Suite | Passed | Failed | Ignored/skipped | Failure boundary |
| --- | ---: | ---: | ---: | --- |
| Game workspace, `--no-fail-fast` | 738 | 18 | 24 | GPU adapter required |
| Nine ordinary app workspaces | 48 | 2 | 1 | Sprite/particle GPU tests |
| Fixed Lanterns workspace | 4 | 0 | 1 | — |
| Affected core (kernel/plan/runner/hosts/motion/GPU/Contract) | 542 | 3 | 1 | Native GPU registry load |
| `cd game && bun test` | 146 | 3 | 1 | Missing Chrome |
| `bun test ./host/web/tests` | 145 | 4 | 3 | Missing Apple `xcrun` |

The strict Fox residency test ran as part of the workspace and failed because no
adapter exists; it was not skipped or weakened. The other GPU errors likewise
report no suitable adapter. Bun's named failures are browser reuse/IndexedDB,
E10 keyboard focus/hover, and the newly generated game's web proof; its Linux
proof passed. The four web-fixture failures are Mac/IOS zero-size/display-none
capture and hidden placement tests. Real browser rendering, GPU residency/pixels,
Apple execution and the Chrome-only Cubes proof remain unverified.

Game, affected-core and all ten app/fixed workspace Clippy runs passed with
`--all-targets -- -D warnings`. All workspace formatting checks passed. Four app
Clippy runs initially reached host bakes before their generated surface declaration
existed; checking their GPU shell first and rerunning cleared them. The initial
core sweep also lacked frozen Bun dependencies/generated game metadata;
`bun install --frozen-lockfile` and shell preparation cleared its formatter and
TypeScript failures. There is no residual source fix or queue item for those
preparation failures. Root build/test/Clippy were all attempted and remain blocked
by the producer's missing lean Hermes executor in TypeScript app bakes.

Live and fixed Lanterns each ran three direct Linux proofs with scratch-only
collection, without publishing their empty first baselines. Every assertion and
child cleanup passed. Exact inventory comparison confirmed three tick/two save
keys for live Lanterns, and 1,210 tick/1,210 save keys for fixed Lanterns, identical
across continuous/Save/FreshGame. Status is still `UNVERIFIED`: Linux agreement
alone cannot establish their first Linux/web baseline. The fixed reference also
compares every full continued save including queued input against the real host.

Staged caps and boot passed: 742 sources within limits; two reachable JS modules,
one wasm reference, 88,699 JS bytes and a 3,468-byte page. No unsafe code, assertion
weakening, new workspace membership, remote Git action, push, worktree or sub-agent.
Disk stayed above the 25 GiB stop threshold (about 60 GiB free at completion).
Detailed command exits, proof summaries and logs are in
`~/lanes/gamenext/scratch/landfix4/`; no generated build output is committed.

This machine has no Chrome. The owner must rerun the reported reproducer:

```sh
bun game/prove.mjs beacons --repin --hosts linux,web
```

Then exercise the same agreement for the other six established games:

```sh
for game in greybox asset-fixture skinned-fixture particles-fixture sprites-fixture placement-fixture; do
  bun game/prove.mjs "$game" --repin --hosts linux,web
done
```

No web agreement is claimed by this Linux-only receipt.
