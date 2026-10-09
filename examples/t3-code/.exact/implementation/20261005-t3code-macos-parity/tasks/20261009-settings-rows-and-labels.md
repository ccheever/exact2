---
name: 20261009-settings-rows-and-labels
plan: 20261005-t3code-macos-parity
implementation: planned
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: null
pr_url: null
verified_commit: null
---

# Settings rows and labels: confirmations, inheritance, policy tooltips, scheduled-task texts, Connections and Source Control names

## Outcome

Eight small Settings differences in text, tooltips and accessible names, and the Icon submenu's row alignment, match the
reference. Each is a row, label or
message; none changes a page's structure.

Found by the 2026-10-09 desktop audit ([review](../reviews/20261009-desktop-audit.md)). Reference: T3 Code `1e2ecbd975`
as an Electron production build. Clone: `c603c22d6`, a development build.

## Findings

Evidence paths are under the repository root. They stay local and are not committed.

| Id | Reference | Clone | Steps | Evidence |
| --- | --- | --- | --- | --- |
| S1-8 | "Restore default settings?" / "This will reset: Theme, Time format." with Cancel and **Confirm** (`ConfirmDialogHost.tsx:89-90`). | Same title and body; buttons Cancel and "Restore defaults" (`settings-core-body.contract:56`). | General › Time format › 24-hour › Restore device defaults. | `target/t3-audit/evidence/settings-1/S1-8-ref.png`, `S1-8-clone.png` |
| S1-9 | Background activity's inheritance popover: "… \| Environment \| Inherits \| Default \| Custom" (`SettingInheritance.tsx:55-79` has no profile branch and returns "Custom" for the object). | "… \| Default \| Balanced" (`settings-core.ts:264` maps `entry.profile` to a label). | General › Background activity › the layers icon ("Built-in default. Show where this value comes from"). | `target/t3-audit/evidence/settings-1/S1-9.txt` |
| S1-16 | Hovering the (i) next to Health check interval shows: "This interval is configured here, then the shared Background activity policy decides whether provider probes may run when the timer fires. Custom intervals appear as Advanced in General settings." | No tooltip on hover; the text shows as the row's status line only after a press (`providers.contract:247`, `policyOpen`). | Providers › Advanced › hover the (i) next to Health check interval. | `target/t3-audit/lanes/settings-1/dumps/c-run17.jsonl` |
| S2-7 | Git details: row title "Git fetch interval", then a "Background policy details" info button ("This interval is configured for Git only. The shared Background activity policy still decides…"). | Row title "Automatic Git fetch interval"; no info button (`settings-source-control.contract:297`; `settings-catalog.ts:93` already has "Git fetch interval"). | Source Control › Toggle Git details. | `target/t3-audit/evidence/settings-2/S2-7-ref.png`, `S2-7-clone.png`, `settings-2/clone/d6-vcs.tree.txt` |
| S2-3 | Toast title "Checkout path is required", description "Enter the path of the checkout to run in." | Title "Could not save scheduled task", description "Checkout path is required: Enter the path of the checkout to run in." (`scheduled-view.ts:150` throws the joined text; `shell-commands.ts:173` maps only `/existing checkout path/`). | New task: Name "Audit task", Prompt "Check things", Workspace "Use a specific checkout", Every interval 15, empty Checkout path, Create task. | `target/t3-audit/evidence/settings-2/S2-3-ref.png`, `S2-3-clone.png`, `settings-2/clone/d5-toast-path.tree.txt` |
| S2-6 | Base branch reads "From main" when the ref is not in the project's refs (Verification fixture, "No refs found."); "From origin/main" only for a known local branch. | Always "From origin/main" while Start from origin is on (`settings-scheduled.contract:359`). | New task with project Verification fixture: read the Base branch trigger. | `target/t3-audit/evidence/settings-2/S2-6-ref.png`, `S2-6-clone.png` |
| S2-8 | Host "not a url", pairing code "ABC": "Failed to fetch remote environment endpoint https://not%20a%20url/.well-known/t3/environment (HttpClientError: Transport error (GET https://not%20a%20url/.well-known/t3/environment))." | "invalid URL: invalid international domain name". | Connections › Add environment › Remote link › Host "not a url", Pairing code "ABC" › Add environment. | `target/t3-audit/evidence/settings-2/S2-8-ref.png`, `S2-8-clone.png` |
| S2-10 | switch "Default automatic pull"; combobox "Default pull request merge method"; buttons "Reset branch naming to default", "Reset source control writing style to default"; Icon items are `menuitemradio` with "Laptop detected" checked. | switch "Automatically pull"; combobox "Default merge method"; "Reset Worktree branch naming to default", "Reset Source control writing style to default"; Icon items are `menuitem` with no checked state. | Compare the ARIA (reference) with the agent tree (clone) on these controls. | `target/t3-audit/evidence/settings-2/ref/source-control.aria`, `settings-2/clone/source-control.tree.txt`, `settings-2/ref/cn-menu-icon.aria`, `settings-2/clone/d6-icon-menu.tree.txt` |
| S2-9 (part) | The Icon submenu's rows are left-aligned. | The rows' labels are centred. (The submenu's placement is X17, not this task.) | Connections › … › Icon. | `target/t3-audit/evidence/settings-2/S2-9-ref.png`, `S2-9-clone.png` |

## Scope and exclusions

Included: the findings above. S1-16 and S2-7 share one control: build one "Background policy details" info button with a
hover tooltip and use it in both rows.

Excluded:
- The Icon submenu's placement (S2-9): X17, Charlie's decision: T3 waits for main fix of
  [#112](https://github.com/ccheever/exact2/issues/112) and adds no per-site flip arithmetic
  ([X17](../issues/20261005-x17-popover-position-try.md), line 13; main file `issues/20261009-popover-css-flip-fallbacks.md`).
- The number fields (Days of inactivity, Run every, Port): X60, waits for main fix of #301.
- Framework code.

## Context and guidance

Reference (`target/t3-ref/src-1e2ecbd975`):
- S1-8: `apps/web/src/components/ConfirmDialogHost.tsx:89-90`.
- S1-9: `apps/web/src/components/settings/SettingInheritance.tsx:55-79`.
- S1-16, S2-7: `apps/web/src/components/settings/settingsLayout.tsx:150` (the shared info button);
  `apps/web/src/components/settings/SourceControlSettings.tsx:367-380`.
- S2-3: `apps/web/src/components/settings/ScheduledTasksSettings.tsx:576-578`.
- S2-6: `apps/web/src/components/BranchToolbar.logic.ts:240-246` (`origin/` only when `resolvedActiveBranchIsRemote === false`).
- S2-8: `packages/client-runtime/src/rpc/http.ts:156`.

Clone (`examples/t3-code`): `settings-core-body.contract:56`, `settings-core.ts:264`, `providers.contract:247`,
`settings-source-control.contract:297`, `settings-catalog.ts:93`, `scheduled-view.ts:150`, `shell-commands.ts:173-174`,
`settings-scheduled.contract:359`, the Connections machine menu, the Add environment error path. Use the clone's hover
layer (from fix-hover-cards) for the tooltip. For S2-9's rows use `text-align="left"`, as pr-list-title-clip did (X57).

Shared files: `settings-core.ts` with [settings-diagnostics-and-scope](20261009-settings-diagnostics-and-scope.md);
`settings-scheduled.contract` and `settings-source-control.contract` with [model-picker-parity](20261009-model-picker-parity.md).

## Acceptance

Before/after evidence: one side-by-side image per scenario (base build | branch build, same state,
`screenshot <abs.png> window`). Text rows may use tree text before/after instead.

| Id | How to verify | Before/after pair | Input |
| --- | --- | --- | --- |
| S1-8 | The confirm button reads "Confirm". | `s1-8-restore-confirm.png` | agent |
| S1-9 | The popover's last value reads "Custom". Unit test. | text: tree before/after | agent |
| S1-16 | Hover on the (i) shows the reference text as a tooltip. | `s1-16-policy-tooltip.png` | agent (hover) |
| S2-7 | The row reads "Git fetch interval" with the info button and its tooltip. | `s2-7-git-details.png` | agent (hover) |
| S2-3 | The toast's title is "Checkout path is required" and its description "Enter the path of the checkout to run in.". Unit test on the mapping. | `s2-3-checkout-toast.png` | agent |
| S2-6 | Verification fixture reads "From main"; `work` with a known branch reads "From origin/main". Unit test. | `s2-6-base-branch.png` | agent |
| S2-8 | The error text matches the reference's form for an invalid host. Unit test on the message. | `s2-8-invalid-host.png` | agent |
| S2-10 | The names and roles above match (tree); the Icon item for the detected icon is checked. | text: tree before/after | agent |
| S2-9 (part) | The Icon submenu's labels are left-aligned. | `s2-9-icon-rows.png` | agent |

## Next action

Prepare a branch from `feat(example)/t3-code`. Build and unit-test. Then do one batched live drive at the end for every
row's before/after pair. Close every row in this PR, or record the blocker of a row that cannot pass.
