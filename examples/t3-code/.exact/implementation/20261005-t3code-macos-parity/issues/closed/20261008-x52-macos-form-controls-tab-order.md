---
name: 20261008-x52-macos-form-controls-tab-order
plan: 20261005-t3code-macos-parity
status: moved-to-main
kind: framework-gap
blocks: [20261008-dialog-shortcut-focus]
upstream_url: https://github.com/ccheever/exact2/issues/280
reproduced_on: 0365ad1a4 (main)
---

# X52: on macOS, `input type="date"`, `input type="time"` and `select` are not Tab stops

Moved to main `issues/20261009-macos-controls-default-tab-order.md` (2026-10-09); tracked there.

## Summary

On the web, a date input, a time input and a `select` are sequential focus stops (Chrome stops in
each of the date input's fields too). On macOS, Tab skips all three: the key-view loop
(`Presenter.tabbable`) admits none of these `control` kinds.

## Why it arose

### The T3 Code behavior
`CustomSnoozeDialog.tsx` (1e2ecbd975): Tab moves through the schedule type, the Date button, the
time input (`type="time"`), Cancel, Snooze and Close; in Duration mode through the number field's
Decrease, input and Increase, the Unit select, Cancel, Snooze and Close.

### Where the clone hits it
`sidebar-overlays.contract` `SidebarSnoozeDialog`: in the lane drive of task
[dialog-shortcut-focus](../../tasks/closed/20261008-dialog-shortcut-focus.md), Tab goes "Date and time" → Cancel → Snooze → Close (Date and Time
skipped), and in Duration mode the Unit select is skipped. The dialog's own trap and order are
right; the host leaves these controls out. Nonblocking for that task: everything else in the Custom
snooze order matches.

## Clone workaround

None needed: the snooze dialog's stops follow tree order once the host admits them. On adoption,
re-drive the Custom snooze dialog's Tab cycle (`toggleKey`/`closeKey` in `sidebar-overlays.contract`)
with Date, Time and Unit as Tab stops, and drop the X52 note in `dialog-focus.test.ts`.

## Evidence and history

- Local draft (2026-10-08, dialog-shortcut-focus), reproduced on the feature branch's framework (main
  `1f19b2400`) in a scratch app: macOS `focus.logical` read f-before → f-text → f-after; the web read
  f-before → f-date (three stops) → f-time → ….
- Filed as [#280](https://github.com/ccheever/exact2/issues/280) ([Bug] macOS: date, time and select
  inputs are not Tab stops) on 2026-10-08, reproduced on main `0365ad1a4` with a minimal public-API app:
  macOS Tab walk 3 → 10 → 11 → 3, Shift+Tab 11 → 10 → 3; web 3 → 5 (×4) → 6 (×4) → 7 → 10 → 11. An
  explicit `tabindex=0` on the date input makes it a macOS stop (3 → 5 → 10). No duplicate found.
- Accepted upstream on 2026-10-08 ("Put date, time and select controls in the default Tab order").
