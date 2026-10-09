---
name: 20261010-restore-defaults-agent-browser-access
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified
delivery: draft-pr
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-restore-defaults-agent-browser-access
pr_url: null
verified_commit: null
---

# Restore defaults also lists and resets "Agent browser access"

## Outcome

Found by browser-surface-profiles (#354, merged as `a6e31eed7`), outside its findings. The reference's Settings ›
Restore defaults lists "Agent browser access" among the changed settings and resets it (`enableAgentBrowserAccess`, an
environment setting). The clone's dialog and reset leave it out. After this task both match the reference.

## Findings

| Id | Reference (T3 Code `1e2ecbd975`) | Clone | Steps |
| --- | --- | --- | --- |
| RD-1 | With Agent browser access changed from its default, Restore defaults' dialog names it ("This will reset: …, Agent browser access …") and Confirm resets it on the environment. | The dialog does not name it, and Confirm leaves it as it was. | Settings › Integrations (or where the switch lives) › change Agent browser access; Settings › Restore defaults; read the dialog; Confirm; read the switch. |

## Context and guidance

- Reference: `getChangedBrowserSettingLabels` and the restore path in `apps/web/src/components/settings/` (the
  Restore defaults handler), and where `enableAgentBrowserAccess` is written (an environment setting through
  `server.updateSettings`).
- Clone: `settings-core.ts` `restoreLabels` and `restoreDeviceDefaults` (#354 ported the browser labels),
  `browser-defaults.ts`, the environment settings write path.

## Acceptance

| Row | How to verify | Before/after |
| --- | --- | --- |
| RD-1 | Bun test of the label list and the reset write; one agent drive (change it, Restore defaults, read the dialog, Confirm, read the switch) | before / after / reference image |

## What the reference does (read and driven, lane `restore-defaults-agent-browser-access`, base port 16440)

`SettingsPanels.tsx` `useSettingsRestore` builds its list from the scoped settings: the device and environment rows,
"Text generation model", `getChangedBrowserSettingLabels`, then "Agent browser access" when
`enableAgentBrowserAccess` differs from `DEFAULT_UNIFIED_SETTINGS` (true). Confirm writes the default back with the
rest through `useUpdateScopedSettings` (the comment: "the confirmation dialog lists it by name, so a user restoring
defaults is told the agent regains access"). Driven over CDP (1280x840): Agent browser access off and Snooze limited
threads on read "This will reset: Snooze limited threads, Agent browser access."; Confirm turns the switch back on and
the key leaves the lane's `settings.json`.

## Cause and fix

`settings-core.ts` `restoreLabels` listed only `SERVER_LABELS` and the browser rows, and the restore write took its keys
from `SERVER_LABELS`, so the clone neither named nor reset the key; with only it changed, Restore stayed disabled
(`restoreCount` 0). Now `SERVER_DEFAULTS` carries `enableAgentBrowserAccess: true`, `restoreLabels` appends "Agent
browser access" after `changedBrowserSettingLabels` (the reference's order), and the restore write's keys
(`RESTORED_SERVER_KEYS`) include it, so Confirm patches it to true on the selected environments (a project scope clears
its override, as for the other project-scoped keys). No framework change; no new declared difference.

## Acceptance results

Agent mode, one session per build (lane env, 1280x840, the same steps: Settings › Integrations, dismiss the Nightly
notice, Agent browser access off; General, Snooze limited threads on; Restore device defaults; Confirm; Integrations).
Before: the evidence base `950e8e2e5` (lane `restore-defaults-agent-browser-access-before`); its restore code for this
row is the feature tip's (the Bun test below fails the same way on `0605c00d2`'s `settings-core.ts`). After: the branch
bundle at `d877f8e83` (lane `restore-defaults-agent-browser-access`). Both clone lanes point the codex and claude CLIs
at missing lane paths (`providers.*.binaryPath`), so no provider-update toast covers the switch. Reference: CDP,
1280x840.

| Row | Result | Evidence |
| --- | --- | --- |
| RD-1 | pass (agent mode + Bun test): the confirmation reads "This will reset: Snooze limited threads, Agent browser access.", word for word the reference's; Confirm turns Agent browser access back on (the tree's switch `accessibilityChecked: true`, the key gone from the lane's `settings.json`), as the reference. Before: "This will reset: Snooze limited threads." and the switch stays off (`enableAgentBrowserAccess: false` kept); with only Agent browser access changed, Restore device defaults stayed disabled. Bun test: the label after the browser rows, none while disconnected, the write `{ snoozeLimitedThreads: false, enableAgentBrowserAccess: true }`, the Integrations row checked after it, and alone: `restoreCount` 1, "This will reset: Agent browser access.", the write `{ enableAgentBrowserAccess: true }`. | [before/after/reference](https://raw.githubusercontent.com/ccheever/exact2/af95fae6be37ea6829607834d6232c7b6de441b0/restore-defaults-agent-browser-access/rd-1-restore-defaults.png), [Bun before/after](https://raw.githubusercontent.com/ccheever/exact2/c1fb63fc07726085324759e70ceebbda3aba6439/restore-defaults-agent-browser-access/rd-1-bun-before-after.txt), [trees and ARIA](https://raw.githubusercontent.com/ccheever/exact2/f621e5281f049e625387c985f6cdf80e3ec3a2d2/restore-defaults-agent-browser-access/rd-1-drive-trees.txt), [drive.sh](https://raw.githubusercontent.com/ccheever/exact2/8e9e7cfd2cb33f7a1731c9745a01e609af9dcb84/restore-defaults-agent-browser-access/drive.sh.txt), [compose.py](https://raw.githubusercontent.com/ccheever/exact2/720efe4326ab2d6f080ef51452929d93e33a52cf/restore-defaults-agent-browser-access/compose.py.txt) |

Tests: `settings-core.test.ts` "restore lists and re-grants Agent browser access after the browser rows (RD-1;
useSettingsRestore)" (new).

## Found during this task (not this task; for the coordinator)

- Restore defaults' list ignores the scope: `settings-core-view.ts` passes the focused environment's
  `client.config.settings` to `restoreLabels`, while the reference's `useScopedSettings` reads the representative
  target with project overrides applied. With All environments › a project selected and Agent browser access
  overridden off for it, the clone shows Restore disabled (`restoreCount` 0, a throwaway Bun probe), though its Confirm
  path would clear the override; the reference lists it. Every project-scoped key (Response streaming, Auto-settle…)
  is affected alike, on the base too. From source and that probe; not driven.

## Real-input batch steps

None: every row is agent mode.

## Progress

2026-10-10: the reference read and driven; the before drive settled in five runs on the evidence base (the Restore
button disabled with only the one key changed, then two startup toasts over the switch: the provider-update toast
removed by pointing the lane CLIs away, the Nightly notice dismissed as a step); built (one commit, its Bun test fails
on the old `settings-core.ts`); merged `origin/feat(example)/t3-code` at `e4647764f` (clean); the bundle built and the
branch driven once; the image composed and uploaded.

## Next action

Review and merge.
