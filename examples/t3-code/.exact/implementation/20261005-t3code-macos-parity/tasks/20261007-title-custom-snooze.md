---
name: 20261007-title-custom-snooze
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

# Open Custom snooze from the thread title menu

## Outcome

Thread title > Snooze > Custom opens the existing Custom snooze dialog for that thread, as
the sidebar's Custom action does. Choosing a valid time snoozes the intended thread; Cancel
or Escape leaves its snooze state unchanged.

This is a discovery record from the 2026-10-07 desktop comparison. No fix is included.
`verification: unverified` describes the future implementation, not the observed discrepancy.

## Observed behavior and reproduction

Reference source: `1e2ecbd9758830669684b494d4398f626b0576e0`.
Clone source: `fbce02624d2e33449ee2cde34497083d6fd47457`.

The missing route was repeated in the running Exact app on the same idle, unsnoozed scratch
thread, `40c97546-4d18-4d65-bb04-3f859f73bdcb`:

1. Open the thread, press its title (`thread-title`) and hover Snooze (`title-menu-snooze`).
2. Press Custom (`title-menu-snooze:custom`). The menu closes, but no Custom snooze dialog
   appears, including after `clock +500 real`.
3. Hover the same thread's sidebar row, press its snooze button, then Custom
   (`snooze-<thread-id>-custom`). The Custom snooze dialog opens with Date and Time fields.
4. Press Cancel (`snooze-cancel`). No snooze value was saved during this comparison.

The reference Electron app was also driven through **Thread title > Snooze > Custom**.
It opened the Custom snooze dialog with Date and time / Duration choices, Date, Time,
Cancel and Snooze. Pressing Cancel closed it without confirming a snooze.

Local evidence, relative to the checkout root (not committed):

- `target/desktop-audit/native/native-final-scratch-title-custom-no-dialog.{png,json}`
- `target/desktop-audit/native/native-final-scratch-title-custom-settled.{png,json}`
- `target/desktop-audit/native/native-final-sidebar-custom-snooze.{png,json}`
- `target/desktop-audit/native-actions.ndjson` records both routes and the cancellation.
- `target/desktop-audit/evidence/ref-panels-title-custom-snooze.{png,txt}`
- `target/desktop-audit/evidence/ref-panels-title-snooze-cancelled.{png,txt}`

The title-route captures contain neither `title-menu` nor `sidebar-snooze` after the click.
The sidebar capture contains `sidebar-snooze` (`Custom snooze`), `snooze-date`, `snooze-time`
and `snooze-cancel`. The reference capture contains `dialog "Custom snooze"`; the cancellation
capture no longer contains that dialog. The source path below confirms both reference entry
points use the same dialog.

## Cause and implementation guidance

The clone already constructs the title submenu's Custom entry: `shell.ts:198` emits
`op: "ui:custom-snooze"` with the thread id. `shell-panels.contract` forwards it to
`titleMenuPick`. That action in `app.contract:1430-1456` closes the title menu, handles
`ui:rename`, `ui:project-settings` and `ui:confirm`, but has no `ui:custom-snooze` branch.
Its generic dispatch explicitly excludes other `ui:` operations. Nothing opens the dialog.

The working sidebar route is `sidebar-row.contract:148` → `sidebarRun` in `app.contract:1152`
→ `sidebar:snooze:custom` → `sidebar-commands.ts:489` `openSnoozeDialog`. Route the title action
through this existing command/dialog behavior for its explicit target thread. Retain the
current command clock, capability checks, validation, cancellation and confirmation behavior.
Opening Custom must not immediately send a `thread.snooze` mutation.

Reference paths in `apps/web/src/`:

- `components/chat/ChatHeader.tsx:136` installs `useThreadActionMenu`; `openTitleMenuNow`
  invokes it at line 160.
- `hooks/useThreadActionMenu.ts:160-165` recognizes `snooze:custom`, awaits
  `requestCustomSnooze()`, returns on cancellation and snoozes only after a choice.
- `components/Sidebar.tsx:597` calls the same `requestCustomSnooze()`.
- `components/CustomSnoozeDialog.tsx:39` stores the request; its host renders the dialog at
  line 53. `routes/__root.tsx:239` mounts that host globally.

## Dependencies and deduplication

- [Thread commands and keys](closed/20261005-thread-commands-and-keys.md) owns worktree deletion
  and its listed shortcuts. It does not implement this title-menu action.
- [Minor UI fixes](closed/20261007-fix-minor-ui-issues.md) fixes title-menu settlement and
  snooze availability rules, not the Custom action route. Its `shell.test.ts` check establishes
  that the Custom label exists; it does not execute the Contract handler.
- [Reference test inventory](20261005-reference-logic-tests-done-areas.md) records tests and
  explicitly excludes fixing divergences. It does not track this observed interaction failure.
- No framework issue is needed for this finding: the same app already opens the dialog from
  the sidebar. This functional omission is separate from the shared visual-parity task.

## Acceptance and reproduction

| Criterion | Action | Expected result | Proof |
| --- | --- | --- | --- |
| Title route | On an eligible thread, open title > Snooze > Custom | Title menu closes and the existing Custom snooze dialog opens for that thread | Live native capture, alongside the reference route |
| Same behavior from both entry points | Open the dialog from title and sidebar on the same thread | Matching initial date/time, Duration mode, validation and controls | Paired native captures |
| Cancel and Escape | Open from the title and cancel by each method | Dialog closes; no thread.snooze request and no snooze-state change | Bounded drive and request/state readback |
| Confirm | Choose a valid future date/time, then a valid duration in a separate repeat | Only the intended thread receives the expected snoozedUntil value | Request/state readback |
| Unavailable actions | Use a running, ineligible or unsupported thread | Existing reference-compatible Snooze visibility/disabled rules remain | Targeted cases and UI capture |
| Keyboard and target | Open by keyboard, choose Custom and return; switch threads before reopening | Usable focus and Escape behavior; no dialog targets a previously open thread | Bounded live drive |

## Next action

Wire the missing title-menu route, add coverage that executes the action, and run the affected
app checks plus a rebuilt macOS comparison before changing verification status.
