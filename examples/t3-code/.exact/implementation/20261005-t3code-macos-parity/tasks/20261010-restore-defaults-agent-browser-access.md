---
name: 20261010-restore-defaults-agent-browser-access
plan: 20261005-t3code-macos-parity
implementation: planned
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: null
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

## Next action

Start now.
