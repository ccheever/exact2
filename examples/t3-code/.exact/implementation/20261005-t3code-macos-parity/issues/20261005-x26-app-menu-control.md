---
name: 20261005-x26-app-menu-control
plan: 20261005-t3code-macos-parity
status: closed-upstream
kind: framework-gap
blocks: [20261005-app-developer-tools, 20261005-app-update-feed, 20261005-browser-surface, 20261005-desktop-shell-details, 20261005-legacy-sidebar, 20261005-media-actions, 20261005-pr-handoffs-and-quick-actions, 20261005-right-panel-tab-menu, 20261005-ssh-password-and-remote-open, 20261005-terminal-integrations, 20261005-terminal-layout]
upstream_url: https://github.com/ccheever/exact2/issues/141
reproduced_on: null
---

# X26: App menu control (declared application menu, hide host Go/Develop, zoom, submenus, menu at a point)

## Summary

T3 Code's desktop app builds its own application menu bar and shows native context menus with submenus, check marks and a position. In exact2 the host's menu bar is fixed and an app cannot declare menus. The clone rewrites the host's menu bar from its Swift module and pops its own `NSMenu`s. What is needed is a supported way for an app to declare its menu bar and to pop a context menu at a point, so the result equals the reference without reaching into host menus.

## Why this issue arose

### The T3 Code behavior

**Application menu** (`apps/desktop/src/window/DesktopApplicationMenu.ts`, template built in `configure`, lines 156–268; test `window/DesktopApplicationMenu.test.ts`). On macOS the bar is:

- App menu: About (role), "Check for Updates...", separator, "Settings..." (⌘,), separator, Services, separator, Hide, Hide Others, Show All, separator, Quit.
- File: Close (role `close`, ⌘W).
- Edit: Undo, Redo, separator, Cut, Copy, Paste, "Paste as Text" (⇧⌘V), Delete, separator, Select All, separator, Speech › Start Speaking, Stop Speaking.
- View: Reload, Force Reload, Toggle Developer Tools (see X2), separator, "Actual Size" (⌘0), "Zoom In" (⌘=), a hidden second "Zoom In" (⌘+), "Zoom Out" (⌘-), separator, Toggle Full Screen. The zoom items act on the main window (0.5-level steps, `DesktopWindow.ts:1008-1017`), not the focused page.
- Window (role `windowMenu`), Help (role `help`) with "Check for Updates...".

Menu clicks that need the page are sent to it as an action name (`open-settings`, `paste-as-text`); "Paste as Text" ignores a click that came from its accelerator, because the page already handled the keystroke (lines 140–154). "Check for Updates..." shows "You're up to date!", "Update check failed" or "Updates unavailable" boxes (lines 60–110; update behavior is a T3 update feed matter and stays excluded).

**Context menus** (`apps/desktop/src/electron/ElectronMenu.ts`, `showContextMenu` lines 225–282; types `packages/contracts/src/ipc.ts:32–62`, bridge member at :1211). The page sends items `{id, label, destructive?, disabled?, separatorBefore?, checked?, children?}` and an optional position; the menu shows at that position (CSS pixels multiplied by the page zoom, lines 100–110) or at the pointer; the reply is the chosen id or none. Header items are skipped; a submenu with no visible children is dropped. Destructive items get a separator and a 12 pt trash symbol on macOS. About 19 call sites use it (thread and project rows in `Sidebar.tsx` and `LegacySidebar.tsx`, `RightPanelTabs.tsx`, `ChatMarkdown.tsx`, `ThreadTerminalDrawer.tsx`, `BranchToolbarBranchSelector.tsx`, `FileBrowserPanel.tsx`, `SettingsPanels.tsx`, `externalLinkContextMenu.ts`, `pullRequestLinkContextMenu.ts`, `fileContextMenu.ts`). Tests: `electron/ElectronMenu.test.ts`, web `contextMenuFallback.test.ts`, `localApi.test.ts`.

### What exact2 does today

From `EXACT2-GAPS.md` (written by earlier sessions from framework source at `c1522fdac`, checked against `main` `d2cb661eb`; citations as given there): table row X26 "App menu control: standard items (Paste as Text, Speech, Help), hide host Go/Develop, page zoom, submenus, menu at the pointer"; and "The host menu bar is fixed (`Mac/DevMenuMac.swift:141-181`). App chords now win over host items (`9824f0e3a`). Submenus are out (LLP 1021:614)."

Bundled library (`20261005-platforms-v3`): menus are not covered; macOS "pointer/menu behavior" appears only as a verification priority (`platforms.md`). **Not covered: unknown.** Anything beyond the quoted lines is to confirm at `issue-open`.

### Where the clone hits it

- `modules/apple/R8KeysMenus.swift` (header, `arrange(_:)` from line 27): after the host builds its bar, the module finds the menus by title, removes "Develop" and "Go", removes AppKit's tab items, keeps File as only Close Window, and turns every other host-filed ⌘ chord item into a hidden key equivalent. Edit › Undo and File › Close Window route their chord to the window's command first.
- `modules/apple/T3Menus.swift` (header; lines 65–80, 123–138): adds "Paste as Text", Speech, Actual Size, Zoom In (and the hidden ⌘+ item), Zoom Out, the app menu "Check for Updates...", and a Help menu, by editing the host's `NSMenu` found by title.
- Two context-menu builders in the module. `modules/apple/T3ContextMenu.swift` (lines 12–20) pops a flat `NSMenu` at the pointer (separator and trash symbol for destructive items). `modules/apple/T3Sidebar.swift` (`menu(_:target:)` from line 130 and the `menu` operation around lines 96–110) builds the reference's template with submenus, check marks and the trash symbol, and `anchorPoint` places a keyboard-opened menu at the centre or bottom-left of the focused view (Chromium's keyboard `contextmenu` behavior). Under the agent that operation never shows a menu: it answers "dismissed" with where the menu would open (comment at line 101), so an agent drive cannot choose a menu item; only an attended session or an AppKit test can.
- Tickets that need a menu at a given point (keyboard-opened menus) reuse that helper: `20261005-media-actions`, `20261005-right-panel-tab-menu`, `20261005-pr-handoffs-and-quick-actions`, `20261005-terminal-integrations`, `20261005-legacy-sidebar`.

What differs from the reference, by the plan's own records (the audit's inventory E6 and the ticket `20261005-desktop-shell-details` "Excluded"): Reload and Force Reload are shown only while the host's Develop menu supplies the reload action, and the full-screen item's title is the host's, not the reference's. These were seen by the audit, not re-observed here.

## Why it must be resolved

Parity goal: the menu bar and context menus are part of the desktop app, and the plan must end with them equal to the reference, not with a declared deviation. Today the equality depends on string-matching the host's menu titles (`"Develop"`, `"Go"`, `"Edit"`, `"View"`, `"Help"`) at runtime: a host change (renamed or restructured menu, another language) breaks the bar silently, and the result can only be checked by a person or an AppKit test. Eight tickets add or change a menu or context menu and each writes native Swift for it (listed in `blocks`). Cost of keeping the workaround: three Swift files that must track the host's menu code, two separate context-menu builders, no agent drive that can pick a menu item (only attended sessions), and the two visible differences above.

## Requested support

The web has no application menu; Electron's model (`Menu.setApplicationMenu(template)`, `Menu.popup`) is the closest reference. Proposed, macOS host first:

- **A. Declarative (preferred).** An app-declared menu template (in `app.json` or the Contract): menus and items with `label`, `role` for the standard items (about, services, hide, hideOthers, unhide, quit, close, undo, redo, cut, copy, paste, delete, selectAll, startSpeaking, stopSpeaking, togglefullscreen, windowMenu, help), `accelerator`, `visible`, `enabled`, `checked`, `submenu`, separators, and a click that runs a Contract action with a flag for "triggered by accelerator". The host's own menus are replaced by the template, so Develop and Go are simply absent. Plus a context-menu operation that takes a point or rect, items with submenus, checks, destructive and disabled flags, and returns the chosen id.
- **B. Module hook (smaller).** A documented hook that hands the module the host's menu bar to rewrite, with stable identifiers for the host's items instead of titles. This formalizes what the clone does; it still needs Swift per app.

Trade-offs: A touches the host, the Contract grammar and the agent (to read the bar); B needs only the host. The web-standard direction for context menus is the `contextmenu` event plus a popover; native menus need the host either way.

## How to reproduce

To confirm on the pinned `main` at `issue-open`:

1. Minimal app with no module: launch and read the menu bar. Expected (the reference list above, or an app-declared bar): only the app's menus. Actual (per `EXACT2-GAPS.md`): the host's fixed bar, including Develop and Go.
2. Clone: `bun host/apple/build.mjs t3-code-macos --bundle --run` with `EXACT_APP_DIR` set; compare the View menu with the list above (Reload, Force Reload, separator, Actual Size, Zoom In, Zoom Out, separator, Toggle Full Screen) and the File menu (Close Window only). Remove the module's `arrange` call (local, uncommitted) and observe the host bar return.
3. Context menu: open the draft row menu and a thread row menu; try to open a menu from the keyboard at the focused row; expected: it opens at the row, with Copy ▸ submenu and check marks where the reference has them.

## Acceptance for the fix

- An AppKit test (or agent operation, to be defined with the maintainers) lists `NSApp.mainMenu` titles, items, key equivalents and enabled state; for the reference template they equal the list above.
- A click on "Settings..." and "Paste as Text" runs the Contract action; an accelerator-triggered "Paste as Text" does not run the click action twice.
- The context-menu operation returns the chosen id, `null` on dismiss, shows submenus and check marks, and honors the point.
- No hidden duplicate key equivalents are needed for button chords (⌘ chords declared on the page work without being filed in File).
- Conformance case against Chrome: not applicable (no web equivalent).

## App adoption after resolution

Replace `R8KeysMenus.swift` surgery and the additions in `T3Menus.swift` with the declared template; rewrite `T3ContextMenu.swift` and the sidebar menu builder in `T3Sidebar.swift` on the new operation; delete the host-title lookups. Reopen the "declared deviation" lines in `20261005-desktop-shell-details` (Reload/Force Reload, full-screen title) as rows that must pass. `issue-close` verifies the AppKit menu list and the eight tickets' menu rows.

## Status and next action

Draft; not reproduced on the pinned `main`; not searched upstream; not published.
Next: `issue-open` (reproduce, search for duplicates, prepare the report for the user's approval; publication only after approval).

## Merged upstream in part (2026-10-07, adopt-main-fixes-r5)

Filed as [#141](https://github.com/ccheever/exact2/issues/141), still open for an app-declared menu bar. Two parts
landed (in the feature branch since main `261dd4e10`, [adopt-main-fixes-r5](../tasks/20261007-adopt-main-fixes-r5.md)):
- **#223 (`d988e318b`): context-menu submenus.** A menu row whose `popovertarget` names another menu popover is a
  submenu `NSMenuItem` on macOS and a nested popover under the agent. Adopted: the sidebar's thread-row and draft-row
  right-click menus are context popovers (`sidebar-row.contract` ThreadMenu, DraftMenu). Each row carries its
  menu (`sidebar-menu.ts` `menuRows`, the bulk menu on a selected row), a choice runs `menu-choice` / `draft-choice`
  (`runChoice`, `bulkChoice`, `draftChoice`), and Snooze ▸, Auto-settle behavior ▸ and Copy ▸ are nested popovers
  placed `right span-bottom`. A real right-click shows the same `NSMenu` the module built before (items, separators,
  submenus, the 12 pt trash glyph). The agent can now open and choose the menu (Snooze ▸ In 1 hour ran), where the
  module's menu answered "dismissed".
- **#226 (`dfcf8e9cf`): Edit ▸ Speech, paste variants after Paste.** The host's Edit has Speech, so `T3Menus.swift`
  no longer adds its own (it would have shown twice). #226 also files a ⌘F/⌘D/⌘G button under Edit after Select
  All, which showed the clone's ⇧⌘G as Edit ▸ Branch and ⌘D as Edit ▸ Toggle Diff (at the end, before #226).
  `R8KeysMenus.swift` keeps Edit's app commands as hidden key equivalents, as it does File's, so Edit is
  DesktopApplicationMenu.ts's again: Undo, Redo, Cut, Copy, Paste, Paste as Text, Delete, Select All, Speech.

Kept, with the open issue: the menu bar surgery (`R8KeysMenus.swift`, `T3Menus.swift`: Paste as Text as the responder
chain's `pasteAsPlainText:`, the zoom items, Check for Updates…, Help, removing Develop and Go) for #141; the
keyboard-opened row menus (ContextMenu, Shift+F10 at the focused row) through `T3Sidebar.swift`, since a context
popover opens only from a right-click: [#235](https://github.com/ccheever/exact2/issues/235) (filed 2026-10-07);
the module's `contextMenu` menus (`T3ContextMenu.swift`: links, archive, branch, media, the right-panel tab's keyboard
menu), which are flat and needed no submenu. Two right-click menus with submenus still use the module's `NSMenu` and
are not converted in this round: the Files tree row's (Open with ▸) and the legacy sidebar project's (a grouped
project's per-member submenus). They look the same as the reference's; the agent cannot choose from them. Converting
them as the sidebar's were is a follow-up task (user decision: this round's live-drive budget was spent on the sidebar
menus #223 was filed for). The issue stays open for #141 and #235.
