---
name: 20261008-x52-macos-form-controls-tab-order
plan: 20261005-t3code-macos-parity
status: draft
kind: framework-gap
blocks: [20261008-dialog-shortcut-focus]
upstream_url: null
reproduced_on: d82fb6a47 (feature branch on main 1f19b2400's framework; main 462308f9c unchanged)
---

# X52: on macOS, `input type="date"`, `input type="time"` and `select` are not Tab stops

## Summary

On the web, a date input, a time input and a `select` are sequential focus stops (Chrome stops in
each of the date input's fields too). On macOS, Tab skips all three: the key-view loop is built
from `Presenter.tabbable` (`host/apple/Sources/ExactKit/Mac/PresenterMac.swift`), which admits
fields, text areas, native modules with a focus target, buttons, toggles and pressables, and none
of these `control` kinds. The requested support is that a date, time and select control is a Tab
stop on macOS as on the web (and takes the keys its platform control takes once focused).

## Why this issue arose

### The T3 Code behavior
`CustomSnoozeDialog.tsx` (1e2ecbd975): Tab moves through the schedule type, the Date button, the
time input (`type="time"`), Cancel, Snooze and Close; in Duration mode through the number field's
Decrease, input and Increase, the Unit select, Cancel, Snooze and Close.

### What exact2 does today
Repro (a scratch app made with `bun scripts/exact.mjs new`, view excerpt):

```contract
row gap=8 testId="fields"
  button testId="f-before"
    text "Before"
  input type="date" value=day change=setDay aria-label="Date" testId="f-date"
  input type="time" value=at change=setAt aria-label="Time" testId="f-time"
  select value=unit change=setUnit aria-label="Unit" testId="f-unit"
    option "Minutes" value="minutes"
    option "Hours" value="hours"
  input value=unit aria-label="Text" testId="f-text"
  button testId="f-after"
    text "After"
```

`bun exact.mjs agent macos --json "type f-before key Space" state "type root key Tab" state …`
reads `focus.logical`: f-before (14) → f-text (21) → f-after (22). The same drive on `web` reads
f-before (14) → f-date (16, three stops: its month, day and year fields) → f-time (17) → …

### Where the clone hits it
`sidebar-overlays.contract` `SidebarSnoozeDialog`: in the lane drive of task
`20261008-dialog-shortcut-focus`, Tab goes "Date and time" → Cancel → Snooze → Close (Date and Time
skipped), and in Duration mode the Unit select is skipped. The dialog's own trap and order are
right; the host leaves these controls out.

## Why it must be resolved

Keyboard users cannot reach a date, time or select control in any Exact app on macOS without the
pointer. Nonblocking for the dialog focus task: everything else in the Custom snooze order matches.

## Requested support

Count `input type="date"`, `type="time"` and `select` (the `control` kinds that are not radios)
as sequential focus stops on macOS, in tree order, with their platform keys once focused.

## Acceptance for the fix

The repro's macOS drive reads f-before → f-date → f-time → f-unit → f-text → f-after, and Shift+Tab
the reverse.

## App adoption after resolution

Nothing to change in the clone: the snooze dialog's stops follow tree order once the host admits
them. Re-run the snooze drive in the task record.

## Status and next action

Local draft (2026-10-08, dialog-shortcut-focus). Reproduced on the feature branch's framework (main
`1f19b2400`); `tabbable` is unchanged on main `462308f9c`. Not published: publication needs the
user's approval (`issue-open`).
