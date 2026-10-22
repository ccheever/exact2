# Parallel T3 Code clone work

Nate authorized parallel clone work and separate framework triage on 2026-10-21.
Linear is the task record; this document defines how workers share resources.
The coordinating thread owns task assignment, file ownership and publishing turns.

## Project environment

```json
{
  "is_react_native": false,
  "is_native_ios": true,
  "is_native_android": false,
  "app": "examples/t3-code/mobile",
  "cargo_workspace": "examples/t3-code/Cargo.toml",
  "crate": "t3-code-ios",
  "bundle_id": "com.exact.t3code.ios",
  "reference_pin": "365aa87982"
}
```

Use the Bun pinned in root package.json. On this machine it is available at
`/Users/nate/.bun-1.4.2/bin/bun`; the default PATH currently resolves an older
Bun. Rust launchers are under `/Users/nate/.cargo/bin`. Read the mobile README
for setup and build commands. Root Cargo checks do not cover this app workspace.

## Unattended execution

The coordinator owns the whole clone through final acceptance, not just this
initial ticket list. After setup approval, start coordination and the reference
audit, then keep up to three workers on unblocked tasks. Split broad tasks,
supply missing acceptance details, resolve routine choices and dispatch the next
task without asking Nate to manage the queue.

Keep the coordinator in this thread. Create one T3 scheduled continuation bound
to it every 15 minutes, using a stable creation request ID. Record the schedule ID
in the Linear project specification. Worker completion should also notify this
thread. On every continuation, read Linear and current thread/task status first;
resume or replace interrupted work from its durable checkpoint without duplicating
an active worker, ownership claim or publishing turn.

A continuation reconciles completed work, inspects failures, verifies the shared
head, frees finished resources and assigns the next available task. The audit
and first ten tickets are a starting queue. Newly discovered in-scope parity
gaps become tasks and are completed under the same acceptance requirements.

Workers may commit, rebase and push their assigned app changes, run checks,
allocate owned simulators, clean disposable owned output and investigate bounded
framework defects within the approved workflow. Routine steps need no renewed
permission. The coordinator checks task results rather than accepting a worker's
summary as whole-app acceptance.

A fix still gets at most three correction rounds. Preserve an exhausted target
and escalate it; a new thread does not reset that limit. Continue independent
work while a framework ruling or genuinely missing access is pending. An unresolved
required behavior remains a blocker to finishing the clone.

Completion means the exact integrated head implements the agreed pinned-reference
routes and behavior on iPhone and iPad, passes the agreed interaction and visual
matrix and relevant checks, has a runnable installable build with evidence, and
has completed task cleanup. Empty queues, passing source tests and worker reports
are not completion. Record remaining discrepancies explicitly rather than
silently weakening the spec. Keep app work on PR216; do not merge it automatically.

Disable the scheduled continuation once final acceptance passes. If every
remaining task requires an external decision or unavailable access, publish one
precise blocked report and disable the timer until that condition changes.
Do not leave a timer producing repeated blocked updates. A local unattended run
requires the computer and T3 execution environment to remain available.

## Task assignment

Start with a current-source audit against the pinned mobile reference and
PR216's remaining acceptance. Confirm a reference change with Nate before
changing the pin. Use existing app decisions rather than writing another full spec.

Each Linear task names its source reference, starting commit, expected behavior,
owned files, dependencies, tests and device acceptance. Record the worker thread,
branch and worktree. Keep shared routing, client state and native infrastructure
under one owner until the coordinator splits their file boundaries.

Start at most three implementation workers. Give each a separate worktree and
unique task branch from the current PR head. Use T3's worktree-bound thread
launch for independent threads. Use child-task delegation for investigations;
pass the failing task, commit, reproduction and rules explicitly.

## One clone PR

The clone PR is https://github.com/ccheever/exact2/pull/216.
Its shared head is `feat(example)/t3-code-ios`; its base is
`daehyeon/t3-code-main`. Workers commit on their own task branches and push
their commits to the shared head. They create no task PRs into PR216.

The coordinator grants one publishing turn at a time. During that turn:

1. Start with a clean task worktree containing only that task's commits.
2. Fetch the shared remote head and rebase the task branch onto it.
3. Resolve conflicts within the task's owned files. Return cross-owner conflicts
   to the coordinator. Review the resulting diff and run affected checks.
4. Push `HEAD:refs/heads/feat(example)/t3-code-ios` to origin normally.
5. Record the landed commit and release the publishing turn.

A rejected push means fetch, rebase and recheck before retrying. Never force-push
the shared head or check it out simultaneously in multiple worktrees.
After a batch, the coordinator tests the integrated head and updates PR216.
Link PR216 to each working thread through T3's PR-linking tool.

## Exclusive simulators

A booted simulator is not necessarily idle. The coordinator must grant exclusive
ownership by host and UDID before installation, launch or interaction. Record the
owner thread and task. Treat devices belonging to other work as busy.

Reuse a compatible, explicitly released simulator. If none is idle, boot an unused
compatible simulator; create one if no suitable unowned device exists. Use the
same model and OS for clone/reference comparisons. If machine capacity is
exhausted, queue device work instead of taking another agent's simulator.

Use T3 `device_list`, then `device_open` so Nate can watch. Keep its exact
agent-device launcher, host configuration and session arguments on every command.
Prefer element references. Supply `--sim <UDID>` or `EXACT_SIM` to Exact builds
and drives. Always give Exact's driver a unique storage name.

Exact's iOS driver installs and replaces its app on that UDID. Different storage
names do not isolate two agents sharing the same simulator. Keep each worktree's
target directory, generated assets, fixture server home and ports independent.
Use owned test data. Stop only recorded processes and release only owned devices.
If using Argent, scope cleanup to that worker's devices.

Before concurrent implementation starts, the coordination task must verify a
shared ownership mechanism, serialized publishing and storage cleanup across
independent threads. Instruction files alone do not enforce locks.

## Storage and cleanup

The coordinator checks free space and owned storage before dispatch, before a
native build and after each completed task. Keep at least 50 GiB free on the
volume receiving the build. Allow one native build at a time across clone and
framework workers. Code and focused non-native checks may proceed in parallel.
If capacity is insufficient, clean eligible owned output or queue the build.

Keep active worktrees warm while they are being used. Each task records its
worktree, build directories, generated output, fixture homes, processes and
created simulators. The coordination helper must make that ownership available
across threads. Never share target directories.

A task is finished only after its agent:

1. Pushes accepted source to the shared head, or preserves unfinished source
   in a named commit before handing it off.
2. Publishes the required screenshots, test results and concise failure logs
   to its Linear task or linked framework issue. Keep needed reproduction
   inputs outside directories selected for deletion.
3. Stops its recorded processes, releases its simulator and removes its
   disposable build output, generated viewer bundles and fixture data.
4. Reports retained paths, before/after sizes and any reason cleanup is blocked.

The coordinator removes a task worktree with ordinary `git worktree remove`
after the worker is terminal and its source and evidence are safe. Inspect
tracked, untracked and ignored files first. Keep the branch ref until its
commits are reachable from the shared head or another durable handoff ref.
An uncommitted change or active owner blocks automatic removal; never force it.

Retain at most two idle simulators created by this clone effort: a phone and
an iPad. Reuse those before creating another. Shut down a released owned device;
delete surplus task-created devices once their evidence is captured and no
thread owns them. Never reset, erase or delete another task's device.

Keep Exact's shared content-addressed caches and installed toolchains. Their
small shared cache avoids rebuilding every task. A blocked or failed task may
discard reproducible build output after preserving its diagnostics and source.
Old age alone never proves a build, worktree or simulator is safe to delete.
Existing worktrees require an ownership audit before reclamation.

## Framework defects

A suspected framework defect starts an investigation child task with the failing
commit, reproduction, expected behavior, logs and relevant rules. Give it a
separate worktree. Preserve the app checkpoint; reproduce the defect against
current remote main in a clean framework branch before proposing a fix.

A small fix has a reproduced cause, bounded changes, existing admitted behavior
and focused acceptance checks. It requires no new API, deferred feature or
unapproved apparatus. Its agent may open an independent draft framework PR
targeting main after running the applicable repository checks and native checks.
Keep mobile commits out of that PR. Do not merge it automatically.

For unclear causes, broader design, permissions or trust changes, deferred scope,
or an exhausted three-round fix loop, file or update a GitHub issue with evidence,
options, the decision needed and a link to the Linear blocker. Do not turn an
app-level gap into a framework issue without reproduction.

The coordinator explicitly integrates an accepted framework fix into the clone
head, recording its source PR. Dependent tasks remain blocked until their required
fix is available and verified there.

## Acceptance

Run focused task checks in each worker and the app's full checks on integrated
checkpoints. For runtime changes, build and drive the resulting app. Compare
reference and clone with matching device, OS, appearance, text size, locale,
seeded data and scroll position. Record behavior and visual evidence separately.

The coordinator owns final phone/iPad, accessibility, motion, persistence and
performance acceptance. Passing source tests or a build alone does not complete
a parity task. Final delivery remains gated by whole-app acceptance.
