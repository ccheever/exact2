# Native-backed views composite above painted overlays on macOS

**Status:** Open
**Systems:** Apple host (NodeViewMac, the virtualized list, native modules, text fields)
**Severity:** P2
**Author:** Claude Fable 5.1, building Ocho for Eliot Hertenstein
**Date:** 2026-09-28
**Related:** LLP 1068 (flat leaf boxes as layers), LLP 1024 (NativeView), LLP 1021 (the top layer)

Ocho's launcher is a `when launcher` column, `position="absolute"`, inset to
the window, `z-index=20`, painted last: a half-black scrim with a card in
it. On macOS the scrim dims the sidebar and the page header, but not what is
native-backed underneath it: the virtualized `list`'s rows (their text shows
through at full brightness, most visibly at the card's edges), the `input`
in the header, and a `<ghostty-terminal>` module view (its text shows through
the scrim; only the opaque card hides it). The web paints the same tree
correctly.

Under the agent a tap by id on a painted popover row that lies over the
sidebar's `scroll` reached the scroll's row instead, so hit-testing follows
the same order as painting.

The Contract's z-order (tree order and `z-index`) should decide what is on
top; a box that happens to be backed by an NSView (a scroll view, a collection,
a text field, a module view) should not float above painted siblings that come
later in the tree. `visibility="hidden"` on the module view did not hide it
either.

Ocho works around it by unmounting the terminal (`when launcher … else
ghostty-terminal`) and hiding the list and the search field while the launcher
is open.
