# LLP 1146: Evidence an agent can trust — what fails on main, what ran, and what the screen did not show

**Type:** RFC
**Status:** Accepted r2, as built, 2026-10-28. Charlie asked for this document on 2026-10-27 ("Write up an RFC or RFCs about the things you think are worth implementing") and, on 2026-10-28, for its questions to be decided and the result built ("ask the panel or make your own decisions on those calls and then implement"). §0 records those decisions. D1–D4, D6 and D7 are built (§12); D5 is not. Not reviewed by another model family.
**Systems:**
- the async lane (`scripts/async.mjs`) and `scripts/known.mjs`;
- the driver (`scripts/agent.mjs`, `scripts/agent-test.mjs`, `scripts/agent-inspect.mjs`, `scripts/agent-build.mjs`), `scripts/verify.mjs` and `scripts/evidence.mjs`;
- the authored test language (`contract/syntax/src/test_steps.rs`, `contract/syntax/src/parser/steps.rs`, `contract/cli/src/tests_json.rs`, `docs/contract-grammar.md` "Authored tests");
- the runner's per-resource asks and churn alert (`runner/src/perf.rs`, `runner/src/runner/perf.rs`, `runner/src/runner/query.rs`);
- the web JS target's asks and shape check (`host/web-js/perf.js`, `host/web-js/resource-inspection.js`, `host/web-js/shape.js`, `host/web-js/rt.js`).

**Author:** Claude (Opus 5.5) for Charlie Cheever
**Implementer:** Claude (Opus 5.5), 2026-10-28
**Date:** 2026-10-27
**Related:** LLP 1012.001.000 (fresh builds; D1's freshness checks), LLP 1079 (`perf`; D6, work counts are compared and times are not), LLP 1142 (D4, failure counts and alerts; D10, counts within an interaction), LLP 1144 (fling measurement), LLP 1116.004 D7 (`verify`), LLP 1080.002 (`tree --ax`, the one comparison between hosts DEFERRED admits), LLP 1017 P7 (authored tests), `rules/DEFERRED.md` §Agent API and §Process

## Summary

Agents working in this repository say clearly what they did and did not verify. What
they could not do was find that out cheaply:

- Whether a failure is theirs took a second full run on origin/main.
- What has been run against the current sources was known only from memory.
- Several failures that cost the most this week never appeared on any screen a drive reads.

This RFC makes six changes and defers a seventh. None adds an agent operation or a blocking check.

| | Change | Where it lands |
|---|---|---|
| D1 | A failing local run says which of its failures also fail on origin/main | the lane's git notes; `bun scripts/known.mjs -- <command>` |
| D2 | Each drive and test run is recorded against the build it ran on | `<app>/.exact/runs.jsonl`; `bun scripts/evidence.mjs` |
| D3 | An authored test can assert how much work a few steps cost | `perf` counts each resource's asks; the test language gains `perf` and `expect perf` |
| D4 | A test run reports failures the test did not arrange | `agent-test.mjs`, `verify`, `expect no failures`, the runner's churn alert |
| D5 | A host reports the value it applied for an authored style row | deferred (§0) |
| D6 | The web refuses a data answer that native refuses | the web JS target's shape check |
| D7 | An authored test names the hosts it is for | `test "…" on web` |

## 0. Decisions (2026-10-28)

Charlie left §11's questions to the author. They were decided so:

1. **D1's place:** git notes on origin, one ref per lane (`refs/notes/async`, `refs/notes/async2`), so each ref has one writer and a push never conflicts.
2. **D5:** not built. It reads applied values back on four presenters, and it touches DEFERRED's refusal of "behavior diff". It waits for a ruling of Charlie's on that point.
3. **D6:** the web refuses, as native does.
4. **D3's grammar:** a `perf` line with an indented block, as a `test` has, and `expect perf` lines after it. The subject first named `rows` is `built`: a site counts views created and virtual rows constructed, not one per component instance.
5. **The working set:** LLP 1011.001 (SVG image sources on native hosts), whose three stages are built, leaves `llp/current/` for this document.
6. **`RULES.md` §Agents:** the request to implement is the approval for D1's notes and wrapper and D2's record.

## 1. Motivation

The evidence is this week's diaries (`apps/*/diary/2026-10-2[5-7]-*.md`) and James Ide's
commits for Lexy and the iPhone Duo (#1021–#1081).

**Failures that also fail on main were worked out by hand.**
- Each of James's PRs carries a paragraph like this one, from #1079–#1081: "passes except for five tests that also fail without these changes … `IframeCaptureTests.testAClipAfterAPinchAndAPanCapturesItsElement`, which fails 4 of 10 runs on its own". Each such paragraph cost a run of the iOS suite without the change.
- Excord's diaries report "72 of 75; the same three fail without these changes", "three test lanes were broken on main" and "eight Linux presenter tests that compile Shelf fail on main".
- Color-gallery reports "13 keyboard and navigation failures" that it had to rule out.
- The async lane already named each failing test from every step's log (`scripts/async.mjs` `failures`), in a `result.json` on one machine that no other checkout could read.

**What has been verified was tracked in prose.**
- James writes "the existing phone observations do not qualify this new build" and "Not checked: the icon and the 'Far Focus' title in a running app" (#1023).
- Excord writes "Not driven: the macOS rows and attention, and anything on Windows" (desktop) and "Not seen yet: Charlie's account on his MacBook" (Mac scroll).
- The driver checks that a build is fresh before it drives (LLP 1012.001.000 D1), and recorded nothing afterwards.

**Work that grows unseen.** Excord's Mac transcript stalled on a real account. In a drive:
- Twelve events in another channel asked 60 list queries, and 6 after the fix.
- A closed member sheet built 98 rows 318 times.

Neither changed anything on screen, so no `expect` could hold the fix. `perf … during` measured both kinds of work (LLP 1079 D6), but a test could not assert on it.

**Failures nothing reported.**
- **Connecting… for good.** Excord sat on "Connecting…" because the host refused a gateway answer over its turn budget, and nothing asked again. The refusal was in the journal, and it took a `SIGUSR1` trace to find.
- **The test runner hid resource failures.** It treated them as authored states and printed them only under `EXACT_AGENT_DEBUG`.
- **The churn line stayed in the journal.** The runner's line for a resource asked again and again (`count_churn`) went to the journal through `log()`, not to the alert queue.

**A host drew something other than what the author wrote.**
- On Windows the composer's emoji face drew in the accent, while its authored `color` was grey. The painted presenter drew symbols in `tint-color` (Excord, windows-symbols).
- `layout <testId>` showed the cause at once, because its rows say where each value came from (`runner/src/agent/source.rs`).
- Those rows are the kernel's computed values, not what the host drew. So a person still had to look at the screen to see the difference.

**The web accepted what native refused.**
- One of Excord's channels never loaded on the Mac: its answer held a lone UTF-16 surrogate.
- The native data seam refuses such an answer ("result string contains a lone surrogate", `js/src/engine.rs`).
- The web JS target took it, so the web drives, the cheapest the repository has, passed.

**Some tests can only pass on one host.** Excord's "a user action before the launch's restore lands wins: Back to the list cancels it" can only pass on the web. Without a host scope it would turn any native gate red.

## 2. Constraints

- **No new agent operation** (DEFERRED §Agent API). A new measurement is a form of `perf`.
- **No new blocking check.** Each change reports. Exit statuses change only where an author writes an assertion (D3, D4's `expect no failures`).
- **No maintained expected-failure list or debt ledger** (DEFERRED §Process). D1's record is the lane's observation of origin/main, written by the lane and edited by no one.
- **No session record and replay** (DEFERRED §Agent API). D2 records that a run happened and what it found, never its inputs or frames.
- **No timings as checks** (LLP 1079 D6, `RULES.md`). D3 asserts counts on the agent's clock, with answers the test controls.

## 3. D1: what fails on main, said beside your run

**The lane publishes.** After it checks a commit, `async.mjs` attaches what it found to that commit as a git note and pushes the lane's ref to origin (`known.mjs` `noteOf`, `publish`). The note holds:
- each check's command, and its exit status or why it was skipped;
- whether the check ran to the end: one that did not compile, hung, or failed with nothing a pattern names says nothing of the tests it held;
- every failure, named as `failures()` names it.

A push that fails is said in the lane's output, and the next commit's push carries it.

**A local run reads.** `bun scripts/known.mjs -- <command>` runs `cargo test …`, `bun test …` or `bun host/apple/build.mjs --test …`, prints its output as it comes, and, when it fails, names the failures with the lane's own patterns. It fetches `refs/notes/async*` from origin and finds the nearest first-parent ancestor of `HEAD` on origin/main whose note ran a check of the same kind to the end. Each failure is then one of three kinds, compared by the test's name without the check's:
- **known:** it failed there, with how often it failed in the last ten such notes;
- **new:** a check of that kind ran there and this test did not fail;
- **unknown:** no note there ran such a check to the end.

What a failed iOS run would print (an illustration):

```
3 failed: 2 known on origin/main at 9407a448a2 (lane, 2026-10-27 14:02), 1 new
  known ExactKitTests.IframeCaptureTests testAClipAfterAPinchAndAPanCapturesItsElement failed (failed 4 of its last 10 lane runs)
  known ExactKitTests.PageEdgesIOSTests testALargeTitleCollapsesOverAFixedControlBar failed (failed 10 of its last 10 lane runs)
  new   ExactKitTests.NavigationPopIOSTests testAFinishedPopTakesTheAppBackAfterTheSourcesKeyMoved failed
```

The exit status stays the command's: a known failure is still a failure. `--log <file> --kind cargo|xctest|bun` reads a saved log instead. The lane runs no app's authored tests, so `agent.mjs --test` is not wrapped: every failure there would be unknown.

## 4. D2: a record of what ran, against which build

**Every run is recorded.** `agent.mjs` (each drive and each `--test`) and `verify` append one line to `<app>/.exact/runs.jsonl` (`evidence.mjs` `recordRun`). It holds the time, the host and target, the timing and touch modes, what ran, what it found, the commit, and the build: the digest of the build's record, the web's `dist/.exact-build.json` or an Apple build's `receipt.json`.

**Current or stale.** The driver refuses a build older than its sources, so a run's build was fresh when it ran. A run is current while its build is still the host's build and the driver's own freshness check (`webChanges`, `nativeBuild`) still finds nothing changed; otherwise it is stale, and the record says which files changed or that a later build replaced it. Linux, Windows, Android, tvOS and phone builds are not identified yet: their runs are listed as unchecked.

**A person's check is recorded too.** `bun scripts/evidence.mjs note <host> "<what was seen>" [--device "<model, OS>"]`.

**`bun scripts/evidence.mjs --app <name> [--since <rev>] [--draft]`** prints the latest run of each kind on each host with its standing, then the hosts whose code the branch changed (`git diff <since>...HEAD` and the working tree; shared layers touch every host) that no current run covers. `--draft` prints lines for a diary's Checkpoints or a PR's Test Plan:

```
- web, chrome: test apps/caltrain/app.test.contract: 3 passed, 0 failed (agent clock)
- macos: not run against these sources
```

The record is local and git-ignored. Deleting it costs only the drafts, and no step requires it. It holds no inputs, frames or state.

## 5. D3: work counts an author can assert

**`perf` counts each resource's asks.** While measuring, the runner counts each ask of a resource's data source where it journals the ask (`query.rs`, `count_ask`), and the web JS target counts each request it begins (`resource-inspection.js` `begin`). Neither count is rolled back by a refused commit: an ask that was sent happened. `perf`'s reply lists `resources: [{name, asked}]`, and the driver's `perfDelta` subtracts them.

**The test language gains a measured block and an assertion on it:**

```
test "typing and a message in another channel re-ask only the channel list"
  clock data
  tap "channel-general"
  perf
    request POST "/control/inject" {"channel_id": "<#help's id>", "what": "typing"}
    request POST "/control/inject" {"channel_id": "<#help's id>", "what": "message"}
    clock data
  expect perf query channelList <= 1
  expect perf query serverList == 0
  expect perf built "MemberList" == 0
```

(`/control/inject` is Excord's fixture's; `<#help's id>` stands for the id the fixture gives #help.)

- **The block.** `perf` runs its indented steps between two whole-tree `perf` reads and keeps their difference (`perfDelta`, which refuses a difference across a new plan, incarnation or `seq`). It also reads, before and after, the view of each `testId` an `expect perf` line after it names.
- **The subjects.**
  - `query <resource>`: the resource's asks.
  - `built "<name>"`: views created plus virtual rows constructed, at the plan sites of the component of that name (the development source map's names; one the plan declares and never built counts 0), else under the view with that `testId`.
  - `evaluated "<name>"`: the bindings evaluated there. Each runtime evaluates differently (Caltrain's station search: 7 on the web, 34 on macOS), so a test that compares it names one host.
- **The comparisons** are `==` and `<=`. An `expect perf` reads the last `perf` block before it; the parser refuses one with none.
- **Determinism.** Counts repeat when the steps and the answers are the test's (LLP 1079 D6). The parser refuses a launch line, a `clock … real` or another `perf` in the block; the runner refuses the block under platform timing.
- **A failure names what counted:** `built "StationRow" counted 6 in the \`perf\` block at line 3, expected == 0 (app.contract:465 1, app.contract:470 1, app.contract:471 1)`.
- **How this fits LLP 1142 D10,** which introduced no warning threshold or performance check of the framework's own: this is an author's test, like `expect state`, and fails only on a count its author wrote down.

## 6. D4: a run says what the screen did not

**Unarranged failures are noted beside the test.** At the end of each test, `agent-test.mjs` compares what has failed (`state.failures`' refusal counts and `failed`'s reasons, `mutationFailures`, and the journal's churn alerts) with nothing, and notes each failure the test did not arrange:

```
test "…": ok
  note: unarranged: resource gateway was refused once (turn 868 ms over 100 ms)
```

A resource's failures are the test's own while a `fail fetch` it armed fires; a send's, when an `expect refusal` names it. The test still passes: resource failures stay authored states. `verify` counts each as a finding, beside `check`'s, which never fail its run.

**An author can ask for none.** `expect no failures` fails when, since launch or the last `expect no failures`, an unarranged failure arrived.

**Repeated asks are an alert.** `count_churn` reports through `alert()`, so the line reaches the platform's log in a development build, and D4's notes.

**A test that ends mid-request says so:** `ended with 1 request in flight (channelWindow): a reply lands at a clock step, so what the test read last may not include it`.

`check` is unchanged. It reads the screen by heuristics; these are the runtime's facts.

## 7. D5: what a host applied for an authored row (deferred)

Proposed: `layout <target> applied` would add, to each style row a host can read back from its own views (`color`, `background-color`, `border-radius`, `opacity`, `font-size`, `font-weight`, `padding`), the value it painted, and `verify` would count a row the author wrote whose applied value differs from the kernel's computed value. A row from `initial` is the platform's to choose (LLP 1115) and never a finding. The driver's text form already prints `applied` when a row carries one (`agent-inspect.mjs` `renderNode`); no host fills it for style rows.

It compares a host with its own kernel, not two hosts, but DEFERRED refuses "behavior diff" and admits one comparison between hosts (LLP 1080.002's). Building it waits for Charlie to rule whether it needs a waiver; if it does, the take offered is LLP 1080.002's shape: a fixed set of rows, findings that never fail a run, and no scoring.

## 8. D6: the web refuses what native refuses

A Rust `String` cannot hold a lone UTF-16 surrogate, and the native data seam refuses an answer with one. The web JS target's shape check now takes a string only when `String.prototype.isWellFormed()` holds (`shape.js` `conforms`), and a resource answer refused for one fails with the native message, "result string contains a lone surrogate" (`rt.js`).

## 9. D7: a test names its hosts

`test "…" on web` or `on web, macos`; a test without `on` is for every host. The hosts are the driver's names (`web`, `macos`, `ios`, `tvos`, `linux`, `windows`, `android`; the sample host's sessions are `macos` and `ios`). The runner skips a test on a host it does not name and says so (`test "…": skipped on macos (on web)`); a skipped test is neither passed nor failed. Excord's web-only restore test carries `on web`.

## 10. Not proposed here

- **Device trace to replay.** Lexy's hidden tab bar was not reproduced on a simulator, and "which transition ran on the device … is not known" (#1081). Replay is refused (DEFERRED). Native transition events in a development trace would be a form of LLP 1142's trace, and belong there.
- **Driver gaps.** These are LLP 1080.000's, LLP 1142 D8's and QUEUE.md's:
  - the browser's Back after a reload;
  - a drag into the iOS keyboard;
  - filming under real timing;
  - a drag's release velocity under the agent's clock;
  - taps under the macOS title bar.
- **Fixture realism**, such as a busy account and raw request bodies. This is Excord's own RFC's (`apps/excord/llp/0001-excord.rfc.md`).
- **Fix-its for compiler errors.** These belong in the builder-facing LLP 1116 series.
- **A provisioning check before a device build.** Excord's build compiled and then could not install, because the profile lacked the Declared Age Range capability. This is a QUEUE.md line, not a design.

## 11. Not built

- **D5**, for the reason in §0.
- **A second churn rule:** a resource refused three times in a row with the same arguments within ten seconds (Excord's unloaded channel). The runner keeps refusal counts, not the arguments of each, so it needs its own record.
- **Phone, Linux, Windows, Android and tvOS build identities in D2:** their runs are recorded as unchecked. A phone build's receipt would carry the digest of what it installed.
- **D1 in the authored test runner:** the lane runs no app's authored tests, so it would have nothing to compare.
- **`--again N`** (rerun only the new failures N times) in D1.

## 12. As built (2026-10-28)

Verified by running:

- **D3, D4, D7 on the web and macOS.** Caltrain driven with a test file using `on macos`, a `perf` block, `expect perf query`, `built`, `evaluated`, a `testId`, and `expect no failures`. On the web (Chromium 1243 via `CHROME`, the pinned 1248 not being installed on this Mac): a search asked `matches` once and no `northBoard`; `StationRow` built 6; the macOS-only test skipped; a wrong count failed naming what it counted, by source line. On macOS (`bun host/apple/build.mjs`, then `agent.mjs macos --test`): the same asks and views built; 34 evaluations against the web's 7; the macOS-only test ran.
- **D2.** Two Caltrain web runs recorded as current; an edit to `apps/caltrain/app.contract` made both stale, naming the file; restoring its bytes made them current again; `--draft` listed them and the hosts with no current run.
- **D1.** `known.test.mjs` publishes a note from one clone to a bare origin and classifies a failure from another clone; `bun scripts/known.mjs -- bun test <a failing file>` keeps the exit status and says the failure is unknown on origin/main, which has no notes until the lane publishes one.
- **Unit tests:** the parser's forms and refusals (`contract/cli/tests/it/tests_decl.rs`), the runner's ask count and churn alert (`runner/src/runner/query.rs`), `perfDelta`'s resources, `unarranged`, `churnLines` and `perfExpectation` (`scripts/agent-*.test.mjs`), D1's classification (`scripts/known.test.mjs`), and D6's shape check (`host/web-js/shape.test.mjs`).
- **Not run:** an unarranged failure in a real drive (unit-tested only), D6 in a browser, and the lane publishing from the machine that runs it.
