# Historical change trials: all three reported cohorts

These are historical observations, not measurements of the combined runtime.
Original immutable source: `360921d386f212965cbc28229f21c93d280a753a`. Raw receipts remain in that commit under
`game/bench/trials/evidence/`; links below are interpreted relative to their original
`game/bench/trials/` source directory at that ref. No receipts were repinned or
reclassified for this merge. The third report does not establish a speed improvement.


---

Original path: `game/bench/trials/RESULTS.md` at `360921d386f212965cbc28229f21c93d280a753a`.

# Lanterns change trials — next/baseline

**This is not a clean four-attempt engine ranking.** All four requested sessions
were dispatched sequentially; none was replaced. A1 failed before gameplay work
because the instrument denied DNS configuration. A2 passed Task A functionally
but lost time to instrument permissions and saw extra game examples. B1 passed
Task B functionally. B2 failed the shared inspection phase contract. No exact2
attempt is a strict full pass: the baseline has a crate-route failure and Linux
cannot supply world pixels.

The measured ref is `next/baseline` at `e47a09fc36c07103cdc3645b7061cca305af6c14`.
The harness is committed on this clone's current branch, `next/i1`, following the
later “commit on the branch you are on” instruction. No engine, host or Lanterns
source changes landed here. Candidate changes exist only as retained diffs.

Times below are wall seconds. Setup is excluded; agent time includes the fresh
CLI/session and its own verification. Parent build and evaluation are separate.
The reference rows are copied from the supplied [recorded summary](evidence/reference-trials-summary.json),
not rerun on this box. Godot and Three.js had no qualified attempts in that record.
PlayCanvas B1 passed functionally but exceeded its repair cap. Results do not
support a universal speed or success-rate claim, especially across these differing
render/input capabilities and the instrument faults disclosed below.

| Engine | Task/attempt | Agent s | Build s | Eval s | Outcome |
|---|---|---:|---:|---:|---|
| babylon | A1 | 407.2 | 2.0 | 20.9 | Pass |
| babylon | A2 | 468.7 | 2.1 | 20.4 | Pass |
| babylon | B1 | 220.4 | 2.2 | 20.2 | Pass |
| babylon | B2 | 347.3 | 2.1 | 19.9 | Pass |
| playcanvas | A1 | 477.5 | 0.5 | 8.3 | Pass |
| playcanvas | A2 | 339.4 | 0.5 | 8.5 | Pass |
| playcanvas | B1 | 457.5 | 0.5 | 8.2 | Functional pass; repair-cap failure |
| playcanvas | B2 | 566.2 | 0.5 | 9.2 | Pass |
| exact2 Linux | A1 | 620.1 | 17.7 | 18.0 | Infrastructure abort; no game edits |
| exact2 Linux | A2 | 516.2 | 17.6 | 20.5 | Task functional pass; instrument/context deviations |
| exact2 Linux | B1 | 268.1 | 18.5 | 19.8 | Task functional pass; baseline/render incomplete |
| exact2 Linux | B2 | 494.9 | 17.6 | 17.9 | Fails phase contract (`returning`) |

The initial external evaluator had identical content hashes across all four
attempts. A final global adapter correction was applied to the baseline and all
unchanged submissions: “all lanterns lit” is an observation rather than a win
event, and load must release restored held keys. A focused W-held save/load
probe now shows zero subsequent movement and no held keys. Version-1 reports
remain under each attempt’s `evaluation-v1/`; A1 also retains its intermediate
version-2 report. Current reports use version 3. No model was restarted and no
functional verdict changed. Table timings remain the original measurements;
replay build/evaluation durations are separate `reevaluation` fields. Each
prints one JSON report and returns nonzero for failures or unsupported rendering.
The [complete measurement records](evidence/trials-summary.json) include UTC
dispatch/completion/build/evaluation timestamps, source hashes, changed files,
usage, invocation counts, and cleanup receipts. External service queue delay is
unknown, not zero. No model inspected screenshots in the retained CLI logs.
No attempt reached the 900-second ceiling; A1 was explicitly aborted by the parent
at 620.1 seconds after confirming the sandbox's resolver denial. The remaining
sessions completed below the ceiling. The PID/start-time inventory and kill path
are implemented, but a real 900-second timeout was not exercised in this run.

| Attempt | Pre-dispatch s (warm build) | Uncached input / output tokens | Files | Added / deleted lines | Explicit build / proof / test calls | Repairs |
|---|---:|---:|---:|---:|---:|---|
| A1 | 131.5 (104.8) | unknown | 0 | +0 / −0 | 0 / 0 / 0 | No model work |
| A2 | 128.7 (102.1) | 83,805 / 15,178 | 7 | +158 / −12 | 1 / 1 / 3 | 1 |
| B1 | 128.2 (101.3) | 54,432 / 9,254 | 5 | +51 / −16 | 0 / 1 / 4 | 3 in one loop |
| B2 | 133.1 (106.8) | 122,573 / 17,719 | 5 | +83 / −19 | 0 / 2 / 9 | 2 + 1 in separate loops |

Invocation counts are completed shell command records: file reads mentioning
`proof.mjs` do not count. A2's build call was a scene-only bake. Builds nested
inside a proof are not double-counted. B2's nine test calls include diagnostic
reruns, not nine repairs. All full usage counters remain in the JSON records.

Where the sessions lost time, with quotes from their retained logs:

- **A1 — instrument networking, not game work.** The log repeatedly says
  “failed to lookup address information: Try again” and then “Reconnecting...
  waiting for network”. `/etc/resolv.conf` resolves outside `/etc`; the first
  policy denied its real target. There are zero completed shell commands, no
  source edits and no reported model usage. The parent stopped the recorded CLI
  PID; it did not reset the clock or replace the attempt. [A1 stdout](evidence/next-baseline/a-1/agent.log),
  [A1 stderr](evidence/next-baseline/a-1/agent.stderr).
- **A2 — file discovery and instrument permissions.** It first looked for the
  nonexistent `game/games/lanterns/logic/tests/sim.rs`. Git then reported
  “fatal: unknown error occurred while reading the configuration files” after
  `.gitconfig` reads were denied; the installed `rg` shim was also denied. It
  reread files with semicolon-separated commands and fell back to `grep`.
  It searched other games' proofs, which the first archive exposed; this is a
  context violation, recorded rather than hidden. Docs read included the first
  240 lines of `game/README.md`, the game README, scene/fragment data, Contract,
  capture tests, and proof/build scripts. [A2 session](evidence/next-baseline/a-2/agent.log).
- **A2 — test update, then asset-gate detour.** Its first full-route test failed
  with “left: 1” / “right: 2” because twelve correctly stopped winning. One repair
  corrected the route/test. Its Linux proof then stopped before game assertions:
  “the host bake failed in its asset gate with an `EACCES` permission error”. It
  read `scripts/filesystem.mjs`, `contract/cli/src/receipt.rs`, and
  `filesystem/src/{main,directory}.rs`, then reproduced the gate failure directly.
  The parent sandbox had denied opening `/` as a directory. Those delays are
  included in 516.2 seconds; they are not an engine-authoring measurement. The
  unchanged submission later passed all nine Task A functional checks.
- **B1 — route planning.** “the direct path home drove into the solid signpost
  near `(3, 10)`”. Repair one routed off the ledge to `z=12` before approaching
  spawn. It read the game README/API README, `light_nearest`, the UI, browser
  adapter, scene tests and existing capture route. [B1 session](evidence/next-baseline/b-1/agent.log).
- **B1 — an unsuccessful state hypothesis and removed coverage.** The next
  failure was “corrupt checkpoint: semantic state/hash differs”. It guessed that
  clearing `returning_home` on victory would fix post-terminal capture. That
  failed again. Its third repair said: “removes only the redundant second
  capture-from-an-already-frozen-world assertion”. The external seven task checks
  pass, but they do not validate the deleted live-terminal capture assertion.
  This is a material limitation of its claimed green self-test, not evidence
  that the original capture behavior was repaired. [B1 diff](evidence/next-baseline/b-1/changes.diff).
- **B2 — the same route obstruction and a longer capture investigation.** Its
  diagonal return route also wedged against the sign. After a waypoint repair,
  the post-win capture reported the same semantic/hash error. It read
  `game/engine/src/{capture,sim}.rs` and engine capture tests, reran with
  `RUST_BACKTRACE=1`, and compared saves: “restored checkpoint bytes differ”,
  lengths 99025 versus 99031. Its diagnosis was that reconstructed physics bytes
  were not canonical; that is the session's diagnosis, not an independent engine
  fix. Repair two moved the terminal capture to a restored checkpoint while
  preserving full-route replay coverage. [B2 session](evidence/next-baseline/b-2/agent.log).
- **B2 — extra compatibility work, then a borrow error.** It searched for
  `fn migrate` and the `Game` trait, then reconsidered phase numbering because
  it could “reinterpret older saved losses as wins”. It changed to a saved
  boolean and bumped the save version. That refactor produced Rust `E0502`;
  “narrowing that mutable resource borrow before logging/playing sound” fixed
  the separate compile loop. Both final self-checks passed.
- **B2 — missed contract requirement.** Despite preserving numeric game phases,
  it changed the adapter's public phase to `returning`, outside BRIEF's
  `title|playing|paused|won|lost`. Its instruction is visible, but the immutable
  evaluator expects `playing` during the return. The parent driver consequently
  does not advance this unrecognized phase, so the timer/timeout/return failures
  cascade from that mismatch; they do not independently prove the underlying
  game timer is broken. No repair was offered after the external result.
  [B2 task report](evidence/next-baseline/b-2/task-b.json), [B2 diff](evidence/next-baseline/b-2/changes.diff).

The [same-ref baseline](evidence/next-baseline/controls/acceptance.json) runs all
ten cases: five pass, four have passing mechanics but unsupported world renders,
and the direct crate approach fails at approximately `(4.179, 10.401)` near the
sign. The full twelve-lantern route still reaches victory. The adapter quantizes
analog requests to the nearest eight keyboard directions because Linux rejects
held pointer contacts; it does not silently claim continuous analog parity.
Player/crate velocity, grounded state, animation and gameplay journal events were
available through world inspection/logs; no queried state field needed a fabricated
zero. Save/load uses the existing world-save carrier and a sidecar for UI bindings,
then a full host restart. UI Save itself remained “Saving…” in the probe.

Both [Task A](evidence/next-baseline/controls/task-a.json) and
[Task B](evidence/next-baseline/controls/task-b.json) negative controls fail on their
actual missing behavior, not merely on rendering. A2 provides a positive Task A
control; B1 provides a positive Task B control. CPU renders contain Contract UI
and a flat canvas; they are always reported unsupported for world-pixel acceptance.
Browser keyboard delivery, GPU appearance, Apple hosts and actual world rendering
were not verified on this machine.

Preparation needed three target-relocation repairs (sibling `ibex`, sibling
`snapback-sb4`, copied Cargo absolute dep-info), three CLI-access repair rounds
(resolver, capability directory reads, Git/rg), and one context-packaging repair
(Cargo dev-dependency metadata after removing other game implementations). All
failed pre-dispatch preparations remain in scratch; none is counted as an agent
attempt. These corrections make this run an instrument bring-up, not a pristine
comparison. Read-only dependency links and all scratch source/auth/build copies
were deleted after evaluation. Logs, diffs and JSON reports remain; temporary
trial PNGs were removed. No remote repository commands, pushes or collaboration
sub-agents were used; the four explicitly requested fresh CLI sessions were the
only model trials.

Verification of the harness checkout is recorded in
[evidence/verification/verification.json](evidence/verification/verification.json).
The initial workspace test/clippy builds required Beacons' generated scene; the
normal Linux proofs produced it before the second workspace check round.

Final checkout checks: **406 Rust tests/doc-tests passed**, 11 ignored;
game-workspace clippy with `-D warnings`, game/root formatting and boot passed.
The additional root-workspace build, test and clippy commands were attempted;
all stop in unrelated TypeScript app bakes because the lean Hermes executor is
unavailable (`TypeScript bake requires the lean Hermes executor on this producer`).
Their logs and exit codes are retained in `evidence/verification/root-*`. All four Linux game
proofs passed (130 named assertions: Lanterns 8, Beacons 54,
Greybox 62, asset fixture 6). Bun: **38 pass, 2 fail** (826 assertions).
The generated-game web proof fails at browser/CDP startup on this Chrome-less
box; the feel test requires missing prebuilt 60/120 Hz web artifacts. These are
retained failures, not skips or reported passes. All ten baseline cases and both negative controls ran on the unmodified checkout
before the first harness commit; same-ref controls subsequently ran in every
archive before model dispatch. Source caps passed on staged changes. See the
[verification summary](evidence/verification/summary.json) and raw logs.

---

Original path: `game/bench/trials/RESULTS-after.md` at `360921d386f212965cbc28229f21c93d280a753a`.

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

---

Original path: `game/bench/trials/RESULTS-third.md` at `360921d386f212965cbc28229f21c93d280a753a`.

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

---

Original path: `game/bench/trials/README.md` at `360921d386f212965cbc28229f21c93d280a753a`.

# Lanterns headless change trials

Parent-owned instrument, version 3. `createCommand({root, store?})` returns
`{command(request), reload(), tree(), screenshot(path), close(), unavailable,
limitations}`. Requests are BRIEF's ready/start/reset/input/step/state/pause/save/load.
`step` uses the selected ref's `s.world('world').ticks(N)`, verifies the exact
world tick delta, and leaves title/paused/terminal states stationary. State comes
from `state world:*`, world resources/publications and bounded logs. Missing data
is null, listed in reports. Feet are the physics capsule center minus 0.65m.

Linux cannot hold pointer contacts. Input vectors are explicitly quantized to the
nearest of eight ordinary keyboard directions. This is a contract deviation;
continuous analog input cannot be implemented on this carrier without a host
change, forbidden by I1. World pixels are unavailable; CPU screenshots show UI
and flat canvas. Cases requiring screenshots are unsupported, never pass, even
when their mechanics pass. Asset readiness means the world declares the Fox
asset and animation, not that a GPU uploaded it. Save/load uses the existing
world save carrier plus a sidecar for the UI's live title/pause binding, then
restarts the entire host and releases held input. Linux's UI save button remained
at “Saving…” in the probe, so it is not used to claim checkpoint persistence.

Baseline case bodies come from comparison-full/harness/acceptance.mjs. The
transport changes only: native driver keys, process reload, CPU screenshot
classification. The movement helper and case assertions are retained. Task
routes reuse the comparison's ordinary input route. No teleport or test mutation.

Run from the repository root (Bun and Cargo on PATH, development update trust):

```
bun game/bench/trials/build.mjs
bun game/bench/trials/acceptance.mjs
bun game/bench/trials/task-a.mjs
bun game/bench/trials/task-b.mjs
bun game/bench/trials/run-trial.mjs --ref next/baseline a 1
```

Evaluators accept `root=/archive` and `out=/evidence`. Reports distinguish failures
from unsupported checks and exit nonzero for either. `functionalPassed` on task
reports excludes only unavailable rendering. This is not a full comparison pass.
The baseline currently has a direct crate-route failure at the sign; it is
retained, not repaired by changing the game or evaluator's route.

The original request named next/trunk; the later common rule says commit on the
current branch. This clone is next/i1, so increments land there. No engine, host
or game changes are authorized. Parent construction used three adapter rounds:
(1) rejected pointer contact, (2) keyboard mapping and discovery that UI save
stays pending, (3) existing world-save carrier. Baseline records and both negative
controls are committed before any model attempt.

`run-trial.mjs --ref REF <a|b> ATTEMPT` archives the resolved commit into
`~/lanes/gamenext/scratch/trials/REF/TASK-ATTEMPT/workspace`, copies the local warm
Cargo target (no shared writable target), and builds before dispatch. The existing
read-only `../ibex` dependency is linked beside the archive. Setup/build/control
times are outside agent time. Same-ref controls run before every dispatch. The
trial sees only its game brief/change request, README and source workspace as
installed dependencies; inherited instructions, research, earlier trial sources,
history and evaluator are absent. Linux Landlock denies reading the parent
instrument and other attempts, verified by a real read-denial probe. The model
may edit only its game; the parent reports changes outside that directory as
scope violations. The entire source/build/auth copy is removed after evaluation.

Wall timeout records the CLI PID and descendant PID/start-time identities before
signalling them. Agent JSONL, stderr, command inventory, prompt, source diff and
hashes, timestamps and reports survive. A shell command containing multiple
build/proof invocations is one matching command record; proof-internal builds
are excluded. Repair counts and screenshot inspections require retained-log
audit and remain null until audited. Rendering unavailability means a strict
`passed` can never be true on this machine; functional task results are separate.

Runner setup correction 1: git archive did not include the sibling ibex path
dependency. The failed pre-dispatch setup is retained separately; no model was
launched and no timed attempt was consumed. Link the authorized read-only
sibling in scratch and permit read-only dependency access in Landlock.

Runner setup correction 2: the root workspace's filesystem helper also resolves
`snapback-sb4` even for a game bake. Preserve that existing sibling path as another
read-only dependency link. Its sources are not modified; all output remains in
scratch. Both failed setup runs were before model dispatch, outside agent timing.

Runner setup correction 3 (final repair in this loop): copied Cargo dep-info
retained absolute generated-source paths into the original checkout, which the
build receipt correctly refused. Invalidate copied Cargo fingerprints and warm
at the new archive path before dispatch. This can recompile dependencies during
setup; no setup time is charged to the model. If this repair fails, do not add a
fourth repair or manufacture timed trial results.

A1 launch audit found `/etc/resolv.conf` resolves to
`/run/systemd/resolve/stub-resolv.conf`; the first Landlock policy denied reading
that symlink target. A1 is retained, never replaced or timed from a reset point.
Subsequent attempts permit the resolver's exact real path and probe that it is
readable alongside evaluator denial. This is an instrument defect, not evidence
about Lanterns authoring difficulty. Agent parameters and evaluator remain fixed.

A2's live log revealed that the archive still exposed other games in the source
workspace: it searched `game/games` for existing proof examples. This exceeds
“only the game” context, so A2 must carry that protocol deviation. Later archives
remove other game consumers, retaining only Lanterns and its shared scene
fragments. Engine/path dependencies remain source-available for Rust compilation.
No session is restarted or supplied feedback as a result of this audit.

A2 also exposed a second isolation omission: the directory-capability filesystem
helper opens `/` before walking to authorized assets. Landlock denied that open,
so the model's Linux proof bake failed with EACCES even though parent setup built.
Future policy grants READ_DIR (directory listing only) at `/`; READ_FILE remains
restricted and the evaluator-content denial probe remains mandatory. A2 keeps
this instrument-induced time loss and is not a clean cross-engine measurement.

Final launcher repair round: A2's log also showed Git failing to read the global
`.gitconfig` and the installed `rg` shim denied execution. Future sessions use
`GIT_CONFIG_GLOBAL=/dev/null`, `GIT_CONFIG_NOSYSTEM=1`, and read/execute permission
for the installed rg directory. This adds no project/history access. These
instrument failures are quoted separately from game/API difficulties. No fourth
launcher repair is authorized in this run.

The first B1 context-trim setup failed before dispatch: the renderer declares
other games as dev-dependencies, and Cargo requires their manifests even for a
release Lanterns build. Context-packaging repair 1 retains those original
manifests with empty library targets, without example/game implementation bytes.
These dev-only crates are not compiled by the release Lanterns build or its
package-scoped tests. Lanterns and the engine dependency sources remain at the
selected ref. The failed pre-dispatch setup is retained separately. This is a
new packaging failure introduced by context trimming, not a fourth repair of the
previously resolved target relocation or CLI-access failure loops.

Version 2 corrects event classification globally: “all lanterns lit” is an
observation, not necessarily victory after Task B. Win events come from the
world journal's `publish phase: ... "won"` entry and retain its actual tick.
The correction is included in the final replay of the four unchanged submissions
and baseline; original timing and version-1 evaluation files remain available. No model is
restarted and no requirement is relaxed.

Version 3 also releases every supported key through existing key-up operations
after load. A focused probe saved while W was held and observed another 0.75m of
movement after load; the corrected probe observes no movement and no held keys.
All unchanged candidates receive the final version-3 evaluation, with no model
restarts. The version-2 A1 intermediate evaluation is retained separately.

## I2 interleaved after run

The runner now loads `prompt-a.txt` / `prompt-b.txt` from this directory. These
are byte-for-byte copies of the original retained baseline prompts (both
attempts of each task agree), eliminating reads from another lane. SHA-256:
A `94682fced5fc4c9e3c0c6036976dc049c09a3713f541821d7cd8e8a688f431a7`;
B `5bdd23dd35416be1f5b5c18847b87fc27995b8e6d8d218e48e3a2f6fd71fe75a`.
The scratch destination is `~/lanes/gamenext/scratch/I2/trials/`; original
attempts remain untouched. Pin after to `7c65726`, because `next/trunk` has
moved. The sequence is after A1, baseline A3, after B1, baseline B3, after A2,
after B2. Evaluator version 3 and all six evaluator hashes remain unchanged.
The 900-second dispatch ceiling, three-repair prompt, same-ref missing-feature
negative controls, isolation read-denial probe and cleanup remain in force.

I2 context audit during after A2 found an inherited omission: `game/bench/cubes`
was not stripped with `game/games` consumers. Its broad grep returned the cubes
proof's status-print line and a one-line crate comment. Both refs contained this
consumer; no other completed I2 log shows cubes implementation output. A2 is
retained with that context deviation. Before final after B2, the runner removes
benchmark implementations and keeps only the cubes manifest and an empty library,
as for other Cargo consumer stubs. No running workspace, prompt or evaluator was
changed. This late packaging correction is disclosed; the six attempts are not
claimed to have identical context packaging.
