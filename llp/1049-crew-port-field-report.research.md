# LLP 1049: The Crew port field report

**Type:** Research
**Status:** Draft
**Systems:** All; chiefly the data seam (grants, native executor admission, the runner's journal and kept answers), the agent driver, the Apple host, and building an app outside the repository
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-24
**Related:** LLP 1012.000, 1016.000, 1027.000.000, 1036.001 (the RFCs this report produced); LLP 1041 §8.4 (the ordered lane's numbers, amended); LLP 1035.001 D10 (the tablist rule, amended); LLP 1035.004 (symbol roles); LLP 1036 (the earlier frictions list)

## Summary

Seth's agent ported Crew, an iOS app with a TypeScript data module that
talks to a local daemon, to exact2 at `53839332` and kept a log of every
problem: nine flaws (exact2 wrong or unsafe) and sixteen DevX frictions.
This document records what each became. Sixteen were fixed in code
(F4/F6 turned out to be F2 and F7, §2) and three in part (F9, D7, D16);
four went to RFCs as decisions (D5, D9, D12, D13; D1–D4's larger shape
is a fifth question, LLP 1036.001), and two to issues (D2, D3). Three
decisions are Charlie's (§3). The report is the first
from someone building a real app outside the repository, and most of it
landed where no in-repo app goes: worker placement, `else` placeholders
under load, a daemon with a large JSON body, a camelCase secret name, an
agent that has to see a simulator.

## Findings

### 1. What each item became

| item | what | became |
|---|---|---|
| F1 | a bad `secret.keep` name silently dropped every grant | fixed: the TypeScript bake refuses the grants naming the line; native hosts journal the parse error and name it in every refusal; `apps/realworld`'s `jwtToken` renamed (`503123c1`). Readers still lenient: issue `grant-readers-disagree` |
| F2 | ordered admission refused a trivial app at boot | fixed: the ordered lane charges request bytes at admission and the response ceiling only to the running job, so its sixteen-call count is the limit (LLP 1041 §8.4 amended); `else` placeholders the bake answered are not asked again at `data_ready` (`c318ed04`) |
| F3 | superseded requests kept their executor slots | fixed: hosts ask `Runner::holds` after each commit and `Core::forget` drops or aborts what nothing wants; writes still run (`c318ed04`) |
| F4, F6 | a 190 KB reply never reached a worker-placed module on iOS | fixed by F2/F3/F7's commit: reproduced on the iOS simulator at the pre-fix base with a Crew-shaped app and gone on the integrated branch; the cause was the admission wedge, not the body's size (§2). Both halves' 190,785-byte paths are regression tests (`e96d18eb`) |
| F5 | `clock settle` hung for minutes | fixed: one twenty-second deadline per call on Apple and Linux, not per round; the reply says it gave up on requests (`7a7b6ab8`) |
| F7 | refused placeholder asks stayed pending forever | fixed: a resource refused ordered admission is asked again once the last refusal settles; a refused mutation ends unsent (`c318ed04`) |
| F8 | `role="tablist"` silently became a segmented control | fixed: projected only when every tab is one image or its label alone; otherwise kept as authored with the tab-bar trait, and journaled either way (`a3956ef2`; LLP 1035.001 D10 amended) |
| F9 | a promise shared between two answers aborted the driver | message and docs: the refusal names the cause and the remedy (`b68ed51e`); module-wide liveness stays LLP 1027.003.000 §13's open decision (`queue/`) |
| D1 | the Bun pin was a hard stop | fixed: the scripts put the Bun that passed first on PATH for Cargo's build steps, and the refusal names a side-by-side install (`f1fe9aac`); the floor itself is LLP 1036.001 Q2 |
| D2 | a private git dependency blocked offline builds | issue `root-lock-pins-private-snapback`; LLP 1036.001 D4 |
| D3 | Hermes provisioning was manual and slow | issue `ios-hermes-provisioned-by-hand` (also: the pin disagrees between `js/build.rs` and LLP 1027); LLP 1036.001 D5 |
| D4 | no scaffold for an app outside the repo | fixed now: a workspace without `[profile.apple-dev]` builds `release` and says so (`f1fe9aac`); the scaffold is LLP 1036.001 |
| D5 | `agent.mjs` relaunched the app every call | LLP 1012.000 D1 (`--keep`/`--attach`) |
| D6 | `tap` reported success under the keyboard | fixed on iOS and the web: a covered target is refused and nothing changes; replies carry `pressed` (`7a7b6ab8`). Off-viewport policy: issue `agent-tap-offscreen-target` |
| D7 | no real-time wait in the driver | F5's fix makes `clock settle` an honest wait; LLP 1012.000 D4 proposes `clock replies` |
| D8 | diagnostics stopped at the TypeScript boundary | fixed: every `fulfil` and dropped-reply journal line carries the outcome (status and bytes, or kind and message) (`72faa0d9`) |
| D9 | no push transport | LLP 1016.000 |
| D10 | the simulator was invisible | fixed: driving a simulator brings it up without taking focus (`7a7b6ab8`); Xcode 27 has no Simulator.app: issue `xcode27-no-simulator-app` |
| D11 | only 17 symbol roles | fixed: `repeat` and `activity` rows (`b25436c9`); clip paths accept a subset of CSS: issue `clip-path-subset-of-css` |
| D12 | the time ban was invisible until runtime | LLP 1027.000.000 (the date as a host fact; D5 the build-time diagnostic) |
| D13 | the driver's clock was epoch 0 | LLP 1027.000.000 D3 (an agent substitute for the date; `now()` stays elapsed time) |
| D14 | `store exact.kept.*` denied on every answer | fixed, and worse than noise: Apple never kept an answer, so the returning-user first frame never happened; kept answers now go to the kv store (`503123c1`; LLP 1027 as-built note) |
| D15 | long driver runs died silently | fixed: errors name the operation, the app's exit or signal, crash reports since launch and the last twenty host lines; a 120 s reply deadline on native hosts; the app logs why its stream ended (`7a7b6ab8`) |
| D16 | reads as a wait desynchronised fake and real time | partly: under `--timing platform` the driver's clock starts from the app's (`7a7b6ab8`); wall latency in the journal: issue `journal-has-no-wall-latency` |

### 2. F4/F6: not the body's size

The report's strongest claim, "a 190 KB reply never reaches a
worker-placed module; main placement fulfils in milliseconds; 24 B works",
pointed at the size switch in `DarwinTransport` (a body over 64 KiB
completes only while a reader drains it). On macOS neither half stalls:
the owner-thread half (Castle's `profile` source under
`Placement::Worker`) and the executor-and-transport half (a real
`NSURLSession` against a loopback server, handed-off turns queued around
the read) both deliver 190,785 bytes in order in about 0.2 s.

A Crew-shaped app on the iOS simulator (worker placement, three resources
each with `else`, an `every(2000)` poll changing one argument, a loopback
server with the same two bodies) reproduced it at the pre-fix base and not
on the integrated branch:

- **Base.** Six ordered asks at boot. The fourth was refused ("native
  executor admission limit reached") and the fence refused the rest
  ("earlier ordered admission refusal must settle first"). The three
  fetches were logged but never reached the server; their continuations
  were refused; `session` and `conv` stayed pending for good, and `clock
  settle` took 351 s (F5). Once the fence lifted, the poll's next 190,785
  byte `/api/state` reached the server and fulfilled.
- **Integrated.** Three asks at boot, no `#else` re-asks, no refusals:
  `fulfil 6 (conv) [HTTP 200, 190785 bytes]`, `fulfil 4 (session) [HTTP
  200, 24 bytes]`; the superseded poll request was dropped before it was
  sent; `clock settle` settled with nothing pending, and the run took 7 s.

So F4/F6 was F2 and F7: at boot nothing reached the network, and which
request happened to come back later decided which body looked broken.
Main placement answered placeholders synchronously and never filled the
lane, which is why it "fixed" it.

### 3. Decisions left for Charlie

1. **Module-wide liveness** (F9): LLP 1027.003.000 §13, unchanged.
2. **The four RFCs' open questions**, chiefly: is Crew the consumer that
   puts push data (1016.000) on the doing list, and what comes off; is it
   the consumer 1027.000's "Alternative" waits for (1027.000.000); the
   resident driver (1012.000 D1); and Node or Bun only for outside apps,
   the Bun floor, and Messages' Snapback client (1036.001).
3. **Off-viewport taps** (issue `agent-tap-offscreen-target`): refuse, or
   scroll into view first.

Lane decisions made under the fixes and worth a look: F3 releases a
forgotten GET on the independent lane too, which changes Completion
Storm's recovery (a new cohort is admitted with no release-all first);
Apple's kept answers moved from "beside the secrets" to the kv store,
with the host granting itself `storage.kv exact.kept`.

## Confidence

High for every "fixed" row: each was driven or tested in the lane that
made it and the integrated branch passed the five checks (see the
landing commit). F4/F6's cause was reproduced on the iOS simulator
with a synthetic app shaped like Crew, not with Crew itself. The report's cost
figures (rebuild cycles, minutes) are Seth's and were not re-measured.
