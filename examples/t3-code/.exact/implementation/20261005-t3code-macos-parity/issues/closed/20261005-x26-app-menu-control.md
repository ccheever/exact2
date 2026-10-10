---
name: 20261005-x26-app-menu-control
plan: 20261005-t3code-macos-parity
status: moved-to-main
kind: framework-gap
blocks: [20261005-app-developer-tools, 20261005-app-update-feed, 20261005-browser-surface, 20261005-desktop-shell-details, 20261005-legacy-sidebar, 20261005-media-actions, 20261005-pr-handoffs-and-quick-actions, 20261005-right-panel-tab-menu, 20261005-ssh-password-and-remote-open, 20261005-terminal-integrations, 20261005-terminal-layout]
upstream_url: https://github.com/ccheever/exact2/issues/141
reproduced_on: null
---

# X26: App menu control (declared application menu, hide host Go/Develop, zoom, submenus, menu at a point)

Moved to main `issues/20261009-contract-menu-bar-extensions.md` (2026-10-09); tracked there.

## Summary

T3 Code's desktop app builds its own application menu bar and shows native context menus with submenus, check marks and a position. In exact2 the host's menu bar is fixed and an app cannot declare menus. The clone rewrites the host's menu bar from its Swift module and pops its own `NSMenu`s.

## Why it arose

### The T3 Code behavior

**Application menu** (`apps/desktop/src/window/DesktopApplicationMenu.ts`, template built in `configure`, lines 156–268; test `window/DesktopApplicationMenu.test.ts`). On macOS the bar is:

- App menu: About (role), "Check for Updates...", separator, "Settings..." (⌘,), separator, Services, separator, Hide, Hide Others, Show All, separator, Quit.
- File: Close (role `close`, ⌘W).
- Edit: Undo, Redo, separator, Cut, Copy, Paste, "Paste as Text" (⇧⌘V), Delete, separator, Select All, separator, Speech › Start Speaking, Stop Speaking.
- View: Reload, Force Reload, Toggle Developer Tools (see X2), separator, "Actual Size" (⌘0), "Zoom In" (⌘=), a hidden second "Zoom In" (⌘+), "Zoom Out" (⌘-), separator, Toggle Full Screen. The zoom items act on the main window (0.5-level steps, `DesktopWindow.ts:1008-1017`), not the focused page.
- Window (role `windowMenu`), Help (role `help`) with "Check for Updates...".

Menu clicks that need the page are sent to it as an action name (`open-settings`, `paste-as-text`); "Paste as Text" ignores a click that came from its accelerator, because the page already handled the keystroke (lines 140–154).

**Context menus** (`apps/desktop/src/electron/ElectronMenu.ts`, `showContextMenu` lines 225–282; types `packages/contracts/src/ipc.ts:32–62`). The page sends items `{id, label, destructive?, disabled?, separatorBefore?, checked?, children?}` and an optional position; the menu shows at that position or at the pointer; the reply is the chosen id or none. Destructive items get a separator and a 12 pt trash symbol on macOS. About 19 call sites use it (thread and project rows in `Sidebar.tsx` and `LegacySidebar.tsx`, `RightPanelTabs.tsx`, `ChatMarkdown.tsx`, `ThreadTerminalDrawer.tsx`, `BranchToolbarBranchSelector.tsx`, `FileBrowserPanel.tsx`, `SettingsPanels.tsx`, `externalLinkContextMenu.ts`, `pullRequestLinkContextMenu.ts`, `fileContextMenu.ts`).

### Where the clone hits it

The host menu bar was fixed (`Mac/DevMenuMac.swift:141-181`) and submenus were out (LLP 1021:614). Eleven tickets add or change a menu or context menu (`blocks`); the ones that need a menu at a given point (keyboard-opened menus) are `20261005-media-actions`, `20261005-right-panel-tab-menu`, `20261005-pr-handoffs-and-quick-actions`, `20261005-terminal-integrations` and `20261005-legacy-sidebar`.

## Clone workaround

- `modules/apple/R8KeysMenus.swift` (`arrange(_:)`): after the host builds its bar, the module finds the menus by title, removes "Develop" and "Go", removes AppKit's tab items, keeps File as only Close Window, and turns every other host-filed ⌘ chord item (File's and Edit's app commands) into a hidden key equivalent. Edit › Undo and File › Close Window route their chord to the window's command first.
- `modules/apple/T3Menus.swift`: adds "Paste as Text" (the responder chain's `pasteAsPlainText:`), Actual Size, Zoom In (and the hidden ⌘+ item), Zoom Out, the app menu "Check for Updates...", and a Help menu, by editing the host's `NSMenu` found by title.
- Context menus: the sidebar's thread-row and draft-row menus are host context popovers since main #223 (below). The module's `contextMenu` menus (`T3ContextMenu.swift`: links, archive, branch, media, the right-panel tab's keyboard menu) are flat `NSMenu`s at the pointer. Two right-click menus with submenus still use the module's `NSMenu`: the Files tree row's (Open with ▸) and the legacy sidebar project's; they look the same as the reference's, but the agent cannot choose from them (converting them is a follow-up).
- Keyboard-opened row menus: since round 7 a thread row's ContextMenu key is the host's (#314); Shift+F10 and the draft and legacy rows' anchored menus stay `T3Sidebar.swift`'s (`anchorPoint`, centre or bottom-left of the focused view).
- `T3MenuTurn` keeps native work running while an `NSMenu` tracks ([#292](https://github.com/ccheever/exact2/issues/292), main `issues/20261009-macos-menu-stalls-native-work.md`).
- Declared differences from the plan's records (the audit's inventory E6 and `20261005-desktop-shell-details` "Excluded"): Reload and Force Reload are shown only while the host's Develop menu supplies the reload action, and the full-screen item's title is the host's. #141's decision (2026-10-08) keeps Develop in development builds, a declared difference there only; the menu bar surgery becomes Contract menu items when main builds them.

## Evidence and history

- Filed 2026-10-06 as [#141](https://github.com/ccheever/exact2/issues/141).
- 2026-10-07 ([adopt-main-fixes-r5](../../tasks/closed/20261007-adopt-main-fixes-r5.md), main `261dd4e10`):
  - **#223 (`d988e318b`): context-menu submenus.** A menu row whose `popovertarget` names another menu popover is a submenu `NSMenuItem` on macOS and a nested popover under the agent. Adopted: the sidebar's thread-row and draft-row right-click menus are context popovers (`sidebar-row.contract` ThreadMenu, DraftMenu; `sidebar-menu.ts` `menuRows`; `runChoice`, `bulkChoice`, `draftChoice`), with Snooze ▸, Auto-settle behavior ▸ and Copy ▸ as nested popovers. A real right-click shows the same `NSMenu` the module built before; the agent can now open and choose the menu (Snooze ▸ In 1 hour ran), where the module's menu answered "dismissed".
  - **#226 (`dfcf8e9cf`): Edit ▸ Speech, paste variants after Paste.** `T3Menus.swift` no longer adds its own Speech; `R8KeysMenus.swift` keeps Edit's app commands as hidden key equivalents, so Edit is DesktopApplicationMenu.ts's again.
  - Keyboard-opened row menus filed as [#235](https://github.com/ccheever/exact2/issues/235) (2026-10-07).
- 2026-10-08: [#292](https://github.com/ccheever/exact2/issues/292) filed (main-queue work stalls while an `NSMenu` tracks).
- 2026-10-10 ([adopt-main-fixes-r7](../../tasks/closed/20261010-adopt-main-fixes-r7.md), #384): main #314 (`d236c36d5`, #235's ContextMenu key) adopted: a thread row's ContextMenu opens its host context popover at the row's centre; Shift+F10 stays the module's (the reference binds it in its own key handlers).
