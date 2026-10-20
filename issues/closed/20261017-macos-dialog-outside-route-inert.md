# macOS: a dialog outside the route columns opens as an inert sheet

**Status:** Closed
**Resolution:** fixed: a stack's authored routes keep only nodes with a navigationKey, so a dialog in its sheet is no route and is not gated
**Systems:** Apple host (NavigationHost, BlockHost, DialogHost)
**Severity:** P2
**Author:** Claude (Opus 5.5)
**Date:** 2026-10-17
**Related:** LLP 1115.002 D7.2, LLP 1038 D6

A `dialog` whose parent is the navigation root (a sibling of the stack's `for`) opens as a
sheet with its field and buttons disabled: the focus lands on the dialog, not its
`autofocus` field, and the accessibility tree holds the dialog and no children. Inside a
route it works. Repro: `agent macos --chrome platform "tap <opener>" "tree --ax"` shows
`modal (no view) by sheet` and a childless `dialog`.

**Cause.** `NavigationHost.sync` reads a stack's routes through `blocks.authored`
(`NavigationMac.swift:118`), which puts back every subtree that left the root for a
container (`BlockHostMac.swift:89`), route or not. The dialog in its sheet joins the routes,
is not the selected one and is gated `routeInert` (`:177`); `inert` reads that on the
dialog before stopping there (`NodeViewMac.swift:221`), so its subtree is inert.
`AgentMac.swift:154` reads the stack the same way.

**Fix.** Keep only nodes with a `navigationKey` among the authored routes, in both readers.
Acceptance: an AppKit XCTest where a dialog outside the route columns, in its sheet, focuses
its `autofocus` field and exposes its children; one inside a route is unchanged.
