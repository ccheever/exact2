---
name: 20261005-x45-app-relaunch
plan: 20261005-t3code-macos-parity
status: closed-upstream
kind: framework-gap (unconfirmed)
blocks: [20261005-local-primary-environment, 20261005-this-machine-network-access]
upstream_url: https://github.com/ccheever/exact2/issues/122
reproduced_on: null
rest_upstream_url: https://github.com/ccheever/exact2/issues/271
---

# X45: An app cannot relaunch itself

**Status (reclassified 2026-10-08):** Bucket 4, approved, no fix in progress: #271 (a process relaunch, after #269's hold); no PR.

## Summary
The T3 Code desktop app relaunches the whole app after the user changes the Local environment,
Network access or Tailscale HTTPS setting. Whether an exact2 app can relaunch itself is not
known (the bundled library does not cover it). The user decided (U4, 2026-10-05) that the clone
must behave the same as T3 Code, so the relaunch is required; until this issue is resolved,
the clone can only ship a restart-in-place stopgap, which is a visible difference.

## Why this issue arose
### The T3 Code behavior
Each of the three settings opens a confirm dialog whose action reads "Restart and turn off",
"Restart and turn on", "Restart and enable" or "Restart and disable". Confirming stores the
setting and calls the desktop `relaunch`: the window closes, the app quits, and a new app
process starts with the new setting; the server starts with the new exposure. The spinner
label is "Restarting…". Evidence: `apps/desktop/src/ipc/methods/localEnvironment.ts:20-29`,
`apps/desktop/src/backend/DesktopServerExposure.ts:97-130` (mode changes call `relaunch`),
`apps/web/src/components/settings/LocalEnvironmentSetting.tsx`, and
`apps/web/src/components/settings/ConnectionsSettings.tsx:2968-3000` (reference `1e2ecbd975`).
### What exact2 does today
Not covered by the bundled library (`20261005-platforms-v3`): **unknown**. `EXACT2-GAPS.md`
has no entry for it. To confirm at `issue-open`: whether a module or a host command can quit
the app and start a new instance of the same bundle, and what happens to window state and
pending writes.
### Where the clone hits it
`20261005-local-primary-environment` and `20261005-this-machine-network-access`. Stopgap
until this issue is resolved: stop and restart only the embedded server, then reconnect the
window (decision U4 requires the relaunch itself). The
user sees the window stay open with a reconnecting state instead of the app closing and
reopening; app-level state that the reference resets on relaunch (for example window frame
and in-memory UI state) is kept.

## Why it must be resolved
The goal is a complete clone. The relaunch is the reference's way to guarantee that every part
of the app (window, client state, server) starts clean with the new exposure. A restart in
place risks stale client state after an exposure change (cached endpoints, pairing links,
the primary environment's origin) — the clone already had a stale-origin bug of this kind
(round-12 F5).

## Requested support
A host command that relaunches the running app bundle (the equivalent of Electron's
`app.relaunch()` + `app.exit()`): it lets pending storage writes finish, quits, and starts the
same bundle again with the same arguments. macOS first; other hosts may refuse it.

## How to reproduce
Minimal app with a button that calls the relaunch command; press it and check that a new
process with a new pid shows the first frame. Expected (reference): new process, clean state.
Actual: to confirm on the pinned `main` at `issue-open`.

## Acceptance for the fix
Agent: press the control, then `state` shows a new launch id; `ps` shows one app process with a
new pid; a storage write made just before the press is present after relaunch.

## App adoption after resolution
`20261005-local-primary-environment` and `20261005-this-machine-network-access` replace the
restart-in-place path with the relaunch, and their confirm-dialog rows check the new process.
`issue-close` verifies it.

## Status and next action
Filed upstream as [#122](https://github.com/ccheever/exact2/issues/122); closed after main #170, which only
moves `reload()`'s log to stderr, so exact2 still has no process relaunch.
2026-10-07: `20261005-local-primary-environment` ships the stopgap behind `applyLocalSetting` (`this-machine.ts`):
off hands the focus to a saved environment and stops the embedded server; on starts it and connects; the window
stays. Its "Relaunch after a setting change" row stays blocked until a relaunch exists and is adopted.

## Rest filed upstream (2026-10-08)

Upstream (the rest): https://github.com/ccheever/exact2/issues/271 (#271, [Design] A host command that relaunches the app's process (rest of #122)). Reproduced on main `0365ad1a4` (relevant files unchanged on main `e200397ec`) before filing: `relaunch()` is `type-unknown-command`; `reload()` resets the session in the same pid; with `EXACT_DEV_MENU=0` `reload()` does nothing. One "Decision needed" comment. Searched open and closed issues and PRs: no duplicate.

## Decided upstream (2026-10-08): waits for main fix of #271 (after #269)

[Charlie on #271](https://github.com/ccheever/exact2/issues/271#issuecomment-6055586957): "Choose an explicit process relaunch command, after orderly quit is complete. … Depends on #269 and
durable-write teardown."
- Waits for main fix of [#271](https://github.com/ccheever/exact2/issues/271), which follows #269 (X6). The U4 relaunch rows stay blocked, and the
  restart-in-place stopgap stays, until then.
- [#327](https://github.com/ccheever/exact2/pull/327) audit (open on main, 2026-10-08): approved larger lifecycle feature, after #269.
