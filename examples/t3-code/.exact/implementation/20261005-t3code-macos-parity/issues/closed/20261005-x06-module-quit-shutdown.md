---
name: 20261005-x06-module-quit-shutdown
plan: 20261005-t3code-macos-parity
status: moved-to-main
kind: framework-gap
blocks: [20261005-app-activation, 20261005-app-update-feed, 20261005-embedded-server-runtime, 20261005-managed-codex-chatgpt, 20261005-telemetry]
upstream_url: https://github.com/ccheever/exact2/issues/105
reproduced_on: null
rest_upstream_url: https://github.com/ccheever/exact2/issues/269
---

# X6: A module hook at quit that can delay termination for a bounded time

Moved to main `issues/20261009-macos-orderly-sigterm-quit-hold.md` (2026-10-09); tracked there.

## Summary

When a user quits T3 Code, the desktop app holds the quit until it has stopped its server: SIGTERM first, SIGKILL after 2 s, and a total wait of at most 5 s.
The server then removes its own Tailscale Serve mapping and releases its tunnel. On the exact2 macOS host, ⌘Q returned `.terminateNow`, and a module's `destroy()` did not run at ⌘Q.
The clone needed a supported hook that holds termination for a bounded time.

## Why it arose

### The T3 Code behavior
- Trigger: Electron's `before-quit` (⌘Q or the app menu's Quit), or SIGINT/SIGTERM sent to the app (`apps/desktop/src/app/DesktopLifecycle.ts:99-135` for `before-quit`, `:141-160` for signals).
- The first `before-quit` is cancelled with `event.preventDefault()`. The app sets `quitting`, destroys all windows, and waits for the shutdown to finish (`requestDesktopShutdownAndWait`). Then it marks the quit as allowed and quits again (`DesktopLifecycle.ts:115-135`).
- The shutdown work is a layer finalizer: `stopAllPoolInstances` (`apps/desktop/src/app/DesktopApp.ts:335-347`). It stops every backend with a 5 s timeout each, "to guarantee the quit path makes progress even if a backend hangs" (`:146-158`).
- Stopping a backend: SIGTERM, then a forced kill after 2 s (`DesktopBackendManager.ts:67,485-486`). The server's finalizers disable the Tailscale Serve mapping it created (`apps/server/src/server.ts:733-770`) and release the managed tunnel (`:807-812`).
- The same stop runs before an update install (`apps/desktop/src/updates/DesktopUpdates.ts:635-648`).

### Where the clone hit it
`20261005-embedded-server-runtime` starts the server, `20261005-this-machine-network-access` verifies Tailscale teardown on top of it, and `20261005-app-activation` removes its socket at quit.
Before main #200 the plan's workaround was a `willTerminateNotification` observer plus a next-launch reaper; T3Ssh already had such an observer ([native-module-termination](20261006-native-module-termination.md)).

## Clone workaround

- The embedded server stops in the module's synchronous `destroy()` (SIGTERM, SIGKILL after 2 s, at most 5 s, as `stopAllPoolInstances`), built by
  [20261005-embedded-server-runtime](../../tasks/closed/20261005-embedded-server-runtime.md); no `willTerminateNotification` observer.
- The pid file (`embedded-server.pid`) and the next-launch reaper stay as crash safety (`kill -9` of the app leaves the server running with ppid 1 until the next launch stops it).
- The agent driver's end of drive (`ExactKit/Agent.swift` `exitAfterStorage`) calls `exit(0)` without destroying sessions, so the module also stops the server from `atexit`.
- The `t3 app` control socket (`T3AppControl.swift`, app-activation, #254) closes with the last session's module: `T3Module.destroy()` detaches it, closes the listener and unlinks the socket file; for the agent's `exit(0)` it is unlinked from `atexit`.
- Once main has a bounded hold: move the embedded server's stop out of the blocking `destroy()` into it; keep the pid-file reaper and the `atexit` stop.

## Evidence and history

- Filed as [#105](https://github.com/ccheever/exact2/issues/105) (2026-10-06). Closed by main #200 (`6cd178efc`), in the feature branch since main `463acda68`
  ([20261007-adopt-main-fixes-r4](../../tasks/closed/20261007-adopt-main-fixes-r4.md)): `applicationWillTerminate` destroys every session synchronously, so a native module's `destroy()` runs at
  ⌘Q, at an Apple Event quit and when the last window closes (before, it ran in 3 of 8 last-window closes and never at ⌘Q).
- Adopted for SSH in adopt-main-fixes-r4: T3Ssh's own `willTerminateNotification` observer was removed (live: both SSH tunnels end at ⌘W, an Apple Event quit and ⌘Q).
- Adopted by embedded-server-runtime (2026-10-07). Measured on a lane copy: a quit through `osascript … to quit` took the server down in 0.79 s and the app in 0.864 s; the server logged its SIGTERM exit (code 130) 33 ms after the stop.
- Adopted by app-activation (2026-10-08, #254). Live on a lane copy: an Apple Event quit while a request waited on a stopped server answered the CLI at once and left no socket file.
- The rest filed as [#269](https://github.com/ccheever/exact2/issues/269) ([Design], 2026-10-08), reproduced on main `0365ad1a4` before filing: a module `destroy()` that waits 3 s blocks the main thread through an Apple Event quit (3.42 s, no hold API or budget); `kill -TERM` ends in 0.04 s without `destroy()`.
- #269's SIGTERM part merged as main #313 (`a3d61c023`; SIGTERM through the orderly quit, synchronous `destroy()`), brought in by main adoption round 7; nothing to remove.
