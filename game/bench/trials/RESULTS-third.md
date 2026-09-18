# Lanterns change trials — third interleaved round

**This round does not establish an authoring-speed improvement.** After2 A1/A2
pass Task A; after2 B1 passes Task B and B2 fails its phase/text contract. Fresh
baseline A4 passes A and B4 fails B. All exact2 rows still fail strict full
acceptance because the unchanged direct crate route stalls and world pixels are
unavailable on this host.

E1's maintenance change is visible: both after2 A sessions used `--repin`, changing
one pin file at +3/−3 lines each, with no edits to frozen historical evidence.
No after2 session encountered the capture hash failure or borrow error, and all
retained their live-terminal capture assertions. Baseline B4 did encounter the
hash failure and deleted its failing recapture block. This is a useful observed
recurrence contrast, not proof from four trials that every capture is fixed.

After2 Task A mean is **506.1 s**, versus **308.0 s** for fresh baseline A4
(**+64.3%**) and **512.4 s** for prior after at 7c65726 (**−1.2%**).
After2 Task B mean is **269.1 s**, versus **261.6 s** for fresh baseline B4
(**+2.9%**) and **330.0 s** for prior after (**−18.4%**). Each after mean has only
two observations; each new baseline comparator has one. Functional after2 task
passes are **3/4**, with **0/4 strict full passes**. These are descriptive numbers,
not causal estimates or reliable success rates.

After2 is pinned to `a30e5e36b3be8d518471b76b1b439af5e227f98a`, this checkout's
initial HEAD, containing the engine re-merge, EXCAP v2 and E1. The local
`next/trunk` ref instead points to `7de897543937341668ef122b0d4fe98d02be3ef7`,
which predates those requested changes. With no clarification available, the
specified feature boundary wins; neither ref was moved. This is an explicit
ref-selection deviation from a literal `next/trunk` lookup. Baseline remains
`e47a09fc36c07103cdc3645b7061cca305af6c14`; prior after remains
`7c65726ca8be72a3f10de3a44a836e1b3b00eed5`.

[Protocol and frozen hashes](evidence/I4/protocol.json). Evaluator v3 and prompts
are unchanged. Fresh CLI sessions use gpt-5.6-sol, high reasoning, a 900-second
ceiling and the frozen three-repair rule. Dispatch order: after2 A1, baseline A4,
after2 B1, baseline B4, after2 A2, after2 B2. No checkout builds run concurrently
with trials. Each trial's sources/auth/build outputs are deleted after evaluation.

Times are wall seconds. Tokens are uncached input / output. Historical rows and
unknown cells are preserved from [the prior report](RESULTS-after.md), including
original A1's infrastructure abort and the browser references' limited metadata.
Build/evaluation exclude preparation and model time; invocation and repair counts
retain the prior report's definitions. Functional task success is distinct from
strict full acceptance, which also requires common-route and world-pixel checks.

| Cohort | Attempt | Agent s | Build s | Eval s | Outcome | Uncached input / output tokens | Files | + / − text lines | Build / proof / test calls | Repairs (largest loop) |
|---|---|---:|---:|---:|---|---:|---:|---:|---:|---|
| Baseline original | [A1](evidence/next-baseline/a-1/trial.json) | 620.1 | 17.7 | 18.0 | Infrastructure abort; no edits | unknown | 0 | 0 / 0 | 0 / 0 / 0 | No model work |
| Baseline original | [A2](evidence/next-baseline/a-2/trial.json) | 516.2 | 17.6 | 20.5 | Task pass; instrument/context deviations | 83,805 / 15,178 | 7 | 158 / 12 | 1 / 1 / 3 | 1 (1) |
| Baseline original | [B1](evidence/next-baseline/b-1/trial.json) | 268.1 | 18.5 | 19.8 | Task pass | 54,432 / 9,254 | 5 | 51 / 16 | 0 / 1 / 4 | 3 (3) |
| Baseline original | [B2](evidence/next-baseline/b-2/trial.json) | 494.9 | 17.6 | 17.9 | Fail: phase contract | 122,573 / 17,719 | 5 | 83 / 19 | 0 / 2 / 9 | 3 (2) |
| Baseline fresh | [A3](evidence/I2/baseline/a-3/trial.json) | 313.9 | 17.5 | 20.6 | Task pass | 67,853 / 11,414 | 7 | 160 / 12 | 1 / 1 / 5 | 3 (2) |
| Baseline fresh | [B3](evidence/I2/baseline/b-3/trial.json) | 216.4 | 17.6 | 17.7 | Fail: phase contract | 52,451 / 8,549 | 5 | 60 / 18 | 0 / 2 / 1 | 1 (1) |
| Baseline fresh | [A4](evidence/I4/baseline/a-4/trial.json) | 308.0 | 17.4 | 20.5 | Task pass | 91,475 / 11,936 | 7 | 152 / 29 | 1 / 1 / 3 | 1 (1) |
| Baseline fresh | [B4](evidence/I4/baseline/b-4/trial.json) | 261.6 | 17.4 | 18.0 | Fail: phase contract | 48,705 / 9,666 | 5 | 90 / 28 | 0 / 1 / 5 | 3 (2) |
| After | [A1](evidence/I2/after/a-1/trial.json) | 541.6 | 20.0 | 21.3 | Task pass | 178,032 / 18,170 | 13 | 906 / 671 | 1 / 2 / 12 | 8 (3) |
| After | [B1](evidence/I2/after/b-1/trial.json) | 259.9 | 18.3 | 18.7 | Fail: phase contract | 63,217 / 9,696 | 5 | 65 / 15 | 0 / 1 / 4 | 3 (2) |
| After | [A2](evidence/I2/after/a-2/trial.json) | 483.3 | 18.4 | 21.5 | Task pass; context deviation | 154,867 / 18,857 | 13 | 815 / 696 | 0 / 3 / 6 | 4 (1) |
| After | [B2](evidence/I2/after/b-2/trial.json) | 400.0 | 18.2 | 18.7 | Fail: phase contract | 80,958 / 14,329 | 5 | 103 / 27 | 0 / 3 / 5 | 3 (2) |
| After2 | [A1](evidence/I4/after2/a-1/trial.json) | 506.0 | 20.2 | 21.1 | Task pass | 310,295 / 12,631 | 8 | 130 / 13 | 0 / 2 / 2 | 1 (1) |
| After2 | [B1](evidence/I4/after2/b-1/trial.json) | 258.3 | 20.2 | 20.3 | Task pass | 69,322 / 9,370 | 5 | 66 / 8 | 0 / 2 / 4 | 3 (1) |
| After2 | [A2](evidence/I4/after2/a-2/trial.json) | 506.1 | 20.2 | 21.2 | Task pass | 80,136 / 10,829 | 7 | 130 / 13 | 0 / 2 / 4 | 2 (1) |
| After2 | [B2](evidence/I4/after2/b-2/trial.json) | 280.0 | 20.1 | 18.3 | Fail: phase contract + instruction regex | 81,027 / 10,335 | 5 | 72 / 13 | 0 / 1 / 3 | 2 (2) |
| babylon reference | A1 | 407.2 | 2.0 | 20.9 | Pass | 63,450 / 13,307 | unknown | unknown | unknown | unknown |
| babylon reference | A2 | 468.7 | 2.1 | 20.4 | Pass | 51,682 / 12,038 | unknown | unknown | unknown | unknown |
| babylon reference | B1 | 220.4 | 2.2 | 20.2 | Pass | 49,007 / 8,330 | unknown | unknown | unknown | unknown |
| babylon reference | B2 | 347.3 | 2.1 | 19.9 | Pass | 53,998 / 13,441 | unknown | unknown | unknown | unknown |
| playcanvas reference | A1 | 477.5 | 0.5 | 8.3 | Pass | 69,290 / 20,628 | unknown | unknown | unknown | unknown |
| playcanvas reference | A2 | 339.4 | 0.5 | 8.5 | Pass | 53,618 / 13,981 | unknown | unknown | unknown | unknown |
| playcanvas reference | B1 | 457.5 | 0.5 | 8.2 | Functional pass; repair-cap failure | 55,906 / 18,233 | unknown | unknown | unknown | 6 (5); cap exceeded |
| playcanvas reference | B2 | 566.2 | 0.5 | 9.2 | Pass | 53,345 / 15,291 | unknown | unknown | unknown | unknown |


## Where after2 sessions lost time

CLI JSONL records item order and output but has no per-item wall timestamps.
Attribution below is qualitative; no seconds are invented for individual phases.

**After2 A1 — discovery, one repin, then repeated verification.**
[Log](evidence/I4/after2/a-1/agent.log),
[diff](evidence/I4/after2/a-1/changes.diff),
[audit](evidence/I4/after2/a-1/audit.json).
It read README lines 1–240 (item_2), then the game, route implementation and
physics sources, and inspected generated scene artifacts after its final diff.
The full route passed first time. The timing test failed on changed scene hashes:
“The only failure is the expected deterministic pin mismatch caused by adding
scene entities; all three execution modes produced identical new hashes”
(item_18). Item_19 invoked `bun game/proof.mjs lanterns --repin` successfully,
including the three-mode tests/proofs. Only `pins.json` changed for pin maintenance:
**one file, +3/−3 lines**. Historical engine evidence stayed untouched.
It then said “I’m now running the two exact verification commands requested
against the final files” (item_23), repeating package tests and the ordinary Linux
proof after repin's checks. **One repair**. The original live-terminal recapture
and the inherited fractional-clock regression both remain; neither failed.
The sign-board diagnostic appeared only in the inherited successful negative-control
proof, not as an unexpected route failure demanding a repair.

**After2 B1 — formatting, terminal tolerance, and a Contract type name.**
[Log](evidence/I4/after2/b-1/agent.log),
[diff](evidence/I4/after2/b-1/changes.diff),
[audit](evidence/I4/after2/b-1/audit.json).
It read README lines 1–240 (item_1). Its first combined test command stopped at
formatting. After that repair, `Sim::move_to` failed toward exact spawn with
`{"blocker":null,"nearestClearSide":null}` (item_14). The agent correctly identified
that “simulation froze on victory around 1.98 units, so the helper kept waiting
for 1.95” (item_15) and changed tolerance to 2.0. This was a terminal-state stall,
not a geometric obstruction that could truthfully be named. It then passed the
capture test and full package suite. Linux proof compilation reported
“unknown type `boolean`”; item_21 calls it “one schema spelling issue,” and the
agent changed the Contract field to `bool`, then passed the proof. **Three repairs
in three distinct loops**, none above the cap. A broad spelling search returned
unrelated root applications' `bool` field declarations; no other game's
implementation appeared. No fixture edits or `--repin` were needed. Its derived
`returnHome` publication leaves the public phase `playing`, and all seven v3 Task B
checks pass. Both live-terminal capture assertions remain and pass.

**After2 A2 — focused tests, formatting, one repin, then repeated verification.**
[Log](evidence/I4/after2/a-2/agent.log),
[diff](evidence/I4/after2/a-2/changes.diff),
[audit](evidence/I4/after2/a-2/audit.json).
It read README lines 1–260 (item_2). Its focused platform test and full-route test
both passed before the full suite. A formatting failure led to one repair.
Then the timing test failed: “all three simulation modes agree byte-for-byte,
but the committed scene hashes still describe the 12-lantern world” (item_19).
The documented `--repin` (item_20) succeeded, touching **one pin file, +3/−3 lines**,
without changing the frozen engine evidence. It repeated the requested package
tests and Linux proof: “I’m now rerunning the exact requested Rust command against
the updated pins, followed by the exact requested Linux proof command” (item_23).
**Two repairs in separate loops**. No unexpected route stall, capture hash failure,
or borrow error occurred. The named sign-board stall is the inherited diagnostic
proof; both live-terminal capture checks remain. This submission leaves the host
proof unchanged and adds its new ordinary-input feature assertions in Rust.

**After2 B2 — deeper reading, two terminal-route repairs, and the phase mismatch.**
[Log](evidence/I4/after2/b-2/agent.log),
[diff](evidence/I4/after2/b-2/changes.diff),
[audit](evidence/I4/after2/b-2/audit.json).
It read README lines 1–240, 241–520, 508–1150, and 2280–2345 (the file ends at
2337), plus headings: the deepest after2 read. It chose saved numeric phase 4 and
public `returning`. Its first route failed with a null blocker: “once the fox
crosses the 2-unit boundary, gameplay correctly freezes in `won`, while `move_to`
was still trying to reach the exact spawn center” (item_15). The boundary-waypoint
repair then stopped outside victory: “within its navigation tolerance just outside
the win radius” (item_19). Four ticks of ordinary S input resolved it. **Two repairs
in one loop**; the package tests and Linux proof passed. No pins changed or
`--repin` ran, and both live-terminal captures remain and pass.

The external result is **1/7**, not a task pass. Five failures are coupled to public
`returning` while v3 expects and advances `playing`. The sixth is a distinct
instrument limitation: the visible instruction is “Return to where you began before
night falls!” but the frozen text check requires `return … home/spawn` or `go … home`.
The UI supplies a semantically explicit instruction; the exact regex rejects its
wording. The evaluator is not changed and the failed cell is retained. This is not
six independent demonstrations of broken game mechanics.

## Recurrence and maintenance

“Not observed” means absent from the completed session's failures, not impossible.
README line ranges are actual commands, not inferred from the model's claims.
All four after2 sessions read the task cards at lines 8–44. A command's requested
range can extend beyond EOF; B2's final range actually ends at 2337.

| Item | After2 A1 | After2 B1 | After2 A2 | After2 B2 | New baseline A4 / B4 |
|---|---|---|---|---|---|
| Fixture/pin maintenance | `--repin` yes; 1 file, +3/−3 | No repin; 0 files/lines | `--repin` yes; 1 file, +3/−3 | No repin; 0 files/lines | Neither repins; 0 pin/fixture files/lines |
| Frozen engine evidence | Untouched; checks pass | Untouched; checks pass | Untouched; checks pass | Untouched; checks pass | No E1 frozen-evidence boundary in baseline |
| Unexpected route stalls | None | Exact-spawn request after victory; blocker null | None | Exact-spawn request after victory; blocker null; first repair stops outside radius | A4 new platform; B4 signpost; old messages name no blocker |
| Did it act on the stall? | N/A | Yes: tolerance 1.95 → 2.0 | N/A | Yes: boundary waypoint, then 4 ticks S | Both yes: clear waypoints inferred from geometry |
| Named-blocker diagnostic | Inherited proof names `sign-board`; no unexpected obstruction to diagnose | Same inherited proof; terminal stall truthfully has no blocker | Same inherited proof | Same inherited proof | Not available in baseline helper |
| Capture semantic/hash failure | Not observed; live assertions retained | Not observed; live assertions retained | Not observed; live assertions retained | Not observed; live assertions retained | A4 absent; **B4 recurs at item_14 and assertion deleted at item_16** |
| Resource-lease `E0502` borrow error | Not observed | Not observed | Not observed | Not observed | Not observed in either |
| API README read depth | 1–240, item_2 | 1–240, item_1 | 1–260, item_2 | 1–240, 241–520, 508–1150, 2280–2345; items 1/2/7/8; heading search item_6 | Both 1–240, item_1 |
| Task B phase contract | N/A | **Pass**: `playing` + `returnHome`; 7/7 | N/A | **Fail**: `returning`; 1/7, including separate text-regex rejection | A4 N/A; B4 `returning`, 2/7 |

Fixture counts include generated fixture files and pin-bearing timing files;
they exclude behavior tests and UI edits. For prior after A1/A2, four fixture
files plus `logic/tests/timing.rs` changed in each: **5 files, +645/−635** text lines
for A1 and **5 files, +645/−635** for A2; two files in each set are binary and add
no text lines. A1's timing file includes one test-function rename, so this is a
file-level maintenance footprint, not a claim that every counted line is a hash.
Both prior sessions also weakened historical-byte checks, as documented in
[RESULTS-after.md](RESULTS-after.md). After2's one-file maintenance and preserved
historical coverage are the clearest measured E1 difference; total A agent time
did not materially fall relative to those two prior after observations.

The four after2 sessions execute the inherited `Sim::move_to`/`world.moveTo`
regressions and preserve their work limits. The observed unexpected B stalls are
terminal-state navigation errors, not tests of whether a geometric blocker name
helps an agent choose a path. No after2 session had an unexpected sign obstruction
and then acted on its named diagnostic. Thus this round confirms the diagnostic
runs and names the sign in its regression, but cannot estimate its authoring benefit.

## Protocol, retained limitations, and sample size

All six [raw records and audits](evidence/I4/trials-summary-third.json) retain
full/cached/uncached input, output and reasoning counters, commands, binary-capable
diffs, controls, evaluations and PID inventories. Every prompt and evaluator hash
matches v3; no old submissions needed re-evaluation. All six sessions completed
below 900 seconds, without restarts, replacements, parent feedback or scope
violations. Every repair loop stayed at or below three. No new session inspected
screenshots. The timeout termination path was not exercised.

The model's own verification is inside agent time. Parent preparation/build/eval
are separate. Each archive started without a copied Cargo target; cold setup took
98.9–102.4 s, complete preparation 123.3–127.5 s. No checkout verification build ran
alongside a trial. Shared-machine load and external service queue delay are unknown.
Dispatches, in UTC on 2026-09-18: after2 A1 **20:15:17.171**, baseline A4
**20:26:42.126**, after2 B1 **20:34:58.904**, baseline B4 **20:42:17.642**,
after2 A2 **20:49:38.389**, after2 B2 **21:01:05.513**.

Invocation counts are completed shell-command records, counting a combined command
once in each matching category; an `&&`-blocked test request still counts. Proof
internal builds are excluded. `--repin` is one proof invocation, although it
internally runs multiple tests/proofs. Repairs count source changes following a
failure, including formatting, pin updates and assertion deletion; diagnostics and
voluntary pre-failure edits are not repairs. File totals include every changed
tracked file; binary fixtures contribute files but no text lines. Historical
baseline timing cells preserve the original report, while verdicts use final v3;
the older cold replay times remain in their records. Browser references are supplied
measurements, not reruns or Linux parity claims; their missing metrics stay unknown.
PlayCanvas B1 remains a repair-cap failure despite functional success.

The corrected I2 packaging is used for all six: other game/benchmark consumers are
metadata-only stubs, inherited instructions/history/memories are stripped, and
Landlock probes deny evaluator reads while allowing resolver and directory-capability
access. Shared source dependencies and root apps remain available, as before.
B1's `bool` search returned three unrelated root-app field declarations. No new log
shows another game's implementation; E1's frozen Lanterns source is intentionally
available to its historical tests. This is source-available isolation, not a claim
that only Lanterns bytes can be read. Prior original A1/A2 infrastructure/context
faults and I2 A2's cubes exposure remain in the retained table, not silently excluded.

The frozen B prompt preserves inspection command names but does not enumerate
phase values. `returning` therefore remains a real mismatch with v3 and also a
prompt limitation. Candidate browser-adapter edits do not alter the parent driver.
B2 additionally exposes the text matcher limitation described above. Neither
limitation was repaired mid-round or used to relabel a failed check as passing.

The unfavorable cases are retained: unchanged-ref missing-feature controls fail on
absent A/B behavior, the common direct crate route still stalls at approximately
(4.179, 10.401), baseline B4's live-terminal failure is recorded, and after2 B2's
contract failure remains. Work bounds are the existing 900-second session ceiling,
three repairs per loop, evaluator navigation at most 180 iterations, integer step
counts 0–216,000 with explicit refusal outside that interval, complete-or-refused
world rosters, and a 4,096-event journal. E1's inherited movement probes run at most
160 bursts / 1,760 ticks and refuse with diagnostic output. No new gameplay feature
or tick work was introduced by this lane.

**What this n supports:** after2 has **n=4, two per task**; this round has just
**one new baseline observation per task**. The full table has 16 exact2 attempts
(including an infrastructure abort) and eight supplied browser references across
different rounds. It supports a reproducible account of these attempts, observed
maintenance reduction, capture nonrecurrence with preserved assertions, and the
reported contract outcomes. It does **not** establish a general speedup/regression,
a stable success probability, universal capture correctness, the causal value of
individual changes, or a ranking against browser engines. Fixed order, changed
engine/tests/docs, model variation, shared-host load and instrument limits prevent
those inferences. We do not pool all historical rows into an exchangeable sample.

## Checkout verification and delivery

[Commands and timings](evidence/I4/verification/commands.json),
[repair reruns](evidence/I4/verification/repair-commands.json),
[summary](evidence/I4/verification/summary.json), and
[final evidence audit](evidence/I4/verification/final-audit.json) are retained.
These checks ran after all timed sessions and do not enter trial timing cells.

- Game Rust workspace: **658 passed, 2 failed, 26 ignored**. The failures are
  `skinned_normal_is_inverse_transpose_under_scaled_rotated_joints` and
  `first_presented_fox_matches_current_pose_in_fox_rectangle`; both explicitly
  report no suitable graphics adapter. No assertion was disabled. The suite
  includes the inherited 200k interleaved/churn, bounded layout, movement-stall,
  capture-clock and pending-asset regressions. Device-dependent tests that return
  early do not establish pixel correctness.
- Game workspace Clippy with `-D warnings` and formatting pass. Root formatting
  and boot pass; boot remains two reachable JS modules and one Wasm reference.
- All six ordinary Linux proofs pass: Beacons **57**, Greybox **68**, Lanterns
  **11**, asset fixture **19**, skinned fixture **12**, cubes **3**: **170 assertions**,
  zero failures. This parent sweep is ordinary mode; the timed A sessions' repin
  workflows separately ran their three-mode checks.
- Bun: **59 passed, 2 failed**, **1,014 assertions**. The generated game reaches
  web-proof CDP startup and fails with `null ... output.setEncoding` on this box
  without Chrome. Its three Rust tests pass. The feel test lacks prebuilt
  60/120 Hz artifacts. The generated game and its temporary shells are cleaned up.
- Full root Cargo build/test/Clippy were attempted and fail at TypeScript app
  bakes requiring the unavailable lean Hermes executor. They are not green.
- Frozen-lockfile Bun install and staged caps pass. The source reconstruction
  audit applies all six diffs to their pinned inputs and matches **37 file hashes**;
  its source-only copies are deleted. The 12 final evidence-audit checks pass.

One local repair was necessary: ignored asset-fixture model/texture outputs lacked
the bake manifest and caused the first game-test and Clippy runs to refuse an
“authored asset” collision. Both failed logs remain. The **2,190-byte model** and
**1,475-byte texture** were preserved under `scratch/I4/stale-assets/` with their
[hash receipt](evidence/I4/verification/stale-assets.json); the ordinary proof bake
regenerated them. The second game-test run reaches only the two GPU failures, and
Clippy passes. No tracked fixture, deterministic position, or pin changed in this
checkout. Free space stayed above the 25 GiB floor (136 GiB at final validation).

No engine/host/game implementation or public API signature changed. The only
harness edit relocates scratch from I2 to I4; it is committed separately from
results/evidence. The report, retained evidence and `QUEUE.md` follow-ups are the
remaining delivery. There were no collaboration sub-agents, pushes or remote Git
commands. GPU pixels, Chrome/browser execution, Apple SDK/runtime and physical
audio remain unverified. The full-SHA after2 choice and local `next/trunk` mismatch
are disclosed above; no branch ref was rewritten to conceal that choice.
