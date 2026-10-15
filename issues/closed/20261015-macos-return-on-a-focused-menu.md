# macOS: Return on a focused menu row in an open popover does nothing

**Status:** Closed
**Resolution:** fixed: on the Mac a focused button or pressable whose role is menuitem, menuitemradio, menuitemcheckbox or option chooses on Return as on Space, before a dialog's default button (choosesOnReturn)
**Systems:** host/apple macOS, keyboard, menus
**Author:** Daehyeon Mun
**Date:** 2026-10-15
**Severity:** P2
**Related:** LLP 1115.000 D7.3

On macOS, a focused `button` with `role="menuitem"` (or `menuitemradio`, `menuitemcheckbox`,
`option`) inside an open popover takes Space but not Return. In an `NSMenu`, Return chooses the
highlighted item, and in Chrome, Enter activates a focused menu item. Found while building an
example app: its painted menus (a `popover` holding `role="menu"` and rows moved by the arrow
keys) ignore Return unless each row handles the key itself.

Cause: LLP 1115.000 D7.3 gives Return to a dialog's default button only, and both button paths
drop it for a merely focused control: `NativeButtonsMac.swift:98` returns on `Enter`, and
`NodeViewMac.swift:263` presses a pressable on Space (Enter only for a link). Neither checks the
row's role or whether it is in an open popover.

Reproduction: a `button` with `popovertarget` opens a `popover` with `role="menu"` holding two
`button role="menuitem" press=…` rows; open it with the keyboard, move to a row, press Return.
The row's action does not run; Space runs it.

Fix: press a focused row whose role is `menuitem`, `menuitemradio`, `menuitemcheckbox` or
`option` on Return as on Space, in both `NativeButtonsMac.keyDown` and `NodeViewMac.keyDown`,
and keep D7.3 for other buttons. A dialog's default button still takes Return before the row
when the menu is inside a dialog (`DialogHost.key`), so check that order too.

Acceptance: in the reproduction, Return runs the focused row's action once, and the popover
closes as it does after Space; a plain focused button still ignores Return.
