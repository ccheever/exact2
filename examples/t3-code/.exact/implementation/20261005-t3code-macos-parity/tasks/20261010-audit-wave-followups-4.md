---
name: 20261010-audit-wave-followups-4
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

# Differences the audit fix agents found outside their tasks (fourth set)

## Outcome

PRs #378 and #379 reported two more differences from the reference, outside their findings. This task fixes them as the
reference does. Earlier sets: [1](closed/20261010-audit-wave-followups.md), [2](closed/20261010-audit-wave-followups-2.md),
[3](closed/20261010-audit-wave-followups-3.md).

## Findings

| Id | Reference (T3 Code `1e2ecbd975`) | Clone | Found by | Steps |
| --- | --- | --- | --- | --- |
| FX-1 | Restore defaults lists the scope's settings (`useScopedSettings`): in a project scope the list names what that scope would reset. | `restoreLabels` lists the focused environment's settings, so in a project scope only the list differs (the reset itself matches since #379). | restore-defaults-agent-browser-access (#379) | Settings › scope a project with an override › Restore defaults; compare the dialog's list. |
| FX-2 | A menu keeps one highlight: moving with the keys from a row the pointer rests on moves the one highlight (Base UI's `highlightedIndex` serves both). | The clone's painted menus shade two rows (hover and focus) until the pointer moves. All painted menus share this hover model. | audit-wave-followups-3 (#378) | Pull Requests › Filters; rest the pointer on Author; ArrowDown; compare. Then another painted menu (a sidebar row menu, the scope menu). |

## Scope and exclusions

Included: the two rows. FX-2 is a shared model change: do it once in the shared menu pieces (`menu-keys.contract`,
`KeyMenu`, the painted menu rows), not per menu, and keep the native NSMenus untouched. Excluded: framework changes.

## Acceptance

| Row | How to verify | Before/after |
| --- | --- | --- |
| FX-1 | Bun test of the scoped label list | text |
| FX-2 | agent drive on two painted menus (hover a row, ArrowDown: one highlight); Bun/Contract test of the shared rule | before / after / reference image |

## Next action

Start after view-depth-under-test-stack merges (FX-2 touches many menu contracts).
