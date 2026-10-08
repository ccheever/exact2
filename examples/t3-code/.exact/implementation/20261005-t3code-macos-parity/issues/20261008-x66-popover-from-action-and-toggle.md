---
name: 20261008-x66-popover-from-action-and-toggle
plan: 20261005-t3code-macos-parity
status: draft
kind: framework-gap
blocks: []
upstream_url: null
reproduced_on: 9314e7a81 (main) with two one-file apps; contract/, runner/, plan/ and kernel/tables are unchanged through main 263c8b96e; again on 4bc1fc9ff (feat(example)/t3-code-fix-keyboard-focus)
---

# X66: A popover cannot be shown or hidden from an action, and it says nothing when it toggles

## Summary

A Contract popover (`popover="auto"`) opens only when the user presses a button that names it
(`popovertarget`), and it closes only through such a button, light dismiss, or Escape. An action
cannot open or close one: there is no `showPopover(id)`, `hidePopover(id)` or `togglePopover(id)`
host command. The popover also has no `toggle` (or `beforetoggle`) event, so the app never learns
that it opened or closed, or how. The web has all of these (HTML's popover API:
`HTMLElement.showPopover()`, `hidePopover()`, `togglePopover()`, and the `beforetoggle` and
`toggle` events with `oldState`/`newState`).

## Why this issue arose

The real-input batch (#298, bugs 4 and 13) found that the clone's menus did not move focus with
the arrow keys. The clone matches the reference's Base UI Menu with one pattern
(`examples/t3-code/menu-keys.contract`, `KeyMenu`): ↑/↓ with wrap, Home/End and typeahead
inside the open menu, and the first item focused when the menu opens from the keyboard. Three
parts of Base UI's menu can't be built without this capability:

1. **↓/↑ on a closed trigger.** Base UI's menu is built on Floating UI's list navigation. By
   default (`openOnArrowKeyDown`) that opens the menu from its focused trigger on ArrowDown and
   ArrowUp, as WAI-ARIA's menu button pattern does, focusing the first or last item. That is the
   library default; the reference app itself was not driven to confirm it. With no `showPopover`, a trigger's `key` handler
   can't open its popover. In the clone, Enter and Space open a menu, and ↓ and ↑ on a closed
   trigger do nothing.
2. **Knowing that a menu opened, and how.** With no `toggle` event, `KeyMenu` infers the open
   from the popup taking focus (`focus=entered` on the `autofocus` column). It infers the
   modality from a counter that the trigger's `key` handler bumps on Enter or Space. Every
   trigger carries that counter (`keyed`), a dozen-odd states and actions across the area
   files.
3. **State that should follow the menu's open state.** Base UI's sidebar snooze menu keeps the
   row's hover actions shown while it is open. The clone can't read "open", so it pins the row
   while the focus is inside the menu or the pointer is over one of its rows
   (`sidebar-row.contract`: `snoozeFocus`, `snoozeHover`). The first proxy alone failed with the
   real pointer (bug 4: a real mouse-down takes the focus from the popup before the click lands,
   so the row collapsed and the menu moved out from under the pointer).

A state-driven menu (one an `if` mounts) can open from an action. But it gets no popover
semantics (top layer, light dismiss, Escape, anchoring), and X53 (#282) covers its focus.

## How to reproduce (main 9314e7a81)

Record: [x66-repro](https://raw.githubusercontent.com/ccheever/exact2/488156a47931ae37a228b3665b0078660c046237/fix-keyboard-focus/x66-repro.txt).

`a.contract`, a trigger that opens its menu on ArrowDown:

```
component App
  state open = false
  action down(k: string)
    if k == "ArrowDown"
      showPopover("m")
  view
    column
      button popovertarget="m" key=down testId="trigger"
        text "Menu"
      column id="m" popover="auto" role="menu"
        button popovertarget="m" popovertargetaction="hide" role="menuitem"
          text "One"
```

`contract build a.contract`:
`a.contract:5:7 [type-unknown-command] showPopover is not a host command; the hosts answer blur,
copyText, …, showPicker, showModal, …, close, …`

`b.contract`, the same menu reporting its open state:

```
component App
  state open = false
  action toggled(value: bool)
    open = value
  view
    column
      button popovertarget="m" testId="trigger"
        text "Menu"
      column id="m" popover="auto" role="menu" toggle=toggled
        button popovertarget="m" popovertargetaction="hide" role="menuitem"
          text "One"
```

`contract build b.contract`: `b.contract:9:48 [lower-unknown-attr] column has no attribute toggle`

| Step | Actual | Expected (web) |
| --- | --- | --- |
| ArrowDown on the trigger calls `showPopover("m")` | refused at build | the popover opens; the handler can then `focus` its first item |
| The popover opens or closes | no event | `toggle` with `newState` `"open"`/`"closed"` |

## Why it must be resolved

Every popover menu in the clone (36 menus, see the fix-keyboard-focus task) misses the
reference's ↓/↑-to-open. Each trigger also carries the opened-by-keyboard counter workaround.
Any Exact app that builds a menu, combobox or disclosure on `popover` hits the same wall.

## Proposed resolution

Host commands `showPopover(id)`, `hidePopover(id)` and `togglePopover(id)`, plus a `toggle`
event on a `popover` node carrying the new state, as on the web. With them, `KeyMenu` drops the
`keyed` counters (a trigger's ↓/Enter/Space calls `showPopover` and focuses the first item), and
the snooze row pins on the menu's open state.

## Workaround in the clone

`menu-keys.contract`: the popup focuses itself on open (`autofocus`, `retainFocus`). A trigger's
`key` handler counts Enter and Space, and the popup's `focus` handler compares that count to
move focus to the first item. ↓/↑ on a closed trigger do nothing. The snooze row pins on focus
inside the menu or the pointer on a menu row.
