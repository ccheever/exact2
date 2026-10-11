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
