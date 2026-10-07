---
name: 20261005-embedded-server-runtime
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: unverified
delivery: merged
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-embedded-server-runtime
pr_url: https://github.com/ccheever/exact2/pull/222
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

Parent specification: [spec](../../spec.md). Line numbers are from the mc-orch tree on 2026-10-05; `20261005-hot-file-split` moves code, so find it by symbol.
Tools are named by their `target/t3-ui-parity/…` path (committed under `examples/t3-code/tools/` with the same relative paths, decision U23): `trace-proxy.mjs`, `trace-diff.mjs`.
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
| merged task PR | [20261005-clone-on-exact2-main](../20261005-clone-on-exact2-main.md) | pending | Merged | pending |
| merged task PR | [20261005-desktop-oracle-and-trace](../20261005-desktop-oracle-and-trace.md) | pending | Merged (trace of the token exchange) | pending |
| recorded decision | U2 (apparatus): stage script, fake server for AppKit tests | none | User approves | pending |
| recorded decision | U3 (decided: CLI archive at the release matching the reference pin, `<T3 home>/runtime/versions`; items 3–4 settled by measurement) | none | Decided | user 2026-10-05 |

## Issue assessment at preparation

Checked sources and time: planning pass 2026-10-05 against the local drafts in `../issues/` (unpublished, not reproduced); upstream not searched.

| Issue / reference | Capability and target | Evidence / affected revision | Impact | Next action |
| --- | --- | --- | --- | --- |
| [X4](../../issues/20261005-x04-bundle-helper-executables.md) | Executables and large trees in the `.app` | `EXACT2-GAPS.md` X4 at `d2cb661eb`; limits not re-measured | unknown: blocking if the parts exceed the bake limit or modes cannot be restored | Spike first: measure archive size and the bake limit on the pin |
| [X6](../../issues/20261005-x06-module-quit-shutdown.md) | Bounded delay of termination for the stop | `destroy()` timing unconfirmed | unknown (workaround: `willTerminate` stop + pid reaper; the reference stops before exit) | Measure ⌘Q to server exit time; if over 5 s or skipped, tell the user |
| [X21](../../issues/20261005-x21-two-way-websocket.md) | Native transport | existing | nonblocking | none |
| [X19](../../issues/20261005-x19-data-source-timers.md) | Timers | Swift timers for readiness and restart | nonblocking | none |
| new: asset bytes are base64 in the bake | Size after base64 may cut the usable bundle | `EXACT2-GAPS.md` X4 text | unknown | Record at prepare with a measured number |

## Implementation notes

- Do the spike first and stop for a go/no-go: size, bake limit, modes after unpack, signatures after unpack, `t3` runs from the data folder with
  `client/` removed, ⌘Q timing. A no-go on X4 blocks this ticket and `20261005-portable-app-download`.
- Write native code in new files (`T3LocalRuntime.swift`, `T3LocalBackend.swift`, `T3LocalShellEnvironment.swift`, `T3LocalLog.swift`); the only hot-file edit is one registration through the per-area seam that `20261005-hot-file-split` creates for `T3Module.swift`.
- Ported tests are Swift binaries in `macos/tests/local-backend/` with the original names (a change from `bun:test`, because the logic is native; record it in each header). Pure helpers that can live in TS stay bun tests.

## Acceptance and reproduction

| Criterion | Setup/reset and fixture | Action or command | Expected result | Required platform | Proof |
| --- | --- | --- | --- | --- | --- |
| Pin verification | Tampered part; wrong `SHA256SUMS` line; offline cache | `bun stage-runtime.mjs` | Fails with the failing hash; succeeds on the pinned file; no reference path is read (run under a `sandbox-exec` profile that denies the reference checkout) | macOS | test log |
| Ported tests | Fake `t3` script reading the envelope | `macos/tests/local-backend`: "retries HTTP readiness before reporting the backend ready", "restarts an unexpectedly exited backend with the Effect clock", "starts the configured backend and closes the scoped process on stop", "restarts when start is requested during stop teardown", "does not restart after stop cancels a scheduled restart", "cancels a scheduled restart when start is requested manually", "drains trailing child output before persisting an unexpected exit", "keeps a timed-out run active until its process exits", "stopAllPoolInstances bounds the quit finalizer when backends hang", "resolvePrimary produces a stable scoped bootstrap token", the eight macOS cases of `DesktopShellEnvironment.test.ts`, "buffers backend child output and persists it only when a failure is reported", "retains only the last mebibyte of backend child output", "bounds the number of retained backend child output chunks" | Pass with original names | macOS | AppKit log |
| First launch unpack | Empty isolated `T3_LOCAL_HOME` | Launch the lane app | `installing` then `ready`; `.install-complete` = version; `t3` mode 0755; `codesign --verify` passes; manifest hashes match | macOS 26.6.2 | `state`, `stat`, `codesign` log |
| Real server | Lane port 16xxx | `agent ... state`; `curl /.well-known/t3/environment`; `lsof -nP -iTCP -sTCP:LISTEN`; `ps eww -p <pid>` | Ready; only the lane port listens; argv is `t3 --bootstrap-fd 0`; no token in argv or env | macOS | transcript |
| Token exchange | T0 trace on the reference oracle and the clone | Trace diff | Same request: no `scope`, label "T3 Code Desktop"; bearer never logged or written | macOS | ndjson |
| Restart ladder | Fake `t3` that exits before answering readiness; the AppKit test clock supplies `now` | Start; let it exit six times before ready; then let it become ready; then `kill -9` the recorded PID of the ready server once | Delays 0.5, 1, 2, 4, 8, 10 s for the six early exits (`restartAttempt` 1 to 6); once ready the counter is 0, so the next delay after the kill is 0.5 s, not 16 s (`DesktopBackendManager.ts:937`) | macOS | status timeline from `localBackendStatus` |
| Ladder against the real server | Lane port 16xxx | `kill -9` of the recorded server PID twice, with a ready state in between | 0.5 s both times; a second kill before ready gives 1 s | macOS | `state` log |
| Failure log | Make the server exit at start | Read the lane `server-child.log` | Records only for the failed run; clean stop leaves none | macOS | file |
| Quit and crash | Lane app | ⌘Q; then `kill -9` of the recorded app PID and relaunch | Server gone within 5 s of ⌘Q; after a crash, one server only, the old one reaped | macOS | `ps` before/after |
| Development build with no variables | A development build launched with no `T3_LOCAL_*` variables (`bun host/apple/build.mjs t3-code-macos --bundle --run`, then `agent macos --json state logs`) | Record `stat -f %m ~/.t3 ~/.t3/userdata` and `lsof -nP -iTCP:3773`, `pgrep -x t3` before and after, with T3 Code (Nightly) quit | `localBackendStatus.state` is `refused`; the mtimes are unchanged; nothing listens on 3773; no `t3` process; no `runtime` folder created | macOS | transcript, `stat` and `lsof` logs |
| Refusals | Development build | Set `T3_LOCAL_HOME` to `~/.t3`, to `~/.t3/x`, to a symlink to `~/.t3`, to `$HOME`; set `T3_LOCAL_PORT=3773` and `=8080` | Each is `refused` with the reason text; nothing is created; with `T3_LOCAL_ALLOW_REAL=1` and a lane port the home is accepted (not run against the real home here) | macOS | status log |
| Packaged default | AppKit test of the flavor reader on two Resources folders (with and without `distribution.json`) and a scratch `HOME` | `macos/tests/local-backend` | With the marker: home `<scratch>/.t3`, scan start 3773, no `T3_LOCAL_*` needed; without: `refused`. The end-to-end packaged launch is the clean-account row of `20261005-portable-app-download`, which writes the marker | macOS | AppKit log |
| Isolation of lane runs | — | Compare `~/.t3` mtime and listeners before and after every lane run in this ticket | Unchanged; nothing on 3773 | macOS | log |
| Telemetry off | Lane build, isolated home; one session (pair, send, approval) | Read the child's environment (`ps eww <pid>`) and watch its outbound connections (`lsof -i -a -p <pid>`) during the session | `T3CODE_TELEMETRY_ENABLED=false` is set; no connection to the PostHog or OTLP hosts named in issue X39; no `telemetry/anonymous-id` read from the real `~/.t3` | macOS | env excerpt, `lsof` log |

Task-owned source paths: `runtime-pin.json`, `stage-runtime.mjs` (+ test), `app.json` commands, `modules/apple/T3Local*.swift`, `T3Module.swift` (seam registration), `T3Transport.swift` (exchange refactor), `macos/tests/local-backend/`, `.gitignore` lines for generated assets.
Required environment: network for the staging step only; Xcode 27.0; pinned Bun.

## Progress

Implemented on `feat(example)/t3-code-embedded-server-runtime` (2026-10-07), after merging main `463acda68`
(X4 fixed by main #215, X6 by main #200, X37 by #199; the only conflict was `QUEUE.md`, both sides kept).

### Spike (go, every criterion passed)

| Criterion | Measured | Verdict |
| --- | --- | --- |
| Pin | `0.0.46-nightly.20261005.2667` (tag `v0.0.46-nightly.20261005.2667`, commit `37de6cbde65c`), the first release built from `1e2ecbd975` or later (4 commits after it, `gh api compare`); not older than `CLIENT_VERSION` `0.0.46-nightly.20261004.1` | go |
| Size | `t3-…-darwin-arm64.tar.gz` 77,117,663 bytes (73.5 MiB; download 2.9 s). Unpacked: 259,924,430 bytes in 4,290 files (4,451 entries, no symlinks), 11 Mach-O files; `t3` 151 MB, `client/` 50 MB, `node_modules/` 58 MB, `resource-monitor/` 0.5 MB | go |
| Bundle under #215 | The archive as one file in a native resource tree (`host.macos.resources`, `server-runtime` → `Resources/t3-runtime`): the bundle builds (incremental 1:38), 59.5 MB → 133.8 MB, the archive's SHA-256 in the bundle equals the pin, `codesign --verify --deep --strict` passes on the bundle. Native resources bypass the bake (no base64, no 256 MiB buffer, no 64 MiB per-file rule, no parts) | go |
| Why the archive and not the tree | #215 re-signs every Mach-O file in a native resource tree. Measured on `t3`: the release's signature (Developer ID T3 Tools, Inc. `ARK85ZXQ4Z`, hardened runtime, JIT / unsigned executable memory / library-validation entitlements) becomes ad hoc with no entitlements; signed as `exact release` signs (`--options runtime`, no entitlements) `t3` aborts at start (`Fatal process out of memory: Failed to reserve virtual memory for CodeRange`, exit 133). The archive keeps the release's bytes; U3 already put the unpacked tree in `<T3 home>/runtime/versions` | unpack kept (U3) |
| Modes and signatures after unpack | `/usr/bin/tar -xzf` 0.59 s; `t3`, `spawn-helper`, `t3-resource-monitor` 0755; 11 of 11 Mach-O files pass `codesign --verify --strict`; `t3` keeps its Developer ID signature and entitlements | go |
| Starts from the data folder | `t3 --bootstrap-fd 0`, envelope on stdin, scratch home, port 16437: ready in 2.86 s cold, 1.61 s warm; `serverVersion` is the pin; the exchange with no `scope` grants all 8 scopes; the bearer still authenticates after a server restart. `client/` kept: the server serves it to network clients (`resolveStaticDir`), as the reference's server tree does | go |
| SIGTERM and ⌘Q (with #200) | SIGTERM → exit in 0.365 s (exit code 130). In the app (lane copy, quit through the app menu's terminate path, `osascript … to quit`): server gone 0.79 s and app gone 0.864 s after the quit; the status log shows `stopped` and then `code=130` 33 ms apart. `destroy()` runs synchronously in `applicationWillTerminate` (#200), so the bounded wait (≤ 5 s) holds the quit as the reference's finalizer does | go |

### What was built

| Item | Files |
| --- | --- |
| Pin and stage step | `server-runtime/runtime-pin.json` (committed); `stage-runtime.mjs` (`app.json` `commands.runtime`): download + cache (`.runtime-cache/`, `--offline`), size and pin = `SHA256SUMS` = computed hash, extract, `codesign --verify --strict` on every Mach-O, `t3 --version` with an empty environment, one start on a scratch home and lane port, then the archive and `runtime-manifest.json` beside the pin; `--no-smoke` for sandboxed runs. `.gitignore` (example-local) keeps only the pin |
| Bundle | `app.json` `host.macos.resources: [{from: "server-runtime", to: "Resources/t3-runtime"}]` (#215) |
| Install | `T3LocalRuntime.swift`: `pinnedRuntimePaths` layout, `.install-complete`, an installed runtime (also one the `t3` installer wrote) used as is, `flock` lock, `.staging-*` cleanup, archive hash before expanding, manifest check (type, mode, size, SHA-256, link, nothing extra), `t3` 0755, `t3 --version`, rename; `installing {verify|extract, fraction}`, `runtime-missing`, `failed {reason}` |
| Start | `T3LocalBackend.swift`: the item-8 policy (`T3LocalPolicy`), `resolveDesktopBackendPort` (configured port wins, else the scan over 127.0.0.1, 0.0.0.0, ::), the envelope (`resolvePrimaryStartConfig` for the SEA, `--bootstrap-fd 0`, cwd home, no telemetry fds or OTLP, `resourceMonitorPath` when the tree has one), token 24 random bytes once per run, `T3LocalAuth` (one exchange per app run, no scope, "T3 Code Desktop", desktop; memory only), pid file and reaper, status topic `t3.local` and a stderr status timeline; `T3LocalShellEnvironment.swift` (`installPosixEnvironment` for darwin, the ten `DESKTOP_BACKEND_ENV_NAMES` removed, `T3CODE_TELEMETRY_ENABLED=false`) |
| Supervise | `T3LocalBackendManager.swift` (port of `makeBackendInstance`/`runBackendProcess`: readiness rounds, `min(500 ms·2ⁿ, 10 s)` reset on ready, stop/restart races, 5 s output drain, `stopAll` with a 5 s bound); `T3LocalLog.swift` (`appendBoundedOutputChunk`, rotating 10 MiB × 10, the reference's JSON records in `<T3 home>/userdata/logs/server-child.log`) |
| Quit and crash | The backend is one per app; each session's module attaches at init and detaches at `destroy()` (#200: every session is destroyed at quit); the last detach stops it (bounded 5 s). The same stop runs from `atexit`, because the agent driver's end of drive calls `exit(0)` without tearing sessions down (`Agent.swift` `exitAfterStorage`). No `willTerminateNotification` observer. `embedded-server.pid` in the app's data folder; the next launch reaps a leftover only when pid, start time and `proc_pidpath` match |
| TypeScript | `local-backend.ts` (`readLocalBackend`, `parseLocalBackendStatus`); `client.ts` keeps `localBackend` from `localBackendStatus` (one line in `refresh`). No UI |
| Seams | `T3Module+Local.swift` (op `localBackendStatus`), one entry in `T3Module.swift`'s area list plus attach/detach; `T3RemoteAuth.exchangeForm` takes the client metadata (remote keeps "Exact T3 for Mac") |
| Tests | `macos/tests/local-backend/` (44 XCTest cases); `local-backend.test.ts` (6), `stage-runtime.test.ts` (6) |

Ported tests (original names; each file's header records the change from vitest/Effect to XCTest over the
native port): "retries HTTP readiness before reporting the backend ready", "re-probes readiness after the first
budget expires while the backend is still alive", "restarts an unexpectedly exited backend with the Effect clock",
"starts the configured backend and closes the scoped process on stop", "restarts when start is requested during
stop teardown", "keeps a timed-out run active until its process exits", "does not restart after stop cancels a
scheduled restart", "cancels a scheduled restart when start is requested manually", "drains trailing child output
before persisting an unexpected exit", "stopAllPoolInstances bounds the quit finalizer when backends hang",
"resolvePrimary produces a stable scoped bootstrap token", "exchanges the desktop bootstrap credential only once",
the seven darwin cases of `DesktopShellEnvironment.test.ts` plus its probe-failure case, "buffers backend child
output and persists it only when a failure is reported", "keeps buffering output after a non-terminal failure
snapshot", "retains only the last mebibyte of backend child output", "bounds the number of retained backend child
output chunks", "advances a retained output offset instead of repeatedly copying a full head chunk".

Differences from the reference, each with its reason: the server is the release's Node SEA, not Electron-as-Node
(U3); the envelope goes on fd 0, not fd 3 (Foundation `Process` cannot pass fd 3; the reference's WSL path uses
fd 0); no telemetry fds, OTLP or PostHog (telemetry excluded, X39); the login-shell values go into the server's
environment, not the app's own process; a first-launch unpack exists (U3, and the re-signing finding above); the
WSL preflight states are not ported (no WSL, X41).

## Acceptance results

| Criterion | Result | Proof |
| --- | --- | --- |
| Pin verification | pass. Tampered archive (one byte) `--offline`: fails naming the computed hash `3d2aea2d…` against the pin's; a `SHA256SUMS` listing another hash: fails naming it; the pinned file from the cache under `sandbox-exec` denying every read of the reference checkout: passes (`--no-smoke`: the server's own spawns fail with `EPERM` under any `sandbox-exec` profile, `(allow default)` alone included, so the smoke ran outside it and passed: ready in 1,610 ms) | `pin-verification.log` (lane, not committed); `stage-runtime.test.ts` |
| Ported tests | pass (44/44 XCTest, 12/12 bun) | `macos/tests/local-backend` |
| First launch unpack | pass. Empty lane home: `installing` (verify 0.04 s, extract and check 2.9 s), `starting`, `ready` 1.65 s after the spawn, bearer 3 ms later; 5.15 s from `open` to ready with the bearer. `.install-complete` = the version; `t3` 0755; 11/11 Mach-O pass `codesign --verify --strict`, `t3` still Developer ID; 4,451 manifest entries, 0 mismatches. The second launch has no `installing` line | After session §1, §4 |
| Real server | pass. Process tree `ExactMac` → `t3 --bootstrap-fd 0` (argv exactly that); only `127.0.0.1:16437` listens; no 48-hex token in argv or environment; `/.well-known/t3/environment` serves the pinned version | After session §1 |
| Token exchange | pass by request comparison: the ported test asserts the form is the reference's (`grant_type`, `subject_token`, the two token types, `client_label` "T3 Code Desktop", `client_device_type` desktop; no `scope`, no `client_os`), sent once; on the lane server `t3 auth session list` shows one bearer session with all 8 scopes. The trace diff against the reference oracle was not run: its tools come with `20261005-desktop-oracle-and-trace`, not merged | `auth.swift`; After session §1 |
| Restart ladder | pass (AppKit test clock): six early exits give 0.5, 1, 2, 4, 8, 10 s (`restartAttempt` 1 to 6); ready resets to 0; a kill after ready gives 0.5 s | `manager.swift` `testRestartLadder` |
| Ladder against the real server | pass. Four `kill -9` of a ready server: 500 ms each time (attempt 1, ready again in 2.3 s, attempt back to 0). Exits before ready (port held by another listener for 3.5 s): 500, then 1,000, then 2,000 ms, then ready and 0. The "second kill before ready" was produced by the held port's exits (the scripted second kill found no pid), which is the same before-ready case | After session §2 |
| Failure log | pass. 32 lines in `server-child.log`: START/END records (with output) for the six failed runs (`signal=9` ×4, `code=1` ×2) only; the first clean run left none and the quit added none (32 → 32) | After session §2, §3 |
| Quit and crash | pass. Quit: server gone 0.79 s, app 0.864 s. `kill -9` of the app: the server stays (ppid 1, still listening on 16437); the next launch logs "stopped the server pid 66371 a previous run left behind", the old one is gone and exactly one server runs | After session §3, §4 |
| Development build with no variables | pass. `agent.mjs macos` with no `T3_LOCAL_*`: status `refused` with the item-8 text; `~/.t3` and `~/.t3/userdata` mtimes unchanged, nothing on 3773, no `t3` process, `~/.t3/runtime` unchanged (Oct 2) | after-agent A |
| Refusals | pass. AppKit: `~/.t3`, `~/.t3/x`, a symlink to `~/.t3`, `~`, `$HOME`, `~/lanes/../.t3`, the account home when `HOME` differs; ports 3773, 8080, 15999, 17000, `x`; `T3_LOCAL_ALLOW_REAL=1` lifts only the home refusal. Live: `T3_LOCAL_HOME=~/.t3` gives `refused` ("Refusing the real T3 home / port 3773 in a development build.") and nothing changes | `install.swift` `LocalPolicyTests`; after-agent B |
| Packaged default | pass (AppKit): with `distribution.json` home `<scratch>/.t3`, configured port none, scan from 3773; without, `refused`. The end-to-end packaged launch belongs to `20261005-portable-app-download` | `LocalPolicyTests` |
| Isolation of lane runs | pass. Before and after every session: `~/.t3` mtime `1791202431`, `userdata` `1791315809`, no listener on 3773, T3 Code (Nightly) not running; lane copies used their own bundle ids; their data, caches and preferences were deleted; no Keychain item was created | session logs |
| Telemetry off | pass for the criterion: `T3CODE_TELEMETRY_ENABLED=false` in the server's environment; no connection to `us.i.posthog.com` (its addresses `3.41.202.x`); no OTLP endpoint; the lane home has no telemetry id and the real `~/.t3` is untouched. Observed connections: the model manifest (`raw.githubusercontent.com`), provider version checks (`registry.npmjs.org`), and one HTTPS connection to an AWS us-east-1 host (`52.44.53.175`, `98.94.121.104`; not a PostHog address; its name was not resolved). Pair was run, send and approval were not: no provider fixture is committed for this lane and a real provider would be an outside effect | after-agent C, `server-connections.txt` |

## Evidence

Before = `t3-code-evidence-base` at `887b2491b` (feature tip, untouched); After = this branch. Lane copies
`com.exact.t3code.laneesr.before` / `.after`, `T3_LOCAL_HOME=<lane>/t3-home`, `T3_LOCAL_PORT=16437`.
Sessions under `.t3-live-drive-lock` 15:59:58–16:08:31: After (first attempt 16:00:03, lost to a script error:
its server-pid pattern used `/private/tmp`, while the policy resolves the home to `/tmp`; the retry ran
16:03:06–16:03:42 and is the record), After agent rows 16:04:25–16:05:54, Before 16:07:53–16:08:18.

| Scenario | Before | After |
| --- | --- | --- |
| Bundle contents and size | 60,784 KiB; `Contents/Resources`: `AppIcon.icns assets receipt.json` | 137,368 KiB; adds `t3-runtime/` (archive 77,117,663 bytes, SHA-256 = pin; `runtime-manifest.json` 734,787; `runtime-pin.json`) |
| Process tree after launch (same lane variables) | `ExactMac` alone; no child; nothing on 16437; no `t3.local` line | `ExactMac` (62308) → `t3 --bootstrap-fd 0` (62874); `127.0.0.1:16437` only |
| Readiness (`localBackendStatus` timeline) | op absent | `installing` → `starting` (pid) → `ready` → `bearerReady: true`, 5.15 s from `open` on an empty home |
| Quit | app gone 0.129 s; no server to stop | server gone 0.79 s, app 0.864 s after the quit; no server left; failure log unchanged |
| Crash (`kill -9` of the app) | — (no server) | server orphaned (ppid 1) until the next launch reaps it; then one server |
| Restart backoff | — | 0.5 s after each kill of a ready server; 0.5, 1, 2 s for exits before ready, then 0 |
| Pairing with `http://127.0.0.1:16437` | "Could not connect to the server." | Connected: "Daehyeon's MacBook Pro http://127.0.0.1:16437/" |

![pair](https://raw.githubusercontent.com/ccheever/exact2/t3-code-evidence/embedded-server-runtime/01-pair-with-embedded-server.png)

## Verification

- `bun test examples/t3-code`: 2,310 pass, 0 fail (193 files); strict `tsc` on `app.ts` clean; `contract build`:
  2,543 slots, 45 resources, 58,929 nodes; `cargo test -p t3-code-macos --lib`: 11 pass.
- AppKit/XCTest binaries (README recipe): all 30 pass, `local-backend` 44/44, `transport` 49/49, `fleet` 9/9
  (mermaid needs a live server and timeline-keyboard its own recipe; neither touches this change).
- Repository: `cargo build --all-targets --keep-going`, `cargo test --lib --bins --tests --no-fail-fast`
  (3,348 pass, 0 fail), `cargo clippy --all-targets --keep-going -- -D warnings`, `cargo fmt --all -- --check`,
  `bun scripts/caps.mjs` (after `git add -A`), `bun scripts/boot.mjs`: all exit 0.

## Open items, each with its reason

- Token-exchange trace diff: needs the trace tools of `20261005-desktop-oracle-and-trace` (not merged).
- Third-party notices in the licenses page: the archive carries `client/third-party-licenses.json`; showing it is
  UI, which this ticket excludes (licenses page work with `20261005-local-primary-environment`).
- The "Telemetry off" session's send and approval: no committed provider fixture; a real provider is an outside
  effect.
- Packaged end-to-end launch on a clean account: `20261005-portable-app-download`.
- X4 residual (framework, recorded in the X4 file): native resource trees re-sign Mach-O without the file's own
  entitlements, so a hardened-runtime release of a JIT helper shipped as a tree would not start.

## Next action

Review the PR. `20261005-local-primary-environment` builds "This machine" on `localBackendStatus`, `t3.local`
and `T3LocalBackend.shared.bearerToken()`.
