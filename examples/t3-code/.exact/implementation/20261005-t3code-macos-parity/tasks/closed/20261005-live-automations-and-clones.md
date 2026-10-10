---
name: 20261005-live-automations-and-clones
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: unverified
delivery: merged
repository: https://github.com/ccheever/exact2
base_branch: daehyeon/t3-code
branch: feat(example)/t3-code-live-automations-and-clones
pr_url: https://github.com/ccheever/exact2/pull/156
verified_commit: null
---

# Live automations and tracked project clones

Current acceptance: client implementation and partial UI verification only.
Authenticated Codex completion, real-time scheduled execution and the tracked-clone
runtime flow remain unverified. PR #156 must not be described as end-to-end complete.

## Outcome

**Automations.** Settings › Scheduled tasks lists the tasks of every connected environment and updates by itself while the server schedules and runs them (status, next run, last error). A thread's details panel gets an **Automations** section for the tasks bound to that thread, with Edit, Run now and a Pause/Resume switch. **Clones.** Adding a project by cloning a repository returns at once: the server creates the project and clones in the background. A toast per clone shows progress with Cancel, then "Cloned X" with Open project, or a failure with Retry and Remove project. The project's draft shows the same state in its composer and cannot send until the clone ends.

## Scope and exclusions

Included:

1. **Task stream** (`scheduledTasks.subscribe`: a snapshot on subscribe, then the full list after every change) replacing the one-shot `scheduledTasks.list` reads (`scheduled-view.ts:84`, `settings-rest-commands.ts:129`, `client.ts:1193`).
2. **Settings page:** one section per connected environment (heading only when more than one), scope filter `matchesScheduledTaskScope`, per-environment loading, error and "Environment disconnected" states, `?taskId=` deep link that opens the editor once ("Task unavailable" when missing), 60 s relative-time refresh. The editor, toggle, run and delete already exist (`scheduled-settings.ts`, `settings-scheduled.contract:244-358`, `settings-rest-commands.ts:129-145`); keep them and align them with the ported logic. Today the page reads only the focused environment (`scheduled-settings.ts:27-34`).
3. **Thread details "Automations"** between Version Control and the lineage section, for a thread (not a draft) in the full presentation: status dot (never, running, succeeded, failed), title, schedule label with "next <relative>" or "paused", heading action "Manage scheduled tasks", Edit (opens the page with `taskId`), Run now, switch.
4. **Tracked clone** (`projectClone.start`, `projectClone.cancel`, `projectClone.retry`, stream `subscribeProjectClones`), used when the environment reports `capabilities.projectCloneTracking`; keep the blocking `sourceControl.cloneRepository` path only for servers without it (`palette-commands.ts:129-148` today).
5. **Toasts, composer banner, send block, Remove project** (non-forced `project.delete`; the clone's `manageProject` forces, `client.ts:1311-1321`).

Excluded: the Add project palette flow itself (done: `palette-add.ts`, `palette-commands.ts`); the editor's fields (done); server scheduler and clone tracker (never re-implemented); the reference's ThreadRelationshipsPanel (lineage, done).

## Context and guidance

Parent specification: [spec](../../spec.md). Paths: `C/` = `examples/t3-code/`; `W/` = `apps/web/src/components/` at T3 Code `1e2ecbd975`. Line numbers are from the mc-orch tree on 2026-10-05; `20261005-hot-file-split` moves code, so find it by symbol. Tools are named by their `target/t3-ui-parity/…` path (committed under `examples/t3-code/tools/` with the same relative paths, decision U23).
Source behavior: `W/settings/ScheduledTasksSettings.tsx:164-200, 288-330, 480-520`, `W/settings/scheduledTasksSettings.logic.ts:21-144`, `W/chat/ThreadAutomationsPanel.tsx:39-217`, `W/chat/ThreadDetailsPanel.tsx:226`, `packages/contracts/src/scheduledTask.ts`, `packages/contracts/src/rpc.ts:471-476, 517-520, 1085-1118`, `W/ProjectCloneToastCoordinator.tsx:33-240`, `W/CommandPalette.tsx:2663-2740`, `W/ChatView.tsx:2405-2515, 11163-11178`, `apps/web/src/hooks/useRemoveClonedProject.ts`, `apps/web/src/state/projectClones.ts`, `packages/contracts/src/projectClone.ts`, `packages/contracts/src/environment.ts:196-199`.
Library revision: `20261005-platforms-v3`. Selected topics: state-and-data (stream snapshot semantics; late replies; mutation then refresh), layout-and-interaction (the details panel is a scroll column), design (all states), accessibility (switch and icon button names), motion (pulsing "running" dot; reduced motion), testing-and-debugging, platforms. Unknown in the library: app-local Swift modules, native toasts. The clone's runtime evidence on the pinned main is the basis.
Consumer framework revision and toolchain: main pin from `20261005-clone-on-exact2-main`; Xcode 27.0; pinned Bun 1.4.2.
Observed: the server keeps clone state in memory; a finished clone drops after a short grace, a failed one stays until retried or the project is removed, and a restart forgets running clones (`projectClone.ts` header). `projectCloneTracking` is true at HEAD (`apps/server/src/environment/ServerEnvironment.ts:251`). Toasts in the clone are a per-client queue rendered with the shell (`toast.ts:19-47`); `updateToast` can patch only title, description, details, expandLabels and action (`toast.ts:39`), so a phase change needs kind, timeout, secondary action and copy-button patches (additive). The send status chain is at `composer-controls-view.ts:114`; the project-clone reasons ("Cloning repository", "Repository not cloned") come last, after "Preparing worktree". `composerNotices` returns early when there is no thread (`composer-controls-view.ts:181`); a project draft has no thread, so the clone banner must be added for that case.
Stream precedent: `C/r4-surfaces-device.ts:25-62` and one dispatch line in `client.ts:485`. The transport allows 16 streams (`modules/apple/T3Transport.swift:529`). Other environments are already streamed to TypeScript: `EnvironmentFleet` subscribes each switched-on environment (`config`, `shell`) and drains its events per subscription id (`settings-b-fleet.ts:121-146`), over its own transport with its own 16-stream cap (`T3Fleet.swift:28-58`). The two new keys are added to `bootstrap` and `drain` there.
Scheduling preference (not a prerequisite): after `20261005-main-fix-adoption` (tooltip/popover Contract). With `20261005-hot-file-split` merged, stream keys and the unforced delete go into area files instead of `client.ts`.

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| merged task PR | [20261005-clone-on-exact2-main](../20261005-clone-on-exact2-main.md) | pending | Merged | pending |
| merged task PR | [20261005-desktop-oracle-and-trace](20261005-desktop-oracle-and-trace.md) | pending | Merged | pending |
| merged task PR | [20261005-hot-file-split](20261005-hot-file-split.md) | pending | Merged (area files and registration points exist) | pending |

## Issue assessment at preparation

Checked sources and time: plan `issues/` drafts read 2026-10-05; no upstream search (no network). Records: [X19](../../issues/closed/20261005-x19-data-source-timers.md), [X21](../../issues/closed/20261005-x21-two-way-websocket.md), [X9](../../issues/closed/20261005-x09-root-component-across-files.md), [X13](../../issues/closed/20261005-x13-hover-keys-during-pan.md), [X10](../../issues/closed/20261005-x10-text-rendering-parity.md).

| Issue / reference | Capability and target | Evidence / affected revision | Impact | Next action |
| --- | --- | --- | --- | --- |
| X19 | Timers in data sources | 60 s time refresh, 8 s success toast; the reference uses page timers | nonblocking (workaround: `now` passed as an argument, Contract tasks, Swift timers) | Keep `now` an argument in every ported function |
| X21 | Streams from a data module | Existing Swift transport; two more keys per environment | nonblocking | Record the open stream count (limit 16) |
| X9 | Root cap | `app.contract` 1,327 and `client.ts` 1,455 of 1,500 lines | nonblocking until the cap | New modules and child components; one dispatch line |
| X13, X10 | Hover during a pan; text truncation | Toast hover pause, details row text | nonblocking | Cite in the matrix |
| none new | Streams for other environments | `EnvironmentFleet` already delivers subscription events per entry (`settings-b-fleet.ts:121-146`) | not applicable | Extend it with the two keys; a fleet entry that is not `connected` shows "Environment disconnected" |

## Implementation notes

- Ports with headers and listed changes: `scheduled-tasks.ts` (`matchesScheduledTaskScope`, `validateScheduledTasksSearch`, `taskToDraft`, `scheduledTaskDefaultModel`, `scheduleLabel`, `relativeLabel`; the clone's `editorDraft`, `defaultModelKey`, `scheduleLabel` and `runLabel` become thin users of it), `project-clones.ts` (`projectCloneDisplayName`, `projectCloneProgressSummary`, stage labels: Connecting, Counting objects, Receiving objects, Resolving deltas, Checking out files; the toast and banner state machines of the two reference components, as pure functions of the snapshot list and the viewed draft).
- Streams: keys `scheduled-tasks` and `project-clones` per environment: the focused client as in `r4-surfaces-device.ts`, every other environment as new keys in `EnvironmentFleet` (`settings-b-fleet.ts`: `FleetEntry.subscriptions`, `bootstrap`, `drain`). Palette, editor writes and task actions address an environment with `EnvironmentFleet.native(native, key)` (`connections.ts:248`). Show a toast only from the shell view build, as `providerUpdates` does (`shell.ts:244`).
- Toast rules (from `ProjectCloneToastCoordinator`): key by project id; skip an identical redraw; running = loading, timeout none, Cancel, no copy button; done = success, 8 s, destination path, "Open project" closes the toast and opens a draft for the project (`openDraft`, `client.ts`); failed = error with the server text, cancelled = info with the path, timeout none, Retry plus "Remove project", cancelled hides the copy button; a snapshot that leaves the list closes its toast unless it is a done toast; step aside while the project's draft is open and return when it is not.
- Palette: after a successful `projectClone.start {projectId, title, createdAt, remoteUrl, destinationPath}`, close the palette, wait up to 3 s for the project to appear in the shell, then open its draft; errors before git runs show the "Clone failed" toast and keep the palette.
- Remove project: `project.delete` without `force`; on success drop the project's draft and, if it is open, go home; on failure toast "Failed to remove project".
- Details row layout reuses `DetailsSection` and the switch used elsewhere in the panel; the "running" dot pulses (reduced motion: static). `aria-label`: "Manage scheduled tasks", "Edit `<title>`", "Run `<title>` now", "Pause `<title>`" / "Resume `<title>`", "Dismiss", banner buttons "Cancel", "Retry", "Remove project".
- States: loading ("Loading scheduled tasks…"), empty ("No scheduled tasks", "No tasks match this environment and project selection."), error ("Could not load scheduled tasks", "Could not load automations: <text>"), disabled (busy row, running task, read-only), hover and keyboard focus, permission (server failure toast "Could not update automation" / "Could not run automation"). A load error must not look like "no automations".

## Acceptance and reproduction

| Criterion | Setup/reset and fixture | Action or command | Expected result | Required platform | Proof |
| --- | --- | --- | --- | --- | --- |
| Live list | Lane backend (real server); a task `Run now` through the page, then an interval task every minute | `bun scripts/agent.mjs macos --json tree state "tap run-now" "clock settle" tree logs` and a second observation after real time passes | Status dot, "next …" and last error change with no reload; trace shows one `scheduledTasks.subscribe` per environment and no `list` | macOS, 1280×840 | transcript, trace |
| Scope and sections | Two environments (lane backend + a second lane backend), a project filter | Open the page at each scope | Section per environment, heading only when more than one; scope filter as the ported tests | macOS | screenshots |
| Deep link | A task id | Open `taskId` for an existing and a missing task | Editor opens once; missing shows "Task unavailable" | macOS | transcript |
| Thread Automations | Task bound to a thread; a thread without tasks; a draft; a stream error | Open details | Section only for a thread with tasks or a load error; rows and actions as described; toggle sends `scheduledTasks.setEnabled`, Run now sends `runNow` and is disabled while running | macOS, both sizes | screenshots, trace |
| Tracked clone, success | Real server; bare repo `repos/pr-demo-origin.git`; local destination | Palette › Add project › Git URL › clone | Palette closes, draft opens, toast "Cloning X · Receiving objects · N%" then "Cloned X" with Open project; send blocked "Cloning repository" meanwhile | macOS | transcript, trace |
| Cancel, failure, retry, remove | Slow remote (proposal: `uploadpack.packObjectsHook` sleep in the lane HOME gitconfig); bad URL; non-empty destination | Cancel; Retry; Remove project | Cancelled info toast and banner "Retry to bring in the repository."; failed shows the server text; Retry restarts; Remove deletes the project without `force` and clears the draft; toast steps aside while the draft is open | macOS | transcript, trace, `git` and file effect check |
| Old server | Environment without `projectCloneTracking` | Same palette flow | Blocking `sourceControl.cloneRepository` path unchanged | macOS | trace |
| Trace and pixels | Oracle and clone on one lane backend | `target/t3-ui-parity/trace-diff.mjs`; pairs at both sizes, light and dark | Same calls and payloads; every moved cell is fixed, or declared in `EXACT2-GAPS.md` with an issue link | macOS | diff, images |
| Real network clone and real hover `(attended session)` | A small public repository; real pointer; lane build with `T3_LOCAL_HOME=<lane>/t3-home`, `T3_LOCAL_PORT=<lane port 16xxx>` | Clone it; hover and Tab through toast, banner and details switch | Progress text moves; focus rings visible; toast timing as the reference | macOS, real input | session notes |
| Ported tests | — | `bun test` | `scheduledTasksSettings.logic.test.ts` (8 tests: scope `:105`, `:123`; branch `:156`; model defaults `:215`–`:253`, original names); new tests derived from the two reference components for the toast and banner state machines and the Automations panel (no reference test exists) | macOS host machine | test log |
| Keyboard focus, Escape, reduced motion | Task bound to a thread; a running clone | Tab through an Automations row (Edit, Run now, switch); Space on the switch; open the task editor with Edit and press Escape; Tab to the clone toast and banner buttons (Cancel, Retry, Remove project); `prefer prefers-reduced-motion reduce` with a running task | Controls take focus in the order shown with a visible ring; Space toggles; Escape closes the editor without saving; toast and banner buttons are reachable in the same order as on the oracle; the "running" dot is static under reduced motion | macOS | transcript |
| Gates | `git add -A` | Clone checks, `bun scripts/caps.mjs`, the five repository checks | Green; every moved cell is fixed, or declared in `EXACT2-GAPS.md` with an issue link | macOS | logs |

Task-owned source paths: `C/scheduled-tasks.ts`, `C/project-clones.ts`, `C/scheduled-view.ts`, `C/scheduled-settings.ts`, `C/settings-scheduled.contract`, `C/settings-rest-commands.ts`, `C/shell-details.ts`, `C/shell-details.contract`, `C/palette-commands.ts`, `C/toast.ts`, `C/composer-controls-view.ts`, `C/client.ts` (one dispatch line, unforced delete), their `*.test.ts`.
Required environment: Xcode 27.0, pinned Bun and Hermes, oracle build, isolated lane backends (ports 16000–16999), `repos/pr-demo-origin.git`; attended row: network. Never port 3773, `~/.t3` or the `t3code` scheme. Attended and normal-launch rows run a lane build with `T3_LOCAL_HOME=<lane>/t3-home` and `T3_LOCAL_PORT=<lane port 16xxx>`; dev and lane builds refuse the real `~/.t3` and port 3773 by default (`20261005-embedded-server-runtime`).

## Progress

Implemented 2026-10-06 on `feat(example)/t3-code-live-automations-and-clones` (from the feature
branch after hot-file-split #147). Verification: unverified (no independent review, no oracle).

- `scheduled-tasks.ts`: ports of `matchesScheduledTaskScope`, `resolveSettingsScope` (as
  `resolveTaskScope`), `validateScheduledTasksSearch`, `taskToDraft`, `scheduledTaskDefaultModel`,
  `scheduleLabel`, `relativeLabel`; `now` is an argument (X19 #124). The reference's 8 logic tests are
  ported with their names (`scheduled-tasks.test.ts`, 22 cases with the `each` rows).
- `live-streams.ts`: `scheduledTasks.subscribe` and `subscribeProjectClones` (clones only with
  `projectCloneTracking`) per environment: the focused one over T3Client's transport (one dispatch line
  in `client.ts` drain), every background one over its fleet transport (`liveFleetEvent` in the fleet
  drain, `liveFleetPass` after it). Two more streams per transport (16-stream cap, X21 #126). A write
  checks the live list; `scheduledTasks.list` runs only before a stream's first value.
- Settings › Scheduled tasks (`scheduled-view.ts`, `settings-scheduled.contract`): a section per
  environment in scope (heading only above one), loading / error / "Environment disconnected", the
  scope filter, the `link|<env>|<task>` deep link that opens the editor once ("Task unavailable" when
  missing), 60 s refresh from the existing clock; row writes go to the task's own environment
  (`scheduled-tasks-commands.ts`).
- Thread details › Automations (`thread-automations.ts/.contract`) between Version Control and
  Lineage: status dot (pulses while running, still under reduced motion), title, schedule line,
  Manage scheduled tasks, Edit (deep link), Run now (disabled while running or busy), Pause/Resume
  switch; failures toast "Could not run automation" / "Could not update automation".
- Tracked clones (`project-clones.ts`, `project-clones-live.ts`): the palette calls
  `projectClone.start` when the server tracks clones, closes and opens the draft; the toast
  coordinator runs from the shell build (loading + Cancel, 8 s success + Open project, error / info +
  Retry + Remove project, steps aside while the draft is open); the composer banner and the send
  block ("Cloning repository", "Repository not cloned"); Remove project is an unforced
  `project.delete`. `updateToast` now patches kind, timeout, second button and copy button; a changed
  timeout restarts the toast's timer.

Left / limits: the editor's "Runs on" does not switch environment for a new task; Open project on a
background environment's clone toast is not offered (no focus-a-project path; the reference opens
it); action buttons close a toast here, so a toast closed by Retry returns on the next change; icon
buttons have no tooltips; hover and focus rings unverified (attended, X13); a deep link that found no
task leaves `restEditor` set until the next settings write.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| 1 (2026-10-06) | `0c0a95a4a` on `da40e6590` | `bun test examples/t3-code` 1364 pass / 0 fail (base 1321); strict tsc clean; `contract build` 2196 slots, 43 resources, 48535 nodes; `cargo test -p t3-code-macos --lib` 10 pass; caps within budget; five checks green (build, test, clippy, fmt, caps, boot); macOS bundle builds. No Swift changed, so no AppKit binary is touched | Live drive (lane servers 16170 after / 16171 before, isolated HOME, a seeded thread with two bound tasks): Automations section rows `Nightly triage` "Every 60 min · next in 278d" (agent clock), `Weekly summary` "Weekdays at 09:00 · paused", labels Edit / Run … now / Pause / Resume; `tap details-automation-run-…` started the run on the thread and Settings › Scheduled tasks then showed `succeeded` from the stream with no reload; Edit opened the editor through the deep link (`scheduled-task-dialog`, "Edit task", its fields). Before/after pairs 01 and 02 in the PR | Tracked clone live flow unverified: both drives stopped before it (drive 1: the sidebar was inert after the welcome import; drive 2: `tap add-project` did not open the palette after Settings closed, op 32/48). Clone toasts, banner, send block, Cancel/Retry/Remove covered by unit tests only. Not run: oracle and trace-diff (desktop-oracle-and-trace not built), second environment, old-server path, keyboard focus and reduced-motion transcript, attended rows |

## Next action

### Evidence correction (2026-10-06)

The earlier `succeeded` observation proves the scheduled-task status reached the UI,
not that a provider completed the prompt. The retained lane-after provider log
(`target/lane-after/t3-home/userdata/logs/provider/events.fa62d03f-0b88-408f-9fb2-a1ae6a85aec5.log`,
lines 18–30) records the Codex run on `automations-demo` failing with HTTP 401,
`Missing bearer or basic authentication in header`, followed by `turn/completed`
with status `failed`. An authenticated provider completion and a timed interval
execution remain unverified. The unit tests in `live-automations.test.ts` use a
recording fake client, including its `authenticated` provider state; they require
no connected provider account.

Merge validation against `origin/feat(example)/t3-code`: both `projectCloneBlock`
and `highlightPending` imports are retained in `presentation.ts`. With the pinned
Bun 1.4.2 the app suite has 1421 passing tests, one skipped generator test and no
failures. The initial run used PATH's Bun 1.3.14, which failed one HTML comment
highlighting test on both this merge and an isolated export of the base branch;
using the already installed pinned version resolves that failure.
Strict TypeScript checking, Contract build, the five repository checks, 2928
repository Rust tests and 10 macOS Rust tests pass. The macOS application bundle
also builds. These merge checks do not add authenticated provider or timed-run
coverage to the original evidence.

### Handoff: authenticated automation verification

Resume from [PR #156](https://github.com/ccheever/exact2/pull/156). Read its current
verification note and this task's acceptance table. Keep `verification: unverified`
until the remaining required acceptance rows pass. Provider execution and scheduling
belong to the T3 server; a missing credential is not evidence of an Exact framework bug.

1. Build the current app with Bun 1.4.2 and start an isolated T3 backend. Previous
   fixture ports were 16170/16171 and its data was under `target/lane-after` and
   `target/lane-before`; these ignored artifacts may no longer exist. Recreate the
   disposable project/thread/tasks if needed. Do not use the production `~/.t3` or
   port 3773. Record app/server revisions, origin, provider instance/model and UTC time.
2. Check authentication in the exact provider process and credential home used by
   that backend. A signed-in interactive Codex session elsewhere does not prove this
   process is authenticated. Complete sign-in with the user if required; do not copy
   credentials from another agent or include tokens in evidence. First send a bounded
   ordinary prompt such as `Reply exactly AUTOMATION_OK; do not run tools or edit files.`
   Require an actual assistant response and successful provider completion.
3. Create a disposable task bound to the test thread with that prompt. Run it from
   both Thread details > Automations and Settings > Scheduled tasks. Record task,
   thread and provider turn IDs, RPC, final assistant response, provider terminal
   status, server run status and UI updates without reload. A successful `runNow`
   RPC or a `succeeded` label alone does not meet the completion criterion.
4. Enable a short interval task and observe a real server-clock deadline without
   pressing Run now. Record scheduled time, actual start, completed response,
   run count and next run. The UI agent's `clock` changes do not advance the backend
   scheduler. Pause across a due interval and verify no new turn; resume and verify
   the next due run. Disable/delete only the test tasks after the check.
5. In a separate disposable unauthenticated environment, reproduce an authentication
   failure. Correlate the task/run/thread/turn IDs and timestamps with the server
   stream and both UI views. Investigate the earlier `succeeded`/provider `failed`
   discrepancy; its cause and whether the records name the same run are unknown.
   Determine whether the server status means dispatch or actual provider completion.
   If a change is needed, fix the owning layer and verify against the reference;
   do not invent a client success state or claim a server fix without evidence.
6. Preserve sanitized transcripts and evidence links in the PR and this task,
   including remaining failures. Logs under `target/` are local and not durable
   handoff artifacts. Never infer current account identity from the old 401 log.

Automation acceptance requires real manual and scheduled provider completions plus
consistent, understood status/error behavior. It does not close the other task rows:
tracked-clone success/cancel/failure/retry/remove, a second environment, the old-server
path, keyboard/Escape/reduced-motion and reference/trace checks remain open as listed
above. Resume those separately after authentication is available.
