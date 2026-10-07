---
name: 20261005-local-primary-environment
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified-with-unverified-rows
delivery: draft
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-local-primary-environment
pr_url: https://github.com/ccheever/exact2/pull/237
verified_commit: 8997645e807a2d1d3b55bf868bff647b3e5adb87
---

# "This machine" is a real primary environment backed by the embedded server

## Outcome

At launch the app starts its embedded server and connects to it as the primary environment, with the
reference's wording and states: a "This machine" section in Settings > Connections with a Local environment
switch and a Version row, a first-run decision that works with a primary, the "Local environment turned off"
home, the "Restoring your threads…" toast, and running threads kept live in every enabled environment. The
loopback stand-ins that pretend a saved environment is the primary are gone. A user with an existing shared
`~/.t3` sees their projects and threads.

## Scope and exclusions

Included (clone rows missing or partial; evidence `connections.ts:1-8`, `r11-misc-connections.ts:1-22`, `r12-sidebar-connections.ts:1-16`):

1. **Primary model.** A `primary` source built from `localBackendStatus` (`20261005-embedded-server-runtime`): id `primary`, label from the descriptor
   (`Local environment` is the bootstrap label on macOS, `DesktopBackendConfiguration.ts:934-940`), HTTP/WS base URLs, in-memory bearer. Port the rules of
   `resolveDesktopPrimaryTarget` / `readPrimaryEnvironmentTarget` (`apps/web/src/environments/primary/target.ts:253-283,309`): no bases means none, one base is
   an error, disabled means none. It connects through `T3Fleet`/`T3Transport` with a memory credential instead of Keychain, and joins `environmentSources`
   (`connections.ts:131-147`). It has no row under Environments (`r12-sidebar-connections.ts` header states the reference rule). Status maps to the connection phases the rows already use: `installing` and
   `starting` are "Connecting", `restarting` is "Reconnecting: <last exit reason>", `failed` is "Connection failed: <reason>", `ready` plus a connected socket is "Connected"; `refused` (a development build without
   `T3_LOCAL_HOME` and `T3_LOCAL_PORT`, see `20261005-embedded-server-runtime`) draws no primary and shows the status text in the "This machine" section only.
2. **Remove the stand-ins.** `isLoopback` as "primary" (`settings-b-fleet.ts:30-33`), `balanceSources`, `environmentRows`, the composer's
   `localEnvironment: isLoopback(client.origin)` (`composer-editor.ts:215`, `T3Composer.swift:221`): use `isPrimary`. Ports change shape every launch, so
   never remember a primary origin: `T3Transport` init already has `remembersOrigin` (`T3Transport.swift:66`); store a focus token `primary` beside
   `t3.server.origin` (`:219,829`). A saved entry that has the primary's `environmentId` (same `~/.t3`) is a duplicate: decision below.
3. **Session.** The local session has all eight scopes: desktop treats itself as `AuthAdministrativeScopes` (`ConnectionsSettings.tsx:1912-1916`), and
   `canManageLocalBackend` = local on and `access:write`.
4. **"This machine" section** (`ConnectionsSettings.tsx:3424-3530`, `LocalEnvironmentSetting.tsx`): header with machine icon, title (descriptor label, else
   "This machine"), `aria-label` "More actions for this machine" menu (the existing environment icon menu), the Local environment row, the Version row
   (`serverVersion · displayUrl`, "Loading…", "Up to date", `ServerUpdateProgress`; a mismatch shows "Update the desktop app on that machine to update this
   server." because `serverSelfUpdate` is `desktop-managed` and `desktopAppUpdate` is false, `ServerUpdateAction.tsx:262-267`; the sentence itself comes from `20261005-remote-scopes-and-update-commands`, and without it this row shows the version only). Rows for Network access,
   endpoints and clients are `20261005-this-machine-network-access`. Enable the gated catalog entries (`settings-catalog.ts:100-107`, flags `desktop`,
   `localBackend`, `localEnvironment`; `settings-search.ts:~65`).
5. **Local environment switch.** Row description on: "Run agents on this computer. Turn off to use T3 Code only with remote environments."; off: "Turned off.
   Agents only run in remote environments." Confirm dialog: "Turn off local environment?" / "T3 Code will restart without running a server on this computer. Any
   agents and terminals running here will stop, and other devices will no longer be able to connect to this computer. Your projects, history, and remote
   environments are unaffected." with destructive "Restart and turn off"; "Turn on local environment?" / "T3 Code will restart and start running a server on this
   computer again." with "Restart and turn on"; busy "Restarting…" with spinner, Cancel disabled, inline error text and the switch re-enabled on failure.
   Persist `localEnvironmentEnabled` (default true) in `t3-code.json` (versioned). Off state (`apps/web/src/routes/_chat.index.tsx:114-125`): title "Connect to a
   computer running T3 Code", the two paragraphs, "The local environment is turned off. Connect a remote environment, or turn the local environment back on in
   Connections.", button "Open Connections"; settings search hides local-only items; dropping a folder reports "Folders can't be dropped into remote environments"
   (`components/chat/folderDrop.ts`, only if the clone has folder drop).
6. **First run.** The clone decides with the hosted rule only (`hostedDecision`, `pages-welcome.ts:31,146`). Port `onboarding/firstRun.logic.ts`
   (`resolveFirstRunDecision`, `isFreshFirstRunWorkspace`, `isFirstRunWorkspaceProvenanceAuthoritative`, `resolveHostedFirstRunDecision`) with its tests
   (about 40, e.g. "opens the wizard for an authoritative fresh workspace", "does not complete onboarding from cached remote projects"); the primary is
   preselected in the wizard's computer list (`WelcomeWizard.tsx:147`). This needs the server `welcome` event, so subscribe to `subscribeServerLifecycle` next to
   `subscribeServerConfig` (`client.ts:360`) from a new file through the per-area seam of `20261005-hot-file-split` (`client.ts` has 45 lines of room without it).
7. **Fatal start.** Port `handleFatalStartupError` (`DesktopApp.ts:109-141`): alert "T3 Code failed to start", "Stage: <stage>" and the message, then quit. Port
   exhaustion is fatal; a crashing server is not (it backs off and the primary shows "Reconnecting: <reason>").
8. **Running threads stay live.** Port `createRunningThreadKeepAliveAtom` (`apps/web/src/state/threads.ts:60-135`): every running thread of every enabled
   environment keeps its detail stream; a stopped one stays until its own detail shows the stop; environments that connect or leave are followed
   (`state/threads.test.ts:117,136,162`). The clone subscribes thread detail only for the focused thread (`client.ts:451`) and config + shell for background ones
   (`settings-b-fleet.ts:135-137`).
9. **Legacy thread migration toast (CN5).** `ServerLifecycleLegacyThreadMigrationPayload {status: running|complete, totalThreadCount}` (`packages/contracts/src/server.ts:854`) on the
   primary only: while `running`, a loading toast (no timeout) "Restoring your threads…" / "Migrating N thread(s) from the previous version. You can keep working while
   this finishes." (`N` via `toLocaleString`), closed when it leaves `running`; the toast kind `loading` already exists (`toast.ts:6`).

Excluded: T3 Connect, WSL, the activation socket, update of the app, window-state persistence (done).

## Context and guidance

Parent specification: [spec](../spec.md). Reference at `1e2ecbd975`. Line numbers are from the mc-orch tree on 2026-10-05; `20261005-hot-file-split` moves code, so find it by symbol.
Tools are named by their `target/t3-ui-parity/…` path (committed under `examples/t3-code/tools/` with the same relative paths, decision U23): `trace-proxy.mjs`, `trace-diff.mjs`, `electron-oracle.mjs`.
Every attended or normal-launch row runs a lane build with `T3_LOCAL_HOME=<lane>/t3-home` and `T3_LOCAL_PORT=<lane port 16xxx>` (dev and lane builds refuse the real `~/.t3` and port 3773, see `20261005-embedded-server-runtime`);
the one exception is the real-home smoke row (U13). Library revision: `20261005-platforms-v3`. Selected topics: state-and-data (snapshot semantics; late old
replies; persistence awaited), layout-and-interaction (settings structure), design and accessibility (every state; dialogs with a focus target; icon button labels),
testing-and-debugging. Native fleet/transport, Keychain and app lifecycle are **unknown in the library**.
Decision U4 (2026-10-05): the same as T3 Code — after a Local environment change the whole app relaunches (`ipc/methods/localEnvironment.ts:20-29`). Put the change behind one function `applyLocalSetting` that relaunches the app. If issue X45 confirms that exact2 cannot relaunch an app, `applyLocalSetting` ships a stopgap (stop or start the embedded server and reconnect) and the relaunch rows stay blocked until X45 is resolved and adopted. Open decisions (plan decisions U5 to U7, U13): (b) The window:
Electron opens it only after the backend is ready (`DesktopApp.ts:237-257`, `DesktopWindow.ts:874`); the Exact host creates it first. Show the existing connecting state and
declare the difference, or ask for a framework hook ([X31](../issues/20261005-x31-deferred-window-readiness.md); U5). (c) A saved entry that equals the primary: remove it silently and forget its credential
(recommended), or keep a duplicate row (U6). (d) Whether to read `~/.t3/userdata/desktop-settings.json` (the original's file) for the Local environment and exposure
settings; default is the clone's own `t3-code.json`, so a change made in the original does not carry over (U7).
`app.contract` (1327 lines) and `client.ts` (1455) are near the 1,500-line cap: put state in TS and a new `.contract` file; do not grow either.

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| merged task PR | [20261005-clone-on-exact2-main](20261005-clone-on-exact2-main.md) | [#99](https://github.com/ccheever/exact2/pull/99) | Merged | the clone is on exact2 in `feat(example)/t3-code` (this PR's base); #99 to main is the user's end-of-project step |
| merged task PR | [20261005-desktop-oracle-and-trace](20261005-desktop-oracle-and-trace.md) | none | Merged | blocked: not built (user decision 2026-10-06); the Trace row and the oracle comparisons stay blocked |
| merged task PR | [20261005-embedded-server-runtime](closed/20261005-embedded-server-runtime.md) | [#222](https://github.com/ccheever/exact2/pull/222) | Merged; spike result is go | merged; spike go |
| scheduling preference | `20261005-environment-routes` first | [#148](https://github.com/ccheever/exact2/pull/148) | Both touch `environmentKey` call sites | merged first |
| merged task PR | [20261005-hot-file-split](closed/20261005-hot-file-split.md) | [#147](https://github.com/ccheever/exact2/pull/147) | Merged into `daehyeon/t3-code` (common prerequisite: room and per-area seams in the shared files) | merged |
| recorded decision | U4 decided (relaunch, as T3 Code); U5 to U7 and U13 (real-home smoke) | none | U5–U7 and U13 answered at `prepare` | U4: user 2026-10-05; U5, U6, U7, U13 taken provisionally for this PR (coordinator brief 2026-10-07), user decision pending |
| scheduling preference | After `20261005-remote-scopes-and-update-commands` | [#142](https://github.com/ccheever/exact2/pull/142) | The Version row's desktop-managed sentence | merged first |

## Issue assessment at preparation

Checked sources and time: planning pass 2026-10-05 against the local drafts in `../issues/` (unpublished, not reproduced); upstream not searched.

| Issue / reference | Capability and target | Evidence / affected revision | Impact | Next action |
| --- | --- | --- | --- | --- |
| [X21](../issues/20261005-x21-two-way-websocket.md) | Native WebSocket transport carries the primary | existing | nonblocking | none |
| [X9](../issues/20261005-x09-root-component-across-files.md) | Resources in child components | line caps | nonblocking until the cap | keep state out of `app.contract` |
| [X31](../issues/20261005-x31-deferred-window-readiness.md) | Defer the first window until the server is ready | Reference opens the window after readiness; the clone does not control host window creation (not in the library) | unknown (workaround: connecting state in the first window, which differs from the reference's no-window-until-ready) | Check on the pin at `prepare`; the user decides per U5 |
| [X45](../issues/20261005-x45-app-relaunch.md) | App relaunch after an exposure change | X45 (unconfirmed) | blocking for the relaunch rows if X45 is confirmed missing (stopgap meanwhile: restart the server in place and reconnect; a visible difference) | use the relaunch when X45 is adopted Update 2026-10-07 (adopt-main-fixes-shell): #122 was closed after main #170, which only moves `reload()`'s log to stderr; exact2 still has no process relaunch, so the relaunch rows stay blocked. |

## Implementation notes

- Smallest working order: primary source and session, remove stand-ins, first-run logic, switch and off state, keep-alive, toast.
- Local-setting handler is one seam shared with `20261005-this-machine-network-access`.
- The server's own provider checks run in the embedded server; the app adds nothing there.

## Acceptance and reproduction

| Criterion | Setup/reset and fixture | Action or command | Expected result | Required platform | Proof |
| --- | --- | --- | --- | --- | --- |
| Ported tests | — | `bun test`: `firstRun.logic.test.ts` cases, `state/threads.test.ts` keep-alive cases, primary-target cases | Original names pass | macOS | log |
| Fresh start | Empty isolated `T3_LOCAL_HOME`, lane port, fixture provider | Launch; `agent macos --json tree state logs` | Server starts, primary connected, wizard shown with "This machine" preselected, no refused operation | macOS 1280×840 and 840×620, light and dark | transcript, png |
| Work end to end | Same | Add a project, start a thread, stream a reply, answer an approval, open the diff | Works; effect: lane home has the project and thread; git effects read back | macOS | state, files |
| Trace | T0 on the reference oracle and the clone | `target/t3-ui-parity/trace-diff.mjs T0` | Same connection sequence; client label "T3 Code Desktop"; only allow-listed differences | macOS | ndjson |
| Switch off and on | Running primary | Switch, dialog, confirm | Dialog text and states as above; server PID gone, then new PID; off home text; relaunch keeps the setting | macOS | state, `ps`, png pairs vs oracle |
| No stale origin | Two launches on different ports | Quit, relaunch | Primary reconnects; `t3.server.origin` not rewritten | macOS | defaults read |
| Keep-alive | Two fixture backends, a running thread on the second | Open it from the first | Live state at once, no replay flash | macOS | state timeline |
| Migration toast (CN5) | Lifecycle events injected through `target/t3-ui-parity/trace-proxy.mjs` (running, complete); live legacy data unverified | Watch toasts | Appears with the count, closes on complete | macOS | png, state |
| Crash while open | Kill the server (recorded PID) | Watch the primary | "Reconnecting: …" then connected; no alert | macOS | state |
| Dialog keyboard and focus | Running primary | Open "Turn off local environment?" and "Turn on local environment?"; Tab and Shift-Tab; Enter on Cancel; Escape | Initial focus, Tab order and default button equal the oracle's (record them from the oracle with `tree --ax` first); Escape closes and returns focus to the Local environment switch; Enter on Cancel closes without a restart; visible focus ring on each button | macOS 1280×840 and 840×620, light and dark | `tree --ax` diff, png pairs |
| "Restarting…" state | Fake server that ignores SIGTERM for 2 s (AppKit-level double, lane port) | Confirm Turn off | Button reads "Restarting…" with a spinner, Cancel is disabled, Escape and an outside click do nothing, the switch is disabled; on success the dialog closes; on a forced failure the red inline error appears under the description, Cancel and the switch work again | macOS | state timeline, screenshots |
| Restart-failure and fatal-start dialogs | AppKit test with a recording alert closure; forced start failure | Trigger a start failure and, in the packaged-flavor test, port exhaustion | Inline error text for the first; alert "T3 Code failed to start" with "Stage: <stage>" and the message, then quit, for the second; Return and Escape both dismiss the alert | macOS | AppKit log |
| Reduced motion | `prefer prefers-reduced-motion reduce` | Open and close the dialogs; watch the spinner | No scale or fade (compare frames from `screenshot … over 300 every 30` with the oracle's value, recorded first); the spinner keeps turning because it is not spatial | macOS | frame contact sheets |
| Own `~/.t3` smoke (attended session) | The user's real home, T3 Code (Nightly) quit, a backup taken by the user, explicit go (U13); a development build with `T3_LOCAL_ALLOW_REAL=1` and a port in 16000-16999 (the one row that does not use a lane home) | Launch | Existing projects and threads appear; nothing is written outside the runtime folder and server data | macOS | screenshots, user note |
| No WSL on macOS (issue X41) | Lane build of the clone; the reference on this Mac with a lane home and port | Search settings for "WSL" and "Windows Subsystem"; open Settings → Connections | No result and no WSL row in either app; "This machine" has no WSL row | macOS 1280×840 and 840×620, light and dark | `tree`, png pairs |
| Relaunch after a setting change (U4) | Running primary, lane build | Turn the Local environment off and on through the dialog | The app quits and a new app process starts (new pid, as the oracle does), the server starts with the new setting, and the setting persists; blocked on issue X45 if exact2 cannot relaunch (the stopgap then keeps the window and restarts only the server) | macOS | `ps` before/after, recording beside the oracle |

Task-owned source paths: `local-primary.ts`, `first-run.ts` (+ tests), `connections.ts`, `settings-b-fleet.ts`, `pages-welcome.ts`, `r11-misc-connections.ts`, `r12-sidebar-connections.ts`, `settings-catalog.ts`, `settings-search.ts`, a new `.contract` for the section and dialog, `modules/apple/{T3Fleet,T3Transport}.swift`, `t3-code.json` schema.
Required environment: the staged runtime, pinned Bun, Xcode 27.0, lane ports 16000-16999.

## Progress

Implemented on `feat(example)/t3-code-local-primary-environment` (2026-10-07) from `feat(example)/t3-code` `38352ceaf`,
with `fbce02624` (#231, records only) merged in. Reference `1e2ecbd975`. Decisions taken for this PR, each
**provisional, user decision pending**: U5 (the first window shows the connecting state until the primary connects,
until exact2 #117 / X31), U6 (a saved duplicate of the primary is removed silently and its credential forgotten),
U7 (the switch lives in the clone's own `t3-code.json`, top-level `localEnvironmentEnabled`), U13 (the real-`~/.t3`
smoke is not run without the user's explicit go). U4 (relaunch) is decided; exact2 has no process relaunch
(#122 closed by main #170 without one, X45), so `applyLocalSetting` ships the stopgap.

| Scope item | Built | Where |
| --- | --- | --- |
| 1. Primary model | `primary` source from `localBackendStatus` (id `primary`, the descriptor's label, HTTP/WS bases, `environmentId`); `readPrimaryTarget` ports `readPrimaryEnvironmentTarget` (no bases: none; one base: `DesktopEnvironmentBootstrapIncompleteError`; disabled: none); phases: installing/starting "Connecting", restarting "Reconnecting" with "The local server exited (<exit>).", failed/runtime-missing an error, ready follows the socket; refused draws no primary. The transport connects it with `primary: true` and the in-memory bearer from `T3LocalBackend` (never saved, origin never remembered, focus token `t3.server.focus = primary`); the fleet carries it when another environment is focused. | `local-primary.ts`, `r8-pointer-reconnect.ts`, `settings-b-fleet.ts`, `connections.ts`, `T3Transport.swift`, `T3LocalBackend.swift` |
| 2. Stand-ins removed | `isLoopback` is gone: launch choice, fleet, Auto balance (`balanceSources`), sidebar environment rows, composer folder drop (`folderDropTarget` ported), Open in Finder, remote open, server-update notices, shell details, git env and the favicon use the primary. | `r11-misc-connections.ts`, `r12-sidebar-connections.ts`, `composer-editor.ts`, `palette-add.ts`, `remote-open.ts`, `server-update-notices.ts`, `shell-details.ts`, `r4-git-env.ts`, `desktop-shell-favicon.ts` |
| 3. Session scopes | The primary's session holds the eight `AuthAdministrativeScopes`; `canManageLocalBackend` = on, not refused, `access:write`. | `local-primary.ts` (`sessionScopes`) |
| 4. "This machine" section | Machine icon, the descriptor label, "More actions for this machine" menu, Local environment row, Version row (`serverVersion · URL`, update progress and button, "Up to date"), a Local server status row; replaces "Administrative access". Settings search shows `desktop`, `localBackend` (manageable) and `localEnvironment` (on) rows and never WSL/Windows rows. | `this-machine.ts`, `connections.contract` (`ThisMachineSection`), `settings-search.ts` |
| 5. Switch, dialogs, off state | Reference copy for both dialogs; "Restarting…" with spinner, buttons disabled, Escape ignored, the section held still while it runs; inline red error on failure. Cancel takes the focus, Tab stays on the two buttons, Escape is Cancel's and returns the focus to the switch; 200 ms fade and 98 % scale each way, spinner still under reduced motion (the reference's `dialog-styles.ts` and Spinner `motion-safe:`). Off: the home's "The local environment is turned off…" with "Open Connections". `applyLocalSetting` stopgap: off hands the focus to a saved environment and stops the server (SIGTERM, SIGKILL after 2 s, at most 5 s); on starts it and connects when nothing else is focused. | `this-machine.contract`, `this-machine.ts`, `app-settings.contract`, `pages-home.ts`, `pages-hero.contract`, `T3LocalBackend.swift` (`setEnabled`) |
| 6. First run | `firstRun.logic.ts` ported with its tests (`first-run.ts`, 43 tests); `primaryDecision` uses `resolveFirstRunDecision` with the primary's `welcome` lifecycle event (`subscribeServerLifecycle`), the hosted rule stays for no primary; the wizard lists the primary first and preselected. | `first-run.ts`, `local-lifecycle.ts`, `pages-welcome.ts` |
| 7. Fatal start | "T3 Code failed to start" / "Stage: <stage>" and the message, once, then quit; Return and Escape dismiss; port exhaustion is fatal, a crashing server backs off. | `T3LocalFatal.swift`, `T3LocalBackend.swift` |
| 8. Keep-alive | `createRunningThreadKeepAliveAtom` ported as `runningThreadKeepAlive` (3 reference tests by name); each environment keeps a running thread's detail on its own transport (limit 6): its bounded snapshot over HTTP, then `keep:<thread>` after its sequence, as the reference's thread state; opening a kept thread starts from its detail with no refetch, also across a fleet focus change (a reconnect no longer synchronizes twice, `client.ts` drain). | `keep-alive.ts`, `settings-b-fleet.ts`, `client.ts` (via `local-environment.ts`) |
| 9. CN5 toast | Loading toast "Restoring your threads…" / "Migrating N threads from the previous version. You can keep working while this finishes." (`toLocaleString`), no timeout, closed when the migration leaves `running`; primary only. | `local-lifecycle.ts` |

`app.contract` (1500 lines) and `client.ts` (882 lines) keep their base lengths; `client.ts` reaches this task's
hooks through `local-environment.ts`.

### Acceptance results

| Criterion | Result | Proof |
| --- | --- | --- |
| Ported tests | Pass with the original names: `firstRun.logic.test.ts` (43), `createRunningThreadKeepAliveAtom` (3), `environmentBootstrap` / `folderDropTarget` cases | `bun test` |
| Fresh start | Pass: lane home and port 16801, the server starts, the wizard lists "Daehyeon's MacBook Pro http://127.0.0.1:16801/ Connected" first and preselected, no refused operation; 1280×840 and 840×620, light and dark | drive A1, pair `01-first-run.png` |
| Work end to end | Pass with a real Claude login (the provider lane's, reused by path; hold lifted 2026-10-07): project "work" added on the primary, a Supervised thread on Claude streamed "I'll read greet.py and add the farewell function.", a File change approval for `greet.py` was approved, the reply finished ("1 changed file"), and Open diff showed `greet.py` +4. Read back: the lane home's database holds the project (`…/projects/work`) and the thread (`claudeAgent`, `approval-required`); `git diff` in the project shows `farewell` (+4) | session P, pair `09-work-end-to-end.png` |
| Trace (`trace-diff T0`) | **Blocked**: user decision 2026-10-06, the oracle/trace tools are not built | — |
| Switch off and on | Pass: dialog copy and states; server pid 24931 gone at off, new pid 28991 at on (agent); 87904 → off → 3279 (real input, retry session 9587 → off → 12937); off home text. "Relaunch keeps the setting": see the relaunch row | drives A2, real input, pairs `02`–`05` |
| No stale origin | Pass: two launches of the lane copy on ports 16805 and 16806; the primary connected on each; its defaults domain held only `t3.server.focus = primary`, no `t3.server.origin` | real input, `defaults read` |
| Keep-alive | Pass, after two fixes this row found. Two environments (the primary and a second lane server on 16812, each with Claude): a Claude turn running on the second while the primary is focused; its sidebar row shows Working; opening it shows its live state at 100 ms and the stream continues to the end. The second server's log: the bounded snapshot is fetched once while it is in the background (the keep-alive) and not on opening. First run: the kept stream started from sequence 0, never went live, so opening refetched (fixed `3cc1ac4d9`); second run: a reconnect's spurious inbox reset resynchronized and refetched once (fixed `8997645e8`); third run: no refetch. A thread Claude leaves "Waiting" (a background monitor) is not kept, as the reference's `isRunning` | sessions P, Q, R; pair `10-keep-alive.png`; `keep-alive.test.ts`, `client.test.ts` |
| Migration toast (CN5) | Pass with a lane stub server (lifecycle `running` then `complete`; replaces trace-proxy, user decision 2026-10-06): "Restoring your threads…" / "Migrating 1,234 threads…", closed on complete; no toast on the base. Live legacy data unverified | drive B, pair `06-migration-toast.png` |
| Crash while open | Pass: kill -9 of the recorded server pid 78791, restart pid 81646 after 0.6 s, the primary "Reconnecting…" then "Connected", no alert | drive C |
| Dialog keyboard and focus | Pass under real input (`orca computer`), after a fix this pass found: Cancel takes the focus on open (the first pass left it on the switch), Tab and Shift+Tab alternate Cancel and the confirm, Escape closes and returns the focus to the switch (server pid unchanged), Enter on Cancel closes without a restart, Space on the switch opens it, the ring shows on Cancel; both dialogs. Checked against the reference's source (Base UI AlertDialog), not the oracle (**blocked**: oracle not built, user decision 2026-10-06). At 840×620 (agent driver, light and dark): Cancel focused on open, Tab → confirm → Cancel, Shift+Tab → confirm, Escape and Enter on Cancel close it and focus the switch, server pid unchanged | real input, session P; pairs `08-dialog-keyboard.png`, `11-dialog-840.png` |
| "Restarting…" state | Pass: lane server holding SIGTERM 1.8 s; "Restarting…" with a turning spinner (two real-input frames differ only in the spinner), both buttons disabled, Escape ignored, the switch disabled; forced start failure shows the red inline error, retry turns it on | drive B3, real input, pair `04-restarting.png` |
| Restart-failure and fatal-start dialogs | Pass: `LocalSwitchTests` (7) — failed start answers why, port exhaustion is fatal with the stage then quits, the alert shows once, Return and Escape dismiss | AppKit `local-backend` |
| Reduced motion | Pass against the reference's source: its dialog has no reduced-motion variant (`dialog-styles.ts`), so the 200 ms fade and scale still run (frames B09a–c); the Spinner is `motion-safe:`, so it stays still (B10a/b). This corrects the ticket's expected result. The oracle frame comparison is **blocked** (oracle not built, user decision 2026-10-06) | drive B3 |
| Own `~/.t3` smoke | **Not run**: U13 needs the user's explicit go | — |
| No WSL on macOS | Pass in the clone: no WSL or Windows result, no WSL row. The reference app side is **blocked** with the oracle (user decision 2026-10-06); its source shows `wsl` only on Windows | drive A2 (`A06-search-wsl`) |
| Relaunch after a setting change (U4) | **Blocked** on exact2 #122 / X45: no process relaunch; the stopgap keeps the window and restarts only the server | — |

Seen while driving, not this task's: under real input, the Add Environment form in the re-signed lane copy stopped at
"The server credential could not be saved in Keychain." (an ad hoc re-signed copy with a new bundle id; macOS's
SecurityAgent prompt was left unanswered). The pairing itself reached the server, and no "native.watch outside an
answer" error appeared on this branch (asked by the round-5 adoption, PR #236). In sessions Q and R, adding the second
environment from Settings › Connections › Add environment connected with no error either.

Provider setup for the live rows (lane, not committed): the provider lane's Claude login is reused by path, never
copied. Each lane T3 home's `userdata/settings.json` gives the `claudeAgent` instance a `binaryPath` (a lane symlink
to `~/.local/bin/claude`) and an instance environment of `HOME` (the user's) and `CLAUDE_CONFIG_DIR` (the provider
lane's `att/claude`); the lane servers keep their own HOME, XDG_* and T3 home. The second server also needs `USER`
(the CLI's Keychain lookup) and runs outside the tool sandbox; no logout was run.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| 1 | `c872d6f3c`, `3a79e9877`, `3d743f09d` | Built scope items 1–9; agent drives A1, A2, B, B3, C on lane ports 16801–16804 with isolated homes; `~/.t3` mtimes unchanged, nothing on 3773 | lane evidence (not committed); PR pairs `01`–`07` | provider hold (end to end, keep-alive live; lifted 2026-10-07, attempt 4); oracle not built (trace, oracle comparisons); #117 / X31; #122 / X45; U13 |
| 2 | `8f2240e75` | Real-input pass (one drive) found the dialog's initial focus on the switch and Tab leaving the modal; fixed (the switch moves the focus to Cancel; the buttons keep Tab), retried once: pass | PR pair `08-dialog-keyboard.png` | same |
| 3 | `73495bba0` + records | `client.ts` back to its base 882 lines through `local-environment.ts` | PR body | same |
| 4 | `3cc1ac4d9`, `8997645e8` | Provider hold lifted (2026-10-07). Live session P (agent driver, lock held): work end to end on Claude, the dialog at 840×620, keep-alive found the kept stream never going live; fixed. Session Q: the handoff applied but a spurious reset refetched; fixed. Session R: no refetch. Final checks below | pairs `09`–`11` | oracle not built; #117 / X31; #122 / X45; U13 |

Final checks on `8997645e8`:
- `bun test examples/t3-code`: 2386 tests in 196 files; 2385 pass, 1 skip, 0 fail.
- Strict `tsc`: clean.
- `contract build`: 2545 slots, 45 resources, 2617 actions, 59013 nodes.
- `cargo test -p t3-code-macos --lib`: 11 passed.
- The app bundle builds and bakes.
- AppKit binaries (README recipe, run on `73495bba0`; no Swift changed since), 31 plus `timeline-keyboard`, all 0 failures:
  - transport 53 (`PrimaryTransportTests` 4, new), local-backend 51 (`LocalSwitchTests` 7, new), fleet 9.
  - `mermaid` 10 ok against a lane server.
  - `snapshot` 86 checks with the real HOME. Under a lane HOME, its keyboard-layout check reads the input source from HOME and fails; this task does not touch that code.
- caps: within cap. `app.contract` is 1500 lines and `client.ts` 882, both the base's lengths.
- The five repository checks all exit 0. `cargo test --lib --bins --tests --no-fail-fast`: 94 test binaries; 3348 passed, 0 failed, 33 ignored.
- Evidence pairs are on `t3-code-evidence` under `local-primary-environment/` and are linked from #237.

## Next action

Review of the draft PR; the user answers U5, U6, U7 and U13. Then: the real-`~/.t3` smoke on U13's go; the oracle
rows if the oracle is built; the relaunch rows when exact2 has a process relaunch (#122 / X45); the window gate when
#117 / X31 lands.
