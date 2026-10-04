# LLP 1087: The authoring bench — measure building an app with exact2, then make it cheaper

**Type:** Plan
**Status:** Draft r2, 2026-10-04. r2 records Charlie's answers to r1's questions (§11) and adds comparison against other frameworks (§9). Nothing built yet.
**Systems:** None in exact2 until Phase 1 (the bench lives in its own repository, §2); then the authoring diary (`docs/diary.md`, the `exact new` AGENTS.md block in `game/new.mjs`), the generated `exact.mjs` command log (`.exact/commands.jsonl`), `scripts/feedback.mjs`
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-04
**Related:** LLP 1086 (what an app author is given; the dice-tray trial that motivated it); `docs/diary.md` (the authoring diary this reuses); LLP 1053 (gaps the list benchmarks found, the precedent for "a bench finds gaps, an RFC disposes them"); `~/.tuft/projects/web-framework-bench` (layout and conventions to copy); `game/diaries/001-beacons-exact*.md` (five builder runs of one brief, the closest thing to a bench run so far); `skills/orchestrate` (fix lanes, blind reviews)

## Summary

Charlie asked, 2026-10-04, for a benchmark he can use to optimize authoring. It runs
an agent through building an app with exact2 and records four things:

1. **Wall-clock time**, from the brief to "done".
2. **Tokens**, and what they cost.
3. **Fidelity**: did the built app match what the author asked for, and how buggy is it.
4. **Experience**: how good building it was from the authoring agent's side. This is
   a holistic score out of 100 plus sub-scores out of 100. A grading agent produces it
   from a very detailed diary.

The bench must run across agent harnesses, models, target platforms and machines.
It should run in a loop that finds problems in the authoring experience, fixes them,
and shows by re-running that time, tokens and gotchas went down.

The plan:

- **The bench is a separate repository.** It holds tasks, a runner, graders and
  results, like `web-framework-bench`. exact2 gains no apparatus from it except one
  detail level for the diary (§6.2), which Charlie approved (§11).
- **A trial** is one agent building one task, starting from `exact new` in a clean
  directory, with one harness, model, platform set and machine, at a pinned exact2
  commit. It produces a run record (§4) plus the diary, transcript, command log and
  built app.
- **Time and tokens are measured, not self-reported.** They come from the harness's
  event stream and `exact.mjs`'s command log. Each is attributed to a phase and a
  cause: docs reading, build waits, error loops, and diary writing.
- **Fidelity is graded in three layers.** Hidden scripted checks come first, then a
  judge agent that drives the built app against the spec on each platform, then a
  parity diff across platforms.
- **Experience is graded by a panel of agents** reading the diary *and* the
  transcript, against an anchored rubric. Claude, Astra and Grok each grade blind.
  The bench reports the median, the disagreement, and whether any family favours its
  own builds.
- **Builders vary too.** Opus, Sonnet, GPT, Grok and Gemini all author, because docs
  that only work for one model are bad docs.
- **The loop** runs a batch, clusters the findings across trials, and ranks them by
  minutes and tokens lost × frequency. It fixes the top ones in exact2 through the
  usual lanes and re-runs the affected tasks paired against the baseline. A fix lands
  only if the gain is outside noise and fidelity didn't regress. Held-out tasks guard
  against tuning to the bench.
- **Other frameworks** get the same tasks, specs and graders on a slower cadence (§9).
  The set is UIKit, SwiftUI, Android Views, Jetpack Compose, React, Expo, React Native
  without Expo, Flutter, Vue, Svelte, Solid, plain HTML+CSS+JS, KMP / Compose
  Multiplatform, and a few others. Every comparison is reported per platform, and also
  as the cost of covering all the platforms a task targets.

## 1. Words

- **Task**: a spec for one app, or a change to an existing app, plus its hidden
  grading material.
- **Harness**: the agent product driving the model: Claude Code, Codex CLI, Grok CLI,
  Cursor's agent, Tuft, Gemini CLI. ("Hosts" in the original ask was an agent's word.
  In exact2, "host" means a platform host, so this document says *harness* for the
  agent and *machine* for the box it runs on.)
- **Stack**: what the app is built with. `exact2`, or one of the comparators in §9.
- **Platform**: web, iOS, macOS, Linux (later Android, tvOS, Windows).
- **Cell**: one task × stack × harness × model × platform set × machine × exact2 commit (exact2 cells only).
- **Trial**: one run of a cell. Each cell runs N ≥ 3 times, because a single agent run
  is noise.
- **Batch**: a set of trials run against one exact2 commit, to be compared with
  another batch.

## 2. Where it lives

The bench gets its own repository, `~/.tuft/projects/authoring-bench`, for three reasons:

- RULES §Agents: agents add no apparatus to exact2 without a human saying so. A
  runner, graders and result stores are apparatus.
- The bench must test exact2 the way an outsider meets it: through `exact new`, the
  generated `AGENTS.md`, and the docs. If the bench lived inside the checkout, every
  trial would be able to see it.
- Results accumulate. They belong next to the tasks, not in exact2's history.

```
authoring-bench/
  tasks/<id>/            BRIEF.md (what the agent sees), SPEC.md (the author's full intent),
                         checks/ (hidden scripted checks), reference/ (screenshots, optional),
                         task.json (tier, platforms, budget, holdout flag)
  runner/                run.mjs (one trial), batch.mjs (a matrix), harnesses/<name>.mjs
  graders/               fidelity/ (scripted + judge prompts), experience/ (rubric + prompt)
  analysis/              aggregate.mjs, compare.mjs (paired A/B), triage prompt
  results/<batch>/<trial>/   record.json, diary.md, transcript.jsonl, commands.jsonl,
                             app/ (the built source), shots/, grades/
  site/                  a static report: matrix, trends, findings
```

## 3. Tasks

Tiers are chosen so that each exercises a different stretch of the path. Start with
six tasks (one per tier) and grow toward 20. Each tier gets one *variant* that is held
out (§8).

| Tier | Example | Exercises |
|---|---|---|
| T1 tiny | dice tray, counter with history | install, `exact new`, first build, dev loop |
| T2 data | todo list with edit, filter and persistence | `app.ts` data module, durable state, lists |
| T3 navigation | multi-screen app over a stand-in API (transit times, a feed) | routing, async data, loading and error states |
| T4 visual match | one screen of a real app against reference shots (Signal's chat list; the shots in `signal-clone-shots/`) | CSS fidelity, native controls, fonts, the web-as-standard rule |
| T5 capability | needs camera, notifications or haptics | the diary's Needed section; native modules |
| T6 change | add a feature to an existing mid-size exact2 app the agent didn't write | reading unfamiliar Contract; the most common real job |

What each task carries:

- **`BRIEF.md`**: what a person would actually type, from 2 to 15 lines. It is
  deliberately underspecified in places a person would be.
- **`SPEC.md`**: the author's full intent. It is a numbered list of requirements,
  each with a weight and a "how you'd tell", plus explicit non-goals. The agent never
  sees it. Only the graders, and the optional author persona (§5.4), see it.
- **`checks/`**: hidden scripted checks that run through platform drivers (§5.1),
  so the same checks grade every stack. To make them possible without leaking the spec, the brief names a
  small set of required element ids, as `web-framework-bench`'s SPEC data-testids do.
  Only checks needing a stable handle use ids; everything else falls to the judge.
- **`task.json`**: tier, target platforms, a time and token ceiling (the trial is cut
  off at the ceiling and graded as-is), and the holdout flag.

## 4. A trial, and its record

### 4.1 Running one

`runner/run.mjs <task> --harness claude-code --model claude-opus-5-5 --platforms web,ios --exact2 <sha>`:

1. Make a clean scratch directory on the machine. Pin exact2 at `<sha>` in a dedicated
   worktree, never the live main (the web-framework-bench convention). Point
   `EXACT2` at it.
2. Run `exact new <app>` the way a user would. Set the diary to bench detail (§6.2)
   and set `feedback` to `never`-send, so nothing leaves the machine. The diary stays
   on.
3. Start the harness headless with the brief as its prompt and a fixed preamble ("you
   are building this for someone; when you're done, say DONE and stop"). Use
   `claude -p … --output-format stream-json` for Claude Code and `codex exec --json`
   for Codex. Each `harnesses/<name>.mjs` adapter knows how to start its harness,
   stream its events to `transcript.jsonl`, and pull token usage out of them. A
   harness that can't run headless isn't benched until it can.
4. Stop at DONE, at the ceiling, or at 10 minutes with no tool calls. Record why it
   stopped.
5. Snapshot the app source, the diary, `.exact/commands.jsonl` and the transcript,
   then run the graders (§5, §6) in fresh contexts. Graders never share a context with
   the builder.

Builders vary as a first-class axis. The nightly matrix includes at least:

- Claude Code with Opus 5.5 (the baseline cell), and with Sonnet 5
- Codex with `gpt-6-astra`
- Grok CLI with Grok 4.7
- one more family when its harness runs headless (Gemini CLI)

The weekly batch adds Fable 5.1, Haiku 4.5 and reasoning-effort variants. A cheap
model that can finish a task is the strongest test of the docs.

Trials in a batch run in parallel on any fleet machine that is free. A machine runs one iOS trial at a
time, since simulators and Xcode builds fight. Machines are probed for load before
dispatch, as `skills/orchestrate` already does.

### 4.2 The record

`record.json`, one per trial:

```json
{
  "trial": "2026-10-05T03:12Z-t2-todo-cc-opus55-web+ios-mac03-r2",
  "cell": { "task": "t2-todo", "harness": "claude-code@2.x", "model": "claude-opus-5-5",
            "platforms": ["web","ios"], "machine": "mac03", "exact2": "3394b5292" },
  "stop": "done",
  "time": {
    "wall_s": 1712,
    "milestones_s": { "first_contract_build_clean": 210, "first_web_render": 260,
                      "first_ios_launch": 905, "done": 1712 },
    "by_cause_s": { "model": 640, "tool_exec": 1072, "build": 780, "docs_reading": 95,
                    "error_loops": 410, "diary": 120 }
  },
  "tokens": {
    "input": 0, "output": 0, "cache_read": 0, "cache_write": 0, "usd": 0,
    "by_cause": { "docs": 0, "exact2_source": 0, "build_output": 0, "own_code": 0, "diary": 0 }
  },
  "fidelity": { "score": 0, "by_platform": { "web": 0, "ios": 0 }, "requirements": [], "bugs": [] },
  "experience": { "overall": 0, "overall_other_family": 0, "sub": {},
                  "by_grader": { "claude-opus-5-5": {}, "gpt-6-astra": {}, "grok-4.7": {} }, "spread": 0 },
  "findings": [ { "id": "", "category": "", "minutes_lost": 0, "tokens_lost": 0, "evidence": "" } ]
}
```

### 4.3 Where time and tokens come from

- **Wall clock** comes from the runner's own timestamps. **Milestones** are the first
  success of each step, recovered from `commands.jsonl` (which already logs each
  `exact.mjs` command's exit code and duration) and the transcript.
- **Time by cause** joins transcript tool calls with the command log:
  - `build`: the time inside `exact.mjs` build, run and test commands.
  - `error_loops`: the time between a failing command and the next success of the same
    command.
  - `docs_reading`: the time spent on tool calls that read files under exact2's `docs/`.
  - `diary`: the time spent writing `.exact/diary/`.
  - `model`: what's left, i.e. time spent generating.
- **Tokens** come from the harness's usage events. They are attributed to a cause by
  what each tool result contained: a read of `docs/` counts as `docs`; a read of
  exact2's source (`kernel/`, `contract/`…) counts as `exact2_source`, which is a
  docs-gap signal; build and compiler output counts as `build_output`, which is a
  verbosity signal. USD uses each model's list price at batch time.
- **The diary's overhead is reported separately and subtracted** from the headline
  time and token numbers. A very detailed diary is how the experience score is fed,
  and it must not make every other number look worse. Note that it still perturbs the
  run: an agent writing notes thinks differently. §8 covers that.

## 5. Fidelity

Fidelity is scored per platform, 0–100, in three layers. The trial's fidelity is the
mean over its target platforms, with the per-platform numbers kept.

### 5.1 Scripted checks (deterministic)

Checks drive the app through the platform, never through exact2's own tooling, so the
same check grades every stack (§9):

- **Web**: Playwright.
- **iOS and macOS**: the accessibility tree, through XCUITest or idb.
- **Android**: uiautomator over adb.
- **Linux**: AT-SPI.

They find elements by the accessibility identifiers the brief asks for. `data-testid`,
`accessibilityIdentifier`, `testTag` and `Semantics` all reach the platform's tree.
exact2 trials are graded by the same drivers. `exact.mjs agent` stays in the builder's
hands, where its worth shows up in the experience score. Each check maps to a
requirement in SPEC.md. This layer is cheap, repeatable, and
catches outright breakage. It covers maybe a third of a spec.

### 5.2 The judge (an agent with the app and the spec)

A fresh agent gets SPEC.md, the built app (running), the same platform drivers (plus
screenshots) on each platform, and a budget: 15 minutes and a token cap. It does three things:

- **Grades each requirement** `met` / `partial` / `missing`, with the screenshot or
  tree excerpt that shows it.
- **Hunts bugs**: crashes, wrong state after a sequence, broken layout at another
  viewport size, dark mode, a keyboard covering input, an empty state that was never
  handled. Each bug gets a severity: crash, major, minor or cosmetic.
- **Compares against `reference/`** for T4, judging the screenshots side by side.

The judge does not see the builder's diary or transcript, so its view of the app isn't
coloured by the builder's account of it.

Score per platform = 100 × Σ(weight × {1, 0.5, 0}) / Σ weight, minus 15 per crash,
6 per major, 2 per minor and 0.5 per cosmetic, floored at 0. Scripted checks override
the judge where both cover a requirement.

### 5.3 Parity

The same drive script runs on every target platform. The bench diffs the accessibility
trees and layout boxes, and compares screenshots perceptually. A disagreement is a
`parity` finding: an exact2 bug, not the author's. It is reported separately and does
not lower the trial's fidelity.

### 5.4 Optional: the author persona (Phase 4)

Real authoring is a conversation. An author agent holds SPEC.md, and the builder can
ask it questions through the harness, getting the answers a person would give. This
measures how the builder handles ambiguity, and how often the docs, rather than the
person, should have answered. It stays off until the single-shot bench is stable,
because it adds variance.

## 6. Experience

### 6.1 Sub-scores

Each is 0–100 with anchored levels (90: "never noticed it"; 70: "small friction,
fixed in minutes"; 50: "cost a real detour"; 30: "needed a workaround or source
reading"; 10: "blocked or gave up"). The grader writes a sentence of evidence for each
score, citing the diary line or transcript turn.

| Sub-score | What it covers |
|---|---|
| Setup | install, `setup --check`, `exact new`, first build |
| Orientation | finding the right doc; the generated AGENTS.md; knowing what to read next |
| Docs accuracy | did the docs say what's true; missing or wrong examples |
| Language ergonomics | writing Contract and `app.ts`; how often intent fit the language |
| Diagnostics | compiler and runtime errors: did each one say what to do |
| Dev loop | edit-to-seen speed; reload reliability; stale-build surprises |
| Platform builds | iOS/macOS/Linux builds: cold time, signing, simulators, failures |
| Verification | `exact.mjs agent` and `test`: could it see and prove the app works |
| Capability coverage | the Needed section: what was provided, built by hand, or missing |
| Predictability | surprises; behaviour that contradicted the docs or CSS |
| Confidence at done | did it finish sure the app worked, and was it right |

**Overall** is graded as its own holistic judgement against its own anchors, not as an
average. The average is reported beside it, and a large gap between the two is worth
reading.

### 6.2 The diary at bench detail

`docs/diary.md` already asks for Rough, Lean in, Needed and Checkpoints. The bench
asks for more, behind a switch so real users don't pay for it. When
`EXACT_DIARY=detailed` (set by the runner; the AGENTS.md block says what it means) the
agent also keeps:

- **A timeline**: one line per step, with a timestamp taken from `date`, never
  estimated. The beacons r5 diary shows why: its builder estimated a start time and
  then had to retract it.
- **For every error**: the command, the first lines of the error, what was believed,
  what was tried, which attempt worked, and the minutes and tokens it took as best
  known.
- **Every doc read, and why**: what was looked for, and whether it was there.
- **Every guess**: places where the docs didn't say, and the agent guessed. Each gets
  "right" or "wrong", found out later.
- **Every workaround** left in the app, and what it should have been.
- **A closing self-assessment**: what it would tell the next agent, and how sure it is
  that the app works on each platform.

This is the one change to exact2 the plan needs before Phase 1: a short section in
`docs/diary.md` and a line in the generated AGENTS.md block. It needs Charlie's yes
under RULES §Agents.

### 6.3 Grading

The experience grader is a fresh agent. It reads, in order: the diary, the
transcript (condensed: tool calls, their results cut to 40 lines, the agent's
messages), `commands.jsonl`, and the record's time and token breakdown.

It reads the transcript as well as the diary for two reasons:

- **Agents under-report.** A diary written mid-task forgets the third retry. The
  transcript doesn't forget.
- **Agents mis-attribute.** "The compiler is wrong" is sometimes the agent's own typo.
  The grader marks each Rough entry `confirmed`, `agent error` or `unclear`.
  `agent error` lowers no exact2 sub-score, but it becomes a finding if the docs could
  have prevented it.

It outputs the scores, the evidence, and a list of **findings**. Each finding is
`{category, title, minutes_lost, tokens_lost, evidence, suggested fix}`, and findings
are the loop's raw material.

A **panel** of graders scores each trial blind:

- Claude (Opus 5.5)
- Astra (`gpt-6-astra`, xhigh, via `codex exec`)
- Grok 4.7 (xhigh)
- Gemini, when available

Each grader is a separate context, and none sees another's scores. The headline is
the panel median. Beside it, the record keeps every grader's score and an
**other-family median**, which leaves out the grader from the builder's family. If a
family rates its own builds higher than the others do, the gap between the two
medians shows it, and the report tracks that gap per grader. The fidelity judge
(§5.2) rotates through the same families from trial to trial, so no builder is
always judged by its own family. The bench reports the mean and the spread. If the spread
across the panel is above 15 on the overall score, the trial is flagged, and the
triage agent reads it before trusting its findings. A
**calibration set** of 6 to 10 past diaries is re-graded whenever a grader prompt or
model changes. The set is signal, bluesky, dice-tray and beacons r1–r5. Charlie has no
time to hand-score them, so their reference scores are the two graders' reconciled
consensus across the panel: each grader sees the others' evidence once, then the
result is frozen. That
anchors *stability*, not *truth*. A human spot-check can replace it later. A grader change that moves the calibration
scores is a grader change, and must not be mistaken for exact2 getting better or worse.

## 7. The loop

```
batch(commit A) → grade → aggregate → triage → fix lanes → batch(commit B, affected tasks) → compare → land / revert
```

1. **Batch.** A nightly matrix on the fleet, sized to a budget: all tasks × the two or
   three main harness/model pairs × web and iOS × N=3. A weekly batch adds the other
   harnesses, platforms and machines.
2. **Aggregate.** Medians and IQRs per cell and per tier, trends over commits, and
   the four metrics side by side. There is no single composite. Charlie reads it as a
   Pareto picture; a change that saves tokens but loses fidelity is not a win.
3. **Triage.** An agent clusters the batch's findings across trials ("the iOS build
   needed `--update-lock`": 7 of 18 trials, about 9 min each). It dedupes them against
   open items in exact2's `QUEUE.md` and `docs/issues.md`, and ranks them by
   frequency × (minutes + tokens in minute-equivalents). It outputs a short ranked
   list. Each item says whether it is a **docs fix**, a **diagnostic fix**, a
   **tooling fix**, a **bug**, or a **feature**.
4. **Fix.** Docs, diagnostic and tooling fixes, and bugs with a clear reproduction,
   go to fix lanes as `skills/orchestrate` runs them, with Charlie's routing (2026-10-04):
   Opus 5.5 implements, Astra xhigh and Grok 4.7 xhigh review blind, and RULES'
   three-round limit holds. A fix that passes review and §7.5's comparison is pushed to
   origin/main without waiting for Charlie. **Features go to Charlie** as an
   RFC or a QUEUE line and are never auto-built. When a pitfall can't be fixed yet, it
   goes in `docs/agent-pitfalls.md`, whose own deletion rule removes it later.
5. **Compare** (§7.5). Re-run the tasks the finding came from, plus one task from every other
   tier, at the fix commit. Pair each against the baseline (same task, harness, model
   and machine) with N ≥ 3 each. A fix **lands** if the median improves on the metric
   it targeted, a bootstrap 90% interval on the paired difference excludes zero, and
   no task's fidelity median dropped by more than 5. Otherwise it is reverted or
   rethought, and the loop records that it didn't help.
6. **Report.** A daily digest to Slack: what the batch found, what landed, what moved,
   and what's waiting on Charlie.

### Stopping, and human checkpoints

- The loop never edits tasks, checks, graders or the rubric. Those change only by a
  human's hand, or with a human's yes, and each such change bumps a bench version.
  Batches on different bench versions aren't compared.
- A finding that comes back after its fix landed gets one more round, then escalates.
- Charlie reviews the ranked list weekly. Anything touching the language, the kernel
  or a platform host's design is his call.

## 8. Keeping the numbers honest

- **Variance.** Agent runs vary a lot. Every claim compares paired medians with an
  interval, never two single runs. Phase 1 measures the noise floor: the same cell 10
  times. That sets N.
- **Goodhart.** The fix lanes see the visible tasks' findings, never the held-out
  variants. Each tier keeps one holdout that is run in every batch but whose findings
  aren't fed to triage. If the visible tasks improve and the holdouts don't, the
  fixes are tuning to the bench. Tasks rotate each quarter.
- **Contamination.** A trial's scratch directory is fresh. The builder can read the
  pinned exact2 checkout (a real user can), but it can't see the bench repository,
  other trials, or past diaries.
- **Diary perturbation.** Once a week, a slice of trials runs with the diary off. They
  get time, tokens and fidelity only. The gap between them and the diary-on trials is
  the diary's true cost, and it is reported.
- **Harness drift.** Harness and model versions are recorded per trial. A harness
  upgrade starts a new baseline for its cells.
- **Cost.** The cap is $2000 a day across builders, graders and judges, enforced by the
  runner from live usage. It stops dispatching at the cap, and the report says the
  batch is partial. The default split:
  - 60% for the nightly exact2 batch
  - 25% for fix-lane comparison reruns
  - 15% held back, which accrues toward comparator runs (§9)

  Phase 0 measures the cost of a trial, and that sets N and the matrix size.

## 9. Comparators: the same apps in other stacks

### 9.1 The set

| Group | Stacks |
|---|---|
| Apple native | UIKit, SwiftUI (iOS; SwiftUI also for macOS tasks) |
| Android native | Android Views, Jetpack Compose |
| Cross-platform mobile | Expo, React Native without Expo, Flutter, KMP with Compose Multiplatform, .NET MAUI, Ionic/Capacitor, Lynx |
| Web | React (Vite), Vue, Svelte, Solid, Angular, plain HTML+CSS+JS |
| Desktop (macOS/Linux tasks) | Tauri, Electron, SwiftUI for macOS |

Each comparator gets a pinned toolchain and a one-line starter (`npx create-expo-app`,
`flutter create`, Xcode's template via `xcodegen`, `npm create vite`…), recorded per
batch. The starter is the one a person would use, with no bench-specific scaffolding.

### 9.2 What stays the same, and what can't

- **Same:** the brief, SPEC.md, checks, judge, rubric and grader panel. The same
  builder pairs are used, one per family, at the monthly cadence.
- **The diary:** comparator builders get a framework-neutral copy of the bench-detail
  diary. It has the same sections; its Checkpoints use generic step names (install,
  new project, dev loop, editing, each platform build, verifying, deploy). exact2
  trials keep `docs/diary.md`, which has the same shape. The experience rubric scores
  both alike.
- **Platform coverage differs.** UIKit covers iOS and nothing else. Results are
  therefore reported two ways:
  - **Per platform**: exact2's iOS fidelity, time and tokens beside SwiftUI's, UIKit's,
    Expo's, Flutter's…
  - **Cover the task**: for a web+iOS+Android task, exact2 once, against the cheapest
    native combination (SwiftUI + Compose + React, summed: separate trials, added up),
    against each cross-platform stack once.
- **The judge's bug hunt** uses the same budget per platform for every stack.
- **T6 (change an existing app)** needs the same starting app in each stack. It is
  built once per comparator by a bench trial, reviewed, frozen, and reused.

### 9.3 Cadence

Comparators run **monthly**, and on demand when Charlie asks. They use one main
harness/model pair per sweep, rotating family each month, the visible tasks only, and
N=3. A full sweep is roughly 20 stacks × 6 tasks × 3, around 360 trials. It is paid for from the 15% held back, and runs over
a weekend if the cost of one day exceeds the cap. Results go in the same report as a
separate page, with exact2's median from the same month beside them.

Comparator findings are about the other stacks. They feed exact2's triage only as
"Lean in" evidence: what another stack made easy that exact2 made hard, with the
minutes it cost.

## 10. Phases

**Phase 0: one trial, by hand (1–2 days).** One harness (Claude Code headless), web
only, tasks T1 and T2. `run.mjs`, the record, and time and tokens by cause. A person
scores fidelity and experience by hand, to produce the first calibration entries. Exit
when one trial runs end to end unattended and its numbers match the transcript.

**Phase 1: the graders (about a week).** The diary's bench detail lands in exact2
(§6.2, with Charlie's yes). Scripted checks and the fidelity judge for T1–T3. The
experience grader with two families, and the calibration set. A noise-floor run: one
cell × 10. Exit when the graders agree within the spread threshold on the calibration
set and N is chosen. The platform drivers (§5.1) are built here, since every later
phase leans on them.

**Phase 2: the matrix (about a week).** Codex and one more harness. iOS and macOS.
`batch.mjs` across the fleet, T4–T6, and the static report. The parity layer. Exit
when a nightly batch runs unattended within its budget.

**Phase 3: the loop (ongoing).** Triage, fix lanes, paired comparisons, the daily
digest, and holdouts. Exit criterion: none. Success is the headline medians going down
release over release while fidelity holds.

**Phase 3b: comparators.** The neutral drivers from Phase 1 already grade any stack.
Add the §9 starters, then the first monthly sweep. Web stacks go first (cheapest),
then Expo, SwiftUI and Compose, then the rest.

**Phase 4: extensions.** The author persona (§5.4), Linux and Android for exact2 when
the hosts exist, and Windows.

## 11. Charlie's answers to r1 (2026-10-04)

1. **"Hosts"** was an agent's word. This document says *harness* and *machine*.
2. **The diary's bench detail in exact2** (§6.2): yes.
3. **Budget and machines**: $2000 a day, on any machines that are free.
4. **Calibration**: no hand-scoring for now. Use the panel's frozen consensus (§6.3).
5. **Fix autonomy**: yes. Fixes that are well reviewed (Astra xhigh, Grok 4.7 xhigh)
   go to origin/main. Opus 5.5 authors.
6. **Comparators**: added as §9.
7. **Models**: author with a variety of models, and grade with a variety, Claude
   included (§4.1, §6.3).
