---
name: 20261005-x41-wsl-environments
plan: 20261005-t3code-macos-parity
status: draft
kind: scope-decision
blocks: [20261005-wsl-environments]
upstream_url: null
reproduced_on: null
---

# X41: WSL backends (Windows only)

## Summary

On Windows, T3 Code can run a second server inside a Windows Subsystem for Linux distro, or run only that server. The feature needs `wsl.exe`, so it exists only on Windows. On macOS the reference shows no WSL row and does nothing.
The clone targets macOS and hides the row the same way. The likely outcome is to close this issue as not applicable. The decision belongs to the user. Ticket `20261005-wsl-environments` is blocked until then.

## Why this issue arose

### The T3 Code behavior
- Availability. The desktop reports WSL as available only on Windows with `wsl.exe` present (`apps/desktop/src/wsl/DesktopWslEnvironment.ts:1154-1163`: `if (platform !== "win32") return false`).
  Settings → Connections shows the "WSL backend" row only when WSL is available, enabled, or in WSL-only mode (`apps/web/src/components/settings/ConnectionsSettings.logic.ts:14-20`); the settings-search entry is marked Windows-only (`apps/web/src/components/settings/settingsSearch.ts:810-821`).
- State and controls. The bridge exposes `getWslState`, `setWslBackendEnabled`, `setWslDistro` and `setWslOnly` (`packages/contracts/src/ipc.ts:1197-1200`). The state holds `enabled`, `distro` (null means "track the WSL default"), `available`, `wslOnly`, the distro list and a `preflightError` (`ipc.ts:553-572`).
- Dual mode. The Windows server stays primary. A second server is registered in the backend pool under the id `wsl:default` or `wsl:<distro>`, on its own loopback-only port (`apps/desktop/src/wsl/DesktopWslBackend.ts:1-60`).
  Changing the distro unregisters the old instance and registers a new one. Errors are logged and never fatal. A failed preflight (no Node, wrong version, missing build tools) is shown inline in Connections.
  The server runtime installs itself inside the distro; the first launch after an app update can take longer (`docs/user/install.md:79-85`). The packaged app carries a runtime archive that is installed into the distro (`DesktopBackendConfiguration.ts:650-660`, code comment).
  WSL resource telemetry is reported unavailable because the packaged monitor is a Windows executable (`DesktopBackendConfiguration.ts:644-648`).
- WSL-only mode. Only the WSL server runs, as primary. Turning it on needs an app restart. During a cold boot the app shows a "Connecting to WSL" splash instead of the main window (`apps/desktop/src/app/DesktopApp.ts:237-245`; `window/DesktopWindow.ts:881-924`).
  If preflight fails in this mode, a dialog appears and the app falls back to Windows (`DesktopWslBackend.ts:58,112`).
- Paths. Folder picking understands WSL UNC paths such as `\\wsl.localhost\<distro>\…` and maps them to Linux paths (`apps/desktop/src/wsl/wslPathParsing.ts`; used in `ipc/methods/window.ts`); distro names are validated; `wsl.exe -l` output is parsed as UTF-16 when needed.
  `wsl.exe` calls have timeouts and fixed messages, for example "WSL backend preflight could not start wsl.exe to probe for <subject>. Check that WSL is installed and the distro is accessible." (`DesktopWslEnvironment.ts:176-180`).
- Local environment off: "WSL backends stay off" (`docs/user/remote-access.md:229-232`).
- Reference tests: `DesktopWslBackend.test.ts`, `DesktopWslEnvironment.test.ts` (924 lines), `DesktopWslServerTree.test.ts`, `wslPathParsing.test.ts`.

### What exact2 does today
- Not in `EXACT2-GAPS.md`. Bundled library (`20261005-platforms-v3`): not covered (WSL and Windows hosts: **unknown**).
- The repository's `CLAUDE.md` names the hosts the agent driver and builds support: web, macos, ios, linux. It names no Windows host. This example is a macOS example (`examples/t3-code`, `app.json` `host.macos`).
- Observed in the clone on 2026-10-05: the catalog entry `wsl-backend` carries the flags `desktop,windows,localBackend,wsl` (`settings-catalog.ts:104`). The search filter treats `windows` and `wsl` as false (`settings-search.ts:46-56`: only `mac`, `localEnvironment`, and the connection-state and setting flags can be true),
  so the entry stays hidden even after `20261005-local-primary-environment` turns on `desktop` and `localBackend`. `20261005-this-machine-network-access` states "The T3 Connect and WSL rows are not drawn."

### Where the clone hits it
The clone has no WSL code and needs none to match the reference on macOS. Reference on macOS: `available` is false, so the row, the state controls and the splash never appear. A user of the clone sees the same.
Differences from the reference on Windows (which the clone does not target): everything above. There is no workaround and none is needed for macOS.
What a macOS build would mean: the feature has no macOS meaning. The reference gates it by platform and has no macOS counterpart, so the only parity row is "no WSL row on macOS", which already holds.

## Why it must be resolved
The goal is a full clone, and the user excluded WSL. The plan must not leave a blocked ticket open forever. The pending decision is: close as not applicable to macOS, or keep it as a future Windows task.
- Close (likely): record that the reference has no macOS behavior to match, close `20261005-wsl-environments`, keep the hidden catalog entry (it mirrors the reference's catalog).
- Keep: the work belongs to a Windows build of the example, which this plan does not target and cannot verify (no Windows host and no WSL here). It would need its own plan.
Nothing else in the plan waits on this issue.

## Requested support
None — product decision only. If the user wants a Windows build later, exact2 would need a Windows host, which the project instructions do not list; that is a different project.

## How to reproduce
Not a defect. To record at `issue-open`:
1. Run T3 Code (Nightly) on this Mac with a lane home and port. Open Settings, search for "WSL" and "Windows Subsystem". Expected: no result and no row in Connections.
2. Run the clone's lane build and do the same. Expected: the same.

## Acceptance for the fix
- If closed: the decision is recorded here and in the ticket, and the two searches above match. No code changes.
- If kept for Windows: a separate plan with a Windows host and WSL2 (names only), reference tests ported, and pixel and trace pairs from a Windows oracle.

## App adoption after resolution
If closed: set the ticket to closed with the decision; nothing in the app changes. If kept: the plan for a Windows build starts from the ticket's behavior list; `settings-catalog.ts:104` gets a real availability flag.
`issue-close` checks the decision text and the two searches.

## Status and next action
Draft; not reproduced; not searched upstream; not published. Decision pending; the recommendation is to close as not applicable, and the user decides.
Next: `issue-open` (record the two searches and prepare the report for the user's approval; publication only after approval), then ask the user to decide.
