---
name: 20261011-scheduled-tasks-followups
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: unverified
delivery: draft-pr
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-scheduled-tasks-followups
pr_url: https://github.com/ccheever/exact2/pull/456
verified_commit: null
---

# Scheduled tasks follow-ups: Runs on, Open project on another environment's clone, tooltips

## Outcome

The user decided on 2026-10-11 to build now the three items
[live-automations-and-clones](closed/20261005-live-automations-and-clones.md) left ("Left / limits"), matching the
reference T3 Code.

| Id | Clone | Reference (check it first) |
| --- | --- | --- |
| ST-1 | Settings › Scheduled tasks › New task: choosing another environment under "Runs on" does nothing (the editor's `choose("environment", …)` has no branch); the choices list every connected environment, not the settings scope's | `ScheduledTaskEditorDialog` "Runs on": the scope's connected environments; a choice switches the task's environment and resets project, model, base branch (`main`), Start from origin (on) and the checkout path, keeping name, prompt, schedule, workspace and Enabled; the project list, the model choices and the base branch picker are that environment's |
| ST-2 | The toast after a clone on a background (not focused) environment finishes ("Cloned X") offers no "Open project" (no path to open another environment's project) | `ProjectCloneToastCoordinator` offers "Open project" on every environment's success toast; it closes the toast and opens a new thread draft for that project on its environment (`handleNewThread(scopeProjectRef(environmentId, projectId))`) |
| ST-3 | Thread details › Automations: the Manage, Edit and Run now icon buttons show no tooltip | `ThreadAutomationsPanel`: `TooltipPopup` "Manage scheduled tasks", "Edit automation", "Run now" (Base UI tooltip, side top, 600 ms) |

## Steps

1. Reference first (source under `target/t3-ref/src-1e2ecbd975`; the reference app on the lane where a second
   environment is reachable).
2. ST-1: the editor keeps its fields across a Runs on change (the dialog stays mounted, as the reference's `setDraft`);
   the page resource reads the chosen environment's projects, models and refs; the environment list is the scope's
   connected environments.
3. ST-2: the success toast's Open project names the clone's environment and project; on a background environment it
   moves the window to that environment, on that project's draft (the reconnect `focusFleetThread` and the pull request
   hand-off already do).
4. ST-3: the three tooltips on the window's hover layer (the details panel is a scroll area), with Base UI's 600 ms
   open delay.
5. Tests; one agent drive with a second lane server (port base+3) paired through Settings › Connections; before / after
   / reference images.

## Acceptance

| Row | How to verify | Before/after |
| --- | --- | --- |
| ST-1 | reference comparison; Bun tests of the page resource and the editor's contract; agent drive: New task, Runs on the second environment, the project list and base branch follow, the typed name stays | before / after / reference images |
| ST-2 | reference comparison; Bun tests of the toast and of the `new-thread` op with another environment's project; agent drive: a clone on the background environment finishes, Open project opens its draft there | before / after images |
| ST-3 | reference comparison; Bun test; agent drive: hover each button, the tooltip after its delay | before / after / reference images |

## Reference

- ST-1: `apps/web/src/components/settings/ScheduledTasksSettings.tsx` `ScheduledTaskEditorDialog` (the Runs on
  `Select`'s `onValueChange`, `projects` filtered by `environmentId`, `useEnvironmentSettings(environmentId)`,
  `providersValueAtom(environmentId)`, `WorktreeBaseBranchPicker key={environmentId:projectId}`),
  `settings/SettingsScopeContext.tsx` and `settings/scopedSettings.ts` `selectScopedSettingsEnvironments`
  (`connectedEnvironments`: the scope's environments that are connected).
- ST-2: `apps/web/src/components/ProjectCloneToastCoordinator.tsx` (`EnvironmentCloneToasts` per environment,
  `openProject` → `useNewThreadHandler`), `hooks/useHandleNewThread.ts`.
- ST-3: `apps/web/src/components/chat/ThreadAutomationsPanel.tsx` (three `Tooltip`s), `components/ui/tooltip.tsx`
  (`TooltipPopup` side top, sideOffset 4, Base UI's default 600 ms delay).

## Cause and fix

**ST-1.** The editor's `choose("environment", value)` had no branch, and the page resource (`scheduled-view.ts`
`scheduledPage`) chose the editing environment from the row or deep link only, so a new task always stayed on the
focused environment; its Runs on choices were every connected environment. Fix: a Runs on choice of a new task resets
project, base branch (`main`), Start from origin (on) and the checkout path in the dialog (`settings-scheduled.contract`
`ScheduledEditor`, as the reference's `setDraft`), and reopens the editor on `new|<environmentId>` through the window's
`restOpen` (`runsOn`), which also clears the model pick and the picker's search; `editTarget` reads `new|<environment>`
as a new task there, and a new task's dialog key is `new` whatever its environment (the reference keys the dialog by
the environment it opened on), so the dialog stays mounted and keeps name, prompt, workspace, schedule and Enabled. The
project the dialog shows is `chosenProject` (the chosen one, else the environment's first: `selectedProjectId`), which
every reader uses (the Project select, the base branch picker and its lookups, Save). The page reads that environment's
projects, default model and refs (over its fleet transport for a background environment); Runs on lists the settings
scope's connected environments (`selectScopedSettingsEnvironments`). A new task closing after a change gives the focus
back to New task (`ProjectDialogs` `DialogFinalFocus`).

**ST-2.** `cloneToast` offered Open project only for the focused environment (`openable`), because the clone had no
path to open another environment's project. Fix: every success toast's Open project is `new-thread` with the project
named on its environment (`fleet:<environmentId>:<projectId>`, the sidebar's form); `client-ops-threads.ts` opens the
focused environment's project here and moves the window to another environment's project draft
(`settings-b-fleet.ts` `focusFleetProject`: that environment's selection is the project's draft, its fleet transport
stops and T3Client connects there, the reconnect `focusFleetThread` already does, now shared as `refocus`). The qualified
id also keeps a toast created while another environment was focused pointing at its own environment.

**ST-3.** The three `AutomationButton`s had `title=""` and no tooltip. Fix: each hands its frame to the window's hover
layer (`hover-layer.contract`, a new kind `tooltip` with Base UI's 600 ms open delay, drawn as a tip that takes no
pointer) on pointer hover and keyboard focus, side top, centred, as `PolicyTip` does; the words are the reference's
TooltipPopups "Manage scheduled tasks", "Edit automation", "Run now". The details panel is a scroll area, so the layer
(the reference portals every TooltipPopup) keeps the panel from cutting the tooltip.

## Tests

- `scheduled-tasks-followups.test.ts` (10): ST-1 `editTarget('new|env2')`; the page resource with a background
  environment "Studio" (env2) in the window's fleet: the editor on `new|env2` is Studio's (key `new`, its project `q1`,
  its default model, base `main`, origin on, no path, not missing), its project list, Runs on with Studio selected, the
  refs read over Studio's transport and the reset base looked up by name there; Runs on is the scope's connected
  environments; the dialog's contract (the reset branch, `runsOn`, `chosenProject` for every reader, the wiring through
  `SettingsRestDialogs`, `ProjectDialogs` and `T3Window`, the final focus); the Runs on, Base branch and Settings select
  labels keyed by their words with `title=""` on their triggers (X64 #316 / X82). ST-2 the toast's Open project on a
  background environment (`cloneToasts` over the fleet entry's clone stream); `new-thread` with
  `fleet:env2:q7` sets Studio's selection to the project's draft, stops its fleet transport, connects there and adopts
  the status; the focused environment's qualified project opens here with no reconnect. ST-3 the button's
  `hoverTipAt` call (kind `tooltip`, top, centre), hover and focus, `title=""`, the 600 ms delay, the three words and their alignment (Manage `end`, the rows `center`).
- Updated: `project-clones.test.ts` (Open project's qualified id, no `openable`), `live-automations.test.ts` and
  `audit-wave-followups-2.test.ts` (a new task's key `new`, the picker's `chosenProject`), `hover-layer.test.ts` (the
  `tooltip` kind), `settings-labels.test.ts` (the keyed base label), `dialog-trap.test.ts` (the final focus of a
  `new|<environment>` editor).

## Acceptance results

| Row | Result | Proof |
| --- | --- | --- |
| ST-1 | pass (agent drive, reference): before, choosing Studio under Runs on leaves the dialog on the Mac (Runs on, "Verification fixture", the fixture's "No refs found."); after, Runs on reads Studio, the typed name stays, Project is Studio's `studio-app`, Base branch "From origin/main" over Studio's refs (main current, develop, release/2026-10), Model "Choose model" (Studio has no provider), as the reference; the tree reads "Studio", "studio-app", "From origin/main" and the page resource `new|<Studio>`, key `new` | [st1-runs-on.png](https://raw.githubusercontent.com/ccheever/exact2/4faca56f7b584b7ca6c052d550e73520f0f8db6a/scheduled-tasks-followups/st1-runs-on.png), [st-record.txt](https://raw.githubusercontent.com/ccheever/exact2/4bb388e76ed08d204b76d7def203e1ce350d6000/scheduled-tasks-followups/st-record.txt) |
| ST-2 | pass (agent drive, reference): a clone on the background environment Studio fails, Retry from its toast finishes it; before, "Cloned widgets" has no button; after, it offers Open project, which closes the toast and moves the window to Studio on the widgets draft ("What should we build in widgets?", the strip "Studio · main"), as the reference | [st2-open-project.png](https://raw.githubusercontent.com/ccheever/exact2/45164c8e0273e4c7154ec99210266bc97f3501fe/scheduled-tasks-followups/st2-open-project.png), [st-record.txt](https://raw.githubusercontent.com/ccheever/exact2/4bb388e76ed08d204b76d7def203e1ce350d6000/scheduled-tasks-followups/st-record.txt) |
| ST-3 | pass for Edit and Run now (agent drive, reference): before, a hovered button shows no tooltip; after, "Edit automation" and "Run now" show 700 ms into the hover, none 300 ms into a new hover (the reference's 600 ms: none at 300 ms, the words at 1000 ms). "Manage scheduled tasks" showed its words but, centred over the heading's button at the window's right edge, ran past the window; fixed after the drive (`2890e2d30`, end-aligned, Bun test), not driven again (the one retry was used): real-input step 3 | [st3-tooltips.png](https://raw.githubusercontent.com/ccheever/exact2/94f4b8026767dade42dc120f76eb69cf55ee5709/scheduled-tasks-followups/st3-tooltips.png) |

Drives (agent mode, 1280×840, `screenshot … window`): the clone's embedded lane server on 16422 and a second lane T3 server
"Studio" (the staged 0.0.46 runtime, its own home, no provider CLI, a `scutil` shim for its label) on 16423, holding a
project `studio-app` (branches main, develop, release/2026-10), a thread "Release notes" with two scheduled tasks bound to
it (one paused, one weekdays at 09:00; none ran) and a clone whose remote was missing (failed; the remote made afterwards,
so Retry finishes it); Studio paired through Settings › Connections with a fresh pairing link each drive. Before: the
evidence worktree at `6db5cadea`. After: this branch's bundle; the first drive (`1e1e0f481`'s app code) showed the page
resource on Studio while the Runs on, Project and Base branch labels kept the Mac's words (X64 #316 / X82: a rastered
label under a button changed in place), fixed in `468cf3ea2` (each label keyed by its words), whose bundle the retry
drove; every after image is from the retry. Reference: T3 Code 1e2ecbd975 (Electron) on the lane (backend 16420, CDP
16421), paired with the same Studio. The lane helper (start, seed, clean, pair, stop) is
`target/lane/studio.mjs` in this worktree, not committed.

## Real-input batch steps

Launch this branch's bundle as a lane copy (its own bundle id, the lane home outside any git checkout); start a second
server as the drive did (`cd <this worktree>/target/lane && bun studio.mjs start probe 16423 && bun studio.mjs seed probe
16423`, then `bun studio.mjs pair probe 16423` writes a pairing link to a file) and pair it in Settings › Connections ›
Add environment (paste the link into Host). Never send a message; never press Run now.

1. **ST-1.** Settings › Scheduled Tasks › New task; type a name; Runs on › Studio (a real click). Expected: Runs on
   reads Studio, the name stays, Project reads studio-app, Base branch "From origin/main"; open Project and Base branch:
   Studio's project and refs. Then Runs on › the Mac: the Mac's project and base again, the name still there. Cancel.
2. **ST-2.** Studio's "Failed to clone widgets" toast › Retry (a real click), then "Cloned widgets" › Open project.
   Expected: the window moves to Studio on "What should we build in widgets?".
3. **ST-3.** Studio's "Release notes" thread, thread details › Automations: rest the real pointer on the heading's
   Manage button, then a row's Edit, then Run now. Expected: after about 0.6 s "Manage scheduled tasks" (whole, inside
   the window, its right edge at the button's), "Edit automation", "Run now"; Tab to each button shows the same.

## Seen while building, not a finding row

- After Open project the clone's details card read "Changes +447358 −1" for the fresh widgets checkout, the
  reference's "+0 −0": Studio's own `subscribeVcsStatus` reports `branchChanges` {baseRef origin/main, 447358, 1} and a
  clean `workingTree`; the clone shows `branchChanges` (a later upstream rule, `shell-details.ts`), the reference's pin
  the working tree (the server's number for a `file://` remote, recorded in `st-record.txt`).
- The reference's Runs on menu items carry each environment's machine icon; the clone's carry none.
- Within the server's 30 s retention of a finished clone (`ProjectCloneTracker.ts` `DONE_RETENTION`), leaving the
  project's draft brings "Cloned widgets" back (the coordinator re-adds a success toast for a clone it no longer
  tracks once the draft is not viewed); the reference's coordinator does the same.

## Checks

The code at `0310dd8b7` (the final head adds this record only). No Rust and no Swift changed.

| Check | Exit | Note |
| --- | --- | --- |
| `bun test examples/t3-code --timeout 60000` | 0 | 4663 pass, 1 skip, 0 fail (312 files); the first run at `2890e2d30` failed 3 tests of `audit-wave-followups-2.test.ts` that pinned the old new-task key and `projectId` in the picker, fixed in `0310dd8b7` |
| strict `tsc` (README command) | 0 | |
| `bun scripts/exact.mjs contract build examples/t3-code/app.contract` | 0 | 6157 slots, 48 resources, 110195 nodes, 29,002,795 bytes; `app.contract` 1,398 lines (unchanged) |
| `git add -A && bun scripts/caps.mjs` | 0 | all budgets within cap |
| `cargo build --all-targets --keep-going` | 0 | at `2890e2d30` |
| `cargo test --lib --bins --tests --no-fail-fast` | 0 | 3,679 passed, 0 failed, at `2890e2d30` |
| `cargo clippy --all-targets --keep-going -- -D warnings` | 0 | at `2890e2d30` |
| `cargo fmt --all -- --check` | 0 | at `2890e2d30` |
| `bun scripts/boot.mjs` | 0 | at `2890e2d30` |
| `cargo test -p t3-code-macos --lib`, AppKit binaries | not run | no Rust and no Swift changed |
| `host/apple/build.mjs t3-code-macos --bundle` | 0 | twice: `1e1e0f481` (first drive), `468cf3ea2` (the retry); `2890e2d30`'s tooltip alignment is not in a driven bundle |

## Not done / not verified

- ST-3's "Manage scheduled tasks" placement after its fix (`2890e2d30`, end-aligned): Bun test only; the brief allows one
  live drive and one retry, both used (the retry found it). Real-input step 3 checks it.
- ST-1, ST-2 and ST-3 under a real pointer: real-input steps 1-3.

## Delivery

Draft pull request [#456](https://github.com/ccheever/exact2/pull/456) into `feat(example)/t3-code`.

## Next action

The coordinator reviews the draft PR; real-input steps 1-3 in the next batch.
