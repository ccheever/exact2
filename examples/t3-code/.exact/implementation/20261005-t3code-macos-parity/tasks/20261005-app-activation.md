---
name: 20261005-app-activation
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
| merged task PR | [20261005-local-primary-environment](20261005-local-primary-environment.md) | pending | Merged | pending |
| recorded decision | U10 (hosted-web deep link kept or dropped; command-line install action) | none | Answered at `prepare` | pending |

## Issue assessment at preparation

Checked sources and time: planning pass 2026-10-05 against the local drafts in `../issues/` (unpublished, not reproduced); upstream not searched.

| Issue / reference | Capability and target | Evidence / affected revision | Impact | Next action |
| --- | --- | --- | --- | --- |
| [X5](../issues/20261005-x05-url-scheme-delivery.md) | URL scheme delivered to a module | needed only for the hosted-web deep link; #104 closed by main #201, which only journals a launch URL no navigation root hears (checked in [adopt-main-fixes-r4](20261007-adopt-main-fixes-r4.md)) | blocking if it is kept; none otherwise | U10 |
| [X6](../issues/20261005-x06-module-quit-shutdown.md) | Cleanup at quit | the socket file must go when the app quits | nonblocking (workaround: the next app instance re-binds; stale files are replaced) | main #200 (#105): `destroy()` now runs at ⌘Q, an Apple Event quit and last-window close, so remove the socket in the module's `destroy()` ([adopt-main-fixes-r4](20261007-adopt-main-fixes-r4.md)) |

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

Planned.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| none | — | — | — | — |

## Next action

`prepare` after the four merged task PRs; ask U10 first. Close with clone checks green (bun test, strict tsc, contract build, `cargo test -p t3-code-macos --lib`, the new AppKit binary), `bun scripts/caps.mjs` after `git add -A`, the repository's five checks, and every moved matrix cell fixed or declared in `EXACT2-GAPS.md` with an issue link.
