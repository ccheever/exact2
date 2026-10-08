---
name: 20261005-app-activation
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified-with-unverified-rows
delivery: draft
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-app-activation
pr_url: https://github.com/ccheever/exact2/pull/254
verified_commit: f31125732
---

# `t3 app <dir>` opens a project in the running app

## Outcome

Running `t3 app <dir>` (the server CLI's own command) in a terminal on this Mac brings the app to the front, adds
`<dir>` as a project in "This machine" when it is not there yet, and opens a new thread for it. The CLI prints
`Opened <dir> in T3 Code.` or one of the reference's error codes (`invalid-request`, `renderer-unavailable`, `environment-unavailable`, `platform-mismatch`, `project-create-failed`, `thread-open-failed`, `request-timeout`, `internal-error`).
The hosted-web deep-link handoff (`t3code://`) is only an open decision here.

## Scope and exclusions

Included (clone: missing; `app.json` has no activation, `modules/apple` has no socket server):

1. **Control socket (Swift, `T3AppControl.swift`).** Port `startDesktopAppControlServer` (`apps/desktop/src/app/DesktopAppActivation.ts:119-299`)
   and `resolveDesktopAppControlAddress` (`packages/shared/src/desktopAppControl.ts`): socket
   `<$TMPDIR>/t3code-<uid>/<first 12 bytes of sha256(stateDir) as 24 hex>.sock`, where `stateDir` is the resolved `<T3 home>/userdata`;
   directory 0700 owned by the user and not a symlink; bind a staging `<12 hex>.tmp`, chmod 0600, then `rename` onto the address
   (the newest app takes the path over; a close unlinks only if the inode is still its own); watch the directory and re-bind when the file
   vanishes, never replacing an existing socket; one JSON line per connection (limit 64 KiB, 5 s to send it, invalid JSON and
   invalid request answered with `invalid-request`); the answer is one JSON line; a client that disconnects before the answer cancels its request.
2. **Broker.** Port `DesktopAppActivationBroker` (`DesktopAppActivationBroker.ts`): requests wait until the renderer registers ready; one request is
   dispatched at a time, the rest queue; the window is activated on arrival; 15 s timeout `request-timeout` ("The desktop app did not finish opening the
   project in time."); a duplicate id is `invalid-request` ("The request id is already in use."); renderer gone while dispatched is `renderer-unavailable`
   ("The T3 Code window closed before it opened the project."); shutdown ("T3 Code is shutting down."); a cancelled client ("The command closed before T3 Code was ready.").
   Tests, as Swift tests with their names: from `DesktopAppActivation.test.ts` "roundtrips a request and removes its socket on shutdown", "cancels a queued request when the client disconnects", "keeps a
   newer app's socket when an older app on the same state dir quits", "binds its address again after the socket file is removed"; from `DesktopAppActivationBroker.test.ts` "focuses immediately and waits for renderer readiness",
   "fails an in-flight request when the renderer goes away", "queues requests after unsubscribe until a new renderer registers", "removes a queued request when its CLI connection closes", "never sends a canceled request that was
   queued behind another request", "times out a request without polling". From `desktopAppControl.test.ts` "keeps Unix socket paths short and separates desktop state directories" (and the named-pipe case, which stays out) as a bun test with the same vectors in Swift.
3. **Renderer side (TS).** Port `handleDesktopAppActivationRequest` (`apps/web/src/desktopAppActivation.ts`, tests "reuses an existing project and opens a new thread", "waits for a created project
   before it opens the thread", "rejects a Windows path when the primary environment is WSL" (kept as the platform-mismatch case), "returns a project error without opening a thread")
   and `findProjectByPath`, `inferProjectTitleFromPath` (`apps/web/src/lib/projectPaths.ts`, `projectPaths.test.ts`). Use the clone's existing project creation (`client.ts:943`, `createWorkspaceRootIfMissing:false`)
   and `client.openDraft` (`client.ts:1015`). The module reports ready only when the primary is connected, its config is known and the shell snapshot is loaded
   (`apps/web/src/components/desktop/DesktopAppActivationCoordinator.tsx:26-31`), and not ready again on reload or disconnect. Error texts: `environment-unavailable` "The desktop app's primary local environment is not connected.";
   `platform-mismatch` "The command path is for <os>, but the desktop app's primary environment uses <os>. Cross-platform path mapping is not supported."; `project-create-failed` "T3 Code could not add the project.";
   `thread-open-failed` "T3 Code could not open a new thread for the project.". The activation response carries `projectId` and `threadId`.
4. **Lifecycle.** Start the socket after the embedded server is started (reference order, `DesktopApp.ts:246-251`); a bind failure is logged and the app continues ("desktop app control socket unavailable").
   Not started when the local environment is off.
5. **The CLI.** Not shipped by this ticket. `t3 app` comes from the user's own `t3` (npm, install script) or the runtime folder's `t3`
   (`<T3 home>/runtime/versions/<version>/t3`, `20261005-embedded-server-runtime`). Open decision: add an "install command line tool" action (symlink to `~/.local/bin/t3` like `scripts/install.sh:219-221`); the
   reference desktop has none, so the default is no.

Accepted consequence (user decision, shared `~/.t3`): the clone and T3 Code (Nightly) compute the same socket path, so the later-started app takes `t3 app` over and the other can re-bind after it quits. Only one runs at a time.
Lanes use an isolated `T3_LOCAL_HOME`, so their path differs (`t3 app <dir> --base-dir <lane home>`).

Hosted-web deep link (`t3code://auth/codex`, plan decision U10): the handoff and `open-url` handling are registered by the Clerk bridge (`apps/desktop/src/app/DesktopClerk.ts:127-193`), which is excluded. The in-app Codex sign-in uses a loopback listener over IPC, not a URL scheme
(`20261005-managed-codex-chatgpt`). Default: not built. If the user keeps it: a new scheme name (never `t3code`), delivery blocked by [X5](../issues/20261005-x05-url-scheme-delivery.md).

## Context and guidance

Parent specification: [spec](../spec.md). Line numbers are from the mc-orch tree on 2026-10-05; `20261005-hot-file-split` moves code, so find it by symbol.
Tools are named by their `target/t3-ui-parity/…` path (committed under `examples/t3-code/tools/` with the same relative paths, decision U23): `trace-diff.mjs`, `electron-oracle.mjs`.
Every attended or normal-launch row runs a lane build with `T3_LOCAL_HOME=<lane>/t3-home` and `T3_LOCAL_PORT=<lane port 16xxx>` (dev and lane builds refuse the real `~/.t3` and port 3773, see `20261005-embedded-server-runtime`).
Reference at `1e2ecbd975`: `apps/desktop/src/app/DesktopAppActivation*.ts`, `apps/server/src/cli/app.ts:177-261` (the CLI: not run over SSH; tries the `userdata` hash, then `dev` when no base dir is given;
17 s response wait), `packages/contracts/src/desktopAppActivation.ts`. Library revision: `20261005-platforms-v3`. Selected topics: state-and-data, testing-and-debugging (record build identity; real CLI as the proof), capabilities. Unix sockets, window activation and
app-local Swift modules are **unknown in the library**; the clone's runtime evidence on the pinned main is the basis. Consumer framework revision: the pin from `20261005-clone-on-exact2-main`.
Edge cases to test: `$TMPDIR` differs between the CLI's shell and the app (the reference has the same limit; the CLI then reports it cannot reach the app); paths with `~`, spaces and symlinks; a directory that does not exist
(the server refuses: `project-create-failed`); two CLIs at once (serialized); the app quits during a request.

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| merged task PR | [20261005-hot-file-split](closed/20261005-hot-file-split.md) | pending | Merged into `daehyeon/t3-code` (common prerequisite: room and per-area seams in the shared files) | pending |
| merged task PR | [20261005-clone-on-exact2-main](20261005-clone-on-exact2-main.md) | pending | Merged | pending |
| merged task PR | [20261005-desktop-oracle-and-trace](20261005-desktop-oracle-and-trace.md) | pending | Merged | pending |
| merged task PR | [20261005-local-primary-environment](closed/20261005-local-primary-environment.md) | [#237](https://github.com/ccheever/exact2/pull/237) | Merged | merged into `feat(example)/t3-code` (`ed98ab3c0`); this PR's base |
| recorded decision | U10 (hosted-web deep link kept or dropped; command-line install action) | none | Answered at `prepare` | taken provisionally for this PR (coordinator brief 2026-10-08): the ticket's defaults, neither built; **user decision pending** |

## Issue assessment at preparation

Checked sources and time: planning pass 2026-10-05 against the local drafts in `../issues/` (unpublished, not reproduced); upstream not searched.

| Issue / reference | Capability and target | Evidence / affected revision | Impact | Next action |
| --- | --- | --- | --- | --- |
| [X5](../issues/20261005-x05-url-scheme-delivery.md) | URL scheme delivered to a module | needed only for the hosted-web deep link; #104 closed by main #201, which only journals a launch URL no navigation root hears (checked in [adopt-main-fixes-r4](closed/20261007-adopt-main-fixes-r4.md)) | blocking if it is kept; none otherwise | U10 |
| [X6](../issues/20261005-x06-module-quit-shutdown.md) | Cleanup at quit | the socket file must go when the app quits | nonblocking (workaround: the next app instance re-binds; stale files are replaced) | main #200 (#105): `destroy()` now runs at ⌘Q, an Apple Event quit and last-window close, so remove the socket in the module's `destroy()` ([adopt-main-fixes-r4](closed/20261007-adopt-main-fixes-r4.md)) |

## Implementation notes

- The server has no part in this: the socket and broker are Swift, the handler is TS. Use the clone's native-event pattern (`changed` topic, then a `later` read op) for requests, and one op to complete.
- Window activation: `NSApp.activate` and un-minimize the main window; no toast, as in the reference.
- Agent-only seam `T3_ACTIVATION_TIMEOUT_MS` (development flavor only) shortens the 15 s request timeout for the expired-request row.
- Do not use port 3773 or the `t3code` scheme anywhere in tests.

## Acceptance and reproduction

| Criterion | Setup/reset and fixture | Action or command | Expected result | Required platform | Proof |
| --- | --- | --- | --- | --- | --- |
| Ported tests | — | Swift: the activation and broker tests named in item 2; bun: `desktopAppControl`, `desktopAppActivation`, `projectPaths` | Original names pass; the sample socket path is shorter than the 104-byte macOS limit | macOS | logs |
| Real CLI round trip | Lane app (isolated `T3_LOCAL_HOME`), fixture provider, a temp git directory | `<runtime>/t3 app <dir> --base-dir <lane home>` (the pinned release's own CLI) | Exit 0, `Opened <dir> in T3 Code.`; project exists on the server; a new draft thread is open in the app | macOS 1280×840 | CLI output, `state`, server effect |
| Existing project | Same directory again | Run again | No second project; a new draft opens | macOS | state |
| Errors | App not running; local environment off; disconnected primary; nonexistent directory; renderer closed mid-request; an expired request (accelerated timeout seam) | Run the CLI | The CLI messages and codes of the reference (`Could not reach the T3 Code desktop app…`, `environment-unavailable`, `project-create-failed`, `renderer-unavailable`, `request-timeout`) | macOS | CLI output |
| Concurrency and cancel | Two CLIs at once; Ctrl-C one before the app is ready | Run | Serialized; the cancelled one never opens a thread | macOS | state, log |
| Socket hygiene | — | `stat` the directory and socket; remove the socket while running; quit the app | 0700 / 0600; re-bound within a second; file gone after a clean quit | macOS | `stat`, `ls` |
| Trace | T0-style scenario on oracle (`t3 app` against the reference desktop) and clone | `target/t3-ui-parity/trace-diff.mjs` | Same `project.create` command payload fields | macOS | ndjson |
| Look | Same directory on the oracle and the clone | Screenshot after the CLI returns | Same draft-thread route, light and dark, both sizes | macOS | png pairs |

Task-owned source paths: `modules/apple/T3AppControl.swift`, `T3Module.swift` (op prefix `activation`), `desktop-activation.ts`, `project-paths.ts` (+ tests), `macos/tests/app-control/`.
Required environment: the staged runtime (for the CLI), Xcode 27.0, pinned Bun, lane ports 16000-16999.

## Progress

Implemented on `feat(example)/t3-code-app-activation` (2026-10-08) from `feat(example)/t3-code` `da4f4512f`, with
`d82fb6a47` (#244, records only) merged in. Reference `1e2ecbd975`; the CLI is the pinned release's own `t3`
(`0.0.46-nightly.20261005.2667`, 4 commits after the pin). Decision U10 is taken provisionally, **user decision
pending**: the ticket's defaults, so no "install command line tool" action (the reference desktop has none) and no
hosted-web deep link (`t3code://` belongs to the excluded `t3-connect-sign-in`; a lane build registers no scheme).
Neither is built.

| Scope item | Built | Where |
| --- | --- | --- |
| 1. Control socket | `<$TMPDIR>/t3code-<uid>/<24 hex of sha256(<T3 home>/userdata)>.sock` (Node's `os.tmpdir()` rule; the home's `userdata` normalized as `path.join` does, symlinks kept); the directory made 0700, refused when it is a symlink or another user's; a staging `<12 hex>.tmp` bound, chmod 0600, then `rename`d onto the address; a directory watch binds the address again (`link`, never over an existing socket) when it vanishes; a close unlinks only its own inode (also from `atexit`, for the agent driver's `exit(0)`, without the watch binding it again); one JSON line per connection: 64 KiB, 5 s idle deadline, `invalid-request` for bad JSON, a bad request (with its id when it has one) and a too-large line; one JSON line back; a client that leaves before its answer cancels its request | `modules/apple/T3AppControl.swift` (`T3AppControlAddress`, `T3ActivationProtocol`, `T3AppControlServer`) |
| 2. Broker | `DesktopAppActivationBroker`: waits for a ready window, one request handed at a time, the rest queued in order; the window brought forward on arrival (`NSApp.activate`, un-minimize, key); 15 s `request-timeout` (`T3_ACTIVATION_TIMEOUT_MS` in a development build); duplicate id `invalid-request`; window gone `renderer-unavailable` (a module destroyed, its NSWindow starting to close, or a reloaded page's new token); shutdown; a cancelled client. Ten reference tests by name plus 15 of the clone's | `T3AppControl.swift` (`T3ActivationBroker`, `T3AppControl`), `macos/tests/app-control/` (25 tests) |
| 3. Window side | `handleDesktopAppActivationRequest` and `findProjectByPath`/`inferProjectTitleFromPath` (the whole `projectPaths` module, its 11 tests by name); ready only while the primary is connected with its config and shell snapshot (focused, or in the fleet), not ready on a disconnect or with the Local environment off; a handed request opens like a clicked notification (the shell view's open request), so the root's `shellOpen` task leaves Settings and the utility pages as the reference's route change does and sends `activation:open`; that op reads the request back (a cancelled or expired one opens nothing), finds or adds the project (`project.create`, `createWorkspaceRootIfMissing: false`, no toast on failure, `reportFailure: false`), waits up to 10 s for it in the shell, opens its draft with the draft's own thread id (`ensureDraftThreadId`), and answers with `projectId` and `threadId`. A primary in the fleet gets the project on its own transport, then becomes the focus on the new draft | `desktop-activation.ts`, `project-paths.ts` (+ tests), `client-ops.ts` (one entry), `shell.ts` (one line), `app.ts` (one line), `app.contract` (`shellOpenThread`, net 0 lines) |
| 4. Lifecycle | Listens once the embedded server's start is under way (its status names the T3 home), never while the Local environment is off or a build is refused; the stopgap switch (X45) closes and reopens it as the reference's relaunch would; a bind failure is logged (`desktop app control socket unavailable: …`) and the app goes on; closed with the last window | `T3AppControl.swift`, `T3Module+Activation.swift`, `T3Module.swift` (area entry, attach, `detachAppControl()` first in `destroy()`) |
| 5. The CLI | Not shipped (U10, provisional). `t3 app` is the user's own `t3` or `<T3 home>/runtime/versions/<version>/t3`; a lane build listens for its own home (`t3 app <dir> --base-dir "$T3_LOCAL_HOME"`, README) | — |

`app.contract` stays at 1500 lines (two comment lines became trailing comments); `client.ts` is unchanged.

### Acceptance results

All live rows ran on lane builds (`T3_LOCAL_HOME=target/lane/t3-home`, port 16901; isolated HOME/CODEX_HOME/
CLAUDE_CONFIG_DIR/XDG_*; `T3CODE_TELEMETRY_ENABLED=false`) with the pinned release's own `t3 app … --base-dir <lane
home>` (HOME a lane folder; never `~/.t3`, never 3773). The socket sat in the user's `$TMPDIR/t3code-501/` under the
lane home's hash (`55b411b92e964d4addd4e9d1.sock`); the Nightly app's socket there was never touched.

| Criterion | Result | Proof |
| --- | --- | --- |
| Ported tests | Pass. Swift: the 4 control-socket and 6 broker tests by name, the address test, and 14 more (25, 0 failures). Bun: `desktopAppActivation` (4), `desktopAppControl`'s Unix case, the shared macOS vector `073e69d4582c50be74c1c2d0` (also checked in Swift, 102-byte limit kept), `projectPaths` (11), 15 more for the window side (31 in all). Before: the modules do not exist on the base | [16-bun-tests.txt](https://github.com/ccheever/exact2/blob/63913cffb2c87d97acd3f436832a001a5b8e3926/app-activation/16-bun-tests.txt), [17-xctest-app-control.txt](https://github.com/ccheever/exact2/blob/5e5a71116dfdecdf6db56d869c77c12a0d400d3c/app-activation/17-xctest-app-control.txt) |
| Real CLI round trip | Pass (agent drive, 1280×840 light and 840×620 dark): `t3 app <lane>/projects/gamma` exit 0, `Opened <lane>/projects/gamma in T3 Code.` in 0.6 s; the lane database lists `gamma` beside the seed's `alpha`; the window shows "gamma / New thread", "What should we build in gamma?"; the app log has the response's project and thread ids. Base: `Could not reach the T3 Code desktop app…`, window unchanged | pairs `01`, `03`; [10-cli-1280-light.txt](https://github.com/ccheever/exact2/blob/9d3ff4cad8a6cba617ea7eb1480c0da0fe41abe8/app-activation/10-cli-1280-light.txt), [11-cli-840-dark.txt](https://github.com/ccheever/exact2/blob/a79756ef289c096c6f8e19a35e4cd5a5764b7554/app-activation/11-cli-840-dark.txt), [12-cli-base-1280-light.txt](https://github.com/ccheever/exact2/blob/1bec7f351b9e0243ec24f84b59be9fce5e346733/app-activation/12-cli-base-1280-light.txt) |
| Existing project | Pass: the same folder again, exit 0, no second project; the same empty draft opens with the same thread id (`7b3c6854…` both times, as `useNewThreadHandler` reuses an untouched draft) | 10-cli-1280-light.txt |
| Errors | App not running: `DesktopAppUnreachableError` "Could not reach…" (after a drive and after a quit). Local environment off (Settings › Connections switch, agent taps): no socket, "Could not reach…"; on again: listens, opens. Nonexistent folder: `project-create-failed` (server: "Failed to mutate project."). Expired request (5 s seam, server held with SIGSTOP): `request-timeout` after 5.4 s; the server, once resumed, still created the project, as in the reference. Window gone mid-request: `renderer-unavailable` "The T3 Code window closed before it opened the project." when the lane copy quits (Apple Event) while it handles a request held by a stopped server; the project was not created. Disconnected primary (server SIGKILLed): at the default 15 s the request was handed after the restart and opened (1.8 s); with the 5 s seam one run expired while reconnecting (queued, never handed). `environment-unavailable` needs the primary to drop between hand-off and handling: unit test only. `$TMPDIR` differing between the CLI and the app: "Could not reach…" (the reference's limit) | [13-lifecycle.txt](https://github.com/ccheever/exact2/blob/a7b42a268efdd770f82260799a71ada063b9404f/app-activation/13-lifecycle.txt), [14-primary-reconnect.txt](https://github.com/ccheever/exact2/blob/143489c2b05bde3cb4e4d7c2be16acfcd15fd908/app-activation/14-primary-reconnect.txt), [15-quit-during-request.txt](https://github.com/ccheever/exact2/blob/43077ab05d5c2d46becff8988739227f4ce331c7/app-activation/15-quit-during-request.txt), 10-cli-1280-light.txt |
| Concurrency and cancel | Pass: two CLIs at once, both exit 0, handled one after the other (log: the second "arrived" while the first was handed, "handed" only after the first answered). With the first held by a stopped server, the second was Ctrl-C'd (SIGINT, exit 130) while it waited behind it: "its command line closed the connection", answered `renderer-unavailable` "The command closed before T3 Code was ready.", never handed; its folder (`theta`) never became a project; the first opened once the server resumed. Ctrl-C while the window itself is not ready yet (at launch) was not driven: the broker test "removes a queued request when its CLI connection closes" covers that queue | 10-cli-1280-light.txt, 13-lifecycle.txt |
| Socket hygiene | Pass: `drwx------ $TMPDIR/t3code-501`, `srw------- …/55b411b92e964d4addd4e9d1.sock`; removed while running: bound again within 1 s (`stat` 1 s later) and a CLI request answered; gone after a clean quit (Apple Event) and after an agent drive's `exit(0)` | 10-cli-1280-light.txt, 15-quit-during-request.txt |
| Paths | `~/tilde proj` (the CLI expands `~`) and a folder with a space open; a symlink (`alpha-link` → `alpha`) is a separate project, since neither the CLI nor the server resolves it (the reference's logic, unchanged) | 10-cli-1280-light.txt |
| Window to the front | **Deferred to the real-input batch — screen locked (user away)**: with the screen locked `loginwindow` stays frontmost, so the activation (and un-minimizing) cannot be read back. Steps below | — |
| Trace | **Not run** — user decision 2026-10-06 (the oracle and trace tools are not built). Instead: the payload is a unit test (`project.create`, `createWorkspaceRootIfMissing: false`, `defaultModelSelection: null`, the title from the path, the focused connection's generation) and the server's effect is read from its database | 16-bun-tests.txt |
| Look | Oracle comparison **not run** — user decision 2026-10-06. Base vs branch at 1280×840 light and 840×620 dark: the new folder's draft, and leaving Settings for it | pairs `01`–`04` |

Before/after pairs (base `da4f4512f` vs this branch; same lane home seed, size and drive):

- ![new folder, 1280x840 light](https://raw.githubusercontent.com/ccheever/exact2/9d82ef5997ae46985569615dc00278f6a0dfc526/app-activation/01-new-folder-1280-light.png)
- ![from Settings, 1280x840 light](https://raw.githubusercontent.com/ccheever/exact2/b7059cbe56466c8d74d87287ecf32e3878994505/app-activation/02-from-settings-1280-light.png)
- ![new folder, 840x620 dark](https://raw.githubusercontent.com/ccheever/exact2/325f4c824bb54ee5d8abe84afc89a0d23387bfd0/app-activation/03-new-folder-840-dark.png)
- ![from Settings, 840x620 dark](https://raw.githubusercontent.com/ccheever/exact2/e1a71b438c165cde2ce602959bb730a7f750b4c0/app-activation/04-from-settings-840-dark.png)

Seen while driving: the agent driver waits for a window's in-flight answers before it runs its next operation, so
its `close` cannot close a window in the middle of a request (the request then expires); the real ⌘W case is in the
real-input batch. A lane copy launched with `CFFIXED_USER_HOME` still writes its preferences to the real
`~/Library/Preferences/<bundle id>.plist`; this task's copy (`com.exact.t3code.macos.laneactivation`) was deleted
after the run.

### Real-input batch steps

Deferred — screen locked (user away). One session, the real-input lock held (`target/t3-ui-parity/lanes/.realinput-lock`
in the base checkout, owner "app-activation: real input"):

1. Build: in this worktree, `export PATH="$HOME/.bun-1.4.2/bin:$PATH" EXACT_APP_DIR=$PWD/examples/t3-code; bun
   examples/t3-code/terminal-host/build.mjs; bun examples/t3-code/stage-runtime.mjs; bun host/apple/build.mjs
   t3-code-macos --bundle`. Copy the bundle to `target/lane/apps/T3 Code (Lane Activation).app`, set
   `CFBundleIdentifier` `com.exact.t3code.macos.laneactivation` and the name "T3 Code (Lane Activation)" in its
   Info.plist, `codesign --force --deep -s -`.
2. Lane env: `T3_LOCAL_HOME=<worktree>/target/lane/t3-home` (a copy of `t3-home-seed`), `T3_LOCAL_PORT=16901`,
   `T3_LOCAL_RUNTIME_DIR=<worktree>/target/lane/runtime` (the release archive extracted), `CFFIXED_USER_HOME` and
   `HOME` = `target/lane/home`, `CODEX_HOME`, `CLAUDE_CONFIG_DIR`, `XDG_*` under `target/lane`,
   `T3CODE_TELEMETRY_ENABLED=false`. Launch `…/Contents/MacOS/T3 Code (Exact)` with stderr to a file; record its pid.
   CLI: `env -i HOME=<lane>/clihome PATH=/usr/bin:/bin TMPDIR="$TMPDIR" <lane>/runtime/t3 app <dir> --base-dir <lane>/t3-home`.
3. Front: click another app's window (Finder) so the lane app is not frontmost; run the CLI on `<lane>/projects/gamma`.
   Read back: `lsappinfo info -only name $(lsappinfo front)` names "T3 Code (Lane Activation)", and
   `orca computer list-windows --app pid:<pid> --json` shows its window on screen; the window shows gamma's draft.
4. Un-minimize: `orca computer hotkey --app pid:<pid> --key CmdOrCtrl+M`; confirm it is minimized (list-windows);
   run the CLI on `<lane>/projects/delta`. Read back: the window is on screen again, frontmost, delta's draft shown.
5. ⌘W mid-request: `kill -STOP <server pid>` (the lane `runtime/t3 --bootstrap-fd` child of the app), run the CLI on
   a new folder (`<lane>/projects/iota`), after 2 s press `CmdOrCtrl+W` on the window. Expect the CLI to print
   `… (renderer-unavailable).` at once and the app log `the window is closing: its request fails`; then
   `kill -CONT <server pid>`. Read back the database: `iota` may exist (the reference also keeps a project the
   server created), but no thread was opened in a window.
6. Clean quit with ⌘Q (`CmdOrCtrl+Q`, through the quit hold): the socket file is gone (`ls $TMPDIR/t3code-501/`).
   Then `defaults delete com.exact.t3code.macos.laneactivation` and remove its plist.

2026-10-08 (real-input batch, records PR): front, ⌘W mid-request and ⌘Q pass; a minimized window is not restored and the request times out (clone bug). Results and proof: "Real-input batch (2026-10-08)" below.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| 1 | working tree on `da4f4512f` | First agent drive: the round trip worked; a socket file stayed after the drive because the directory watch bound the path again while `exit(0)` ran the server's stop. Fixed (exit marks the servers closed). Also `Math.random` is not available in data sources (bake refused it): the page token uses `crypto.getRandomValues` | lane logs (not committed) | — |
| 2 | working tree | Window closed under the agent while the server was stopped: the request expired instead of failing. `destroy()` now detaches the control first and the module watches its NSWindow's close; the agent's `close` still waits for the in-flight answer (driver behavior), so the ⌘W row moves to the real-input batch; the Apple Event quit passes | 13-lifecycle.txt, 15-quit-during-request.txt | screen locked (⌘W, front, un-minimize) |
| 3 | `f31125732` (+ merge `c5fab5dbf`) | Final: drives after-1280-light, after-840-dark, base-1280-light, base-840-dark, lifecycle, primary-reconnect, quit-during-request; checks below | PR evidence `app-activation/` | real-input batch; U10 |

Final checks on `c5fab5dbf`:
- `bun test examples/t3-code`: 2542 pass, 1 skip, 0 fail (206 files; 31 new). Strict `tsc` on `app.ts`: clean.
- `contract build`: 2629 slots, 46 resources, 2665 actions, 59420 nodes; `app.contract` 1500 lines, `client.ts` unchanged.
- `cargo test -p t3-code-macos --lib`: 11 passed. AppKit `macos/tests/app-control`: 25 tests, 0 failures (4 runs).
- caps: within cap. The five repository checks exit 0; `cargo test --lib --bins --tests --no-fail-fast`: 94 binaries,
  3383 passed, 0 failed, 33 ignored.
- Evidence: `t3-code-evidence` under `app-activation/`, linked from #254.

## Real-input batch (2026-10-08)

Run by the coordinator's real-input batch on an unlocked Mac (2026-10-08, 02:58-05:15 UTC), under the shared real-input lock (owner "real-input batch"), on the merged feature branch (`b7761f556`, rebuilt once at `07dcef1ab` for #263): one lane copy "T3 Code (Lane RIB)" launched normally (not agent mode) with isolated homes and lane ports 16450-16499. Real input: cliclick / CGEvent real mouse and wheel events, real HID key chords (posted only after a check that the lane app is frontmost), orca computer clicks and pastes. Records PR: draft "T3 Code clone: real-input batch for the tasks merged on 2026-10-08".

| Row | Result | Proof |
| --- | --- | --- |
| 3. Window to the front | PASS: Finder front → `t3 app …/gamma` exit 0 in 2 s → lane app frontmost, gamma draft | [act-01-front](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/app-activation/01-act-01-front.png) |
| 4. Un-minimize | FAIL (clone bug): with the window minimized (⌘M) `t3 app …/delta` times out (request-timeout, twice); the window stays minimized and the app does not come forward | [act-04-restored-crop](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/app-activation/02-act-04-restored-crop.png) |
| 5. ⌘W mid-request (server SIGSTOPped) | PASS: CLI "(renderer-unavailable)" at once; log "the window is closing: its request fails" | — |
| 6. ⌘Q | PASS: real ⌘Q hold → app exits, socket file removed | — |

Full record: [app-activation.txt](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/app-activation/app-activation.txt).

## Next action

Review of the draft PR; the user answers U10 (provisional: no CLI install action, no deep link). The coordinator runs
the "Real-input batch steps" above (front, un-minimize, ⌘W mid-request, ⌘Q) once the screen is unlocked.
