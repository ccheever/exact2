# macOS: a right-click whose context popover is not menu-shaped opens nothing

**Status:** Closed
**Resolution:** a context popover that is not menu-shaped opens as a button's popover does, anchored to its node
**Systems:** host/apple
**Author:** Claude (Opus 5.5)
**Date:** 2026-10-31

## Problem

A right-click on a node whose `contextPopover` names a popover with a row that is not
only text and at most one image (a check icon in a box beside each label) opened nothing
on macOS: no menu, no popover. The node's `contextmenu` action ran. Under the agent the
same click opened the painted popover, so drives passed. The T3 Code clone's thread
details card ("Show in thread details") failed this way with real input, 0 of 3.

## Cause

`MenuHost.context` (`host/apple/Sources/ExactKit/Mac/MenusMac.swift`) built an `NSMenu`
from any popover with a button row. Before the menu tracked, `track` sends the press's
`pointerup`; the batch an app's action answers it with runs `MenuHost.sync`, which keeps a
native menu's entry only while its popover is menu-shaped (`NodeView.menuShaped`,
LLP 1021 D3). It closed the entry, and `track` then gave up. A button's popover was not
affected: `show` presents only a menu-shaped popover as an `NSMenu` and paints any other.

## Fix

`context` checks the shape as `show` does: a popover that is not menu-shaped opens through
`show` with the node as its source, as under the agent and on the web. A menu-shaped
popover is still the `NSMenu` at the pointer. `docs/contract-for-agents.md` says what makes
a context popover the Mac's menu.

## Acceptance

`ContextMenuShapeMacTests.testARightPressOpensAContextPopoverThatIsNoMenuAsAPopover`
(`host/apple/build.mjs --test`): a right press whose `pointerup` brings a batch opens a
popover with an icon box in a row, and it stays open through a later batch; a menu-shaped
one still tracks as an `NSMenu`. Before the fix the first assertion failed.
