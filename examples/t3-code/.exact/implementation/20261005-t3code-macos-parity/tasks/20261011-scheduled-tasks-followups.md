---
name: 20261011-scheduled-tasks-followups
plan: 20261005-t3code-macos-parity
implementation: in-progress
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-scheduled-tasks-followups
pr_url: null
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

- `scheduled-tasks-followups.test.ts` (9): ST-1 `editTarget('new|env2')`; the page resource with a background
  environment "Studio" (env2) in the window's fleet: the editor on `new|env2` is Studio's (key `new`, its project `q1`,
  its default model, base `main`, origin on, no path, not missing), its project list, Runs on with Studio selected, the
  refs read over Studio's transport and the reset base looked up by name there; Runs on is the scope's connected
  environments; the dialog's contract (the reset branch, `runsOn`, `chosenProject` for every reader, the wiring through
  `SettingsRestDialogs`, `ProjectDialogs` and `T3Window`, the final focus). ST-2 the toast's Open project on a
  background environment (`cloneToasts` over the fleet entry's clone stream); `new-thread` with
  `fleet:env2:q7` sets Studio's selection to the project's draft, stops its fleet transport, connects there and adopts
  the status; the focused environment's qualified project opens here with no reconnect. ST-3 the button's
  `hoverTipAt` call (kind `tooltip`, top, centre), hover and focus, `title=""`, the 600 ms delay and the three words.
- Updated: `project-clones.test.ts` (Open project's qualified id, no `openable`), `live-automations.test.ts` (a new
  task's key `new`), `hover-layer.test.ts` (the `tooltip` kind).
