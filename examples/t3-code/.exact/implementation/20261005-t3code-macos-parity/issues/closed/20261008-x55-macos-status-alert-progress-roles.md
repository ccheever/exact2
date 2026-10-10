---
name: 20261008-x55-macos-status-alert-progress-roles
plan: 20261005-t3code-macos-parity
status: moved-to-main
kind: framework-gap
blocks: [20261005-portable-app-download]
upstream_url: https://github.com/ccheever/exact2/issues/278
reproduced_on: 0365ad1a4 (main)
---

# X55: the macOS host exposes no live region, alert, dialog or progress bar to accessibility

Moved to main `issues/20261009-macos-role-modal-accessibility.md` (2026-10-09); tracked there.

## Summary

Contract carries `role="status"`, `aria-live="polite"`, `role="alert"`, `role="dialog"`/`"alertdialog"`
with `aria-modal`, and `role="progressbar"` with `aria-label`. On macOS these nodes are plain static
text or nothing: VoiceOver does not announce a status change or an alert, does not find the progress
bar, and the agent's `tree --ax` lists none of them. Related to X49 (the bar's value).

## Why it arose

The first-launch view ([portable-app-download](../../tasks/closed/20261005-portable-app-download.md) item 3,
`first-launch.contract`) asks for a progress bar labelled "Setting up T3 Code", a stage line that is a
polite live region, an error that is an alert, and a modal dialog. `bun scripts/agent.mjs macos …
"tree --ax first-launch"` on the lane build (2026-10-08): the elements are the heading ("Setting up T3
Code…", AXHeading) and the stage line (AXStaticText); the progress bar is absent; `modal.present` is
false; in the failure state the error is AXStaticText and only the two buttons are interactive. The
Contract `tree` of the same nodes has `accessibilityRole: progressbar`, `accessibilityLive: polite`,
`accessibilityRole: status`/`alert` and `accessibilityModal: true`.

## Clone workaround

None. portable-app-download was dropped on 2026-10-08, so no clone view waits on it; the clone's
`role="status"`/`"alert"` nodes, the drawn progress bar and its dialogs reach VoiceOver once main maps
the roles, with nothing to remove.

## Evidence and history

- Seen in the clone's lane build on 2026-10-08 (above). Record copied from the closed #260 branch.
- Filed as [#278](https://github.com/ccheever/exact2/issues/278) ([Bug] macOS: progressbar, status, alert
  and modal dialog roles are not exposed to accessibility) on 2026-10-08, reproduced on main `0365ad1a4`
  with a one-file app (an indeterminate `progress`, a `box role="progressbar"`, `text role="status"
  aria-live="polite"`, `text role="alert"`, `column role="dialog" aria-modal=true`): macOS `tree --ax`
  lists the `progress` element (`progressbar "Working" [busy]`), plain `text` for the status and the
  alert, and no drawn bar, dialog or modal; the web lists `progressbar "Download"`, `status`, `alert` and
  `dialog "Setup" [modal]`. No duplicate found.
- Accepted upstream on 2026-10-08 as a correctness fix ("Complete admitted role mappings, implicit live
  announcements and macOS modal accessibility").
