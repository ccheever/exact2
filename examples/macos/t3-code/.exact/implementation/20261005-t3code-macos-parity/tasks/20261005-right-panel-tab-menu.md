---
name: 20261005-right-panel-tab-menu
plan: 20261005-t3code-macos-parity
implementation: planned
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: daehyeon/t3-code
branch: null
pr_url: null
verified_commit: null
---

# Right-panel tab menu: rename, copy path and close actions

## Outcome

A right-click on a right-panel tab opens the reference's menu. A device tab can be renamed from the
menu or by double-click, with an inline editor. A file tab can copy its path. Every tab has Close, Close
others, Close to the right and Close all, and a middle-click closes one tab. The closing rules (which tab
becomes active, when the panel closes) match the reference. This is the work of gap TN7; it does not depend on
the terminal tickets, and a NO-GO verdict of `20261005-terminal-surface` does not affect it.

## Scope and exclusions

Included:

Also included (TN8): device surfaces keyed by host and device, as the reference keeps them
(`rightPanelStore.ts`; test `rightPanelStore.test.ts:30`), instead of the clone's single
`singleton('device')` surface per thread; Rename then applies per device.

1. **Tab context menu** (`RightPanelTabs.tsx:903-990`), shown at the pointer. Items in the reference order: Rename (device tabs),
   Copy path (file tabs without an attachment), Mute tab / Unmute tab (Browser tabs), Close, Close others, Close to the right,
   Close all. The Mute slot and the tab audio indicator belong to the Browser surface, which this app excludes (`EXACT2-GAPS.md` X1): the
   menu keeps the slot, nothing renders it, and the three `tabMuteMenuItem` tests (`RightPanelTabs.test.tsx:253-283`) are `deferred (X1)`.
2. **Close actions** with the reference's rules: `closeSurface`, `closeOtherSurfaces`, `closeSurfacesToRight`, `closeAllSurfaces`
   (`rightPanelStore.ts:802-852`), the page-level handlers (`ChatView.tsx:5878-5960`), and middle-click close
   (`RightPanelTabs.tsx:998-1009`).
3. **Close seam for other surface kinds**: a surface kind may register a close guard (an optional confirmation for a single close) and a
   cleanup that runs for every close, bulk or not (`cleanupRightPanelSurfaces`, `ChatView.tsx:5828-5876`). This ticket registers none. The
   terminal kind registers its own in `20261005-terminal-layout`.
4. **Rename**: inline editor in the tab (`RightPanelTabs.tsx:1162-1196`), double-click start, and `renameDevice` (`rightPanelStore.ts:632-642`).
5. **Copy path**: the file tab's workspace-relative path to the clipboard with the toasts (`ChatView.tsx:5961-5990`).
6. **`tabContextMenuItems(surface, surfaces)`**: the item list as a pure function extracted from `RightPanelTabs.tsx:914-956`.

Excluded: terminal surfaces, their close confirmation and history deletion (`20261005-terminal-layout`); Browser tabs and Mute behavior
(X1); the tab strip's scrolling and persistence (round 12, `20261005-round12-wrapup`); device tab tooltips.

## Context and guidance

Parent specification: [spec](../spec.md). Source behavior (T3 Code `1e2ecbd975`): `apps/web/src/components/RightPanelTabs.tsx:186-228,903-1009,1162-1215`;
`apps/web/src/rightPanelStore.ts:632-642,802-852`; `apps/web/src/components/ChatView.tsx:5878-5990`; tests `rightPanelStore.test.ts:992-1050`.
Rules to keep. Closing the active tab activates the neighbor at the same index (or the last). Close others keeps the chosen tab active and opens the
panel; it does nothing with one tab. Close to the right keeps the active tab if it survives, else activates the chosen tab; it does nothing on the last
tab. Close all closes the panel. Rename: the editor takes focus with its text selected; Enter commits; Escape restores the old title; leaving the field
commits; an empty name becomes the device's name, then "Device". Copy path success toast "Path copied" with the path; failure toast "Failed to copy path"
with the message or "Clipboard API unavailable.". The Rename item shows only for device tabs, Copy path only for file tabs without an attachment.
Clone state (mc-orch tree, 2026-10-05): no right-panel tab context menu, no tab rename, no file Copy path (I found no `contextmenu=` in the tab files and none
of the item names). The clone keeps one device surface per thread (`singleton('device')`, `r4-surfaces-panel.ts:195-202`); the reference keeps one per
host and device. This ticket changes the clone to the reference model (TN8, scope above), so Rename applies per device. Reuse: `T3ContextMenu.swift` (native menus), the sidebar
row's context-menu path, the hook `t3-select-on-open` (`R10Connect.swift`) for the editor, the clone's toast function (`pushToast`, `providers.ts`),
`closeSurfaceIn` and the diff flags (`r4-surfaces-panel.ts:178-186`), the tab strip (`r12-threads-tabs.ts`, `r12-threads.contract`).
Library revision: `20261005-platforms-v3`. Selected topics: layout-and-interaction (identity: `testId`, `id`, `each` keys), accessibility (labels, keyboard
focus), design (complete states), testing-and-debugging. Native menus and key monitors are unknown in the library; the clone's own code is the basis.
Consumer framework revision: the `main` pin of `20261005-clone-on-exact2-main`. Scheduling preference: after `20261005-main-fix-adoption` (tooltips and
popovers; the keyboard context-menu path for rows).
Line numbers are from the mc-orch tree on 2026-10-05; `20261005-hot-file-split` moves code, so find it by symbol.
Tools are named by their `target/t3-ui-parity/…` path (committed under `examples/macos/t3-code/tools/` with the same relative paths, decision U23): `electron-oracle.mjs`, `drive.mjs`, `lane-backend.sh`.

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| merged task PR | [20261005-clone-on-exact2-main](20261005-clone-on-exact2-main.md) | pending | Merged | pending |
| merged task PR | [20261005-hot-file-split](20261005-hot-file-split.md) | pending | Merged into `daehyeon/t3-code` (common prerequisite: room and per-area seams in the shared files) | pending |
| merged task PR | [20261005-desktop-oracle-and-trace](20261005-desktop-oracle-and-trace.md) | pending | Merged (oracle for the menu items and the editor) | pending |

## Issue assessment at preparation

Checked sources and time: plan issue drafts in [issues](../issues/README.md), 2026-10-05; not reproduced, not searched upstream. No prior attempt.

| Issue / reference | Capability and target | Evidence / affected revision | Impact | Next action |
| --- | --- | --- | --- | --- |
| [X8](../issues/20261005-x08-agent-pointer-native-views.md) | Pointer input for native views and native menus | `EXACT2-GAPS.md` X8 | nonblocking (workaround: `(attended session)`) | The agent can open the menu with a context-menu tap and run items by command; the visible menu and middle-click are attended |
| [X26](../issues/20261005-x26-app-menu-control.md) | Menu at the pointer | `EXACT2-GAPS.md` X26 | nonblocking (workaround: `T3ContextMenu.swift`) | Show the native menu at the pointer |
| [X25](../issues/20261005-x25-keyboard-keyup-code-capture.md) | Key facts for the editor and the keyboard menu | `EXACT2-GAPS.md` X25 | nonblocking (workaround: native key monitors) | Follow what `20261005-main-fix-adoption` establishes for the context-menu key on rows; if the host cannot deliver it, declare it |
| [X1](../issues/20261005-x01-chromium-cdp-browser-surface.md) | Browser surface | `EXACT2-GAPS.md` X1 | excluded feature | Mute slot kept, never rendered |
| [X9](../issues/20261005-x09-root-component-across-files.md) | Resources in child components | Line cap | nonblocking until the cap | State in a TS module, not `app.contract` |

## Implementation notes

- Port with tests (`bun:test`, original names): `rightPanelStore.test.ts` "closing the final surface closes the panel" (`:992`), "closing other surfaces keeps
  the selected surface active" (`:1003`), "closing surfaces to the right activates the selected surface when active was removed" (`:1025`), "closing all surfaces
  closes the panel" (`:1039`), "closing the active surface activates a neighboring surface" (`:982`). These tests open Browser and terminal surfaces; use files, diff and device
  surfaces instead and record the substitution in the file header; `renameDevice` (the
  upstream test "gives each host/device its own tab and preserves renamed tabs", `:30`, needs per-device tabs, which this ticket adds: port the whole test (the earlier note to port only its rename lines is replaced) and say which lines
  were dropped; add cases for trim, empty fallback and other kinds untouched). `tabContextMenuItems` has no upstream test: add cases per kind, order and
  disabled flags. Keep the names `closeSurface`, `closeOtherSurfaces`, `closeSurfacesToRight`, `closeAllSurfaces` over the clone's `closeSurfaceIn`; record the
  mapping in the file header.
- Closing the diff surface through any path keeps the clone's diff state in step (`client.diffOpen`, `r4-surfaces-panel.ts:178-186`).
- Menu shape: plain items, no separators, no destructive flag, in the reference order; disabled flags are `Close others` with one tab, `Close to the right` on
  the last tab, `Close all` with no tabs. Keyboard: the tab buttons are in the tab order; the menu follows the host rules for native menus (arrow keys, Escape).
- Editor: `aria-label="Device tab name"`, 96 pt wide, ring focus style, the tab's title as the initial value; it stops key events from reaching tab shortcuts
  (`event.stopPropagation()` in the reference). The surface title lives with the surface so it returns after a relaunch if the surface does.
- States. Loading: a file tab that is still loading keeps Copy path available. Empty: no tabs, no menu. Error: the Copy path failure toast. Disabled: the flags above.
  Hover and keyboard focus: the editor and the tab buttons use the clone's focus ring; the menu returns focus to the panel after an action. Escape closes the menu or
  the editor without closing a tab or changing a name. Permission: none. Every icon-only tab button keeps its `aria-label`. Motion: nothing animates here; the toast uses
  the clone's toast motion (reduced motion follows the toast).

## Acceptance and reproduction

| Criterion | Setup/reset and fixture | Action or command | Expected result | Required platform | Proof |
| --- | --- | --- | --- | --- | --- |
| Ported tests | — | `bun test examples/macos/t3-code` | The named tests pass | macOS | log |
| Menu items | Lane backend (`target/t3-ui-parity/lane-backend.sh`, isolated HOME, ports 16000–16999); panel with a file, an attachment preview, a diff and a device surface | Agent context-menu tap on each tab; read the item list; Escape | Items, order and disabled flags per kind equal the oracle's: Rename only on the device tab, Copy path only on the file tab (not on the attachment), no Mute item; Escape closes nothing. If the agent cannot read the native menu, the AppKit test checks the template and the visible menu is an `(attended session)` row | macOS 1280×840 and 840×620 | `--json`, AppKit test |
| Close actions | Same, five tabs | Close (active, inactive); Close others; Close to the right (middle tab, last tab); Close all | Active tab, open flag and disabled no-ops equal the ported tests and the oracle; the diff state follows the diff tab | macOS | `--json`, `state` |
| Close seam | Unit test with a stub surface kind that registers a guard and a cleanup | Close alone, then via the three bulk actions | The guard runs only for the single close; the cleanup runs for all four; this ticket registers no real guard | macOS | log |
| Rename | Device fixture hub; device tab open | Menu › Rename; type; Enter. Again with Escape. Double-click. Empty name. Relaunch | The editor appears focused with text selected; Enter keeps the new name; Escape keeps the old; empty gives the device's name; the name returns after relaunch if the surface does | macOS | `--json`, shots |
| Copy path | Files surface open on a file; clipboard cleared | Menu › Copy path | The clipboard holds the workspace-relative path; toast "Path copied" with the path; the clipboard-failure path shows "Failed to copy path" (unit test) | macOS | clipboard read, toast shot |
| Pairs | Same scenes: editor open, toast, tab strip with 5 tabs | Screenshots next to `target/t3-ui-parity/electron-oracle.mjs` shots | The editor, the toast and the strip match at both sizes, light and dark | macOS | pairs |
| Real input `(attended session)` | Lane build with `T3_LOCAL_HOME=<lane>/t3-home` and `T3_LOCAL_PORT=<lane port 16xxx>`, normal launch | Right-click each tab kind; read the visible menu; Escape; middle-click a tab; the context-menu key on a focused tab (if the host delivers it) | The visible menu and its position match the oracle; middle-click closes one tab; Escape closes only the menu | macOS | notes, shots |
| Standard gates | `git add -A` | Clone checks (bun test, strict tsc, contract build, `cargo test -p macos-t3-code-apple --lib`, affected AppKit binaries), `bun scripts/caps.mjs`, the five repository checks | Green; every moved matrix cell is fixed, or declared in `EXACT2-GAPS.md` with an issue link | macOS | logs |
| Device tabs per host and device (TN8) | Device fixture with two devices on one host and one on a second host | Open each device; rename one; close one; relaunch | One tab per host+device; the rename stays on its tab only; closing one leaves the others; tabs restored after relaunch as the reference does | macOS 1280×840 | `--json` state + shots beside the oracle; ported `rightPanelStore.test.ts` case |

Task-owned source paths: `r4-surfaces-panel.ts`, `r4-surfaces.contract`, `r12-threads-tabs.ts`, `r12-threads.contract`, `shell.ts`, `modules/apple/T3ContextMenu.swift`,
a new `right-panel-tabs.ts` with its test, `apple/tests/contextmenu/**`, `AGENT-HANDOFF.md`.
Required environment: lane backend with the device fixture hub, oracle build, Xcode 27.0, Bun 1.4.2; lane builds set `T3_LOCAL_HOME=<lane>/t3-home` and
`T3_LOCAL_PORT=<lane port 16xxx>` (dev and lane builds refuse the real `~/.t3` and port 3773).

## Progress

Planned. No branch.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| none | — | — | — | — |

## Next action

`prepare` after the three dependencies merge.
