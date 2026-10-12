# macOS: VoiceOver's show menu (AXShowMenu) runs no node's `contextmenu`

**Status:** Closed
**Resolution:** fixed by #486
**Systems:** host/apple macOS, accessibility
**Severity:** P3
**Author:** Daehyeon Mun (via the T3 Code clone)
**Date:** 2026-10-12
**Related:** T3 PR #474 (reply-links-reference-behavior RL-6); https://github.com/ccheever/exact2/blob/feat(example)/t3-code/examples/t3-code/EXACT2-GAPS.md (X84); `issues/closed/20261010-macos-link-url-accessibility.md` (X78, the same AX surface); main PR #327

## Summary

VoiceOver's "show menu" command (VO-⇧-M, the `AXShowMenu` accessibility action) does nothing on an Exact node on
macOS, even when the node has a `contextmenu` handler or a `contextPopover`. The node's handler runs for a right-click
(`NodeViewMac.swift:1429` `rightMouseDown` → `dispatchContextMenu`, `MouseEventsMac.swift:85`) and for the ContextMenu
key at the focused node (`KeyEvents.swift:131` → `MouseEventsMac.swift:24` `contextMenuKey`). No accessibility path
reaches it. `NodeView` is an `NSView` that implements `accessibilityPerformPress` (`NodeViewMac.swift:1337`) and no
show-menu method: `rg 'ShowMenu|accessibilityActionNames' host/apple/Sources` finds nothing. AppKit's `NSView`
default `accessibilityPerformShowMenu` returns `false`, and AppKit lists `AXShowMenu` for a view only when the view has
a `menu`, which `NodeView` never sets. `menu(for:)` (`TextSelectionMac.swift:507`) answers a paragraph's text
selection only.

So a VoiceOver user cannot open any menu an Exact app puts on `contextmenu`: a link's or a row's app menu, a file
tree's, a message's. In Chrome the same command on an `<a href>` or a `<button>` fires a `contextmenu` event at the
element, which the page's listener hears (read from Chromium's source; see Reproduction). A hand-built AppKit control
with a context menu lists `AXShowMenu` and opens its menu.

## Why this arose

The T3 Code desktop app (Electron) answers the ContextMenu key and VoiceOver's show menu on a focused link in a reply
with its shell's menu: Chromium raises `context-menu` at the anchor. The clone's link nodes carry a `contextmenu`
handler that asks for that menu. T3 PR #474 (task reply-links-reference-behavior, RL-6) made the ContextMenu key work.
VoiceOver's show menu still opens nothing because no `contextmenu` runs. The clone declares this as X84 in its
`EXACT2-GAPS.md`.

## Reproduction

Main `4e6a4523964fce298771fa382480199eb98b7395`, macOS 26.6.2 (25G83), Apple Silicon. A one-file app made with
`bun scripts/exact.mjs new <dir>` (`app.ts` exports an empty `sources`), built with `bun exact.mjs mac`:

```text
component App
  state linkMenus = 0
  state buttonMenus = 0
  action linkMenu
    linkMenus = linkMenus + 1
  action buttonMenu
    buttonMenus = buttonMenus + 1
  action noop
    linkMenus = linkMenus
  view
    main testId="root" width="100%" height="100%" padding=16 display="flex" flex-direction="column" gap=12
      link href="mailto:team@example.test" press=noop contextmenu=linkMenu testId="the-team"
        text "the team"
      button press=noop contextmenu=buttonMenu testId="options"
        text "Options"
      text `link menus: ${linkMenus}, button menus: ${buttonMenus}` testId="counts"
```

VoiceOver was not turned on, and no real input was used. The session was locked. While it is locked, an `AXUIElement`
read from another process stops at the application element: `AXWindows` returns an element whose role is
`AXApplication`, and a hand-built AppKit app answers the same way. So scenario B calls AppKit's two `AXShowMenu` entry
points on each node in-process, through a probe dylib loaded into the built binary (`DYLD_INSERT_LIBRARIES`; the
development build has no hardened runtime).

| Scenario | Platform | Revision | Actual | Expected | Evidence |
|---|---|---|---|---|---|
| A: the agent: `type the-team key ContextMenu`, then `tap options contextmenu` | macOS, agent | main `4e6a45239` | counts `0, 0` → `1, 0` → `1, 1`: both handlers run | (the control) | record A |
| B: `-accessibilityPerformShowMenu` and `-accessibilityPerformAction:@"AXShowMenu"` on the link's and the button's `NodeView`, then a synthesized right-click on each | macOS, in-process probe | main `4e6a45239` | `accessibilityPerformShowMenu` returns `NO` on both; `NodeView` has no implementation of its own (its IMP is `NSView`'s); counts stay `0, 0`; after the right-clicks `1, 1` | show menu runs each node's `contextmenu`: counts `1, 1` before any click | record B |
| C1: AppKit: a role-link `NSView` with no `menu`, the same view with one, an `NSButton` with one | macOS, in-process | — | actions `[]`, `[AXShowMenu]`, `[AXShowMenu]` | (the platform: a view with a menu offers show menu) | record C2 |
| C2: Chrome, read from source (not run because of the lock) | Chromium `58d4d3e64`, Blink `212fee7ea` | — | `browser_accessibility_cocoa.mm` `SupportsShowMenuAction()` is true for every web content node, so `AXShowMenu` is listed. `accessibilityPerformShowMenu` → `ShowContextMenu` → Blink `AXObject::OnNativeShowContextMenuAction()` → `ShowNonLocatedContextMenu(element, kKeyboard)`, which fires `contextmenu` at the element | (the reference) | record C3 |
| C3: the macOS host read from another process while unlocked (the X78 record) | macOS | main `474999b9f` | a `link href` node is an `AXLink` whose only action is `AXPress` (that link had no `contextmenu`) | — | record C4 |

Record (the contract, the commands and their output, the oracle's source, and the code read):
https://raw.githubusercontent.com/ccheever/exact2/e0fe7119e6533851572e1b2d0baa0ec0a65d8b09/fw-issues-20261012-x84/x84-record.txt.
The probe's source:
https://raw.githubusercontent.com/ccheever/exact2/cf6dd83c55ebc7b476dd1d6da8d2542805bbbb49/fw-issues-20261012-x84/x84-showmenu-probe.m.txt.

Not run: VoiceOver itself; `AXShowMenu` from another process (blocked by the lock); Chrome's show menu (read from
source); iOS, Linux and Windows; a `contextPopover` node (the same dispatch as `contextmenu`, read from code).

## Constraints

- Show menu should take the ContextMenu key's path. Chrome does: Blink sends both through
  `ShowNonLocatedContextMenu` with a keyboard source. The node's `contextmenu` (and `contextPopover`) would run at the
  node's drawn middle, the point `contextMenuKey` already computes (`drawnRect`, clipped by overflow and clip views).
  Then a show-menu request and the ContextMenu key behave the same way, and an app's own menu code needs no change.
- Show menu should be listed only where something answers it: a node with a `contextmenu` handler or a
  `contextPopover`. AppKit lists it only for a view with a menu (C1); Chrome lists it for every web node. Offering it
  where nothing opens would leave VoiceOver's command silent.
- `preventDefault` semantics are unchanged: a `contextmenu` that does not prevent the default leaves the request to
  the next responder, as a right-click and the key do now.
- Where an app wants the system's own menu (`NSView.menu`), nothing changes.
- No open or closed issue on main covers it, and neither do the GitHub issues (searched `AXShowMenu`, `show menu`,
  `VoiceOver`). Main PR #327 (head `14493d253`) does not touch it: no `ShowMenu` or `menu(for:)` in its diff.

## Acceptance criteria

- In the repro, `AXShowMenu` on the link's element (from another process, or VO-⇧-M with VoiceOver on) runs
  `linkMenu` once and on the button's runs `buttonMenu` once; each element lists `AXShowMenu` next to `AXPress`.
- A node with neither `contextmenu` nor `contextPopover` lists no `AXShowMenu`.
- A `contextPopover` node opens its popover for `AXShowMenu`, at the node, as for the ContextMenu key.
- A host test (`host/apple` XCTest) performs `accessibilityPerformShowMenu` on a `NodeView` with a `contextmenu` handler
  and sees its action run, and sees `false` on a node with none.
- Right-click and the ContextMenu key behave as before.
