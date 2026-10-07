---
name: 20261005-x06-module-quit-shutdown
plan: 20261005-t3code-macos-parity
status: fixed-upstream-adopted-in-part
kind: framework-gap
blocks: [20261005-app-activation, 20261005-app-update-feed, 20261005-embedded-server-runtime, 20261005-managed-codex-chatgpt, 20261005-telemetry]
upstream_url: https://github.com/ccheever/exact2/issues/105
reproduced_on: exact2 main (fixed by PR #200, a091828a3)
---

# X6: A module hook at quit that can delay termination for a bounded time

## Summary

When a user quits T3 Code, the desktop app holds the quit until it has stopped its server: SIGTERM first, SIGKILL after 2 s, and a total wait of at most 5 s.
The server then removes its own Tailscale Serve mapping and releases its tunnel. On the exact2 macOS host, ⌘Q returns `.terminateNow`, and whether a module's `destroy()` runs at ⌘Q is unconfirmed.
The clone has no embedded server yet. The plan stops the server from a `willTerminateNotification` observer and reaps leftovers at the next launch. The clone needs a supported hook that holds termination for a bounded time.

## Why this issue arose

### The T3 Code behavior
- Trigger: Electron's `before-quit` (⌘Q or the app menu's Quit), or SIGINT/SIGTERM sent to the app (`apps/desktop/src/app/DesktopLifecycle.ts:99-135` for `before-quit`, `:141-160` for signals).
- The first `before-quit` is cancelled with `event.preventDefault()`. The app sets `quitting`, destroys all windows, and waits for the shutdown to finish (`requestDesktopShutdownAndWait`). Then it marks the quit as allowed and quits again (`DesktopLifecycle.ts:115-135`).
- The shutdown work is a layer finalizer: `stopAllPoolInstances` (`apps/desktop/src/app/DesktopApp.ts:335-347`). It stops every backend with a 5 s timeout each, "to guarantee the quit path makes progress even if a backend hangs" (`:146-158`).
- Stopping a backend: SIGTERM, then a forced kill after 2 s (`DesktopBackendManager.ts:67,485-486`). The server registers finalizers that undo what it set up:
  it disables the Tailscale Serve mapping it created (`apps/server/src/server.ts:733-770`, `disableTailscaleServe`) and registers a release of the managed tunnel (`:807-812`).
  The plan's rule is to never send SIGKILL first, so those finalizers can run (that they run on SIGTERM is to confirm in the spike of `20261005-embedded-server-runtime`).
- The same stop runs before an update install (`apps/desktop/src/updates/DesktopUpdates.ts:635-648`).
- Failure state: the stop has a 5 s timeout, so the quit path makes progress even if a backend hangs (`DesktopApp.ts:146-158`, code comment).
- Reference tests: `apps/desktop/src/app/DesktopLifecycle.test.ts` and the pool test "stopAllPoolInstances bounds the quit finalizer when backends hang" (recorded in `20261005-embedded-server-runtime`).

### What exact2 does today
Quoted from `examples/t3-code/EXACT2-GAPS.md` section X6 (written from framework source at exact2 `c1522fdac`, checked against `main` `d2cb661eb`; not re-measured for this plan):
- "On main there is no `applicationWillTerminate`; ⌘Q returns `.terminateNow` (`ExactMac/main.swift:400-413`). `destroy()` runs only from `Session.destroy` on `windowWillClose` (`ExactKit/Session.swift:1378-1398`);
  whether that runs at ⌘Q is unconfirmed. Governing: LLP 1069.010 Q4, D7."
- "Support needed. A module hook at quit that can delay termination for a bounded time."
- Bundled library (`20261005-platforms-v3`): app lifecycle and quit are **not covered: unknown**.
- Observed in the clone on 2026-10-05: ⌘Q goes through the clone's own quit-hold object (`modules/apple/T3Menus.swift:8-36,235-291`; `quit.quit = { NSApp.terminate(nil) }` at `:36`).
  The clone has no code that stops a child process at quit, because it has no child process yet.

### Where the clone hits it
The clone does not run a server today, so nothing leaks today. `20261005-embedded-server-runtime` starts one. The workaround in that ticket (item "Quit and crash"):
(1) an `NSApplication.willTerminateNotification` observer sends SIGTERM and waits up to 5 s for the child; (2) the app writes `embedded-server.pid` (pid, start time, executable path) and reaps a leftover server at the next launch
only when `proc_pidpath` matches the runtime's `t3`. The server has no parent-death watch (the ticket found no `ppid` or stdin-EOF handling in `apps/server/src`).
Differences a user can see: after a crash, a force quit or a kill of the app, the server keeps running (and keeps its port and pairing endpoint) until the next launch of the app. The reference's behavior after an app crash was not tested here.
Also unknown: whether a notification handler may block the main thread for 5 s without the OS or the host cutting it short.

## Why it must be resolved
The goal is a full clone, and "This machine" is the base of it. A clean stop is user-visible: Tailscale Serve mappings, the listening port and the activation socket must be gone after ⌘Q.
`20261005-embedded-server-runtime` carries a row "Quit and crash" (server gone within 5 s of ⌘Q; one server only after a crash). `20261005-this-machine-network-access` verifies Tailscale teardown on top of it.
`20261005-app-activation` needs the socket file removed at quit, but it already re-binds a stale file, so it is not blocked.
The workaround is accidental. It depends on AppKit behavior that the framework does not promise, and a later host change can break it without notice.
A supported hook also makes the 5 s bound testable by the framework's own checks.

## Requested support
Web analogy: none that can block. `pagehide` and `beforeunload` cannot delay a page for work. The request is native-specific, on the macOS host first.
- **A (preferred).** A module hook (for example `willQuit()` returning a promise or a completion) that the host calls from `applicationShouldTerminate`. It returns `.terminateLater` and replies when the module finishes or after a fixed budget (the app asks for 5 s).
  It runs for ⌘Q, the app menu, the Dock, `NSApp.terminate` and SIGTERM, and once only.
- **B.** The host guarantees that `destroy()` runs on every quit path, synchronously, with a stated time budget. The module then does the work in `destroy()`.
- **C.** Document the `willTerminateNotification` method as supported, with its blocking budget. This needs no code change and keeps the pid-file reaper as the only crash safety.
Other hosts: iOS has no quit; the web host needs nothing.

## How to reproduce
To confirm on the pinned `main` at `issue-open`.
1. Make a minimal app with a module whose `destroy()` appends a timestamp to a file and whose `willTerminateNotification` observer sleeps 3 s and appends another timestamp.
2. Run it as a lane build. Quit with ⌘Q (agent `key` operation or the app menu), then with `osascript -e 'tell application "<name>" to quit'`, then with `kill -TERM <pid>`.
3. Expected (reference behavior): the app stays alive until the module has finished (up to 5 s) and then exits; the file shows `destroy()` or the hook ran once.
   Actual (per `EXACT2-GAPS.md`): `.terminateNow`; whether `destroy()` runs at ⌘Q is unconfirmed. Record for each of the three paths whether the file has the timestamps and how long the process lived.

## Acceptance for the fix
- For ⌘Q, the app menu, the Dock, `osascript quit` and SIGTERM: a fake child (a script that traps SIGTERM and takes 3 s to exit) is gone before the app process exits, and the app exits within the budget.
- A child that ignores SIGTERM is killed after 2 s and the app still exits by 5 s. The hook runs once when quit is requested twice.
- With the module hook absent, the quit still returns at once (no regression for apps that do not use it).
- Test: an AppKit test with the fake child, plus an agent run that records the process list before and after.

## App adoption after resolution
In `20261005-embedded-server-runtime`: replace the `willTerminateNotification` observer with the supported hook; keep the pid file and the next-launch reaper as crash safety; update the "Quit and crash" row.
In `20261005-this-machine-network-access`: run the Tailscale teardown row through the hook. In `20261005-app-activation`: remove the socket in the same hook.
`issue-close` checks that no `willTerminateNotification` workaround remains and that the three rows pass on the pinned `main`. If the user chooses option C, close this issue by that decision and keep the notification method.

## Status and next action
Draft; not reproduced on the pinned `main`; not searched upstream; not published.
Next: `issue-open` (reproduce, search for duplicates, prepare the report for the user's approval; publication only after approval). The first measurement belongs to the spike in `20261005-embedded-server-runtime`.

## Resolved upstream (main #200) and adopted by embedded-server-runtime (2026-10-07)

Filed as [#105](https://github.com/ccheever/exact2/issues/105) and fixed by main PR #200 (`a091828a3`): the macOS
host's `applicationWillTerminate` destroys every live session, so each native module's `destroy()` runs synchronously
before the process ends, whichever way the quit came (⌘Q, the app menu, an Apple Event, the last window closing).
That is option **B** without a stated time budget; the host holds nothing itself, but a module that blocks in
`destroy()` holds the exit for as long as it blocks.

Adopted in [20261005-embedded-server-runtime](../tasks/closed/20261005-embedded-server-runtime.md): the embedded
server stops when the last session's module is destroyed (SIGTERM, SIGKILL after 2 s, at most 5 s,
`stopAllPoolInstances`); no `willTerminateNotification` observer was added. Measured on a lane copy: a quit through
`osascript … to quit` took the server down in 0.79 s and the app in 0.864 s; the server logged its SIGTERM exit
(code 130) 33 ms after the stop. The pid file and next-launch reaper stay as crash safety (`kill -9` of the app left
the server running with ppid 1 until the next launch stopped it).

Residual: the agent driver's end of drive (`ExactKit/Agent.swift` `exitAfterStorage`) calls `exit(0)` without
destroying sessions, so `destroy()` does not run there; the module also stops the server from `atexit`. The other
tickets this issue blocks (app-activation, app-update-feed, managed-codex-chatgpt, telemetry) adopt #200 in their
own work, so the issue stays open until they do.
