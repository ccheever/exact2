---
name: 20261005-local-primary-environment
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
| merged task PR | [20261005-clone-on-exact2-main](20261005-clone-on-exact2-main.md) | pending | Merged | pending |
| merged task PR | [20261005-desktop-oracle-and-trace](20261005-desktop-oracle-and-trace.md) | pending | Merged | pending |
| merged task PR | [20261005-embedded-server-runtime](closed/20261005-embedded-server-runtime.md) | pending | Merged; spike result is go | pending |
| scheduling preference | `20261005-environment-routes` first | none | Both touch `environmentKey` call sites | pending |
| merged task PR | [20261005-hot-file-split](closed/20261005-hot-file-split.md) | pending | Merged into `daehyeon/t3-code` (common prerequisite: room and per-area seams in the shared files) | pending |
| recorded decision | U4 decided (relaunch, as T3 Code); U5 to U7 and U13 (real-home smoke) | none | U5–U7 and U13 answered at `prepare` | U4: user 2026-10-05 |
| scheduling preference | After `20261005-remote-scopes-and-update-commands` | none | The Version row's desktop-managed sentence | pending |

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

Planned.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| none | — | — | — | — |

## Next action

`prepare` after the four merged task PRs. Close with clone checks green (bun test, strict tsc, contract build, `cargo test -p t3-code-macos --lib`, `fleet` and `transport` AppKit binaries), `bun scripts/caps.mjs` after `git add -A`, the repository's five checks, and every moved matrix cell fixed or declared in `EXACT2-GAPS.md` with an issue link.
