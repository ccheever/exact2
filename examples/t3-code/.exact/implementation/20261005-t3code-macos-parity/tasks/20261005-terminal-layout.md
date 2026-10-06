---
name: 20261005-terminal-layout
plan: 20261005-t3code-macos-parity
implementation: in-progress
verification: unverified
delivery: open
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-terminal-drawer
pr_url: https://github.com/ccheever/exact2/pull/175
verified_commit: null
---

# Terminal tabs, splits, right-panel surface, keys and sidebar indicator

## Outcome

A thread can hold several terminals. The drawer shows a tab list and split views (side by side or
stacked, 4 per group). The right panel has a Terminal surface that closes the way the reference closes it. The reference shortcuts work, including while a terminal has focus. The sidebar and the
command palette show when a thread has a running terminal process. All of it matches the reference
desktop app.

## Scope and exclusions

Included:

1. **Drawer layout**: tab list (shown with 2 or more terminals), group headers, split grid, toolbar
   Split/New/Close, limit 4 per group, lowest free `term-N` ids, active group and terminal.
2. **Right-panel Terminal surface** `terminal:<id>`: "+" menu row (enabled with a project), panel mode,
   splits up to 4, a new surface per "New", surface close confirmation, focus owner `right-panel`, tab
   title and icon. Reuse the clone's surface machinery (`shell.ts` surface rows, `r4-surfaces-panel.ts`).
   **Terminal close behavior** on top of the close seam of `20261005-right-panel-tab-menu`: closing one terminal surface asks once and names every
   terminal in it (`confirmTerminalClose` with all labels, `ChatView.tsx:5890-5918`); the bulk closes (Close others, Close to the right, Close all)
   ask nothing, and the terminal cleanup sends `terminal.close` with `deleteHistory:true` for each terminal (`ChatView.tsx:5828-5844`); a middle-click on
   a terminal tab is a single close and asks; closing the last terminal in a surface removes the surface and closes the panel (`rightPanelStore.ts`
   `closeTerminal`). The menu, rename, Copy path and the close actions themselves belong to `20261005-right-panel-tab-menu` (TN7).
3. **Keys**: `terminal.toggle` (no condition), `terminal.split`, `terminal.splitVertical`, `terminal.new`,
   `terminal.close` (all `terminalFocus`), `rightPanel.close` (`!terminalFocus`). Replace the four fixed
   `terminalFocus:false` / `terminalOpen:false` contexts in `keyboard-dispatch.ts`, `keybinding-settings.ts`,
   `composer-presentation.ts`, `composer-editor-intent.ts`. Page key policy: the chords above and `diff.toggle`
   pass to the app; ⌥←/→ send ESC b/f, ⌘←/→ send ^A/^E, ⌘⌫ sends ^U, ⌘K and ⌃L send `\f`. Hide the ⌘-hold thread
   jump hints while a terminal has focus (`keybindings.ts:320-345`). Block a held ⌘W repeat and a ⌘W during a
   pending close dialog.
4. **Sidebar and palette indicator** for running subprocesses.

Excluded: the right-panel tab context menu, device-tab rename, Copy path, middle-click and the close actions for other surface kinds
(`20261005-right-panel-tab-menu`); session transport, single-terminal drawer, font and theme (`20261005-terminal-drawer`); selection actions,
Add to chat, menus on the terminal, links, scripts, "Run in terminal", "Open terminal" (`20261005-terminal-integrations`);
sign-in terminals (`20261005-sign-in-terminals`); the in-app Browser (`EXACT2-GAPS.md` X1); ⌘W for the whole window
(`20261005-desktop-shell-details`).

## Context and guidance

Parent specification: [spec](../spec.md). Source behavior (T3 Code `1e2ecbd975`):
`ThreadTerminalDrawer.tsx:1228-1262,1392-1716`; `terminalUiStateStore.ts:254-348`; `rightPanelStore.ts:47-51,715-790`;
`ChatView.tsx:4426-4435,4696-4806,5666-5780,5828-5844,5890-5918,7586-7752`; `RightPanelTabs.tsx:344-349,579-593`; tests `rightPanelStore.test.ts:910-985`;
`packages/shared/src/keybindings.ts:25-30`; `keybindings.ts:320-345,439-515`; `Sidebar.tsx:347-349,509-518,1673-1685`;
`ThreadStatusIndicators.tsx:69-72,759-771,981-1024`; `DesktopWindow.ts:628-664`.
Behavior to keep. A split adds the new terminal after the active one and sets the group's direction for all its
members; at 4 it does nothing and the tooltip reads `Split Terminal Horizontally (max 4 per group)`. The tab list is
144 pt wide with a 22 pt toolbar; group headers read Single, Side by side, Stacked, with a count. "New" in the panel
makes a new surface; "New" in the drawer makes a new group. Closing a surface asks once and names every terminal in it.
Library revision: `20261005-platforms-v3`. Selected topics: layout-and-interaction, accessibility, design (complete states),
motion (pulse), testing-and-debugging. Menus, key monitors, hooks and WKWebView are unknown in the library; the clone's own code
is the basis (`T3ContextMenu.swift` for menus, `R8KeysLauncher.swift` and `R9Input.swift` for key paths).
Consumer framework revision: the `main` pin of `20261005-clone-on-exact2-main`. Scheduling preference: after `20261005-main-fix-adoption`
(sidebar and tooltip Contract).
Line numbers are from the mc-orch tree on 2026-10-05; `20261005-hot-file-split` moves code, so find it by symbol.
Tools are named by their `target/t3-ui-parity/…` path (committed under `examples/t3-code/tools/` with the same relative paths, decision U23): `electron-oracle.mjs`, `trace-proxy.mjs`,
`trace-diff.mjs`, `drive.mjs`, `lane-backend.sh`.

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| merged task PR | [20261005-clone-on-exact2-main](20261005-clone-on-exact2-main.md) | pending | Merged | pending |
| merged task PR | [20261005-desktop-oracle-and-trace](20261005-desktop-oracle-and-trace.md) | pending | Merged | pending |
| merged task PR | [20261005-terminal-drawer](20261005-terminal-drawer.md) | pending | Merged | pending |
| merged task PR | [20261005-right-panel-tab-menu](20261005-right-panel-tab-menu.md) | pending | Merged (this ticket registers the terminal close guard and cleanup in its close seam and uses its bulk close actions) | pending |
| merged task PR | [20261005-hot-file-split](20261005-hot-file-split.md) | pending | Merged into `daehyeon/t3-code` (common prerequisite: room and per-area seams in the shared files) | pending |

## Issue assessment at preparation

Checked sources and time: plan issue drafts in [issues](../issues/README.md), 2026-10-05; not reproduced, not searched upstream. No prior attempt.

| Issue / reference | Capability and target | Evidence / affected revision | Impact | Next action |
| --- | --- | --- | --- | --- |
| [X8](../issues/20261005-x08-agent-pointer-native-views.md) | Pointer input for native views | `EXACT2-GAPS.md` X8 | nonblocking (workaround: `(attended session)`) | Tabs and toolbar are Contract: the agent drives them; clicks inside the terminal canvas are attended |
| [X25](../issues/20261005-x25-keyboard-keyup-code-capture.md) | Key facts, repeat, capture phase | `EXACT2-GAPS.md` X25 | nonblocking (workaround: native key monitors) | Repeat guard and focus fact in `R8Keys*`/`R9Input` style code |
| [X15](../issues/20261005-x15-non-latin-key-equivalents.md) | Chords under Korean 2-Set | `R10Connect.swift` | nonblocking | Check ⌘D, ⌘N, ⌘W, ⌃L in the terminal under Korean 2-Set 2026-10-07: #110 closed by main #168, which covers declared chords and the host's command items only; `R10Connect.swift` and the key-code fallbacks stay (adopt-main-fixes-input). |
| [X26](../issues/20261005-x26-app-menu-control.md) | App menu items and key equivalents | `EXACT2-GAPS.md` X26 | nonblocking | ⌘W and ⌘N must not trigger the menu while a terminal has focus |
| [X13](../issues/20261005-x13-hover-keys-during-pan.md) | Hover and keys during a pan | `EXACT2-GAPS.md` X13 | nonblocking | None expected here |
| [X9](../issues/20261005-x09-root-component-across-files.md) | Resources in child components | Line cap | nonblocking until the cap | New state in `terminal-*.ts`, not `app.contract` |

## Implementation notes

- Port with tests (`bun:test`, original names): `terminalCloseShortcut.ts` `preventTerminalCloseShortcut`,
  `preventRepeatedTerminalCloseShortcut` (3 tests); the terminal describes of `keybindings.test.ts` (`:183,282,1076,1122,1150`:
  toggle, split/new/close, `isTerminalClearShortcut`, `terminalDeleteShortcutData`, `terminalNavigationShortcutData`);
  `terminalStatusFromRunningIds` (no upstream test: add one named after the function); from `rightPanelStore.test.ts`: "tracks one surface per
  terminal session" (`:910`), "tracks vertical layout for a terminal surface" (`:957`), "closing the final terminal pane removes its surface and closes the
  panel" (`:971`), over the store actions `openTerminal`, `splitTerminal`, `activateTerminal`, `closeTerminal` (the two migration tests at `:225,:250` are
  `n/a` for a new store). Register the terminal kind's close guard and cleanup through the seam. `terminalFocus.test.ts` (DOM) is `n/a-ui`:
  the focus owner is a native fact. Extract the drawer's pure layout decisions (`visibleTerminalIds`, `showGroupHeaders`,
  `hasReachedSplitLimit`, the action labels) from `ThreadTerminalDrawer.tsx:1212-1268` into `terminal-layout.ts` with new tests; record
  the extraction in the file header.
- Keep the page key policy in the page (it must answer in the same event). TS sends the chord list for the contexts
  `terminalFocus:true, terminalOpen:true` (the clone's `chordWinners`, `keyboard-dispatch.ts`). The hook reports focus changes;
  TS turns them into the real `terminalFocus` and `terminalOpen` values.
- Keys while the terminal has focus (read from the reference at `1e2ecbd975`, recorded by terminal-drawer):
  `ThreadTerminalDrawer.tsx:745-781` `handleBeforeKey` hands these to the app instead of the terminal:
  the close shortcut (`preventTerminalCloseShortcut`, ⌘W), `terminal.toggle` (⌘J), `terminal.split` (⌘D),
  `terminal.splitVertical` (⇧⌘D), `terminal.new` (⌘N) and `diff.toggle`, each evaluated with
  `terminalFocus:true, terminalOpen:true`. It keeps for the terminal: navigation chords
  (`terminalNavigationShortcutData`), delete chords (`terminalDeleteShortcutData`) and clear (⌘K → `^L`,
  `isTerminalClearShortcut`). The window handler (`ChatView.tsx:7583-7600`) still runs with
  `terminalFocus:true` when the terminal owns focus, so commands whose default binding has no
  `!terminalFocus` condition (`keybindings` defaults, e.g. ⌘B, ⌥⌘B, ⇧⌘J, ⌘O, ⇧⌘[ / ⇧⌘], ⌘1–9) also apply;
  those with `!terminalFocus` (⌘K palette, ⌘P, ⌘N new chat, …) do not. Check the Ghostty surface's
  `beforeKey` path (`terminal/ghostty/surface.ts:1057`) for which events reach the window before porting.
  terminal-drawer passes only ⌘J through today.
- Sidebar: teal (`text-teal-600`, dark `text-teal-300/90`) terminal icon, 14 pt in the row, 12 pt in the hover card; pulse 2 s stepped
  opacity 1 → 0.5 → 1 (`index.css:302-315`), no pulse under reduced motion. Row label `N terminal process(es) running`; palette tooltip
  `Terminal process running`. The server polls every 1 s, so the icon lags by up to about 1 s (`Manager.ts:100`).
- States. Disabled: split buttons at 4 are dimmed (`opacity-64`) and inert; no project disables the surface row (hint "Available when a
  project is open."). Hover and keyboard focus on all toolbar and
  tab buttons; the tab close button replaces the icon (`PanelTabCloseButton`). Empty: "No terminal sessions for this thread yet." Error:
  `[terminal]` lines. Permission: `terminal:operate` (TN2). Dialog: the surface close dialog closes on Escape without closing anything and returns focus to the terminal when an action ran. Every icon
  button has an `aria-label` with its shortcut text. Motion: only the sidebar pulse; reduced motion stops it. The dialog does not animate.

## Acceptance and reproduction

| Criterion | Setup/reset and fixture | Action or command | Expected result | Required platform | Proof |
| --- | --- | --- | --- | --- | --- |
| Ported tests | — | `bun test examples/t3-code` | The named tests pass | macOS | log |
| Tabs and splits | Lane backend (`target/t3-ui-parity/lane-backend.sh`, isolated HOME, ports 16000–16999), project thread | Agent: New, Split, Split vertical, close one, New again | Tab list at 2+; ids reuse the lowest free `term-N`; 5th split is refused; `terminal.open` payloads equal the oracle's (`target/t3-ui-parity/trace-diff.mjs`) | macOS 1280×840 | `--json`, trace diff |
| Layout pairs | Same, 1, 2 and 4 terminals, both directions | Screenshots next to `target/t3-ui-parity/electron-oracle.mjs` shots | Pairs match at both sizes, light and dark | macOS | pairs |
| Right-panel surface | Same | "+" › Terminal; ⌘N; split; close surface | Titles `Terminal N`; the confirm lists every label; surfaces return after relaunch | macOS both sizes | transcript, pair |
| Terminal close behavior | Panel with a terminal surface of 2 terminals and a files surface | Close the surface; Cancel; Confirm. Then Close others, Close to the right, Close all (through `20261005-right-panel-tab-menu`); middle-click | The single close asks once and names both labels; the bulk closes ask nothing and the trace shows `terminal.close` with `deleteHistory:true` for each terminal (`target/t3-ui-parity/trace-diff.mjs`); a middle-click asks; closing the last terminal pane removes the surface and closes the panel | macOS both sizes | `--json`, trace |
| Close dialog keys | Surface with 2 terminals | Close; Escape; Close; Enter on the default control | Escape keeps both; the confirm names both labels | macOS | `--json` |
| Shortcuts | Terminal focus, then composer focus | Agent `press` ⌘J, ⌘D, ⇧⌘D, ⌘N, ⌘W | Terminal focus: split, split, new, confirm; composer focus: ⌘D toggles diff, ⌘W closes the panel surface; `state` shows the real focus and open values | macOS | `--json` |
| Key repeat `(attended session)` | Lane build with `T3_LOCAL_HOME=<lane>/t3-home` and `T3_LOCAL_PORT=<lane port 16xxx>`, normal launch; terminal focus, close dialog | Hold ⌘W | Only one dialog; no window closes; matches the oracle | macOS | notes |
| Korean 2-Set `(attended session)` | Same lane build; terminal focus | ⌘D, ⌘N, ⌘W, ⌃L, ⌃C | Same result as the US source | macOS | notes |
| Sidebar indicator | Terminal with `sleep 30`; second with `sleep 30` | Wait, agent `clock`, screenshots | Icon and `aria-label` `1` then `2 terminal processes running`; idle shell shows none; icon clears when the process ends; pair; reduced motion has no pulse | macOS both sizes | `tree --ax`, pair, frames |
| Standard gates | `git add -A` | Clone checks (bun test, strict tsc, contract build, `cargo test -p t3-code-macos --lib`, affected AppKit binaries), `bun scripts/caps.mjs`, the five repository checks | Green; every moved matrix cell is fixed, or declared in `EXACT2-GAPS.md` with an issue link | macOS | logs |

Task-owned source paths: `terminal.contract`, `terminal-*.ts` and tests, `modules/apple/T3Terminal*.swift`, `keyboard-dispatch.ts`,
`keybinding-settings.ts`, `composer-presentation.ts`, `composer-editor-intent.ts`, `shell.ts`, `r4-surfaces-panel.ts`, `r4-surfaces.contract`,
sidebar Contract and model, `macos/tests/terminal/**`.
Required environment: lane backend at the pin, oracle build, Xcode 27.0, Bun 1.4.2; lane builds set `T3_LOCAL_HOME=<lane>/t3-home` and
`T3_LOCAL_PORT=<lane port 16xxx>` (dev and lane builds refuse the real `~/.t3` and port 3773).

## Progress

Implemented on PR #175 with the expanded terminal work. The actual native app exercises terminal groups, horizontal/vertical splits, the four-pane limit, independent right-panel terminals, terminal-owned shortcuts and busy-process labels. Reused native session callbacks and queued resizes are fenced by session identity. This does not close every acceptance permutation.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| Expanded native verification, 2026-10-07 | Source fingerprints in linked evidence | Groups/splits/panel/keys/busy indicators driven; native and real-server regressions pass | [Matrix](https://github.com/ccheever/exact2/blob/t3-code-evidence/evidence/terminal-drawer/20261007-expanded-parity/MATRIX.md) | Fine-grained remaining subcases remain explicit |

## Next action

Review the combined implementation and recorded runtime evidence on PR #175. Keep verification open for the remaining matrix subcases; the original Browser route remains a separate full-parity blocker.
