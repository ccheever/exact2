---
name: 20261009-fix-settings-integrations-loop
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified
delivery: merged
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-fix-settings-integrations-loop
pr_url: https://github.com/ccheever/exact2/pull/353
verified_commit: 950e8e2e5460194fb942440fd1009d62f86cbea8
---

# Settings › Integrations reads its config, settings and device state once, as the reference does

## Outcome

While Settings › Integrations was open, the app asked the server `server.getConfig`, `server.getSettings` and
`device.list` about 26 times a second each, and the runner stayed busy (a 3-s agent wait took 40 s). The page now reads
the connection's server config and its device-state stream, as the reference's `IntegrationsSettings.tsx` does: it sends
no request when it opens or answers again, and it changes when the server config or the device state changes, or after
"Check versions". The page shows the same rows as before.

Found by browser-surface-automation (#346), STATUS "Found, not in scope" (2026-10-09; measured 26.4/s on `d564a5c02`).

## Scope and exclusions

Included: how the `integrations` resource's answer (`integrationsPage`, `source-control-view.ts`) reads its data. Not
changed: the page's rows and the Contract (`app.contract` keeps the resource and its keys; browser-surface-profiles,
part 4, adds the Browser rows to the same page in parallel). The other Settings pages that still read
`server.getConfig`/`server.getSettings` per answer (Source control, Storage, Archive, Diagnostics, Scheduled tasks,
Keybindings) do not loop: nothing they read publishes back. They are listed under "Found, not changed".

## Context and guidance

- Reference (`1e2ecbd975`): `IntegrationsSettings.tsx` `DeviceIntegrationControls` reads `useScopedSettings()` (the
  scope's environment `serverConfig.settings`: `server.getConfig` when the connection starts, `rpc/session.ts`, then the
  `subscribeServerConfig` stream; `hooks/useSettings.ts`) and `useDeviceState(environmentId)` (`state/device.ts`: the
  `subscribeDeviceState` stream, `client-runtime/src/state/device.ts`). Only the "Check versions" button sends
  `device.list({ inspectOnly: true })` (`versionActions`, `IntegrationsSettings.tsx:731-746`).
- Server (`apps/server/src/ws.ts:3526-3540`, `device/DeviceService.ts:473-503`): `device.list` with `inspectOnly` runs
  `DeviceService.inspect`, which publishes each host's fresh summary (revision + 1) to every `subscribeDeviceState`
  stream. Its `settingsUpdated` config events and `server.getSettings` carry the same redacted settings
  (`redactServerSettingsForClient`).

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| finding | [browser-surface-automation](20261005-browser-surface-automation.md) "Live session 3" | #346 | Merged | `5f0ae7dca` |
| base | the feature branch after #351 (part 2 reverted until its checks) | #351 | Merged | `c603c22d6` |

## Cause and fix

Each `integrations` answer sent `device.list { inspectOnly: true }`. The server's inspection publishes a new device state
to every device-state stream, and the app always holds one: the shell opens it at connect (`useDeviceState`,
`r12-threads-device.ts` → `watchDevice`). Its event wakes `data` (the transport's `t3.events`), whose `drain` bumps
`client.revision`, and `integrations` is asked on `data.revision` (`app.contract`). So every answer asked for the next
one: getConfig, getSettings, device.list, an event, a new revision, again. Leaving Settings ended it (`active` false
returns at once). Not a framework limit.

Fix (`integrationsPage`): read `client.config` and its `settings` (the connection's `server.getConfig` at connect, kept
by `subscribeServerConfig`, as `useScopedSettings`), and the device state from the stream (`watchDevice`, a no-op once
the shell's stream is open, then `deviceStateOf`, as `useDeviceState`). "Check versions" (`rest:device-tools`
`action=check`) stays the one inspection; its result reaches the page through the stream. No throttle; the resource and
its keys are unchanged.

## Acceptance results

| Criterion | Result | Evidence |
| --- | --- | --- |
| Opening the page reads once and an answer never re-asks it (Bun, through the app's `answer()`) | pass: 1 ask, 0 server reads, idle; again after closing and reopening. Fails on the base: 12 asks (the test's limit), 36 reads, never idle | `settings-integrations-reads.test.ts` "opening the page answers once with no server reads, and its answer never asks it again"; [base vs fix](https://raw.githubusercontent.com/ccheever/exact2/e84540a35484258e3e15ced0fd7e15e87ce81db4/fix-settings-integrations-loop/test-base-vs-fix.txt) |
| It changes on the reference's events: `settingsUpdated`, a device state, Check versions | pass: each change is one ask with no read; Check versions sends one `device.list { inspectOnly: true }` and the page shows the stream's new version. Fails on the base (it loops) | same file, "it changes on the events the reference listens to…" |
| Live rate, same steps on base and branch (agent mode, 30 s open) | pass. Base `c603c22d6`: 25.5/s each of getConfig, getSettings and device.list over 61.0 s; the 3-s wait took 40.2 s and the 30-s wait 60.8 s. Branch: 0 reads in 30.1 s; the waits took 3.0 s and 30.0 s. After leaving Settings: 0 in both | [record](https://raw.githubusercontent.com/ccheever/exact2/e84540a35484258e3e15ced0fd7e15e87ce81db4/fix-settings-integrations-loop/settings-loop.txt), [pair](https://raw.githubusercontent.com/ccheever/exact2/e84540a35484258e3e15ced0fd7e15e87ce81db4/fix-settings-integrations-loop/01-integrations-before-after.png) |
| The page shows the same rows | pass: the device rows' text is the same in both runs' trees (Not installed, Required 0.12.0 / 0.21.12, Update to…, Check versions, No device hosts); the top of the page is the same in the pair | record, "The device rows read the same" |

Checks: see the PR ("Checks").

## Found, not changed

- `client.drain` bumps `client.revision` on every `data` answer, events or not, so every resource keyed on
  `data.revision` is asked again on any `t3.status`/`t3.events` wake. That is cheap for answers that read no server; the
  Settings pages listed in "Scope and exclusions" re-read `server.getConfig`/`server.getSettings` on each such wake (the
  reference reads both from the subscribed config). Not a loop: nothing they read publishes back.

## Progress

2026-10-09: cause found by reading the reference and the server, reproduced by the Bun test and live on the base;
fixed, tested and driven on the branch. The feature branch moved to `c603c22d6` (#351 reverted part 2) before the first
commit; the branch merged it, and both builds were made on that base. Draft PR #353 (fix `63d4c4be9`).

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| Bun test on the base's `source-control-view.ts` | `c603c22d6` | 2 fail (12 asks, never idle) | [base vs fix](https://raw.githubusercontent.com/ccheever/exact2/e84540a35484258e3e15ced0fd7e15e87ce81db4/fix-settings-integrations-loop/test-base-vs-fix.txt) | — |
| Live base-1 | evidence-base `c603c22d6` | setup failure, no app launched: the driver found no `rustc` under the lane HOME | record, "run base-1" | fixed in the drive script |
| Live base-2 (the retry) | evidence-base `c603c22d6`, under its `.build-lock` | the loop at 25.5/s per method | [record](https://raw.githubusercontent.com/ccheever/exact2/e84540a35484258e3e15ced0fd7e15e87ce81db4/fix-settings-integrations-loop/settings-loop.txt), [script](https://raw.githubusercontent.com/ccheever/exact2/e84540a35484258e3e15ced0fd7e15e87ce81db4/fix-settings-integrations-loop/drive.sh.txt) | — |
| Live branch-1 | `c603c22d6` + the fix | 0 reads while open; waits on time | same record | none |

## Next action

None. Merged by the coordinator on 2026-10-09 as `950e8e2e5` (#353, squash). The "Found, not changed" re-reads of the
other Settings pages are tracked in `tasks/20261009-settings-pages-subscribed-config.md`.
