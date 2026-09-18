# Lanterns change trials — interleaved after measurement

**These data do not support an authoring-speed improvement or elimination of the
capture failure.** After A1/A2 pass Task A functionally; after B1/B2 fail the
inspection phase contract. Fresh baseline A3 passes A and B3 fails that same B
contract. After B2 again hits **“corrupt checkpoint: semantic state/hash differs”**
and removes the failing live-terminal capture assertion. No exact2 row is a
strict full pass: the unchanged common crate route fails and world pixels are
unavailable on this host.

The after Task A mean is **512.4 s**, versus **313.9 s** for fresh baseline A3
(+63.3%); after Task B is **330.0 s**, versus **216.4 s** for fresh baseline B3
(+52.5%). These are descriptive differences, with two after observations and one
fresh baseline observation per task, not estimated causal effects or reliable
population averages. The larger after fixture suite created substantial work in
both A sessions. The B sessions chose different routes and state representations.
The original faulted attempts remain below; none is silently excluded or replaced.

## Protocol and complete measurements

After is pinned to `7c65726ca8be72a3f10de3a44a836e1b3b00eed5`; baseline is
`e47a09fc36c07103cdc3645b7061cca305af6c14`. `next/trunk` had moved, so the explicit
requested commit wins. Each session is fresh `gpt-5.6-sol`, high reasoning,
900-second wall ceiling, no sub-agents, and at most three source repairs per
failure loop. All six completed below the ceiling; none was restarted, aborted,
or replaced. Original baseline A1 remains an infrastructure abort, not a success
or a zero-second observation. The timeout termination path was not exercised.

The evaluator is **version 3, unchanged**. All six current file hashes equal the
original four submissions' final v3 hashes; receipts are in
[evaluator-hashes.json](evidence/I2/evaluator-hashes.json). No evaluator correction
or retrospective verdict change was made in I2. Original prompts were recovered
from retained scratch evidence: both A prompts match byte-for-byte, as do both B
prompts. The runner now reads those [A](prompt-a.txt)/[B](prompt-b.txt) copies from
this clone instead of another lane. Prompt equality is checked for all six new
attempts. Landlock probes deny evaluator reads, allow the resolver, and permit
the directory-capability root; no inherited instruction files, history, memories,
or prior submissions enter the CLI session. Dependencies remain source-available.
The context-packaging defect and correction below limit the isolation claim.

Times are wall seconds. Agent time includes CLI startup and the model's own
verification. Parent build/evaluation and all pre-dispatch work are separate.
There was no existing Cargo target in this clone; each archive built before
dispatch. Setup builds measured 99.4–102.8 s, and total preparation 123.8–128.0 s.
No checkout verification build ran alongside a trial. External service queue
delay and shared-machine load are unknown, not assumed zero.

| Cohort | Attempt | Agent s | Build s | Eval s | Outcome | Uncached input / output tokens | Files | + / − text lines | Build / proof / test calls | Repairs (largest loop) |
|---|---|---:|---:|---:|---|---:|---:|---:|---:|---|
| Baseline original | [A1](evidence/next-baseline/a-1/trial.json) | 620.1 | 17.7 | 18.0 | Infrastructure abort; no edits | unknown | 0 | 0 / 0 | 0 / 0 / 0 | No model work |
| Baseline original | [A2](evidence/next-baseline/a-2/trial.json) | 516.2 | 17.6 | 20.5 | Task pass; instrument/context deviations | 83,805 / 15,178 | 7 | 158 / 12 | 1 / 1 / 3 | 1 (1) |
| Baseline original | [B1](evidence/next-baseline/b-1/trial.json) | 268.1 | 18.5 | 19.8 | Task pass | 54,432 / 9,254 | 5 | 51 / 16 | 0 / 1 / 4 | 3 (3) |
| Baseline original | [B2](evidence/next-baseline/b-2/trial.json) | 494.9 | 17.6 | 17.9 | Fail: phase contract | 122,573 / 17,719 | 5 | 83 / 19 | 0 / 2 / 9 | 3 (2) |
| Baseline fresh | [A3](evidence/I2/baseline/a-3/trial.json) | 313.9 | 17.5 | 20.6 | Task pass | 67,853 / 11,414 | 7 | 160 / 12 | 1 / 1 / 5 | 3 (2) |
| Baseline fresh | [B3](evidence/I2/baseline/b-3/trial.json) | 216.4 | 17.6 | 17.7 | Fail: phase contract | 52,451 / 8,549 | 5 | 60 / 18 | 0 / 2 / 1 | 1 (1) |
| After | [A1](evidence/I2/after/a-1/trial.json) | 541.6 | 20.0 | 21.3 | Task pass | 178,032 / 18,170 | 13 | 906 / 671 | 1 / 2 / 12 | 8 (3) |
| After | [B1](evidence/I2/after/b-1/trial.json) | 259.9 | 18.3 | 18.7 | Fail: phase contract | 63,217 / 9,696 | 5 | 65 / 15 | 0 / 1 / 4 | 3 (2) |
| After | [A2](evidence/I2/after/a-2/trial.json) | 483.3 | 18.4 | 21.5 | Task pass; context deviation | 154,867 / 18,857 | 13 | 815 / 696 | 0 / 3 / 6 | 4 (1) |
| After | [B2](evidence/I2/after/b-2/trial.json) | 400.0 | 18.2 | 18.7 | Fail: phase contract | 80,958 / 14,329 | 5 | 103 / 27 | 0 / 3 / 5 | 3 (2) |
| babylon reference | A1 | 407.2 | 2.0 | 20.9 | Pass | 63,450 / 13,307 | unknown | unknown | unknown | unknown |
| babylon reference | A2 | 468.7 | 2.1 | 20.4 | Pass | 51,682 / 12,038 | unknown | unknown | unknown | unknown |
| babylon reference | B1 | 220.4 | 2.2 | 20.2 | Pass | 49,007 / 8,330 | unknown | unknown | unknown | unknown |
| babylon reference | B2 | 347.3 | 2.1 | 19.9 | Pass | 53,998 / 13,441 | unknown | unknown | unknown | unknown |
| playcanvas reference | A1 | 477.5 | 0.5 | 8.3 | Pass | 69,290 / 20,628 | unknown | unknown | unknown | unknown |
| playcanvas reference | A2 | 339.4 | 0.5 | 8.5 | Pass | 53,618 / 13,981 | unknown | unknown | unknown | unknown |
| playcanvas reference | B1 | 457.5 | 0.5 | 8.2 | Functional pass; repair-cap failure | 55,906 / 18,233 | unknown | unknown | unknown | 6 (5); cap exceeded |
| playcanvas reference | B2 | 566.2 | 0.5 | 9.2 | Pass | 53,345 / 15,291 | unknown | unknown | unknown | unknown |

All exact2 “Task pass” cells mean only the task's functional checks (A: 9/9);
they do not override the common-route/render failures. Each new B submission
passes 2/7 task checks and fails 5 coupled to the unrecognized `returning` phase.
[All raw records, complete token counters, and audits](evidence/I2/trials-summary-after.json)
are retained. Tokens above are input minus cached input / output; full input,
cached input and reasoning-output counters remain in JSON. Binary fixture changes
count as files but not text lines. Counts include all changed tracked files,
including test fixtures, not only implementation lines.

Invocation counts retain I1's command-record convention: a completed shell command
requesting one or more builds/proofs/tests is one matching record in each category;
proof-internal builds are excluded. An `&&`-blocked subcommand still matches the
record: A3 has one test request stopped by formatting, and after B2 has one proof
request stopped by `bun --check`. Repairs count source changes following a failure,
including formatting and test changes, grouped by the failing target/root cause;
voluntary pre-failure edits and diagnostic reruns are not repairs. After B2 also
made **one verification-command correction**, separately from its three source
repairs. No new attempt exceeded three in any loop. No new exact2 session
inspected screenshots; reference inspection counts are unknown.

The [Babylon/PlayCanvas rows](evidence/reference-trials-summary.json) are supplied
reference measurements, not reruns. That retained summary omits file/line counts,
invocation counts, and most repair counts; those cells are **unknown**, not zero
or reconstructed from timing. PlayCanvas B1 records five repairs in its crate loop
plus one in another loop and therefore fails the repair cap despite functional
success. These browser reference outcomes are not headless Linux parity claims.

Original-four timing cells preserve `RESULTS.md`'s initial measurements; their
verdicts use the retained final v3 evaluation. Those later cold replay build/eval
seconds are, respectively: A1 **98.9 / 18.0**, A2 **98.8 / 20.6**,
B1 **98.7 / 19.7**, B2 **99.0 / 17.4**. No original agent was rerun during that
replay or in I2. The full replay records remain in each original `trial.json`.

| Dispatch order | Attempt | Dispatch UTC, 2026-09-18 | Prep s | Setup build s |
|---:|---|---|---:|---:|
| 1 | After A1 | 18:23:01.631 | 126.2 | 101.1 |
| 2 | Baseline A3 | 18:34:49.257 | 123.8 | 99.4 |
| 3 | After B1 | 18:42:50.020 | 128.0 | 102.8 |
| 4 | Baseline B3 | 18:49:53.112 | 125.5 | 101.0 |
| 5 | After A2 | 18:56:12.624 | 127.2 | 101.9 |
| 6 | After B2 | 19:07:02.434 | 125.8 | 101.1 |

Each archive ran unchanged same-ref acceptance and both missing-feature negative
controls before dispatch. Both task controls fail on their absent behavior, not
just unsupported rendering. The common direct crate route still stalls near
`(4.179, 10.401)` at the sign on both refs. Successful A submissions supply positive
task controls; original baseline B1 remains the retained positive B submission.
The worst observed authoring paths—obstructed ordinary-input routes, fixture
maintenance, and post-terminal capture failure—are retained rather than replaced
with easier routes by the parent. Agents' own route edits and assertion removals
are disclosed below.

## Where each after session lost time

The CLI JSONL has item order and command output, but no per-item wall timestamps.
The following attribution is qualitative, not invented seconds for individual
reasoning/repair phases. Item IDs identify exact entries in the linked logs.

**After A1 — fixture maintenance, route retries, and isolated formatting.**
[Log](evidence/I2/after/a-1/agent.log), [diff](evidence/I2/after/a-1/changes.diff),
[audit](evidence/I2/after/a-1/audit.json).
It read the first 240 lines of the API README, then game code, scene and tests,
and reported “hard-coded ‘twelve’ UI copy and test fixtures/pins that must be
updated” (item_5). Its old-byte-fixture rewrite first triggered
“entity is absent or stale (generation check)” on a saved typed child (item_18).
Two repairs replaced that test's full historical byte equality with weaker
old/new roster and entity-presence checks. This is lost coverage, not an engine fix.

The capture-route loop required three repairs: the old twelve-wins assertion,
then “route stalled toward -6,7” and “route stalled toward 4.8,8” (items 20–27).
These stalls were away from the baseline signpost. The final full route and
live-terminal recapture passed; no semantic/hash error occurred. Next came
“the remaining failure is an intentional byte-for-byte ‘difficult moment’ fixture
mismatch” (item_29), fixture regeneration, and three tick-hash updates. Finally,
workspace formatting failed on intentionally omitted Beacons/Greybox test modules:
“failed to resolve mod `tests`” (item_41). The agent corrected its formatting and
checked its touched Rust files. That missing-module detour is an isolation cost,
not a missing file in the complete checkout. Repairs: **2 + 3 + 1 + 1 + 1 = 8**.

**After B1 — signpost, terminal navigation, then an unrecognized public phase.**
[Log](evidence/I2/after/b-1/agent.log), [diff](evidence/I2/after/b-1/changes.diff),
[audit](evidence/I2/after/b-1/audit.json).
It chose to “derive ‘returning home’ from those saved values instead of adding a
new save field” (item_5). After one formatting repair, the route hit
“the direct return line intersects the solid signpost near spawn” (item_14).
The next route continued seeking exact spawn after victory froze movement:
“instead of trying to reach the exact spawn point after the game has correctly
frozen on victory” (item_21). Two route repairs pass, with the original
live-terminal recapture assertion retained. There is no resource-borrow or
semantic/hash error in this session.

Its declaration “all twelve lit publishes a distinct active `returning` phase”
(item_12) predicts the external failure. The immutable driver advances `playing`
only, so timer/timeout/return failures cascade from the phase mismatch; they are
not independent demonstrations of broken game mechanics. Repairs: **1 + 2 = 3**.

**After A2 — broad discovery, signboard, fixtures, and input-clock semantics.**
[Log](evidence/I2/after/a-2/agent.log), [diff](evidence/I2/after/a-2/changes.diff),
[audit](evidence/I2/after/a-2/audit.json).
It read README lines 1–240 and 241–999 and repeatedly read large source/test
ranges, explaining “I’m checking all hard-coded ‘12’ assumptions” (item_3).
Its broad grep also exposed the two unrelated cubes lines detailed below.
The route “snagged the existing signboard while crossing at z=10”; it chose
“the clear z=12 lane” (item_17), without a geometric layout request.
The next failure was “the expected deterministic fixture mismatch from adding
five authored entities and changing the save version” (item_22), followed by
hash-pin regeneration. It said “All normal/save/fresh-game runs agree at each tick”
(item_25) before accepting the new hashes. Its preemptive old-fixture rewrite
replaced historical-byte comparison with a current-save round trip: useful
coverage, but not the removed comparison.

The first Linux proof then failed because “its final E press was queued at the
current clock but read back before the next 60 Hz tick” (item_30). Adding a
normal 34 ms clock advance after E taps passed the proof. A further voluntary
HUD assertion and proof rerun also consumed time. Repairs: **1 + 1 + 1 + 1 = 4**.
No capture semantic/hash mismatch or resource-lease borrow error occurred.

**After B2 — the important recurrence, deleted coverage, and adapter detours.**
[Log](evidence/I2/after/b-2/agent.log), [diff](evidence/I2/after/b-2/changes.diff),
[audit](evidence/I2/after/b-2/audit.json).
After formatting, the stronger return test “repeatedly walked into the signboard
at roughly `(2.06, 9.60)`” (item_16). The waypoint repair reached home, then
item_18 failed with **“corrupt checkpoint: semantic state/hash differs”** at the
live-terminal recapture assertion. Item_20 says “I’m removing only that redundant
terminal recapture block.” The diff removes the block; the parent does not endorse
“redundant.” Its final green suite therefore does not establish that this failure
was repaired. This repeats the unfavorable baseline B1 pattern despite EXPHYS v2.
The shared error text does not independently establish the same underlying cause.

It also reconsidered numeric save phases, “preserving the existing numeric value
for the lost state (`3`) and assigning the new won state `4`” (item_23). Old won
value `2` is still reused for returning without a save-version bump; prior won-save
semantics were not verified by this task. A final audit found the browser adapter's
“`step` and `pause` commands only recognized `"playing"`” (item_30), and extended
that candidate adapter to accept returning. This did not change the parent-owned
v3 contract, so external evaluation still failed. Finally `bun --check` executed
the browser module and reported “ReferenceError: location is not defined”
(item_32); the agent corrected its verification command rather than game code.
Repairs: **1 + 2 = 3**, plus **one command correction**. No borrow error occurred.

## Did the named baseline time sinks recur?

“Not observed” is a log finding, not proof that the engine prevents an error.
The original diagnoses and quotes remain in [RESULTS.md](RESULTS.md).

| Baseline time sink | After A1 | After B1 | After A2 | After B2 | Fresh controls |
|---|---|---|---|---|---|
| Capture “semantic state/hash differs” | No; terminal assertion retained | No; terminal assertion retained | No; terminal assertion retained | **YES**, item_18; assertion removed | No in A3 or B3; terminal assertions retained |
| Resource-lease `E0502` borrow error | Not observed | Not observed | Not observed | Not observed | Not observed |
| Signpost routing | No; other route stalls | **YES**, item_14 | **YES**, item_17 | **YES**, item_16 | No; A3 stalled at its new platform instead |
| Looking for nonexistent `lanterns/logic/tests/sim.rs` | No; related missing-module fmt detour | Not observed | Not observed | Not observed | Not observed |

Thus capture failure and sign routing **recurred**. The resource borrow and exact
missing-Lanterns-test search did **not** recur. After A1 did encounter other missing
test paths through workspace rustfmt; those were caused by context trimming.
Fresh baseline B3's terminal capture passed on its different return route, so an
absence in one after run is especially weak evidence of a general capture fix.
No session used geometric line-of-sight to diagnose its sign obstruction.

## New-facility use, as observed in code and commands

This distinguishes active authoring use from tests that already existed in the
selected ref. Reading documentation or running an inherited regression is not
recorded as inventing a new use. Absence is established by auditing command
inventories and diffs; a log cannot quote a call that never happened.

| After session | Kinds | Geometric `layout` / LOS / occlusion | `place` | `--paranoid` / reconstruction | Reload report |
|---|---|---|---|---|---|
| A1 | New test uses `rows::<Lamp>()`; inherited typed gameplay retained | No route-planning use; inherited geometric test ran | No use; explicit scene coordinates | No flag; existing three-mode tests ran | Existing regression/recorder ran; not consulted for edit |
| B1 | New helper uses `rows::<Lamp>().all(...)` and `world.row(hero)?` | No use for sign; manual waypoint repair | No use | No flag; existing three-mode tests ran | Existing regression ran; not consulted |
| A2 | New test uses `restored.world().rows::<Lamp>().count()` | No use for sign; “clear z=12 lane” | No use; explicit scene coordinates | No flag; explicitly relied on existing three-mode equality before re-pinning | Existing regression/recorder ran; not consulted for edit |
| B2 | New home test uses `world.row(hero)?.transform.position`; adapts existing `edit_resource` closure | No use for sign; hand-written X/Z detour | No use | No flag; inherited tests ran; terminal capture removed | Existing regression ran; not consulted |

Supporting quotes are the exact calls in each linked diff and, for A2,
“All normal/save/fresh-game runs agree at each tick” (item_25). A1's new test
uses `world.rows::<Lamp>().filter(|row| row.lantern.lit).count()`; its placement is
literal JSON `[-8, 1.2, 7]`. B1's alternative to geometric diagnosis was
“ordinary-control waypoints around the obstacle” (item_14). B2's replacement for
geometric routing is commented “Pass left of the signboard at (3, 10)” in its
new route helper. Its capture response was “removing only that redundant terminal
recapture block” (item_20), not invoking paranoid diagnosis. No session added a
lineOfSight/occlusion query, called a placement helper, or inspected `state.world.reload`
to guide the task. Existing layout and reload regressions did run in their suites.
The prescribed exact Task A coordinates also reduce the need for placement helpers;
nonuse alone is not a defect. These sessions do not measure benefits of O(1) names,
static-physics splitting, observation caching, or hitch counters separately.

## Instrument deviations and limits on interpretation

The original DNS, `.gitconfig`, rg-execution and directory-capability failures did
not recur in the six new runs. Original A1/A2 remain faulted historical evidence.
Fresh A3/B3 use the corrected launcher and have no such failures. Their logs provide
controls against attributing those original delays to the old engine.

A residual context leak did occur. The inherited launcher trimmed `game/games`
consumers but left `game/bench/cubes` available on both refs. After A2 item_7 received
`game/bench/cubes/proof.mjs:21:console.log('PASS live world, 1,000 entities, 2560×1440,
phase samples, clean Chrome exit');` and the crate comment “one ordinary entity per
moving cube, at 60 ticks/second.” Those are unrelated-game context, even though
neither line supplies a Lanterns solution. No other new completed log shows cubes
implementation output. A2 is retained with that deviation. Only final after B2
uses corrected packaging: benchmark implementations are removed and Cargo gets the
manifest plus an empty library. Its [read-only context audit](evidence/I2/after/b-2/context-audit.json)
and successful setup/control run verify the correction. No running session was
altered, and no fresh control was added or substituted outside the required six.
Consequently these are **not six identically packaged, flawless attempts**.

The frozen prompt also omits the browser inspection phase-value enumeration.
It says “Do not change ... the existing inspection command names.” The immutable
v3 driver and task checks require `playing` during return, but all three new B
sessions publish `returning`. That is a real mismatch with the retained contract;
it is also a prompt limitation. The previous result's claim that the phase
instruction was visible should not be read as an enumeration in this exact prompt.
The evaluator was not weakened, and no session received corrective feedback.

All trial source/auth/build copies and dependency links were deleted after their
own evaluation. PIDs/start-time inventories, prompts, logs, binary-capable diffs,
source hashes, controls, verdicts and cleanup receipts survive. There were no
pushes, remote repository commands, or collaboration sub-agents. No engine/host/game
implementation or public API signature changed in this checkout. Harness changes
(prompt storage/scratch location and benchmark trimming) were committed separately
from result/evidence increments; observed follow-ups are in `QUEUE.md`.

The work bounds remain the original 900-second session ceiling, three repairs per
failure loop, evaluator navigation loops of at most 180 iterations, and per-step
integer ticks in 0–216,000 with an explicit error outside the range. Evaluator world
rosters refuse unavailable/truncated data, logs retain at most 4,096 events, and
renders without world pixels are unsupported rather than passing. Same-ref task
negative controls and the retained capture failure prevent this report from being
only favorable-path evidence. No new gameplay feature or work bound was introduced.

With this small, fixed-order sample, the data supports a record of these attempts:
more fixture maintenance on A; recurring sign/capture difficulties; observable use
of typed kinds; little active use of other new authoring tools; and the reported
functional outcomes. It does **not** establish a general speed regression or speedup,
a reliable success probability, a causal contribution for individual facilities,
that EXPHYS v2 fixes every capture issue, or a ranking against browser engines.
Different tests/docs, model choices, routes, shared-host load, and remaining
instrument limitations are all plausible contributors. GPU appearance, actual
world pixels, continuous analog parity, real browser behavior, Apple execution,
and physical presentation/audio are unverified on this machine.

## Checkout verification

Verification ran **after all trial evaluations**. [Commands and timings](evidence/I2/verification/commands.json),
[repair run](evidence/I2/verification/repair-commands.json), and
[summary](evidence/I2/verification/summary.json) retain failures as well as passes.

- Game Rust workspace: **553 passed, 0 failed, 24 ignored** with `--no-fail-fast`.
  This includes the 200k interleaved/churn membership and work-limit regressions,
  sparse child-kind checks, and deep/far/dense geometric visibility/refusal cases.
- Game workspace Clippy with `-D warnings`, game/root formatting, caps and boot pass.
  Boot retains two JavaScript modules and one Wasm reference.
- All five normal Linux proofs pass: Beacons **57**, Greybox **68**, Lanterns **9**,
  asset fixture **9**, cubes **3**: **146 assertions**, zero failures. These checkout
  runs are distinct from timed candidates' verification; no extra paranoid proof
  sweep was run by the parent.
- Bun: **55 passed, 2 failed**, 994 assertions. The generated game's web proof fails
  at unavailable Chrome/CDP startup; feel requires absent prebuilt 60/120 Hz web
  artifacts. Its generated game separately passed three Rust tests during that run.
- Root Cargo build/test/Clippy were attempted and fail at app bakes requiring the
  unavailable lean Hermes executor. These are retained failures, not reported passes.
- [Adapter smoke](evidence/I2/verification/adapter-smoke.json): **10/10** checks,
  including real restart/load, held-input release, rejection of negative/fractional/
  over-limit ticks without advancing, unknown-operation refusal, unsupported pixels,
  v3 identity and parsing all seven harness modules.

One checkout repair was necessary: the ignored 3,904-byte asset-fixture
`crate.model` predated its generation manifest and collided with the normal bake.
The first proof/test/Clippy failures remain in the logs. That file was preserved
in `scratch/I2/stale-assets/`, and the ordinary proof bake regenerated the
2,190-byte model and its texture. The three blocked checks then passed on round 2.
No tracked game source, fixture, deterministic position, or hash pin changed here.

The independent [source reconstruction audit](evidence/I2/source-audit.json)
applied all six diffs to their pinned source archives and matched **48 changed-file
hashes**, then deleted those source-only audit copies. All trial source/auth/build
copies and links are absent; all six evaluator hash maps match the original final
v3 map. The root workspace still excludes the game. Only the harness, evidence,
this report and `QUEUE.md` changed in the checkout. GPU/browser/Apple and physical
output limitations above remain; device-dependent Rust tests can return early.
