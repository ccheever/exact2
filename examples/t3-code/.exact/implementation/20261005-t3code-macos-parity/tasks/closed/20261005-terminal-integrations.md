---
name: 20261005-terminal-integrations
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

# Terminal selection, Add to chat, links and script runs

## Outcome

The user can select text in a terminal and get the reference's popup and right-click menu: Add to
chat, Copy, Paste. "Add to chat" puts a terminal chip in the composer, and the sent message shows the
chip like the reference. URL and path links in terminal output open where the reference opens them
(URLs in the system browser). Project scripts, "Run in terminal" on shell code blocks, and the worktree
setup card's "Open terminal" run in or open a terminal. All of it matches the reference desktop app.

## Scope and exclusions

Included:

1. **Selection actions**: popup after a selection settles (Add to chat, Copy), right-click menu (Add to chat,
   Copy, Paste), their positions, and the rules that a newer right-click supersedes a pending popup.
2. **Composer terminal chip**: insert at the caret, record in the draft, expired chip (no text), toast on send,
   the sent-message chip label and popover.
3. **Links**: URL and path activation. A requested in-app Browser route remains unavailable under framework issue #100; the explicit external-browser route is separate. A system-browser fallback does not establish full functional parity.
4. **Scripts**: run a project script in a terminal, "Run in terminal" on shell code blocks, setup card "Open terminal".

Excluded: tabs, splits, panel surface, keys, sidebar indicator (`20261005-terminal-layout`); session transport,
drawer, font and theme (`20261005-terminal-drawer`); the emulator's own copy/paste keys and mouse selection (vendored in
`20261005-terminal-surface`); sign-in terminals (`20261005-sign-in-terminals`); the in-app Browser (`EXACT2-GAPS.md` X1).

## Context and guidance

Parent specification: [spec](../../spec.md). Source behavior (T3 Code `1e2ecbd975`):
`ThreadTerminalDrawer.tsx:239-297,552-730,784-829`; `terminalContext.ts`; `ChatComposer.tsx:6396-6420`;
`composerContextRecords.ts:122-193`; `composerContextReferences.ts:192-197`; `ChatMarkdown.tsx:596-610,1003-1011,1121-1133,3466-3470`;
`ChatView.tsx:3970-3978,4808-4964`; `WorktreeSetupCard.tsx:357,416`; `ProjectSetupScriptRunner.ts:357,380-446`;
`terminal-links.ts:207-224`; `openTerminalLinkInPreview.ts`; `LocalApi.contextMenu` in `packages/contracts/src/ipc.ts:1371-1394`.
Behavior to keep. A plain click on a link opens it; ⌘-click always uses the system browser; a path opens in the preferred editor
resolved against the terminal's folder; errors show as `[terminal] <message>` or the toast "Unable to open link". The popup waits for
the selection to settle (a multi-click waits 500 ms, `selectionActions.ts`). A script reuses the active terminal unless it runs a
subprocess, then it makes a new terminal (120×30), writes `command\r`, and remembers the script. "Run in terminal" shows only for closed
`sh bash zsh fish shell powershell pwsh` fences that end with a newline, have no control or invisible format characters, and do not end
with `\`; it needs a project. The setup terminal id is `setup-<scriptId>` (server chosen); the button shows once the setup stage has
started and only for the setup thread. The model sees selected lines as `N | text` (server side); the client sends the record. A chip
label is `Terminal 1 line 3` or `Terminal 1 lines 3-5`; empty text makes the chip expired and unsent, with a toast. The server caps
the record text at 64,000 chars and the desktop client does not check it: match the reference.
Library revision: `20261005-platforms-v3`. Selected topics: layout-and-interaction, accessibility, design (complete states),
testing-and-debugging. Menus, key monitors, hooks and WKWebView are unknown in the library; the clone's own code is the basis
(`T3ContextMenu.swift` for menus; `composer-editor.ts` `insertContext` and `composer-editor-menu.ts` `contextId`/`contextLink` for
context chips; `r6-polish-scripts.ts` for the script row).
Consumer framework revision: the `main` pin of `20261005-clone-on-exact2-main`.
Line numbers are from the mc-orch tree on 2026-10-05; `20261005-hot-file-split` moves code, so find it by symbol.
Tools are named by their `target/t3-ui-parity/…` path (committed under `examples/t3-code/tools/` with the same relative paths, decision U23): `electron-oracle.mjs`, `trace-proxy.mjs`,
`trace-diff.mjs`, `drive.mjs`, `lane-backend.sh`.

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| merged task PR | [20261005-clone-on-exact2-main](../20261005-clone-on-exact2-main.md) | pending | Merged | pending |
| merged task PR | [20261005-desktop-oracle-and-trace](../20261005-desktop-oracle-and-trace.md) | pending | Merged | pending |
| merged task PR | [20261005-terminal-layout](20261005-terminal-layout.md) | pending | Merged (terminal-layout builds on `20261005-terminal-drawer`) | pending |
| merged task PR | [20261005-hot-file-split](20261005-hot-file-split.md) | pending | Merged into `daehyeon/t3-code` (common prerequisite: room and per-area seams in the shared files) | pending |
| recorded decision | U20: script `autoOpenPreview` — the reference opens `previewUrl` in the in-app Browser (`ChatView.tsx:4909-4932`) | none | Default by the agreed rule: system browser. User confirms | pending |

## Issue assessment at preparation

Checked sources and time: plan issue drafts in [issues](../../issues/README.md), 2026-10-05; not reproduced, not searched upstream. No prior attempt.

| Issue / reference | Capability and target | Evidence / affected revision | Impact | Next action |
| --- | --- | --- | --- | --- |
| [X8](../../issues/closed/20261005-x08-agent-pointer-native-views.md) | Pointer input for native views | `EXACT2-GAPS.md` X8 | nonblocking (workaround: `(attended session)`) | Mouse selection, link clicks, right-click and the popup are attended; chip, store, menu-command and script paths run through the agent |
| [X26](../../issues/20261005-x26-app-menu-control.md) | Menu at the pointer / selection end | `EXACT2-GAPS.md` X26 | nonblocking (workaround: `T3ContextMenu.swift`) | Use the native menu at the reported position |
| [X20](../../issues/20261005-x20-rich-text-editing.md) | Rich-text composer with atomic inline nodes | `EXACT2-GAPS.md` X20 | nonblocking (workaround: native `NSTextView` composer) | Reuse the clone's context-chip insertion |
| [X9](../../issues/20261005-x09-root-component-across-files.md) | Resources in child components | Line cap | nonblocking until the cap | New state in `terminal-*.ts`, not `app.contract` |

## Implementation notes

- Port with tests (`bun:test`, original names): `terminalContext.ts` (5 tests: `formatTerminalContextLabel`, `formatTerminalContextReference`,
  `normalizeTerminalContextText`, `isTerminalContextExpired`, `migrateLegacyTerminalContextPlaceholders`); from `ThreadTerminalDrawer.test.ts`
  4 of 6 (`terminalSelectionMenuItems`, `terminalContextMenuItems`, `shouldClearTerminalSelectionAction`, `terminalSelectionLineRange`; the
  theme test is replaced in `20261005-terminal-drawer`); `resolveSelectionActionPosition` (done in `20261005-terminal-surface`);
  `buildExpiredTerminalContextToastCopy` (1 test "formats empty and omission guidance", `ChatView.logic.test.ts:342-353`); `openTerminalLinkInPreview` reduced to its system-browser
  branches (`openTerminalLinkInPreview.test.ts:99,209`; the other 4 tests are `deferred (X1)`, in-app Browser); `terminalContextRecord` and
  `terminalContextReference` (`composerContextRecords.ts:122-193`, with the terminal cases of `lib/composerContextRecords.test.ts`).
  Extract the "Run in terminal" condition into `canRunShellCommand(language, code, streaming, closedFence)` from `ChatMarkdown.tsx:1003-1011` and
  `isClosedCodeFence` (`:596-610`); translate "runs only a complete single-line shell block after a click" (`ChatMarkdown.test.tsx:182`) and add
  cases for the other conditions; record the extraction in the file header.
- Composer chip: follow the clone's context path (`insertContext`, `contextLink`): insert `[label](t3-context://v1/terminal/<id>)` at the caret,
  keep the record in the draft, send it with the message. Fix the sent chip (`r4-timeline-chips.ts` terminal case): label `Line N` or `Lines a–b`,
  popover `<pre aria-label="Captured terminal output">`, tooltip text as the reference.
- Menus use native menus at the pointer (right-click) or at the selection end (popup, `resolveSelectionActionPosition`). The context-menu items
  are Add to chat (omitted when the terminal has no chat target), Copy and Paste; Copy and Add to chat are disabled without a selection. Menu action
  runs in the same event turn: `selectionActionRequestId` rules of `ThreadTerminalDrawer.tsx:551-730` keep a superseded menu silent.
- Scripts: the script row exists and is disabled today (`r6-polish.contract`, `r6-polish-scripts.ts`). Enable it and run each script in the drawer through
  the `terminal.open` and `terminal.write` path that `20261005-terminal-drawer` provides. Remember the last script per project. A failed open or write sets the thread error
  `Failed to run script "<name>".` or the server message.
- States. Disabled: Run script with no project; "Run in terminal" hidden while streaming or for non-shell fences; Copy and Add to chat off without a
  selection. Loading: none. Empty: none. Error: `[terminal] <message>`; toast "Unable to open link"; thread error for a failed script run; expired-chip toasts from
  `buildExpiredTerminalContextToastCopy` ("Expired terminal context won't be sent" with "Remove it or re-add it to include terminal output.";
  "Expired terminal contexts omitted from message" with "Re-add it if you want that terminal output included."). Hover and keyboard
  focus: the code-block button, the setup card button and the chip behave like the clone's other icon buttons and chips; the chip's popover opens
  on focus. Menus: Escape closes the popup or the right-click menu without any action; arrow keys move; focus stays in the terminal. Permission:
  `terminal:operate` (TN2). Every icon button has an `aria-label` ("Run in terminal"). Motion: none; nothing animates.

## Acceptance and reproduction

| Criterion | Setup/reset and fixture | Action or command | Expected result | Required platform | Proof |
| --- | --- | --- | --- | --- | --- |
| Ported tests | — | `bun test examples/t3-code` | The named tests pass | macOS | log |
| Add to chat (store path) | Lane backend (`target/t3-ui-parity/lane-backend.sh`, isolated HOME, ports 16000–16999); selection injected through the TS entry point | Add; edit; send | Chip `Terminal 1 line 3`; the record in `message.send` equals the oracle's for the same selection (`target/t3-ui-parity/trace-diff.mjs`); an expired chip is not sent and shows the toast | macOS 1280×840 | trace |
| Sent chip | Message with a terminal record | Screenshot; focus the chip | Label and popover (`Line N`/`Lines a–b`, captured output) match the oracle at both sizes, light and dark | macOS | pairs |
| Menu items (command path) | Selection injected; menu opened through the menu command op | Check the item list and enabled states; Escape | Items equal the oracle's: popup (Add to chat, Copy), right-click (Add to chat, Copy, Paste), Copy and Add to chat off without a selection; Escape does nothing | macOS | `--json`, AppKit test |
| Mouse selection and menus — agent row (exact2 #186; adopt-main-fixes-r3): `tap terminal-view drag … mouse` (popup items in `state` `selectionMenu`), `tap terminal-view contextmenu at x y` (Add to chat, Copy, Paste); under the agent the popup is reported, not tracked (`T3TerminalActions.agentShown`), so choosing an item stays the AppKit `perform` tests; result pending the r3 drive. Was `(attended session)` | Lane build with `T3_LOCAL_HOME=<lane>/t3-home` and `T3_LOCAL_PORT=<lane port 16xxx>`, normal launch; terminal with output | Drag select; popup; Add to chat; right-click; Copy; Paste; Escape on each menu | Same menus, items and positions as the oracle; Paste writes a bracketed paste; Escape closes without action | macOS | notes, shots |
| Links — agent row (exact2 #186; adopt-main-fixes-r3): `tap terminal-view mouse at <link>` and `… modifiers Meta`; the page's `link` message (text, metaKey) is checked in `macos/tests/terminal/pointer.swift`; the live drive does not follow the link (it would open the user's browser or editor). Was `(attended session)` | Same lane build; output with an `https://` URL and `src/a.ts:10:5` | Click; ⌘-click | URL opens in the system browser; the path opens in the preferred editor at the position; both match the oracle's targets | macOS | notes |
| Link target logic | Unit tests | Plain click, ⌘-click, unsupported scheme | Decision equals the ported branches; non-http(s) is not opened | macOS | log |
| Scripts | Project with two scripts and a setup script | Agent: run script; run again while busy | First reuses `term-1`; busy makes a new terminal (120×30); frames `terminal.open` + `terminal.write cmd\r` equal the oracle's; the script's file effect exists | macOS | trace, effect |
| Run in terminal | Thread with closed `bash`, open, `python` and multi-line fences | Agent `tap` the button | Button only on a closed shell fence with a trailing newline and no control character; click writes `cmd\r` once; hidden while streaming | macOS | `--json`, trace |
| Open terminal | Worktree thread with a setup script | Create the worktree; click "Open terminal" | Button appears when the stage starts; the drawer opens `setup-<id>`; absent for another thread | macOS | `--json` |
| Failure paths | Fake transport fails open, then write | Run script | Thread error `Failed to run script "<name>".` or the message; no terminal left half-open | macOS | unit test |
| Standard gates | `git add -A` | Clone checks (bun test, strict tsc, contract build, `cargo test -p t3-code-macos --lib`, affected AppKit binaries), `bun scripts/caps.mjs`, the five repository checks | Green; every moved matrix cell is fixed, or declared in `EXACT2-GAPS.md` with an issue link | macOS | logs |

Task-owned source paths: `terminal-*.ts` and tests, `modules/apple/T3Terminal*.swift`, `modules/apple/T3ContextMenu.swift`, `composer-editor.ts`,
`composer-editor-menu.ts`, `r4-timeline-chips.ts` and test, `r6-polish*.ts`/`.contract`, `timeline-worktree.ts`, `markdown.contract`, `macos/src/markdown.rs`,
`macos/tests/terminal/**`.
Required environment: lane backend at the pin, oracle build, Xcode 27.0, Bun 1.4.2; lane builds set `T3_LOCAL_HOME=<lane>/t3-home` and
`T3_LOCAL_PORT=<lane port 16xxx>` (dev and lane builds refuse the real `~/.t3` and port 3773).

## Progress

Implemented in the active terminal parity worktree (2026-10-06). Selection records now insert at the native caret, persist with local drafts, deduplicate by terminal/range, and join message context. Missing/expired terminal references are omitted with the reference guidance; an expired-only send stays in the composer. Sent chips preserve the full inline label and show the terminal title, Line/Lines range and captured output in their popover. The terminal popover uses the existing native auto-popover top layer with position-area top: click-away/Escape dismissal and viewport clamping follow the host, without clipping inside the transcript. Source ContextChipPopover is a button PopoverTrigger, so keyboard activation opens it; focus alone does not (the earlier ticket prose was overbroad). Draft chip tooltips carry captured output and expired guidance.

Project-script controls are enabled and resolve project settings, remember the last choice, and use the drawer's open/write runner. Shell-fence buttons use the reference eligibility conditions plus per-block closed-fence state; identical code in a Python or unfinished fence cannot inherit a runnable shell block's button. Terminal path links resolve against the terminal folder and use the preferred editor. Browser links honor the original default/system and modifier branches; the selected in-app Browser branch remains explicitly unavailable (issue #100), so full original equivalence is not claimed.

Development checks: 37 focused Bun tests/106 assertions passed before the added two script integration tests; the complete integration file now passes 13 tests/60 assertions including real runner open/write request shapes, reuse, remember and failure paths. Strict TypeScript app compilation passed. Contract build and Rust fence test attempt were blocked by concurrently edited pages-welcome.contract syntax; rerun after integration. Follow-up: 29 integration/chip tests and 87 assertions pass, including explicit autoOpenPreview unmatched-dependency behavior. Rust retry reached capture but source changes during capture aborted it; rerun once source edits settle. Native menus are owned by the terminal surface agent; GUI selection, draft/sent chips, tooltips, script runs and links still require the integrated macOS sweep.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| none | — | — | — | — |

## Next action

Review the integrated evidence below and the remaining matrix subcases. Browser app-target remains a tracked parity blocker.

## Integrated evidence, 2026-10-07

The [expanded report](https://github.com/ccheever/exact2/blob/t3-code-evidence/evidence/terminal-drawer/20261007-expanded-parity/README.md) and [acceptance matrix](https://github.com/ccheever/exact2/blob/t3-code-evidence/evidence/terminal-drawer/20261007-expanded-parity/MATRIX.md) distinguish runtime observations from component tests and remaining gaps. [Final drawer motion](https://github.com/ccheever/exact2/blob/t3-code-evidence/evidence/terminal-drawer/20261007-expanded-parity/motion/final-drawer-observations.json) records actual macOS open/close, sampled reversal, reduced-motion endpoints and focus restoration. Publication URLs target the evidence branch.

C02–C05 runtime core now passes: actual three-line selection and AppKit Add to chat, exact clipboard, retained-caret insertion with one separator, atomic deletion, normal restart persistence, fake-provider send and draft/sent previews. D04 automatically displays setup completion and activates the existing PTY. See the linked matrix and its selection/setup reports. Remaining coverage includes C01 selection permutations, 64k GUI variants, C06 external editor display/caret (routing and process spawn pass), C07 in-app Browser mismatch, C08 external-app outcomes, D01 prefer-new/remembered-script GUI variants, and D02 real failed-RPC injection.
