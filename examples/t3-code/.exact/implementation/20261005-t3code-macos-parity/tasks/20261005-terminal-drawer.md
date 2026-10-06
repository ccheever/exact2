---
name: 20261005-terminal-drawer
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

# Terminal drawer: one shell per thread on the server's terminal sessions

## Outcome

In a thread that has a project, ⌘J or the layout button opens a bottom drawer with a shell in the
thread's folder (or its worktree). The user types, sees output, drags the drawer height, closes
the terminal after a confirmation, and sees the exit handling. The terminal font and colors follow
Settings and the theme. Drawer state is saved per thread. A session on the server survives an app
relaunch and shows its history again. Terminal output never overflows the app's event inbox or
the 16-stream cap.

## Scope and exclusions

Included:

1. **Ported logic with tests** (table in Implementation notes). Change only what exact2 needs. Record each change in the file header.
2. **Session transport** in Swift (`T3Terminal*.swift`, `terminal-*.ts`): `terminal.open`,
   `terminal.attach` (stream), `terminal.write`, `terminal.resize`, `terminal.close`, and
   `subscribeTerminalMetadata`. Output bytes go from the socket to the output buffer to the web
   view. They do not pass the TS inbox. Each attach stream is acknowledged after the buffer holds the chunk.
3. **Output buffer**: 512 KiB, 16 KiB chunks, 1,024 chunks, UTF-8 safe trimming, generation and
   reset cursors (`terminalOutput.ts:45-47,201-281`).
4. **Drawer** in a new `terminal.contract`: top separator, single-terminal view, toolbar with
   Close, empty state, height state, persistence, panel animation, focus rules.
5. **Toggle** (⌘J, layout button) with `terminal.open` for `term-1`, cwd and env; the toggle's three
   places in the clone (`chat.contract:76-79`, `r4-surfaces.contract:111-114`, `shell-panels.contract:68-71`).
6. **Close, exit and error handling**; font, size and theme; thread delete cleanup (TN3) and
   the cwd/env helpers (TN4). They belong here: the toggle cannot open a shell without TN4, and a
   delete without TN3 leaves sessions behind.

Interim state until `20261005-terminal-layout` merges: the toolbar shows Close only. Split and New,
the tab list and the Terminal row in the right panel come with that ticket. Pixel pairs of the
toolbar crop out the missing buttons and say so.
Excluded: tabs, splits, panel surface, other keys, sidebar icon (`20261005-terminal-layout`); Add to chat,
menus, links, scripts (`20261005-terminal-integrations`).

## Context and guidance

Parent specification: [spec](../spec.md). Source behavior (T3 Code `1e2ecbd975`):
`apps/web/src/components/ThreadTerminalDrawer.tsx:93-105,299-307,432-448,1309-1377,1392-1420`;
`ChatView.tsx:938-1312,1219-1252,4640-4694,6570-6604,7537-7565`; `terminalUiStateStore.ts`;
`packages/client-runtime/src/state/{terminal,terminalSession,terminalOutput}.ts`;
`apps/server/src/terminal/{Manager.ts:96-105,2590-2630,3091-3170,OutputProtocol.ts:5-9}`;
`packages/contracts/src/terminal.ts` (cols 1–1000, rows 1–500, write ≤ 65,536 chars, env ≤ 128 keys);
`appearanceFonts.ts:35-51`; `index.css:1127-1130,1168-1169,1288-1291`.
Facts to keep: the web client calls only open, attach, write, resize, close and the metadata
stream. It never calls clear, restart or `subscribeTerminalEvents`. A shell exit prints
`[terminal] Process exited`, then the tab closes on the next tick without a confirmation. Attach also starts a
shell if none exists. The server stops a stream after 8 chunks or 64 KiB without an Ack. Write is
one RPC per input; the reference does not split a paste over 65,536 chars (it shows the error). The "5-minute TTL" and
"write FIFO" of the earlier draft have no reference source: do not build them.
Library revision: `20261005-platforms-v3`. Selected topics: state-and-data (await persistence on macOS),
layout-and-interaction (bounded layout, `pan` resize), performance (flood workload), accessibility
(labels, keyboard focus), testing-and-debugging. Native views, data-module networking and timers
are unknown in the library; the Swift transport (`T3Transport.swift`) is the clone's evidence.
Consumer framework revision: the `main` pin of `20261005-clone-on-exact2-main`.
Line numbers are from the mc-orch tree on 2026-10-05; `20261005-hot-file-split` moves code, so find it by symbol.
Tools are named by their `target/t3-ui-parity/…` path (committed under `examples/t3-code/tools/` with the same relative paths, decision U23): `electron-oracle.mjs`,
`trace-proxy.mjs`, `trace-diff.mjs`, `drive.mjs` (pointer drags), `lane-backend.sh`.

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| merged task PR | [20261005-clone-on-exact2-main](20261005-clone-on-exact2-main.md) | pending | Merged | pending |
| merged task PR | [20261005-desktop-oracle-and-trace](20261005-desktop-oracle-and-trace.md) | pending | Merged (oracle and trace) | pending |
| merged task PR | [20261005-terminal-surface](20261005-terminal-surface.md) | pending | Merged with a GO verdict | pending |
| merged task PR | [20261005-remote-scopes-and-update-commands](20261005-remote-scopes-and-update-commands.md) | pending | Remote pairing asks for the standard 5 scopes including `terminal:operate` (the clone asks for 3, `T3Transport.swift:310,365`) | pending |
| recorded decision | U12: sessions paired with three scopes before the scope fix (decision owned by `20261005-remote-scopes-and-update-commands`) | none | Answered at that ticket's `prepare`; default: no re-pair offer | pending |
| merged task PR | [20261005-hot-file-split](20261005-hot-file-split.md) | pending | Merged into `daehyeon/t3-code` (common prerequisite: room and per-area seams in the shared files) | pending |

## Issue assessment at preparation

Checked sources and time: plan issue drafts in [issues](../issues/README.md), 2026-10-05; not reproduced, not searched upstream. No prior attempt.

| Issue / reference | Capability and target | Evidence / affected revision | Impact | Next action |
| --- | --- | --- | --- | --- |
| [X21](../issues/20261005-x21-two-way-websocket.md) | Two-way WebSocket for data modules | LLP 1016.000 receive-only | nonblocking (workaround: Swift transport) | Add terminal streams to `T3Transport.swift` |
| [X8](../issues/20261005-x08-agent-pointer-native-views.md) | Pointer input for native views | `EXACT2-GAPS.md` X8 | nonblocking (workaround: `(attended session)`) | Drag on the separator is Contract, so the agent can drive it |
| [X25](../issues/20261005-x25-keyboard-keyup-code-capture.md) | Key facts for ⌘J while the web view has focus | Result of the spike S1 | nonblocking | Use the spike's path |
| [X9](../issues/20261005-x09-root-component-across-files.md) | Resources in child components | `app.contract` near 1,500 lines | nonblocking until the cap | New resources go to `terminal.contract` and TS modules; do not grow `client.ts` |
| Terminal-capable server | `node-pty` in the server runtime | The fixture runtime at the pin has it; whether the embedded runtime of `20261005-embedded-server-runtime` ships it is unknown | unknown for the embedded server | Checked in the plan's integrated acceptance row "Terminal"; this ticket has no edge to `20261005-local-primary-environment` |

## Implementation notes

| Reference module and names | Tests (`bun:test`, original names) | Clone file |
| --- | --- | --- |
| `client-runtime` `terminalOutput.ts`: `appendOutput`, `resetOutput`, `readTerminalOutputUpdate`, `terminalOutputText` | cases `terminalSession.test.ts:216-494` | Swift buffer; shared vectors in `macos/tests/terminal/vectors.json`, also read by a bun test |
| `terminalSession.ts`: `applyTerminalAttachStreamEvent`, `applyTerminalMetadataStreamEvent`, `combineTerminalSessionState`, `selectRunningSubprocessTerminalIds`, `nextTerminalAttachSeedState` | `terminalSession.test.ts` (20) | `terminal-session.ts` (status, error, label, subprocess flag only) |
| `state/terminalSessions.ts`: `selectKnownTerminalSessions` | `terminalSessions.test.ts` (6) | `terminal-sessions.ts` |
| `terminalUiStateStore.ts`: all actions, `selectThreadTerminalUiState`, `migratePersistedTerminalUiStateStoreState` | `terminalUiStateStore.test.ts` (14) | `terminal-ui-state.ts`; saved in `t3-code.json` per scoped thread key (replaces Zustand and `localStorage`) |
| `shared/terminalLabels.ts`: `getTerminalLabel`, `resolveTerminalSessionLabel`, `nextTerminalId` | `terminalLabels.test.ts` (8) | `terminal-labels.ts` |
| `lib/terminalCloseConfirm.ts`: `confirmTerminalClose`, `isTerminalCloseConfirmPending` | `terminalCloseConfirm.test.ts` (4) | `terminal-close.ts`; the clone's confirm dialog (`app.contract:1272-1289`) replaces `localApi.dialogs.confirm` |
| `ThreadTerminalDrawer.tsx`: `shouldHandleTerminalExit`, `writeTerminalOutputUpdate`, drawer height clamp (export `clampDrawerHeight` and `maxDrawerHeight`) | `ThreadTerminalDrawer.test.ts` "handles an exit that lands while the terminal surface is still loading"; new clamp tests | `terminal-drawer.ts` |
| `ChatView.logic.ts`: `reconcileMountedTerminalThreadIds`, `MAX_HIDDEN_MOUNTED_TERMINAL_THREADS` | 2 tests (`ChatView.logic.test.ts:509-540`) | `terminal-drawer.ts`; the cap may change after the spike S2 (record it) |
| `appearanceFonts.ts`: `resolveTerminalFontPreference`, `resolveTerminalFontSizePreference` | `describe("resolveTerminalFontPreference")` (2) and `describe("resolveTerminalFontSizePreference")` (2), `appearanceFonts.test.ts:73-109` | `terminal-drawer.ts` |
| `shared/projectScripts.ts`: `projectScriptCwd`, `projectScriptRuntimeEnv` | "builds default runtime env for scripts", "allows overriding runtime env values", "prefers the worktree path for script cwd resolution" (`apps/web/src/projectScripts.test.ts:133-173`) | `terminal-drawer.ts` |

- Theme: the reference reads CSS variables (`terminalThemeFromApp`, `ThreadTerminalDrawer.tsx:176-237`).
  The clone sends colors from its theme roles `terminalBackground`, `terminalForeground`,
  `terminalCursor`, `terminalSelection`, `terminalScrollbar(Hover)` (`settings-appearance-editor.ts:20-30,69-79`).
  Stock cursor and selection: light `rgb(38 56 78)`, `rgb(37 63 99 / 20%)`; dark `rgb(180 203 255)`, `rgb(180 203 255 / 25%)`.
  Update the web view on a theme or appearance change. Re-apply the font once the page is ready (a setting that loads late must not be lost, `ThreadTerminalDrawer.tsx:522-528`).
- Drawer: sibling below the chat canvas, so the composer stays above it. The reference box is
  `border-t border-border/80`, `bg-background`, 280 default, 180 minimum, `floor(75% of window height)` maximum
  and never below 180. The 6 pt separator has `cursor-row-resize` and `pan` (precedent `diff.contract:155`,
  `app.contract:984-988`: `pan` gives incremental deltas). Commit the height at `panrelease`, then refit.
  Re-clamp on window resize. Toolbar: 4 pt from the right and top, `border border-border/80`, `shadow-xs`,
  13 pt icons, 16 pt dividers, tooltips below with a 6 pt offset (`ThreadTerminalDrawer.tsx:1028-1048,1444-1488`).
- Cleanup: close with `deleteHistory:true`; on failure write `exit\n`; remove the tab at once and hide the
  id from stale metadata until it opens again (`terminalUiStateStore.ts:522-553`). On thread delete: close all sessions
  with history deletion, then `clearTerminalUiState` (`useThreadActions.ts:494-499,526`).
- Large paste: do not split it. The server error shows as `[terminal] <message>` (open decision: keep the reference behavior).
- Stream budget: count terminal attach streams apart from the 16-stream cap (`T3Transport.swift:529`).
  The reference worst case is 11 threads × 4 terminals. `T3Transport.swift` is shared with other tickets: expect merge conflicts.
- States. Loading: themed empty canvas. Empty: "No terminal sessions for this thread yet." with an `xs` outline
  "New Terminal" button. Error: attach error as `[terminal] <message>`; a started page that fails shows the page error text.
  Disabled: no project, so the toggle is disabled (tooltip "Terminal drawer is unavailable"). Hover: toolbar button `bg-accent`.
  Keyboard focus: toolbar buttons are in tab order with the clone's icon-button focus style. Permission (TN2): a missing
  `terminal:operate` scope shows the server's message as `[terminal] <message>`; the stream is not retried
  (`rpc/client.ts:322-330`). I found no re-pair offer in the reference terminal path. A session that has only the three old scopes gets the same server
  message and no re-pair offer, unless decision U12 (owned by `20261005-remote-scopes-and-update-commands`) chooses the one-time notice; then this ticket shows
  that notice and its action when a terminal request fails for the missing scope, and otherwise matches the reference.
  Close dialog: Escape and Cancel both leave the terminal open; keyboard focus starts on the clone's confirm dialog default control
  and returns to the terminal after a close (`bumpFocusRequestId`, `ChatView.tsx:1241-1242`). Icon buttons carry the tooltip text as
  `aria-label`; the toggle has `aria-label="Toggle terminal drawer"` and `aria-pressed`. Motion: height and opening animate only when
  `panelMs > 0` and reduced motion is off, and not on the first frame after navigation (`panelAnimations.ts:34-42`); otherwise the
  change is instant. The dialog has no motion.

## Acceptance and reproduction

| Criterion | Setup/reset and fixture | Action or command | Expected result | Required platform | Proof |
| --- | --- | --- | --- | --- | --- |
| Ported tests | — | `bun test examples/t3-code` | Named tests pass; vectors agree in bun and AppKit | macOS | logs |
| Open | Lane backend (isolated HOME etc., `SHELL=/bin/sh`, `PS1='$ '`, ports 16000–16999); project thread | Agent `press` ⌘J | Drawer 280 pt high; trace shows `terminal.open`, `terminal.attach`, one `terminal.resize` with cwd, worktree and env equal to the oracle trace | macOS 1280×840 | `--json`, trace diff |
| Type and output | Same | Agent `type` `touch drawer-proof` + Enter | File exists in the thread folder; `status` text shows the prompt; `terminal.write` frames equal the oracle's for the same keys | macOS | effect check, trace |
| Drawer pair | Same | Screenshot drawer open, empty state, one terminal | Pair with oracle at 1280×840 and 840×620, light and dark; toolbar crop per interim state | macOS | pairs |
| Resize | Same | Pointer drag on the separator (drive tool, as the settle sweep in `AGENT-HANDOFF.md:84,252`) by +120, −120 and to both extremes; relaunch | 280→400; min 180; max 630 at 840 pt window; saved after relaunch; `stty size` equals the grid | macOS | `layout`, `state` |
| Close and exit | Same | Close button; confirm text; Cancel; Escape; Confirm; type `exit`; fake a failing close | Text equals `Close terminal "Terminal 1"?` + "This stops the running process and clears its history."; Cancel and Escape keep the terminal; close sends `deleteHistory:true`; `exit` prints `[terminal] Process exited` and the drawer closes; failing close writes `exit\n` (fake transport test) | macOS | transcript, trace |
| Flood | `seq 1 200000` | Run in the drawer | No inbox overflow or resync in `logs`; Acks in trace; retained bytes ≤ 512 KiB; last line correct; UI answers (`perf`) | macOS | logs |
| Many threads | 3 threads with open drawers; unit test for 12 | Switch threads; relaunch | Each thread keeps its state; mounted count follows `reconcileMountedTerminalThreadIds`; history replays after relaunch | macOS | `state` |
| Settings | Advanced typography on | Change terminal font size and theme | Grid cols×rows change; colors change live; Simple mode follows the code font | macOS | `state`, shots |
| Authorization | Fake transport returns `EnvironmentAuthorizationError` (`message`, `requiredScope`) | Open | Message in the terminal; no retry loop | macOS | unit test |
| Three-scope session (terminal half of the check in `20261005-remote-scopes-and-update-commands`) | A saved environment whose session has only the three old scopes (the pre-change build's list, `T3Transport.swift:310,365`; fixture store on the lane backend) | Open the drawer (⌘J); `state`, `logs` | The terminal shows the server's message as `[terminal] <message>` (required scope `terminal:operate`); trace shows one `terminal.open`/`terminal.attach` refusal and no retry; the thread, composer and other operations still work; no re-pair offer unless U12 chose the notice, then the notice and its action appear | macOS 1280×840 | `--json`, trace, shot |
| Delete cleanup | Thread with a shell | Delete the thread | Trace shows `terminal.close` with `deleteHistory:true`, then delete; UI state removed | macOS | trace |
| Keys and feel `(attended session)` | Lane build with `T3_LOCAL_HOME=<lane>/t3-home` and `T3_LOCAL_PORT=<lane port 16xxx>`, normal launch | ⌘J while the terminal has focus; Korean typing; live window resize | Drawer toggles; text correct; grid refits | macOS | notes |
| Standard gates | `git add -A` | Clone checks, `bun scripts/caps.mjs`, five repository checks | Green; every moved matrix cell is fixed, or declared in `EXACT2-GAPS.md` with an issue link | macOS | logs |

Task-owned source paths: `terminal.contract`, `terminal-*.ts` and tests, `modules/apple/T3Terminal*.swift`,
`modules/apple/T3Transport.swift` (stream accounting), `macos/tests/terminal/**`, `app-main.contract`, `chat.contract`,
`r4-surfaces.contract`, `shell-panels.contract`, `app.json`, `AGENT-HANDOFF.md`.
The embedded-server run (open, type, close with the official runtime) is checked in the plan's integrated acceptance row "Terminal".
Required environment: lane backend at the pin, oracle build, Xcode 27.0, Bun 1.4.2; lane builds set `T3_LOCAL_HOME=<lane>/t3-home` and `T3_LOCAL_PORT=<lane port 16xxx>` (dev and lane builds refuse the real `~/.t3` and port 3773).

## Progress

2026-10-06: implemented on `feat(example)/t3-code-terminal-drawer` (base `9670b0723`).

- Ported with original test names: `terminalOutput` (TS twin + Swift `T3TerminalOutput`, shared
  `macos/tests/terminal/vectors.json`, 11 vectors read by bun and AppKit), `terminalSession` (21),
  `terminalSessions` (6), `terminalUiStateStore` (14), `terminalLabels` (8), `terminalCloseConfirm` (4),
  drawer helpers (exit rule, mounted threads 2 + a 12-thread case, fonts 4, project scripts 3, new clamp tests).
- Sessions are native (`T3TerminalSessions.swift`): `terminal.attach` is a side stream of the transport
  (`T3Transport+Terminal.swift`): not the inbox, not the 16-stream cap (44 at most); each chunk is
  acknowledged after the 512 KiB buffer holds it; a lost socket ends the streams and they attach again.
  `terminal.write` (one RPC per input) and `terminal.resize` (latest-wins) go native. The view writes
  `[terminal] <message>` lines and reports `Process exited` once; the tab then closes without a confirm.
- Drawer (`terminal.contract`): below the chat column, 6 pt `pan` separator (commit at release), Close
  toolbar button with the app confirm dialog (`Close terminal "Terminal 1"?` / "This stops the running
  process and clears its history."), empty state with New Terminal, height animation when `panelMs > 0`
  and motion is not reduced. Toggle: ⌘J (MAIN_ROWS), chat header, right panel header, launcher bar.
- Client (`terminal-drawer-view.ts`): toggle/new/close/exited/height ops, `terminal.open` with cwd,
  worktree and the project script env, close with `deleteHistory:true` then `exit\n` on failure,
  `subscribeTerminalMetadata` (labels, reconcile, suppressed closed ids; an authorization failure is
  not retried), thread delete closes the thread's terminals before `thread.delete`, state saved per
  thread under `terminal` in `t3-code.json`. Mounted threads (active + 10 hidden) keep their sessions
  natively without a web view (`terminalRetain`); a view replays the buffer when its thread returns.
- Decision U19: the paste is not split; the server's refusal shows as `[terminal] <message>`.
- Focus on close (ChatView.tsx:7545-7565, the `terminalOpen` effect): whenever the drawer closes the reference
  focuses the composer on the next frame, and opening it focuses the terminal. The clone now focuses the
  composer on every close path: ⌘J and the layout toggles, the confirmed Close, and the shell's exit
  (`app.contract` `command` and `confirmUi`). Checked by the Contract build and the Bun suite; not driven (drive limit).

Not done / limits: split, tabs, New in the toolbar (20261005-terminal-layout); drafts without a server
thread have no drawer (the clone's drafts carry no thread id); a background (fleet) environment's
thread has no drawer (sessions use the focused transport); the chat canvas layout still measures the
window height, not the height above the drawer; under the agent the terminal takes the system scheme, so it can be dark in a light
window (spike note). Oracle and trace-diff rows: not run. Attended rows: unverified (attended).

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| 1 (implementation) | `0f66bbb0e` (drive build), checks at `c7e240c80` | `bun test examples/t3-code` 1921 pass / 1 skip / 0 fail (base 1829); strict tsc clean; `contract build` 2324 slots, 44 resources; `cargo test -p t3-code-macos --lib` 10/0; AppKit: terminal 13 pass + 1 skip, transport 49/0, composer 45/0, menus 43/0, r5-panels 8/0, every other binary pass (mermaid needs a server, timeline-keyboard separate recipe: not run); caps pass; five checks pass (build, test 2928 pass, clippy, fmt, caps, boot); macOS bundle builds | Live drive (one AFTER call after two runs that stopped in the welcome wizard on the recipe): paired with the lane server (port 16200, `SHELL=/bin/sh`), opened seeded thread "Drawer one", `toggle-terminal` → drawer 280 pt, `touch drawer-proof` + `ls` (file created in the thread folder; screen `drawer-proof README.md`), separator drag −120 → 400, `seq 1 200000` ran and the drawer kept answering, Close → confirm text as above, Cancel kept it, Confirm closed it (`metadata` 0 sessions), reopen + `exit` → `[terminal] Process exited` and the drawer closed. BEFORE drive on the base shows the disabled toggle | The drive exposed "the reply was not JSON" (an ArraySlice in the terminal status) — fixed in `9d4fd4deb` with a test, not re-driven (drive limit). Flood: retained bytes and Ack counts were not visible to the agent (native status is not in `state`); covered by the transport and vector tests. Relaunch replay, 3-thread switching, Settings font/theme, three-scope session: not driven (unit tests only). Attended: ⌘J with the terminal focused, Korean typing, live window resize |
| 2 (⌘J focus) | after `da0cdf31f` | Matches the reference: composer focus on every drawer close. `contract build` 2328 slots, 45 resources; `bun test examples/t3-code` 1959 pass / 1 skip / 0 fail | Reference read: `ChatView.tsx:7545-7565` | Not driven (one-drive rule); attended row "⌘J closes and the composer has focus" |

## Repair verification, 2026-10-06

The user requested terminal verification, repair of discovered bugs, and PR evidence. The previous
Electron comparison is preserved at PR #175 comment 6017408751 and on the `t3-code-evidence`
branch, commit `a80581a8dc5ac3995da6abf8af1723fb187ec0da`.

Two defects were reproduced and repaired:

- Delayed WebKit/WASM readiness reclaimed focus after the user typed in the composer. Focus is
  now requested when the terminal mounts; readiness completes it only if the terminal still owns
  the native first responder. AppKit tests exercise initial focus, composer focus during loading,
  and a later explicit focus request.
- A failed resize was recorded as applied, so a disconnected resize remained stale after
  reconnection. The session now keeps the desired grid separately from the successful grid and
  reapplies it on attach. Loopback WebSocket tests cover disconnected resize, failure retry,
  successful deduplication, and coalescing of pending sizes.

Guidance remains `20261005-platforms-v3`: accessibility, state/data, testing/debugging. These are
app-native repairs; no framework or dependency changes. Repair verification passed: terminal XCTest 16 executed / 1 skipped / 0 failures; transport
XCTest 51 executed / 2 skipped / 0 failures; Bun 2115 passed / 1 skipped; Cargo 2928 passed /
18 ignored; build, Clippy, fmt, caps and boot passed. Both repair source identity and root-check
source identity remained unchanged. Independent review found no blocking repair findings.
Actual macOS capture confirms initial focus, composer focus retention, shell filesystem effect,
280→400 resize with settled 21×129 grid, ⌘J focus return, Cancel retention and confirmed close.
Evidence: `t3-code-evidence` branch, `evidence/terminal-drawer/20261006-repairs/README.md`.

The previous live focus recipe was confounded by an asynchronous open. A new actual WKWebView
regression compiled with the original view fails at the expected composer-focus assertion;
the repaired view passes. The corrected live sequence checks drawer-open before typing.
Full parity acceptance remains open for the previously unverified normal-launch persistence,
physical Korean IME, three-scope authorization, and visual/motion rows.

## Next action

Review the repairs and evidence on PR #175. Keep the overall ticket active until the remaining
acceptance rows have direct evidence.
