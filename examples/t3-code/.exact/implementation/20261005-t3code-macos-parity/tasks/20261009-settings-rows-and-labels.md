---
name: 20261009-settings-rows-and-labels
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified
delivery: draft-pr
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-settings-rows-and-labels
pr_url: https://github.com/ccheever/exact2/pull/367
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

## Cause and fix

- **S1-8.** `SettingsRestoreDialog` labelled its confirm button "Restore defaults". ConfirmDialogHost's buttons are
  always Cancel and Confirm; the button now reads "Confirm" (`settings-core-body.contract`).
- **S1-9.** General's `formatSettingValue` had a `profile` branch that SettingInheritance's `formatValue` does not have,
  so Background activity's `{ profile }` read "Balanced" where the reference reads "Custom". The branch is gone, and
  the function now has the reference's other branches: a writing style's label, the merge method labels, and "Text
  generation model" for an unset writer model. Source Control's and Integrations' popovers had their own copy of the
  same reference function (`source-control-view.ts` `formatValue`, with "Unset", "Off" for the writer model and branch
  naming labels the reference does not have). They now use the one function (`settings-core.ts`).
- **S1-16, S2-7: one control.** `PolicyTip` (`settings-kit.contract`) is settingsLayout's `PolicyTooltip`: the
  icon-micro "Background policy details" button. On hover or keyboard focus it hands its frame and words to the
  window's hover layer (`hover-layer.contract`, new kind "policy": a tooltip, side top, centred, after Base UI's 200 ms
  `delay`). The layer draws it above the page, as the reference portals every TooltipPopup, so no card or scroll around
  the row clips it. Three rows use it:
  - General's Background activity: it replaces `InfoTip`, an absolute popup inside the row, which had no other use and
    is deleted.
  - Providers' Health check interval: `PvRow` takes the words as a `policy` prop and draws the tip after the title, as the
    reference's title span does. It replaces the press-to-toggle status line (`policyOpen`).
  - Source Control's Git details: the row title is "Git fetch interval" (the catalog's title), with the tip between the
    title and the reset slot.
- **S2-3.** The editor's own check throws "Checkout path is required: Enter the path of the checkout to run in."
  (`scheduled-view.ts` `taskInput`). The toast mapping recognised only the module's "Enter an existing checkout path."
  (`shell-commands.ts`). It now takes both and toasts the reference's title and description.
- **S2-6.** The trigger said "From origin/<ref>" whenever Start from origin was on. resolveBranchTriggerLabel adds
  `origin/` only for a ref that the project's refs list as a local branch (`isRemote === false`). Each ref now carries
  `remote` (`scheduled-view.ts` `branchRef`). The editor derives `baseLocal` from the chosen project's refs, and
  `taskBaseLabel` (`settings-scheduled.contract`) is the reference's function for a future worktree. A base that is
  not on the project's first page of 100 refs is looked up by name, as WorktreeBaseBranchPicker's `selectedRefQuery`
  does (`listRefs { query: <base>, limit: 10 }`, review fix): the page resource does it for the draft's project and base,
  and for the project the editor switched to (`chooseProject` hands `key`, `project` and `ref` to the root's `taskBase`
  state, an argument of the `scheduled` resource, because a child component has no resource of its own, X9). The
  result is each project group's `selected`, which `baseLocal` reads after `refs`.
- **S2-8.** URL refuses a host with a space ("invalid URL: invalid international domain name"). The reference's
  Chromium renderer instead escapes it (`https://not%20a%20url/`) and fails at the transport. `parsePairing`
  (`protocol.ts`) now returns that escaped origin, marked `unreachable`, for a host with spaces, and "Backend URL is
  invalid." (RemoteBackendUrlInvalidError) for any other host URL refuses. Add environment, Add route and the
  connect op report `environmentFetchFailure` (failRemoteRequest's message, `rpc/http.ts:156`) after their own checks:
  without a pairing code it is still "Enter a pairing code.", as in the reference. Nothing is sent to the native
  transport. Review fix: Chromium still reads such a link's `token` and `host`, so `escapedUrl` (`protocol.ts`) gives
  the escaped origin with the query and fragment as written, and both `parsePairing` and Add environment's Host field
  split (`pairingFields`, `r10-connect-pairing.ts`, parsePairingUrlFields) use it: `https://not a url/pair#token=abc`
  pasted into Host fills Host `https://not%20a%20url` and Pairing code `abc`, and Add environment then reports the
  transport failure, as the reference does.
- **S2-10.**
  - `ScopedRow` carries the control's accessible name (`control`) and the reset's label (`resetLabel`) where the
    reference's differ from the title: switch "Default automatic pull"; combobox "Default pull request merge method";
    resets "default automatic pull", "default merge method", "branch naming", "branch prefix", "branch naming
    instructions", "source control writing style", "change request templates", "default browser access".
  - `CnMenuItem` has a `radio` prop: role `menuitemradio`, checked while highlighted. The Icon submenu's kinds pass it,
    so the current kind, "Laptop detected", is the checked one.
- **S2-9 (part).** A `button` centres its text. `CnMenuItem`'s label and the "Icon" trigger now have
  `text-align="left"`, as X57 did for the pull request list. This also left-aligns the other menus built on
  `CnMenuItem` (the environment row menu, CnSelect, the provider sign-in method), as the reference's menus are. The
  submenu's placement is X17's and is not changed.

## Acceptance results

Before is the evidence-base build (`950e8e2e5`, lane `settings-rows-and-labels-before`). After is this branch's bundle
(lane `settings-rows-and-labels`, embedded server on 16242). Both drives used the same steps, in agent mode at
1280×840. The reference is the Electron build on this lane (16240/16241); `S1-8` and `S2-8` reuse the audit lane's
reference shots.

| Id | Result | Evidence |
| --- | --- | --- |
| S1-8 | Pass: Cancel and Confirm (before: "Restore defaults") | [s1-8 triple](https://raw.githubusercontent.com/ccheever/exact2/cf954a07139524c6d2cd57df0795f4e06b053d45/settings-rows-and-labels/s1-8-restore-confirm.png) |
| S1-9 | Pass: "Daehyeon's MacBook Pro \| Environment \| Inherits \| Default \| Custom" (before: "… Default \| Balanced"); the reference reads the same. Unit test | [tree text](https://raw.githubusercontent.com/ccheever/exact2/b4489a2c3ba1632c3d371c5a711d35c6ed5c0bb7/settings-rows-and-labels/text-before-after.txt) |
| S1-16 | Pass: hovering the (i) next to "Health check interval" shows the reference's words above it, unclipped (before: nothing on hover) | [s1-16 triple](https://raw.githubusercontent.com/ccheever/exact2/a0940eab8b3f551e8d5111357aebd217521d2ebc/settings-rows-and-labels/s1-16-policy-tooltip.png) |
| S2-7 | Pass: "Git fetch interval" with the info button; hovering it shows the reference's words (before: "Automatic Git fetch interval", no button) | [s2-7 triple](https://raw.githubusercontent.com/ccheever/exact2/c6417ace18c9599d0d3a55d5f11841f428259bc8/settings-rows-and-labels/s2-7-git-details.png) |
| S2-3 | Pass: "Checkout path is required" / "Enter the path of the checkout to run in." (before: "Could not save scheduled task" / "Checkout path is required: Enter …"). Unit test on the mapping. Reference: from the source (`ScheduledTasksSettings.tsx:576-578`); the reference lane has no signed-in provider, so its editor stops at "Scheduled task is incomplete" first | [s2-3 pair](https://raw.githubusercontent.com/ccheever/exact2/df6ccfb5f67c0eae0a4efa3dedeeee8392cdcf91/settings-rows-and-labels/s2-3-checkout-toast.png), [tree text](https://raw.githubusercontent.com/ccheever/exact2/b4489a2c3ba1632c3d371c5a711d35c6ed5c0bb7/settings-rows-and-labels/text-before-after.txt) |
| S2-6 | Pass: Verification fixture reads "From main" (before: "From origin/main"); `work` reads "From origin/main"; the reference reads the same for both | [s2-6 triples](https://raw.githubusercontent.com/ccheever/exact2/eefcc8da4795eab27d91b4c4f73e18b32321bae7/settings-rows-and-labels/s2-6-base-branch.png) |
| S2-8 | Pass: inline error and toast read "Failed to fetch remote environment endpoint https://not%20a%20url/.well-known/t3/environment (HttpClientError: Transport error (GET https://not%20a%20url/.well-known/t3/environment))." (before: "invalid URL: invalid international domain name"). Unit tests | [s2-8 triple](https://raw.githubusercontent.com/ccheever/exact2/15a2bec2e8da4b919c89296f6a2ed3cac8b58b81/settings-rows-and-labels/s2-8-invalid-host.png), [tree text](https://raw.githubusercontent.com/ccheever/exact2/b4489a2c3ba1632c3d371c5a711d35c6ed5c0bb7/settings-rows-and-labels/text-before-after.txt) |
| S2-10 | Pass: switch "Default automatic pull", combobox "Default pull request merge method", resets "Reset default automatic pull / default merge method / branch naming / source control writing style to default"; the Icon kinds are `menuitemradio` with Laptop checked (before: the titles, `menuitem`, no checked state). Unit test | [tree text](https://raw.githubusercontent.com/ccheever/exact2/b4489a2c3ba1632c3d371c5a711d35c6ed5c0bb7/settings-rows-and-labels/text-before-after.txt) |
| S2-9 (part) | Pass: the Icon submenu's labels and the "Icon" row start at the left (before: centred). Placement stays X17's | [s2-9 triple](https://raw.githubusercontent.com/ccheever/exact2/a547af9d67e0fe87af99b7f92e0a66724fc092f1/settings-rows-and-labels/s2-9-icon-rows.png) |
| Review R1 (S2-8 edge): a pairing link whose host has spaces | Pass: pasted into Host it splits into `https://not%20a%20url` and `abc`, and Add environment reports the reference's transport failure (before: the link stayed in Host, "invalid URL: invalid international domain name"; this PR's previous head: credential lost, "Enter a pairing code."). The reference reads the same. Unit tests | [r1 triple](https://raw.githubusercontent.com/ccheever/exact2/88886bc564d0a5773a9b31d7a51f3391112ac3e5/settings-rows-and-labels/s2-8-pairing-link.png), [text](https://raw.githubusercontent.com/ccheever/exact2/522068efbd752e0d48dd58d44ab184871dbe9cde/settings-rows-and-labels/text-review-fixes.txt) |
| Review R2 (S2-6 edge): a local base past the first 100 refs | Pass: lane project many-refs (122 branches, main past the first page): "From origin/main" after switching to it, as the reference (this PR's previous head: "From main"; the feature tip: "From origin/main" because it always added origin/). Unit test | [r2 triple](https://raw.githubusercontent.com/ccheever/exact2/f3ae6efd75846d3622b63b753402f35306d5ce2e/settings-rows-and-labels/s2-6-base-past-first-page.png), [text](https://raw.githubusercontent.com/ccheever/exact2/522068efbd752e0d48dd58d44ab184871dbe9cde/settings-rows-and-labels/text-review-fixes.txt) |
| Review R3: tests for the connect op and Add route | Pass: `r10-connect.test.ts` covers both paths (no UI change) | tests |
| PolicyTip on General (no finding; the replaced `InfoTip`) | Pass, no regression: the Background activity tooltip shows the same words above the (i) | [general pair](https://raw.githubusercontent.com/ccheever/exact2/e5ec221a10d53343c6e80248e06c8cd8fe5e0014/settings-rows-and-labels/general-policy-tooltip.png) |

## Tests

- `settings-a.test.ts`:
  - Background activity's popover layers read Environment "Inherits", then "Custom", before and after a Performance
    write.
  - `formatSettingValue` on a model, a writing style, the merge methods, the unset writer model, a submodule mode and an
    unlabelled string.
  - `branchRef` carries `remote`.
- `settings-rest.test.ts`: Source Control's rows carry the reference's control names and reset labels.
- `shell.test.ts`: the editor's own "Checkout path is required: …" maps to the reference's toast title and description.
- `client.test.ts`: `parsePairing` gives the escaped, unreachable origin for a host with spaces (direct, with a port,
  and in a hosted link's `host`), "Backend URL is invalid." for another refused host, and `environmentFetchFailure`'s
  message.
- `r10-connect.test.ts`: Add environment with "not a url" and "ABC" rejects with the reference's message and toasts it,
  connected or not. Nothing reaches the native transport. Without a code it is "Enter a pairing code.". Review fixes:
  the split and the whole link (`https://not a url/pair#token=abc`) fail the same way; Add route to a host with spaces
  ("Could not add route") and the connect and reconnect ops (`connectionOps`) report the same failure and send nothing;
  `pairingFields` splits a link whose host has spaces as Chromium does (escaped host, token, a hosted `host`).
- `client.test.ts` (review fix): `parsePairing` reads a refused link's `#token=`, `?token=` and `?host=`, and maps
  `ws://` to `http://`.
- `live-automations.test.ts` (review fix): the editor's base past the first page is looked up by name, and only then;
  nothing found leaves it unknown; after a project switch (`taskBase`) that project is looked up, and a stale key asks
  nothing.
- `hover-layer.test.ts`: `PolicyTip`'s layer call (kind policy, top, centre, hover and focus), the policy kind as a
  tooltip with a 200 ms open delay, and its three uses with the reference's words.
- New `settings-labels.test.ts`, which reads the Contract sources:
  - S1-8's buttons;
  - S2-6's `taskBaseLabel`, `baseLocal` (page, then lookup), `chooseProject`'s `lookupBase` and the resource argument;
  - S2-10's control and reset labels in `ScopedSettingRow`;
  - S2-9 and S2-10's radio Icon kinds and left-aligned menu labels.

Checks: see the PR ("Checks").

## Real-input batch steps

None. Every row was verified in agent mode: hovers, presses, typing and the tree.

## Progress

2026-10-10: built every finding row. Shot the reference's tooltips, base branch labels and Icon menu on this lane. Drove
the base build and then this branch once each, with the same steps. Every row passes. Draft PR
[#367](https://github.com/ccheever/exact2/pull/367). Merged `6cb2828ba` (settings-escape-and-nav and two more) before
the final checks.

2026-10-10, review round: an independent review found three should-fix problems (R1 a refused pairing link lost its
token, R2 the base branch read only the first 100 refs, R3 two unreachable-host paths had no test). All three fixed in
one round; R1 also covers Add environment's Host field split, the reference's path for a pasted link. Merged
`eba445c44` before the final checks. One live drive per side (lane project many-refs added with `t3 project add`).

## Attempts and evidence

| Attempt | Revision | Outcome | Evidence |
| --- | --- | --- | --- |
| Before drive | evidence-base `950e8e2e5` | Complete on the first run | [tree text](https://raw.githubusercontent.com/ccheever/exact2/b4489a2c3ba1632c3d371c5a711d35c6ed5c0bb7/settings-rows-and-labels/text-before-after.txt) (with the drive's steps) |
| After drive (the one live drive) | this branch, bundle built after merging `c081d6d43` | Complete on the first run, every row passes | the links above |
| Review round, before drive | evidence-base `950e8e2e5` | Complete on the first run | [text](https://raw.githubusercontent.com/ccheever/exact2/522068efbd752e0d48dd58d44ab184871dbe9cde/settings-rows-and-labels/text-review-fixes.txt) |
| Review round, after drive | this branch `b6efbf144` (bundle after merging `eba445c44`) | Complete on the first run, R1 and R2 pass | the review rows above |

## Next action

The coordinator reviews the draft PR and merges it. Seen during the review drive, not this task's findings (both also
on the feature tip): the base branch picker's ref names are centred (a `button` centres its text; the reference's are
left-aligned), and the picker has no "Showing 100 of 122 refs" status, server-side search or next page
(usePaginatedBranches). `settings-core.ts` is shared with
[settings-diagnostics-and-scope](20261009-settings-diagnostics-and-scope.md), and `settings-scheduled.contract` and
`settings-source-control.contract` with [model-picker-parity](20261009-model-picker-parity.md). The second of these to
merge keeps both sides.
