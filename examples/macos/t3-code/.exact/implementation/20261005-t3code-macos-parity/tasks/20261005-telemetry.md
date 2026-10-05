---
name: 20261005-telemetry
plan: 20261005-t3code-macos-parity
implementation: blocked
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: daehyeon/t3-code
branch: null
pr_url: null
verified_commit: null
---

# Telemetry: analytics proof, OTLP pass-through and the host-telemetry pipes

## Outcome

For each part the user decides to build in issue X39, the clone behaves as the reference desktop app does. The embedded server sends no product analytics unless the user chose that. OTLP settings reach the server when the user supplies a collector.
The embedded server receives host power data and answers the control messages over two private pipes, so the Background activity switches "Pause when host is locked" and "Pause on battery", the thermal and sleep rules, and the host-power intervals take effect as in the reference, and Diagnostics shows host data.
This ticket starts only after the decision. If the user closes every part, the ticket is closed, and the one setting that keeps analytics off moves into `20261005-embedded-server-runtime` (see the note under Scope).

## Scope and exclusions

Included, per part, if built (reference at `1e2ecbd975`; evidence and file:line are in [X39](../issues/20261005-x39-telemetry.md)):

1. **Part 1, product analytics proof.** The server sends events to PostHog by default (`T3CODE_TELEMETRY_ENABLED` defaults to true, host `https://us.i.posthog.com`; `apps/server/src/telemetry/AnalyticsService.ts:69-80`). The events are `server.boot.heartbeat`, `client.connected`, `client.thread.started`,
   `client.turn.requested`, `provider.turn.completed` and Codex sign-in events, with platform, architecture, version, `clientType`, server OS, architecture and mode. The identifier is a SHA-256 of `~/.codex/auth.json` `tokens.account_id`, else `~/.claude.json` `userID`, else `~/.t3/telemetry/anonymous-id` (`Identify.ts:157-177,204,257-266`).
   Whatever the user chooses, this ticket owns the capture harness and the test: a local listener on a lane port set as `T3CODE_POSTHOG_HOST`, and the assertions below. `20261005-embedded-server-runtime` (scope and the "Telemetry off" row) and every lane server already set `T3CODE_TELEMETRY_ENABLED=false`.
2. **Part 2, OTLP pass-through.** If the user supplies a collector: read `T3CODE_OTLP_TRACES_URL`, `_METRICS_URL`, `_LOGS_URL`, `_EXPORT_INTERVAL_MS`, `_HEADERS`, `_PROTOCOL` from the launch environment (`apps/desktop/src/app/DesktopConfig.ts:48-55`), fall back to persisted observability settings, and put the three URLs into the bootstrap envelope
   (`DesktopBackendConfiguration.ts:68-76,244-255,524-535`). The server then exports to the collector; stdout logs and the local `server.trace.ndjson` are unchanged. The module itself exports nothing. Local `desktop.trace.ndjson` is Electron-specific and is not ported.
3. **Part 3, host-telemetry pipes.** Start the server with descriptors 4 (module to server) and 5 (server to module) and put `desktopTelemetryFd: 4` and `desktopTelemetryControlFd: 5` in the bootstrap (`DesktopBackendConfiguration.ts:561-562`). Messages are one JSON object per line (`DesktopTelemetryPublisher.ts:402`).
   - On attach, send `{"version":1,"type":"desktopTelemetryHello","electronPid":<pid>}`; the module sends its own process id (the schema calls it `electronPid`).
   - Send `desktopTelemetry` snapshots: `version`, `sequence` (increasing), `sampledAtUnixMs`, `electronPid`, `power` {`source`, `idle` ("true"/"false"/"unknown"), `idleSeconds`, `locked`, `suspended`, `onBattery`, `lowPowerMode`, `thermalState` ("unknown", "nominal", "fair", "serious", "critical"), `stale`, `updatedAt`}, `speedLimitPercent`, `electronProcesses` (empty for the clone)
     (`packages/contracts/src/resourceTelemetry.ts:258-345`, `background.ts:14-46`). `source` must be a schema literal (`unknown`, `node-macos-shell`, `node-macos-native`, `node-linux`, `node-windows`, `electron-main`); the choice is made at `prepare` and recorded.
   - Cadence (`DesktopTelemetryPublisher.ts:21-28,125-143`): one sample at start; without diagnostics demand the interval is the idle interval when the host is suspended, locked or idle, else the active interval (defaults 30 s and 2 min; the server sets them with `setHostPowerIntervals` from the settings
     `hostPowerMonitorActiveInterval` and `hostPowerMonitorIdleInterval`). A power event (lock, unlock, suspend, resume, on battery, on AC, thermal change, speed limit) wakes the loop and samples at once. After an interval ends with no event, a suspended flag is cleared (wake recovery).
     With diagnostics demand on (`setDiagnosticsDemand`): 15 s when suspended, locked or under serious or critical thermal state, 5 s on battery, else 1 s. The clone has no Electron processes, so it sends an empty `electronProcesses` and keeps the same interval rules. Idle is read with a 60 s threshold (`:27`).
   - `lowPowerMode` is always `"unknown"` in the reference snapshot, `source` is `electron-main` and `stale` is false. So in the reference the switch "Pause on host low power" never fires from the desktop feed. The clone matches that unless the user asks for more; record it as a known inert switch.
   - Staleness: the server marks a snapshot stale after max(90 s, longer interval + 30 s), checked every 30 s (`apps/server/src/resourceTelemetry/DesktopTelemetryReceiver.ts:33-37,225-226`). Stale means "not constrained" (`apps/server/src/background/BackgroundPolicy.ts:140-155`), so a missing feed silently disables the pause switches.
   - Control messages `requestDesktopUpdate`, `commitDesktopUpdate`, `cancelDesktopUpdate` belong to `20261005-app-update-feed`; here the module ignores unknown control messages without failing.
   - macOS sources: lock and unlock and sleep and wake from workspace and distributed notifications, battery state from the IOKit power-source info, thermal state from `ProcessInfo.thermalState`, idle seconds from the HID idle time; low power mode stays "unknown" as in the reference. The exact calls are chosen at `prepare`.
4. **Settings effect.** With the feed running, the Background activity page (`settings-a-background.ts`) works end to end: the switches "Pause when host is locked" and "Pause on battery" and the two intervals change the server's behavior, as does a suspended or hot host. "Pause on host low power" stays inert, as in the reference.

Excluded: the Browser surface, Clerk and relay tracing (`20261005-t3-connect-sign-in`), the update feed, WSL, a PostHog or OTLP client in the module, and any outbound call to T3's analytics host. Never send test data to T3.

Note for the coordinator: if X39 closes all three parts, close this ticket and move the Part 1 environment setting and its capture test into `20261005-embedded-server-runtime`.

## Context and guidance

Parent specification: [spec](../spec.md). Source behavior: `apps/server/src/telemetry/*`, `apps/desktop/src/telemetry/DesktopTelemetryPublisher.ts`, `apps/desktop/src/app/DesktopObservability.ts`, `apps/server/src/resourceTelemetry/DesktopTelemetryReceiver.ts`, `docs/user/telemetry.md`, `docs/internals/resource-telemetry.md`, `docs/operations/observability.md`.
Library revision: `20261005-platforms-v3`. Selected topics: to select at `prepare`; child-process file descriptors, power state and Keychain are **unknown in the library**, so the pinned `main` and the oracle are the evidence.
Consumer framework revision and toolchain: the pin from `20261005-clone-on-exact2-main`.
Descriptor passing is app-side Swift: Foundation `Process` offers only the standard streams, so the module needs `posix_spawn` file actions (`posix_spawn_file_actions_adddup2`) or a small launcher; confirm at `prepare` that this works from the module under the host's settings. If it cannot, record a new framework issue and keep Part 3 blocked.
Tools are named by their `target/t3-ui-parity/…` path (committed under `examples/macos/t3-code/tools/` with the same relative paths, decision U23): `electron-oracle.mjs`, `trace-diff.mjs`. The capture listener is new apparatus and needs approval (CLAUDE.md: agents add no apparatus without a human saying so).
Every launch runs a lane build with `T3_LOCAL_HOME=<lane>/t3-home` and `T3_LOCAL_PORT=<lane port 16xxx>`; dev and lane builds refuse the real `~/.t3` and port 3773 (see `20261005-embedded-server-runtime`). Part 1 tests also set `T3_LOCAL_HOME`, so the identity file is created inside the lane, never in the real `~/.t3`.
Port changes for headers: `Effect` services become plain Swift types with an injected clock and an injected power source for tests.

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| recorded decision | [X39](../issues/20261005-x39-telemetry.md) | pending | The user decides each part (1 off or on, 2 pass-through or close, 3 build or close), or closes it | pending |
| merged task PR | [20261005-hot-file-split](20261005-hot-file-split.md) | pending | Merged into `daehyeon/t3-code` (common prerequisite: room and per-area seams in the shared files) | pending |
| merged task PR | [20261005-clone-on-exact2-main](20261005-clone-on-exact2-main.md) | pending | Merged | pending |
| merged task PR | [20261005-desktop-oracle-and-trace](20261005-desktop-oracle-and-trace.md) | pending | Merged | pending |
| scheduling preference | After `20261005-embedded-server-runtime` | none | The server starts and stops under the module; bootstrap fields are in place | pending |

## Issue assessment at preparation

Checked sources and time: planning pass 2026-10-05 against the local drafts in `../issues/` (unpublished, not reproduced); upstream not searched.

| Issue / reference | Capability and target | Evidence / affected revision | Impact | Next action |
| --- | --- | --- | --- | --- |
| [X39](../issues/20261005-x39-telemetry.md) | Decision per part | scope decision; not in the library | blocking | `issue-open`, then the user decides |
| [X6](../issues/20261005-x06-module-quit-shutdown.md) | Bounded delay at quit | `EXACT2-GAPS.md` X6 at `d2cb661eb`; not re-measured | nonblocking (the pipes close when the server stops) | none |
| [X19](../issues/20261005-x19-data-source-timers.md) | Data-source timers | DEFERRED policy issue | nonblocking (sampling runs in Swift) | none |
| [X40](../issues/20261005-x40-app-update-feed.md) | Update feed | scope decision | nonblocking (control messages are ignored here) | none |

## Implementation notes

- New Swift file `modules/apple/T3HostTelemetry.swift` (pipes, sampler, framing) and `modules/apple/T3PowerSource.swift` (system calls behind a protocol). `T3Module.swift` stays at op prefixes.
- The sampler is a Swift timer with an injected clock. It stops when the server stops and starts again on restart (the server's restart ladder is in `20261005-embedded-server-runtime`); descriptors are closed on both sides when the child exits.
- A write error ends the sampler quietly; the server keeps running ("A missing or failed collector leaves the server running").
- Nothing from the pipes is logged beyond counts. Part 1's listener binds a lane port only.

## Acceptance and reproduction

| Criterion | Setup/reset and fixture | Action or command | Expected result | Required platform | Proof |
| --- | --- | --- | --- | --- | --- |
| Ported tests | — | `bun test` and `cargo test -p macos-t3-code-apple --lib` plus the new AppKit binary: reference cases of `DesktopTelemetryPublisher.test.ts` (power events, intervals, hello, demand), `DesktopObservability.test.ts` (endpoint precedence), `AnalyticsService.test.ts` (retry delay, buffer limit), `Identify.test.ts` (precedence) as applicable to the built parts | Original test names pass | macOS | logs |
| Part 1, analytics off | Lane server, listener on a lane port as `T3CODE_POSTHOG_HOST`, child environment from the runtime ticket | Connect, start a thread, send a turn with a fake provider, wait 30 s, quit | The listener received zero requests; no `anonymous-id` file is written outside the lane home | macOS | listener log, `find` of the lane home |
| Part 1, analytics on (only if the user chose it) | Same with `T3CODE_TELEMETRY_ENABLED=true` | Same | `POST /batch/` carries the five event names above, the properties of X39, no prompt or file content; first flush within about 1 s of the event | macOS | listener log |
| Part 2 pass-through | Local OTLP HTTP listener on a lane port | Launch with `T3CODE_OTLP_TRACES_URL` pointing at it | The server posts spans there; with none set, nothing is posted; the bootstrap holds the three URLs only when set | macOS | listener log, bootstrap capture |
| Part 3 attach | Fake server that reads fds 4 and 5 and logs lines | Start, stop, restart | A hello line first; snapshots after; descriptors closed when the child exits; restart reattaches with a new hello and sequence from 0 | macOS | fake server log |
| Part 3 values | Injected power source | Change lock, battery, thermal, suspend and speed limit | A snapshot right after each event (no wait for the interval) with the changed fields; `lowPowerMode` stays "unknown"; `source` as chosen; `updatedAt` ISO UTC; `stale` false | macOS | fake server log |
| Part 3 cadence | Test clock; control messages `setHostPowerIntervals` (30 s, 120 s) and a changed pair | Idle for several intervals; lock the screen; send `setDiagnosticsDemand` on | Snapshots at the active interval while active and the idle interval when idle, locked or suspended; the new pair applies at the next wait; with demand on the intervals are 1 s, 5 s on battery, 15 s when suspended, locked or hot | macOS | fake server log |
| Part 3 staleness | Real server, lane port | Stop sending for 100 s | The server marks the snapshot stale (`has not updated for … ms` in its log); resume clears it | macOS | server log |
| Part 3 settings effect | Real server; Background activity switch "Pause when host is locked" on | Lock the Mac, then unlock (attended session) | The server treats the host as constrained while locked and not after unlock (read through the server's diagnostics call named at `prepare`) | macOS | recording, call log |
| Unknown control messages | Fake server sends `requestDesktopUpdate` and unknown types | — | Ignored; no crash; sampler continues | macOS | log |
| Failure | Close fd 5 from the server side; fill fd 4 | — | The sampler ends or backs off without stopping the server | macOS | status timeline |

Task-owned source paths: `modules/apple/{T3HostTelemetry,T3PowerSource}.swift`, `modules/apple/T3Backend*.swift` (bootstrap and spawn, shared with the runtime ticket), `settings-a-background.ts` (only if a display text changes), tests under `apple/tests/telemetry` and `bun test`, the capture listener (apparatus, approval needed).
Required environment: Xcode 27.0, pinned Bun, a lane port range 16000-16999, an attended session with the Mac's lock screen for the lock row.

## Progress

Blocked. Not started.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| none | — | — | — | X39 decision |

## Next action

Blocked until X39 is resolved or decided; then `prepare`, or close this ticket if the decision is to close.
