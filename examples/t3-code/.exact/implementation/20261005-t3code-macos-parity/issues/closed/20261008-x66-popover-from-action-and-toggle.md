---
name: 20261008-x66-popover-from-action-and-toggle
plan: 20261005-t3code-macos-parity
status: moved-to-main
kind: framework-gap
blocks: []
upstream_url: https://github.com/ccheever/exact2/issues/319
reproduced_on: 9314e7a81 (main) with two one-file apps; contract/, runner/, plan/ and kernel/tables are unchanged through main 263c8b96e; again on 4bc1fc9ff (feat(example)/t3-code-fix-keyboard-focus)
---

# X66: A popover can be shown or hidden from an action only through an invisible invoker, and it says nothing when it toggles

Moved to main `issues/20261009-popover-action-commands.md` (2026-10-09); tracked there.

## Summary

A Contract popover (`popover="auto"`) opens only when a button that names it is pressed (`popovertarget`), and closes
only through such a button, by light dismiss, or by Escape. An action cannot open or close one directly
(`showPopover(id)`, `hidePopover(id)` and `togglePopover(id)` are not host commands), and the popover has no `toggle`
event, so the app never learns that it opened or closed. Today an app reaches the same result one way only: a second,
invisible button that names the popover (`popovertargetaction="show"` or `"hide"`), laid over the real trigger and
pressed through a scoped `aria-keyshortcuts`. This is the popover sibling of #282 (X53).

## Why it arose

The real-input batch (#298, bugs 4 and 13) found that the clone's menus did not move the focus with the arrow keys. The
clone matches the reference's Base UI Menu with one pattern (`examples/t3-code/menu-keys.contract`): `KeyMenu` gives ↑/↓
with wrap, Home/End and typeahead inside the open menu, and `KeyMenuOpen` lets ↓/↑ on a closed trigger open the menu at
its first or last item. Base UI's menu is built on Floating UI's list navigation, which opens from the focused trigger on
ArrowDown and ArrowUp by default (`openOnArrowKeyDown`); the user's rule is to match the original.

Three parts of the reference's menu can only be built today with the invisible invoker, or not at all:

1. **↓/↑ on a closed trigger.** No key handler can open a popover, so `KeyMenuOpen` lays two invisible
   `popovertargetaction="show"` buttons over each popover menu's trigger, armed with `aria-keyshortcuts="ArrowDown"` /
   `"ArrowUp"` only while the trigger holds the focus. Each trigger needs a positioned box that it fills, so the menu
   anchors where a press would anchor it. Six triggers gained a wrapper, and one container became positioned. The
   invokers must stay mounted: a menu whose invoker unmounts closes before its item's press lands.
   fix-provider-auth-state built the same invoker for the sign-in method menu.
2. **Knowing that a menu opened, and how.** With no `toggle` event, `KeyMenu` infers an open from its popup taking the
   focus (`focus=entered` on the `autofocus` column), and the modality and the end from a count that every trigger keeps
   (`keyed`, bumped by `kmBump`; its sign is the end).
3. **State that follows the menu's open state.** Base UI's sidebar snooze menu keeps the row's hover actions shown while
   it is open. The clone cannot read "open", so it pins the row while the focus is inside the menu or the pointer is over
   one of its rows (`sidebar-row.contract`: `snoozeFocus`, `snoozeHover`). The first proxy alone failed with the real
   pointer (bug 4).

Other clone work that wants the same capability: #290 (popover-escape-parity) tracks a pinned Usage segment popover's
state by hand, and #307 (fix-hover-cards) draws the Usage segment's hover card inside a scroll as an absolute child,
because no action can show a top-layer popover on hover.

## Clone workaround

Built by [fix-keyboard-focus](../../tasks/closed/20261008-fix-keyboard-focus.md) (#310) in `menu-keys.contract`:
- `KeyMenuOpen`: two invisible invokers over the trigger, armed by the trigger's focus.
- `kmBump`: a keyboard opening's count, with its sign as the end.
- `kmKeyEnd`, `kmOpenTarget`, `kmEndKeyed`: the same for menus that their owner mounts.
- `KeyMenu`'s `entered`, `-first` and `-last` focus targets.
- The snooze row pins on focus inside the menu or the pointer on a menu row.
- One limit remains. The table Copy menu mounts at the window, outside its trigger's tree, so ↓/↑ open it with the popup
  focused, and the next ↓ or ↑ reaches the first or last item.
- browser-surface part 1 (2026-10-09): the "+" menu's Browser row is the reference's `MenuSubTrigger`, whose profile list
  opens on hover. A hover cannot show a popover here, so the row's chevron is a `popovertarget` invoker of the nested
  profile popover (`browser-surface.contract` `BrowserAddItem`), declared in `EXACT2-GAPS.md` ("Browser surface: declared
  differences"). Nonblocking.

With `showPopover` and a `toggle` event, `KeyMenuOpen` and the wrappers go, `KeyMenu` reads the opening from `toggle`,
and the snooze row pins on the menu's open state.

## Evidence and history

- Reproduced on main `9314e7a81` with two one-file apps: `showPopover("m")` in a key action is refused at build
  (`[type-unknown-command] showPopover is not a host command`), and `toggle=` on a popover column is refused
  (`[lower-unknown-attr] column has no attribute toggle`). Record:
  [x66-repro](https://raw.githubusercontent.com/ccheever/exact2/488156a47931ae37a228b3665b0078660c046237/fix-keyboard-focus/x66-repro.txt).
  Again on `4bc1fc9ff` (feat(example)/t3-code-fix-keyboard-focus).
- Filed by the coordinator as [#319](https://github.com/ccheever/exact2/issues/319) on 2026-10-08.
