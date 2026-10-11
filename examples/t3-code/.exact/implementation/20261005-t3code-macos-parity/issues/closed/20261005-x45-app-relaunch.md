---
name: 20261005-x45-app-relaunch
plan: 20261005-t3code-macos-parity
status: moved-to-main
kind: framework-gap (unconfirmed)
blocks: [20261005-local-primary-environment, 20261005-this-machine-network-access]
upstream_url: https://github.com/ccheever/exact2/issues/122
reproduced_on: null
rest_upstream_url: https://github.com/ccheever/exact2/issues/271
---

# X45: An app cannot relaunch itself

Moved to main `issues/20261009-native-process-relaunch.md` (2026-10-09); tracked there.

## Summary
The T3 Code desktop app relaunches the whole app after the user changes the Local environment,
Network access or Tailscale HTTPS setting. The user decided (U4, 2026-10-05) that the clone
must behave the same as T3 Code, so the relaunch is required; without it the clone ships a
restart-in-place stopgap, which is a visible difference.

## Why it arose
### The T3 Code behavior
Each of the three settings opens a confirm dialog whose action reads "Restart and turn off",
"Restart and turn on", "Restart and enable" or "Restart and disable". Confirming stores the
setting and calls the desktop `relaunch`: the window closes, the app quits, and a new app
process starts with the new setting; the server starts with the new exposure. The spinner
label is "Restarting…". Evidence: `apps/desktop/src/ipc/methods/localEnvironment.ts:20-29`,
`apps/desktop/src/backend/DesktopServerExposure.ts:97-130` (mode changes call `relaunch`),
`apps/web/src/components/settings/LocalEnvironmentSetting.tsx`, and
`apps/web/src/components/settings/ConnectionsSettings.tsx:2968-3000` (reference `1e2ecbd975`).
### Where the clone hit it
`20261005-local-primary-environment` and `20261005-this-machine-network-access`. A restart in
place risks stale client state after an exposure change (cached endpoints, pairing links, the
primary environment's origin); the clone already had a stale-origin bug of this kind (round-12 F5).

## Clone workaround
Stop and restart only the embedded server, then reconnect the window. Since 2026-10-07
[local-primary-environment](../../tasks/closed/20261005-local-primary-environment.md) ships it behind
`applyLocalSetting` (`this-machine.ts`): off hands the focus to a saved environment and stops the embedded
server; on starts it and connects; the window stays.
[this-machine-network-access](../../tasks/closed/20261005-this-machine-network-access.md) restarts the server in
place the same way. Declared difference: the window stays open with a reconnecting state instead of the app closing
and reopening, and app-level state the reference resets (window frame, in-memory UI state) is kept. The U4
"Relaunch after a setting change" rows stay blocked; once main has a process relaunch, both tasks replace the
restart-in-place path with it and their confirm-dialog rows check the new process.

## Evidence and history
- Filed as [#122](https://github.com/ccheever/exact2/issues/122); closed after main #170, which only moves
  `reload()`'s log to stderr, so no process relaunch.
- The rest filed as [#271](https://github.com/ccheever/exact2/issues/271) on 2026-10-08, reproduced on main
  `0365ad1a4`: `relaunch()` is `type-unknown-command`; `reload()` resets the session in the same pid; with
  `EXACT_DEV_MENU=0` `reload()` does nothing.
- Decision on #271 (2026-10-08): an explicit process relaunch command, after orderly quit (#269, X6).
