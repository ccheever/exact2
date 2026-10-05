---
name: 20261005-embedded-server-runtime
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

# Embedded T3 server: fetch and verify at build, unpack at first launch, run and supervise

## Outcome

The example builds without the reference checkout. A build step downloads the official T3 server
release archive, verifies it, and packs it into the `.app`. At first launch the app unpacks it,
starts the server with the desktop envelope, waits until it answers, exchanges the bootstrap token for a
bearer in memory, restarts it with the reference's backoff when it dies, keeps the reference's failure log,
and stops it at quit. A crash of the app does not leave a second server behind. No user interface here: the
state is exposed to TypeScript for `20261005-local-primary-environment`.

## Scope and exclusions

Included:

1. **Pin.** Committed `runtime-pin.json`: release version and tag, source commit, asset name
   `t3-<version>-darwin-arm64.tar.gz`, size, SHA-256, URL. Chosen at `prepare`: the release built from
   `1e2ecbd975` or later whose server version is not older than `CLIENT_VERSION` (`connections.ts:24`),
   or the embedded "This machine" would offer an update to itself.
2. **Stage step** `stage-runtime.mjs` (an `app.json` command; apparatus needing approval). Download the
   archive and `SHA256SUMS` from `pingdotgg/t3code` Releases (`packages/shared/src/cliRelease.ts:57-80`,
   `parseChecksums`); accept only if the `SHA256SUMS` line, the committed hash and the computed hash agree.
   `SHA256SUMS` is unsigned, so the committed hash is the trust anchor. Extract to scratch, run
   `codesign --verify --deep --strict` on `t3` and every `.node` (the release signs them: Developer ID or
   ad hoc, `docs/operations/release.md:43`), run `t3 --version` with an empty `PATH` and compare to the pin,
   start it once on an isolated home and a lane port and probe `/.well-known/t3/environment` (the idea of
   `scripts/smoke-cli-archive.ts:93-186`). Write `runtime-manifest.json` (path, mode, size, SHA-256, link target),
   then split the tree into parts with the tool the spike picks (Apple Archive `aa` or `tar`; it must keep
   modes and symlinks) as `assets/t3-runtime.NNN`, each below the per-asset limit measured on the pin
   (`EXACT2-GAPS.md` X4: a 256 MiB bake buffer, names `[A-Za-z0-9._-]`, mode 0o644; it plans parts of ≤ 60 MiB; any per-asset limit is to confirm, see issue X4). Output is
   git-ignored. Nothing reads the reference checkout.
3. **Install (Swift, `T3LocalRuntime.swift`).** Port the layout of `apps/server/src/cloud/pinnedRuntime.ts:24-70`
   (`pinnedRuntimePaths`, `.install-complete` holding the version) and the staging-then-rename of
   `scripts/install.sh:177-215`: `<T3 home>/runtime/versions/<version>/`. Verify each part's SHA-256 before
   expanding; restore modes; check `t3` is 0755 and the manifest hashes after; keep a lock so two launches do
   not unpack together; remove `.staging-*` left by an interrupted run. Never write inside the app bundle.
   States: `runtime-missing` (dev build without staging), `installing {verify|extract, fraction}`, `failed {reason}`.
4. **Start (Swift, `T3LocalBackend.swift`).**
   - Port selection: `resolveDesktopBackendPort` (`apps/desktop/src/app/DesktopApp.ts:34-36,73-107`): a configured
     port wins, else scan upward; each port must bind on `127.0.0.1`, `0.0.0.0` and `::`; typed error text kept.
     The scan start is one constant: 3773 in the packaged build only, as in the reference (one app at a time on the
     shared `~/.t3`); development and lane builds never scan: they require `T3_LOCAL_PORT` in 16000-16999 (item 8).
   - Environment: port `DesktopShellEnvironment.installPosixEnvironment` (`shell/DesktopShellEnvironment.ts:70-94,159,319,397-460`):
     `$SHELL -ilc` marker probe (5 s), `/bin/zsh` fallback, `launchctl getenv PATH` (2 s) only when no PATH,
     PATH merged and de-duplicated, `SSH_AUTH_SOCK`/`HOMEBREW_*`/`XDG_*` filled when missing, `LC_CTYPE=en_US.UTF-8`
     only when `LANG`, `LC_ALL`, `LC_CTYPE` are all unset. Remove the ten names in `DESKTOP_BACKEND_ENV_NAMES`
     (`backend/DesktopBackendConfiguration.ts:79-90,133`); other `T3CODE_*` variables pass through.
     Telemetry is excluded (spec; issue X39): the child gets `T3CODE_TELEMETRY_ENABLED=false`.
     Without it the pinned server sends PostHog events by default and, with the shared `~/.t3`,
     reuses the original app's anonymous id (`AnalyticsService.ts:69-80`, `Identify.ts:257-266`).
   - Envelope: `DesktopBackendBootstrap` (`packages/contracts/src/desktopBootstrap.ts:5-21`): `mode:"desktop"`,
     `noBrowser:true`, `port`, `t3Home`, `host`, `desktopBootstrapToken`, `tailscaleServeEnabled:false`,
     `tailscaleServePort:443`; no telemetry fds or OTLP; `resourceMonitorPath` only if the tree has
     `resource-monitor/t3-resource-monitor`. Run `<versionDir>/t3 --bootstrap-fd 0` with the JSON line on stdin
     (the WSL path does the same, `DesktopBackendConfiguration.ts:802-815`; macOS desktop uses fd 3, which Foundation
     `Process` cannot pass), written at once and the pipe closed (the server waits 1 s, `apps/server/src/bootstrap.ts`).
     Token: 24 random bytes as hex, once per app run. `cwd` is the home directory. No envelope or token in argv or env.
   - Ready: `GET /.well-known/t3/environment` returns 2xx; every 100 ms, 1 s per probe, 60 s rounds repeated while the
     child lives (`backend/DesktopBackendManager.ts:57-69,579-596`); a failed round persists a snapshot.
   - Auth: `POST /oauth/token` (token-exchange grant, `subject_token_type` `urn:t3:params:oauth:token-type:environment-bootstrap`),
     no `scope` (so all eight scopes), `client_label` "T3 Code Desktop", `client_device_type` desktop
     (`backend/DesktopLocalEnvironmentAuth.ts:49-92`). Refactor the clone's exchange (`T3Transport.swift:307-330`) to take
     an optional scope. Bearer in memory only; exchange again after each server (re)start (the seed grant lasts 24 h and
     replaces the previous desktop session, `apps/server/src/auth/PairingGrantStore.ts:240-325`, `EnvironmentAuth.ts:826`).
5. **Supervise.** Restart delay `min(500 ms * 2^n, 10 s)`, `n` back to 0 on ready (`Manager.ts:347,937`); a stop cancels a
   pending restart; a start during stop restarts after teardown. Output goes to a per-run buffer (1 MiB, 256 chunks,
   `appendBoundedOutputChunk`, `app/DesktopObservability.ts:161`), written to `<T3 home>/userdata/logs/server-child.log`
   (rotating 10 MiB x 10, `:230`) only after an unexpected exit or a readiness failure, discarded on a clean stop.
   Stop: SIGTERM, SIGKILL after 2 s; at quit wait at most 5 s (`DesktopApp.ts:146-158`). So Tailscale Serve is torn down,
   never SIGKILL first.
6. **Quit and crash.** `destroy()` at ⌘Q is unconfirmed (X6). Add an `NSApplication.willTerminateNotification` stop. Also write
   `embedded-server.pid` (pid, start time, executable path) in the app's own data folder and reap at the next launch only when
   `proc_pidpath` matches the runtime's `t3`. The server has no parent-death watch (no `ppid` or stdin-EOF handling found in
   `apps/server/src`).
7. **Status for TypeScript.** Op `localBackendStatus` and topic `t3.local`: `{state, port, httpBaseUrl, wsBaseUrl, bearerReady,
   restartAttempt, nextRestartMs, lastExit, install, refused}`; `state` is one of `refused`, `runtime-missing`, `installing`, `starting`, `ready`,
   `restarting`, `stopped`, `failed`. `refused` carries the reason in the words of item 8.
8. **Real data and port 3773 are opt-in.** Only the packaged build defaults to the shared `~/.t3` and the 3773 scan (user decision: production shares `~/.t3`
   with the original app). The packaged build is the one that carries `distribution.json` (`{"flavor":"packaged"}`), which `package-app.mjs` of
   `20261005-portable-app-download` writes into the finished bundle's `Contents/Resources` before signing; it is never written into `assets/`, so a leftover
   staging folder cannot turn a development build into a packaged one. Every other build (`host/apple/build.mjs … --bundle --run`, agent and lane runs) is a development flavor and:
   - starts nothing unless both `T3_LOCAL_HOME` (an isolated folder) and `T3_LOCAL_PORT` (16000-16999) are set; with either missing the status is `refused`
     ("Development build: set T3_LOCAL_HOME and T3_LOCAL_PORT to start the local server.") and no process, folder or listener is created;
   - refuses `T3_LOCAL_HOME` when its resolved path (after `~`, `..` and symlinks) is the real `~/.t3`, inside it, or an ancestor of it, and refuses port 3773 and any port outside
     16000-16999 ("Refusing the real T3 home / port 3773 in a development build.");
   - accepts one explicit override, `T3_LOCAL_ALLOW_REAL=1`, used only by the attended real-home smoke row of `20261005-local-primary-environment` (decision U13); it lifts only the home refusal: port 3773 stays refused and the port must still be in 16000-16999.
   `T3_LOCAL_RUNTIME_DIR` points at a pre-extracted tree to skip unpacking (development only). The pattern is the clone's `T3_SSH_COMMAND` guard (`T3Ssh.swift:11-12`).

Excluded: any UI, the exposure and Tailscale arguments (`20261005-this-machine-network-access`), telemetry, WSL, runtime
upgrades, guarding against the original app running at the same time (accepted: one app at a time).

## Context and guidance

Parent specification: [spec](../spec.md). Line numbers are from the mc-orch tree on 2026-10-05; `20261005-hot-file-split` moves code, so find it by symbol.
Tools are named by their `target/t3-ui-parity/…` path (committed under `examples/macos/t3-code/tools/` with the same relative paths, decision U23): `trace-proxy.mjs`, `trace-diff.mjs`.
Every attended or normal-launch row runs a lane build with `T3_LOCAL_HOME=<lane>/t3-home` and `T3_LOCAL_PORT=<lane port 16xxx>`; this ticket enforces that (item 8).
Library revision: `20261005-platforms-v3`. Selected topics: foundations, state-and-data
(await persistence on macOS), testing-and-debugging (an exit code is not proof; record build identity), capabilities. Bundle
assets, child processes, Keychain, native timers and sockets are **unknown in the library**; the pinned main's runtime evidence is the basis.
Decision U3 (2026-10-05, the recommended options): (1) the original desktop runs the server as Electron-as-Node (`ELECTRON_RUN_AS_NODE`,
`DesktopBackendConfiguration.ts:571-587`); this ticket uses the same release's Node single executable (Node 26.8.2, N-API addons, arm64 only;
`apps/server/vite.config.ts:42`): same version and commit, not the same bytes; (2) unpack location `<T3 home>/runtime/versions` (the
product's own layout); (3) drop `client/` from the tree if the spike shows the server starts without it;
(4) archive tool and part size. Licenses: T3 Code is MIT; check that the archive carries its third-party notices and surface them in the licenses page.
Consumer framework revision: the pin from `20261005-clone-on-exact2-main`; Xcode 27.0; pinned Bun 1.4.2.

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| merged task PR | [20261005-hot-file-split](20261005-hot-file-split.md) | pending | Merged into `daehyeon/t3-code` (common prerequisite: room and per-area seams in the shared files) | pending |
| merged task PR | [20261005-clone-on-exact2-main](20261005-clone-on-exact2-main.md) | pending | Merged | pending |
| merged task PR | [20261005-desktop-oracle-and-trace](20261005-desktop-oracle-and-trace.md) | pending | Merged (trace of the token exchange) | pending |
| recorded decision | U2 (apparatus): stage script, fake server for AppKit tests | none | User approves | pending |
| recorded decision | U3 (decided: CLI archive at the release matching the reference pin, `<T3 home>/runtime/versions`; items 3–4 settled by measurement) | none | Decided | user 2026-10-05 |

## Issue assessment at preparation

Checked sources and time: planning pass 2026-10-05 against the local drafts in `../issues/` (unpublished, not reproduced); upstream not searched.

| Issue / reference | Capability and target | Evidence / affected revision | Impact | Next action |
| --- | --- | --- | --- | --- |
| [X4](../issues/20261005-x04-bundle-helper-executables.md) | Executables and large trees in the `.app` | `EXACT2-GAPS.md` X4 at `d2cb661eb`; limits not re-measured | unknown: blocking if the parts exceed the bake limit or modes cannot be restored | Spike first: measure archive size and the bake limit on the pin |
| [X6](../issues/20261005-x06-module-quit-shutdown.md) | Bounded delay of termination for the stop | `destroy()` timing unconfirmed | unknown (workaround: `willTerminate` stop + pid reaper; the reference stops before exit) | Measure ⌘Q to server exit time; if over 5 s or skipped, tell the user |
| [X21](../issues/20261005-x21-two-way-websocket.md) | Native transport | existing | nonblocking | none |
| [X19](../issues/20261005-x19-data-source-timers.md) | Timers | Swift timers for readiness and restart | nonblocking | none |
| new: asset bytes are base64 in the bake | Size after base64 may cut the usable bundle | `EXACT2-GAPS.md` X4 text | unknown | Record at prepare with a measured number |

## Implementation notes

- Do the spike first and stop for a go/no-go: size, bake limit, modes after unpack, signatures after unpack, `t3` runs from the data folder with
  `client/` removed, ⌘Q timing. A no-go on X4 blocks this ticket and `20261005-portable-app-download`.
- Write native code in new files (`T3LocalRuntime.swift`, `T3LocalBackend.swift`, `T3LocalShellEnvironment.swift`, `T3LocalLog.swift`); the only hot-file edit is one registration through the per-area seam that `20261005-hot-file-split` creates for `T3Module.swift`.
- Ported tests are Swift binaries in `apple/tests/local-backend/` with the original names (a change from `bun:test`, because the logic is native; record it in each header). Pure helpers that can live in TS stay bun tests.

## Acceptance and reproduction

| Criterion | Setup/reset and fixture | Action or command | Expected result | Required platform | Proof |
| --- | --- | --- | --- | --- | --- |
| Pin verification | Tampered part; wrong `SHA256SUMS` line; offline cache | `bun stage-runtime.mjs` | Fails with the failing hash; succeeds on the pinned file; no reference path is read (run under a `sandbox-exec` profile that denies the reference checkout) | macOS | test log |
| Ported tests | Fake `t3` script reading the envelope | `apple/tests/local-backend`: "retries HTTP readiness before reporting the backend ready", "restarts an unexpectedly exited backend with the Effect clock", "starts the configured backend and closes the scoped process on stop", "restarts when start is requested during stop teardown", "does not restart after stop cancels a scheduled restart", "cancels a scheduled restart when start is requested manually", "drains trailing child output before persisting an unexpected exit", "keeps a timed-out run active until its process exits", "stopAllPoolInstances bounds the quit finalizer when backends hang", "resolvePrimary produces a stable scoped bootstrap token", the eight macOS cases of `DesktopShellEnvironment.test.ts`, "buffers backend child output and persists it only when a failure is reported", "retains only the last mebibyte of backend child output", "bounds the number of retained backend child output chunks" | Pass with original names | macOS | AppKit log |
| First launch unpack | Empty isolated `T3_LOCAL_HOME` | Launch the lane app | `installing` then `ready`; `.install-complete` = version; `t3` mode 0755; `codesign --verify` passes; manifest hashes match | macOS 26.6.2 | `state`, `stat`, `codesign` log |
| Real server | Lane port 16xxx | `agent ... state`; `curl /.well-known/t3/environment`; `lsof -nP -iTCP -sTCP:LISTEN`; `ps eww -p <pid>` | Ready; only the lane port listens; argv is `t3 --bootstrap-fd 0`; no token in argv or env | macOS | transcript |
| Token exchange | T0 trace on the reference oracle and the clone | Trace diff | Same request: no `scope`, label "T3 Code Desktop"; bearer never logged or written | macOS | ndjson |
| Restart ladder | Fake `t3` that exits before answering readiness; the AppKit test clock supplies `now` | Start; let it exit six times before ready; then let it become ready; then `kill -9` the recorded PID of the ready server once | Delays 0.5, 1, 2, 4, 8, 10 s for the six early exits (`restartAttempt` 1 to 6); once ready the counter is 0, so the next delay after the kill is 0.5 s, not 16 s (`DesktopBackendManager.ts:937`) | macOS | status timeline from `localBackendStatus` |
| Ladder against the real server | Lane port 16xxx | `kill -9` of the recorded server PID twice, with a ready state in between | 0.5 s both times; a second kill before ready gives 1 s | macOS | `state` log |
| Failure log | Make the server exit at start | Read the lane `server-child.log` | Records only for the failed run; clean stop leaves none | macOS | file |
| Quit and crash | Lane app | ⌘Q; then `kill -9` of the recorded app PID and relaunch | Server gone within 5 s of ⌘Q; after a crash, one server only, the old one reaped | macOS | `ps` before/after |
| Development build with no variables | A development build launched with no `T3_LOCAL_*` variables (`bun host/apple/build.mjs macos-t3-code-apple --bundle --run`, then `agent macos --json state logs`) | Record `stat -f %m ~/.t3 ~/.t3/userdata` and `lsof -nP -iTCP:3773`, `pgrep -x t3` before and after, with T3 Code (Nightly) quit | `localBackendStatus.state` is `refused`; the mtimes are unchanged; nothing listens on 3773; no `t3` process; no `runtime` folder created | macOS | transcript, `stat` and `lsof` logs |
| Refusals | Development build | Set `T3_LOCAL_HOME` to `~/.t3`, to `~/.t3/x`, to a symlink to `~/.t3`, to `$HOME`; set `T3_LOCAL_PORT=3773` and `=8080` | Each is `refused` with the reason text; nothing is created; with `T3_LOCAL_ALLOW_REAL=1` and a lane port the home is accepted (not run against the real home here) | macOS | status log |
| Packaged default | AppKit test of the flavor reader on two Resources folders (with and without `distribution.json`) and a scratch `HOME` | `apple/tests/local-backend` | With the marker: home `<scratch>/.t3`, scan start 3773, no `T3_LOCAL_*` needed; without: `refused`. The end-to-end packaged launch is the clean-account row of `20261005-portable-app-download`, which writes the marker | macOS | AppKit log |
| Isolation of lane runs | — | Compare `~/.t3` mtime and listeners before and after every lane run in this ticket | Unchanged; nothing on 3773 | macOS | log |
| Telemetry off | Lane build, isolated home; one session (pair, send, approval) | Read the child's environment (`ps eww <pid>`) and watch its outbound connections (`lsof -i -a -p <pid>`) during the session | `T3CODE_TELEMETRY_ENABLED=false` is set; no connection to the PostHog or OTLP hosts named in issue X39; no `telemetry/anonymous-id` read from the real `~/.t3` | macOS | env excerpt, `lsof` log |

Task-owned source paths: `runtime-pin.json`, `stage-runtime.mjs` (+ test), `app.json` commands, `modules/apple/T3Local*.swift`, `T3Module.swift` (seam registration), `T3Transport.swift` (exchange refactor), `apple/tests/local-backend/`, `.gitignore` lines for generated assets.
Required environment: network for the staging step only; Xcode 27.0; pinned Bun.

## Progress

Planned.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| none | — | — | — | — |

## Next action

`prepare` after the two merged task PRs and the approvals; run the spike and report go/no-go before implementation. Close with clone checks green (bun test, strict tsc, contract build, `cargo test -p macos-t3-code-apple --lib`, `local-backend` and `transport` AppKit binaries), `bun scripts/caps.mjs` after `git add -A`, the repository's five checks, and every moved matrix cell fixed or declared in `EXACT2-GAPS.md` with an issue link.
