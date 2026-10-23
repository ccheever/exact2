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

### Ordinary native acceptance

Exact's `timing: 'platform'` preserves UIKit animation timing. The agent carrier
still controls runner scheduling; external Argent touches alone can leave
mutation continuations and tasks waiting. A driver `clock data` or `clock settle`
result is diagnostic evidence, not ordinary app acceptance. Preserve the exact
operation history before attributing a timing failure to the app or framework.

For an ordinary comparison, launch the admitted artifact through Argent without
`EXACT_AGENT` or its socket. Development builds may need the task's computed
`EXACT_ASSETS` path and an explicit plan only if the normal launcher uses one.
Record and restore only environment values the task changes. Named
`EXACT_AGENT_STORAGE` isolates storage only while `EXACT_AGENT` is present;
`EXACT_AGENT=0` is not a substitute. Audit and claim the dedicated task-created
simulator's app container and Keychain scope before using them without the agent.
Preserve their initial contents and identities, and use only task test data.

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

## Shared coordination commands

Run from the repository root with the pinned Bun:

```sh
export PATH=/Users/nate/.bun-1.4.2/bin:/Users/nate/.cargo/bin:$PATH
coord=examples/t3-code/tools/coordination/coordination.mjs
bun "$coord" help
bun test examples/t3-code/tools/coordination/coordination.test.mjs
```

The helper stores machine-wide claims and receipts in
`~/.local/state/exact-t3-coordination/v1`. All worktrees and separate framework
checkouts on this machine use that location. Repository identity comes from the
real Git common directory. Do not copy the state into a worktree or override it.
The fixture transport uses private state only for disposable tests.

Claims use atomic directory creation and an atomic state-file replacement.
No claim expires. A dead PID, old timestamp or booted device does not grant
ownership. A failed state write or incomplete process launch requires an audit.
This helper coordinates cooperating workers; commands run outside it bypass its
claims. Use it for every publishing turn, native build and task fixture process.

Initialize once, then register each worker from its own worktree. Retain the
returned task ID in Linear. Repeating registration with the same issue, thread
and worktree returns the same ID; another owner is refused.

```sh
bun "$coord" init --thread 628aac75-8ba2-4cdc-a567-4b7e160f700a
bun "$coord" register --issue ENG-28497 --thread "$worker_thread"
bun "$coord" status
```

The coordinator grants a turn using the worker's returned task ID. The worker
then runs `publish` from its clean task branch, supplying its affected check as
arguments after `--`. The helper holds the turn through fetch, rebase, that
check and an ordinary push to PR216's head. It never force-pushes. A failed
rebase leaves the source available for resolution in the same worktree.

```sh
bun "$coord" grant-publish --task "$task_id" --thread 628aac75-8ba2-4cdc-a567-4b7e160f700a
bun "$coord" publish --task "$task_id" --thread "$worker_thread" -- bun test examples/t3-code/tools/coordination/coordination.test.mjs
```

`release-publish` returns an unused turn. Launched operations require
`process-audit` when their ownership remains held. Return cross-owner conflicts
to the coordinator and request a new turn before retrying.

### Allocate a device and build

The coordinator audits an unused shutdown simulator or creates one using the
T3 device tooling, then enrolls it. Do not infer ownership from inventory alone.
The inventory file is a fresh normalized array with `host`, `udid` and `state`
for every device on that host, with states such as `Shutdown` and `Booted`.
Record the same host identifier that T3 returned. A device is identified by
host and UDID together. `--origin audited-unused` attests an explicit ownership
audit; use `created` only for a device this effort created.

```sh
bun "$coord" device-enroll --thread 628aac75-8ba2-4cdc-a567-4b7e160f700a --host "$device_host" --udid "$udid" --class phone --runtime "$runtime" --inventory "$inventory_file" --origin created
bun "$coord" device-claim --task "$task_id" --thread "$worker_thread" --host "$device_host" --class phone --runtime "$runtime"
```

To assign a newly created simulator directly to an active worker, the coordinator
adds `--task` to enrollment. Its fresh Shutdown inventory entry must also include
`class` and `runtime` matching the command. The helper enrolls and assigns that
exact device atomically, preserving the existing idle pool:

```sh
bun "$coord" device-enroll --thread 628aac75-8ba2-4cdc-a567-4b7e160f700a --task "$task_id" --host "$device_host" --udid "$udid" --class phone --runtime "$runtime" --inventory "$inventory_file" --origin created
```

Use `ipad` for an iPad. Claiming returns one compatible released device. If none
is available, queue the work or have the coordinator audit/create and enroll
another; the helper never takes an unknown device. After claiming, call T3
`device_list` and `device_open`. Boot or drive only the claimed host/UDID, keeping
the exact launcher, host configuration and session arguments from `device_open`.
The helper records ownership; it does not issue simulator lifecycle commands.

Allocate fresh disposable output with a declared upper estimate in GiB. The
receiving volume must have that estimate plus 50 GiB available. The check runs
before allocation and every build. It is admission control, not a disk quota;
stop and preserve diagnostics if a build grows beyond its estimate.

```sh
bun "$coord" build --task "$task_id" --thread "$worker_thread" --output target/clone-build --budget-gib 20 -- cargo check --manifest-path examples/t3-code/Cargo.toml -p t3-code-ios
```

The build wrapper sets `CARGO_TARGET_DIR` to the owned output and holds the
machine-wide native-build slot. Use `--existing-output target/clone-build` for a
later build in the same receipt. `allocate` reserves a fresh ignored directory
without starting a command. Allowed output roots are `target`, the app and
mobile `target` directories, mobile `dist` and mobile `assets/browser-mobile`.
Existing output cannot be adopted. Put disposable fixture homes under an owned
target directory. Unknown output and shared caches remain untouched.

`scripts/filesystem.mjs` also builds a checkout-private helper in
`target/exact-filesystem-tool`, independently of `CARGO_TARGET_DIR`. Allocate that
initially absent sibling before the first build or driver setup and include it
in cleanup inventories. A scanner that excludes all of `target/` cannot prove
that another target output was absent or task-owned.

Claim ports and Exact storage names before launching a fixture. Use a host and
identifier together, and verify that the actual port is unused before binding.
A claim excludes other cooperating tasks; it does not reserve an operating
system socket. Run fixtures in the foreground through `run`, which records their
PID and launch identity. Never detach or daemonize a command.

```sh
bun "$coord" resource-claim --task "$task_id" --thread "$worker_thread" --kind port --value "$device_host:49161"
bun "$coord" resource-claim --task "$task_id" --thread "$worker_thread" --kind storage --value "$device_host:t3-$task_id"
bun "$coord" run --task "$task_id" --thread "$worker_thread" -- bun path/to/owned-fixture.mjs
```

### Release processes and clean output

An exited build or fixture launcher does not prove its descendants exited. Its
lease remains held until `process-audit`. Commands run in a dedicated process
group; the helper records surviving members and refuses release while recorded
processes are live or unknown. `stop` signals only an individually recorded PID
whose host and launch identity still match. It never kills by pattern or group.

Save the operation ID from the build/run result. Supply an explicit audit file:

```json
{
  "taskId": "the returned task ID",
  "worktree": "/absolute/owned/worktree",
  "auditorThreadId": "the worker or coordinator thread ID",
  "operation": "the returned operation ID",
  "processes": [],
  "noUnrecordedDescendants": true
}
```

`processes` contains any additional descendants recorded at launch or found by
an ownership audit, each with `pid`, `host` and `started` as returned by the
helper's `processIdentity(pid)`. Live entries are retained as receipts and keep
the lease held. Do not assert `noUnrecordedDescendants` when lifetime is uncertain.
A process that creates a new session can escape its group. The helper cannot
prove that such a process exited; record its individual identity and stop it,
or leave ownership held for investigation. A terminal thread proof alone never
releases a launched native-build or fixture operation.

```sh
bun "$coord" process-audit --task "$task_id" --thread "$worker_thread" --proof "$process_audit_file"
bun "$coord" stop --task "$task_id" --thread "$worker_thread" --pid "$recorded_pid"
bun "$coord" resource-release --task "$task_id" --thread "$worker_thread" --kind port --value "$device_host:49161"
bun "$coord" resource-release --task "$task_id" --thread "$worker_thread" --kind storage --value "$device_host:t3-$task_id"
```

Repeat the audit after the recorded processes exit. Shut down the owned simulator
through T3's device tooling, then release it with fresh inventory. Source and
evidence must be preserved before deleting output:

```sh
bun "$coord" device-release --task "$task_id" --thread "$worker_thread" --host "$device_host" --udid "$udid" --inventory "$inventory_file"
bun "$coord" preserve --task "$task_id" --thread "$worker_thread" --evidence /absolute/path/outside/output/checks.log
bun "$coord" cleanup --task "$task_id" --thread "$worker_thread"
```

Process receipts include the local boot identity, so a hostname change does not
change a new receipt's machine identity. A legacy receipt without that field
still refuses an unfamiliar hostname. Keep its leases held. The coordinator can
bind its old hostname to this boot using `bind-process-host --thread
<coordinator> --proof <file>`. The proof names `fromHost`, current `toHost` and
`bootId`, plus `anchor: { taskId, operation, pid, started }` from a still-live
legacy helper launcher. The command verifies the PID/start, owned worktree,
helper script and task/thread arguments before recording the binding. It neither
stops processes nor releases claims. The normal process audit remains required.
Another boot, a dead witness or unknown ownership refuses recovery; never edit
state or substitute a new hostname in old evidence.

`preserve` keeps HEAD under `refs/t3-coordination/<task-id>` and copies each
supplied evidence file into shared state. Repeat it for additional files.
`cleanup` requires current committed source, preserved evidence, no live or
unknown processes and no leases. It deletes only recorded disposable directories
whose device/inode still match, refusing symlinks and nested Git repositories.
It reports removed KiB, free space before/after and retained paths. Other ignored
files block worktree retirement and require a separate ownership audit.

After the worker stops, the coordinator reads fresh `t3_thread_read` state and
saves its `structuredContent` or `{ "thread": ... }` response to a proof file.
`recover` requires a terminal status, matching thread/worktree, no active run or
pending requests and no live/unknown recorded process. It does not infer safety
from age. The coordinator audits any held operation before cleanup, then retires
the clean worktree from a different worktree:

```sh
bun "$coord" recover --task "$task_id" --thread 628aac75-8ba2-4cdc-a567-4b7e160f700a --proof "$terminal_proof_file"
bun "$coord" retire --task "$task_id" --thread 628aac75-8ba2-4cdc-a567-4b7e160f700a --proof "$terminal_proof_file"
```

Retirement uses ordinary `git worktree remove` and retains the source ref and
evidence. It refuses uncommitted, untracked or ignored files. Never remove your
own running worktree. For a crashed state mutex, inspect `mutex/owner.json` and
use `recover-mutex` only when that exact recorded process is dead. Missing or
unknown identity requires investigation, never a timestamp-based override.

A read-only native collaboration investigation can instead supply fresh
`collaboration.list_agents` output as `nativeCollaboration`, with its registered
`worktreePath`. Only a matching `/root/` agent with completed status and no
outputs, processes, pending launches, leases or devices qualifies. Preserve its
source and evidence before retirement. Other native work requires a T3-bound
owner and the normal terminal proof.

At most one released task-created phone and one iPad per host remain `idle`.
Surplus released devices become `retiring` and cannot be claimed. The coordinator
captures their needed evidence, deletes only those exact devices through T3,
then runs `device-retire` with host, UDID and fresh inventory proving absence.

If a released local simulator remains idle under another recorded host label,
the coordinator can use `device-surplus --thread COORDINATOR --proof FILE`.
The proof names the full expected device record, its directory `identity`
(`path`, `dev`, `ino`), one or two `retainedDevices` with the same expected-record
and identity fields, fresh Shutdown `inventory`, preserved canonical `evidence`
paths, a `reason` and `observedAtMs` within the last minute. The target field is
`expectedDevice`; each retained entry uses `expected`. All directories must be
siblings under `Devices`, and a compatible released pool device must remain.
This marks only the selected record `retiring`; it changes no host label and
deletes nothing. The normal deletion and `device-retire` sequence still applies.

### Recover the continuation without duplicate work

Keep the existing coordinator schedule ID in Linear. On restart, call
`list_scheduled_tasks` and locate that ID and its bound coordinator thread.
Do not create another timer. Read Linear, `t3_thread_read` for every owned thread,
retained delegated task IDs and `coordination.mjs status` before dispatching.
An interrupted worker with unresolved receipts remains owned until audited;
recover it before creating a replacement. Provision worktrees sequentially.

Send each checkpoint/completion with `t3_thread_send` to
`628aac75-8ba2-4cdc-a567-4b7e160f700a`. Use a stable `clientRequestId` for that
logical message and reuse it on transport retries. Retain the returned
`messageId`; a retry must return the same ID. Include worker thread ID/link,
issue, commit, actual checks/evidence and cleanup receipts. The coordinator
reconciles that checkpoint against live ownership and the shared head before
assigning more work. A recovered task retains its correction-round count.

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
