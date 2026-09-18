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
