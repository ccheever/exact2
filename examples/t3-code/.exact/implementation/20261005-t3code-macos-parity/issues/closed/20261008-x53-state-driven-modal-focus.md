---
name: 20261008-x53-state-driven-modal-focus
plan: 20261005-t3code-macos-parity
status: moved-to-main
kind: framework-gap
blocks: [20261008-dialog-shortcut-focus]
upstream_url: https://github.com/ccheever/exact2/issues/282
reproduced_on: 0365ad1a4 (main)
---

# X53: a dialog an app opens from state has no host focus containment on macOS or the web

Moved to main `issues/20261009-gui-dialog-action-commands.md` (2026-10-09); tracked there.

## Summary

Base UI's modal Dialog moves the focus into the popup, keeps Tab and Shift+Tab inside it and gives
the focus back to its trigger when it closes. In exact2 a modal that does this is only an HTML
`dialog` opened by an invoker button (`commandfor`/`command="show-modal"`). A dialog an app opens
from state — a confirmation a menu item or a server event asks for — has no such modal:
`showModal(id)` compiles but only the terminal host carries it, and `aria-modal` keeps no focus inside.

## Why it arose

### The T3 Code behavior
Every T3 dialog is a Base UI Dialog or AlertDialog (`components/ui/dialog.tsx`,
`alert-dialog.tsx`; `DialogPopup` passes `initialFocus`, `returnFocus` and `modal` to Floating UI's
focus manager), opened from state (`ConfirmDialogHost`, `requestCustomSnooze`, the Settings
dialogs' `open` props).

### Where the clone hits it
Task [dialog-shortcut-focus](../../tasks/closed/20261008-dialog-shortcut-focus.md) needs the modal's
focus behavior in every dialog the clone opens from state.

## Clone workaround

[dialog-shortcut-focus](../../tasks/closed/20261008-dialog-shortcut-focus.md) keeps the focus inside
AppConfirm, SettingsConfirm, the Custom snooze dialog and Add Environment with per-dialog `key`
handlers on their first and last stops (`fromCancel`/`fromConfirm`, `toggleKey`/`closeKey`,
`wrapKey`/`closeKey`), an explicit `focus()` where the opener keeps the focus (the terminal's close,
the sidebar's dialogs through a root task, `sidebarDialogFocus`) and an explicit `focus()` back to the
trigger (`dialogReturn`). The clone's other dialogs rely on the page behind them being `inert`; a toast
shown at the time is still a Tab stop from them.

On adoption, `dialog` elements opened by `showModal`/`close` replace the per-dialog traps,
`sidebarDialogFocus`, `dialogReturn` and the `focus()` returns (AppConfirm, SettingsConfirm, Custom
snooze, Add Environment, the Local environment dialog, the pull request confirmation and stack dialogs).

## Evidence and history

- Local draft (2026-10-08, dialog-shortcut-focus), reproduced on the feature branch's framework (main
  `1f19b2400`).
- Filed as [#282](https://github.com/ccheever/exact2/issues/282) ([Feature] `showModal(id)` and
  `close(id)` from an action on macOS and the web) on 2026-10-08, reproduced on main `0365ad1a4` with a
  minimal public-API app: macOS logs `exact: unknown command showModal`, the web `refused: showModal is
  not a command this runtime carries`. The invoker form (`command="show-modal"`) traps Tab and returns
  the focus on Escape on both hosts; a `role="dialog" aria-modal=true` overlay lets Tab leave on both
  hosts (22 → 3). No duplicate found.
- Accepted upstream on 2026-10-08 ("Carry showModal(id) and close(id) on GUI hosts"). #324 (dialog
  commands and close events, refs #282) was closed unmerged.
