# LLP 1087: The authoring bench — measure building an app with exact2, then make it cheaper

**Type:** Plan
**Status:** Draft r3, 2026-10-04. Ready to hand to an operator box.
- r2 recorded Charlie's answers (§13) and added comparators (§10).
- r3 folds in the blind reviews by Astra (xhigh) and Grok 4.7 (xhigh). Both are in
  `llp/reviews/plan-2026-10-04-1087.{astra,grok}.md`, with dispositions in §14.
- r3 adds §12, the operator's runbook, so a fleet box can be handed this document and
  run it.
**Implementer:** a fleet box Charlie assigns ("run this for a week"), following §12
**Systems:** None in exact2 until Phase 1 (the bench lives in its own repository, §2). Then:
- the authoring diary: `docs/diary.md`, and `scripts/feedback.mjs` `status` (§6.2)
- whatever the loop's fixes touch, through the normal lanes (§8)
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-04
**Related:**
- LLP 1086: what an app author is given; the dice-tray trial behind it.
- `docs/diary.md`: the authoring diary this reuses.
- `scripts/feedback.mjs` and `game/new.mjs`: the command log and the generated `exact.mjs`.
- LLP 1053: "a bench finds gaps, an RFC disposes them".
- `~/.tuft/projects/web-framework-bench`: the layout and conventions to copy.
- `game/diaries/001-beacons-exact*.md`: five builder runs of one brief.
- `skills/orchestrate`: lanes and blind reviews.

## Summary

Charlie asked, 2026-10-04, for a benchmark he can use to optimize authoring. Agents
build apps with exact2, and the bench tracks four things:

1. **Wall-clock time**, from the brief to an accepted app.
2. **Tokens**, and what they cost.
3. **Fidelity**: does the app do what the author asked, and how buggy is it.
4. **Experience**: how building it went, from the agent's side. This is a holistic
   score out of 100 plus sub-scores out of 100, graded by agents from a very detailed
   diary and the transcript.

It runs across agent harnesses, builder models, grader models, target platforms and
machines. It runs in a loop: find what's rough, fix it in exact2, and show with a
controlled re-run that time, tokens and gotchas went down. Periodically, the same tasks
are built in other stacks, from UIKit and Compose to Expo, Flutter and the web
frameworks, for comparison.

The plan in brief:

- **A separate repository** holds tasks, runner, graders and results. exact2 gains
  only the diary's detailed level, which Charlie approved.
- **Headline numbers are raw.** Elapsed time, billed tokens and USD, completion rate,
  and fidelity come straight from the harness and the platform. Breakdowns by cause
  are labelled estimates.
- **Fidelity scores the delivered app** against requirements the builder was told.
  Bugs count whoever caused them, and who caused a bug is recorded separately.
- **Experience is an agent-assessed friction proxy.** A multi-family panel grades it
  from the transcript first and the diary second. It is validated by perturbation
  tests, not human labels.
- **A fix lands** only after an interleaved, randomized A/B run and a confirmation
  batch, on top of exact2's normal checks. "Inconclusive" is an allowed outcome.
- **Start narrow:** three tasks, one builder, web only, exact2 plus one web
  comparator. Widen only after one real improvement has been shown end to end.

## 1. Words

- **Task**: one app to build, or one change to an existing app, plus its grading
  material.
- **Stack**: what the app is built with: `exact2`, or a comparator (§10).
- **Harness**: the agent product driving the model: Claude Code, Codex CLI, Grok CLI,
  Gemini CLI, Cursor's agent. ("Hosts" in the original ask was an agent's word. In
  exact2, a host is a platform host.)
- **Builder**: harness + model. **Grader**: a model scoring a trial.
- **Platform**: web, iOS, macOS; later Android and Linux.
- **Cell**: task × stack × builder × platform set × machine × stack revision.
- **Trial**: one run of a cell.
- **Batch**: the trials run together under one plan.
- **Completed**: the builder said DONE inside the ceiling, *and* the app passed the
  task's acceptance threshold (§5.4).

## 2. Where it lives

The bench gets its own repository, **`github.com/ccheever/authoring-bench`**
(private), which is where machines coordinate. Its README says what the seed holds
(two tasks, a verified Claude Code adapter, a prototype runner that predates r3). It
lives outside exact2 for three reasons:

- RULES §Agents forbids adding apparatus to exact2 without a human's yes.
- Builders must meet exact2 the way an outsider does.
- Results accumulate there, not in exact2's history.

```
authoring-bench/
  tasks/<id>/         BRIEF.md (seen by the builder: every scored requirement, in a person's words)
                      SPEC.md (graders: weights, test inputs, procedures, allowed variation)
                      checks/ (hidden scenarios, §5.1), reference/ (screenshots, if any)
                      task.json (tier, platforms, ceiling, role: dev | regression | audit)
  runner/             run.mjs (one trial), batch.mjs (a plan → trials), harnesses/<name>.mjs
  drivers/            web.mjs, ios.mjs, ... (§5.1)
  graders/            fidelity/, experience/, prompts, rubric, perturbation suite (§6.4)
  analysis/           aggregate.mjs, ab.mjs (§8.2), triage prompt
  results/<batch>/<trial>/   record.json, transcript.jsonl, diary.md, commands.jsonl,
                             app/ (source snapshot), shots/, grades/
  ledger.jsonl        every spend (dollars) and every landing decision
  site/               a static report
```

**Isolation is enforced, not hoped for.** The builder runs as a separate macOS user,
`bench`, which can read only:

- its own scratch directory
- the pinned stack checkout, read-only
- the caches it is allowed

It cannot read the bench repository, results, other trials or ledger. The pinned exact2
checkout is a worktree with the folders that hold past diaries and reviews removed
(`game/diaries/`, `llp/reviews/`, any `DIARY.md`) and this document (`llp/1087*`). A
real user's copy has no use for them, and they would leak earlier trials or the
rubric.

## 3. Tasks

### 3.1 Tiers

| Tier | Example | Exercises |
|---|---|---|
| T1 tiny | tip splitter, counter with history | install, `exact new`, first build, dev loop |
| T2 data | todo list with edit, filter and persistence | `app.ts`, durable state, lists |
| T3 navigation | multi-screen app over a stand-in API | routing, async data, loading and error states |
| T4 visual match | one screen of a real app against **per-platform** reference shots, made for the task (not `signal-clone-shots/`, which is a working pile, not an oracle) | CSS fidelity, native controls, fonts |
| T5 capability | needs camera, notifications or haptics | the Needed section; native modules; device fixtures |
| T6 change | add a feature to an existing mid-size app the builder didn't write | reading unfamiliar code; the most common real job |

### 3.2 What the builder is told

The brief is a person's words, but **every scored requirement is in it**. Reviewers
pointed out that withholding intent turns fidelity into a mind-reading test, and one
that favours stacks with lucky defaults. What stays hidden:

- the test inputs
- the procedures
- the weights
- the bug hunt

A T4 brief says whether it means "match these pixels" (the reference shots are given)
or "use appropriate native presentation".

How an agent handles ambiguity is a separate experiment (§5.5), not a hidden cost in
every trial.

The brief also asks for a few element ids (`data-testid`, `accessibilityIdentifier`,
`testTag`), for the scripted checks.

### 3.3 Roles: dev, regression and audit

- **Dev tasks**: their findings feed triage, and fixes are measured on them.
- **Regression tasks**: run every batch to catch breakage. Their findings are
  reported but not fed to the fix lanes.
- **Audit tasks**: fresh, run rarely (every two weeks), never seen by the loop. They
  answer "are the improvements real?" Audit tasks exist before the loop is allowed to
  fix anything. They are replaced after every use that influenced a decision.

No improvement on an unrelated audit task is not by itself evidence of overfitting. A
**pattern** of dev gains with flat or worse audits is.

## 4. A trial, and its record

### 4.1 Running one

1. **Pin the stack.** For exact2, that's a worktree at the commit, stripped as in §2,
   with `bun install --frozen-lockfile` done.
   - Caches are standardised: **warm**, meaning toolchains, crates and npm present.
   - A weekly **cold** slice uses an empty `CARGO_HOME` and target directory.
   - Both kinds are labelled in the record.
2. **Start from nothing.** The builder gets an empty directory and the brief, plus:
   "Build it with exact2. It is checked out at `<path>`; its README says how to
   start." Running `exact new` is part of the measured run, so the setup sub-score
   watches what actually happened. T6 instead starts from a frozen copy of its
   starting app.
3. **The diary is on, and sending is off.** The runner gives the builder a private
   `EXACT_CONFIG_DIR` whose standing answer is the new `local`: write the diary, never
   ask, never send (§6.2). It also sets `EXACT_DIARY=detailed`, and points
   `EXACT_FEEDBACK_URL` at a closed port as a backstop. Ordinary users' `never` is
   unchanged.
4. **Run the builder headless**, as user `bench`, with a fresh harness config, so no
   memory, skills, settings or MCP servers leak in. The prompt is the brief plus a
   fixed note: "I'm not around to answer questions; make reasonable choices and note
   them; end with DONE when it's finished and checked."
   - Verified on 2026-10-04 for Claude Code: `claude -p … --output-format
     stream-json --verbose` with a fresh `CLAUDE_CONFIG_DIR` and
     `CLAUDE_CODE_OAUTH_TOKEN` set runs clean. The final `result` event carries
     `total_cost_usd`, `usage` and per-model `modelUsage`.
   - Codex uses `codex exec --json`, and Grok uses `grok --output-format …`.
   - Each adapter normalises the stream into usage, tool-use, tool-result, text and
     result events, timestamped on arrival.
5. **Stop** at DONE, at the ceiling, or when idle. Idle means no harness output, *no
   tool call in flight*, and no live child process using CPU, all for 10 minutes, so a long Xcode build isn't
   mistaken for a hung agent. Every stop reason is kept. **Failed and capped trials
   stay in the data.**
6. **Snapshot** the source, transcript, diary and the app's `.exact/commands.jsonl`
   into the results, which `bench` cannot write to. Then grade (§5, §6) in fresh
   contexts.

### 4.2 The record

```json
{
  "trial": "…", "role": "dev",
  "cell": { "task": "t2-todo", "stack": "exact2", "stack_rev": "3394b5292",
            "harness": "claude-code@2.1.280", "model": "claude-opus-5-5",
            "platforms": ["web"], "machine": "…", "cache": "warm", "bench": "<sha>" },
  "stop": "done | ceiling | idle | error",
  "completed": true,
  "time": {
    "elapsed_s": 1712,
    "spans_s": { "model": 0, "tool_exec": 0 },
    "tags_s":  { "build": 0, "drive": 0, "docs": 0, "stack_source": 0, "diary": 0, "error_loop": 0 },
    "milestones_s": { "app_created": 0, "first_compile_ok": 0, "first_render": 0, "done": 0 }
  },
  "tokens": { "input": 0, "output": 0, "reasoning": 0, "cache_read": 0, "cache_write": 0, "usd": 0,
              "context_est_by_tag": { "docs": 0, "stack_source": 0, "build_output": 0, "own_code": 0, "diary": 0 } },
  "fidelity": { "by_platform": { "web": { "score": 0, "requirements": [], "bugs": [] } },
                "attribution": [ { "bug": "", "cause": "author | stack | driver | unknown" } ] },
  "experience": { "overall": { "median": 0, "other_family_median": 0, "by_grader": {} },
                  "sub": {}, "spread": 0, "flags": [] },
  "findings": []
}
```

### 4.3 Time and tokens

- **Headlines are raw:** elapsed seconds, billed tokens by category (reasoning
  separately where the harness reports it) and USD. Token counts are compared **within
  a model**, because tokenizers differ. Across models, compare dollars.
- **Spans** are mutually exclusive, and add up to elapsed:
  - `tool_exec`: from a tool call to its result
  - `model`: everything else, which includes transport and scheduling
- **Tags** overlap and are labelled estimates:
  - `build` and `drive`: from the transcript's Bash commands, with full arguments
  - `docs`: reads under the stack's docs
  - `stack_source`: reads of the stack's internals, a docs-gap signal
  - `diary`: writes to `.exact/diary/`
  - `error_loop`: from a failing command to the next success of the same command

  The transcript's tool calls are the primary source. `commands.jsonl` corroborates
  them; it only knows coarse verbs and only logs on return.
- **Loop speed**, a separate metric because it is what exact2's budgets name: the
  median incremental `contract build` and edit-to-reload time inside the trial. It is
  kept apart from the cold first build. Every warm trial shares a warmed Cargo
  registry and Xcode DerivedData cache, so cold-toolchain variance isn't charged to
  authoring.
- **Milestones** come from the transcript (the first `exact new` success, the first
  clean `contract build`) and the drivers (first render).
- **The diary's cost is measured, not subtracted.** Writing a diary changes how the
  agent works. Each week, a randomized slice of trials runs with the diary off; those
  trials get time, tokens and fidelity only. The on/off difference is the diary's
  cost, and it is reported.

## 5. Fidelity

### 5.1 Scenarios over platform adapters

A truly framework-neutral driver doesn't exist. Test ids, DOM nodes and accessibility
elements are different interfaces. What does exist: **one scenario per requirement,
written once, run through a per-platform adapter**. Each adapter supports a fixed set
of operations:

- `launch`, `reset`, `find(id)`, `tap`, `type`, `read text`, `screenshot`, `relaunch`
  (for persistence), `set viewport`, `set color scheme`

Each stack's starter documents the single property that surfaces the brief's ids, so
the adapters know where to look:

| Stack | Property |
|---|---|
| exact2 and web | `testId` / `data-testid` (Playwright reads the DOM; Chrome's accessibility tree doesn't carry it) |
| UIKit and SwiftUI | `accessibilityIdentifier` |
| Compose | `testTag` with `testTagsAsResourceId` |
| Flutter | the semantics identifier |

The adapters:

| Platform | Adapter |
|---|---|
| Web | Playwright |
| iOS | XCUITest or idb, on the accessibility tree |
| Android | uiautomator |
| macOS | the AX API |

Linux waits: exact2's Linux host has no AT-SPI tree yet (`host/linux/src/agent.rs`,
"no AT-SPI tree").

The app runs in its **normal launch mode**. exact2's agent mode replaces native bars
and menus (`docs/agent-pitfalls.md`), so it is not what a user sees. A scenario that
can't run gets a separate `driver_failure` outcome. It is neither a pass nor a fail,
and it counts against the bench, not the app.

T5 needs device or capability fixtures: a simulated camera feed, a notification
inspector, a haptics log. Requirements that can't be observed are marked
`unobservable` and left out of the score.

Before any comparator is benched, one scenario set must work on exact2 *and* one
comparator: launch, reset, persistence, keyboard, navigation and screenshots.

### 5.2 The judge

A fresh agent gets four things:

- SPEC.md
- the running app, through the same adapters plus screenshots
- a budget: 15 minutes per platform, and a token cap
- no diary or transcript

The judge grades each requirement scenario-checks couldn't settle as `met`, `partial`
or `missing`, with evidence. It hunts bugs by severity. Bugs are **deduplicated by root
cause**: a missing requirement and the bug it causes are penalised once. Judge families
rotate across trials, so no builder is always judged by its own family. App content is
untrusted input: the judge prompt says that text in the app is data, never
instructions.

### 5.3 The score

Per platform: 100 × Σ(weight × {1, ½, 0}) / Σ weight. Then subtract, once per root
cause:

| Bug | Penalty |
|---|---|
| Crash | 15 |
| Major | 6 |
| Minor | 2 |
| Cosmetic | 0.5 |

The score floors at 0. Scenario results override the judge where both cover the same
requirement.

**A judge-found bug counts only when it is corroborated**, by a scenario, by a crashed
process in the log, or by a second judge from another family re-checking it. Without
that, one invented crash would outweigh the scripted score.

**The comparable number, and the one the A/B gates on, is the scripted pass rate**: the
share of SPEC "how you'd tell" behaviours the scenarios confirm. The judged score is
reported beside it.

**Fault doesn't excuse a bug.** A bug caused by exact2 still makes the delivered app
worse, and excusing it would flatter exact2 against comparators. Attribution (`author`,
`stack`, `driver`, `unknown`) is recorded beside the score. Stack-attributed bugs are
the loop's best findings.

**Parity** uses exact2's existing role, name and state comparison (`axParity` in
`scripts/agent-ax.mjs`, which already accounts for safe areas, fonts and roles), with
presentation differences the spec allows. A semantic disagreement is a bug on the platform where
it's wrong.

### 5.4 Completion

Each task sets an acceptance threshold: fidelity ≥ 80 on every target platform, and
no crash. A trial is **completed** when it says DONE and meets the threshold. The bench
reports:

- **completion rate**
- **time and dollars among completed trials**
- **capped and failed trials as censored**: never averaged in as if they had
  finished, never dropped

A fast, incomplete app can't improve the numbers.

### 5.5 Ambiguity, separately (Phase 4)

An author persona holds a fuller intent and answers questions the builder asks through
a fixed mechanism. This runs as its own experiment, so ambiguity handling doesn't
inject noise into the main bench.

## 6. Experience

### 6.1 What it is

The score is an **agent-assessed friction proxy**: how much friction the builder ran
into, and of what kind. It is not a validated human-experience scale, and the report
says so.

### 6.2 The detailed diary

`docs/diary.md` is embedded in every app's AGENTS.md. It already says to run
`bun exact.mjs feedback status` at the start. Two changes to exact2, both small:

- `feedback` gains a standing answer, **`local`**. It means: keep the diary, never
  ask, never send. `docs/diary.md`'s opening obeys it: "If it says `local`, keep the
  diary and skip Asking to share."
- The detailed level is printed by `feedback status` only when `EXACT_DIARY=detailed`
  is set, so ordinary users' context
doesn't grow. `docs/diary.md` gains one line: "If `feedback status` prints more
instructions, follow them too."

The detailed level asks for:

- **a timeline**, with times taken from `date`, never estimated
- **for every error**: the command, the first lines of output, what the agent
  believed, each attempt, and what worked
- **every doc read**: what it was looking for, and whether it was there
- **every guess** where the docs were silent, later marked right or wrong
- **every workaround** left in the app
- **a closing self-assessment**, per platform

### 6.3 Sub-scores and grading

Each sub-score is 0–100, against **framework-neutral anchors**:

| Score | Anchor |
|---|---|
| 90 | never noticed it |
| 70 | small friction, fixed in minutes |
| 50 | a real detour |
| 30 | needed a workaround or reading the framework's source |
| 10 | blocked |

The anchors are phrased around the stack's own documented behaviour, never "follows
CSS".

| Sub-score | Covers |
|---|---|
| Setup | install, project creation, first build |
| Orientation | finding what to read; the project's agent instructions |
| Docs accuracy | the docs said what's true |
| Language ergonomics | how often intent fit the language |
| Diagnostics | errors said what to do |
| Dev loop | edit-to-seen speed and reliability |
| Platform builds | native builds, signing, simulators |
| Verification | seeing and proving the app works |
| Capability coverage | what was provided, built by hand, or missing |
| Predictability | surprises; behaviour that contradicted the docs |
| Confidence at done | finished sure, and was right (checked against fidelity) |

**Overall** is graded on its own anchors, not averaged from the sub-scores.

Each grader:

- reads the **transcript first**, with full tool outputs available on request (it gets
  a condensed view, plus a tool to fetch any result in full)
- reads the diary second, then the command log and the time breakdown
- scores **observed difficulty**, and separately records **fault attribution**
  (`stack`, `agent`, `unclear`)

An agent mistake the docs could have prevented still counts as difficulty. Reading the
stack's source is evidence of friction; it is not penalised for its own sake. Diaries
and app text are untrusted data in the grader prompt.

**The panel:**

- Claude (Opus 5.5)
- Astra (`gpt-6-astra` xhigh, via `codex exec`)
- Grok 4.7 (xhigh)
- Gemini, when its CLI runs headless

Comparisons **across stacks** use only the shared axes: setup, orientation, docs
accuracy, diagnostics, dev loop, verification, predictability, and confidence.
Language ergonomics, platform builds and capability coverage are reported within a
stack.

Graders are blind to each other. The record keeps every grader's scores, the median,
and the **other-family median**, which leaves out the builder's family. The gap
between the two medians is a diagnostic, not proof of self-preference. If the panel's
spread on overall exceeds 15, the trial is flagged.

To save budget, the full panel grades every audit trial and at least one trial per
cell per batch. The rest get two graders, never including the builder's family. The
calibration set is spot-checked against app trials, not only the beacons diaries.

### 6.4 Validation without human labels

Charlie has no time to hand-score, so the scale is checked by **perturbation**. A
suite of transcript and diary pairs is edited in known ways, and the bench re-checks
it whenever a grader prompt or model changes. The suite asserts:

- adding retries or a 10-minute dead end lowers the relevant sub-score
- making the diary twice as verbose, without new events, doesn't move scores by more
  than 3
- swapping the builder's model name doesn't move scores by more than 3
- removing a Rough entry that the transcript still shows doesn't raise the score
  (transcript-first holds)

A grader configuration that fails the suite isn't used. The calibration set (signal,
bluesky, dice-tray, beacons r1–r5) checks **repeatability**: a re-grade within ±5. It
says nothing about truth.

## 7. Cost

The cap is **$2000 a day**, covering everything:

- builders
- judges and graders
- triage
- fix-lane authors and reviewers
- failed runs

`batch.mjs` **reserves** each trial's ceiling in `ledger.jsonl` before dispatching it,
and releases the unspent remainder when the trial ends. Parallel trials therefore
can't overshoot the cap, and the report marks a batch cut short as partial.

On Claude subscription auth, the dollars are list-price equivalents, and the real
constraint is the rate limit. The operator watches for 429s and lowers parallelism.

The narrow start (§11 Phase 0–1) costs a fraction of the cap. Phase 0 measures what a
fully graded trial costs, and that number sizes everything after it. A planning guide,
until measured:

- **under about $15** per graded trial: three or four exact2 cells at N=6 nightly fit,
  with half the budget left for fix lanes and A/B
- **a full 20-stack comparator sweep** runs only when its measured cost fits a single
  weekend's cap; otherwise it is split across months

## 8. The loop

### 8.1 Shape

```
batch → grade → aggregate → triage → fix lane → A/B → confirmation → land → watch
```

1. **Batch**: the nightly plan (§12.3).
2. **Aggregate**: completion rate, then medians and IQRs of time, dollars, fidelity
   and experience, per cell. There is no composite score. A change that saves tokens
   but loses completion or fidelity is not a win.
3. **Triage**: an agent clusters findings across dev-task trials, dedupes them against
   exact2's `queue/` and `docs/issues.md`, and ranks them by frequency × estimated
   cost. Each finding is classed as a docs, diagnostic, tooling, bug or feature fix.
4. **Fix lane**: docs, diagnostic and tooling fixes, plus bugs with a reproduction.
   - Opus 5.5 implements, and Astra xhigh and Grok 4.7 xhigh review blind.
   - Three rounds at most, then the finding is descoped or escalated.
   - **Features, language changes, and design go to Charlie**, as a `queue/` entry or an
     RFC.
   - **Nothing in the lane adds apparatus to exact2** (RULES §Agents).
   - Pitfalls that can't be fixed yet go in `docs/agent-pitfalls.md`.
5. **A/B** (§8.2), then **confirmation**, then **land** (§8.3).

### 8.2 The A/B

The fidelity metric here is the scripted pass rate (§5.3), never the judged score. The
lane's acceptance test is asynchronous; it is not a blocking check (RULES
§Loop shape).

- **Before running:** declare the target metric (one of: completion, elapsed time,
  dollars, or one sub-score), the minimum worthwhile improvement, and the cells. Pick
  the cells so they **represent** the finding's scope. Use **fresh** baseline runs;
  never reuse the bad runs that surfaced the finding, which would invite regression to
  the mean.
- **Running:** A and B trials are **interleaved and randomized within blocks**: the
  same machine, the same hour, alternating order. N per arm comes from the measured
  noise floor (§11 Phase 1), with a minimum of 6.
- **Deciding:** a stratified bootstrap 90% interval on the B−A difference.
  - **Win**: the interval clears zero *and* the median beats the declared minimum.
  - **Inconclusive**: the default outcome, and an allowed one. The fix may still land
    if it's a plain docs correction with no metric claim, labelled `unmeasured`.
  - **Loss**: revert or rethink.
- **Guards, all required:** the regression tasks don't regress; completion rate
  doesn't drop; no new crash class; no cell's
  fidelity median drops by more than 3; and no rise in the share of trials with a
  critical failure.
- **Confirmation:** a win is re-run once on fresh cells before landing. Many fixes are
  tested in a week; the confirmation batch is what keeps the false positives from
  accumulating.

### 8.3 Landing

Charlie authorised pushing well-reviewed fixes to origin/main (§13). Grok's review
suggested limiting auto-landing to docs and diagnostic wording; this plan keeps
Charlie's broader authorisation, with these lines drawn:

- **Docs-only and diagnostic-wording** changes may land `unmeasured` (§8.2).
- **Code in the compiler, runner, kernel or a host** lands only with an A/B **win**
  and confirmation. It is capped at about 300 changed lines per landing. Anything
  larger is split, or goes to Charlie.
- **New documents** (an LLP, a new doc file) are apparatus. They go to Charlie.

- Landing is serialized: one integration at a time.
- Each integration rebases, runs exact2's five checks, verifies the reproduction, and
  builds and drives the app(s) it touches, as AGENTS.md requires.
- The async lane then runs per commit as usual.
- A landed fix that the async lane or the next batch attributes a regression to is
  reverted.
- **Two bad landings in a week pause autonomous landing** until Charlie says go.

### 8.4 What the loop never touches

The loop never touches tasks, scenarios, rubrics, grader prompts or the perturbation
suite. They change only with a human's yes, and each change bumps the bench version.
Batches on different bench versions aren't compared.

## 9. Keeping the numbers honest (summary)

| Risk | Defence |
|---|---|
| Variance | Paired, interleaved A/B; N from the measured noise floor; intervals, never two single runs (§8.2) |
| Goodhart | Dev, regression and audit roles; audits replaced after use (§3.3) |
| Stopping early | Completion and censoring (§5.4) |
| Contamination | Separate user, stripped checkout, results outside builder-writable storage (§2) |
| Grader bias | Multi-family panel, other-family median, perturbation suite (§6) |
| Diary perturbation | Randomized diary-off slice (§4.3) |
| Drift | Harness and model versions per trial; an upgrade starts a new baseline |
| Prompt injection | App content and diaries are untrusted data in judge and grader prompts |

## 10. Comparators

### 10.1 The set

| Group | Stacks |
|---|---|
| Apple native | UIKit, SwiftUI |
| Android native | Android Views, Jetpack Compose |
| Cross-platform mobile | Expo, React Native without Expo, Flutter, KMP with Compose Multiplatform, .NET MAUI, Ionic/Capacitor, Lynx |
| Web | React (Vite), Vue, Svelte, Solid, Angular, plain HTML+CSS+JS |
| Desktop | Tauri, Electron, SwiftUI for macOS |

Each stack is pinned to a toolchain and starts from the starter a person would use,
recorded per batch.

### 10.2 What's comparable

- **Same:** the brief, scenarios, judge, rubric, and grader panel. The same builders
  are used, and each comparator batch includes **contemporaneous exact2 controls**,
  with identical builder and judge assignments.
- **Comparator builders** get a framework-neutral copy of the detailed diary.
- **Two kinds of experiment, never mixed:**
  - **Single platform**: an iOS-only task, built in exact2, SwiftUI, UIKit, Expo,
    Flutter… A joint web+iOS exact2 run is never scored against an iOS-only SwiftUI
    run.
  - **Matched platform set**: a web+iOS+Android task. exact2 once, against each
    cross-platform stack once, and against native per platform (SwiftUI + Compose +
    React). The native set is built **in one author session**, so the shared reading of
    the spec is paid once, as a person would pay it. A parallel variant (three
    sessions) reports **delivery elapsed time** as the critical path. The headline stays
    per platform.
- **T6 baselines** are built per stack to a common functionality and quality bar,
  checked by the same scenarios before they are frozen.

### 10.3 Cadence

Comparators run monthly, or on demand. They start with **React**, next to exact2 in
Phase 2. The first monthly set is four stacks: plain HTML+CSS+JS, Expo, SwiftUI and
Compose. It stays at four until those adapters agree with the scripted checks on a
known-good app. Then Flutter, React Native without Expo, KMP, and the rest are added,
in order of value per dollar. A stack is added
only once the adapters (§5.1) drive it.

## 11. Phases

**Phase 0: the instrument (2–3 days).**
- Build: the repository, user isolation, the Claude Code adapter, `run.mjs`, the web
  adapter, and T1–T3 with their scenarios and the record.
- Exit when all of these hold:
  - a trial runs unattended end to end
  - its time and token numbers match the transcript by hand
  - the runner's own command log agrees with `commands.jsonl` where they overlap
  - the per-trial cost is measured

**Phase 1: graders and noise (about a week).**
- Web only, one harness, T1 and T2 first, T3 when they're stable.
- Build:
  - the `local` standing answer and the detailed diary in exact2 (§6.2; the only
    exact2 changes)
  - the judge and the experience panel
  - the perturbation suite
  - the calibration re-grade
- Run the **noise floor**: one cell × 12. That fixes N and the minimum worthwhile
  improvement per metric. Publish **the smallest effect the budget can detect**, and
  shrink the matrix until an interleaved confirmation of that effect fits in the
  A/B share of the budget.
- Write the **audit tasks** before Phase 2.
- Exit when the perturbation suite passes and N is set.

**Phase 2: the loop, narrow (one to two weeks).**
- Scope: exact2 on web, T1–T3, one builder (Claude Code with Opus 5.5).
- Run triage, the fix lanes, the A/B, confirmation and landing.
- React joins as the first comparator, and gets the scenario-portability proof.
- Exit when **one fix has been shown to win**, and confirmed, end to end.

**Phase 3: widen (ongoing).** Add, in this order, each only once the last is stable:

1. iOS (adapter and simulator capacity)
2. a second and third builder (Codex with Astra, Grok)
3. T4–T6
4. macOS
5. the weekly cold slice
6. more comparators

**Phase 4.** The ambiguity experiment (§5.5), Android and Linux when the hosts and
adapters exist, Windows.

## 12. The operator's runbook

For the agent on the box that Charlie hands this to. The box runs the program; Charlie
is reached only for the items in §12.5.

### 12.1 Bootstrap (once)

1. **Check the box:**
   - macOS with Xcode and simulators (iOS is Phase 3, but check now)
   - bun, Rust (rustup), Node, Playwright with Chromium
   - free disk ≥ 200 GB
   - the exact2 checkout's `bun scripts/exact.mjs setup --check` is clean
2. **Create the `bench` macOS user** (§2). Builders run as `bench` via
   `sudo -u bench`.
3. **Clone the bench repository**, `ccheever/authoring-bench`. Read its README first.
   Then push working steps there, so other boxes can see them. One operator box owns
   the nightly batch and `ledger.jsonl`; any other box takes lanes the owner assigns.
   Transcripts and app snapshots stay on the box that ran them; records, diaries and
   grades are committed.
4. **Harness credentials.** The builder needs credentials it can use without a human:
   - For Claude Code, a long-lived OAuth token in `CLAUDE_CODE_OAUTH_TOKEN`, with a
     fresh `CLAUDE_CONFIG_DIR` per trial (§4.1).
   - For Codex and Grok, their CLIs as already configured on the fleet.
   - Verify each with a one-line prompt before Phase 0 ends.
5. **Write the ledger:** `ledger.jsonl`, with today's cap of $2000.

### 12.2 Phase work

Work through §11 in order, phase by phase. Commit the bench repository after every
working step. Each phase's exit criteria are the gate; don't skip ahead.

### 12.3 Daily, once the loop runs (Phase 2 on)

| When (box local) | What |
|---|---|
| 00:00 | Nightly batch: dev and regression tasks × the current cells × N, reserved against the ledger |
| on finish | Grade, aggregate, triage; open fix lanes for the top 1–3 findings |
| daytime | Fix lanes run; A/B and confirmation batches dispatch as fixes are ready; serialized landings |
| 18:00 | Digest to Charlie (below) |
| every 2 weeks | Audit batch |
| monthly | Comparator batch (§10.3) |

### 12.4 The digest

The digest goes to wherever Charlie says; until then, to the thread that started this.
It is short, and has five parts:

- **Numbers:** completion rate, median elapsed, median dollars, median fidelity, and
  median overall experience for the main cell, each against the previous 7 days.
- **Landed:** each fix, its A/B result (win, inconclusive or unmeasured), and its
  commit.
- **Reverted or paused,** and why.
- **Top 3 open findings,** with frequency and estimated cost.
- **Needs Charlie:** the list from §12.5, if any.

### 12.5 Stop and ask Charlie

- A finding needs a feature, a language change, a design decision, or new apparatus in
  exact2.
- Two bad landings in a week (§8.3).
- Spend above $2000 in a day, or a rate limit that stops the batch two nights running.
- A grader configuration fails the perturbation suite, and there's no obvious fix.
- Any change to tasks, scenarios, rubric or graders: propose it, then wait.
- A metric hasn't moved in two weeks of landings. The approach needs rethinking, not
  more fixes.

Otherwise, decide and keep going. Log decisions made without asking in the digest.

## 13. Charlie's answers (2026-10-04)

1. "Hosts" was an agent's word. This document says *harness* and *machine*.
2. The detailed diary in exact2: yes.
3. Budget: $2000 a day. Machines: any that are free. This document is handed to a
   fleet box to run.
4. No hand-scoring for now. Use §6.4.
5. Push well-reviewed fixes to origin/main. Opus 5.5 authors; Astra xhigh and Grok
   4.7 xhigh review.
6. Comparators: §10.
7. Build with a variety of models, and grade with a variety, Claude included.

## 14. Review dispositions (r3)

Astra (`gpt-6-astra` xhigh) and Grok 4.7 (xhigh) reviewed r2 (`6271021a8`) blind, from
the same brief. Both said SOUND WITH CHANGES. Where both found the same thing
(landing statistics, cause attribution, the `never` gate, the driver's reach,
budget), that is the strongest signal; all of it is taken.

| Astra # | Finding | Disposition |
|---|---|---|
| 1 | N=3 bootstrap; landing turns noise into commits | Taken: §8.2, with interleaved blocks, a predeclared minimum, fresh baselines, confirmation, inconclusive allowed, completion and critical guards |
| 2 | Parity exemption flatters exact2 | Taken: §5.3, where fault doesn't excuse a bug and bugs dedupe by root cause |
| 3 | Hidden spec is mind-reading | Taken: §3.2, where requirements are disclosed and procedures hidden; ambiguity is §5.5 |
| 4 | A neutral driver is adapters; Linux has no AT-SPI; agent mode differs | Taken: §5.1 |
| 5 | Consensus is repeatability; transcript should come first | Taken: §6.1, §6.3, §6.4 |
| 6 | Cause breakdown over-precise; diary subtraction invalid | Taken: §4.3 |
| 7 | DONE rewards stopping early; setup not observed | Taken: §4.1 step 2, §5.4 |
| 8 | Holdouts and isolation weak | Taken: §2, §3.3 |
| 9 | Comparator comparability | Taken: §10.2 |
| 10 | Landing protocol | Taken: §8.3 |
| 11 | Budget breadth | Taken: §7, and the narrow start in §11 |
| 12 | `never` contradiction; command log coarse | Taken: §4.1 step 3, §4.3 |

| Grok # | Finding | Disposition |
|---|---|---|
| 1 | Regression to the mean; holdouts absent from the gate; N=3; auto-land scope | Taken: §8.2 (fresh interleaved A and B, regression-task guard, scripted fidelity in the gate). Auto-land scope partly taken (§8.3): Charlie's authorisation stands, but code needs a measured win and a size cap, and new documents go to him |
| 2 | Causes overlap; dev server spans the log; cache tokens belong to no cause; idle kills builds | Taken: §4.3 (raw headlines, overlapping tags, loop speed, shared warm caches), §4.1 step 5 |
| 3 | `never` disables the diary; the pin leaks the rubric | Taken: the `local` standing answer (§6.2); this LLP stripped from the pin (§2); calibration spot-checked on app trials (§6.3) |
| 4 | Three instruments in one fidelity; testId reach; parity false positives; T4 oracle; judge penalties dominate | Taken: §5.1 (id property per stack), §5.3 (scripted pass rate is the comparable number; corroborated bugs; `axParity`), §3.1 (per-platform T4 shots) |
| 5 | Budget and Phase 1 sizing | Taken: §11 (web only, smallest detectable effect), §10.3 (four stacks first) |
| 6 | Cross-stack experience axes; summed native trials | Taken: §6.3 (shared axes), §10.2 (native set in one session) |

