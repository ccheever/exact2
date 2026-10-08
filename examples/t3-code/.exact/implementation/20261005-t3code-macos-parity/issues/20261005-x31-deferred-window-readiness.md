---
name: 20261005-x31-deferred-window-readiness
plan: 20261005-t3code-macos-parity
status: draft
kind: framework-gap (unconfirmed)
blocks: [20261005-local-primary-environment, 20261005-portable-app-download]
upstream_url: https://github.com/ccheever/exact2/issues/117
reproduced_on: null
---

# X31: Defer the first window until the app says it is ready

## Summary

The T3 Code desktop app opens its main window only after its embedded server is ready. Until then the user sees no window. With the local environment turned off, the window opens at once.
The exact2 macOS host creates the window at launch, and a module has no way to hold it back (not in `EXACT2-GAPS.md`; the plan found it while writing tickets; to confirm at `issue-open`).
The clone will show a connecting state in the first window instead. The clone needs a readiness gate for the first window, or the user must accept the difference.

## Why this issue arose

### The T3 Code behavior
- Start order. At bootstrap the app starts the primary backend and the activation socket and does not open a window (`apps/desktop/src/app/DesktopApp.ts:237-257`).
  The pool calls `handleBackendReady` when the server answers (`apps/desktop/src/backend/DesktopBackendPool.ts:288-302`). That sets a ready flag and calls `createMainIfBackendReady`
  (`apps/desktop/src/window/DesktopWindow.ts:984-988`).
- The gate: `waitingForBackend` is false when the flag is set; otherwise it equals the setting `localEnvironmentEnabled` (`DesktopWindow.ts:867-872`).
  `createMainIfBackendReady` does nothing while it waits (`:874-878`). The code comment gives the reason for the second case: with the local environment disabled "there is no backend to wait for" (`:867-868`).
  So: local environment on and server not ready, no window; local environment off, window at once.
- The window itself is created hidden and shown at `ready-to-show` (`DesktopWindow.ts:410,814`), so the first frame the user sees is the loaded app.
- Activation before ready. A Dock click or `activate` with no window does nothing while the backend is not ready; in WSL-only mode it shows a "Connecting to WSL" splash instead (`DesktopWindow.ts:961-981`; `DesktopApp.ts:237-245`). That splash is a Windows case and is out of scope (issue X41).
- The server stops or crashes after the window exists. `handleBackendNotReady` clears the flag "so a macOS dock click while the backend is down doesn't produce a stranded window pointing at nothing" (`DesktopWindow.ts:114-118,989-991`). The open window stays; the app reconnects.
- A window-open error after readiness is swallowed by the pool; the next `activate` retries it (`DesktopBackendPool.ts:288-302`; `DesktopWindow.ts:348-354,969-972`, code comments).
- How a permanently failing server is shown to the user, and the time from launch to the window, were not traced or measured for this issue.

### What exact2 does today
- Not in `EXACT2-GAPS.md`. Bundled library (`20261005-platforms-v3`): window creation and app lifecycle are **not covered: unknown**.
- `EXACT2-GAPS.md` section X27 (written from framework source at exact2 `c1522fdac`, checked against `main` `d2cb661eb`) describes the related window setup: "`viewport-fit=cover` has no title-row or traffic-light setting (`ExactMac/main.swift:209-223`);
  the frame autosave is restored before the final style (`:310-313`)." This shows the host builds and styles the window at launch. It does not say whether a module can defer the window.
- The plan's ticket `20261005-local-primary-environment` states "the Exact host creates it first" and that the clone "does not control host window creation". That is a planning statement, **not reproduced**. Confirm at `issue-open`.

### Where the clone hits it
Clone files: the connection phases live in `connections.ts` and `r12-sidebar-connections.ts`; the primary status comes from `localBackendStatus` (`20261005-embedded-server-runtime`).
The workaround: the first window opens at launch. `20261005-local-primary-environment` maps `localBackendStatus` onto the connection phases the rows already use: "Connecting" while the server installs or starts, "Reconnecting: <last exit reason>" while it restarts, "Connection failed: <reason>" when it fails, "Connected" when ready.
Differences a user sees: in the reference nothing appears until the app is ready, then the window appears in its final state; in the clone a window appears first and fills in a few seconds later.
The first-window pixel pair against the oracle must wait for "Connected", so the oracle comparison and the timing traces need an explicit ready wait. With the local environment off, the clone matches the reference.
Design note: `20261005-portable-app-download` shows a "Setting up T3 Code…" view in the window during the first-launch unpack. That view needs the window before the server is ready, so a deferral must let the app ask for the window earlier (see Requested support).

## Why it must be resolved
The goal is a full clone of the desktop app, including the launch experience. The first seconds are what every user sees first, and the oracle's first window is the reference for the pixel pairs.
Plan decision U5 chose the interim: accept a connecting state in the first window until this issue is resolved, or block the matrix row for the window. `20261005-local-primary-environment` carries that row.
Keeping the workaround costs a "Connecting" design in every page that can be first (sidebar, empty main area, settings) and a possible flash of empty chrome. It is also a permanent declared difference, and a declared difference is not an end state.

## Requested support
Web analogy: none that matches. A web app has no window to hold back. The closest are the manifest splash (`background_color`, icon) that the OS shows until first paint, and `document.prerendering` / `visibilityState` for a page that is loaded but not yet shown.
So the request is native-specific, on the macOS host first.
- **A (preferred).** An `app.json` field in the `host.macos.window` block (for example `showWhen: "ready"`) plus an app-side signal (a data source or module call, for example `ready`) that tells the host to show the window.
  The host loads the session without showing a window, shows it on the signal, and shows it anyway after a fixed timeout (so a dead server does not hide the app). A Dock click before the signal does nothing, as in the reference.
- **B.** The host always creates the window hidden (`orderOut`), and a module call (`showWindow()`) shows it. This allows the app to show the window early for the setup view and late for the normal start. It needs more code in each app.
- Both: no flash of the default frame (the saved frame from X27 applies before the first show), the window does not take focus from another app while hidden, and the signal can be sent again after a server restart without re-hiding the window.

## How to reproduce
To confirm on the pinned `main` at `issue-open`.
1. Make a minimal app whose data module reports "not ready" for 3 s after launch. Build it as a lane build and launch it (`bun host/apple/build.mjs <crate> --bundle --run`).
2. At t = 1 s run `bun scripts/agent.mjs macos tree` and count the app's windows (also `osascript -e 'tell application "System Events" to count windows of process "<name>"'`).
3. Expected (reference behavior): no window until the app is ready, then one window in its final state. Actual (to confirm): one window at once, showing whatever the page renders before the data is ready.
4. Reference side: launch T3 Code with its server delayed (use the oracle lane controls from `20261005-desktop-oracle-and-trace`) and record when the first window appears.

## Acceptance for the fix
- With the gate on and a module that signals ready at 3 s: no window at 1 s (window count 0 or all windows hidden), one window at 3 s that is already in its final state (first screenshot has no empty chrome).
- Signal never sent: the window appears at the timeout (the value is documented) and the page shows its failure state.
- A Dock click or `open -a` before the signal opens nothing and queues nothing; after the signal it behaves as today.
- The saved window frame applies on the first show (no jump from the default frame). A restart of the module after the first show does not hide the window.
- With the gate off (default), launch behavior is unchanged (a regression check in the repository's smoke).

## App adoption after resolution
`20261005-local-primary-environment`: set the gate in `app.json`, send the signal when `localBackendStatus.state` is `ready` or the local environment is off, and remove the interim "first window shows Connecting" row from the declared differences.
Keep the connecting and reconnecting states for restarts after the window exists (the reference keeps its window too). `20261005-portable-app-download`: signal early for the setup view, or use option B for it.
Update the oracle pairs so the first window needs no ready wait. `issue-close` checks the 1 s and 3 s window counts on the pinned `main`.

## Status and next action
Filed upstream as [#117](https://github.com/ccheever/exact2/issues/117) (open).
2026-10-07: `20261005-local-primary-environment` ships the interim (U5): the first window opens at launch and shows the connecting state until the primary connects; with the Local environment off it matches the reference. The window-gate rows of that task stay blocked on #117.
2026-10-08: U5 decided (user: match the original; `20261008-provisional-decisions-parity`): adopt the hold as soon as exact2
offers it; until then the interim stays with #117 as the blocker. Re-checked on main `f464bad43`: `scripts/app.schema.json`
`host.macos.window` allows only `width`, `height`, `minWidth`, `minHeight` (`additionalProperties: false`), and
`host/apple/Sources/ExactMac/main.swift` calls `window.makeKeyAndOrderFront(nil)` after the first boot with no condition;
#117 is open ("Decision needed", last comment re-checked on main `78286adc1`).
