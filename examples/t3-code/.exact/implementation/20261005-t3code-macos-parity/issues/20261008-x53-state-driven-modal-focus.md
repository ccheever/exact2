---
name: 20261008-x53-state-driven-modal-focus
plan: 20261005-t3code-macos-parity
status: draft
kind: framework-gap
blocks: [20261008-dialog-shortcut-focus]
upstream_url: null
reproduced_on: d82fb6a47 (feature branch on main 1f19b2400's framework; main 462308f9c unchanged)
---

# X53: a dialog an app opens from state has no host focus containment on macOS or the web

## Summary

Base UI's modal Dialog moves the focus into the popup, keeps Tab and Shift+Tab inside it and gives
the focus back to its trigger when it closes. In exact2 a modal that does this is only an HTML
`dialog` opened by an invoker button (`commandfor`/`command="show-modal"`, LLP 1021). A dialog an
app opens from state — a confirmation a menu item or a server event asks for — has no such modal:
`showModal(id)` compiles (LLP 1101.001 P5) but only the terminal host carries it, and `aria-modal`
keeps no focus inside (LLP 1080.003 §4). The requested support is `showModal(id)` / `close(id)`
from an action on macOS and the web, with the invoker form's focus, Tab and Escape behavior.

## Why this issue arose

### The T3 Code behavior
Every T3 dialog is a Base UI Dialog or AlertDialog (`components/ui/dialog.tsx`,
`alert-dialog.tsx`; `DialogPopup` passes `initialFocus`, `returnFocus` and `modal` to Floating UI's
focus manager), opened from state (`ConfirmDialogHost`, `requestCustomSnooze`, the Settings
dialogs' `open` props).

### What exact2 does today
Repro (scratch app, excerpt):

```contract
action openModal
  showModal("sm")
…
button press=openModal testId="open-sm"
  text "showModal"
dialog id="sm" testId="sm"
  button testId="sm-inside"
    text "Inside the dialog"
```

- macOS: `bun exact.mjs agent macos --json "tap open-sm" "clock settle" logs` logs
  `exact: unknown command showModal`; the dialog stays closed and the focus stays on the button.
- Web: the same drive logs `refused: showModal is not a command this runtime carries`.
- An overlay drawn with `role="dialog" aria-modal=true` keeps no Tab inside on either host, and a
  mounted `autofocus` waits while another control holds the focus (HTML's rule), so a dialog opened
  from a still-focused trigger does not take the focus.

### Where the clone hits it
Task `20261008-dialog-shortcut-focus` keeps the focus inside AppConfirm, SettingsConfirm, the Custom
snooze dialog and Add Environment with per-dialog `key` handlers on their first and last stops, an
explicit `focus()` where the opener keeps the focus (the terminal's close, the sidebar's dialogs
through a root task) and an explicit `focus()` back to the trigger. The clone's other dialogs rely on
the page behind them being `inert`; a toast shown at the time is still a Tab stop from them.

## Why it must be resolved

Every app with a confirmation or a form dialog needs the modal's focus behavior; writing it per
dialog is error-prone and the reason the clone's other dialogs still leak to a toast.

## Requested support

`showModal(id)` and `close(id)` from an action on macOS and the web (the terminal host's P5), with
the invoker form's top layer: the focus moves to the first `autofocus` or focusable descendant,
Tab and Shift+Tab stay inside, Escape closes, and closing gives the focus back.

## Acceptance for the fix

The repro opens `sm` on both hosts with the focus on `sm-inside`; Tab from the last control returns
to the first; closing returns the focus to `open-sm`.

## App adoption after resolution

Replace the clone's per-dialog traps (`fromCancel`/`fromConfirm`, `toggleKey`/`closeKey`,
`wrapKey`/`closeKey`), `sidebarDialogFocus`, `dialogReturn` and the `focus()` returns with
`dialog` elements opened by `showModal`, and adopt it in the remaining dialogs.

## Status and next action

Local draft (2026-10-08, dialog-shortcut-focus). Reproduced on the feature branch's framework (main
`1f19b2400`; no change on main `462308f9c`). Not published: publication needs the user's approval
(`issue-open`).
