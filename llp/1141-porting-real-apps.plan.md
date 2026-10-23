# LLP 1141: Porting real apps — the euxy loop, as a method for many apps

**Type:** Plan
**Status:** Accepted, 2026-10-23, as r3: r2 with Charlie's rulings (§10) and Muse's review of r2 folded in (§R3). Draft r2, 2026-10-22. r1 was reviewed blind by GPT-6 Astra (xhigh) and Grok 4.7 (xhigh), direction and method only (`llp/reviews/plan-2026-10-22-1141-r1.{astra,grok}.md`); both READY WITH CHANGES, and r2 folds them in (§R). Muse was not run: its login had lapsed. The pilot (§6, §10.6) starts next, and each app gets a sub-LLP.
**Systems:** Nothing in exact2 directly. Each app's sub-LLP decides changes to whatever its port shows wanting, and adds `QUEUE.md` entries, `docs/agent-pitfalls.md` entries and recipes. The ports, the originals and their measurements live outside the repository (§7).
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-22
**Implementer:** none yet; each app is a lane
**Related:**
- LLP 1141.000 (the candidate apps).
- The euxy record: `~/projects/euxy-compare/` (`BRIEF.md`, `RESULTS.md`, `bench/`); LLP 1125 (what the ports wrote by hand, made Exact's own); about 285 commits citing euxy from 2026-10-15 to 2026-10-22 and the `QUEUE.md` entries naming "euxy port N".
- Earlier ports: LLP 1049 (Crew), LLP 1054, 1054.000 and 1114 (Bluesky).
- The programs this complements: LLP 1087 (the authoring bench), LLP 1109 (the app farm), LLP 1116 and its rounds (the bake-off; 1116.019 for the replay gate, 1116.013 for where tokens go), LLP 1129 (the brand probe), LLP 1046.010 (the garden bake-off, for games).
- LLP 1115 (author > platform > CSS default; the oracle is a hand-built screen on the same OS version); `rules/RULES.md` "Time budgets".
- `docs/recipes/porting-and-release.md`; `scripts/agent-compare.mjs` (`screenshot … compare <reference.png>`); `scripts/no-tells.mjs`.

## R. Review, and what r2 changed

Both reviewers found the direction sound and the replay/fresh-port split (§4.6) the most
valuable part. Where both raised a change, r2 adopts it. A change from one reviewer is
adopted where the record supports it.

| Change | Raised by | Where |
|---|---|---|
| Authoring cost (wall clock, tokens) is recorded on every port but is not a graduation bar: one paired run cannot carry it. A claim that a fix made building cheaper needs repeated pairs, the authoring bench or the bake-off | both (BLOCKING in Grok's) | §2, §4.8 |
| Runtime bars are against the original where it is native; the agent-built native port is the authoring comparator, not the runtime oracle | both (BLOCKING in Grok's) | §2, §4.2 |
| Graduation requires a predefined slice verified complete, no unresolved critical bug and no Exact-caused tell; an app ends as passed, closed with named limits, or inconclusive | Astra (BLOCKING) | §4.7 |
| Startup is measured at milestones a user sees (content shown, a named control usable), not at a share of the final layer count, which differs by renderer | Astra | §2 |
| A timing bar passes only when the medians clear it and the p10–p90 bands are apart; Maestro drives, the probe times; idle CPU is "no recurring work" against the bare template, not a literal 0.0% | both | §2, §5 |
| Peak memory, growth over repeated journeys, and CPU during the app's steady activity are measured | both | §2 |
| The grader runs on builds with no development affordances, records which stack it guessed after grading, and a share of pairs is regraded in reversed order; a second grader confirms graduation | both | §4.3 |
| The brief is written from the original only, by someone who will not build the Exact arm, and frozen; arms cannot read each other's work | Grok | §4.1, §4.2 |
| Intentional design, platform-owned behaviour and the original's own defects are classified before building | Astra | §4.0 |
| A class fix names a second consumer that exists and is probed there; the coverage matrix is mechanisms, and overlap is kept as replication | both | §4.5, §6 |
| A cap of four fresh ports per app, an economic stopping rule, and a three-app pilot before scaling | both | §4.7, §9 |
| A follow-up task on a finished port measures whether Exact stays cheap as an app changes | Astra | §4.9 |
| Originals and comparators are re-benched beside Exact whenever the machine, OS or fixtures change | Astra | §5 |
| The first euxy port wrote 343 lines of Swift; the 1,607 in LLP 1125 are a later port's | Grok (a factual error in r1) | Summary |

Not adopted: relaxing the memory bar to 1.3× for the first wave (Grok). Astra argued against
relaxing it to pass euxy, and a miss is better kept visible as a named limit (§4.7).

### R3. Muse's review of r2, and what r3 changed

Muse Wonder 1.0 (xhigh) reviewed r2 with the rulings, blind to the other reviews' text
(`llp/reviews/plan-2026-10-23-1141-r2.muse.md`): READY WITH CHANGES. r3 takes:

| Change | Where |
|---|---|
| A pass needs one fresh port graded against calibration pairs with known differences, so the grader's sensitivity is measured, not only its regrade spread (Muse's BLOCKING) | §4.3, §4.7 |
| Model family is a matrix axis; before the program scales, one pilot app is ported again by a second family on the same frozen brief (Muse's BLOCKING) | §6 |
| Memory and size bars apply to Exact's marginal cost (the app minus its bare template, against the native app minus a bare native app); the absolute numbers are reported and may close as named limits | §2 |
| A timing bar passes on a paired, interleaved interval of the median ratio, not on non-overlapping bands | §2 |
| Recognition by the grader is tested against chance, and scores are split when it is above chance | §4.3 |
| Every flow adapter and every step diagnosed as not counted is logged and reported | §4.0 |
| A class fix may name a plausible consumer not yet built, flagged as such, with the probe deferred to the next app on that row | §4.5 |
| The journey must complete by accessibility labels as far as the reference's does | §2 |
| The held-back app is quarantined; Charlie's time is counted in the pilot's costs; ports take at most half the lanes, so the bake-off keeps running | §6, §9 |

Not adopted: two independent fresh ports for every pass. The calibration pairs answer the same
doubt (is a closeness difference real?) for a grading run rather than a second build.

## Summary

Charlie's aim is that Exact2 be the best way for an agent to make an app: faithful to what
was asked, fast to start, fast and light while it runs, and cheaper in wall clock and tokens
to build than the alternatives. The bake-off (LLP 1116) measures that against written specs.
A port of a real app measures it against an app someone already shipped, which gives three
things a spec cannot: a fixed answer for every screen and behaviour, a hand-built reference
to compare speed and memory against, and the size and feature mix of real software.

Euxy showed how much that finds. Its first Exact port (2026-10-08) took 1.55× the SwiftUI
port's wall clock and 2.5× its cached input, drew its first frame at 601 ms against 285, and
needed 343 lines of Swift for what Exact lacked. Over the next two weeks euxy was ported again
and again (port 22 by 2026-10-22; by LLP 1125 a port carried 1,607 lines of Swift for its
scheduler, CoreMIDI and sampler). Each port's friction, launch, memory and CPU were measured
against the native ports on the same simulator, and the fixes landed in exact2: a transport
and MIDI (LLP 1125), smaller per-node memory on Apple, idle CPU while playing, sheets, tabs,
the driver.

This plan makes that loop a method and applies it to open-source apps chosen so that,
together, they exercise the mechanisms apps are built from (§6). Every app goes through the
same steps (§4):

1. Pin the original, measure it, and classify what in it is design and what is platform.
2. Write and freeze a brief in tiers.
3. Build an Exact port and a native port with the same model.
4. Measure (§2, §5).
5. Give every loss an owner.
6. Fix in Exact what is Exact's, as a class, and probe the fix in a second app.
7. Replay and port again on the new main, until the app passes, closes with named limits,
   or is inconclusive.

Discovery is what a port does best. It shows what Exact lacks and what it costs at a real
app's scale. Claims about authoring cost are confirmed with repeated pairs, not with one port.

## 1. Why ports of real apps

| Program | What it measures | What it cannot see |
|---|---|---|
| Authoring bench (1087) | The cost of small tasks, over many trials | A whole app's structure |
| App farm (1109), bake-off (1116) | Whole apps from a written spec, Exact against another stack, a blind grader, many pairs | Fidelity has no ground truth: the spec is each builder's to interpret. Apps are as large as a spec, so small |
| Brand probe (1129) | One branded app | Anything but the brand |
| **A port** | Difference from a shipped app, screen by screen; speed and memory against a hand-built app; real size and features | Novel design (the original decided everything); authoring cost beyond one pair |

An original is LLP 1115's oracle made concrete. When it is native, it is the hand-built
screen on the same OS that a tell is judged against. When it is a React or Expo app, the
port shows what Exact makes of the same intent, and the original is the runtime reference
for that stack.

What euxy found that the bake-off's nineteen rounds had not:

- **Whole subsystems.** It needed a lookahead scheduler, CoreMIDI and a sampler, written by
  hand in Swift. LLP 1125 made them Exact's own.
- **Cost at scale.** At launch, 401 layers turned the first frame from the bare template's
  256 ms into 601 ms. Per-node memory and idle CPU while playing were measurable only beside
  a native port doing the same work.
- **Small things a real design asks for.** Sheet detents and grabbers, a selected tab at cold
  launch, a launch-screen image, sharing a picture of a node, and driving a native menu.

A port is not the instrument for every question:

- Causes and scaling are diagnosed with isolated benchmarks (`scripts/metrics.mjs`, the list
  benchmarks).
- Energy and touch latency need a physical device.
- Authoring cost needs many trials: the authoring bench, or the bake-off's pairs.
- How Exact does on a new design, with no original to copy, is the bake-off's.

## 2. The goals, and how each is measured

The **arms** are the builds compared (§4.2): the original, the Exact port and a native port.
The **runtime reference** is the original where it is native on the platform measured.
Otherwise it is whichever of the original and the native port is better on that measure.

| Goal | Measured as | Default bar |
|---|---|---|
| **Complete** | The slice the brief predefines, each tier verified by driving it, not taken from the builder's report; critical bugs (a crash, data loss, a feature that does nothing) | Every tier of the slice verified; no unresolved critical bug |
| **Faithful** | (a) The differences from the original a user would notice, per screen, listed by a blind grader driving the original and both ports (§4.3). (b) Tells (LLP 1115), on a build with no development affordances, in dark mode and at the largest Dynamic Type. (c) Numbers the original defines: a total, a clock, the rows after a relaunch. (d) `screenshot … compare` against the original's pictures, which locates differences and does not judge them | No tell Exact caused. The grader rates the Exact port at least as close to the original as the native port, by more than its measured noise. Every number the original defines matches |
| **Fast start** | From process start, Release: first frame; **content shown** (the first screen's named content element is drawn); **usable** (a named control answers a tap). Layer counts are kept for diagnosis. Web: FCP, LCP and bytes transferred on a throttled mobile profile | Content shown and usable ≤ the runtime reference's × 1.1 |
| **Fast while running** | Over the journey (§4.0), from each input's timestamp: time to first feedback, and time to the settled frame, kept apart from the length of any animation. While scrolling the longest list: the hitch ratio (ms of hitches per s). Web: INP per step | Each step ≤ the reference's + one frame + the probe's quantum; hitch ratio ≤ the reference's, or under 5 ms/s |
| **Light** | Footprint 10 s after launch, its peak over the journey, and its growth over ten repeated journeys. CPU over 20 s at rest, and over 20 s of the app's steady activity (euxy playing). Installed size. Web: JS and CSS bytes | Marginal footprint at rest and at peak (the app's minus its bare template's) ≤ the reference's marginal (its app's minus a bare app of its stack) × 1.2, the absolute footprints reported beside them; no growth across repeated journeys. At rest, no recurring work the bare template does not do. Steady activity ≤ the reference's CPU. Marginal size likewise ≤ the reference's |
| **Accessible** | The journey run by accessibility labels alone (Maestro, §4.0) | It completes as far as the reference's does |
| **Quick and cheap to build** | Gross wall clock and wall clock less service pauses; output tokens, fresh input, cache reads, calls, the largest context, and the dollar cost billed, all to *verified completion*, failed attempts included | None for graduation: recorded on every port, and claimed only as §4.8 allows |

A timing or memory bar passes when a 90% interval of the paired ratio clears it. Launches and
journeys are interleaved arm by arm (§5), so each Exact run is paired with the reference run
beside it. When the interval straddles the bar, more runs are taken, up to a budget declared
before the app's first port; past it, the measure is inconclusive. Energy has no bar until
device runs exist (§5).

`rules/RULES.md` budgets "cold start to interactive first frame" at 100 ms p50. That budget is
Exact's own phases, which `SIMCTL_CHILD_EXACT_SMOKE=1` prints. This plan measures from process
start, which includes the OS's share, so the two are recorded side by side and not compared.

Two more measures have no bar:

- **Native escape.** The lines of Swift, Kotlin or JavaScript the Exact port wrote in native
  modules. Each module is read for what Exact should provide. A well-supported optional
  module can be the right boundary, so a line counted here is a question, not a defect.
- **Reuse.** The lines of the original's source carried into the port. Reuse favours Exact
  whenever the original is TypeScript, so it is reported beside any authoring ratio.

## 3. Choosing an app

An app enters the portfolio when it meets all of these:

1. **Well made.** It is good in its own stack, so the reference is high. Prefer apps known
   for their craft.
2. **Licence.** It may be read, built and run privately (§8); any OSI licence allows that.
3. **Buildable without a private account.** It talks to a public API, a demo server, a
   backend run locally (a container) or fixtures.
4. **Sliceable.** A slice fits one builder session of one to three hours, as tiers; euxy had
   nine. A large app is ported as a slice, named in the brief. Each wave may have one deep
   app whose tiers run longer, since depth is what produced LLP 1125.
5. **Builds today.** It builds with current toolchains, whatever the date of its last commit.

Preferred among those that qualify:

- **Rows of the matrix (§6) not yet covered,** or covered by one app only (a second
  independent app on a row is replication, which is worth having).
- **Several platforms.** One port is then judged against two native references, as Pocket
  Casts would be on iOS and Android.
- **iOS.** Charlie's priority, 2026-10-16: iOS > web > Android > macOS > Linux > Windows.

## 4. One port, step by step

### 4.0 Pin the original, measure it, classify it

- Clone it at a commit, recorded with its licence.
- **Define the slice first**: the tiers the brief will ask for. Capture only what the slice
  reaches. A large original is never captured whole.
- Build it Release for each platform it ships that the port targets.
- Take every screen of the slice in light and dark, at the default and the largest Dynamic
  Type, plus the states the journey reaches. These pictures and the original's numbers stay
  fixed for the life of the app's sub-LLP.
- **Classify** what the screens show:
  - *authored design*: what the original's authors chose, which the port must match;
  - *platform-owned*: what the platform supplied, which the port's platform should supply
    (LLP 1115);
  - *the original's defects*: what the port need not copy.

  The grader receives this classification.

**The journey** is 15 to 40 steps a person would take: open the app, scroll the feed, open a
detail, compose, search, change a setting, background the app and return. It is written once
in prose, with each step's semantic action and assertion, and once as a
[Maestro](https://maestro.mobile.dev) flow. Maestro drives iOS, Android and web apps through
the accessibility tree whatever the stack, so one flow drives every arm; an arm may need a
small adapter. When Maestro can find a step in the native port but not in the Exact port, the
failure is diagnosed before it is counted as an accessibility difference. The journey is then
finished with `scripts/agent.mjs`. Every adapter, and every step diagnosed as not counted, is
logged in the app's `RESULTS.md` and reported beside the results, so the exemption is visible.
Maestro only drives the journey; the bench's probes (§5) take the timings.

### 4.1 The brief

One brief for every arm, in euxy's shape (`~/projects/euxy-compare/BRIEF.md`). It is written
from the original only, by someone who will not build the Exact arm, and frozen before either
arm starts. A change of scope is a new brief with a new id.

It contains:

- **The source.** Where it is, and what to read first.
- **Tiers.** In priority order, each finished before the next starts. They are the slice from
  §4.0.
- **Scope.** What is out of it.
- **Parity.** Match structure, controls, hierarchy, copy, colours and behaviour; use the
  platform's controls where the source does; no sub-point pixel chasing.
- **Done means.** A Release build installed, and each claimed tier driven and pictured.
- **Ground rules.** No subagents, so the tokens are the builder's own. One assigned
  simulator. Fix loops get three rounds.
- **The diary.** Sections Rough, Lean in, Needed and Checkpoints. Every Rough entry has the
  minutes it cost and a category: `framework gap`, `framework bug`, `docs`, `tooling`,
  `platform` or `own mistake`.
- **The final report.**

The brief says what to build and never how. Any instruction to one arm (for example, to do
its best) is written the same for both.

### 4.2 The arms

- **The Exact port**, always: an app outside the repository (`exact new`), consuming an
  exact2 worktree pinned by path at the commit under test.
- **A native port**, in the platform's own toolkit: SwiftUI or UIKit for iOS, Compose for
  Android. It is the **authoring comparator**: it shows what a builder gets in the same time,
  and the cost comparison needs a builder on both sides. It is the runtime reference only
  when the original is not native on that platform and the native port beats the original
  there (§2).
- **An Expo or React port**, on a stratified subset where Charlie wants Exact placed against
  that stack's authoring (§10). An Expo or React original is runtime evidence for its stack,
  not an authoring baseline.

What is pinned per app, and recorded with every run:

- the model and its effort;
- the CLI and its version;
- the docs exact2 ships at the commit under test;
- dependency versions and the cache policy.

The arms start together, each on its own simulator and in its own directory, which the other
cannot read. Each session is fresh and has seen no earlier solution. Across apps the model
rotates among families (Claude, GPT, Muse), because Exact must serve every builder. An app
keeps its pinned model in every later port, so a change in the numbers is Exact's and not
the model's (LLP 1116 §R).

### 4.3 Measure and grade

- **The bench (§5)** runs on the original and every arm, on the quiet machine.
- **Coverage** is checked independently, by driving each tier of the slice.
- **The blind grader** is not the builders' model. It receives:
  - the original and §4.0's classification, as the reference;
  - the two ports as A and B, in random order, built with no development affordances
    (`EXACT_DEV_MENU=0`, no driver overlays).

  It drives each port through the journey and the screens, and lists each difference from the
  original a user would notice, each tell and each bug. It scores closeness to the original
  and platform feel (LLP 1116's rubric). After scoring, it says which arm it took to be Exact.
  Over an app's gradings, that guess rate is tested against chance. When it is above chance,
  scores are reported split by recognised and unrecognised pairs, and a closeness win on a
  recognised pair is not counted.
- **Calibration pairs** are graded in every round beside the real pair: the original against
  itself, and the original against a copy with one deliberate difference a user would notice
  (a wrong label, a missing control, a platform tell). A grader that misses the planted
  difference, or finds one between identical builds, sets the sensitivity floor for that
  round.
- **The grader's noise** is measured continuously:
  - 10–20% of pairs, chosen at random, are regraded in reversed order;
  - the double grade is repeated whenever the rubric changes;
  - a second, independent grader confirms any graduation and any disputed finding.

  A closeness difference counts only when it exceeds both the measured regrade spread and the
  round's calibration floor.

### 4.4 Give every loss an owner

Every Rough entry, every grader finding and every bar missed gets one owner:

- **Exact:** a default, a gap, a bug, the docs, the tooling or the driver;
- **the builder;**
- **the platform;**
- **the original;**
- **the method.**

Before any work starts, an Exact-owned item is reproduced in exact2 itself, by a test, an app
under `apps/` or a drive. An item that does not reproduce stays recorded with its evidence, as
unreproduced, and is retried on the next port. It is not dropped.

### 4.5 Fix

The app's sub-LLP decides what to change, in LLP 1116's shape: decisions in order of expected
effect, each landing as its own commit. The rules:

- **Fix the class, and show it.** Each change names a second consumer: an app under `apps/`,
  a bake-off spec, or another port in this corpus. The change is probed there before the next
  new app starts. A plausible consumer not yet built may stand in, flagged as unbuilt, and the
  probe then waits for the next app on that matrix row. A change with no consumer at all
  stays in the port's own module or in `QUEUE.md`. LLP 1125 is the model: it shaped a sequencer's scheduler so
  that "a drum machine, a metronome, a rhythm game, an interval timer" use it unchanged.
- **Name what each fix should move.** Each fix states the measure it should change (a §2 row,
  or a bake-off failure class). A runtime win is never reported as an authoring win.
- **Remove a footgun in code first.** Only what code cannot remove goes to
  `docs/agent-pitfalls.md` or a recipe.
- **Measure speed and memory work before and after,** on one machine, with the port, and put
  the difference in the commit, as the euxy lanes did ("port 21 … medians of 12").
- **Record what is found and not done** in `QUEUE.md`, citing "<app> port N".

### 4.6 Replay and port again

There are two kinds of re-run, each numbered (port N) and added to the app's results table:

- **Replay.** The last Exact port's source, unchanged, rebuilt on the new main and benched
  beside the frozen original and native builds. It measures Exact's defaults and runtime with
  the builder held fixed (LLP 1116.019's replay gate). It is cheap; run it after each batch of
  runtime fixes.
- **Fresh port.** The same frozen brief and pinned model, on the new main, from an empty
  directory. It measures what builders now trip on. Run it when a batch of authoring fixes is
  the hypothesis, and only after the diary items it targets pass as extracted tasks (§4.8).

### 4.7 How an app ends

The stopping rules are declared before an app's first port:

- **Passed.** Every bar in §2 is met on one fresh port and confirmed by a replay. Its grading
  passed its calibration pairs (§4.3), and the second grader agrees.
- **Closed with named limits.** After at most four fresh ports, each bar still missed is named
  in the sub-LLP with its owner and reason (a platform limit, a decision not to support
  something, work queued), and Charlie accepts the list. Memory may close this way only within
  1.4× of the reference (§10.1).
- **Inconclusive.** The measures could not separate the arms within the run budget.

Only *passed* counts toward a pass rate. An app also stops early when two successive fix
batches move none of its measures.

Graduated and closed ports are kept, and their replays form a regression corpus
(§10.2).

### 4.8 Claims about authoring cost

A single port's wall clock and tokens are discovery evidence. They show where a builder spent
its time, not that Exact is cheaper. Euxy's first pair shows why. The Exact arm reused 3,182
lines of the original's TypeScript, the SwiftUI arm reused none, and their coverage differed.

A claim that Exact got cheaper or quicker to build needs one of:

- **Extracted tasks.** A diary's expensive item, turned into a task and run at least five
  times per arm on the authoring bench (LLP 1087).
- **Independent pairs.** Three to five pairs of the same brief on the same pinned versions,
  crossing at least two model families. The result is reported as a paired distribution, with
  failures and timeouts kept. Three to five pairs is a pilot; further sample sizes come from
  the variance it shows.
- **The bake-off's paired intervals** (LLP 1116), for a change that is not specific to ports.

Each authoring ratio states that the comparison stack is one builders already know (LLP 1116
§0).

### 4.9 A follow-up task

After an app passes or closes, one follow-up task runs on the finished port and on the native
port: a feature the original added later, or a bug it fixed. Building the first version shows
little of whether an app stays quick to change. The task's cost and correctness are recorded
like a port's.

## 5. The bench

The same instruments run on every arm. Timing never runs on the dev Mac, whose load makes it
meaningless. It runs on bones (the quiet M4 Mac mini), and each run records its load average.
A builder's wall clock is discarded above a stated load (load average above the machine's
core count) and the run repeated.

| Platform | Launch | While running | Memory, CPU, size |
|---|---|---|---|
| iOS | euxy's `bench/probe.m`, a dylib injected with `SIMCTL_CHILD_DYLD_INSERT_LIBRARIES`, logging each display refresh (and optional pictures) from process start, in any app unchanged; `bench.sh` interleaves the launches; `analyze.py` finds the content-shown and usable milestones. Exact also prints its phases (`SIMCTL_CHILD_EXACT_SMOKE=1`). 20 interleaved cold launches | The probe's per-refresh log, with input timestamps, over 20 journeys in several sessions; on a device, Instruments' Animation Hitches (`xctrace`) | `heap -s` and `footprint` at fixed points, medians over rotated launches; `ps` over 20 s, medians of 5; the `.app` and `.ipa` size |
| Android | [Macrobenchmark](https://developer.android.com/topic/performance/benchmarking/macrobenchmark-overview)'s `StartupTimingMetric`, which measures any installed package by name | `FrameTimingMetric` over the journey | `dumpsys meminfo`, `dumpsys gfxinfo`; APK size |
| Web | A Chrome trace through Playwright: FCP, LCP | INP per journey step | Heap after the journey; JS and CSS bytes |
| macOS | Process start to the first window frame | The journey's steps | Footprint; bundle size |

The probe's own overhead is measured once per OS version, on an app with and without it.
Simulator rankings are checked on a physical iPhone before any device-performance claim.

Every recorded number carries its machine, OS, build type, n and spread. When any of the
machine, OS version or fixtures changes, the frozen original and comparator builds are
re-benched beside Exact; numbers from different setups are not compared.

Tokens come from each builder's transcript: `bench/tokens.py` for Claude, and each other
CLI's own usage record. Dollars come from the provider's billing at the run's prices.

Android runs on minisforum's three Pixels, never the Nothing phones. The iOS simulator
serves every run; a device run is added for energy and touch latency.

The harness moves from `~/projects/euxy-compare/bench/` to `~/ports/bench/`, shared by every
app and versioned. Whether it enters the repository is §10's second question.

## 6. The portfolio

LLP 1141.000 lists the candidates, each tagged with the mechanisms it exercises. The
coverage matrix's rows are mechanisms, not product categories, because a fix is made to a
mechanism:

| Group | Rows |
|---|---|
| Structure | many nodes on one screen; long and virtualized lists; nested navigation, tabs, sheets and popovers; adaptive layout (split view, tablet, desktop window) |
| Input | text editing and the keyboard (IME included); forms and validation; gestures (swipe, drag, pinch); hardware keyboard and pointer |
| Content | rich text and Markdown; images and grids; media clocks (audio, video, background playback); canvas and custom drawing; charts; maps |
| Data | persistence and relaunch; network, offline and error recovery; sync and realtime; large data sets |
| System | notifications; share and extensions; widgets; permissions (camera, location); background work and resume |
| Quality | accessibility (VoiceOver, Dynamic Type, contrast); localization and RTL; dark mode; motion and haptics |

Each row is crossed with the platform measured, the original's stack (native, Expo or React
Native, React web), data scale and interaction intensity, so a row covered only by small
native iOS apps is visibly thin.

**The pilot** is three apps before the program scales, chosen in §10.6 from LLP 1141.000.
The pilot includes:

- one native original;
- one Expo or React Native original;
- one deep app, if the other two are small.

One candidate is held back as a transfer check after the pilot's fixes. It is quarantined:
no one tuning Exact, writing its docs or classifying another app reads or runs it until the
check. If it is touched, the check is void and another candidate takes its place.

Before the program scales past the pilot, one pilot app is ported again by a second model
family on the same frozen brief, both arms, so that coverage is not one family's. From then on
the family is a matrix axis like the platform and the original's stack.

The pilot measures its own costs, separately: baseline preparation, grading, fixes,
maintenance, and the time Charlie spends on rulings (named limits, classification disputes).

## 7. Records

Each app has a directory, `~/ports/<app>/`:

| Path | Holds |
|---|---|
| `SOURCE.md` | The repository, commit and licence; the original's defects found |
| `BRIEF.md` | The frozen brief (§4.1), with its id |
| `CLASSIFY.md` | Design, platform and defects (§4.0) |
| `JOURNEY.md`, `flows/` | The journey, in prose and as Maestro flows |
| `original/` | The original's builds, screens and bench runs |
| `runs/<N>-<arm>/` | Each port: its source, diary, transcript, final report and pinned versions |
| `RESULTS.md` | One table, one row per run, every §2 measure |

In exact2, the app's sub-LLP (`llp/1141.NNN-<app>.rfc.md`) records:

- the results summary;
- the owner table (§4.4);
- the decisions, each with its as-built note and the measure it moved.

`QUEUE.md` entries cite "<app> port N". Nothing under `~/ports/` is published (§8).

## 8. Rules

- **Licence.** Every original is read, built and run privately. A port that follows
  copyleft source closely may be a derivative work, so no port, and none of an original's
  names, icons or assets, is published or shipped. A port shown outside Charlie's machines
  uses a permissively licensed original and its own assets, and Charlie decides each time.
- **Services.** Public APIs are used at their documented limits, with a test account made
  for the purpose. A volunteer-run server, such as a Mastodon or Lemmy instance, is never the
  backend: a local instance in a container, or recorded fixtures, is. No real person's
  account or data is used.
- **Fairness.** The arms share the frozen brief, the pinned model, the machine class and the
  start time. Each uses its own stack's current documented setup. Neither gets a hint the
  other does not.
- **Honesty.** A feature that compiles but was never driven is "not verified". A loss is
  reported as a loss. An authoring ratio carries LLP 1116 §0's caveat that the comparison
  stack is one builders already know, and §2's note on reuse.
- **Upstream.** A bug found in an original is noted in its `SOURCE.md` and not filed unless
  Charlie says so.

## 9. Cadence and cost

An app's first port takes about a working day, five to nine hours elapsed, before fixes:

- the original's slice, build, baseline and classification, two to three hours;
- the arms, one to three hours, in parallel;
- the bench, an hour;
- grading, one to two hours;
- attribution, one to two hours.

Its fixes take days. The euxy loop ran 22 ports in two weeks; under §4.7 an app has at most
four fresh ports, and replays in between.

Budgets are kept in dollars and in the token categories that set them. Fresh input and output
cost far more per token than cache reads, so a token total is not a cost. Euxy's Exact arm
took 0.72 M fresh input and 0.23 M output against 103.5 M cache reads, and its largest context
was 735 k tokens. Each run records its billed dollars, and the pilot sets the per-app budget
from what its three apps cost.

The pace: two lanes in flight, alternating a new app with a re-port, so the fixes from one
app are measured before the next starts. Ports take at most half the lanes measuring Exact;
the bake-off keeps the rest, since only its paired intervals support authoring-cost claims
(§4.8). After the pilot, a lane pauses when two successive fix batches move no measure in any
app.

An original that stops building with current toolchains is re-pinned to a newer commit. Its
baseline is then taken again, and results before and after the re-pin are not compared.

## 10. Rulings (Charlie, 2026-10-23)

Charlie answered §10's six questions on 2026-10-23, leaving four of them to the author. The
author's choices are marked as such.

1. **The bars.** "1.2x is ballpark right but i could also be ok w 1.3 or 1.4"; the rest
   left to the author. *Author's choice:* the bars stand as §2 gives them, memory at 1.2×.
   A memory miss up to 1.4× may close as a named limit (§4.7). Beyond 1.4×, an app cannot
   close: it stays open until a fix lands.
2. **Apparatus: yes.** The versioned bench moves into exact2 as `scripts/ports/` once two
   apps have run it without app-specific edits. Replays of passed and closed ports run on a
   schedule on bones, outside the blocking checks.
3. **The native arm: as recommended.** It runs on an app's first port, on the port that
   would pass it, and on any pair used for an authoring claim (§4.8). Its frozen build serves
   the replays, and it is rebuilt when the model or the OS major version changes.
4. **An Expo or React arm:** left to the author. *Author's choice:* none in the pilot. The
   bake-off already compares Expo and React on small specs, and the pilot's Expo original
   (§6) gives Expo's runtime evidence. This is revisited after the pilot, for one native
   original, if the bake-off stops running Expo.
5. **The journey driver:** left to the author. *Author's choice:* Maestro, as §4.0 says,
   once a flow finishes on euxy's current Exact and UIKit builds. If it cannot finish one,
   each arm's own driver runs the prose journey: `scripts/agent.mjs` for Exact, XCUITest for
   iOS and Macrobenchmark's `UiAutomator` for Android.
6. **The pilot:** left to the author. *Author's choice:* the four apps of LLP 1141.000 §4:
   - the React Conf app (Expo original);
   - Hackers (native original);
   - Pocket Casts' podcast page and player (deep app);
   - NetNewsWire, held back for the transfer check.
