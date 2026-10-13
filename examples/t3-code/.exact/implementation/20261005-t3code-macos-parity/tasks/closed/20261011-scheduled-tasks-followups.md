---
name: 20261011-scheduled-tasks-followups
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: partial
delivery: merged
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-scheduled-tasks-followups
pr_url: https://github.com/ccheever/exact2/pull/456
verified_commit: 1f813789b
---

# Scheduled tasks follow-ups: Runs on, Open project on another environment's clone, tooltips

## Outcome

The user decided on 2026-10-11 to build now the three items
[live-automations-and-clones](20261005-live-automations-and-clones.md) left ("Left / limits"), matching the
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
(the reference portals every TooltipPopup) keeps the panel from cutting the tooltip. "Manage scheduled tasks", centred
over the heading's button at the panel's right edge, ran past the window (the layer has no collision shift, X17 #112):
it ends at the button's right edge (`align="end"`, `2890e2d30`). A press closes the tooltip, or stops one still waiting
for its delay, before the button acts (`AutomationButton` `pressed`: `show(false)`, then `press`), as Base UI 1.5.0's
`TooltipTrigger` `closeOnClick` (true by default; `onPointerDown`/`onClick` cancel a pending open, the root's
`referencePress` dismiss closes an open one) does, so Edit's and Manage's never linger over the Settings page they open
(the hover layer draws above it); it shows again when the pointer next enters (review round 1, `cb3429ae1`).

## Tests

- `scheduled-tasks-followups.test.ts` (11): ST-1 `editTarget('new|env2')`; the page resource with a background
  environment "Studio" (env2) in the window's fleet: the editor on `new|env2` is Studio's (key `new`, its project `q1`,
  its default model, base `main`, origin on, no path, not missing), its project list, Runs on with Studio selected, the
  refs read over Studio's transport and the reset base looked up by name there; Runs on is the scope's connected
  environments; the dialog's contract (the reset branch, `runsOn`, `chosenProject` for every reader, the wiring through
  `SettingsRestDialogs`, `ProjectDialogs` and `T3Window`, the final focus); the Runs on, Base branch and Settings select
  labels keyed by their words with `title=""` on their triggers (X64 #316 / X82). ST-2 the toast's Open project on a
  background environment (`cloneToasts` over the fleet entry's clone stream); `new-thread` with
  `fleet:env2:q7` sets Studio's selection to the project's draft, stops its fleet transport, connects there and adopts
  the status; the focused environment's qualified project opens here with no reconnect. ST-3 the button's
  `hoverTipAt` call (kind `tooltip`, top, centre), hover and focus, `title=""`, the 600 ms delay, the three words and
  their alignment (Manage `end`, the rows `center`); a press calls `show(false)` before `press()`, and the window's close
  of a `tooltip` ends a shown one at once and a waiting one before its delay.
- Updated: `project-clones.test.ts` (Open project's qualified id, no `openable`), `live-automations.test.ts` and
  `audit-wave-followups-2.test.ts` (a new task's key `new`, the picker's `chosenProject`), `hover-layer.test.ts` (the
  `tooltip` kind), `settings-labels.test.ts` (the keyed base label), `dialog-trap.test.ts` (the final focus of a
  `new|<environment>` editor).

## Acceptance results

| Row | Result | Proof |
| --- | --- | --- |
| ST-1 | pass (agent drive, reference): before, choosing Studio under Runs on leaves the dialog on the Mac (Runs on, "Verification fixture", the fixture's "No refs found."); after, Runs on reads Studio, the typed name stays, Project is Studio's `studio-app`, Base branch "From origin/main" over Studio's refs (main current, develop, release/2026-10), Model "Choose model" (Studio has no provider), as the reference; the tree reads "Studio", "studio-app", "From origin/main" and the page resource `new|<Studio>`, key `new` | [st1-runs-on.png](https://raw.githubusercontent.com/ccheever/exact2/4faca56f7b584b7ca6c052d550e73520f0f8db6a/scheduled-tasks-followups/st1-runs-on.png), [st-record.txt](https://raw.githubusercontent.com/ccheever/exact2/4bb388e76ed08d204b76d7def203e1ce350d6000/scheduled-tasks-followups/st-record.txt) |
| ST-2 | pass (agent drive, reference): a clone on the background environment Studio fails, Retry from its toast finishes it; before, "Cloned widgets" has no button; after, it offers Open project, which closes the toast and moves the window to Studio on the widgets draft ("What should we build in widgets?", the strip "Studio · main"), as the reference | [st2-open-project.png](https://raw.githubusercontent.com/ccheever/exact2/45164c8e0273e4c7154ec99210266bc97f3501fe/scheduled-tasks-followups/st2-open-project.png), [st-record.txt](https://raw.githubusercontent.com/ccheever/exact2/4bb388e76ed08d204b76d7def203e1ce350d6000/scheduled-tasks-followups/st-record.txt) |
| ST-3 | pass (agent drive, reference): before, a hovered button shows no tooltip; after, "Manage scheduled tasks", "Edit automation" and "Run now" show 700 ms into the hover, none 300 ms into a new hover (the reference's 600 ms: none at 300 ms, the words at 1000 ms). Manage's bubble ends at its button's right edge, inside the window (review round 1 drive at `4daf099f1`; the first retry's centred bubble ran past it). Pressing Edit, then Manage, with the tooltip shown opens Settings › Scheduled Tasks with no tooltip (the window's tree has no hover layer), as the reference after a click | [st3-tooltips-r1.png](https://raw.githubusercontent.com/ccheever/exact2/0b18e11e9fdaf8fcee9870dc17ef254cf52a63ec/scheduled-tasks-followups/st3-tooltips-r1.png), [st3-press-closes.png](https://raw.githubusercontent.com/ccheever/exact2/a08f17a9a5939d398bf60f40890fa1c446548732/scheduled-tasks-followups/st3-press-closes.png), [st3-r1-record.txt](https://raw.githubusercontent.com/ccheever/exact2/59bac6b2716359525a2db123b94ed4080734d05f/scheduled-tasks-followups/st3-r1-record.txt); first drive [st3-tooltips.png](https://raw.githubusercontent.com/ccheever/exact2/94f4b8026767dade42dc120f76eb69cf55ee5709/scheduled-tasks-followups/st3-tooltips.png) |

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

Review round 1 drive (ST-3 only, `target/lane/drive2.sh`): this branch's bundle at `4daf099f1` (the press fix and the
merge of the feature branch at `fe0e39c91`), Studio started and paired as before, its "Release notes" thread: hover
Manage, Edit and Run now 700 ms each (screenshots, a tree read after Manage and Edit), then press Edit with its tooltip
shown and read the tree in Settings, cancel, close Settings, hover Manage and press it. The reference, on the same
Studio over CDP: hover Edit and Manage 1000 ms each, then click each.

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
   the window, its right edge at the button's), "Edit automation", "Run now"; Tab to each button shows the same. With
   "Edit automation" shown, click Edit: Settings › Scheduled Tasks opens on the task's editor with no tooltip over it
   (Cancel; do the same with Manage). Never click Run now.

## Seen while building, not a finding row

- After Open project the clone's details card read "Changes +447358 −1" for the fresh widgets checkout, the
  reference's "+0 −0": Studio's own `subscribeVcsStatus` reports `branchChanges` {baseRef origin/main, 447358, 1} and a
  clean `workingTree`; the clone shows `branchChanges` (a later upstream rule, `shell-details.ts`), the reference's pin
  the working tree (the server's number for a `file://` remote, recorded in `st-record.txt`).
- The reference's Runs on menu items carry each environment's machine icon; the clone's carry none.
- After Manage the clone's Scheduled Tasks page read "across All environments", the reference's "on Studio": the
  reference's Manage passes `environmentId`, which its settings scope does not read (`settingsScope.ts` reads `machine`),
  so its scope was the one an earlier drive left; not compared further.
- The review round 1 drive showed the clone's "Nightly needs the beta mobile app" notice (drawn above the Edit task
  dialog); the reference (an Alpha build) has none. Not this task.
- Within the server's 30 s retention of a finished clone (`ProjectCloneTracker.ts` `DONE_RETENTION`), leaving the
  project's draft brings "Cloned widgets" back (the coordinator re-adds a success toast for a clone it no longer
  tracks once the draft is not viewed); the reference's coordinator does the same.

## Checks

Review round 1, on the code at `4daf099f1` (the press fix `cb3429ae1` and the merge of the feature branch at `fe0e39c91`;
the final head adds this record only). No Rust and no Swift changed in this task.

| Check | Exit | Note |
| --- | --- | --- |
| `bun test examples/t3-code --timeout 60000` | 0 | 4672 pass, 1 skip, 0 fail (312 files) |
| strict `tsc` (README command) | 0 | |
| `bun scripts/exact.mjs contract build examples/t3-code/app.contract` | 0 | 6161 slots, 48 resources, 110198 nodes, 29,206,506 bytes; `app.contract` 1,398 lines (unchanged) |
| `git add -A && bun scripts/caps.mjs` | 0 | all budgets within cap |
| `cargo build --all-targets --keep-going` | 0 | |
| `cargo test --lib --bins --tests --no-fail-fast` | 101 | 3,744 passed, 1 failed, 36 ignored (95 suites). The one failure is inherited from main through the feature branch: the `button_migration` sweep's `apps/color-gallery/app.contract:269:17: styled default button needs appearance="none"` (main #440), recorded by adopt-main-fixes-r9 (AD-1); this branch changes nothing outside `examples/t3-code` against the feature branch |
| `cargo clippy --all-targets --keep-going -- -D warnings` | 0 | |
| `cargo fmt --all -- --check` | 0 | |
| `bun scripts/boot.mjs` | 0 | |
| `cargo test -p t3-code-macos --lib`, AppKit binaries | not run | no Rust and no Swift changed |
| `host/apple/build.mjs t3-code-macos --bundle` | 0 | three times: `1e1e0f481` (first drive), `468cf3ea2` (the retry), `4daf099f1` (review round 1 drive) |

Before review round 1 (the code at `0310dd8b7`): Bun 4663 pass, 0 fail; tsc, contract build, caps and the five checks exit 0
(the Rust checks at `2890e2d30`).

## Not done / not verified

- ST-1, ST-2 and ST-3 under a real pointer: ran in `realinput-1011c` ("Real-input results" below).

## Real-input results

`realinput-1011c` (2026-10-12 21:43 to 10-13 20:17 KST, five sittings; the attended session for the verify-1011c groups g1–g5 and the rows the 2026-10-11/12 merges brought) ran on the bundle of `1f813789b`, the T3 tip after #497, which holds this task: one lane copy per batch beside the reference T3 Code `1e2ecbd975`, real keys and pointer, light and dark from each lane app's own Appearance setting, never System Settings. Each row got one pass; failures were recorded, not retried. Session notes: [00-session-notes.txt](https://raw.githubusercontent.com/ccheever/exact2/a94ddd5cdbf485f1f57dc92c965c46c4bb17c8be/realinput-1011c/00-session-notes.txt).

| Step | Session row | Result | Evidence |
| --- | --- | --- | --- |
| 1, ST-1 | STF-1 (B11) | Pass | [STF-1-studio.png](https://raw.githubusercontent.com/ccheever/exact2/dbf3b209457122ad13ae8928de9538c888000b5d/realinput-1011c/b11-studio/STF-1-studio.png) |
| 2, ST-2 | STF-2 (B11) | Partial: the failure toast, Retry, the success toast and its closing pass; Open project pressed while Settings is open leaves Settings in front (the view behind moves to Studio's widgets draft), where the reference's navigation replaces the Settings route | [STF-2-open-project.png](https://raw.githubusercontent.com/ccheever/exact2/dbf3b209457122ad13ae8928de9538c888000b5d/realinput-1011c/b11-studio/STF-2-open-project.png) |
| 3, ST-3 | STF-3 (B11) | Partial: the pointer tooltips and both presses pass; the Tab half not reachable: Manage, Edit and Run now are no Tab stops with the Mac's Keyboard navigation off | [STF-3-tab.txt](https://raw.githubusercontent.com/ccheever/exact2/dbf3b209457122ad13ae8928de9538c888000b5d/realinput-1011c/b11-studio/STF-3-tab.txt) |

Not passed: STF-2's Open project over Settings; STF-3's Tab half (the Mac's setting, not run with it on).

## Delivery

Merged on 2026-10-12 as `3525e9f1a` (#456, squash) into `feat(example)/t3-code`.

## Review round 1

An independent review (2026-10-12) found two should-fix problems, both in ST-3:

| Finding | Fix | Proof |
| --- | --- | --- |
| The end-aligned Manage tooltip (`2890e2d30`) was in no driven bundle; the PR's after image showed the old centred bubble past the window | driven in agent mode at `4daf099f1` (`drive2.sh`): the bubble ends at its button's right edge, inside the window; the PR's ST-3 image replaced | [st3-tooltips-r1.png](https://raw.githubusercontent.com/ccheever/exact2/0b18e11e9fdaf8fcee9870dc17ef254cf52a63ec/scheduled-tasks-followups/st3-tooltips-r1.png) |
| A press left the tooltip up (Base UI 1.5.0's `TooltipTrigger` closes it, `closeOnClick` true by default), so Edit's or Manage's could linger over the Settings page they open | `AutomationButton` `pressed`: `show(false)`, then `press` (`cb3429ae1`); Bun test; agent drive: no hover layer after pressing Edit or Manage with the tooltip shown, as the reference after a click | [st3-press-closes.png](https://raw.githubusercontent.com/ccheever/exact2/a08f17a9a5939d398bf60f40890fa1c446548732/scheduled-tasks-followups/st3-press-closes.png), [st3-r1-record.txt](https://raw.githubusercontent.com/ccheever/exact2/59bac6b2716359525a2db123b94ed4080734d05f/scheduled-tasks-followups/st3-r1-record.txt) |

The feature branch (`fe0e39c91`, main adoption round 9 and progress-native-bars) is merged in (`4daf099f1`, no conflicts).

## Next action

None. Closed after `realinput-1011c`; STF-2 and STF-3's Tab half are in `STATUS.md` "Known differences" (the user's decision of 2026-10-11: no new tasks from real-input failures).
